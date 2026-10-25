use crate::{
    config::Config,
    keys::{Committee, derive},
    market::CreateMarket,
    push,
    secrets::Secrets,
    store, wallet,
};
use anyhow::{Context, Result, bail};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use ed25519_dalek::SigningKey;
use rand_core::{OsRng, RngCore};
use serde_json::{Value, json};
use sqlx::{Row, SqlitePool};
use std::{sync::Arc, time::Duration};
use tokio::sync::Mutex;

pub struct App {
    pub config: Config,
    _process_lock: std::fs::File,
    pub pool: SqlitePool,
    pub(crate) key: Secrets,
    pub(crate) signer: SigningKey,
    pub(crate) http: reqwest::Client,
    pub(crate) operation: Mutex<()>,
}
impl App {
    pub async fn new(config: Config) -> Result<Arc<Self>> {
        use std::os::unix::fs::OpenOptionsExt;
        if let Some(parent) = config.db.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let process_lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(config.db.with_extension("lock"))?;
        process_lock
            .try_lock()
            .context("coordinator database already locked by another process")?;
        let key = config.secrets()?;
        let pool = store::open(&config.db).await?;
        let coordinator = crate::secrets::read_private(&config.coordinator_key)?;
        let signer = SigningKey::from_bytes(
            coordinator
                .as_slice()
                .try_into()
                .context("COORDINATOR_KEY_FILE must contain 32 raw bytes")?,
        );
        for (name, path) in [
            ("envelope_sk", &config.envelope_key),
            ("hot_wallet_seed", &config.hot_seed),
        ] {
            if store::get_secret(&pool, &key, name).await?.is_none() {
                let encrypted = crate::secrets::read_private(path)?;
                let plaintext = key.open(name, &encrypted)?;
                if plaintext.len() != 32 {
                    bail!("invalid encrypted key length");
                }
                store::put_secret(&pool, &key, name, &plaintext).await?;
            }
        }
        Ok(Arc::new(Self {
            config,
            _process_lock: process_lock,
            pool,
            key,
            signer,
            http: push::client()?,
            operation: Mutex::new(()),
        }))
    }
    pub async fn rpc(&self, method: &str) -> Result<Value> {
        self.rpc_params(method, json!([])).await
    }
    pub async fn rpc_params(&self, method: &str, params: Value) -> Result<Value> {
        let response: Value = self
            .http
            .post(&self.config.rpc_url)
            .json(&json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}))
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            // The node URL may carry an access key: keep it out of logs.
            .map_err(reqwest::Error::without_url)?
            .json()
            .await
            .map_err(reqwest::Error::without_url)?;
        if response.get("error").is_some_and(|e| !e.is_null()) {
            bail!("NODE_UNAVAILABLE");
        }
        response.get("result").cloned().context("NODE_UNAVAILABLE")
    }
    pub(crate) async fn tip(&self) -> Result<u32> {
        self.rpc("getblockcount")
            .await?
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .context("NODE_UNAVAILABLE")
    }
    pub async fn create(&self, input: CreateMarket) -> Result<Value> {
        let _guard = self.operation.lock().await;
        let tip = self.tip().await?;
        let balance = if self.config.network == "mainnet" {
            self.sync_all().await?;
            Some(
                sqlx::query_scalar::<_, i64>("SELECT COALESCE(SUM(balance_zat),0) FROM market")
                    .fetch_one(&self.pool)
                    .await?
                    .try_into()?,
            )
        } else {
            None
        };
        input.validate(
            tip,
            self.config.min_bet_zat,
            balance,
            self.config.max_mainnet_demo_zat,
        )?;
        let committee_json: String =
            sqlx::query_scalar("SELECT public_json FROM committee LIMIT 1")
                .fetch_optional(&self.pool)
                .await?
                .context("committee init required")?;
        let committee: Committee = serde_json::from_str(&committee_json)?;
        let keys = derive(&committee, &self.config.network, input.outcomes.len())
            .context("ESCROW_DERIVATION_FAILED")?;
        let mut id = [0; 16];
        OsRng.fill_bytes(&mut id);
        let id = hex::encode(id);
        let envelope = store::get_secret(&self.pool, &self.key, "envelope_sk")
            .await?
            .context("envelope key missing")?;
        let secret = x25519_dalek::StaticSecret::from(<[u8; 32]>::try_from(envelope.as_slice())?);
        let definition = input.definition(
            &self.config,
            id.clone(),
            tip,
            &keys,
            URL_SAFE_NO_PAD.encode(x25519_dalek::PublicKey::from(&secret).to_bytes()),
        );
        let rules_hash = hex::encode(zcash_glass::hash::rules_hash(&definition)?);
        let imported = wallet::call(
            &self.config,
            json!({"op":"import","birthday_height":tip,"ufvk":keys.ufvk}),
        )
        .await
        .context("ESCROW_IMPORT_FAILED")?;
        let snapshot = STANDARD.decode(
            imported["snapshot_b64"]
                .as_str()
                .context("missing wallet snapshot")?,
        )?;
        sqlx::query("INSERT INTO market(market_id,definition_json,rules_hash,nk_enc,rivk_enc,ufvk_enc,wallet_enc,wallet_account_uuid) VALUES(?,?,?,?,?,?,?,?)")
   .bind(&id).bind(serde_json::to_string(&definition)?).bind(&rules_hash).bind(self.key.seal(&format!("{id}/nk"),&keys.nk)?).bind(self.key.seal(&format!("{id}/rivk"),&keys.rivk)?).bind(self.key.seal(&format!("{id}/ufvk"),keys.ufvk.as_bytes())?).bind(self.key.seal(&format!("{id}/wallet"),&snapshot)?).bind(imported["account_uuid"].as_str()).execute(&self.pool).await?;
        // Persist before network work, so the durable worker can retry after API or wallet outages.
        let _ = self.process_market(&id, tip).await;
        self.market(&id).await
    }
    async fn market(&self, id: &str) -> Result<Value> {
        let row=sqlx::query("SELECT definition_json,rules_hash,status,announce_txid,announce_height,balance_zat FROM market WHERE market_id=?").bind(id).fetch_optional(&self.pool).await?.context("MARKET_NOT_FOUND")?;
        Ok(
            json!({"market_definition":serde_json::from_str::<Value>(&row.get::<String,_>("definition_json"))?,"rules_hash":row.get::<String,_>("rules_hash"),"status":row.get::<String,_>("status"),"announce_txid":row.get::<Option<String>,_>("announce_txid"),"announce_height":row.get::<Option<i64>,_>("announce_height"),"balance_zat":row.get::<i64,_>("balance_zat")}),
        )
    }
    pub(crate) async fn sync_all(&self) -> Result<()> {
        let rows = sqlx::query("SELECT market_id,definition_json,wallet_enc FROM market")
            .fetch_all(&self.pool)
            .await?;
        for row in rows {
            let id: String = row.get("market_id");
            let def: zcash_glass::hash::MarketDefinition =
                serde_json::from_str(&row.get::<String, _>("definition_json"))?;
            let snapshot = self.key.open(
                &format!("{id}/wallet"),
                &row.get::<Vec<u8>, _>("wallet_enc"),
            )?;
            let response=wallet::call(&self.config,json!({"op":"sync","birthday_height":def.birthday_height,"snapshot_b64":STANDARD.encode(&snapshot)})).await?;
            let snapshot = STANDARD.decode(
                response["snapshot_b64"]
                    .as_str()
                    .context("missing synced wallet")?,
            )?;
            let balance = response["balance_zat"]
                .as_u64()
                .context("missing escrow balance")?;
            sqlx::query("UPDATE market SET wallet_enc=?,balance_zat=? WHERE market_id=?")
                .bind(self.key.seal(&format!("{id}/wallet"), &snapshot)?)
                .bind(i64::try_from(balance)?)
                .bind(&id)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }
    async fn hot_wallet(&self, tip: u32) -> Result<Value> {
        if let Some(snapshot) = store::get_secret(&self.pool, &self.key, "hot_wallet").await? {
            return Ok(json!({"snapshot_b64":STANDARD.encode(&snapshot)}));
        }
        let seed = store::get_secret(&self.pool, &self.key, "hot_wallet_seed")
            .await?
            .context("hot seed missing")?;
        let response = wallet::call(
            &self.config,
            json!({"op":"hot_init","birthday_height":tip,"seed_hex":hex::encode(&seed)}),
        )
        .await?;
        let snapshot = STANDARD.decode(
            response["snapshot_b64"]
                .as_str()
                .context("hot wallet snapshot missing")?,
        )?;
        let address = response["address"]
            .as_str()
            .context("hot wallet address missing")?;
        let mut transaction = self.pool.begin().await?;
        for (name, value) in [
            ("hot_wallet", snapshot.as_slice()),
            ("hot_address", address.as_bytes()),
            ("hot_birthday", &tip.to_le_bytes()),
        ] {
            sqlx::query("INSERT INTO secret(name,value_enc) VALUES(?,?)")
                .bind(name)
                .bind(self.key.seal(name, value)?)
                .execute(&mut *transaction)
                .await?;
        }
        transaction.commit().await?;
        Ok(response)
    }
    async fn process_market(&self, id: &str, tip: u32) -> Result<()> {
        let row = sqlx::query("SELECT * FROM market WHERE market_id=?")
            .bind(id)
            .fetch_one(&self.pool)
            .await?;
        let def: zcash_glass::hash::MarketDefinition =
            serde_json::from_str(&row.get::<String, _>("definition_json"))?;
        let hash: String = row.get("rules_hash");
        if !row.get::<bool, _>("pushed") {
            let body = serde_json::to_vec(&json!({"market_definition":def,"rules_hash":hash}))?;
            let mut url = self.config.ingest_url.clone();
            url.set_path(&format!("{}/markets", url.path().trim_end_matches('/')));
            push::market(&self.http, &self.signer, &url, &body).await?;
            sqlx::query("UPDATE market SET pushed=1 WHERE market_id=?")
                .bind(id)
                .execute(&self.pool)
                .await?;
        }
        let mut public_url = self.config.ingest_url.clone();
        let base = public_url
            .path()
            .strip_suffix("/ingest")
            .context("API_INGEST_URL must end /ingest")?
            .to_owned();
        public_url.set_path(&format!("{base}/markets/{id}"));
        if let Ok(response) = self.http.get(public_url).send().await
            && response.status().is_success()
        {
            let state: Value = response.json().await?;
            if let Some(status) = state["status"].as_str().filter(|s| {
                ["OPEN", "CLOSED", "VOID", "RESOLVED", "SETTLED", "RESOLVING"].contains(s)
            }) {
                sqlx::query("UPDATE market SET status=?,announce_height=? WHERE market_id=?")
                    .bind(status)
                    .bind(state["announce_height"].as_i64())
                    .bind(id)
                    .execute(&self.pool)
                    .await?;
                return Ok(());
            }
        }
        if tip
            > def
                .birthday_height
                .saturating_add(def.announce_timeout_blocks)
        {
            sqlx::query("UPDATE market SET status='VOID' WHERE market_id=?")
                .bind(id)
                .execute(&self.pool)
                .await?;
            return Ok(());
        }
        if row.get::<Option<String>, _>("announce_txid").is_some() {
            if let Some(pending) =
                store::get_secret(&self.pool, &self.key, &format!("{id}/pending_announcement"))
                    .await?
            {
                let mut request: Value = serde_json::from_slice(&pending)?;
                request["op"] = json!("broadcast");
                wallet::call(&self.config, request).await?;
            }
            return Ok(());
        }
        let hot = self.hot_wallet(tip).await?;
        let birthday = store::get_secret(&self.pool, &self.key, "hot_birthday")
            .await?
            .context("hot birthday missing")?;
        let seed = store::get_secret(&self.pool, &self.key, "hot_wallet_seed")
            .await?
            .context("hot seed missing")?;
        let response=wallet::call(&self.config,json!({"op":"announce","seed_hex":hex::encode(&seed),"birthday_height":u32::from_le_bytes(birthday.as_slice().try_into()?),"snapshot_b64":hot["snapshot_b64"],"address":def.outcomes[0].address,"amount_zat":zcash_glass::params::MIN_ANNOUNCE_ZAT,"memo":format!("FRSTM1:{hash}")})).await?;
        let snapshot = STANDARD.decode(
            response["snapshot_b64"]
                .as_str()
                .context("hot wallet snapshot missing")?,
        )?;
        let txid = response["txid"]
            .as_str()
            .context("announcement txid missing")?;
        let pending = json!({"op":"broadcast","birthday_height":u32::from_le_bytes(birthday.as_slice().try_into()?),"snapshot_b64":response["snapshot_b64"],"tx_hex":response["tx_hex"]});
        let mut transaction = self.pool.begin().await?;
        sqlx::query("INSERT INTO secret(name,value_enc) VALUES('hot_wallet',?) ON CONFLICT(name) DO UPDATE SET value_enc=excluded.value_enc").bind(self.key.seal("hot_wallet",&snapshot)?).execute(&mut *transaction).await?;
        let name = format!("{id}/pending_announcement");
        sqlx::query("INSERT INTO secret(name,value_enc) VALUES(?,?)")
            .bind(&name)
            .bind(self.key.seal(&name, &serde_json::to_vec(&pending)?)?)
            .execute(&mut *transaction)
            .await?;
        sqlx::query("UPDATE market SET announce_txid=? WHERE market_id=?")
            .bind(txid)
            .bind(id)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        wallet::call(&self.config, pending).await?;
        Ok(())
    }
    pub async fn work(self: Arc<Self>) {
        let mut backoff = Duration::from_secs(1);
        loop {
            let result = self.tick().await;
            if result.is_ok() {
                backoff = Duration::from_secs(1);
                tokio::time::sleep(self.config.sync_interval).await;
            } else {
                eprintln!("coordinator worker retry pending");
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(self.config.sync_interval);
            }
        }
    }
    async fn tick(&self) -> Result<()> {
        let _guard = self.operation.lock().await;
        let tip = self.tip().await?;
        self.sync_all().await?;
        let ids: Vec<String> =
            sqlx::query_scalar("SELECT market_id FROM market WHERE status='ANNOUNCING'")
                .fetch_all(&self.pool)
                .await?;
        for id in ids {
            self.process_market(&id, tip).await?;
        }
        self.settlement_tick(tip).await?;
        Ok(())
    }
}

struct Error(anyhow::Error);
impl<E: Into<anyhow::Error>> From<E> for Error {
    fn from(e: E) -> Self {
        Self(e.into())
    }
}
impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let message = if self
            .0
            .downcast_ref::<zcash_glass::payout::PayoutError>()
            .is_some_and(|e| matches!(e, zcash_glass::payout::PayoutError::FeeShortfall { .. }))
        {
            "FEE_SHORTFALL".to_string()
        } else {
            self.0.to_string()
        };
        if matches!(
            message.as_str(),
            "ESCROW_DERIVATION_FAILED" | "ESCROW_IMPORT_FAILED"
        ) {
            eprintln!("{message}");
        }
        let code = match message.as_str() {
            "INVALID_CLOSE_HEIGHT"
            | "INVALID_RESOLVE_HEIGHT"
            | "INVALID_RESOLUTION_PARAMS"
            | "INVALID_FEE_BPS"
            | "MAINNET_DEMO_LIMIT"
            | "MARKET_NOT_FOUND"
            | "NODE_UNAVAILABLE"
            | "MARKET_NOT_RESOLVED"
            | "PLAN_ALREADY_EXISTS"
            | "NOT_COMMITTEE_MARKET"
            | "COMMITTEE_NOT_OPEN"
            | "COMMITTEE_INVALID_VOTE"
            | "COMMITTEE_ALREADY_PROPOSED"
            | "COMMITTEE_NO_PROPOSAL"
            | "COMMITTEE_SAME_MEMBER"
            | "COMMITTEE_MISMATCH"
            | "FEE_SHORTFALL" => message.as_str(),
            _ => "INTERNAL",
        };
        let status = match code {
            "MARKET_NOT_FOUND" => StatusCode::NOT_FOUND,
            "NODE_UNAVAILABLE" => StatusCode::SERVICE_UNAVAILABLE,
            "INTERNAL" => StatusCode::INTERNAL_SERVER_ERROR,
            _ => StatusCode::BAD_REQUEST,
        };
        (status, Json(json!({"error":{"code":code,"message":code}}))).into_response()
    }
}
pub fn router(app: Arc<App>) -> Router {
    Router::new()
        .route("/committee", post(committee))
        .route("/markets", post(create))
        .route("/markets/{id}", get(market))
        .route("/hot-wallet", post(hot))
        .route("/markets/{id}/resolution", get(resolution_status))
        .route("/markets/{id}/resolution/{action}", post(resolution_vote))
        .route("/markets/{id}/payout", get(payout_status).post(payout_plan))
        .route("/markets/{id}/payout/follow-up", post(payout_follow_up))
        .route("/markets/{id}/payout/prepare", post(payout_prepare))
        .route("/markets/{id}/payout/submit", post(payout_submit))
        .with_state(app)
}
async fn committee(
    State(app): State<Arc<App>>,
    input: Result<Json<Committee>, JsonRejection>,
) -> Result<Json<Value>, Error> {
    let Json(c) = input.map_err(|_| anyhow::anyhow!("INVALID_RESOLUTION_PARAMS"))?;
    let _guard = app.operation.lock().await;
    if c.public_key_package.min_signers() != Some(2)
        || c.public_key_package.verifying_shares().len() != 3
        || !c.trusted_dealer
        || c.group_id
            != hex::encode(
                c.public_key_package
                    .verifying_key()
                    .serialize()
                    .map_err(|e| anyhow::anyhow!(e))?,
            )
    {
        return Err(anyhow::anyhow!("invalid committee").into());
    }
    let existing: Option<String> = sqlx::query_scalar("SELECT group_id FROM committee LIMIT 1")
        .fetch_optional(&app.pool)
        .await?;
    if existing.as_ref().is_some_and(|id| id != &c.group_id) {
        return Err(anyhow::anyhow!("committee already initialized").into());
    }
    sqlx::query(
        "INSERT INTO committee(group_id,public_json) VALUES(?,?) ON CONFLICT(group_id) DO NOTHING",
    )
    .bind(&c.group_id)
    .bind(serde_json::to_string(&c)?)
    .execute(&app.pool)
    .await?;
    Ok(Json(json!({"group_id":c.group_id,"trusted_dealer":true})))
}
async fn create(
    State(app): State<Arc<App>>,
    input: Result<Json<CreateMarket>, JsonRejection>,
) -> Result<Json<Value>, Error> {
    let Json(input) = input.map_err(|_| anyhow::anyhow!("INVALID_RESOLUTION_PARAMS"))?;
    Ok(Json(app.create(input).await?))
}
async fn market(State(app): State<Arc<App>>, Path(id): Path<String>) -> Result<Json<Value>, Error> {
    Ok(Json(app.market(&id).await?))
}
async fn hot(State(app): State<Arc<App>>) -> Result<Json<Value>, Error> {
    let _guard = app.operation.lock().await;
    let tip = app.tip().await?;
    app.hot_wallet(tip).await?;
    let address = store::get_secret(&app.pool, &app.key, "hot_address")
        .await?
        .context("hot address missing")?;
    Ok(Json(json!({"address":std::str::from_utf8(&address)?})))
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct VoteBody {
    member: String,
    outcome: Option<u32>,
    evidence_url: Option<String>,
}
async fn resolution_status(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, Error> {
    Ok(Json(app.committee_status(&id).await?))
}
async fn resolution_vote(
    State(app): State<Arc<App>>,
    Path((id, action)): Path<(String, String)>,
    input: Result<Json<VoteBody>, JsonRejection>,
) -> Result<Json<Value>, Error> {
    use crate::committee::Command;
    let Json(b) = input.map_err(|_| anyhow::anyhow!("COMMITTEE_INVALID_VOTE"))?;
    let vote = || -> anyhow::Result<(u32, String)> {
        Ok((
            b.outcome.context("COMMITTEE_INVALID_VOTE")?,
            b.evidence_url.clone().context("COMMITTEE_INVALID_VOTE")?,
        ))
    };
    let command = match action.as_str() {
        "propose" => {
            let (outcome, evidence_url) = vote()?;
            Command::Propose {
                member: b.member.clone(),
                outcome,
                evidence_url,
            }
        }
        "confirm" => {
            let (outcome, evidence_url) = vote()?;
            Command::Confirm {
                member: b.member.clone(),
                outcome,
                evidence_url,
            }
        }
        "withdraw" => Command::Withdraw {
            member: b.member.clone(),
        },
        _ => return Err(anyhow::anyhow!("COMMITTEE_INVALID_VOTE").into()),
    };
    Ok(Json(app.committee_vote(&id, command).await?))
}
async fn payout_status(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, Error> {
    Ok(Json(app.payout_status(&id).await?))
}
async fn payout_plan(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, Error> {
    Ok(Json(app.payout_plan(&id, false).await?))
}
async fn payout_follow_up(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, Error> {
    Ok(Json(app.payout_plan(&id, true).await?))
}

async fn payout_prepare(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, Error> {
    Ok(Json(
        app.payout_prepare(
            &id,
            body["plan_hash"].as_str().context("PLAN_HASH_MISMATCH")?,
        )
        .await?,
    ))
}
async fn payout_submit(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, Error> {
    Ok(Json(
        app.payout_submit(
            &id,
            body["plan_hash"].as_str().context("PLAN_HASH_MISMATCH")?,
            &body["signatures"],
        )
        .await?,
    ))
}

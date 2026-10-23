//! Private chain resolver and payout lifecycle. Only the explicitly built public body is pushed.
use crate::{push, service::App, store, wallet};
use anyhow::{Context, Result, bail};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use sqlx::Row;
use zcash_glass::{
    hash::{MarketDefinition, PlanKind, PlanResult, ResolutionParams},
    payout::{self, EnvelopeResult, FeeModel, NoteInput, PlanInput},
    status::BetStatus,
};

pub fn resolve(def: &MarketDefinition, block: &Value, info: &Value) -> Result<Value> {
    let hash = block["hash"].as_str().context("NODE_UNAVAILABLE")?;
    let (yes, evidence) = match &def.resolution_params {
        ResolutionParams::PoolThreshold(p) => {
            let value = block["valuePools"]
                .as_array()
                .context("NODE_UNAVAILABLE")?
                .iter()
                .find(|v| v["id"] == p.pool)
                .and_then(|v| v["chainValueZat"].as_u64())
                .context("NODE_UNAVAILABLE")?;
            let yes = match p.comparator.as_str() {
                "gte" => value >= p.threshold_zat,
                "lt" => value < p.threshold_zat,
                _ => bail!("INVALID_RESOLUTION_PARAMS"),
            };
            (
                yes,
                json!({"source":"getblock","height":def.resolve_height,"block_hash":hash,"pool":p.pool,"chain_value_zat":value}),
            )
        }
        ResolutionParams::UpgradeActive(p) => {
            let upgrades = info["upgrades"].as_object().context("NODE_UNAVAILABLE")?;
            let list=upgrades.values().map(|v|Ok(json!({"name":v["name"].as_str().context("NODE_UNAVAILABLE")?,"activation_height":v["activationheight"].as_u64().context("NODE_UNAVAILABLE")?}))).collect::<Result<Vec<_>>>()?;
            let yes = list.iter().any(|v| {
                v["name"] == p.upgrade_name
                    && v["activation_height"]
                        .as_u64()
                        .is_some_and(|h| h <= u64::from(def.resolve_height))
            });
            (
                yes,
                json!({"source":"getblockchaininfo","height":def.resolve_height,"block_hash":hash,"reported_upgrades":list}),
            )
        }
        _ => bail!("INVALID_RESOLUTION_PARAMS"),
    };
    Ok(
        json!({"market_id":def.market_id,"status":"RESOLVED","winning_outcome_index":if yes{0}else{1},"evidence":evidence,"votes":[]}),
    )
}
fn open_envelope(sk: &[u8; 32], memo: Option<&[u8]>) -> EnvelopeResult {
    let Some(payload) = memo.and_then(|m| zcash_glass::memo::Frst1Payload::from_bytes(m).ok())
    else {
        return EnvelopeResult::None;
    };
    match zcash_glass::envelope::open(sk, &payload) {
        Ok((receiver, bet_salt))
            if bool::from(orchard::Address::from_raw_address_bytes(&receiver).is_some()) =>
        {
            EnvelopeResult::Opened { receiver, bet_salt }
        }
        _ => EnvelopeResult::Unopenable,
    }
}
impl App {
    async fn indexed(&self) -> Result<(glass_indexer::Indexer, u32)> {
        if let Some(p) = self.config.indexer_db.parent() {
            std::fs::create_dir_all(p)?;
        }
        let idx = glass_indexer::Indexer::open(
            self.config
                .indexer_db
                .to_str()
                .context("invalid indexer path")?,
            &self.config.rpc_url,
            &self.config.network,
        )
        .await?;
        let defs: Vec<String> = sqlx::query_scalar("SELECT definition_json FROM market")
            .fetch_all(&self.pool)
            .await?;
        for def in defs {
            let d: MarketDefinition = serde_json::from_str(&def)?;
            let exists: bool =
                sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM market WHERE market_id=?)")
                    .bind(&d.market_id)
                    .fetch_one(idx.pool())
                    .await?;
            if !exists {
                idx.add_market(&def).await?;
            }
        }
        let height = idx.sync_once().await?;
        Ok((idx, height))
    }
    pub(crate) async fn settlement_tick(&self, tip: u32) -> Result<()> {
        let rows = sqlx::query(
            "SELECT market_id,definition_json FROM market WHERE status NOT IN ('ANNOUNCING')",
        )
        .fetch_all(&self.pool)
        .await?;
        if rows.is_empty() {
            return Ok(());
        }
        let (idx, _) = self.indexed().await?;
        for row in rows {
            let id: String = row.get("market_id");
            let def: MarketDefinition =
                serde_json::from_str(&row.get::<String, _>("definition_json"))?;
            let exists: bool =
                sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM resolution WHERE market_id=?)")
                    .bind(&id)
                    .fetch_one(&self.pool)
                    .await?;
            if !exists
                && tip >= def.resolve_height.saturating_add(def.min_confirmations)
                && !matches!(def.resolution_params, ResolutionParams::Committee(_))
            {
                let block = self
                    .rpc_params("getblock", json!([def.resolve_height.to_string(), 1]))
                    .await?;
                let info = if matches!(def.resolution_params, ResolutionParams::UpgradeActive(_)) {
                    self.rpc("getblockchaininfo").await?
                } else {
                    Value::Null
                };
                let mut body = resolve(&def, &block, &info)?;
                let winner = body["winning_outcome_index"]
                    .as_i64()
                    .context("invalid winner")?;
                let sum:i64=sqlx::query_scalar("SELECT COALESCE(SUM(amount_zat),0) FROM bet WHERE market_id=? AND status='COUNTED' AND outcome_index=?").bind(&id).bind(winner).fetch_one(idx.pool()).await?;
                if sum == 0 {
                    body["status"] = json!("VOID");
                    body["winning_outcome_index"] = Value::Null;
                }
                let mut transaction = self.pool.begin().await?;
                sqlx::query("INSERT INTO resolution(market_id,body_json) VALUES(?,?)")
                    .bind(&id)
                    .bind(serde_json::to_string(&body)?)
                    .execute(&mut *transaction)
                    .await?;
                sqlx::query("UPDATE market SET status=? WHERE market_id=?")
                    .bind(body["status"].as_str())
                    .bind(&id)
                    .execute(&mut *transaction)
                    .await?;
                transaction.commit().await?;
            }
        }
        let pending = sqlx::query("SELECT market_id,body_json FROM resolution WHERE pushed=0")
            .fetch_all(&self.pool)
            .await?;
        for row in pending {
            let id: String = row.get(0);
            let body: String = row.get(1);
            self.publish(&id, "resolution", body.as_bytes()).await?;
            sqlx::query("UPDATE resolution SET pushed=1 WHERE market_id=?")
                .bind(id)
                .execute(&self.pool)
                .await?;
        }
        self.monitor_payouts(tip).await?;
        Ok(())
    }
    async fn publish(&self, id: &str, kind: &str, body: &[u8]) -> Result<()> {
        let mut url = self.config.ingest_url.clone();
        url.set_path(&format!(
            "{}/markets/{id}/{kind}",
            url.path().trim_end_matches('/')
        ));
        push::market(&self.http, &self.signer, &url, body).await
    }
    pub async fn payout_status(&self, id: &str) -> Result<Value> {
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM market WHERE market_id=?)")
                .bind(id)
                .fetch_one(&self.pool)
                .await?;
        if !exists {
            bail!("MARKET_NOT_FOUND")
        }

        let body:Option<String>=sqlx::query_scalar("SELECT public_json FROM payout WHERE market_id=? ORDER BY created_at DESC,rowid DESC LIMIT 1").bind(id).fetch_optional(&self.pool).await?;
        Ok(body
            .map(|s| serde_json::from_str(&s))
            .transpose()?
            .unwrap_or(Value::Null))
    }
    pub async fn payout_plan(&self, id: &str, follow: bool) -> Result<Value> {
        let _guard = self.operation.lock().await;
        let active:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM payout WHERE market_id=? AND status NOT IN ('CONFIRMED','FAILED'))").bind(id).fetch_one(&self.pool).await?;
        if active {
            bail!("PLAN_ALREADY_EXISTS")
        }
        let row = sqlx::query("SELECT definition_json,status FROM market WHERE market_id=?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .context("MARKET_NOT_FOUND")?;
        let status: String = row.get("status");
        if !["RESOLVED", "VOID", "SETTLED"].contains(&status.as_str()) {
            bail!("MARKET_NOT_RESOLVED")
        }
        let previous: Option<i64> = sqlx::query_scalar(
            "SELECT MAX(cutoff_height) FROM payout WHERE market_id=? AND status='CONFIRMED'",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?;
        if follow && previous.is_none() {
            bail!("MARKET_NOT_RESOLVED")
        }
        if !follow && previous.is_some() {
            bail!("PLAN_ALREADY_EXISTS")
        }
        let def: MarketDefinition = serde_json::from_str(&row.get::<String, _>("definition_json"))?;
        self.sync_all().await?;
        let (idx, tip) = self.indexed().await?;
        let resolution: String =
            sqlx::query_scalar("SELECT body_json FROM resolution WHERE market_id=?")
                .bind(id)
                .fetch_one(&self.pool)
                .await?;
        let resolution: Value = serde_json::from_str(&resolution)?;
        let result = if follow || resolution["status"] == "VOID" {
            PlanResult::Void
        } else {
            PlanResult::Outcome(u32::try_from(
                resolution["winning_outcome_index"]
                    .as_u64()
                    .context("MARKET_NOT_RESOLVED")?,
            )?)
        };
        let rows=sqlx::query("SELECT * FROM bet WHERE market_id=? AND block_height>? AND block_height<=? ORDER BY note_ref").bind(id).bind(if follow{previous.unwrap()}else{-1}).bind(tip).fetch_all(idx.pool()).await?;
        let secret = store::get_secret(&self.pool, &self.key, "envelope_sk")
            .await?
            .context("missing envelope key")?;
        let sk: [u8; 32] = secret.as_slice().try_into()?;
        let mut notes = Vec::new();
        let mut inputs = Vec::new();
        let mut own_txids = std::collections::BTreeSet::new();
        let previous_plans =
            sqlx::query("SELECT plan_hash,private_enc FROM payout WHERE market_id=?")
                .bind(id)
                .fetch_all(&self.pool)
                .await?;
        for p in previous_plans {
            let h: String = p.get(0);
            let plain = self
                .key
                .open(&format!("payout/{h}"), &p.get::<Vec<u8>, _>(1))?;
            let saved: Value = serde_json::from_slice(&plain)?;
            for batch in saved["batches"]
                .as_array()
                .context("missing previous batches")?
            {
                if let Some(txid) = batch["txid"].as_str() {
                    own_txids.insert(txid.to_string());
                }
            }
        }
        for r in rows {
            let bet_id: String = r.get("bet_id");
            if bet_id
                .rsplit_once(':')
                .is_some_and(|(txid, _)| own_txids.contains(txid))
            {
                continue;
            }

            let announcement = r.get::<i64, _>("is_announcement") != 0;
            let state: String = r.get("status");
            let status = if announcement {
                BetStatus::InvalidMemo
            } else {
                serde_json::from_value(json!(state))?
            };
            let memo: Option<Vec<u8>> = r.get("memo_payload");
            let envelope = open_envelope(&sk, memo.as_deref());
            let amount = u64::try_from(r.get::<i64, _>("amount_zat"))?;
            if status != BetStatus::UneconomicDust {
                let bet: String = r.get("bet_id");
                let (txid, _) = bet.rsplit_once(':').context("invalid private note id")?;
                inputs.push(json!({"txid":txid,"action_index":r.get::<i64,_>("action_index"),"amount_zat":amount}));
            }
            notes.push(NoteInput {
                note_ref: r.get("note_ref"),
                bet_fingerprint: r.get("bet_fingerprint"),
                outcome_index: r
                    .get::<Option<i64>, _>("outcome_index")
                    .map(u32::try_from)
                    .transpose()?,
                amount_zat: amount,
                block_height: u32::try_from(r.get::<i64, _>("block_height"))?,
                status,
                is_announcement: announcement,
                envelope,
            });
        }
        let outcome = payout::plan(&PlanInput {
            market_id: id.into(),
            plan_kind: if follow {
                PlanKind::FollowUp
            } else {
                PlanKind::Standard
            },
            plan_cutoff_height: tip,
            result,
            fee_bps: def.fee_bps,
            refund_fee_zat: def.refund_fee_zat,
            fee_model: FeeModel::default(),
            notes,
        })?;
        if inputs.is_empty() {
            bail!("NO_PAYOUT_NOTES")
        }
        let hash = hex::encode(zcash_glass::hash::plan_hash(&outcome.public)?);
        let s = &outcome.summary;
        let public = json!({"plan_hash":hash,"status":"PLANNED","plan":outcome.public,"summary":{"winners":s.winners,"refunds":s.refunds,"unclaimable":s.unclaimable,"swept":s.swept,"uneconomic_dust":s.uneconomic_dust,"operator_fee_zat":s.operator_fee_zat,"tx_fee_zat":s.tx_fee_zat,"distributed_zat":s.distributed_zat},"signing":{"sessions_done":0,"sessions_total":s.spends+s.tx_count.saturating_sub(1),"threshold":2},"txs":[]});
        let mut outputs: Vec<Value> = outcome
            .private
            .outputs
            .iter()
            .map(|o| json!({"receiver_hex":hex::encode(o.receiver),"amount_zat":o.amount_zat}))
            .collect();
        if outcome.private.treasury_out_zat > 0 {
            outputs.push(json!({"receiver_hex":hex::encode(zcash_glass::address::parse_payout_ua(&def.treasury_address,&def.network)?),"amount_zat":outcome.private.treasury_out_zat}));
        }
        let private = json!({"inputs":inputs,"outputs":outputs,"fee_zat":s.tx_fee_zat,"batches":[],"remaining_fee_zat":s.tx_fee_zat});
        sqlx::query("INSERT INTO payout(plan_hash,market_id,public_json,private_enc,status,cutoff_height) VALUES(?,?,?,?,?,?)").bind(&hash).bind(id).bind(serde_json::to_string(&public)?).bind(self.key.seal(&format!("payout/{hash}"),&serde_json::to_vec(&private)?)?).bind("PLANNED").bind(tip).execute(&self.pool).await?;
        self.publish(id, "payout", &serde_json::to_vec(&public)?)
            .await?;
        Ok(public)
    }

    async fn save_payout_wallet(
        &self,
        id: &str,
        public: &Value,
        private: &Value,
        wallet: Option<&[u8]>,
    ) -> Result<()> {
        let hash = public["plan_hash"].as_str().context("invalid plan")?;
        let mut transaction = self.pool.begin().await?;
        if let Some(wallet) = wallet {
            sqlx::query("UPDATE market SET wallet_enc=? WHERE market_id=?")
                .bind(self.key.seal(&format!("{id}/wallet"), wallet)?)
                .bind(id)
                .execute(&mut *transaction)
                .await?;
        }
        sqlx::query("UPDATE payout SET public_json=?,private_enc=?,status=? WHERE plan_hash=? AND market_id=?").bind(serde_json::to_string(public)?).bind(self.key.seal(&format!("payout/{hash}"),&serde_json::to_vec(private)?)?).bind(public["status"].as_str()).bind(hash).bind(id).execute(&mut *transaction).await?;
        transaction.commit().await?;
        Ok(())
    }
    async fn private_payout(&self, id: &str, hash: &str) -> Result<(Value, Value)> {
        let row = sqlx::query(
            "SELECT public_json,private_enc FROM payout WHERE market_id=? AND plan_hash=?",
        )
        .bind(id)
        .bind(hash)
        .fetch_optional(&self.pool)
        .await?
        .context("PLAN_HASH_MISMATCH")?;
        let public = serde_json::from_str(&row.get::<String, _>("public_json"))?;
        let plain = self.key.open(
            &format!("payout/{hash}"),
            &row.get::<Vec<u8>, _>("private_enc"),
        )?;
        Ok((public, serde_json::from_slice(&plain)?))
    }
    pub async fn payout_prepare(&self, id: &str, hash: &str) -> Result<Value> {
        let _guard = self.operation.lock().await;
        let (mut public, mut private) = self.private_payout(id, hash).await?;
        if !["PLANNED", "SIGNING"].contains(&public["status"].as_str().unwrap_or("")) {
            bail!("PLAN_ALREADY_EXISTS")
        }
        let active = private["batches"]
            .as_array()
            .context("missing batches")?
            .last()
            .is_some_and(|b| b["confirmed"] != true);
        if !active {
            self.sync_all().await?;
            let row=sqlx::query("SELECT wallet_enc,wallet_account_uuid,definition_json FROM market WHERE market_id=?").bind(id).fetch_one(&self.pool).await?;
            let def: MarketDefinition =
                serde_json::from_str(&row.get::<String, _>("definition_json"))?;
            let snapshot = self.key.open(
                &format!("{id}/wallet"),
                &row.get::<Vec<u8>, _>("wallet_enc"),
            )?;
            let stage = next_stage(
                &mut private,
                &hex::encode(zcash_glass::address::parse_payout_ua(
                    &def.outcomes[0].address,
                    &def.network,
                )?),
            )?;
            let mut request = json!({"op":"payout_build","snapshot_b64":STANDARD.encode(&snapshot),"birthday_height":def.birthday_height,"inputs":stage["inputs"],"outputs":stage["outputs"],"fee_zat":stage["fee_zat"],"max_actions":zcash_glass::params::MAX_ACTIONS_PER_TX});
            let mut built = wallet::call(&self.config, request.clone()).await?;
            request.as_object_mut().unwrap().remove("snapshot_b64");
            let snapshot = STANDARD.decode(
                built["snapshot_b64"]
                    .as_str()
                    .context("missing built wallet snapshot")?,
            )?;
            built
                .as_object_mut()
                .context("invalid built payout")?
                .remove("snapshot_b64");
            built["build_request"] = request;
            built["carry"] = stage["carry"].clone();
            private["batches"].as_array_mut().unwrap().push(built);
            public["status"] = json!("SIGNING");
            self.save_payout_wallet(id, &public, &private, Some(&snapshot))
                .await?;
        } else if private["batches"].as_array().unwrap().last().unwrap()["txid"].is_string() {
            bail!("PLAN_ALREADY_EXISTS")
        }
        self.publish(id, "payout", &serde_json::to_vec(&public)?)
            .await?;
        let committee: String = sqlx::query_scalar("SELECT public_json FROM committee LIMIT 1")
            .fetch_one(&self.pool)
            .await?;
        Ok(
            json!({"public":public,"committee":serde_json::from_str::<Value>(&committee)?,"batches":private["batches"].as_array().context("missing batches")?.iter().rev().take(1).map(|b|json!({"signing_requests":b["signing_requests"]})).collect::<Vec<_>>()}),
        )
    }
    pub async fn payout_submit(&self, id: &str, hash: &str, signatures: &Value) -> Result<Value> {
        let _guard = self.operation.lock().await;
        let (mut public, mut private) = self.private_payout(id, hash).await?;
        if public["status"] != "SIGNING" {
            bail!("PLAN_ALREADY_EXISTS")
        }
        if private["batches"]
            .as_array()
            .and_then(|a| a.last())
            .is_some_and(|b| b["txid"].is_string())
        {
            self.monitor_payouts(self.tip().await?).await?;
            return self.payout_status(id).await;
        }
        let supplied = signatures.as_array().context("invalid signatures")?;
        let batches = private["batches"]
            .as_array_mut()
            .context("missing batches")?;
        if supplied.len() != 1 {
            bail!("invalid signature batches")
        }
        let row = sqlx::query(
            "SELECT wallet_enc,wallet_account_uuid,definition_json FROM market WHERE market_id=?",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?;
        let def: MarketDefinition = serde_json::from_str(&row.get::<String, _>("definition_json"))?;
        let mut snapshot = self
            .key
            .open(
                &format!("{id}/wallet"),
                &row.get::<Vec<u8>, _>("wallet_enc"),
            )?
            .to_vec();
        let previous_done = batches
            .iter()
            .filter(|b| b["confirmed"] == true)
            .map(|b| b["signing_requests"].as_array().map_or(0, Vec::len))
            .sum::<usize>();
        let current_total = batches
            .last()
            .and_then(|b| b["signing_requests"].as_array())
            .map_or(0, Vec::len);
        for (batch, sigs) in batches.iter_mut().rev().take(1).zip(supplied) {
            let extracted=wallet::call(&self.config,json!({"op":"payout_extract","birthday_height":def.birthday_height,"snapshot_b64":STANDARD.encode(&snapshot),"pczt_b64":batch["pczt_b64"],"signatures":sigs})).await?;
            snapshot = STANDARD.decode(
                extracted["snapshot_b64"]
                    .as_str()
                    .context("missing wallet snapshot")?,
            )?;
            batch["tx_hex"] = extracted["tx_hex"].clone();
            batch["txid"] = extracted["txid"].clone();
        }
        public["signing"]["sessions_done"] = json!(previous_done + current_total);
        // Save transaction and updated wallet BEFORE broadcast, including on restart.
        let mut tx = self.pool.begin().await?;
        sqlx::query("UPDATE market SET wallet_enc=? WHERE market_id=?")
            .bind(self.key.seal(&format!("{id}/wallet"), &snapshot)?)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE payout SET public_json=?,private_enc=? WHERE plan_hash=?")
            .bind(serde_json::to_string(&public)?)
            .bind(
                self.key
                    .seal(&format!("payout/{hash}"), &serde_json::to_vec(&private)?)?,
            )
            .bind(hash)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        self.monitor_payouts(self.tip().await?).await?;
        self.payout_status(id).await
    }
    async fn transaction_status(&self, txid: &str) -> Result<Option<Value>> {
        let response: Value = self
            .http
            .post(&self.config.rpc_url)
            .json(&json!({"jsonrpc":"2.0","id":1,"method":"getrawtransaction","params":[txid,1]}))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        if let Some(error) = response.get("error").filter(|e| !e.is_null()) {
            if error["code"].as_i64() == Some(-5) {
                return Ok(None);
            }
            bail!("NODE_UNAVAILABLE")
        }
        Ok(Some(
            response
                .get("result")
                .filter(|v| !v.is_null())
                .cloned()
                .context("NODE_UNAVAILABLE")?,
        ))
    }
    async fn mined_height(&self, observed: &Value) -> Result<u32> {
        if let Some(height) = observed["height"]
            .as_u64()
            .and_then(|h| u32::try_from(h).ok())
        {
            return Ok(height);
        }
        let hash = observed["blockhash"]
            .as_str()
            .context("missing confirmed transaction block hash")?;
        let header = self
            .rpc_params("getblockheader", json!([hash, true]))
            .await?;
        header["height"]
            .as_u64()
            .and_then(|h| u32::try_from(h).ok())
            .context("missing confirmed block height")
    }
    async fn monitor_payouts(&self, tip: u32) -> Result<()> {
        let rows = sqlx::query(
            "SELECT market_id,plan_hash FROM payout WHERE rowid IN (SELECT MAX(rowid) FROM payout GROUP BY market_id) AND status!='FAILED'",
        )
        .fetch_all(&self.pool)
        .await?;
        for row in rows {
            let id: String = row.get(0);
            let hash: String = row.get(1);
            let (mut public, mut private) = self.private_payout(&id, &hash).await?;
            let definition: String =
                sqlx::query_scalar("SELECT definition_json FROM market WHERE market_id=?")
                    .bind(&id)
                    .fetch_one(&self.pool)
                    .await?;
            let definition: MarketDefinition = serde_json::from_str(&definition)?;
            // A failed final push must be retried after local confirmation, without replaying old plans.
            if public["status"] == "CONFIRMED" {
                self.publish(&id, "payout", &serde_json::to_vec(&public)?)
                    .await?;
                continue;
            }
            let mut all = true;
            let mut expired = false;
            let mut txs = Vec::new();
            let mut carries = Vec::new();
            let mut updated_wallet = None;
            for batch in private["batches"]
                .as_array_mut()
                .context("missing batches")?
            {
                let txid_owned = batch["txid"].as_str().map(str::to_owned);
                let Some(txid) = txid_owned.as_deref() else {
                    all = false;
                    continue;
                };
                let observed = self.transaction_status(txid).await?;
                let confirmations = observed
                    .as_ref()
                    .and_then(|v| v["confirmations"].as_u64())
                    .unwrap_or(0);
                if confirmations >= u64::from(definition.min_confirmations) {
                    if batch["confirmed"] != true {
                        if let Some(carry) = batch.get("carry").filter(|v| !v.is_null()) {
                            let output = batch["payout_outputs"]
                                .as_array()
                                .context("missing output metadata")?
                                .iter()
                                .find(|o| {
                                    o["receiver_hex"] == carry["receiver_hex"]
                                        && o["amount_zat"] == carry["amount_zat"]
                                })
                                .context("missing carry output")?;
                            carries.push(json!({"txid":txid,"action_index":output["action_index"],"amount_zat":output["amount_zat"]}));
                        }
                        batch["confirmed"] = json!(true);
                    }
                    txs.push(json!({"txid":txid,"status":"CONFIRMED","confirmed_height":self.mined_height(observed.as_ref().context("missing observed confirmed transaction")?).await?}));
                } else {
                    all = false;
                    if observed.is_none()
                        && batch["expiry_height"]
                            .as_u64()
                            .is_some_and(|h| u64::from(tip) > h)
                    {
                        expired = true;
                        break;
                    }
                    if observed.is_none() {
                        let sent = self
                            .rpc_params(
                                "sendrawtransaction",
                                json!([batch["tx_hex"]
                                    .as_str()
                                    .context("missing durable transaction")?]),
                            )
                            .await?;
                        if sent.as_str() != Some(txid) {
                            bail!("broadcast txid mismatch")
                        }
                    }
                    txs.push(json!({"txid":txid,"status":"BROADCAST","confirmed_height":null}));
                }
            }
            for carry in carries {
                private["inputs"]
                    .as_array_mut()
                    .context("missing pending inputs")?
                    .push(carry);
            }
            let remaining = !private["inputs"]
                .as_array()
                .context("missing inputs")?
                .is_empty()
                || !private["outputs"]
                    .as_array()
                    .context("missing outputs")?
                    .is_empty();
            if expired {
                let last = private["batches"]
                    .as_array_mut()
                    .unwrap()
                    .last_mut()
                    .context("missing expired stage")?;
                let request = last["build_request"].clone();
                let carry = last["carry"].clone();
                let row = sqlx::query("SELECT wallet_enc FROM market WHERE market_id=?")
                    .bind(&id)
                    .fetch_one(&self.pool)
                    .await?;
                let snapshot = self.key.open(
                    &format!("{id}/wallet"),
                    &row.get::<Vec<u8>, _>("wallet_enc"),
                )?;
                let mut request = request;
                request["snapshot_b64"] = json!(STANDARD.encode(&snapshot));
                let mut rebuilt = wallet::call(&self.config, request.clone()).await?;
                let snapshot = STANDARD.decode(
                    rebuilt["snapshot_b64"]
                        .as_str()
                        .context("missing rebuilt wallet snapshot")?,
                )?;
                updated_wallet = Some(snapshot);
                rebuilt
                    .as_object_mut()
                    .context("invalid rebuilt payout")?
                    .remove("snapshot_b64");
                request.as_object_mut().unwrap().remove("snapshot_b64");
                rebuilt["build_request"] = request;
                rebuilt["carry"] = carry;
                *last = rebuilt;
                public["status"] = json!("SIGNING");
                public["txs"] = json!(txs);
                public["signing"]["sessions_done"] = json!(
                    private["batches"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter(|b| b["confirmed"] == true)
                        .map(|b| b["signing_requests"].as_array().map_or(0, Vec::len))
                        .sum::<usize>()
                );
            } else if !txs.is_empty()
                && private["batches"]
                    .as_array()
                    .unwrap()
                    .last()
                    .is_none_or(|b| b["txid"].is_string())
            {
                public["status"] = json!(if all && !remaining {
                    "CONFIRMED"
                } else if all {
                    "PLANNED"
                } else {
                    "BROADCAST"
                });
                public["txs"] = json!(txs);
            }
            self.save_payout_wallet(&id, &public, &private, updated_wallet.as_deref())
                .await?;
            if public["status"] == "CONFIRMED" {
                sqlx::query("UPDATE market SET status='SETTLED' WHERE market_id=?")
                    .bind(&id)
                    .execute(&self.pool)
                    .await?;
            }
            self.publish(&id, "payout", &serde_json::to_vec(&public)?)
                .await?;
        }
        Ok(())
    }
}

/// Partition exact notes into bounded steps. Intermediate outputs return to escrow, never Treasury.
fn next_stage(private: &mut Value, carry_receiver: &str) -> Result<Value> {
    let max = zcash_glass::params::MAX_ACTIONS_PER_TX as usize;
    let mut inputs = private["inputs"]
        .as_array()
        .context("missing inputs")?
        .clone();
    let mut outputs = private["outputs"]
        .as_array()
        .context("missing outputs")?
        .clone();
    inputs.sort_by(|a, b| {
        b["amount_zat"]
            .as_u64()
            .cmp(&a["amount_zat"].as_u64())
            .then_with(|| a["txid"].as_str().cmp(&b["txid"].as_str()))
            .then_with(|| a["action_index"].as_u64().cmp(&b["action_index"].as_u64()))
    });
    let consolidating = inputs.len() > max;
    let distributing = !consolidating && outputs.len() > max;
    let chosen = inputs.drain(..inputs.len().min(max)).collect::<Vec<_>>();
    let mut payees = if consolidating {
        vec![]
    } else if distributing {
        outputs.drain(..max - 1).collect()
    } else {
        std::mem::take(&mut outputs)
    };
    let intermediate = consolidating || distributing;
    let fee = if intermediate {
        u64::try_from(max)? * zcash_glass::params::MARGINAL_FEE_ZAT
    } else {
        private["remaining_fee_zat"]
            .as_u64()
            .context("missing fee")?
    };
    let sum = |items: &[Value]| {
        items.iter().try_fold(0u64, |a, v| {
            a.checked_add(v["amount_zat"].as_u64().context("invalid amount")?)
                .context("amount overflow")
        })
    };
    let total = sum(&chosen)?;
    let used = sum(&payees)?.checked_add(fee).context("amount overflow")?;
    let carry = if intermediate {
        let amount = total
            .checked_sub(used)
            .filter(|v| *v > 0)
            .context("FEE_SHORTFALL")?;
        json!({"receiver_hex":carry_receiver,"amount_zat":amount})
    } else {
        if total != used {
            bail!("payout stage value is not conserved")
        };
        Value::Null
    };
    if intermediate {
        payees.push(carry.clone());
    }
    private["inputs"] = json!(inputs);
    private["outputs"] = json!(outputs);
    private["remaining_fee_zat"] = json!(
        private["remaining_fee_zat"]
            .as_u64()
            .context("missing fee")?
            .checked_sub(fee)
            .context("FEE_SHORTFALL")?
    );
    Ok(json!({"inputs":chosen,"outputs":payees,"fee_zat":fee,"carry":carry}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct MockNode {
        mode: std::sync::atomic::AtomicUsize,
        sends: std::sync::atomic::AtomicUsize,
        pushes: std::sync::Mutex<Vec<Value>>,
    }
    async fn lifecycle_fixture() -> (
        std::sync::Arc<App>,
        tempfile::TempDir,
        std::sync::Arc<MockNode>,
        tokio::task::JoinHandle<()>,
    ) {
        use crate::{
            config::Config,
            secrets::{Secrets, write_private_new},
        };
        use axum::{Json, Router, extract::State, routing::post};
        use std::os::unix::fs::PermissionsExt;
        use std::{
            sync::{Arc, atomic::Ordering},
            time::Duration,
        };
        async fn rpc(State(state): State<Arc<MockNode>>, Json(body): Json<Value>) -> Json<Value> {
            let mode = state.mode.load(Ordering::SeqCst);
            Json(match body["method"].as_str().unwrap() {
                "getrawtransaction" => {
                    if mode == 1 {
                        json!({"error":{"code":-28,"message":"warming up"}})
                    } else if mode == 5 {
                        json!({"result":{"height":4,"confirmations":1},"error":null})
                    } else if mode == 3 {
                        json!({"result":{"confirmations":0},"error":null})
                    } else {
                        json!({"error":{"code":-5,"message":"not found"}})
                    }
                }
                "sendrawtransaction" => {
                    state.sends.fetch_add(1, Ordering::SeqCst);
                    if mode == 2 {
                        json!({"error":{"code":-28}})
                    } else {
                        json!({"result":"11".repeat(32),"error":null})
                    }
                }
                _ => panic!("unexpected RPC"),
            })
        }
        async fn ingest(
            State(state): State<Arc<MockNode>>,
            Json(body): Json<Value>,
        ) -> axum::http::StatusCode {
            state.pushes.lock().unwrap().push(body);
            if state.mode.load(Ordering::SeqCst) == 4 {
                axum::http::StatusCode::SERVICE_UNAVAILABLE
            } else {
                axum::http::StatusCode::OK
            }
        }
        let state = Arc::new(MockNode::default());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let bind = listener.local_addr().unwrap();
        let server = axum::serve(
            listener,
            Router::new()
                .route("/rpc", post(rpc))
                .route("/v1/ingest/markets/{id}/payout", post(ingest))
                .with_state(state.clone()),
        );
        let task = tokio::spawn(async move { server.await.unwrap() });
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write_private_new(&root.join("master"), &[8; 32]).unwrap();
        let master = Secrets::load(&root.join("master")).unwrap();
        write_private_new(&root.join("coordinator"), &[9; 32]).unwrap();
        write_private_new(
            &root.join("envelope"),
            &master.seal("envelope_sk", &[10; 32]).unwrap(),
        )
        .unwrap();
        write_private_new(
            &root.join("hot"),
            &master.seal("hot_wallet_seed", &[11; 32]).unwrap(),
        )
        .unwrap();
        let helper = root.join("helper");
        let response = json!({"snapshot_b64":STANDARD.encode(b"rebuilt wallet"),"pczt_b64":"rebuilt-pczt","signing_requests":[{},{}],"payout_outputs":[],"expiry_height":100,"actual_actions":2});
        std::fs::write(
            &helper,
            format!("#!/bin/sh\ncat >/dev/null\nprintf '%s' '{}'\n", response),
        )
        .unwrap();
        std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o700)).unwrap();
        let config = Config {
            bind,
            network: "regtest".into(),
            db: root.join("coordinator.db"),
            indexer_db: root.join("indexer.db"),
            secrets_key: root.join("master"),
            coordinator_key: root.join("coordinator"),
            envelope_key: root.join("envelope"),
            hot_seed: root.join("hot"),
            treasury: "not used".into(),
            rpc_url: format!("http://{bind}/rpc"),
            zaino_url: "not used".into(),
            ingest_url: format!("http://{bind}/v1/ingest").parse().unwrap(),
            helper,
            min_bet_zat: 1_000_000,
            min_confirmations: 1,
            max_mainnet_demo_zat: 50_000_000,
            sync_interval: Duration::from_secs(30),
        };
        let app = App::new(config).await.unwrap();
        let id = "00112233445566778899aabbccddeeff";
        sqlx::query("INSERT INTO market(market_id,definition_json,rules_hash,nk_enc,rivk_enc,ufvk_enc,wallet_enc,status) VALUES(?,?,?,?,?,?,?,'RESOLVED')").bind(id).bind(serde_json::to_string(&definition()).unwrap()).bind("rules").bind(vec![0]).bind(vec![0]).bind(vec![0]).bind(app.key.seal(&format!("{id}/wallet"),b"old wallet").unwrap()).execute(&app.pool).await.unwrap();
        let public = json!({"plan_hash":"22".repeat(32),"status":"BROADCAST","signing":{"sessions_done":2,"sessions_total":2},"txs":[{"txid":"11".repeat(32),"status":"BROADCAST","confirmed_height":null}]});
        let private = json!({"inputs":[],"outputs":[],"remaining_fee_zat":0,"batches":[{"pczt_b64":"old-pczt","tx_hex":"abcd","txid":"11".repeat(32),"expiry_height":10,"signing_requests":[{},{}],"payout_outputs":[],"build_request":{"op":"payout_build","birthday_height":1,"inputs":[],"outputs":[],"fee_zat":10000,"max_actions":50},"carry":null}]});
        let hash = "22".repeat(32);
        sqlx::query("INSERT INTO payout(plan_hash,market_id,public_json,private_enc,status,cutoff_height) VALUES(?,?,?,?,'BROADCAST',1)").bind(&hash).bind(id).bind(public.to_string()).bind(app.key.seal(&format!("payout/{hash}"),&serde_json::to_vec(&private).unwrap()).unwrap()).execute(&app.pool).await.unwrap();
        (app, dir, state, task)
    }
    #[tokio::test]
    async fn confirmation_policy_uses_locked_market_definition_not_current_environment() {
        use std::sync::atomic::Ordering;
        let (app, _dir, state, task) = lifecycle_fixture().await;
        assert_eq!(app.config.min_confirmations, 1);
        state.mode.store(5, Ordering::SeqCst);
        app.monitor_payouts(5).await.unwrap();
        let (public, _) = app
            .private_payout("00112233445566778899aabbccddeeff", &"22".repeat(32))
            .await
            .unwrap();
        assert_eq!(public["status"], "BROADCAST");
        task.abort();
    }
    #[tokio::test]
    async fn second_coordinator_cannot_open_same_database() {
        let (app, _dir, _state, task) = lifecycle_fixture().await;
        let second = App::new(app.config.clone()).await;
        assert!(second.err().unwrap().to_string().contains("already locked"));
        task.abort();
    }
    #[tokio::test]
    async fn mined_height_uses_observed_block_not_stale_tick_tip() {
        let (app, _dir, _state, task) = lifecycle_fixture().await;
        // A tick begun at 989 may observe three confirmations after mining reaches 992.
        assert_eq!(
            app.mined_height(&json!({"height":990,"confirmations":3}))
                .await
                .unwrap(),
            990
        );
        task.abort();
    }
    #[tokio::test]
    async fn expired_stage_rebuilds_same_hash_resets_sessions_and_saves_fresh_wallet() {
        let (app, _dir, state, task) = lifecycle_fixture().await;
        app.monitor_payouts(11).await.unwrap();
        let (public, private) = app
            .private_payout("00112233445566778899aabbccddeeff", &"22".repeat(32))
            .await
            .unwrap();
        assert_eq!(public["plan_hash"], "22".repeat(32));
        assert_eq!(public["status"], "SIGNING");
        assert_eq!(public["signing"]["sessions_done"], 0);
        assert_eq!(private["batches"][0]["pczt_b64"], "rebuilt-pczt");
        assert!(private["batches"][0]["txid"].is_null());
        assert_eq!(private["batches"][0]["expiry_height"], 100);
        let cipher: Vec<u8> = sqlx::query_scalar("SELECT wallet_enc FROM market")
            .fetch_one(&app.pool)
            .await
            .unwrap();
        assert_eq!(
            &*app
                .key
                .open("00112233445566778899aabbccddeeff/wallet", &cipher)
                .unwrap(),
            b"rebuilt wallet"
        );
        assert_eq!(state.pushes.lock().unwrap()[0]["status"], "SIGNING");
        task.abort();
    }
    #[tokio::test]
    async fn rpc_errors_never_mean_missing_and_broadcast_retry_keeps_durable_raw_bytes() {
        use std::sync::atomic::Ordering;
        let (app, _dir, state, task) = lifecycle_fixture().await;
        state.mode.store(1, Ordering::SeqCst);
        assert!(app.monitor_payouts(5).await.is_err());
        assert_eq!(state.sends.load(Ordering::SeqCst), 0);
        state.mode.store(2, Ordering::SeqCst);
        assert!(app.monitor_payouts(5).await.is_err());
        assert_eq!(state.sends.load(Ordering::SeqCst), 1);
        let (_, before) = app
            .private_payout("00112233445566778899aabbccddeeff", &"22".repeat(32))
            .await
            .unwrap();
        state.mode.store(3, Ordering::SeqCst);
        app.monitor_payouts(5).await.unwrap();
        let (_, after) = app
            .private_payout("00112233445566778899aabbccddeeff", &"22".repeat(32))
            .await
            .unwrap();
        assert_eq!(
            before["batches"][0]["tx_hex"],
            after["batches"][0]["tx_hex"]
        );
        assert_eq!(after["batches"][0]["tx_hex"], "abcd");
        assert_eq!(state.sends.load(Ordering::SeqCst), 1);
        task.abort();
    }
    #[tokio::test]
    async fn final_confirmation_push_is_retried_after_api_outage() {
        use std::sync::atomic::Ordering;
        let (app, _dir, state, task) = lifecycle_fixture().await;
        let (mut public, private) = app
            .private_payout("00112233445566778899aabbccddeeff", &"22".repeat(32))
            .await
            .unwrap();
        public["status"] = json!("CONFIRMED");
        app.save_payout_wallet("00112233445566778899aabbccddeeff", &public, &private, None)
            .await
            .unwrap();
        state.mode.store(4, Ordering::SeqCst);
        assert!(app.monitor_payouts(11).await.is_err());
        state.mode.store(0, Ordering::SeqCst);
        app.monitor_payouts(11).await.unwrap();
        let pushes = state.pushes.lock().unwrap();
        assert_eq!(pushes.len(), 2);
        assert_eq!(pushes[0], pushes[1]);
        assert_eq!(pushes[1]["status"], "CONFIRMED");
        task.abort();
    }
    #[test]
    fn carry_steps_conserve_value_and_match_core_fees_without_splitting_payees() {
        for n in [1u32, 49, 50, 51, 100, 120] {
            for m in [1u32, 49, 50, 51, 100, 120] {
                let (fee, count) = payout::estimate_fee(&FeeModel::default(), n, m, false).unwrap();
                let total = u64::from(n) * 100_000_000;
                let each = (total - fee) / u64::from(m);
                let mut outputs=(0..m).map(|i|json!({"receiver_hex":format!("payee{i}"),"amount_zat":each+if i==0{(total-fee)%u64::from(m)}else{0}})).collect::<Vec<_>>();
                let expected = outputs.clone();
                let inputs=(0..n).map(|i|json!({"txid":format!("original{i}"),"action_index":0,"amount_zat":100_000_000})).collect::<Vec<_>>();
                let mut private =
                    json!({"inputs":inputs,"outputs":outputs,"remaining_fee_zat":fee});
                outputs.clear();
                let mut fees = 0;
                let mut steps = 0;
                loop {
                    let stage = next_stage(&mut private, "escrow").unwrap();
                    steps += 1;
                    let inputs = stage["inputs"].as_array().unwrap();
                    let outs = stage["outputs"].as_array().unwrap();
                    assert!(
                        inputs.len().max(outs.len())
                            <= zcash_glass::params::MAX_ACTIONS_PER_TX as usize
                    );
                    let sum = |v: &[Value]| {
                        v.iter()
                            .map(|v| v["amount_zat"].as_u64().unwrap())
                            .sum::<u64>()
                    };
                    assert_eq!(sum(inputs), sum(outs) + stage["fee_zat"].as_u64().unwrap());
                    fees += stage["fee_zat"].as_u64().unwrap();
                    outputs.extend(
                        outs.iter()
                            .filter(|v| v["receiver_hex"] != "escrow")
                            .cloned(),
                    );
                    if stage["carry"].is_null() {
                        break;
                    }
                    private["inputs"].as_array_mut().unwrap().push(json!({"txid":format!("carry{steps}"),"action_index":0,"amount_zat":stage["carry"]["amount_zat"]}));
                }
                assert_eq!(outputs, expected);
                assert_eq!(fees, fee);
                assert_eq!(steps, count);
                assert_eq!(private["remaining_fee_zat"], 0);
            }
        }
    }
    #[tokio::test]
    #[ignore = "requires a running local Zebra regtest; set FROSTED_LIVE_REGTEST_URL"]
    async fn live_regtest_threshold_yes_no_and_unlisted_nu7() {
        let url =
            std::env::var("FROSTED_LIVE_REGTEST_URL").expect("explicit regtest RPC URL required");
        let parsed: reqwest::Url = url.parse().unwrap();
        assert!(parsed.host_str().is_some_and(|h| {
            h == "localhost"
                || h.parse::<std::net::IpAddr>()
                    .is_ok_and(|ip| ip.is_loopback())
        }));
        let client = push::client().unwrap();
        async fn call(client: &reqwest::Client, url: &str, method: &str, params: Value) -> Value {
            let response: Value = client
                .post(url)
                .json(&json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}))
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            assert!(response["error"].is_null());
            response["result"].clone()
        }
        let info = call(&client, &url, "getblockchaininfo", json!([])).await;
        // Zebra 6.4.2 reports regtest as `test`; explicit loopback URL keeps this opt-in.
        assert!(info["chain"] == "regtest" || info["chain"] == "test");
        let tip = call(&client, &url, "getblockcount", json!([]))
            .await
            .as_u64()
            .unwrap();
        let block = call(&client, &url, "getblock", json!([tip.to_string(), 1])).await;
        let value = block["valuePools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["id"] == "ironwood")
            .unwrap()["chainValueZat"]
            .as_u64()
            .unwrap();
        let mut def = definition();
        def.resolve_height = u32::try_from(tip).unwrap();
        for (threshold, expected) in [(value, 0), (value + 1, 1)] {
            if let ResolutionParams::PoolThreshold(p) = &mut def.resolution_params {
                p.threshold_zat = threshold;
            }
            let body = resolve(&def, &block, &info).unwrap();
            assert_eq!(body["winning_outcome_index"], expected);
            assert_eq!(body["evidence"]["chain_value_zat"], value);
        }
        assert!(
            !info["upgrades"]
                .as_object()
                .unwrap()
                .values()
                .any(|v| v["name"] == "NU7")
        );
        def.resolution_type = zcash_glass::hash::ResolutionType::UpgradeActive;
        def.resolution_params =
            ResolutionParams::UpgradeActive(zcash_glass::hash::UpgradeActiveParams {
                upgrade_name: "NU7".into(),
            });
        let body = resolve(&def, &block, &info).unwrap();
        assert_eq!(body["winning_outcome_index"], 1);
        assert_eq!(
            body["evidence"]["reported_upgrades"]
                .as_array()
                .unwrap()
                .len(),
            info["upgrades"].as_object().unwrap().len()
        );
    }
    #[test]
    fn malformed_receiver_and_broken_ciphertext_are_unclaimable_not_batch_failures() {
        use zcash_glass::{
            envelope::{SealRandomness, envelope_public_key, seal},
            memo::Frst1Header,
        };
        let key = [1; 32];
        let header = Frst1Header::new([2; 16], None);
        let spending = orchard::keys::SpendingKey::from_bytes([3; 32]).unwrap();
        let good = orchard::keys::FullViewingKey::from(&spending)
            .address_at(0u32, orchard::keys::Scope::External)
            .to_raw_address_bytes();
        for receiver in [good, [255; 43]] {
            let sealed = seal(
                &envelope_public_key(&key),
                &header,
                &receiver,
                &[7; 8],
                SealRandomness {
                    env_esk: [4; 32],
                    env_nonce: [5; 24],
                },
            )
            .unwrap();
            let mut payload = sealed.into_payload(header);
            let opened = open_envelope(&key, Some(&payload.to_bytes()));
            if receiver == good {
                assert!(matches!(opened, EnvelopeResult::Opened { .. }));
            } else {
                assert_eq!(opened, EnvelopeResult::Unopenable);
            }
            payload.env_ciphertext[0] ^= 1;
            assert_eq!(
                open_envelope(&key, Some(&payload.to_bytes())),
                EnvelopeResult::Unopenable
            );
        }
    }
    fn definition() -> MarketDefinition {
        serde_json::from_value(json!({"version":1,"params_version":1,"network":"regtest","market_id":"00112233445566778899aabbccddeeff","question":"Pool?","outcomes":[{"outcome_index":0,"label":"YES","address":"u1"},{"outcome_index":1,"label":"NO","address":"u2"}],"resolution_type":"POOL_THRESHOLD","resolution_params":{"pool":"ironwood","comparator":"gte","threshold_zat":100},"birthday_height":1,"close_height":10,"resolve_height":20,"fee_bps":100,"min_bet_zat":1000000,"refund_fee_zat":5000,"min_confirmations":3,"sweep_threshold_zat":5000,"announce_timeout_blocks":100,"uivk":"uivk","envelope_pk":"key","treasury_address":"treasury"})).unwrap()
    }
    #[test]
    fn historical_threshold_yes_no_and_unknown_upgrade_no_with_evidence() {
        let mut def = definition();
        let block = json!({"hash":"abcd","valuePools":[{"id":"ironwood","chainValueZat":100}]});
        assert_eq!(
            resolve(&def, &block, &Value::Null).unwrap()["winning_outcome_index"],
            0
        );
        if let ResolutionParams::PoolThreshold(p) = &mut def.resolution_params {
            p.threshold_zat = 101;
        }
        assert_eq!(
            resolve(&def, &block, &Value::Null).unwrap()["winning_outcome_index"],
            1
        );
        assert!(resolve(&def, &json!({"hash":"abc"}), &Value::Null).is_err());
        def.resolution_type = zcash_glass::hash::ResolutionType::UpgradeActive;
        def.resolution_params =
            ResolutionParams::UpgradeActive(zcash_glass::hash::UpgradeActiveParams {
                upgrade_name: "NU7".into(),
            });
        let resolved = resolve(
            &def,
            &block,
            &json!({"upgrades":{"abcd":{"name":"NU6.3","activationheight":10}}}),
        )
        .unwrap();
        assert_eq!(resolved["winning_outcome_index"], 1);
        assert_eq!(
            resolved["evidence"]["reported_upgrades"],
            json!([{"name":"NU6.3","activation_height":10}])
        );
        assert!(
            resolve(
                &def,
                &block,
                &json!({"upgrades":{"bad":{"name":"NU7","activationheight":null}}})
            )
            .is_err()
        );
    }
}

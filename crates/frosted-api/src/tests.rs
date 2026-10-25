use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
    routing::post,
};
use ed25519_dalek::SigningKey;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::{
    Row,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use tower::ServiceExt;

use crate::{AppState, Config, router};

const ID: &str = "a1f3c0de9b7e4f21a0b1c2d3e4f50617";
const PRIVATE: &str = "private-bet-txid-never-public";

fn definition() -> Value {
    json!({"version":1,"params_version":1,"network":"regtest","market_id":ID,"question":"Pool threshold?",
        "outcomes":[{"outcome_index":0,"label":"YES","address":"uregtest1yes"},{"outcome_index":1,"label":"NO","address":"uregtest1no"}],
        "resolution_type":"POOL_THRESHOLD","resolution_params":{"pool":"orchard","comparator":"gte","threshold_zat":1000},
        "birthday_height":1,"close_height":100,"resolve_height":110,"fee_bps":100,"min_bet_zat":1000000,
        "refund_fee_zat":5000,"min_confirmations":3,"sweep_threshold_zat":5000,"announce_timeout_blocks":100,
        "uivk":"uivk-public","envelope_pk":"AAAA","treasury_address":"uregtest1treasury"})
}
struct Fixture {
    _dir: tempfile::TempDir,
    state: AppState,
    app: Router,
    rpc: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.rpc.abort();
    }
}
async fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("public.db");
    let pool = SqlitePoolOptions::new()
        .connect_with(
            SqliteConnectOptions::new()
                .filename(&path)
                .create_if_missing(true),
        )
        .await
        .unwrap();
    sqlx::raw_sql(include_str!(
        "../../glass-indexer/migrations/0001_public.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    let mut tx = pool.begin().await.unwrap();
    glass_indexer::indexer::register_market(&mut tx, &definition().to_string())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    sqlx::query("INSERT INTO block VALUES (10,'block-hash','previous',0)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO persona_registry VALUES (1,'registry-uivk','uregtest1registry','regtest',1)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO bet (note_ref,bet_id,market_id,outcome_index,amount_zat,block_height,block_hash,tx_index,action_index,has_transparent_input,status,tagged,bet_fingerprint,memo_payload,first_seen_at) VALUES ('0011223344556677',?,?,0,2000000,5,'block',0,0,0,'COUNTED',1,'fingerprint',?,0)")
        .bind(PRIVATE).bind(ID).bind(b"raw-secret-memo".as_slice()).execute(&pool).await.unwrap();
    pool.close().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let rpc_url = format!("http://{}", listener.local_addr().unwrap());
    let rpc = tokio::spawn(async move {
        axum::serve(listener, Router::new().route("/", post(|axum::Json(body): axum::Json<Value>| async move {
            axum::Json(json!({"jsonrpc":"2.0", "id":1,"result":if body["method"] == "getblockcount" { json!(12) } else { json!("tip-hash") }}))
        }))).await.unwrap();
    });
    let config = Config {
        bind: "127.0.0.1:0".parse().unwrap(),
        public_origin: "http://localhost:3000".parse().unwrap(),
        public_db_path: path.to_str().unwrap().into(),
        coordinator_pubkey: SigningKey::from_bytes(&[7; 32]).verifying_key(),
        ingest_max_skew_secs: 60,
        zebra_rpc_url: rpc_url,
    };
    let state = AppState::open(config).await.unwrap();
    let app = router(state.clone());
    Fixture {
        _dir: dir,
        state,
        app,
        rpc,
    }
}
async fn get(app: &Router, path: &str) -> (StatusCode, axum::http::HeaderMap, Value) {
    let response = app
        .clone()
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, headers, serde_json::from_slice(&body).unwrap())
}
#[tokio::test]
async fn health_market_odds_and_public_response_snapshot() {
    let f = fixture().await;
    let (status, _, health) = get(&f.app, "/v1/health").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        health,
        json!({"status":"ok","tip_height":12,"indexer_height":10,"lag_blocks":2})
    );
    let (_, _, tip) = get(&f.app, "/v1/chain/tip").await;
    assert_eq!(tip, json!({"height":12,"hash":"tip-hash"}));
    let (_, _, odds) = get(&f.app, &format!("/v1/markets/{ID}/odds")).await;
    assert_eq!(
        odds,
        json!({"market_id":ID,"at_height":10,"pool_total_zat":2000000,"outcomes":[
        {"outcome_index":0,"pool_zat":2000000,"bets":1,"probability":1.0},
        {"outcome_index":1,"pool_zat":0,"bets":0,"probability":0.0}],"pending_bets":0})
    );
    let (_, _, historical) = get(&f.app, &format!("/v1/markets/{ID}/odds?at_height=4")).await;
    assert_eq!(historical["pool_total_zat"], 0);
    assert!(historical["outcomes"][0]["probability"].is_null());
    let (_, _, bets) = get(&f.app, &format!("/v1/markets/{ID}/bets")).await;
    assert_eq!(
        bets,
        json!({"bets":[{"note_ref":"0011223344556677","bet_fingerprint":"fingerprint","outcome_index":0,"amount_zat":2000000,"block_height":5,"status":"COUNTED","tagged":true}]})
    );
    for path in [
        "/v1/health".into(),
        "/v1/chain/tip".into(),
        "/v1/markets".into(),
        format!("/v1/markets/{ID}"),
        format!("/v1/markets/{ID}/odds"),
        format!("/v1/markets/{ID}/bets"),
        format!("/v1/markets/{ID}/resolution"),
        format!("/v1/markets/{ID}/payout"),
        format!("/v1/markets/{ID}/payouts"),
        "/v1/personas/registry".into(),
        "/v1/markets?kind=committee".into(),
    ] {
        let (status, headers, body) = get(&f.app, &path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert!(!headers.contains_key("set-cookie"));
        let text = body.to_string();
        for private in [
            PRIVATE,
            "raw-secret-memo",
            "env_esk",
            "memo_payload",
            "bet_id",
            "payout_address",
        ] {
            assert!(!text.contains(private), "{path}: {private}");
        }
    }
    for suffix in [
        "",
        "/odds",
        "/bets",
        "/resolution",
        "/payout",
        "/payouts",
        "/stream",
    ] {
        let (status, _, body) = get(&f.app, &format!("/v1/markets/missing{suffix}")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"]["code"], "MARKET_NOT_FOUND");
    }
    assert!(
        sqlx::query("INSERT INTO block VALUES (11,'forbidden','',0)")
            .execute(&f.state.read)
            .await
            .is_err(),
        "public pool must be read-only"
    );
}
#[tokio::test]
async fn odds_p95_under_300ms() {
    let f = fixture().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!(
        "http://{}/v1/markets/{ID}/odds",
        listener.local_addr().unwrap()
    );
    let app = f.app.clone();
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let client = reqwest::Client::new();
    let mut elapsed = Vec::new();
    for _ in 0..100 {
        let start = Instant::now();
        let response = client.get(&url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let _: Value = response.json().await.unwrap();
        elapsed.push(start.elapsed());
    }
    server.abort();
    elapsed.sort();
    let p95 = elapsed[94];
    eprintln!("GET /odds p95 over 100 loopback HTTP requests: {p95:?}");
    assert!(p95 < Duration::from_millis(300));
}
#[tokio::test]
async fn sse_replays_only_after_cursor_and_filters_private_payloads() {
    let f = fixture().await;
    let request = |cursor: &str| {
        Request::builder()
            .uri(format!("/v1/markets/{ID}/stream"))
            .header("last-event-id", cursor)
            .body(Body::empty())
            .unwrap()
    };
    let response = f.app.clone().oneshot(request("0")).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let mut body = response.into_body();
    // Insert after connecting, as the indexer does when it counts a bet.
    for (kind, payload) in [
        (
            "bet_counted",
            json!({"note_ref":"0011223344556677","bet_fingerprint":"fingerprint","bet_id":PRIVATE}),
        ),
        ("odds_updated", json!({"at_height":10,"memo":"secret"})),
    ] {
        sqlx::query("INSERT INTO event (market_id,kind,payload_json,block_height,created_at) VALUES (?,?,?,10,0)").bind(ID).bind(kind).bind(payload.to_string()).execute(&f.state.write).await.unwrap();
    }
    let first = tokio::time::timeout(Duration::from_secs(2), body.frame())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
        .into_data()
        .unwrap();
    let text = String::from_utf8(first.to_vec()).unwrap();
    assert!(text.contains("event: bet_counted"));
    assert!(!text.contains(PRIVATE));
    assert!(text.contains("id: 1"));
    let response = f.app.clone().oneshot(request("1")).await.unwrap();
    let next = tokio::time::timeout(Duration::from_secs(2), response.into_body().frame())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
        .into_data()
        .unwrap();
    let text = String::from_utf8(next.to_vec()).unwrap();
    assert!(text.contains("id: 2"));
    assert!(text.contains("odds_updated"));
    assert!(!text.contains("bet_counted"));
    assert!(!text.contains("secret"));
}
#[derive(Clone)]
struct LogWriter(Arc<Mutex<Vec<u8>>>);
impl std::io::Write for LogWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
#[tokio::test]
async fn cors_and_logs_do_not_track_visitors() {
    let f = fixture().await;
    let logs = Arc::new(Mutex::new(Vec::new()));
    let out = logs.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(move || LogWriter(out.clone()))
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    for origin in ["http://localhost:3000", "https://other.example"] {
        let response = f
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/markets")
                    .header("origin", origin)
                    .header("x-forwarded-for", "127.0.0.1")
                    .header("user-agent", "private-user-agent")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        if origin == "http://localhost:3000" {
            assert_eq!(response.headers()["access-control-allow-origin"], origin);
        } else {
            assert!(
                response
                    .headers()
                    .get("access-control-allow-origin")
                    .is_none_or(|v| v != origin)
            );
        }
        assert!(!response.headers().contains_key("set-cookie"));
    }
    let text = String::from_utf8(logs.lock().unwrap().clone()).unwrap();
    assert!(!text.contains("127.0.0.1"));
    assert!(!text.contains("private-user-agent"));
}
#[tokio::test]
async fn registration_and_rewind_rollback_together() {
    let f = fixture().await;
    let mut def = definition();
    def["market_id"] = json!("00000000000000000000000000000001");
    let mut tx = f.state.write.begin().await.unwrap();
    glass_indexer::indexer::register_market(&mut tx, &def.to_string())
        .await
        .unwrap();
    assert_eq!(
        sqlx::query("SELECT COUNT(*) FROM block")
            .fetch_one(&mut *tx)
            .await
            .unwrap()
            .get::<i64, _>(0),
        0
    );
    tx.rollback().await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM block")
            .fetch_one(&f.state.read)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM market")
            .fetch_one(&f.state.read)
            .await
            .unwrap(),
        1
    );
}

fn signed_request(path: &str, body: &Value, timestamp: u64, key: u8) -> Request<Body> {
    signed_bytes(path, &body.to_string(), timestamp, key)
}
fn signed_bytes(path: &str, body: &str, timestamp: u64, key: u8) -> Request<Body> {
    use base64::{Engine, engine::general_purpose::STANDARD};
    use ed25519_dalek::Signer;
    use sha2::{Digest, Sha256};
    let message = format!(
        "POST\n{path}\n{timestamp}\n{}",
        hex::encode(Sha256::digest(body.as_bytes()))
    );
    let signature = SigningKey::from_bytes(&[key; 32]).sign(message.as_bytes());
    Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json")
        .header("x-frosted-timestamp", timestamp.to_string())
        .header("x-frosted-signature", STANDARD.encode(signature.to_bytes()))
        .body(Body::from(body.to_owned()))
        .unwrap()
}
async fn push(
    app: &Router,
    path: &str,
    body: &Value,
    timestamp: u64,
    key: u8,
) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(signed_request(path, body, timestamp, key))
        .await
        .unwrap();
    assert!(!response.headers().contains_key("set-cookie"));
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap())
}
fn market_push(def: Value) -> Value {
    let hash =
        zcash_glass::hash::rules_hash(&serde_json::from_value(def.clone()).unwrap()).unwrap();
    json!({"market_definition":def,"rules_hash":hex::encode(hash)})
}
fn payout_push() -> Value {
    let plan = json!({"market_id":ID,"plan_kind":"STANDARD","plan_cutoff_height":110,"result":0,"operator_fee_zat":20000,
        "items":[{"note_ref":"0011223344556677","bet_fingerprint":"fingerprint","kind":"WIN","amount_zat":1980000,"receiver_commitment":"c0ffee"}]});
    let hash =
        zcash_glass::hash::plan_hash(&serde_json::from_value(plan.clone()).unwrap()).unwrap();
    json!({"plan_hash":hex::encode(hash),"status":"PLANNED","plan":plan,
        "summary":{"winners":1,"refunds":0,"unclaimable":0,"swept":0,"uneconomic_dust":0,"operator_fee_zat":20000,"tx_fee_zat":10000,"distributed_zat":1980000},
        "signing":{"sessions_done":0,"sessions_total":1,"threshold":2},"txs":[]})
}
#[tokio::test]
async fn signed_ingest_authentication_hashes_and_atomic_replay() {
    let f = fixture().await;
    let now = crate::auth::now();
    let mut def = definition();
    def["market_id"] = json!("00000000000000000000000000000001");
    let body = market_push(def.clone());
    let (status, error) = push(&f.app, "/v1/ingest/markets", &body, now, 8).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(error["error"]["code"], "UNAUTHORIZED");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM market")
            .fetch_one(&f.state.read)
            .await
            .unwrap(),
        1
    );
    let (_, error) = push(&f.app, "/v1/ingest/markets", &body, now - 120, 7).await;
    assert_eq!(error["error"]["code"], "STALE_REQUEST");
    let mut bad = body.clone();
    bad["market_definition"]["question"] = json!("tampered");
    let (status, error) = push(&f.app, "/v1/ingest/markets", &bad, now, 7).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(error["error"]["code"], "RULES_HASH_MISMATCH");
    let (a, b) = tokio::join!(
        push(&f.app, "/v1/ingest/markets", &body, now, 7),
        push(&f.app, "/v1/ingest/markets", &body, now, 7)
    );
    assert_eq!(a, b);
    assert_eq!(a.0, StatusCode::OK);
    assert_eq!(a.1["status"], "ANNOUNCING");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM market")
            .fetch_one(&f.state.read)
            .await
            .unwrap(),
        2
    );
    sqlx::query("INSERT INTO block VALUES (11,'new','',0)")
        .execute(&f.state.write)
        .await
        .unwrap();
    sqlx::query("UPDATE market SET status='OPEN' WHERE market_id = ?")
        .bind(def["market_id"].as_str().unwrap())
        .execute(&f.state.write)
        .await
        .unwrap();
    let replay = push(&f.app, "/v1/ingest/markets", &body, now, 7).await;
    assert_eq!(replay, a, "retry returns original ANNOUNCING response");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM block")
            .fetch_one(&f.state.read)
            .await
            .unwrap(),
        1,
        "retry must not rewind"
    );
    let mut changed = def;
    changed["question"] = json!("different immutable definition");
    let (_, error) = push(&f.app, "/v1/ingest/markets", &market_push(changed), now, 7).await;
    assert_eq!(error["error"]["code"], "RULES_HASH_MISMATCH");
    let body = market_push(definition());
    let mut request = signed_request("/v1/ingest/markets", &body, now, 7);
    *request.body_mut() = Body::from(format!("{} ", body));
    assert_eq!(
        f.app.clone().oneshot(request).await.unwrap().status(),
        StatusCode::UNAUTHORIZED,
        "signature binds exact bytes"
    );
}
#[tokio::test]
async fn signed_resolution_payout_and_public_privacy_snapshot() {
    let f = fixture().await;
    let now = crate::auth::now();
    let path = format!("/v1/ingest/markets/{ID}/resolution");
    let resolution = json!({"market_id":ID,"status":"RESOLVED","winning_outcome_index":0,"evidence":{"source":"getblock","height":110,"block_hash":"resolution-block","pool":"orchard","chain_value_zat":1001},"votes":[]});
    let (status, response) = push(&f.app, &path, &resolution, now, 7).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(response, resolution);
    assert_eq!(
        get(&f.app, &format!("/v1/markets/{ID}/resolution")).await.2,
        resolution
    );
    let path = format!("/v1/ingest/markets/{ID}/payout");
    let body = payout_push();
    let mut bad = body.clone();
    bad["plan"]["operator_fee_zat"] = json!(9);
    let (status, error) = push(&f.app, &path, &bad, now, 7).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(error["error"]["code"], "PLAN_HASH_MISMATCH");
    let (status, response) = push(&f.app, &path, &body, now, 7).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(response, body);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM event")
        .fetch_one(&f.state.read)
        .await
        .unwrap();
    assert_eq!(push(&f.app, &path, &body, now, 7).await.1, body);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM event")
            .fetch_one(&f.state.read)
            .await
            .unwrap(),
        count
    );
    let mut update = body.clone();
    update["status"] = json!("SIGNING");
    assert_eq!(
        push(&f.app, &path, &update, now, 7).await.0,
        StatusCode::OK,
        "same plan can progress with a new content version"
    );
    assert_eq!(
        push(&f.app, &path, &body, now, 7).await.1,
        body,
        "old retry returns cached response"
    );
    let public = get(&f.app, &format!("/v1/markets/{ID}/payout")).await.2;
    assert_eq!(
        public, update,
        "old retry must not regress published status"
    );
    for forbidden in ["payout_receiver", "env_esk", "memo_payload", PRIVATE] {
        assert!(!public.to_string().contains(forbidden));
    }
    let mut private = update;
    private["plan"]["items"][0]["payout_receiver"] = json!("private-address");
    assert_eq!(
        push(&f.app, &path, &private, now, 7).await.0,
        StatusCode::BAD_REQUEST,
        "strict public schema rejects private fields"
    );
    let path = "/v1/ingest/markets/missing/payout".to_string();
    let (status, error) = push(&f.app, &path, &body, now, 7).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(error["error"]["code"], "MARKET_NOT_FOUND");
}

#[tokio::test]
async fn settled_bet_projection_preserves_odds_and_survives_follow_up() {
    let f = fixture().await;
    let now = crate::auth::now();
    let resolution = json!({"market_id":ID,"status":"RESOLVED","winning_outcome_index":0,"evidence":null,"votes":[]});
    assert_eq!(
        push(
            &f.app,
            &format!("/v1/ingest/markets/{ID}/resolution"),
            &resolution,
            now,
            7
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        get(&f.app, &format!("/v1/markets/{ID}/bets")).await.2["bets"][0]["status"],
        "WON"
    );
    let path = format!("/v1/ingest/markets/{ID}/payout");
    let mut payout = payout_push();
    payout["status"] = json!("CONFIRMED");
    assert_eq!(push(&f.app, &path, &payout, now, 7).await.0, StatusCode::OK);
    assert_eq!(
        get(&f.app, &format!("/v1/markets/{ID}/bets")).await.2["bets"][0]["status"],
        "PAID"
    );
    let mut follow = payout_push();
    follow["plan"]["plan_kind"] = json!("FOLLOW_UP");
    follow["plan"]["plan_cutoff_height"] = json!(120);
    follow["plan"]["items"] = json!([]);
    follow["status"] = json!("BROADCAST");
    follow["plan_hash"] = json!(hex::encode(
        zcash_glass::hash::plan_hash(&serde_json::from_value(follow["plan"].clone()).unwrap())
            .unwrap()
    ));
    assert_eq!(push(&f.app, &path, &follow, now, 7).await.0, StatusCode::OK);
    assert_eq!(
        get(&f.app, &format!("/v1/markets/{ID}")).await.2["status"],
        "PAYING"
    );
    assert_eq!(
        get(&f.app, &format!("/v1/markets/{ID}/bets")).await.2["bets"][0]["status"],
        "PAID"
    );
    assert_eq!(
        get(&f.app, &format!("/v1/markets/{ID}/odds")).await.2["pool_total_zat"],
        2000000
    );
}
#[tokio::test]
async fn personas_only_reveal_thawed_roots_and_leaderboard_requires_public_report() {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    let f = fixture().await;
    sqlx::query("INSERT INTO persona (persona_id,commitment,registered_height,registered_note_ref,k_root) VALUES ('registered',?,1,'ref',?)")
        .bind([1u8;32].as_slice()).bind([9u8;32].as_slice()).execute(&f.state.write).await.unwrap();
    sqlx::query("INSERT INTO persona (persona_id,commitment,registered_height,registered_note_ref,status,alias,k_root,thawed_at) VALUES ('thawed',?,1,'ref','THAWED','orion',?,10)")
        .bind([2u8;32].as_slice()).bind([3u8;32].as_slice()).execute(&f.state.write).await.unwrap();
    let (_, _, registered) = get(&f.app, "/v1/personas/registered").await;
    assert_eq!(
        registered,
        json!({"persona_id":"registered","commitment":hex::encode([1u8;32]),"registered_height":1,"status":"REGISTERED","alias":null,"thawed_at":null})
    );
    let (_, _, thawed) = get(&f.app, "/v1/personas/thawed").await;
    assert_eq!(
        thawed,
        json!({"persona_id":"thawed","commitment":hex::encode([2u8;32]),"registered_height":1,"status":"THAWED","alias":"orion","thawed_at":10,"k_root_b64":URL_SAFE_NO_PAD.encode([3u8;32])})
    );
    assert_eq!(
        get(&f.app, "/v1/leaderboard").await.2,
        json!({"entries":[]})
    );
    sqlx::query("INSERT INTO thaw_report (persona_id,report_json) VALUES ('thawed',?)")
        .bind(json!({"stats":{"settled":14,"won":10,"win_rate":0.714,"roi":0.38},"memo_payload":"never expose report internals"}).to_string())
        .execute(&f.state.write).await.unwrap();
    assert_eq!(
        get(&f.app, "/v1/leaderboard?min_bets=10").await.2,
        json!({"entries":[{"persona_id":"thawed","alias":"orion","registered_height":1,"settled":14,"won":10,"win_rate":0.714,"roi":0.38}]})
    );
    assert_eq!(
        get(&f.app, "/v1/leaderboard?min_bets=15").await.2,
        json!({"entries":[]})
    );
    let (status, _, error) = get(&f.app, "/v1/personas/missing").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(error["error"]["code"], "PERSONA_NOT_FOUND");
}
#[tokio::test]
async fn malformed_and_unavailable_node_errors_keep_api_envelope() {
    let f = fixture().await;
    let mut state = f.state.clone();
    state.config.zebra_rpc_url = "http://127.0.0.1:1".into();
    let (status, _, error) = get(&router(state), "/v1/health").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(error["error"]["code"], "NODE_UNAVAILABLE");
    assert_eq!(
        get(&f.app, "/v1/markets?kind=unknown").await.0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn registration_rescan_removes_orphaned_thaw_reports() {
    let f = fixture().await;
    sqlx::query("INSERT INTO persona (persona_id,commitment,registered_height,registered_note_ref,status) VALUES ('reorg',?,5,'ref','THAWED')")
        .bind([11u8;32].as_slice()).execute(&f.state.write).await.unwrap();
    sqlx::query("INSERT INTO thaw_report (persona_id,report_json) VALUES ('reorg','{}')")
        .execute(&f.state.write)
        .await
        .unwrap();
    let mut def = definition();
    def["market_id"] = json!("00000000000000000000000000000002");
    let body = market_push(def);
    assert_eq!(
        push(&f.app, "/v1/ingest/markets", &body, crate::auth::now(), 7)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM thaw_report")
            .fetch_one(&f.state.read)
            .await
            .unwrap(),
        0
    );
}
#[tokio::test]
async fn framework_rejections_and_unsigned_ingest_use_json_error_envelope() {
    let f = fixture().await;
    for (path, status) in [
        ("/missing", StatusCode::NOT_FOUND),
        (
            "/v1/markets/invalid/odds?at_height=oops",
            StatusCode::BAD_REQUEST,
        ),
        ("/v1/leaderboard?min_bets=invalid", StatusCode::BAD_REQUEST),
    ] {
        let (actual, _, error) = get(&f.app, path).await;
        assert_eq!(actual, status);
        assert!(error["error"]["code"].is_string());
    }
    let response = f
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/ingest/markets")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let response = f
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/ingest/markets")
                .body(Body::from(vec![b' '; 2 * 1024 * 1024 + 1]))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let body: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert!(body["error"]["code"].is_string());
}

#[tokio::test]
async fn ingest_rejects_invalid_resolution_parameters_and_nonsequential_outcomes() {
    let f = fixture().await;
    for (field, value) in [("pool", json!("made-up-pool")), ("comparator", json!("eq"))] {
        let mut def = definition();
        def["resolution_params"][field] = value;
        let body = market_push(def);
        assert_eq!(
            push(&f.app, "/v1/ingest/markets", &body, crate::auth::now(), 7)
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
    }
    let mut def = definition();
    def["outcomes"][1]["outcome_index"] = json!(99);
    assert_eq!(
        push(
            &f.app,
            "/v1/ingest/markets",
            &market_push(def),
            crate::auth::now(),
            7
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
}

const THAW_K: [u8; 32] = [0x5a; 32];

async fn post_json(app: &Router, path: &str, body: &Value) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap())
}
/// Inserts a tagged bet whose FRST1 payload carries the Thaw Tag of `key` over its own data.
async fn tagged_bet(
    f: &Fixture,
    key: &[u8; 32],
    note: &str,
    outcome: u8,
    amount: u64,
    epk: u8,
    height: u32,
) {
    use zcash_glass::memo::{Frst1Header, Frst1Payload};
    let market: [u8; 16] = hex::decode(ID).unwrap().try_into().unwrap();
    let tag = zcash_glass::thaw::tag(key, &market, outcome, amount, &[epk; 32]);
    let header = Frst1Header::new(market, Some(tag));
    let payload = Frst1Payload {
        header,
        env_epk: [epk; 32],
        env_nonce: [0; 24],
        env_ciphertext: [0; 67],
    };
    sqlx::query("INSERT INTO bet (note_ref,bet_id,market_id,outcome_index,amount_zat,block_height,block_hash,tx_index,action_index,has_transparent_input,status,tagged,thaw_tag,memo_payload,first_seen_at) VALUES (?,?,?,?,?,?,'block',0,0,0,'COUNTED',1,?,?,0)")
        .bind(note).bind(format!("txid-{note}:0")).bind(ID).bind(outcome as i64).bind(amount as i64).bind(height)
        .bind(tag.as_slice()).bind(payload.to_bytes().as_slice()).execute(&f.state.write).await.unwrap();
}
async fn thaw_fixture() -> Fixture {
    use zcash_glass::thaw;
    let f = fixture().await;
    let commitment = thaw::commitment(&THAW_K);
    sqlx::query("INSERT INTO persona (persona_id,commitment,registered_height,registered_note_ref) VALUES (?,?,3,'ref')")
        .bind(thaw::persona_id(&commitment)).bind(commitment.as_slice()).execute(&f.state.write).await.unwrap();
    tagged_bet(&f, &THAW_K, "aaaaaaaaaaaaaaaa", 0, 2_000_000, 1, 5).await; // wins
    tagged_bet(&f, &THAW_K, "bbbbbbbbbbbbbbbb", 1, 1_000_000, 2, 6).await; // loses
    tagged_bet(&f, &THAW_K, "cccccccccccccccc", 0, 1_000_000, 3, 2).await; // before registration
    tagged_bet(&f, &THAW_K, "dddddddddddddddd", 0, 2_000_000, 1, 7).await; // copy of the first bet
    tagged_bet(&f, &[0x11; 32], "eeeeeeeeeeeeeeee", 0, 1_000_000, 4, 5).await; // someone else
    let resolution = json!({"market_id":ID,"status":"RESOLVED","winning_outcome_index":0,"evidence":null,"votes":[]});
    sqlx::query(
        "INSERT INTO api_publication (market_id,kind,body_json) VALUES (?, 'resolution', ?)",
    )
    .bind(ID)
    .bind(resolution.to_string())
    .execute(&f.state.write)
    .await
    .unwrap();
    let mut payout = payout_push();
    payout["status"] = json!("CONFIRMED");
    payout["plan"]["items"] = json!([{"note_ref":"aaaaaaaaaaaaaaaa","bet_fingerprint":null,"kind":"WIN","amount_zat":1_980_000,"receiver_commitment":"c0ffee"}]);
    sqlx::query("INSERT INTO api_payout_plan (plan_hash,market_id,body_json) VALUES ('plan',?,?)")
        .bind(ID)
        .bind(payout.to_string())
        .execute(&f.state.write)
        .await
        .unwrap();
    f
}
fn thaw_request(key: &[u8; 32], alias: &str) -> Value {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    json!({"k_root_b64":URL_SAFE_NO_PAD.encode(key),"alias":alias})
}
fn thaw_path() -> String {
    format!(
        "/v1/personas/{}/thaw",
        zcash_glass::thaw::persona_id(&zcash_glass::thaw::commitment(&THAW_K))
    )
}

#[tokio::test]
async fn thaw_publishes_wins_losses_and_flags_excluded_bets() {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    let f = thaw_fixture().await;
    let (status, report) = post_json(&f.app, &thaw_path(), &thaw_request(&THAW_K, "orion")).await;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert_eq!(report["alias"], "orion");
    assert_eq!(report["registered_height"], 3);
    let bets = report["bets"].as_array().unwrap();
    let notes: Vec<_> = bets
        .iter()
        .map(|b| b["note_ref"].as_str().unwrap())
        .collect();
    // Ordered by height; the other Persona's bet is absent.
    assert_eq!(
        notes,
        [
            "cccccccccccccccc",
            "aaaaaaaaaaaaaaaa",
            "bbbbbbbbbbbbbbbb",
            "dddddddddddddddd"
        ]
    );
    let by = |n: &str| bets.iter().find(|b| b["note_ref"] == n).unwrap();
    assert_eq!(by("cccccccccccccccc")["exclusion"], "PRE_REGISTRATION");
    assert_eq!(by("dddddddddddddddd")["exclusion"], "DUPLICATE_TAG");
    assert_eq!(by("aaaaaaaaaaaaaaaa")["result"], "WON");
    assert_eq!(by("aaaaaaaaaaaaaaaa")["payout_zat"], 1_980_000);
    assert_eq!(by("aaaaaaaaaaaaaaaa")["counted_in_reputation"], true);
    assert_eq!(by("bbbbbbbbbbbbbbbb")["result"], "LOST");
    assert_eq!(by("bbbbbbbbbbbbbbbb")["counted_in_reputation"], true);
    assert_eq!(
        report["stats"],
        json!({"settled":2,"won":1,"win_rate":0.5,"roi":-0.34})
    );
    // The persona is now public with its root and shows up on the leaderboard from the cached report.
    let id = zcash_glass::thaw::persona_id(&zcash_glass::thaw::commitment(&THAW_K));
    let (_, _, persona) = get(&f.app, &format!("/v1/personas/{id}")).await;
    assert_eq!(persona["status"], "THAWED");
    assert_eq!(persona["k_root_b64"], URL_SAFE_NO_PAD.encode(THAW_K));
    assert_eq!(
        get(&f.app, "/v1/leaderboard").await.2,
        json!({"entries":[{"persona_id":id,"alias":"orion","registered_height":3,"settled":2,"won":1,"win_rate":0.5,"roi":-0.34}]})
    );
    let (status, _, again) = get(&f.app, &format!("/v1/personas/{id}/report")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(again, report);
    // Nothing private leaks through the report.
    let text = report.to_string();
    for private in ["txid-", "memo_payload", "env_epk", "bet_id"] {
        assert!(!text.contains(private), "{private}");
    }
}
#[tokio::test]
async fn thaw_rejects_wrong_key_repeat_and_unknown_persona() {
    let f = thaw_fixture().await;
    let id = zcash_glass::thaw::persona_id(&zcash_glass::thaw::commitment(&THAW_K));
    let (status, error) =
        post_json(&f.app, &thaw_path(), &thaw_request(&[0x01; 32], "orion")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(error["error"]["code"], "THAW_COMMITMENT_MISMATCH");
    // A failed thaw publishes nothing.
    let (_, _, persona) = get(&f.app, &format!("/v1/personas/{id}")).await;
    assert_eq!(persona["status"], "REGISTERED");
    assert!(persona.get("k_root_b64").is_none());
    assert_eq!(
        get(&f.app, &format!("/v1/personas/{id}/report")).await.0,
        StatusCode::NOT_FOUND
    );
    for bad in ["", "has space", &"x".repeat(33)] {
        let (status, _) = post_json(&f.app, &thaw_path(), &thaw_request(&THAW_K, bad)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad:?}");
    }
    let (status, _) = post_json(
        &f.app,
        &thaw_path(),
        &json!({"k_root_b64":"short","alias":"orion"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        post_json(&f.app, &thaw_path(), &thaw_request(&THAW_K, "orion"))
            .await
            .0,
        StatusCode::OK
    );
    let (status, error) = post_json(&f.app, &thaw_path(), &thaw_request(&THAW_K, "other")).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(error["error"]["code"], "PERSONA_ALREADY_THAWED");
    let (status, error) = post_json(
        &f.app,
        "/v1/personas/missing/thaw",
        &thaw_request(&THAW_K, "orion"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(error["error"]["code"], "PERSONA_NOT_FOUND");
}
#[tokio::test]
async fn thaw_report_refreshes_when_markets_settle_later() {
    let f = thaw_fixture().await;
    // Thaw while the winner's payout is not published yet: the win is not settled.
    sqlx::query("DELETE FROM api_payout_plan")
        .execute(&f.state.write)
        .await
        .unwrap();
    let (_, report) = post_json(&f.app, &thaw_path(), &thaw_request(&THAW_K, "orion")).await;
    assert_eq!(report["stats"]["settled"], 1);
    assert_eq!(report["stats"]["won"], 0);
    let mut payout = payout_push();
    payout["status"] = json!("CONFIRMED");
    payout["plan"]["items"] = json!([{"note_ref":"aaaaaaaaaaaaaaaa","bet_fingerprint":null,"kind":"WIN","amount_zat":1_980_000,"receiver_commitment":"c0ffee"}]);
    sqlx::query("INSERT INTO api_payout_plan (plan_hash,market_id,body_json) VALUES ('plan',?,?)")
        .bind(ID)
        .bind(payout.to_string())
        .execute(&f.state.write)
        .await
        .unwrap();
    sqlx::query("INSERT INTO block VALUES (11,'next','block-hash',0)")
        .execute(&f.state.write)
        .await
        .unwrap();
    crate::refresh_thaw_reports(&f.state).await.unwrap();
    let id = zcash_glass::thaw::persona_id(&zcash_glass::thaw::commitment(&THAW_K));
    let (_, _, refreshed) = get(&f.app, &format!("/v1/personas/{id}/report")).await;
    assert_eq!(
        refreshed["stats"],
        json!({"settled":2,"won":1,"win_rate":0.5,"roi":-0.34})
    );
}

#[tokio::test]
async fn payouts_lists_every_plan_oldest_first() {
    let f = fixture().await;
    assert_eq!(
        get(&f.app, &format!("/v1/markets/{ID}/payouts")).await.2,
        json!({"plans":[]})
    );
    let path = format!("/v1/ingest/markets/{ID}/payout");
    let now = crate::auth::now();
    let first = payout_push();
    assert_eq!(push(&f.app, &path, &first, now, 7).await.0, StatusCode::OK);
    let mut follow = payout_push();
    follow["plan"]["plan_kind"] = json!("FOLLOW_UP");
    follow["plan"]["plan_cutoff_height"] = json!(120);
    follow["plan"]["items"] = json!([]);
    follow["plan_hash"] = json!(hex::encode(
        zcash_glass::hash::plan_hash(&serde_json::from_value(follow["plan"].clone()).unwrap())
            .unwrap()
    ));
    assert_eq!(push(&f.app, &path, &follow, now, 7).await.0, StatusCode::OK);
    let (_, _, body) = get(&f.app, &format!("/v1/markets/{ID}/payouts")).await;
    let plans = body["plans"].as_array().unwrap();
    assert_eq!(plans.len(), 2);
    assert_eq!(plans[0]["plan_hash"], first["plan_hash"]);
    assert_eq!(plans[1]["plan"]["plan_kind"], "FOLLOW_UP");
    // `/payout` keeps serving only the latest plan.
    assert_eq!(
        get(&f.app, &format!("/v1/markets/{ID}/payout")).await.2["plan_hash"],
        follow["plan_hash"]
    );
}

const VERIFY_ID: &str = "b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2";

/// A market whose UIVK and Outcome Addresses are real, so the verifier can scan it.
fn verifiable_definition() -> Value {
    use orchard::keys::{FullViewingKey, Scope, SpendingKey};
    use zcash_glass::scan::{ActivationHeights, ScanNetwork};
    use zcash_keys::{address::UnifiedAddress, keys::UnifiedFullViewingKey};
    let net = ScanNetwork::new("regtest", ActivationHeights::default()).unwrap();
    let fvk = FullViewingKey::from(&SpendingKey::from_bytes([0x44; 32]).unwrap());
    let addr = |j| {
        UnifiedAddress::from_receivers(
            Some(fvk.address_at(j, Scope::External)),
            None,
            None,
            None,
            None,
        )
        .unwrap()
        .encode(&net)
    };
    let uivk = UnifiedFullViewingKey::from_orchard_fvk(fvk.clone())
        .unwrap()
        .to_unified_incoming_viewing_key()
        .encode(&net)
        .unwrap();
    let mut def = definition();
    def["market_id"] = json!(VERIFY_ID);
    def["birthday_height"] = json!(5);
    def["uivk"] = json!(uivk);
    def["outcomes"][0]["address"] = json!(addr(0u32));
    def["outcomes"][1]["address"] = json!(addr(1u32));
    def
}

/// zebrad stand-in: an empty chain of 12 blocks. Counts calls so caching can be asserted.
async fn mock_node(calls: Arc<std::sync::atomic::AtomicUsize>) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().route(
                "/",
                post(move |axum::Json(body): axum::Json<Value>| {
                    let calls = calls.clone();
                    async move {
                        calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        axum::Json(json!({"jsonrpc":"2.0","id":1,"result":match body["method"].as_str().unwrap() {
                            "getblockcount" => json!(12),
                            "getblock" => json!({"hash":"blockhash","tx":[]}),
                            "getblockchaininfo" => json!({"chain":"test","upgrades":{}}),
                            other => panic!("unexpected RPC {other}"),
                        }}))
                    }
                }),
            ),
        )
        .await
        .unwrap();
    });
    url
}

#[tokio::test]
async fn verify_endpoint_runs_the_cli_checks_labels_them_and_caches() {
    let f = fixture().await;
    let mut tx = f.state.write.begin().await.unwrap();
    glass_indexer::indexer::register_market(&mut tx, &verifiable_definition().to_string())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut state = f.state.clone();
    state.verify = crate::verify::Verifier::new(&mock_node(calls.clone()).await).unwrap();
    let app = router(state);

    let path = format!("/v1/markets/{VERIFY_ID}/verify");
    let (status, _, report) = get(&app, &path).await;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert_eq!(report["command"], "all");
    assert_eq!(report["trust"], crate::verify::TRUST_NOTICE);
    let by = |id: &str| {
        report["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == id)
            .unwrap_or_else(|| panic!("no check {id}: {report}"))["status"]
            .clone()
    };
    assert_eq!(by("rules_hash"), "PASS");
    assert_eq!(by("addresses"), "PASS");
    assert_eq!(by("pools"), "PASS");
    assert_eq!(by("announcement"), "INFO"); // still ANNOUNCING on an empty chain
    assert_eq!(by("resolution"), "SKIP");
    assert_eq!(report["ok"], true);

    // A second visitor within the window gets the same report without another chain scan.
    let before = calls.load(std::sync::atomic::Ordering::SeqCst);
    assert!(before > 0);
    assert_eq!(get(&app, &path).await.2, report);
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), before);

    let (status, _, error) = get(&app, "/v1/markets/missing/verify").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(error["error"]["code"], "MARKET_NOT_FOUND");
}

#[tokio::test]
async fn verify_endpoint_reports_an_unreachable_node_as_unavailable() {
    let f = fixture().await;
    let mut tx = f.state.write.begin().await.unwrap();
    glass_indexer::indexer::register_market(&mut tx, &verifiable_definition().to_string())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let mut state = f.state.clone();
    state.verify = crate::verify::Verifier::new("http://127.0.0.1:1").unwrap();
    let (status, _, error) = get(&router(state), &format!("/v1/markets/{VERIFY_ID}/verify")).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(error["error"]["code"], "NODE_UNAVAILABLE");
}

#[tokio::test]
async fn committee_votes_flow_through_ingest_including_a_withdrawal() {
    let f = fixture().await;
    let path = format!("/v1/ingest/markets/{ID}/resolution");
    let now = crate::auth::now();
    let vote = |member: &str, action: &str| json!({"member_id":member,"action":action,"outcome_index":0,"evidence_url":"https://example.org/e","created_at":1});
    let body = |status: &str, winner: Value, votes: Vec<Value>| {
        json!({"market_id":ID,"status":status,"winning_outcome_index":winner,
        "evidence":if status == "PROPOSED" { json!({"source":"committee","height":21,"block_hash":"hash"}) } else { Value::Null },"votes":votes})
    };
    let market_status =
        || async { get(&f.app, &format!("/v1/markets/{ID}")).await.2["status"].clone() };

    // The first proposal, still unconfirmed, is public.
    let pending = body(
        "AWAITING_RESOLUTION",
        Value::Null,
        vec![vote("member-1", "PROPOSE")],
    );
    assert_eq!(
        push(&f.app, &path, &pending, now, 7).await.0,
        StatusCode::OK
    );
    assert_eq!(market_status().await, "AWAITING_RESOLUTION");

    let proposed = body(
        "PROPOSED",
        Value::Null,
        vec![vote("member-1", "PROPOSE"), vote("member-2", "CONFIRM")],
    );
    assert_eq!(
        push(&f.app, &path, &proposed, now, 7).await.0,
        StatusCode::OK
    );
    assert_eq!(market_status().await, "PROPOSED");
    let (_, _, published) = get(&f.app, &format!("/v1/markets/{ID}/resolution")).await;
    assert_eq!(published["votes"].as_array().unwrap().len(), 2);
    assert_eq!(published["evidence"]["height"], 21);

    // Withdrawal: back to AWAITING_RESOLUTION, votes cleared.
    let withdrawn = body("AWAITING_RESOLUTION", Value::Null, vec![]);
    assert_eq!(
        push(&f.app, &path, &withdrawn, now, 7).await.0,
        StatusCode::OK
    );
    assert_eq!(market_status().await, "AWAITING_RESOLUTION");
    assert_eq!(
        get(&f.app, &format!("/v1/markets/{ID}/resolution")).await.2["votes"],
        json!([])
    );

    let resolved = body(
        "RESOLVED",
        json!(0),
        vec![vote("member-1", "PROPOSE"), vote("member-2", "CONFIRM")],
    );
    assert_eq!(
        push(&f.app, &path, &resolved, now, 7).await.0,
        StatusCode::OK
    );
    assert_eq!(market_status().await, "RESOLVED");

    // Only a decided market names a winner.
    for status in ["AWAITING_RESOLUTION", "PROPOSED", "VOID"] {
        let wrong = body(status, json!(1), vec![]);
        assert_eq!(
            push(&f.app, &path, &wrong, now, 7).await.0,
            StatusCode::BAD_REQUEST,
            "{status}"
        );
    }
}

#[tokio::test]
async fn tip_is_cached_so_visitors_do_not_spend_node_quota() {
    let f = fixture().await;
    let (status, _, tip) = get(&f.app, "/v1/chain/tip").await;
    assert_eq!(status, StatusCode::OK);
    // With the node gone, the next read inside the freshness window is still served from the cache.
    f.rpc.abort();
    let (status, _, cached) = get(&f.app, "/v1/chain/tip").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cached, tip);
}

#[tokio::test]
async fn live_streams_are_capped() {
    let f = fixture().await;
    let slots = f.state.sse_slots.available_permits() as u32;
    let _held = f
        .state
        .sse_slots
        .clone()
        .try_acquire_many_owned(slots)
        .unwrap();
    let response = f
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/markets/{ID}/stream"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

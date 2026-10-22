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
    for suffix in ["", "/odds", "/bets", "/resolution", "/payout", "/stream"] {
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

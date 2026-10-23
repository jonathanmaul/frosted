//! Regtest integration helper (scripts/it-persona-thaw.sh). Test data only.
//!
//! Opens the `public.db` that `glass-indexer` filled from a real regtest chain, publishes a
//! signed resolution and payout plan as the coordinator would (the FROST payout itself is covered
//! by the M8 run), then thaws the Persona through the real HTTP router and prints the report.
//!
//!   it_thaw <public.db> <market_id> <winner> <win_note_ref> <win_amount_zat> <persona_id> <k_root_hex> <alias>
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use ed25519_dalek::{Signer, SigningKey};
use frosted_api::{AppState, Config, router};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tower::ServiceExt;
use zcash_glass::hash::{Plan, plan_hash};

async fn call(app: &axum::Router, req: Request<Body>) -> anyhow::Result<(StatusCode, Value)> {
    let response = app.clone().oneshot(req).await?;
    let status = response.status();
    let bytes = response.into_body().collect().await?.to_bytes();
    Ok((status, serde_json::from_slice(&bytes)?))
}
fn signed(key: &SigningKey, path: &str, body: &Value) -> Request<Body> {
    let body = body.to_string();
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let message = format!(
        "POST\n{path}\n{ts}\n{}",
        hex::encode(Sha256::digest(body.as_bytes()))
    );
    Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json")
        .header("x-frosted-timestamp", ts.to_string())
        .header(
            "x-frosted-signature",
            STANDARD.encode(key.sign(message.as_bytes()).to_bytes()),
        )
        .body(Body::from(body))
        .unwrap()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    anyhow::ensure!(a.len() == 8, "see module docs");
    let (db, market, winner, note_ref, amount, persona, k_root, alias) = (
        &a[0],
        &a[1],
        a[2].parse::<u32>()?,
        &a[3],
        a[4].parse::<u64>()?,
        &a[5],
        &a[6],
        &a[7],
    );
    let key = SigningKey::from_bytes(&[7; 32]);
    let state = AppState::open(Config {
        bind: "127.0.0.1:0".parse()?,
        public_origin: "http://localhost:3000".parse()?,
        public_db_path: db.clone(),
        coordinator_pubkey: key.verifying_key(),
        ingest_max_skew_secs: 60,
        zebra_rpc_url: "http://127.0.0.1:1".into(),
    })
    .await?;
    let app = router(state);

    let resolution = json!({"market_id":market,"status":"RESOLVED","winning_outcome_index":winner,"evidence":null,"votes":[]});
    let (status, body) = call(
        &app,
        signed(
            &key,
            &format!("/v1/ingest/markets/{market}/resolution"),
            &resolution,
        ),
    )
    .await?;
    anyhow::ensure!(status == StatusCode::OK, "resolution push failed: {body}");

    let plan: Plan = serde_json::from_value(
        json!({"market_id":market,"plan_kind":"STANDARD","plan_cutoff_height":1,"result":winner,
        "operator_fee_zat":0,"items":[{"note_ref":note_ref,"bet_fingerprint":null,"kind":"WIN","amount_zat":amount,"receiver_commitment":"00"}]}),
    )?;
    let hash = hex::encode(plan_hash(&plan)?);
    let payout = json!({"plan_hash":hash,"status":"CONFIRMED","plan":plan,
        "summary":{"winners":1,"refunds":0,"unclaimable":0,"swept":0,"uneconomic_dust":0,"operator_fee_zat":0,"tx_fee_zat":0,"distributed_zat":amount},
        "signing":{"sessions_done":1,"sessions_total":1,"threshold":2},"txs":[]});
    let (status, body) = call(
        &app,
        signed(
            &key,
            &format!("/v1/ingest/markets/{market}/payout"),
            &payout,
        ),
    )
    .await?;
    anyhow::ensure!(status == StatusCode::OK, "payout push failed: {body}");

    let mut root = [0u8; 32];
    hex::decode_to_slice(k_root, &mut root)?;
    let request = Request::builder()
        .method("POST")
        .uri(format!("/v1/personas/{persona}/thaw"))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"k_root_b64":URL_SAFE_NO_PAD.encode(root),"alias":alias}).to_string(),
        ))?;
    let (status, report) = call(&app, request).await?;
    anyhow::ensure!(status == StatusCode::OK, "thaw failed: {status} {report}");
    println!("{}", serde_json::to_string(&report)?);
    Ok(())
}

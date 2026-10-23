use axum::{
    Json,
    body::Bytes,
    extract::{OriginalUri, Path, State},
    http::{HeaderMap, Method, StatusCode},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{Sqlite, Transaction};
use zcash_glass::hash::{
    MarketDefinition, PlanKind, ResolutionParams, ResolutionType, plan_hash, rules_hash,
};

use crate::{
    AppState, auth,
    error::{ApiError, Result},
    public,
    types::{Market, PayoutPublic, Resolution},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MarketPush {
    market_definition: MarketDefinition,
    rules_hash: String,
}

async fn replay(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    kind: &str,
    version: &str,
) -> Result<Option<Value>> {
    let body:Option<String>=sqlx::query_scalar("SELECT response_json FROM api_ingest_replay WHERE market_id = ? AND kind = ? AND version = ?")
        .bind(id).bind(kind).bind(version).fetch_optional(&mut **tx).await?;
    Ok(body.map(|body| serde_json::from_str(&body)).transpose()?)
}
async fn remember(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    kind: &str,
    version: &str,
    response: &Value,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO api_ingest_replay (market_id,kind,version,response_json) VALUES (?,?,?,?)",
    )
    .bind(id)
    .bind(kind)
    .bind(version)
    .bind(response.to_string())
    .execute(&mut **tx)
    .await?;
    Ok(())
}
fn authenticated(
    state: &AppState,
    uri: &axum::http::Uri,
    headers: &HeaderMap,
    body: &Bytes,
) -> Result<String> {
    if uri.query().is_some() {
        return Err(ApiError::unauthorized());
    }
    auth::verify(&state.config, &Method::POST, uri.path(), headers, body)?;
    Ok(hex::encode(Sha256::digest(body)))
}
fn mismatch(code: &'static str) -> ApiError {
    ApiError(
        StatusCode::BAD_REQUEST,
        code,
        "Pushed document does not match its hash".into(),
    )
}
fn validate_definition(def: &MarketDefinition) -> Result<()> {
    let valid = matches!(def.network.as_str(), "regtest" | "testnet" | "mainnet")
        && def.version == 1
        && def.params_version == 1
        && (2..=4).contains(&def.outcomes.len())
        && def.outcomes.iter().enumerate().all(|(i, o)| {
            o.outcome_index as usize == i && !o.label.is_empty() && !o.address.is_empty()
        })
        && def.birthday_height <= def.close_height
        && def.resolve_height >= def.close_height
        && def.min_confirmations > 0
        && def.fee_bps <= 10000
        && !def.question.is_empty()
        && !def.uivk.is_empty();
    if !valid {
        return Err(ApiError::bad("Invalid market definition"));
    }
    if def.resolution_type != ResolutionType::Committee
        && (def.outcomes.len() != 2
            || def.outcomes[0].label != "YES"
            || def.outcomes[1].label != "NO")
    {
        return Err(ApiError::bad(
            "Chain-verified markets require YES and NO outcomes",
        ));
    }
    let params_valid = match &def.resolution_params {
        ResolutionParams::PoolThreshold(p) => {
            matches!(
                p.pool.as_str(),
                "transparent" | "sprout" | "sapling" | "orchard" | "ironwood" | "lockbox"
            ) && matches!(p.comparator.as_str(), "gte" | "lt")
        }
        ResolutionParams::UpgradeActive(p) => !p.upgrade_name.trim().is_empty(),
        ResolutionParams::Committee(p) => {
            !p.criteria.trim().is_empty()
                && !p.evidence_source.trim().is_empty()
                && p.dispute_window_blocks > 0
                && p.void_after_blocks > 0
        }
    };
    if !params_valid {
        return Err(ApiError::bad("Invalid resolution parameters"));
    }
    Ok(())
}
pub(crate) async fn market(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>> {
    let version = authenticated(&state, &uri, &headers, &body)?;
    let push: MarketPush = serde_json::from_slice(&body)?;
    let def = &push.market_definition;
    let actual = rules_hash(def).map_err(|_| mismatch("RULES_HASH_MISMATCH"))?;
    if hex::encode(actual) != push.rules_hash {
        return Err(mismatch("RULES_HASH_MISMATCH"));
    }
    validate_definition(def)?;
    let mut tx = state.write.begin_with("BEGIN IMMEDIATE").await?;
    if let Some(response) = replay(&mut tx, &def.market_id, "market", &version).await? {
        return Ok(Json(response));
    }
    let existing: Option<String> =
        sqlx::query_scalar("SELECT rules_hash FROM market WHERE market_id = ?")
            .bind(&def.market_id)
            .fetch_optional(&mut *tx)
            .await?;
    if let Some(hash) = existing {
        if hash != push.rules_hash {
            return Err(mismatch("RULES_HASH_MISMATCH"));
        }
        // A coordinator retry with different JSON whitespace must not trigger a second rescan.
        let row = sqlx::query("SELECT status, announce_height FROM market WHERE market_id = ?")
            .bind(&def.market_id)
            .fetch_one(&mut *tx)
            .await?;
        use sqlx::Row;
        let response = serde_json::to_value(Market {
            market_definition: push.market_definition,
            rules_hash: push.rules_hash,
            status: row.get(0),
            announce_height: row.get(1),
        })?;
        remember(
            &mut tx,
            response["market_definition"]["market_id"]
                .as_str()
                .unwrap_or_default(),
            "market",
            &version,
            &response,
        )
        .await?;
        tx.commit().await?;
        return Ok(Json(response));
    }
    glass_indexer::indexer::register_market(&mut tx, &serde_json::to_string(def)?)
        .await
        .map_err(|_| {
            ApiError(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL",
                "Market registration failed".into(),
            )
        })?;
    let response = serde_json::to_value(Market {
        market_definition: def.clone(),
        rules_hash: push.rules_hash,
        status: "ANNOUNCING".into(),
        announce_height: None,
    })?;
    remember(&mut tx, &def.market_id, "market", &version, &response).await?;
    tx.commit().await?;
    Ok(Json(response))
}
// shortcut: novel signed versions are coordinator-authoritative and unordered, add monotonic revisions for concurrent coordinators.
async fn publish(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    kind: &str,
    body: &Value,
) -> Result<()> {
    sqlx::query("INSERT INTO api_publication (market_id,kind,body_json) VALUES (?,?,?) ON CONFLICT(market_id,kind) DO UPDATE SET body_json=excluded.body_json")
        .bind(id).bind(kind).bind(body.to_string()).execute(&mut **tx).await?;
    Ok(())
}
async fn emit(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    kind: &str,
    payload: Value,
) -> Result<()> {
    sqlx::query("INSERT INTO event (market_id,kind,payload_json,block_height,created_at) VALUES (?,?,?,COALESCE((SELECT MAX(height) FROM block),0),?)")
        .bind(id).bind(kind).bind(payload.to_string()).bind(auth::now() as i64).execute(&mut **tx).await?;
    Ok(())
}
async fn set_market_status(tx: &mut Transaction<'_, Sqlite>, id: &str, status: &str) -> Result<()> {
    let old: String = sqlx::query_scalar("SELECT status FROM market WHERE market_id = ?")
        .bind(id)
        .fetch_one(&mut **tx)
        .await?;
    if old != status {
        sqlx::query(
            "UPDATE market SET status = ?, status_source = 'coordinator' WHERE market_id = ?",
        )
        .bind(status)
        .bind(id)
        .execute(&mut **tx)
        .await?;
        emit(
            tx,
            id,
            "market_status_changed",
            json!({"previous_status":old,"status":status}),
        )
        .await?;
    }
    Ok(())
}
pub(crate) async fn resolution(
    State(state): State<AppState>,
    Path(id): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>> {
    let version = authenticated(&state, &uri, &headers, &body)?;
    let push: Resolution = serde_json::from_slice(&body)?;
    let market = public::market(&state, &id).await?;
    if push.market_id != id
        || !matches!(
            push.status.as_str(),
            "AWAITING_RESOLUTION" | "PROPOSED" | "RESOLVED" | "VOID"
        )
        || push
            .winning_outcome_index
            .is_some_and(|i| i as usize >= market.market_definition.outcomes.len())
        || (push.status == "RESOLVED" && push.winning_outcome_index.is_none())
        // Only a decided market names a winner; a withdrawn committee proposal goes back to
        // AWAITING_RESOLUTION with its votes cleared.
        || (matches!(push.status.as_str(), "VOID" | "AWAITING_RESOLUTION" | "PROPOSED")
            && push.winning_outcome_index.is_some())
    {
        return Err(ApiError::bad("Invalid resolution"));
    }
    for vote in &push.votes {
        if !matches!(vote.action.as_str(), "PROPOSE" | "CONFIRM" | "WITHDRAW")
            || vote
                .outcome_index
                .is_some_and(|i| i as usize >= market.market_definition.outcomes.len())
        {
            return Err(ApiError::bad("Invalid resolution vote"));
        }
    }
    let response = serde_json::to_value(&push)?;
    let mut tx = state.write.begin_with("BEGIN IMMEDIATE").await?;
    if let Some(response) = replay(&mut tx, &id, "resolution", &version).await? {
        return Ok(Json(response));
    }
    publish(&mut tx, &id, "resolution", &response).await?;
    set_market_status(&mut tx, &id, &push.status).await?;
    remember(&mut tx, &id, "resolution", &version, &response).await?;
    tx.commit().await?;
    Ok(Json(response))
}
pub(crate) async fn payout(
    State(state): State<AppState>,
    Path(id): Path<String>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>> {
    let version = authenticated(&state, &uri, &headers, &body)?;
    let push: PayoutPublic = serde_json::from_slice(&body)?;
    public::market(&state, &id).await?;
    let actual = plan_hash(&push.plan).map_err(|_| mismatch("PLAN_HASH_MISMATCH"))?;
    if push.plan.market_id != id || hex::encode(actual) != push.plan_hash {
        return Err(mismatch("PLAN_HASH_MISMATCH"));
    }
    if !matches!(
        push.status.as_str(),
        "PLANNED" | "SIGNING" | "BROADCAST" | "CONFIRMED" | "FAILED"
    ) || push.signing.sessions_done > push.signing.sessions_total
        || push.signing.threshold != 2
        || push.txs.iter().any(|t| {
            !matches!(
                t.status.as_str(),
                "BUILDING" | "SIGNING" | "BROADCAST" | "CONFIRMED" | "EXPIRED"
            )
        })
    {
        return Err(ApiError::bad("Invalid payout status"));
    }
    let response = serde_json::to_value(&push)?;
    let mut tx = state.write.begin_with("BEGIN IMMEDIATE").await?;
    if let Some(response) = replay(&mut tx, &id, "payout", &version).await? {
        return Ok(Json(response));
    }
    publish(&mut tx, &id, "payout", &response).await?;
    sqlx::query("INSERT INTO api_payout_plan(plan_hash,market_id,body_json) VALUES(?,?,?) ON CONFLICT(plan_hash) DO UPDATE SET body_json=excluded.body_json")
        .bind(&push.plan_hash).bind(&id).bind(response.to_string()).execute(&mut *tx).await?;
    let status: String = sqlx::query_scalar("SELECT status FROM market WHERE market_id = ?")
        .bind(&id)
        .fetch_one(&mut *tx)
        .await?;
    if status != "SETTLED" || push.plan.plan_kind == PlanKind::FollowUp {
        if push.status == "BROADCAST" {
            set_market_status(&mut tx, &id, "PAYING").await?;
        } else if push.status == "CONFIRMED" {
            set_market_status(&mut tx, &id, "SETTLED").await?;
        }
    }
    emit(
        &mut tx,
        &id,
        "payout_updated",
        json!({"plan_hash":push.plan_hash,"status":push.status}),
    )
    .await?;
    remember(&mut tx, &id, "payout", &version, &response).await?;
    tx.commit().await?;
    Ok(Json(response))
}

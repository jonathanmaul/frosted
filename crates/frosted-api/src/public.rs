use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use zcash_glass::{
    hash::MarketDefinition,
    odds::{self, OddsBet},
    status::BetStatus,
};

use crate::{
    AppState,
    error::{ApiError, Result},
    types::{Market, PublicBet},
};

pub(crate) async fn market(state: &AppState, id: &str) -> Result<Market> {
    let row = sqlx::query("SELECT definition_json, rules_hash, status, announce_height FROM market WHERE market_id = ?")
        .bind(id).fetch_optional(&state.read).await?.ok_or_else(ApiError::market)?;
    Ok(Market {
        market_definition: serde_json::from_str(row.get(0))?,
        rules_hash: row.get(1),
        status: row.get(2),
        announce_height: row.get(3),
    })
}
pub(crate) async fn height(state: &AppState) -> Result<u32> {
    Ok(
        sqlx::query_scalar::<_, Option<u32>>("SELECT MAX(height) FROM block")
            .fetch_one(&state.read)
            .await?
            .unwrap_or(0),
    )
}
async fn rpc(state: &AppState, method: &str, params: Value) -> Result<Value> {
    let unavailable = || {
        ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "NODE_UNAVAILABLE",
            "Node unavailable".into(),
        )
    };
    let response = state
        .http
        .post(&state.config.zebra_rpc_url)
        .json(&json!({"jsonrpc":"2.0", "id":1, "method":method, "params":params}))
        .send()
        .await
        .map_err(|_| unavailable())?
        .error_for_status()
        .map_err(|_| unavailable())?
        .json::<Value>()
        .await
        .map_err(|_| unavailable())?;
    if response.get("error").is_some_and(|v| !v.is_null()) {
        return Err(unavailable());
    }
    response
        .get("result")
        .filter(|v| !v.is_null())
        .cloned()
        .ok_or_else(unavailable)
}
pub(crate) async fn tip(State(state): State<AppState>) -> Result<Json<Value>> {
    let height = rpc(&state, "getblockcount", json!([]))
        .await?
        .as_u64()
        .ok_or(ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "NODE_UNAVAILABLE",
            "Invalid node height".into(),
        ))?;
    let hash = rpc(&state, "getblockhash", json!([height])).await?;
    if !hash.is_string() {
        return Err(ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "NODE_UNAVAILABLE",
            "Invalid node hash".into(),
        ));
    }
    Ok(Json(json!({"height":height, "hash":hash})))
}
pub(crate) async fn health(State(state): State<AppState>) -> Result<Json<Value>> {
    let tip = tip(State(state.clone())).await?.0;
    let indexer_height = height(&state).await?;
    let tip_height = tip["height"].as_u64().unwrap_or(0);
    Ok(Json(
        json!({"status":"ok", "tip_height":tip_height, "indexer_height":indexer_height,
        "lag_blocks":tip_height.saturating_sub(indexer_height as u64)}),
    ))
}
pub(crate) async fn get_market(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Market>> {
    Ok(Json(market(&state, &id).await?))
}
#[derive(Default, Deserialize)]
pub(crate) struct OddsQuery {
    at_height: Option<u32>,
}

pub(crate) async fn odds_at(state: &AppState, def: &MarketDefinition, at: u32) -> Result<Value> {
    let rows = sqlx::query("SELECT outcome_index, amount_zat, status FROM bet WHERE market_id = ? AND block_height <= ? AND is_announcement = 0 ORDER BY block_height, tx_index, action_index")
        .bind(&def.market_id).bind(at).fetch_all(&state.read).await?;
    let mut bets = Vec::with_capacity(rows.len());
    for row in rows {
        let status: BetStatus = serde_json::from_value(json!(row.get::<String, _>(2)))?;
        bets.push(OddsBet {
            outcome_index: row.get::<Option<u32>, _>(0).unwrap_or(u32::MAX),
            amount_zat: row.get(1),
            status,
        });
    }
    let mut value = serde_json::to_value(odds::compute(def.outcomes.len() as u32, &bets))?;
    value["market_id"] = json!(def.market_id);
    value["at_height"] = json!(at);
    Ok(value)
}
pub(crate) async fn get_odds(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<OddsQuery>,
) -> Result<Json<Value>> {
    let def = market(&state, &id).await?.market_definition;
    let at = query.at_height.unwrap_or(height(&state).await?);
    Ok(Json(odds_at(&state, &def, at).await?))
}
#[derive(Default, Deserialize)]
pub(crate) struct MarketsQuery {
    status: Option<String>,
    resolution_type: Option<String>,
    kind: Option<String>,
}
pub(crate) async fn markets(
    State(state): State<AppState>,
    Query(query): Query<MarketsQuery>,
) -> Result<Json<Value>> {
    if query
        .kind
        .as_ref()
        .is_some_and(|s| s != "chain_verified" && s != "committee")
    {
        return Err(ApiError::bad("Invalid market kind"));
    }
    let rows =
        sqlx::query("SELECT definition_json, status FROM market ORDER BY created_at, market_id")
            .fetch_all(&state.read)
            .await?;
    let mut markets = Vec::new();
    let at = height(&state).await?;
    for row in rows {
        let def: MarketDefinition = serde_json::from_str(row.get(0))?;
        let status: String = row.get(1);
        let resolution_type = serde_json::to_value(def.resolution_type)?;
        let kind = if resolution_type == "COMMITTEE" {
            "committee"
        } else {
            "chain_verified"
        };
        if query.status.as_ref().is_some_and(|s| s != &status)
            || query
                .resolution_type
                .as_ref()
                .is_some_and(|s| s != &resolution_type)
            || query.kind.as_ref().is_some_and(|s| s != kind)
        {
            continue;
        }
        let odds = odds_at(&state, &def, at).await?;
        markets.push(json!({"market_id":def.market_id, "question":def.question, "status":status,
            "resolution_type":resolution_type, "close_height":def.close_height, "pool_total_zat":odds["pool_total_zat"],
            "odds":def.outcomes.iter().enumerate().map(|(i,o)| json!({"outcome_index":o.outcome_index,
                "label":o.label, "probability":odds["outcomes"][i]["probability"]})).collect::<Vec<_>>()}));
    }
    Ok(Json(json!({"markets":markets})))
}
#[derive(Default, Deserialize)]
pub(crate) struct BetsQuery {
    after_height: Option<u32>,
    limit: Option<u32>,
}
pub(crate) async fn bets(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<BetsQuery>,
) -> Result<Json<Value>> {
    market(&state, &id).await?;
    let rows = sqlx::query("SELECT note_ref, bet_fingerprint, outcome_index, amount_zat, block_height, status, tagged FROM bet WHERE market_id = ? AND is_announcement = 0 AND (? IS NULL OR block_height > ?) ORDER BY block_height, tx_index, action_index, note_ref LIMIT ?")
        .bind(&id).bind(query.after_height).bind(query.after_height).bind(query.limit.unwrap_or(100).min(500)).fetch_all(&state.read).await?;
    let mut bets: Vec<_> = rows
        .into_iter()
        .map(|r| PublicBet {
            note_ref: r.get(0),
            bet_fingerprint: r.get(1),
            outcome_index: r.get(2),
            amount_zat: r.get(3),
            block_height: r.get(4),
            status: r.get(5),
            tagged: r.get(6),
        })
        .collect();
    let settlement = Settlement::load(&state, &id).await?;
    for bet in &mut bets {
        bet.status = settlement.project(&bet.status, bet.outcome_index, &bet.note_ref);
    }
    Ok(Json(json!({"bets":bets})))
}

/// Resolution and payout plans of one market: the public projection of scanner statuses.
/// Scanner statuses stay untouched in the database for historical odds/math.
pub(crate) struct Settlement {
    resolution: Option<crate::types::Resolution>,
    pub(crate) items: zcash_glass::settle::PlanItems,
}
impl Settlement {
    pub(crate) async fn load(state: &AppState, id: &str) -> Result<Self> {
        let resolution: Option<String> = sqlx::query_scalar(
            "SELECT body_json FROM api_publication WHERE market_id=? AND kind='resolution'",
        )
        .bind(id)
        .fetch_optional(&state.read)
        .await?;
        let resolution = resolution
            .map(|body| serde_json::from_str::<crate::types::Resolution>(&body))
            .transpose()?;
        let bodies: Vec<String> =
            sqlx::query_scalar("SELECT body_json FROM api_payout_plan WHERE market_id=?")
                .bind(id)
                .fetch_all(&state.read)
                .await?;
        let plans = bodies
            .iter()
            .map(|body| serde_json::from_str::<crate::types::PayoutPublic>(body))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let items = zcash_glass::settle::PlanItems::from_plans(
            plans.iter().map(|p| (p.status.as_str(), &p.plan)),
        );
        Ok(Self { resolution, items })
    }
    pub(crate) fn view(&self) -> Option<zcash_glass::settle::ResolutionView<'_>> {
        self.resolution
            .as_ref()
            .map(|r| zcash_glass::settle::ResolutionView {
                status: &r.status,
                winning_outcome_index: r.winning_outcome_index,
            })
    }
    /// The status a visitor sees for a bet with scanner status `status`.
    pub(crate) fn project(&self, status: &str, outcome: Option<u32>, note_ref: &str) -> String {
        zcash_glass::settle::project_status(status, outcome, note_ref, self.view(), &self.items)
    }
}
pub(crate) async fn published(state: &AppState, id: &str, kind: &str) -> Result<Value> {
    market(state, id).await?;
    let body: Option<String> = sqlx::query_scalar(
        "SELECT body_json FROM api_publication WHERE market_id = ? AND kind = ?",
    )
    .bind(id)
    .bind(kind)
    .fetch_optional(&state.read)
    .await?;
    Ok(body
        .map(|body| serde_json::from_str(&body))
        .transpose()?
        .unwrap_or(Value::Null))
}
pub(crate) async fn resolution(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    Ok(Json(published(&state, &id, "resolution").await?))
}
pub(crate) async fn payout(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    Ok(Json(published(&state, &id, "payout").await?))
}
/// Every plan of the market, oldest first, so a verifier can project settlement the way the API does.
pub(crate) async fn plans(state: &AppState, id: &str) -> Result<Vec<Value>> {
    let bodies: Vec<String> = sqlx::query_scalar(
        "SELECT body_json FROM api_payout_plan WHERE market_id = ? ORDER BY rowid",
    )
    .bind(id)
    .fetch_all(&state.read)
    .await?;
    Ok(bodies
        .iter()
        .map(|b| serde_json::from_str::<Value>(b))
        .collect::<std::result::Result<Vec<_>, _>>()?)
}
pub(crate) async fn payouts(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    market(&state, &id).await?;
    Ok(Json(json!({"plans": plans(&state, &id).await?})))
}
pub(crate) async fn registry(State(state): State<AppState>) -> Result<Json<Value>> {
    let row =
        sqlx::query("SELECT uivk, address, birthday_height FROM persona_registry WHERE id = 1")
            .fetch_optional(&state.read)
            .await?;
    Ok(Json(row.map(|r| json!({"uivk":r.get::<String,_>(0), "address":r.get::<String,_>(1), "birthday_height":r.get::<u32,_>(2)})).unwrap_or(Value::Null)))
}

pub(crate) async fn persona(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    let row=sqlx::query("SELECT persona_id, commitment, registered_height, status, alias, thawed_at, k_root FROM persona WHERE persona_id = ?")
        .bind(id).fetch_optional(&state.read).await?.ok_or(ApiError(StatusCode::NOT_FOUND,"PERSONA_NOT_FOUND","Persona does not exist".into()))?;
    let status: String = row.get(3);
    let mut value = json!({"persona_id":row.get::<String,_>(0),"commitment":hex::encode(row.get::<Vec<u8>,_>(1)),
        "registered_height":row.get::<u32,_>(2),"status":status,"alias":row.get::<Option<String>,_>(4),"thawed_at":row.get::<Option<i64>,_>(5)});
    if status == "THAWED"
        && let Some(root) = row.get::<Option<Vec<u8>>, _>(6)
    {
        value["k_root_b64"] = json!(URL_SAFE_NO_PAD.encode(root));
    }
    Ok(Json(value))
}
#[derive(Default, Deserialize)]
pub(crate) struct LeaderboardQuery {
    min_bets: Option<u32>,
}
pub(crate) async fn leaderboard(
    State(state): State<AppState>,
    Query(query): Query<LeaderboardQuery>,
) -> Result<Json<Value>> {
    let rows=sqlx::query("SELECT p.persona_id, p.alias, p.registered_height, t.report_json FROM persona p JOIN thaw_report t ON t.persona_id = p.persona_id WHERE p.status = 'THAWED'").fetch_all(&state.read).await?;
    let mut entries = Vec::new();
    for row in rows {
        #[derive(Deserialize)]
        struct Stats {
            settled: u32,
            won: u32,
            win_rate: Option<f64>,
            roi: Option<f64>,
        }
        #[derive(Deserialize)]
        struct Report {
            stats: Stats,
        }
        let stats = serde_json::from_str::<Report>(row.get(3))?.stats;
        if stats.settled < query.min_bets.unwrap_or(0) {
            continue;
        }
        entries.push(json!({"persona_id":row.get::<String,_>(0),"alias":row.get::<Option<String>,_>(1),"registered_height":row.get::<u32,_>(2),
            "settled":stats.settled,"won":stats.won,"win_rate":stats.win_rate,"roi":stats.roi}));
    }
    entries.sort_by(|a, b| {
        b["roi"]
            .as_f64()
            .unwrap_or(0.0)
            .total_cmp(&a["roi"].as_f64().unwrap_or(0.0))
            .then_with(|| a["persona_id"].as_str().cmp(&b["persona_id"].as_str()))
    });
    Ok(Json(json!({"entries":entries})))
}

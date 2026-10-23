//! Persona Thaw (docs/PROJECT_SPEC_FROSTED.md SD-5, ADR-0002): J-11 thaw worker and report cache.
//!
//! The report is a pure function of public data (`zcash_glass::thaw::report`), so
//! `frosted-verify thaw` produces the same bytes. `K_root` is stored only after a successful thaw.

use std::collections::BTreeMap;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Deserialize;
use sqlx::Row;
use zcash_glass::thaw::{self, ReportBet, TaggedBet};

use crate::{
    AppState,
    auth::now,
    error::{ApiError, Result},
    public::{self, Settlement},
};

const MAX_ALIAS_CHARS: usize = 32;

fn not_found() -> ApiError {
    ApiError(
        StatusCode::NOT_FOUND,
        "PERSONA_NOT_FOUND",
        "Persona does not exist".into(),
    )
}

fn valid_alias(alias: &str) -> bool {
    (1..=MAX_ALIAS_CHARS).contains(&alias.chars().count())
        && alias
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

/// Recomputes the report from public bets of every market.
async fn compute(
    state: &AppState,
    k_root: &[u8; 32],
    alias: &str,
    registered_height: u32,
) -> Result<thaw::ThawReport> {
    let rows = sqlx::query(
        "SELECT note_ref, market_id, outcome_index, amount_zat, block_height, tx_index, action_index, status, thaw_tag, memo_payload
         FROM bet WHERE tagged = 1 AND is_announcement = 0 AND thaw_tag IS NOT NULL AND memo_payload IS NOT NULL
         AND outcome_index IS NOT NULL",
    )
    .fetch_all(&state.read)
    .await?;
    let mut settlements: BTreeMap<String, Settlement> = BTreeMap::new();
    let mut bets = Vec::new();
    let mut results = BTreeMap::new();
    for row in rows {
        let (note_ref, market_id): (String, String) = (row.get(0), row.get(1));
        // Rows that cannot be parsed are not tagged bets of anyone: skip rather than fail the report.
        let (Ok(market_bytes), Ok(thaw_tag), Ok(outcome)) = (
            hex::decode(&market_id)
                .ok()
                .and_then(|b| <[u8; 16]>::try_from(b).ok())
                .ok_or(()),
            <[u8; 32]>::try_from(row.get::<Vec<u8>, _>(8)).map_err(|_| ()),
            u8::try_from(row.get::<u32, _>(2)).map_err(|_| ()),
        ) else {
            continue;
        };
        let payload: Vec<u8> = row.get(9);
        let Ok(payload) = zcash_glass::memo::Frst1Payload::from_bytes(&payload) else {
            continue;
        };
        if !settlements.contains_key(&market_id) {
            let loaded = Settlement::load(state, &market_id).await?;
            settlements.insert(market_id.clone(), loaded);
        }
        let settlement = &settlements[&market_id];
        results.insert(
            note_ref.clone(),
            zcash_glass::settle::settlement(
                &row.get::<String, _>(7),
                Some(u32::from(outcome)),
                &note_ref,
                settlement.view(),
                &settlement.items,
            ),
        );
        bets.push(ReportBet {
            note_ref,
            bet: TaggedBet {
                market_id: market_bytes,
                outcome_index: outcome,
                amount_zat: row.get::<i64, _>(3) as u64,
                env_epk: payload.env_epk,
                has_thaw_tag: true,
                thaw_tag,
            },
            block_height: row.get(4),
            tx_index: row.get(5),
            action_index: row.get(6),
        });
    }
    Ok(thaw::report(
        k_root,
        alias,
        registered_height,
        &bets,
        &results,
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ThawRequest {
    k_root_b64: String,
    alias: String,
}

pub(crate) async fn thaw(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<ThawRequest>,
) -> Result<Json<thaw::ThawReport>> {
    let row = sqlx::query(
        "SELECT commitment, registered_height, status FROM persona WHERE persona_id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.read)
    .await?
    .ok_or_else(not_found)?;
    let commitment = <[u8; 32]>::try_from(row.get::<Vec<u8>, _>(0))
        .map_err(|_| ApiError::bad("Stored commitment is invalid"))?;
    let registered_height: u32 = row.get(1);
    let k_root = URL_SAFE_NO_PAD
        .decode(request.k_root_b64.trim_end_matches('='))
        .ok()
        .and_then(|b| <[u8; 32]>::try_from(b).ok())
        .ok_or_else(|| ApiError::bad("k_root_b64 must be 32 bytes of base64url"))?;
    if !thaw::commitment_matches(&k_root, &commitment) {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "THAW_COMMITMENT_MISMATCH",
            "K_root does not match the registered commitment".into(),
        ));
    }
    if row.get::<String, _>(2) != "REGISTERED" {
        return Err(already_thawed());
    }
    let alias = request.alias.trim();
    if !valid_alias(alias) {
        return Err(ApiError::bad(
            "Alias must be 1-32 characters: letters, digits, '_', '-' or '.'",
        ));
    }
    let computed_height = public::height(&state).await?;
    let report = compute(&state, &k_root, alias, registered_height).await?;
    // Status flip, public key and cached report commit together; the guard makes a race a 409.
    let mut tx = state.write.begin().await?;
    let flipped = sqlx::query(
        "UPDATE persona SET status = 'THAWED', alias = ?, k_root = ?, thawed_at = ? WHERE persona_id = ? AND status = 'REGISTERED'",
    )
    .bind(alias)
    .bind(k_root.as_slice())
    .bind(now() as i64)
    .bind(&id)
    .execute(&mut *tx)
    .await?;
    if flipped.rows_affected() != 1 {
        return Err(already_thawed());
    }
    sqlx::query("INSERT OR REPLACE INTO thaw_report (persona_id, report_json, computed_height) VALUES (?, ?, ?)")
        .bind(&id)
        .bind(serde_json::to_string(&report)?)
        .bind(computed_height)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(report))
}

fn already_thawed() -> ApiError {
    ApiError(
        StatusCode::CONFLICT,
        "PERSONA_ALREADY_THAWED",
        "Persona is already thawed".into(),
    )
}

/// Recomputes a thawed Persona's cached report if the indexer moved since it was computed.
pub(crate) async fn refresh(state: &AppState, id: &str) -> Result<Option<thaw::ThawReport>> {
    let Some(row) = sqlx::query(
        "SELECT p.alias, p.k_root, p.registered_height, t.report_json, t.computed_height
         FROM persona p JOIN thaw_report t ON t.persona_id = p.persona_id WHERE p.persona_id = ? AND p.status = 'THAWED'",
    )
    .bind(id)
    .fetch_optional(&state.read)
    .await?
    else {
        return Ok(None);
    };
    let height = public::height(state).await?;
    let cached: thaw::ThawReport = serde_json::from_str(row.get(3))?;
    if row.get::<u32, _>(4) >= height {
        return Ok(Some(cached));
    }
    let Some(k_root) = row
        .get::<Option<Vec<u8>>, _>(1)
        .and_then(|b| <[u8; 32]>::try_from(b).ok())
    else {
        return Ok(Some(cached));
    };
    let alias: Option<String> = row.get(0);
    let report = compute(
        state,
        &k_root,
        alias.as_deref().unwrap_or_default(),
        row.get(2),
    )
    .await?;
    sqlx::query("UPDATE thaw_report SET report_json = ?, computed_height = ? WHERE persona_id = ?")
        .bind(serde_json::to_string(&report)?)
        .bind(height)
        .bind(id)
        .execute(&state.write)
        .await?;
    Ok(Some(report))
}

pub(crate) async fn report(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<thaw::ThawReport>> {
    refresh(&state, &id).await?.map(Json).ok_or_else(|| {
        ApiError(
            StatusCode::NOT_FOUND,
            "PERSONA_NOT_FOUND",
            "Thaw report does not exist".into(),
        )
    })
}

/// Keeps leaderboard stats fresh as markets settle after the thaw.
pub async fn refresh_all(state: &AppState) -> Result<()> {
    let ids: Vec<String> = sqlx::query_scalar("SELECT persona_id FROM thaw_report")
        .fetch_all(&state.read)
        .await?;
    for id in ids {
        refresh(state, &id).await?;
    }
    Ok(())
}

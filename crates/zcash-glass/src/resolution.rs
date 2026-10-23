//! Chain-Verified resolution (docs/PROJECT_SPEC_FROSTED.md §19.4, ADR-0016).
//!
//! Pure over what a node reports: `getblock(resolve_height, 1)` and `getblockchaininfo`. The
//! coordinator and `frosted-verify` call the same function, so they cannot disagree on rules.

use serde_json::{Value, json};
use thiserror::Error;

use crate::hash::{MarketDefinition, ResolutionParams};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ResolveError {
    /// The node response lacks the pool or upgrade data the rule needs.
    #[error("NODE_UNAVAILABLE")]
    NodeUnavailable,
    /// Committee markets, or comparator values outside `gte`/`lt`.
    #[error("INVALID_RESOLUTION_PARAMS")]
    InvalidParams,
}

/// Public `Resolution` body (§22.3) for a `POOL_THRESHOLD` or `UPGRADE_ACTIVE` market. Outcome 0
/// is YES. An upgrade missing from `upgrades` counts as not active (outcome 1).
pub fn resolve(def: &MarketDefinition, block: &Value, info: &Value) -> Result<Value, ResolveError> {
    let hash = block["hash"]
        .as_str()
        .ok_or(ResolveError::NodeUnavailable)?;
    let (yes, evidence) = match &def.resolution_params {
        ResolutionParams::PoolThreshold(p) => {
            let value = block["valuePools"]
                .as_array()
                .ok_or(ResolveError::NodeUnavailable)?
                .iter()
                .find(|v| v["id"] == p.pool)
                .and_then(|v| v["chainValueZat"].as_u64())
                .ok_or(ResolveError::NodeUnavailable)?;
            let yes = match p.comparator.as_str() {
                "gte" => value >= p.threshold_zat,
                "lt" => value < p.threshold_zat,
                _ => return Err(ResolveError::InvalidParams),
            };
            (
                yes,
                json!({"source":"getblock","height":def.resolve_height,"block_hash":hash,"pool":p.pool,"chain_value_zat":value}),
            )
        }
        ResolutionParams::UpgradeActive(p) => {
            let list = reported_upgrades(info)?;
            let yes = list
                .iter()
                .any(|(name, h)| name == &p.upgrade_name && u64::from(def.resolve_height) >= *h);
            let list: Vec<Value> = list
                .into_iter()
                .map(|(name, h)| json!({"name":name,"activation_height":h}))
                .collect();
            (
                yes,
                json!({"source":"getblockchaininfo","height":def.resolve_height,"block_hash":hash,"reported_upgrades":list}),
            )
        }
        ResolutionParams::Committee(_) => return Err(ResolveError::InvalidParams),
    };
    Ok(
        json!({"market_id":def.market_id,"status":"RESOLVED","winning_outcome_index":if yes{0}else{1},"evidence":evidence,"votes":[]}),
    )
}

/// `(name, activationheight)` of every upgrade the node lists (ADR-0016).
pub fn reported_upgrades(info: &Value) -> Result<Vec<(String, u64)>, ResolveError> {
    info["upgrades"]
        .as_object()
        .ok_or(ResolveError::NodeUnavailable)?
        .values()
        .map(|v| {
            Ok((
                v["name"]
                    .as_str()
                    .ok_or(ResolveError::NodeUnavailable)?
                    .to_owned(),
                v["activationheight"]
                    .as_u64()
                    .ok_or(ResolveError::NodeUnavailable)?,
            ))
        })
        .collect()
}

/// A resolved market whose winning outcome has no counted money becomes `VOID` (everyone is refunded).
pub fn void_if_unbacked(body: &mut Value, winning_pool_zat: u64) {
    if winning_pool_zat == 0 {
        body["status"] = json!("VOID");
        body["winning_outcome_index"] = Value::Null;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(params: Value, kind: &str) -> MarketDefinition {
        serde_json::from_value(json!({"version":1,"params_version":1,"network":"regtest","market_id":"a1".repeat(16),
            "question":"q","outcomes":[{"outcome_index":0,"label":"YES","address":"a"},{"outcome_index":1,"label":"NO","address":"b"}],
            "resolution_type":kind,"resolution_params":params,"birthday_height":1,"close_height":5,"resolve_height":10,"fee_bps":100,
            "min_bet_zat":1,"refund_fee_zat":1,"min_confirmations":1,"sweep_threshold_zat":1,"announce_timeout_blocks":1,
            "uivk":"u","envelope_pk":"p","treasury_address":"t"})).unwrap()
    }

    #[test]
    fn pool_threshold_both_comparators() {
        let block = json!({"hash":"h","valuePools":[{"id":"ironwood","chainValueZat":100}]});
        let gte = def(
            json!({"pool":"ironwood","comparator":"gte","threshold_zat":100}),
            "POOL_THRESHOLD",
        );
        assert_eq!(
            resolve(&gte, &block, &Value::Null).unwrap()["winning_outcome_index"],
            0
        );
        let lt = def(
            json!({"pool":"ironwood","comparator":"lt","threshold_zat":100}),
            "POOL_THRESHOLD",
        );
        assert_eq!(
            resolve(&lt, &block, &Value::Null).unwrap()["winning_outcome_index"],
            1
        );
        let bad = def(
            json!({"pool":"ironwood","comparator":"eq","threshold_zat":100}),
            "POOL_THRESHOLD",
        );
        assert_eq!(
            resolve(&bad, &block, &Value::Null),
            Err(ResolveError::InvalidParams)
        );
        let missing = def(
            json!({"pool":"orchard","comparator":"gte","threshold_zat":1}),
            "POOL_THRESHOLD",
        );
        assert_eq!(
            resolve(&missing, &block, &Value::Null),
            Err(ResolveError::NodeUnavailable)
        );
    }

    #[test]
    fn unlisted_upgrade_is_not_active() {
        let d = def(json!({"upgrade_name":"NU7"}), "UPGRADE_ACTIVE");
        let block = json!({"hash":"h"});
        let listed = json!({"upgrades":{"a":{"name":"NU6.3","activationheight":10}}});
        let body = resolve(&d, &block, &listed).unwrap();
        assert_eq!(body["winning_outcome_index"], 1);
        assert_eq!(body["evidence"]["reported_upgrades"][0]["name"], "NU6.3");
        let active = json!({"upgrades":{"a":{"name":"NU7","activationheight":10}}});
        assert_eq!(
            resolve(&d, &block, &active).unwrap()["winning_outcome_index"],
            0
        );
        let later = json!({"upgrades":{"a":{"name":"NU7","activationheight":11}}});
        assert_eq!(
            resolve(&d, &block, &later).unwrap()["winning_outcome_index"],
            1
        );
        let broken = json!({"upgrades":{"a":{"name":"NU7","activationheight":null}}});
        assert_eq!(
            resolve(&d, &block, &broken),
            Err(ResolveError::NodeUnavailable)
        );
    }

    #[test]
    fn unbacked_winner_voids() {
        let mut body = json!({"status":"RESOLVED","winning_outcome_index":0});
        void_if_unbacked(&mut body, 1);
        assert_eq!(body["status"], "RESOLVED");
        void_if_unbacked(&mut body, 0);
        assert_eq!(body, json!({"status":"VOID","winning_outcome_index":null}));
    }
}

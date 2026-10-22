use crate::{config::Config, keys::EscrowKeys};
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use zcash_glass::{
    hash::{MarketDefinition, OutcomeDefinition, ResolutionParams, ResolutionType},
    params::*,
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateMarket {
    pub question: String,
    pub outcomes: Vec<String>,
    pub resolution_type: ResolutionType,
    pub resolution_params: ResolutionParams,
    pub close_height: u32,
    pub resolve_height: u32,
    pub fee_bps: u32,
}
impl CreateMarket {
    pub fn validate(
        &self,
        tip: u32,
        min_bet: u64,
        mainnet_balance: Option<u64>,
        mainnet_limit: u64,
    ) -> Result<()> {
        if self.close_height <= tip {
            bail!("INVALID_CLOSE_HEIGHT");
        }
        if self.resolve_height < self.close_height {
            bail!("INVALID_RESOLVE_HEIGHT");
        }
        if self.fee_bps > 10000
            || u128::from(min_bet) * u128::from(self.fee_bps) / 10000 < u128::from(MARGINAL_FEE_ZAT)
        {
            bail!("INVALID_FEE_BPS");
        }
        if mainnet_balance.is_some_and(|b| b >= mainnet_limit) {
            bail!("MAINNET_DEMO_LIMIT");
        }
        let valid = match (self.resolution_type, &self.resolution_params) {
            (ResolutionType::PoolThreshold, ResolutionParams::PoolThreshold(p)) => {
                [
                    "transparent",
                    "sprout",
                    "sapling",
                    "orchard",
                    "ironwood",
                    "lockbox",
                ]
                .contains(&p.pool.as_str())
                    && ["gte", "lt"].contains(&p.comparator.as_str())
                    && p.threshold_zat < (1 << 53)
                    && self.outcomes == ["YES", "NO"]
            }
            (ResolutionType::UpgradeActive, ResolutionParams::UpgradeActive(p)) => {
                !p.upgrade_name.trim().is_empty()
                    && p.upgrade_name.len() <= 100
                    && self.outcomes == ["YES", "NO"]
            }
            (ResolutionType::Committee, ResolutionParams::Committee(p)) => {
                !p.criteria.trim().is_empty()
                    && !p.evidence_source.trim().is_empty()
                    && p.dispute_window_blocks > 0
                    && p.void_after_blocks >= p.dispute_window_blocks
            }
            _ => false,
        };
        if !valid
            || self.question.trim().is_empty()
            || self.question.len() > 2000
            || !(2..=4).contains(&self.outcomes.len())
            || self.outcomes.iter().enumerate().any(|(i, s)| {
                s.trim().is_empty() || s.len() > 100 || self.outcomes[..i].contains(s)
            })
        {
            bail!("INVALID_RESOLUTION_PARAMS");
        }
        Ok(())
    }
    pub fn definition(
        self,
        config: &Config,
        id: String,
        tip: u32,
        keys: &EscrowKeys,
        envelope_pk: String,
    ) -> MarketDefinition {
        MarketDefinition {
            version: 1,
            params_version: PARAMS_VERSION,
            network: config.network.clone(),
            market_id: id,
            question: self.question,
            outcomes: self
                .outcomes
                .into_iter()
                .zip(&keys.addresses)
                .enumerate()
                .map(|(i, (label, address))| OutcomeDefinition {
                    outcome_index: i as u32,
                    label,
                    address: address.clone(),
                })
                .collect(),
            resolution_type: self.resolution_type,
            resolution_params: self.resolution_params,
            birthday_height: tip,
            close_height: self.close_height,
            resolve_height: self.resolve_height,
            fee_bps: self.fee_bps,
            min_bet_zat: config.min_bet_zat,
            refund_fee_zat: REFUND_FEE_ZAT,
            min_confirmations: config.min_confirmations,
            sweep_threshold_zat: SWEEP_THRESHOLD_ZAT,
            announce_timeout_blocks: ANNOUNCE_TIMEOUT_BLOCKS,
            uivk: keys.uivk.clone(),
            envelope_pk,
            treasury_address: config.treasury.clone(),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> CreateMarket {
        serde_json::from_str(r#"{"question":"Pool?","outcomes":["YES","NO"],"resolution_type":"POOL_THRESHOLD","resolution_params":{"pool":"ironwood","comparator":"gte","threshold_zat":500000000},"close_height":100,"resolve_height":110,"fee_bps":100}"#).unwrap()
    }
    #[test]
    fn validates_height_fee_cap_and_resolution() {
        let mut m = input();
        assert!(
            m.validate(90, MIN_BET_ZAT, None, MAX_MAINNET_DEMO_ZAT)
                .is_ok()
        );
        assert_eq!(
            m.validate(100, MIN_BET_ZAT, None, MAX_MAINNET_DEMO_ZAT)
                .unwrap_err()
                .to_string(),
            "INVALID_CLOSE_HEIGHT"
        );
        m.fee_bps = 10;
        assert_eq!(
            m.validate(90, MIN_BET_ZAT, None, MAX_MAINNET_DEMO_ZAT)
                .unwrap_err()
                .to_string(),
            "INVALID_FEE_BPS"
        );
        m.fee_bps = 100;
        assert_eq!(
            m.validate(
                90,
                MIN_BET_ZAT,
                Some(MAX_MAINNET_DEMO_ZAT),
                MAX_MAINNET_DEMO_ZAT
            )
            .unwrap_err()
            .to_string(),
            "MAINNET_DEMO_LIMIT"
        );
        m.resolve_height = 99;
        assert_eq!(
            m.validate(90, MIN_BET_ZAT, None, MAX_MAINNET_DEMO_ZAT)
                .unwrap_err()
                .to_string(),
            "INVALID_RESOLVE_HEIGHT"
        );
    }
    #[test]
    fn resolution_contract_rejects_unrecognized_comparators_and_labels() {
        let mut m = input();
        m.outcomes.swap(0, 1);
        assert!(
            m.validate(90, MIN_BET_ZAT, None, MAX_MAINNET_DEMO_ZAT)
                .is_err()
        );
        m.outcomes.swap(0, 1);
        if let ResolutionParams::PoolThreshold(p) = &mut m.resolution_params {
            p.comparator = "eq".into();
        }
        assert!(
            m.validate(90, MIN_BET_ZAT, None, MAX_MAINNET_DEMO_ZAT)
                .is_err()
        );
        if let ResolutionParams::PoolThreshold(p) = &mut m.resolution_params {
            p.comparator = "lt".into();
            p.pool = "lockbox".into();
        }
        assert!(
            m.validate(90, MIN_BET_ZAT, None, MAX_MAINNET_DEMO_ZAT)
                .is_ok()
        );
        m.resolution_type = ResolutionType::UpgradeActive;
        assert!(
            m.validate(90, MIN_BET_ZAT, None, MAX_MAINNET_DEMO_ZAT)
                .is_err()
        );
    }
}

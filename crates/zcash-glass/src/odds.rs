//! Pool and implied probability per outcome (docs/PROJECT_SPEC_FROSTED.md F-04, §22.3 `Odds`).
//! Only `COUNTED` bets contribute; probabilities are for display, pools are the source of truth.

use serde::Serialize;

use crate::status::BetStatus;

#[derive(Clone, Copy, Debug)]
pub struct OddsBet {
    pub outcome_index: u32,
    pub amount_zat: u64,
    pub status: BetStatus,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct OutcomeOdds {
    pub outcome_index: u32,
    pub pool_zat: u64,
    pub bets: u32,
    /// `None` (API `null`, UI "—") when no bet is counted yet.
    pub probability: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Odds {
    pub pool_total_zat: u64,
    pub outcomes: Vec<OutcomeOdds>,
    pub pending_bets: u32,
}

/// Computes odds for a market with `outcome_count` outcomes. Bets on unknown outcome indices are
/// ignored (the indexer classifies them as `UNKNOWN_OUTCOME` before this point).
pub fn compute(outcome_count: u32, bets: &[OddsBet]) -> Odds {
    let mut outcomes: Vec<OutcomeOdds> = (0..outcome_count)
        .map(|outcome_index| OutcomeOdds {
            outcome_index,
            pool_zat: 0,
            bets: 0,
            probability: None,
        })
        .collect();
    let mut pending_bets = 0;
    for bet in bets {
        match bet.status {
            BetStatus::Counted => {
                if let Some(o) = outcomes.get_mut(bet.outcome_index as usize) {
                    o.pool_zat += bet.amount_zat;
                    o.bets += 1;
                }
            }
            BetStatus::Pending => pending_bets += 1,
            _ => {}
        }
    }
    let pool_total_zat: u64 = outcomes.iter().map(|o| o.pool_zat).sum();
    if pool_total_zat > 0 {
        for o in &mut outcomes {
            o.probability = Some(o.pool_zat as f64 / pool_total_zat as f64);
        }
    }
    Odds {
        pool_total_zat,
        outcomes,
        pending_bets,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bet(outcome_index: u32, amount_zat: u64, status: BetStatus) -> OddsBet {
        OddsBet {
            outcome_index,
            amount_zat,
            status,
        }
    }

    #[test]
    fn example_market() {
        let mut bets = vec![];
        for a in [
            100_000_000,
            200_000_000,
            50_000_000,
            120_000_000,
            150_000_000,
            150_000_000,
        ] {
            bets.push(bet(0, a, BetStatus::Counted));
        }
        for a in [200_000_000, 170_000_000, 100_000_000] {
            bets.push(bet(1, a, BetStatus::Counted));
        }
        bets.push(bet(1, 5_000_000, BetStatus::Pending));
        let odds = compute(2, &bets);
        assert_eq!(odds.pool_total_zat, 1_240_000_000);
        assert_eq!(odds.outcomes[0].pool_zat, 770_000_000);
        assert_eq!(odds.outcomes[0].bets, 6);
        assert_eq!(odds.pending_bets, 1);
        let p0 = odds.outcomes[0].probability.unwrap();
        assert_eq!(format!("{p0:.4}"), "0.6210");
        let sum: f64 = odds.outcomes.iter().map(|o| o.probability.unwrap()).sum();
        assert!((sum - 1.0).abs() < 1e-9);
    }

    #[test]
    fn empty_pool_has_no_probability() {
        let odds = compute(2, &[bet(0, 5_000_000, BetStatus::Pending)]);
        assert_eq!(odds.pool_total_zat, 0);
        assert!(odds.outcomes.iter().all(|o| o.probability.is_none()));
        assert_eq!(
            serde_json::to_value(&odds).unwrap()["outcomes"][0]["probability"],
            serde_json::Value::Null
        );
    }

    #[test]
    fn non_counted_statuses_are_excluded() {
        let statuses = [
            BetStatus::Late,
            BetStatus::RejectedTransparent,
            BetStatus::IgnoredDust,
            BetStatus::UneconomicDust,
            BetStatus::UnknownOutcome,
            BetStatus::InvalidMemo,
        ];
        let bets: Vec<_> = statuses.iter().map(|s| bet(0, 9_000_000, *s)).collect();
        let odds = compute(2, &bets);
        assert_eq!(odds.pool_total_zat, 0);
        assert_eq!(odds.pending_bets, 0);
    }
}

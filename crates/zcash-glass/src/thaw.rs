//! Persona commitment and Thaw Tag v2 (docs/PROJECT_SPEC_FROSTED.md §19.3, ADR-0002).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::blake2b_256;
use crate::params::{PERSONAL_PERSONA, PERSONAL_THAW_TAG};

/// `BLAKE2b-256(personal = "FrostedPersona01", data = K_root)`.
pub fn commitment(k_root: &[u8; 32]) -> [u8; 32] {
    blake2b_256(&PERSONAL_PERSONA, None, &[k_root])
}

/// `hex(persona_commitment[0..16])` (32 hex characters).
pub fn persona_id(commitment: &[u8; 32]) -> String {
    hex::encode(&commitment[..16])
}

/// True when `k_root` reproduces the registered commitment; a false result is returned by the
/// API as `THAW_COMMITMENT_MISMATCH`.
pub fn commitment_matches(k_root: &[u8; 32], registered: &[u8; 32]) -> bool {
    &commitment(k_root) == registered
}

/// Thaw Tag v2: `BLAKE2b-256(key = K_root, personal = "FrostedThawTag02",
/// data = market_id ‖ outcome_index u8 ‖ amount_zat u64 LE ‖ env_epk)`. No counter.
pub fn tag(
    k_root: &[u8; 32],
    market_id: &[u8; 16],
    outcome_index: u8,
    amount_zat: u64,
    env_epk: &[u8; 32],
) -> [u8; 32] {
    blake2b_256(
        &PERSONAL_THAW_TAG,
        Some(k_root),
        &[
            market_id,
            &[outcome_index],
            &amount_zat.to_le_bytes(),
            env_epk,
        ],
    )
}

/// The public data of one bet needed to check a Thaw Tag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TaggedBet {
    pub market_id: [u8; 16],
    pub outcome_index: u8,
    /// The on-chain note value (not the amount in the Bet Receipt).
    pub amount_zat: u64,
    pub env_epk: [u8; 32],
    pub has_thaw_tag: bool,
    pub thaw_tag: [u8; 32],
}

/// True only when the bet is tagged and the recomputed tag equals its `thaw_tag`.
pub fn matches(k_root: &[u8; 32], bet: &TaggedBet) -> bool {
    bet.has_thaw_tag
        && tag(
            k_root,
            &bet.market_id,
            bet.outcome_index,
            bet.amount_zat,
            &bet.env_epk,
        ) == bet.thaw_tag
}

/// A tagged bet as stored publicly, with the position that orders duplicates (ADR-0002).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReportBet {
    pub note_ref: String,
    pub bet: TaggedBet,
    pub block_height: u32,
    pub tx_index: u32,
    pub action_index: u32,
}

/// How a bet ended, already projected from resolution and payout data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BetResult {
    /// Counted but the market has no final outcome yet, or a winner whose payout is unpublished.
    Pending,
    Won,
    Lost,
    Refunded,
    /// Any other terminal state (late, swept, unclaimable, ...).
    Other,
}

/// Settlement of one bet. `payout_zat` is the Win item amount of the published plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settlement {
    pub result: BetResult,
    pub payout_zat: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Exclusion {
    PreRegistration,
    DuplicateTag,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReportEntry {
    pub market_id: String,
    pub note_ref: String,
    pub outcome_index: u8,
    pub amount_zat: u64,
    pub block_height: u32,
    pub result: BetResult,
    pub payout_zat: Option<u64>,
    pub counted_in_reputation: bool,
    pub exclusion: Option<Exclusion>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stats {
    pub settled: u32,
    pub won: u32,
    pub win_rate: Option<f64>,
    pub roi: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThawReport {
    pub persona_id: String,
    pub alias: String,
    pub registered_height: u32,
    pub bets: Vec<ReportEntry>,
    pub stats: Stats,
}

/// Three decimals keep the report byte-identical across implementations.
fn round3(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}

/// Pure Thaw Report (ADR-0002): recompute the tag of every tagged bet, keep the ones that match,
/// order them by (height, tx, action), mark `PRE_REGISTRATION` and later `DUPLICATE_TAG`, and
/// compute reputation only from settled, unexcluded bets. `settlements` is keyed by `note_ref`;
/// a missing entry means `Pending`. Used by `frosted-api` and the verifier alike.
pub fn report(
    k_root: &[u8; 32],
    alias: &str,
    registered_height: u32,
    bets: &[ReportBet],
    settlements: &BTreeMap<String, Settlement>,
) -> ThawReport {
    let mut mine: Vec<&ReportBet> = bets.iter().filter(|b| matches(k_root, &b.bet)).collect();
    mine.sort_by(|a, b| {
        (a.block_height, a.tx_index, a.action_index, &a.note_ref).cmp(&(
            b.block_height,
            b.tx_index,
            b.action_index,
            &b.note_ref,
        ))
    });
    let mut seen = BTreeSet::new();
    let (mut settled, mut won, mut stake, mut returned) = (0u32, 0u32, 0u128, 0u128);
    let entries = mine
        .into_iter()
        .map(|b| {
            let exclusion = if !seen.insert(b.bet.thaw_tag) {
                Some(Exclusion::DuplicateTag)
            } else if b.block_height < registered_height {
                Some(Exclusion::PreRegistration)
            } else {
                None
            };
            let s = settlements.get(&b.note_ref).copied().unwrap_or(Settlement {
                result: BetResult::Pending,
                payout_zat: None,
            });
            // A winner counts only once the plan publishes its payout, so ROI is never guessed.
            let payout = match s.result {
                BetResult::Won => s.payout_zat,
                BetResult::Lost => Some(0),
                _ => None,
            };
            let counted = exclusion.is_none() && payout.is_some();
            if counted {
                settled += 1;
                won += u32::from(s.result == BetResult::Won);
                stake += u128::from(b.bet.amount_zat);
                returned += u128::from(payout.unwrap_or(0));
            }
            ReportEntry {
                market_id: hex::encode(b.bet.market_id),
                note_ref: b.note_ref.clone(),
                outcome_index: b.bet.outcome_index,
                amount_zat: b.bet.amount_zat,
                block_height: b.block_height,
                result: s.result,
                payout_zat: s.payout_zat,
                counted_in_reputation: counted,
                exclusion,
            }
        })
        .collect();
    let stats = Stats {
        settled,
        won,
        win_rate: (settled > 0).then(|| round3(f64::from(won) / f64::from(settled))),
        roi: (stake > 0).then(|| round3((returned as f64 - stake as f64) / stake as f64)),
    };
    ThawReport {
        persona_id: persona_id(&commitment(k_root)),
        alias: alias.to_owned(),
        registered_height,
        bets: entries,
        stats,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const K: [u8; 32] = [0x5a; 32];

    fn bet() -> TaggedBet {
        let (m, o, a, e) = ([1u8; 16], 0u8, 50_000_000u64, [2u8; 32]);
        TaggedBet {
            market_id: m,
            outcome_index: o,
            amount_zat: a,
            env_epk: e,
            has_thaw_tag: true,
            thaw_tag: tag(&K, &m, o, a, &e),
        }
    }

    #[test]
    fn commitment_and_id() {
        let c = commitment(&K);
        assert_eq!(persona_id(&c).len(), 32);
        assert!(commitment_matches(&K, &c));
        assert!(!commitment_matches(&[0u8; 32], &c));
    }

    #[test]
    fn tag_binds_outcome_and_amount() {
        let b = bet();
        assert_ne!(
            b.thaw_tag,
            tag(&K, &b.market_id, 1, b.amount_zat, &b.env_epk)
        );
        assert_ne!(
            b.thaw_tag,
            tag(&K, &b.market_id, 0, b.amount_zat + 1, &b.env_epk)
        );
    }

    #[test]
    fn matching() {
        let b = bet();
        assert!(matches(&K, &b));
        assert!(!matches(&[0u8; 32], &b));
        assert!(!matches(
            &K,
            &TaggedBet {
                has_thaw_tag: false,
                ..b
            }
        ));
        // Amount edited in the wallet (V-25): the on-chain amount no longer matches.
        assert!(!matches(
            &K,
            &TaggedBet {
                amount_zat: 49_000_000,
                ..b
            }
        ));
    }

    fn rb(note: &str, outcome: u8, amount: u64, epk: u8, height: u32, tx: u32) -> ReportBet {
        let (m, e) = ([1u8; 16], [epk; 32]);
        ReportBet {
            note_ref: note.into(),
            bet: TaggedBet {
                market_id: m,
                outcome_index: outcome,
                amount_zat: amount,
                env_epk: e,
                has_thaw_tag: true,
                thaw_tag: tag(&K, &m, outcome, amount, &e),
            },
            block_height: height,
            tx_index: tx,
            action_index: 0,
        }
    }
    fn st(result: BetResult, payout: Option<u64>) -> Settlement {
        Settlement {
            result,
            payout_zat: payout,
        }
    }

    #[test]
    fn report_lists_wins_and_losses_and_orders() {
        let bets = [
            rb("c", 0, 10_000_000, 3, 30, 0),
            rb("a", 0, 10_000_000, 1, 10, 0),
            rb("b", 1, 10_000_000, 2, 20, 0),
        ];
        let s = BTreeMap::from([
            ("a".into(), st(BetResult::Won, Some(25_000_000))),
            ("b".into(), st(BetResult::Lost, None)),
            ("c".into(), st(BetResult::Won, Some(15_000_000))),
        ]);
        let r = report(&K, "orion", 5, &bets, &s);
        let order: Vec<_> = r.bets.iter().map(|b| b.note_ref.as_str()).collect();
        assert_eq!(order, ["a", "b", "c"]);
        assert_eq!(r.persona_id, persona_id(&commitment(&K)));
        assert_eq!(
            r.stats,
            Stats {
                settled: 3,
                won: 2,
                win_rate: Some(0.667),
                roi: Some(0.333)
            }
        );
        assert!(r.bets.iter().all(|b| b.counted_in_reputation));
    }

    #[test]
    fn foreign_and_untagged_bets_are_not_reported() {
        let mut other = rb("x", 0, 10_000_000, 9, 10, 0);
        other.bet.thaw_tag = [7; 32];
        let mut untagged = rb("y", 0, 10_000_000, 8, 10, 0);
        untagged.bet.has_thaw_tag = false;
        let r = report(&K, "a", 0, &[other, untagged], &BTreeMap::new());
        assert!(r.bets.is_empty());
        assert_eq!(r.stats.settled, 0);
        assert_eq!((r.stats.win_rate, r.stats.roi), (None, None));
    }

    #[test]
    fn pre_registration_is_excluded() {
        let bets = [
            rb("a", 0, 10_000_000, 1, 4, 0),
            rb("b", 0, 10_000_000, 2, 5, 0),
        ];
        let s = BTreeMap::from([
            ("a".into(), st(BetResult::Won, Some(20_000_000))),
            ("b".into(), st(BetResult::Lost, None)),
        ]);
        let r = report(&K, "a", 5, &bets, &s);
        assert_eq!(r.bets[0].exclusion, Some(Exclusion::PreRegistration));
        assert!(!r.bets[0].counted_in_reputation);
        assert_eq!(r.bets[1].exclusion, None);
        // The excluded win does not lift reputation.
        assert_eq!(r.stats.settled, 1);
        assert_eq!(r.stats.won, 0);
        assert_eq!(r.stats.roi, Some(-1.0));
    }

    #[test]
    fn duplicate_tag_keeps_first_occurrence_only() {
        // Same payload copied into a later transaction: identical data, identical tag.
        let first = rb("first", 0, 10_000_000, 1, 10, 2);
        let copy = rb("copy", 0, 10_000_000, 1, 11, 0);
        let same_height = rb("same", 0, 10_000_000, 1, 10, 3);
        let s = BTreeMap::from([
            ("first".into(), st(BetResult::Lost, None)),
            ("copy".into(), st(BetResult::Won, Some(30_000_000))),
            ("same".into(), st(BetResult::Won, Some(30_000_000))),
        ]);
        let r = report(&K, "a", 0, &[copy, same_height, first], &s);
        let by = |n: &str| r.bets.iter().find(|b| b.note_ref == n).unwrap();
        assert_eq!(by("first").exclusion, None);
        assert_eq!(by("same").exclusion, Some(Exclusion::DuplicateTag));
        assert_eq!(by("copy").exclusion, Some(Exclusion::DuplicateTag));
        assert_eq!(r.stats.settled, 1);
        assert_eq!(r.stats.won, 0);
    }

    #[test]
    fn unpublished_payout_and_refunds_are_not_settled() {
        let bets = [
            rb("w", 0, 10_000_000, 1, 10, 0),
            rb("r", 0, 20_000_000, 2, 11, 0),
            rb("p", 1, 10_000_000, 3, 12, 0),
        ];
        let s = BTreeMap::from([
            ("w".into(), st(BetResult::Won, None)),
            ("r".into(), st(BetResult::Refunded, Some(19_000_000))),
        ]);
        let r = report(&K, "a", 0, &bets, &s);
        assert_eq!(r.stats.settled, 0);
        assert_eq!(r.bets[2].result, BetResult::Pending);
        assert!(r.bets.iter().all(|b| !b.counted_in_reputation));
    }

    #[test]
    fn serializes_per_spec() {
        let r = report(&K, "orion", 5, &[rb("a", 0, 1, 1, 6, 0)], &BTreeMap::new());
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["bets"][0]["result"], "PENDING");
        assert!(v["bets"][0]["exclusion"].is_null());
        assert_eq!(v["stats"]["settled"], 0);
    }
}

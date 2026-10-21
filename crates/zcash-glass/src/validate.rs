//! Deterministic note classification (docs/PROJECT_SPEC_FROSTED.md §19.5, ADR-0009).
//!
//! Pure function over facts a public indexer or verifier can observe with the Market's UIVK.
//! Classification depends only on block height, never on the Market status, so every verifier
//! reaches the same result.

use crate::hash::MarketDefinition;
use crate::memo::{FRSTM1_PREFIX, Frst1Payload, decode_frst1, decode_frstm1};
use crate::status::BetStatus;

/// What the scanner learned about one decrypted escrow note.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteFacts<'a> {
    pub amount_zat: u64,
    pub block_height: u32,
    /// Diversifier index of the recipient address, if the UIVK maps it to one.
    pub diversifier_index: Option<u64>,
    /// Memo as text (ZIP 302 text memo, trailing zero bytes removed), if it is valid UTF-8.
    pub memo_text: Option<&'a str>,
    pub has_transparent_input: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Classification {
    /// The Market's announcement (first FRSTM1 with the matching `rules_hash` to outcome 0).
    Announcement,
    /// A note with a bet status. `payload` is set only when the FRST1 memo was valid.
    Note {
        status: BetStatus,
        outcome_index: Option<u32>,
        payload: Option<Frst1Payload>,
    },
}

impl Classification {
    pub fn status(&self) -> Option<BetStatus> {
        match self {
            Classification::Announcement => None,
            Classification::Note { status, .. } => Some(*status),
        }
    }
}

/// Classifies a note of Market `def`, applying §19.5 in order:
/// `UNECONOMIC_DUST`, `UNKNOWN_OUTCOME`, announcement, `REJECTED_TRANSPARENT`, `LATE`,
/// `IGNORED_DUST`, `INVALID_MEMO`, otherwise `PENDING`.
///
/// `rules_hash` is the hash of `def` (computed once by the caller); `announcement_seen` tells
/// whether a valid announcement was already recorded for this Market.
pub fn classify(
    facts: &NoteFacts<'_>,
    def: &MarketDefinition,
    rules_hash: &[u8; 32],
    announcement_seen: bool,
) -> Classification {
    let note = |status, outcome_index, payload| Classification::Note {
        status,
        outcome_index,
        payload,
    };

    if facts.amount_zat <= def.sweep_threshold_zat {
        return note(BetStatus::UneconomicDust, outcome_of(facts, def), None);
    }
    let Some(outcome_index) = outcome_of(facts, def) else {
        return note(BetStatus::UnknownOutcome, None, None);
    };
    let memo = facts.memo_text.unwrap_or("");

    if memo.starts_with(FRSTM1_PREFIX) {
        let valid = outcome_index == 0
            && !announcement_seen
            && decode_frstm1(memo).is_ok_and(|h| &h == rules_hash);
        return if valid {
            Classification::Announcement
        } else {
            note(BetStatus::InvalidMemo, Some(outcome_index), None)
        };
    }
    if facts.has_transparent_input {
        return note(
            BetStatus::RejectedTransparent,
            Some(outcome_index),
            decode(memo, def),
        );
    }
    if facts.block_height > def.close_height {
        return note(BetStatus::Late, Some(outcome_index), decode(memo, def));
    }
    if facts.amount_zat < def.min_bet_zat {
        return note(BetStatus::IgnoredDust, Some(outcome_index), None);
    }
    match decode(memo, def) {
        Some(payload) => note(BetStatus::Pending, Some(outcome_index), Some(payload)),
        None => note(BetStatus::InvalidMemo, Some(outcome_index), None),
    }
}

/// Outcome index of the note if its diversifier index is one of the Market's outcomes.
fn outcome_of(facts: &NoteFacts<'_>, def: &MarketDefinition) -> Option<u32> {
    let index = u32::try_from(facts.diversifier_index?).ok()?;
    def.outcomes
        .iter()
        .any(|o| o.outcome_index == index)
        .then_some(index)
}

/// FRST1 payload if the memo parses and carries this Market's id.
fn decode(memo: &str, def: &MarketDefinition) -> Option<Frst1Payload> {
    let payload = decode_frst1(memo).ok()?;
    (hex::encode(payload.header.market_id) == def.market_id).then_some(payload)
}

/// Confirmations of a note mined at `block_height` when the tip is `tip`.
pub fn confirmations(block_height: u32, tip: u32) -> u32 {
    tip.checked_sub(block_height).map_or(0, |d| d + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::{
        OutcomeDefinition, PoolThresholdParams, ResolutionParams, ResolutionType, rules_hash,
    };
    use crate::memo::{Frst1Header, encode_frst1, encode_frstm1};

    const MARKET_ID: [u8; 16] = [0xa1; 16];

    fn def() -> MarketDefinition {
        MarketDefinition {
            version: 1,
            params_version: 1,
            network: "regtest".into(),
            market_id: hex::encode(MARKET_ID),
            question: "q".into(),
            outcomes: ["YES", "NO"]
                .iter()
                .enumerate()
                .map(|(i, l)| OutcomeDefinition {
                    outcome_index: i as u32,
                    label: (*l).into(),
                    address: format!("uregtest{i}"),
                })
                .collect(),
            resolution_type: ResolutionType::PoolThreshold,
            resolution_params: ResolutionParams::PoolThreshold(PoolThresholdParams {
                pool: "ironwood".into(),
                comparator: "gte".into(),
                threshold_zat: 1,
            }),
            birthday_height: 100,
            close_height: 200,
            resolve_height: 210,
            fee_bps: 100,
            min_bet_zat: 1_000_000,
            refund_fee_zat: 5_000,
            min_confirmations: 3,
            sweep_threshold_zat: 5_000,
            announce_timeout_blocks: 100,
            uivk: "uivk".into(),
            envelope_pk: "pk".into(),
            treasury_address: "utreasury".into(),
        }
    }

    fn frst1(market_id: [u8; 16]) -> String {
        encode_frst1(&Frst1Payload {
            header: Frst1Header::new(market_id, None),
            env_epk: [1; 32],
            env_nonce: [2; 24],
            env_ciphertext: [3; 67],
        })
        .unwrap()
    }

    fn facts(amount_zat: u64, height: u32, index: u64, memo: &str) -> NoteFacts<'_> {
        NoteFacts {
            amount_zat,
            block_height: height,
            diversifier_index: Some(index),
            memo_text: Some(memo),
            has_transparent_input: false,
        }
    }

    fn status(f: &NoteFacts<'_>) -> Option<BetStatus> {
        let d = def();
        classify(f, &d, &rules_hash(&d).unwrap(), false).status()
    }

    #[test]
    fn valid_bet_is_pending() {
        let memo = frst1(MARKET_ID);
        let d = def();
        let c = classify(
            &facts(50_000_000, 150, 1, &memo),
            &d,
            &rules_hash(&d).unwrap(),
            false,
        );
        assert!(matches!(
            c,
            Classification::Note {
                status: BetStatus::Pending,
                outcome_index: Some(1),
                payload: Some(_)
            }
        ));
    }

    #[test]
    fn statuses_follow_section_19_5_order() {
        let memo = frst1(MARKET_ID);
        assert_eq!(
            status(&facts(5_000, 150, 0, &memo)),
            Some(BetStatus::UneconomicDust)
        );
        assert_eq!(
            status(&facts(50_000_000, 150, 5, &memo)),
            Some(BetStatus::UnknownOutcome)
        );
        assert_eq!(
            status(&facts(50_000_000, 201, 0, &memo)),
            Some(BetStatus::Late)
        );
        assert_eq!(
            status(&facts(500_000, 150, 0, &memo)),
            Some(BetStatus::IgnoredDust)
        );
        assert_eq!(
            status(&facts(50_000_000, 150, 0, "hello")),
            Some(BetStatus::InvalidMemo)
        );
        assert_eq!(
            status(&facts(50_000_000, 150, 0, &frst1([0xb2; 16]))),
            Some(BetStatus::InvalidMemo)
        );
        let mut t = facts(50_000_000, 150, 0, &memo);
        t.has_transparent_input = true;
        assert_eq!(status(&t), Some(BetStatus::RejectedTransparent));
        // Transparent but uneconomic: dust wins (refund would not be economic).
        t.amount_zat = 4_000;
        assert_eq!(status(&t), Some(BetStatus::UneconomicDust));
        // A late bet from a transparent input is still REJECTED_TRANSPARENT (rule 4 before 5).
        let mut lt = facts(50_000_000, 250, 0, &memo);
        lt.has_transparent_input = true;
        assert_eq!(status(&lt), Some(BetStatus::RejectedTransparent));
    }

    #[test]
    fn bet_before_announcement_counts_like_open() {
        // Height between birthday and close, announcement not seen yet (ADR-0009).
        let memo = frst1(MARKET_ID);
        let d = def();
        let c = classify(
            &facts(2_000_000, 101, 0, &memo),
            &d,
            &rules_hash(&d).unwrap(),
            false,
        );
        assert_eq!(c.status(), Some(BetStatus::Pending));
    }

    #[test]
    fn announcement_rules() {
        let d = def();
        let h = rules_hash(&d).unwrap();
        let good = encode_frstm1(&h);
        let forged = encode_frstm1(&[9; 32]);
        assert_eq!(
            classify(&facts(10_000, 100, 0, &good), &d, &h, false),
            Classification::Announcement
        );
        // Second valid announcement, wrong outcome, forged hash → INVALID_MEMO.
        assert_eq!(
            classify(&facts(10_000, 101, 0, &good), &d, &h, true).status(),
            Some(BetStatus::InvalidMemo)
        );
        assert_eq!(
            classify(&facts(10_000, 100, 1, &good), &d, &h, false).status(),
            Some(BetStatus::InvalidMemo)
        );
        assert_eq!(
            classify(&facts(10_000, 100, 0, &forged), &d, &h, false).status(),
            Some(BetStatus::InvalidMemo)
        );
        // Announcement at or below the sweep threshold is uneconomic dust.
        assert_eq!(
            classify(&facts(5_000, 100, 0, &good), &d, &h, false).status(),
            Some(BetStatus::UneconomicDust)
        );
    }

    #[test]
    fn confirmation_count() {
        assert_eq!(confirmations(10, 9), 0);
        assert_eq!(confirmations(10, 10), 1);
        assert_eq!(confirmations(10, 12), 3);
    }
}

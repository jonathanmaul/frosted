//! Global constants (docs/PROJECT_SPEC_FROSTED.md §23.4) and startup invariants (NFR-S5).
//!
//! Values that affect bet validation are copied into each `market_definition` when a market is
//! created, so verifiers read them from the definition rather than from this module.

use thiserror::Error;

pub const PARAMS_VERSION: u32 = 1;
pub const MIN_BET_ZAT: u64 = 1_000_000;
pub const MIN_CONFIRMATIONS: u32 = 3;
pub const REORG_DEPTH: u32 = 10;
pub const MARGINAL_FEE_ZAT: u64 = 5_000;
pub const REFUND_FEE_ZAT: u64 = 5_000;
pub const SWEEP_THRESHOLD_ZAT: u64 = 5_000;
pub const DEFAULT_FEE_BPS: u32 = 100;
pub const MAX_ACTIONS_PER_TX: u32 = 50;
pub const MIN_ANNOUNCE_ZAT: u64 = 10_000;
pub const ANNOUNCE_TIMEOUT_BLOCKS: u32 = 100;
pub const DISPUTE_WINDOW_BLOCKS: u32 = 144;
pub const VOID_AFTER_BLOCKS: u32 = 5_760;
pub const MAX_MAINNET_DEMO_ZAT: u64 = 50_000_000;
pub const INGEST_MAX_SKEW_SECS: u64 = 60;
pub const SIGNING_SESSION_TIMEOUT_SECS: u64 = 600;
pub const CHAIN_POLL_INTERVAL_SECS: u64 = 2;
pub const ESCROW_SYNC_INTERVAL_SECS: u64 = 30;
/// ZIP 317 grace actions: every transaction pays for at least this many logical actions.
pub const GRACE_ACTIONS: u32 = 2;

/// `ceil(MARGINAL_FEE_ZAT × 10_000 / MIN_BET_ZAT)`: the lowest `fee_bps` for which every counted
/// bet pays for its own spend (ADR-0004).
pub const MIN_FEE_BPS: u32 = min_fee_bps(MARGINAL_FEE_ZAT, MIN_BET_ZAT);

pub const fn min_fee_bps(marginal_fee_zat: u64, min_bet_zat: u64) -> u32 {
    (marginal_fee_zat * 10_000).div_ceil(min_bet_zat) as u32
}

/// Returns a 16-byte BLAKE2b personalization; fails compilation when used in a `const` with a
/// string of any other length.
pub const fn personalization(s: &str) -> [u8; 16] {
    let bytes = s.as_bytes();
    assert!(
        bytes.len() == 16,
        "BLAKE2b personalization must be exactly 16 bytes"
    );
    let mut out = [0u8; 16];
    let mut i = 0;
    while i < 16 {
        out[i] = bytes[i];
        i += 1;
    }
    out
}

pub const PERSONAL_BET_FP: [u8; 16] = personalization("FrostedBetFP0001");
pub const PERSONAL_NOTE_REF: [u8; 16] = personalization("FrostedNoteRef01");
pub const PERSONAL_RULES: [u8; 16] = personalization("FrostedRules0001");
pub const PERSONAL_PLAN: [u8; 16] = personalization("FrostedPlan00001");
pub const PERSONAL_RCVR: [u8; 16] = personalization("FrostedRcvr00001");
pub const PERSONAL_PERSONA: [u8; 16] = personalization("FrostedPersona01");
pub const PERSONAL_THAW_TAG: [u8; 16] = personalization("FrostedThawTag02");

/// The parameter set checked by [`assert_invariants`]. Defaults are the constants above; tests may
/// override values to emulate environment overrides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Params {
    pub min_bet_zat: u64,
    pub min_announce_zat: u64,
    pub sweep_threshold_zat: u64,
    pub refund_fee_zat: u64,
    pub marginal_fee_zat: u64,
    pub default_fee_bps: u32,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            min_bet_zat: MIN_BET_ZAT,
            min_announce_zat: MIN_ANNOUNCE_ZAT,
            sweep_threshold_zat: SWEEP_THRESHOLD_ZAT,
            refund_fee_zat: REFUND_FEE_ZAT,
            marginal_fee_zat: MARGINAL_FEE_ZAT,
            default_fee_bps: DEFAULT_FEE_BPS,
        }
    }
}

impl Params {
    pub fn min_fee_bps(&self) -> u32 {
        min_fee_bps(self.marginal_fee_zat, self.min_bet_zat)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("parameter invariant violated: {0}")]
pub struct InvariantViolation(pub &'static str);

/// Checks the startup invariants of §23.4. Binaries call this before doing anything else.
pub fn assert_invariants(p: &Params) -> Result<(), InvariantViolation> {
    if p.min_announce_zat <= p.sweep_threshold_zat {
        return Err(InvariantViolation("MIN_ANNOUNCE_ZAT > SWEEP_THRESHOLD_ZAT"));
    }
    if p.min_bet_zat <= p.sweep_threshold_zat {
        return Err(InvariantViolation("MIN_BET_ZAT > SWEEP_THRESHOLD_ZAT"));
    }
    if p.default_fee_bps < p.min_fee_bps() {
        return Err(InvariantViolation("DEFAULT_FEE_BPS >= MIN_FEE_BPS"));
    }
    if p.refund_fee_zat < p.marginal_fee_zat {
        return Err(InvariantViolation("REFUND_FEE_ZAT >= MARGINAL_FEE_ZAT"));
    }
    if p.sweep_threshold_zat < p.marginal_fee_zat {
        return Err(InvariantViolation(
            "SWEEP_THRESHOLD_ZAT >= MARGINAL_FEE_ZAT",
        ));
    }
    Ok(())
}

// Compile-time guarantees for the shipped constants.
const _: () = assert!(MIN_ANNOUNCE_ZAT > SWEEP_THRESHOLD_ZAT);
const _: () = assert!(MIN_BET_ZAT > SWEEP_THRESHOLD_ZAT);
const _: () = assert!(DEFAULT_FEE_BPS >= MIN_FEE_BPS);
const _: () = assert!(REFUND_FEE_ZAT >= MARGINAL_FEE_ZAT);
const _: () = assert!(SWEEP_THRESHOLD_ZAT >= MARGINAL_FEE_ZAT);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_values() {
        assert_eq!(PARAMS_VERSION, 1);
        assert_eq!(MIN_BET_ZAT, 1_000_000);
        assert_eq!(MIN_CONFIRMATIONS, 3);
        assert_eq!(REORG_DEPTH, 10);
        assert_eq!(MARGINAL_FEE_ZAT, 5_000);
        assert_eq!(REFUND_FEE_ZAT, 5_000);
        assert_eq!(SWEEP_THRESHOLD_ZAT, 5_000);
        assert_eq!(DEFAULT_FEE_BPS, 100);
        assert_eq!(MAX_ACTIONS_PER_TX, 50);
        assert_eq!(MIN_ANNOUNCE_ZAT, 10_000);
        assert_eq!(ANNOUNCE_TIMEOUT_BLOCKS, 100);
        assert_eq!(DISPUTE_WINDOW_BLOCKS, 144);
        assert_eq!(VOID_AFTER_BLOCKS, 5_760);
        assert_eq!(MAX_MAINNET_DEMO_ZAT, 50_000_000);
        assert_eq!(INGEST_MAX_SKEW_SECS, 60);
        assert_eq!(SIGNING_SESSION_TIMEOUT_SECS, 600);
        assert_eq!(CHAIN_POLL_INTERVAL_SECS, 2);
        assert_eq!(ESCROW_SYNC_INTERVAL_SECS, 30);
        assert_eq!(MIN_FEE_BPS, 50);
    }

    #[test]
    fn default_parameters_pass() {
        assert_eq!(assert_invariants(&Params::default()), Ok(()));
    }

    #[test]
    fn overridden_min_bet_breaks_invariant() {
        let p = Params {
            min_bet_zat: 5_000,
            ..Params::default()
        };
        assert_eq!(
            assert_invariants(&p),
            Err(InvariantViolation("MIN_BET_ZAT > SWEEP_THRESHOLD_ZAT"))
        );
    }

    #[test]
    fn overridden_announce_breaks_invariant() {
        let p = Params {
            min_announce_zat: 5_000,
            ..Params::default()
        };
        assert_eq!(
            assert_invariants(&p),
            Err(InvariantViolation("MIN_ANNOUNCE_ZAT > SWEEP_THRESHOLD_ZAT"))
        );
    }

    #[test]
    fn personalizations_are_16_bytes() {
        for p in [
            PERSONAL_BET_FP,
            PERSONAL_NOTE_REF,
            PERSONAL_RULES,
            PERSONAL_PLAN,
            PERSONAL_RCVR,
            PERSONAL_PERSONA,
            PERSONAL_THAW_TAG,
        ] {
            assert_eq!(p.len(), 16);
        }
        assert_eq!(&PERSONAL_THAW_TAG, b"FrostedThawTag02");
    }
}

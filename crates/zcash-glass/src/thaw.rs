//! Persona commitment and Thaw Tag v2 (docs/PROJECT_SPEC_FROSTED.md §19.3, ADR-0002).

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
}

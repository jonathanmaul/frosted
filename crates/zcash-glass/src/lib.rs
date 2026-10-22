//! Shared Frosted library: params, memo codec, Sealed Envelope, Thaw Tag, hashing, odds and payout math.
//!
//! Every Frosted component (indexer, API, coordinator, verifier, WASM) uses this crate so that
//! memos, hashes, odds and payouts are byte-identical (docs/PROJECT_SPEC_FROSTED.md §18, §19).
//! The default build has no dependency on `orchard`, `zcash_*` or `zip321`; Orchard receivers are
//! opaque 43-byte values here. Chain scanning lives behind the `zcash` feature (`scan`).

#[cfg(feature = "payout-address")]
pub mod address;
pub mod envelope;
pub mod hash;
pub mod memo;
pub mod odds;
pub mod params;
pub mod payout;
#[cfg(feature = "zcash")]
pub mod scan;
pub mod status;
pub mod thaw;
pub mod validate;

#[cfg(test)]
pub(crate) mod test_vectors;

/// Size of a raw Orchard receiver (diversifier ‖ pk_d).
pub const ORCHARD_RECEIVER_LEN: usize = 43;

/// A raw Orchard receiver, treated as opaque bytes by this crate.
pub type OrchardReceiver = [u8; ORCHARD_RECEIVER_LEN];

/// BLAKE2b-256 with a 16-byte personalization and an optional key.
pub(crate) fn blake2b_256(personal: &[u8; 16], key: Option<&[u8]>, data: &[&[u8]]) -> [u8; 32] {
    let mut params = blake2b_simd::Params::new();
    params.hash_length(32).personal(personal);
    if let Some(key) = key {
        params.key(key);
    }
    let mut state = params.to_state();
    for part in data {
        state.update(part);
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(state.finalize().as_bytes());
    out
}

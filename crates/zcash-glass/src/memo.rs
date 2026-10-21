//! Frosted memo formats (docs/PROJECT_SPEC_FROSTED.md §19.3): FRST1 (bet), FRSTM1 (market
//! announcement), FRSTP1 (Persona registration), plus `bet_fingerprint`.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use thiserror::Error;

use crate::blake2b_256;
use crate::params::PERSONAL_BET_FP;

pub const FRST1_PREFIX: &str = "FRST1:";
pub const FRSTM1_PREFIX: &str = "FRSTM1:";
pub const FRSTP1_PREFIX: &str = "FRSTP1:";
pub const FRST1_VERSION: u8 = 0x01;
pub const FRSTP1_VERSION: u8 = 0x01;
pub const FRST1_PAYLOAD_LEN: usize = 173;
pub const FRST1_MEMO_LEN: usize = 237;
pub const FRSTM1_MEMO_LEN: usize = 71;
pub const MAX_MEMO_LEN: usize = 512;
/// Bytes `payload[0..50]` (version, flags, market_id, thaw_tag) are the envelope AEAD's AAD.
pub const FRST1_HEADER_LEN: usize = 50;
pub const ENV_CIPHERTEXT_LEN: usize = 67;
pub const FLAG_HAS_THAW_TAG: u8 = 0b0000_0001;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MemoError {
    /// §22: `MEMO_TOO_LARGE`.
    #[error("MEMO_TOO_LARGE: memo is {0} bytes, the limit is 512")]
    MemoTooLarge(usize),
    #[error("memo does not start with the expected prefix")]
    WrongPrefix,
    #[error("memo payload is not valid base64url without padding")]
    InvalidBase64,
    #[error("memo payload is not valid lowercase hex")]
    InvalidHex,
    #[error("payload has {0} bytes, expected {1}")]
    WrongLength(usize, usize),
    #[error("unknown memo version {0:#04x}")]
    UnknownVersion(u8),
    #[error("unknown flag bits {0:#04x}")]
    UnknownFlags(u8),
    #[error("thaw_tag must be 32 zero bytes when has_thaw_tag is not set")]
    UntaggedWithTag,
}

/// The fields of `payload[0..50]`, which the envelope AEAD authenticates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frst1Header {
    pub version: u8,
    pub flags: u8,
    pub market_id: [u8; 16],
    pub thaw_tag: [u8; 32],
}

impl Frst1Header {
    /// A header for a bet; `thaw_tag = None` produces an untagged bet (flag cleared, zero tag).
    pub fn new(market_id: [u8; 16], thaw_tag: Option<[u8; 32]>) -> Self {
        Self {
            version: FRST1_VERSION,
            flags: if thaw_tag.is_some() {
                FLAG_HAS_THAW_TAG
            } else {
                0
            },
            market_id,
            thaw_tag: thaw_tag.unwrap_or([0u8; 32]),
        }
    }

    pub fn has_thaw_tag(&self) -> bool {
        self.flags & FLAG_HAS_THAW_TAG != 0
    }

    pub fn to_bytes(&self) -> [u8; FRST1_HEADER_LEN] {
        let mut out = [0u8; FRST1_HEADER_LEN];
        out[0] = self.version;
        out[1] = self.flags;
        out[2..18].copy_from_slice(&self.market_id);
        out[18..50].copy_from_slice(&self.thaw_tag);
        out
    }
}

/// The 173-byte FRST1 payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frst1Payload {
    pub header: Frst1Header,
    pub env_epk: [u8; 32],
    pub env_nonce: [u8; 24],
    pub env_ciphertext: [u8; ENV_CIPHERTEXT_LEN],
}

impl Frst1Payload {
    pub fn to_bytes(&self) -> [u8; FRST1_PAYLOAD_LEN] {
        let mut out = [0u8; FRST1_PAYLOAD_LEN];
        out[..50].copy_from_slice(&self.header.to_bytes());
        out[50..82].copy_from_slice(&self.env_epk);
        out[82..106].copy_from_slice(&self.env_nonce);
        out[106..173].copy_from_slice(&self.env_ciphertext);
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, MemoError> {
        if bytes.len() != FRST1_PAYLOAD_LEN {
            return Err(MemoError::WrongLength(bytes.len(), FRST1_PAYLOAD_LEN));
        }
        let version = bytes[0];
        if version != FRST1_VERSION {
            return Err(MemoError::UnknownVersion(version));
        }
        let flags = bytes[1];
        if flags & !FLAG_HAS_THAW_TAG != 0 {
            return Err(MemoError::UnknownFlags(flags));
        }
        let header = Frst1Header {
            version,
            flags,
            market_id: bytes[2..18].try_into().expect("16 bytes"),
            thaw_tag: bytes[18..50].try_into().expect("32 bytes"),
        };
        if !header.has_thaw_tag() && header.thaw_tag != [0u8; 32] {
            return Err(MemoError::UntaggedWithTag);
        }
        Ok(Self {
            header,
            env_epk: bytes[50..82].try_into().expect("32 bytes"),
            env_nonce: bytes[82..106].try_into().expect("24 bytes"),
            env_ciphertext: bytes[106..173].try_into().expect("67 bytes"),
        })
    }
}

fn guard_len(memo: String) -> Result<String, MemoError> {
    if memo.len() > MAX_MEMO_LEN {
        return Err(MemoError::MemoTooLarge(memo.len()));
    }
    Ok(memo)
}

/// Encodes `"FRST1:" + base64url_nopad(payload)` (237 bytes).
pub fn encode_frst1(payload: &Frst1Payload) -> Result<String, MemoError> {
    encode_frst1_bytes(&payload.to_bytes())
}

fn encode_frst1_bytes(payload: &[u8]) -> Result<String, MemoError> {
    guard_len(format!("{FRST1_PREFIX}{}", URL_SAFE_NO_PAD.encode(payload)))
}

/// Decodes an FRST1 memo. Any error is classified by the indexer as `INVALID_MEMO` (US-07).
pub fn decode_frst1(memo: &str) -> Result<Frst1Payload, MemoError> {
    let body = memo
        .strip_prefix(FRST1_PREFIX)
        .ok_or(MemoError::WrongPrefix)?;
    let bytes = URL_SAFE_NO_PAD
        .decode(body)
        .map_err(|_| MemoError::InvalidBase64)?;
    Frst1Payload::from_bytes(&bytes)
}

/// Encodes `"FRSTM1:" + lowercase hex(rules_hash)` (71 bytes).
pub fn encode_frstm1(rules_hash: &[u8; 32]) -> String {
    format!("{FRSTM1_PREFIX}{}", hex::encode(rules_hash))
}

pub fn decode_frstm1(memo: &str) -> Result<[u8; 32], MemoError> {
    let body = memo
        .strip_prefix(FRSTM1_PREFIX)
        .ok_or(MemoError::WrongPrefix)?;
    if body.len() != 64
        || body
            .bytes()
            .any(|b| !matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(MemoError::InvalidHex);
    }
    let mut out = [0u8; 32];
    hex::decode_to_slice(body, &mut out).map_err(|_| MemoError::InvalidHex)?;
    Ok(out)
}

/// Encodes `"FRSTP1:" + base64url_nopad(0x01 ‖ persona_commitment)`.
pub fn encode_frstp1(persona_commitment: &[u8; 32]) -> String {
    let mut bytes = [0u8; 33];
    bytes[0] = FRSTP1_VERSION;
    bytes[1..].copy_from_slice(persona_commitment);
    format!("{FRSTP1_PREFIX}{}", URL_SAFE_NO_PAD.encode(bytes))
}

pub fn decode_frstp1(memo: &str) -> Result<[u8; 32], MemoError> {
    let body = memo
        .strip_prefix(FRSTP1_PREFIX)
        .ok_or(MemoError::WrongPrefix)?;
    let bytes = URL_SAFE_NO_PAD
        .decode(body)
        .map_err(|_| MemoError::InvalidBase64)?;
    if bytes.len() != 33 {
        return Err(MemoError::WrongLength(bytes.len(), 33));
    }
    if bytes[0] != FRSTP1_VERSION {
        return Err(MemoError::UnknownVersion(bytes[0]));
    }
    Ok(bytes[1..].try_into().expect("32 bytes"))
}

/// `hex(BLAKE2b-256(personal = "FrostedBetFP0001", payload)[0..8])`. Not unique: a copied payload
/// has the same fingerprint (use `note_ref` as the unique key).
pub fn bet_fingerprint(payload: &Frst1Payload) -> String {
    let digest = blake2b_256(&PERSONAL_BET_FP, None, &[&payload.to_bytes()]);
    hex::encode(&digest[..8])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_payload(tagged: bool) -> Frst1Payload {
        Frst1Payload {
            header: Frst1Header::new([7u8; 16], tagged.then_some([9u8; 32])),
            env_epk: [1u8; 32],
            env_nonce: [2u8; 24],
            env_ciphertext: [3u8; ENV_CIPHERTEXT_LEN],
        }
    }

    #[test]
    fn frst1_length_and_round_trip() {
        for tagged in [false, true] {
            let p = sample_payload(tagged);
            let memo = encode_frst1(&p).unwrap();
            assert_eq!(memo.len(), FRST1_MEMO_LEN);
            assert!(memo.starts_with("FRST1:"));
            assert_eq!(decode_frst1(&memo).unwrap(), p);
        }
    }

    #[test]
    fn untagged_bet_has_zero_tag() {
        let p = sample_payload(false);
        assert_eq!(p.header.flags & FLAG_HAS_THAW_TAG, 0);
        assert_eq!(p.header.thaw_tag, [0u8; 32]);
    }

    #[test]
    fn oversized_memo_is_rejected() {
        let big = [0u8; 400];
        assert!(matches!(
            encode_frst1_bytes(&big),
            Err(MemoError::MemoTooLarge(_))
        ));
    }

    #[test]
    fn unparseable_memos_are_rejected() {
        let good = encode_frst1(&sample_payload(true)).unwrap();
        let body = &good[6..];
        assert_eq!(
            decode_frst1(&format!("FRST2:{body}")),
            Err(MemoError::WrongPrefix)
        );
        assert_eq!(decode_frst1("FRST1:@@@@"), Err(MemoError::InvalidBase64));
        assert_eq!(
            decode_frst1(&format!("FRST1:{}", URL_SAFE_NO_PAD.encode([1u8; 10]))),
            Err(MemoError::WrongLength(10, 173))
        );
        let mut bytes = sample_payload(true).to_bytes();
        bytes[0] = 2;
        assert_eq!(
            decode_frst1(&format!("FRST1:{}", URL_SAFE_NO_PAD.encode(bytes))),
            Err(MemoError::UnknownVersion(2))
        );
        let mut bytes = sample_payload(false).to_bytes();
        bytes[20] = 1;
        assert_eq!(
            decode_frst1(&format!("FRST1:{}", URL_SAFE_NO_PAD.encode(bytes))),
            Err(MemoError::UntaggedWithTag)
        );
        // Padded base64 is not accepted.
        assert_eq!(
            decode_frst1(&format!("{good}=")),
            Err(MemoError::InvalidBase64)
        );
    }

    #[test]
    fn frstm1_round_trip() {
        let h = [0xabu8; 32];
        let memo = encode_frstm1(&h);
        assert_eq!(memo.len(), FRSTM1_MEMO_LEN);
        assert_eq!(decode_frstm1(&memo).unwrap(), h);
        assert_eq!(
            decode_frstm1(&memo.replace("FRSTM1:", "frstm1:")),
            Err(MemoError::WrongPrefix)
        );
        assert_eq!(
            decode_frstm1(&format!("FRSTM1:{}", "AB".repeat(32))),
            Err(MemoError::InvalidHex)
        );
    }

    #[test]
    fn frstp1_round_trip() {
        let c = [0x42u8; 32];
        assert_eq!(decode_frstp1(&encode_frstp1(&c)).unwrap(), c);
    }

    #[test]
    fn fingerprint_is_16_hex_and_equal_for_copies() {
        let p = sample_payload(true);
        let fp = bet_fingerprint(&p);
        assert_eq!(fp.len(), 16);
        assert_eq!(fp, bet_fingerprint(&p.clone()));
        assert_ne!(fp, bet_fingerprint(&sample_payload(false)));
    }
}

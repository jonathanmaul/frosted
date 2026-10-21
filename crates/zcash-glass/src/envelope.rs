//! Sealed Envelope (docs/PROJECT_SPEC_FROSTED.md §19.3) and Envelope Proof (ADR-0006).
//!
//! AEAD key = HKDF-SHA256(ikm = X25519(env_esk, envelope_pk), salt = market_id,
//! info = "Frosted envelope v1"); cipher XChaCha20-Poly1305; plaintext = receiver ‖ bet_salt;
//! AAD = payload[0..50].

use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, Key, KeyInit, Payload},
};
use hkdf::Hkdf;
use sha2::Sha256;
use thiserror::Error;
use x25519_dalek::{PublicKey, StaticSecret};

use crate::OrchardReceiver;
use crate::memo::{ENV_CIPHERTEXT_LEN, Frst1Header, Frst1Payload};

pub const HKDF_INFO: &[u8] = b"Frosted envelope v1";
const PLAINTEXT_LEN: usize = crate::ORCHARD_RECEIVER_LEN + 8;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EnvelopeError {
    /// The X25519 shared secret is all zeros (low-order public key).
    #[error("non-contributory key exchange")]
    NonContributory,
    /// Decryption failed; for a `COUNTED` bet the coordinator classifies it as `UNCLAIMABLE`.
    #[error("envelope cannot be opened")]
    CannotOpen,
}

/// Randomness for one seal, supplied by the caller (OS RNG natively, `getrandom` in WASM,
/// fixed bytes in test vectors).
#[derive(Clone, Copy)]
pub struct SealRandomness {
    pub env_esk: [u8; 32],
    pub env_nonce: [u8; 24],
}

/// Result of [`seal`]. `env_esk` stays in the bettor's Bet Receipt and is never sent anywhere
/// unless the bettor publishes an Envelope Proof.
#[derive(Clone, Copy)]
pub struct Sealed {
    pub env_epk: [u8; 32],
    pub env_nonce: [u8; 24],
    pub env_ciphertext: [u8; ENV_CIPHERTEXT_LEN],
    pub env_esk: [u8; 32],
}

impl Sealed {
    pub fn into_payload(self, header: Frst1Header) -> Frst1Payload {
        Frst1Payload {
            header,
            env_epk: self.env_epk,
            env_nonce: self.env_nonce,
            env_ciphertext: self.env_ciphertext,
        }
    }
}

fn aead_key(shared: &[u8; 32], market_id: &[u8; 16]) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(Some(market_id), shared);
    let mut okm = [0u8; 32];
    hk.expand(HKDF_INFO, &mut okm)
        .expect("32 bytes is a valid HKDF-SHA256 output length");
    okm
}

fn shared_secret(secret: &[u8; 32], public: &[u8; 32]) -> Result<[u8; 32], EnvelopeError> {
    let shared = StaticSecret::from(*secret).diffie_hellman(&PublicKey::from(*public));
    if !shared.was_contributory() {
        return Err(EnvelopeError::NonContributory);
    }
    Ok(shared.to_bytes())
}

fn cipher(key: &[u8; 32]) -> XChaCha20Poly1305 {
    XChaCha20Poly1305::new(&Key::<XChaCha20Poly1305>::from(*key))
}

/// Seals `receiver ‖ bet_salt` to `envelope_pk` (from `market_definition`), binding the header.
pub fn seal(
    envelope_pk: &[u8; 32],
    header: &Frst1Header,
    receiver: &OrchardReceiver,
    bet_salt: &[u8; 8],
    randomness: SealRandomness,
) -> Result<Sealed, EnvelopeError> {
    let esk = StaticSecret::from(randomness.env_esk);
    let env_epk = PublicKey::from(&esk).to_bytes();
    let shared = shared_secret(&randomness.env_esk, envelope_pk)?;
    let key = aead_key(&shared, &header.market_id);

    let mut plaintext = [0u8; PLAINTEXT_LEN];
    plaintext[..crate::ORCHARD_RECEIVER_LEN].copy_from_slice(receiver);
    plaintext[crate::ORCHARD_RECEIVER_LEN..].copy_from_slice(bet_salt);

    let aad = header.to_bytes();
    let ciphertext = cipher(&key)
        .encrypt(
            &XNonce::from(randomness.env_nonce),
            Payload {
                msg: &plaintext,
                aad: &aad,
            },
        )
        .expect("XChaCha20-Poly1305 encryption of 51 bytes cannot fail");
    Ok(Sealed {
        env_epk,
        env_nonce: randomness.env_nonce,
        env_ciphertext: ciphertext
            .try_into()
            .expect("51-byte plaintext + 16-byte tag"),
        env_esk: randomness.env_esk,
    })
}

fn decrypt(
    key: &[u8; 32],
    payload: &Frst1Payload,
) -> Result<(OrchardReceiver, [u8; 8]), EnvelopeError> {
    let aad = payload.header.to_bytes();
    let plaintext = cipher(key)
        .decrypt(
            &XNonce::from(payload.env_nonce),
            Payload {
                msg: &payload.env_ciphertext,
                aad: &aad,
            },
        )
        .map_err(|_| EnvelopeError::CannotOpen)?;
    let receiver = plaintext[..crate::ORCHARD_RECEIVER_LEN]
        .try_into()
        .map_err(|_| EnvelopeError::CannotOpen)?;
    let salt = plaintext[crate::ORCHARD_RECEIVER_LEN..]
        .try_into()
        .map_err(|_| EnvelopeError::CannotOpen)?;
    Ok((receiver, salt))
}

/// Opens an envelope with `ENVELOPE_SK` (Payout Coordinator only).
pub fn open(
    envelope_sk: &[u8; 32],
    payload: &Frst1Payload,
) -> Result<(OrchardReceiver, [u8; 8]), EnvelopeError> {
    let shared =
        shared_secret(envelope_sk, &payload.env_epk).map_err(|_| EnvelopeError::CannotOpen)?;
    decrypt(&aead_key(&shared, &payload.header.market_id), payload)
}

/// What a bettor publishes to disprove an `UNCLAIMABLE` label.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnvelopeProof {
    pub env_esk: [u8; 32],
    pub receiver: OrchardReceiver,
    pub bet_salt: [u8; 8],
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProofFailure {
    #[error("env_esk does not match env_epk in the memo")]
    EskMismatch,
    #[error("envelope does not decrypt with this env_esk and envelope_pk")]
    DecryptFailed,
    #[error("decrypted plaintext differs from the claimed receiver and bet_salt")]
    PlaintextMismatch,
}

/// PASS (`Ok`) only if `env_esk` matches `env_epk`, the AEAD key derived with `envelope_pk`
/// decrypts the ciphertext under the same AAD, and the plaintext equals `receiver ‖ bet_salt`.
pub fn verify_proof(
    proof: &EnvelopeProof,
    payload: &Frst1Payload,
    envelope_pk: &[u8; 32],
) -> Result<(), ProofFailure> {
    let epk = PublicKey::from(&StaticSecret::from(proof.env_esk)).to_bytes();
    if epk != payload.env_epk {
        return Err(ProofFailure::EskMismatch);
    }
    let shared =
        shared_secret(&proof.env_esk, envelope_pk).map_err(|_| ProofFailure::DecryptFailed)?;
    let (receiver, salt) = decrypt(&aead_key(&shared, &payload.header.market_id), payload)
        .map_err(|_| ProofFailure::DecryptFailed)?;
    if receiver != proof.receiver || salt != proof.bet_salt {
        return Err(ProofFailure::PlaintextMismatch);
    }
    Ok(())
}

/// The envelope public key for a secret key (coordinator setup and tests).
pub fn envelope_public_key(envelope_sk: &[u8; 32]) -> [u8; 32] {
    PublicKey::from(&StaticSecret::from(*envelope_sk)).to_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SK: [u8; 32] = [0x11; 32];
    const RND: SealRandomness = SealRandomness {
        env_esk: [0x22; 32],
        env_nonce: [0x33; 24],
    };
    const RECEIVER: OrchardReceiver = [0x44; 43];
    const SALT: [u8; 8] = [0x55; 8];

    fn header() -> Frst1Header {
        Frst1Header::new([0x66; 16], Some([0x77; 32]))
    }

    fn sealed_payload() -> (Frst1Payload, Sealed) {
        let s = seal(&envelope_public_key(&SK), &header(), &RECEIVER, &SALT, RND).unwrap();
        (s.into_payload(header()), s)
    }

    #[test]
    fn seal_sizes_and_epk() {
        let (_, s) = sealed_payload();
        assert_eq!(s.env_ciphertext.len(), 67);
        assert_eq!(s.env_epk, envelope_public_key(&s.env_esk));
    }

    #[test]
    fn deterministic_with_fixed_randomness() {
        let (a, _) = sealed_payload();
        let (b, _) = sealed_payload();
        assert_eq!(a, b);
    }

    #[test]
    fn round_trip() {
        let (p, _) = sealed_payload();
        assert_eq!(open(&SK, &p).unwrap(), (RECEIVER, SALT));
    }

    #[test]
    fn tampered_header_fails() {
        let (mut p, _) = sealed_payload();
        p.header.thaw_tag[0] ^= 1;
        assert_eq!(open(&SK, &p), Err(EnvelopeError::CannotOpen));
        let (mut p, _) = sealed_payload();
        p.header.market_id[0] ^= 1;
        assert_eq!(open(&SK, &p), Err(EnvelopeError::CannotOpen));
    }

    #[test]
    fn wrong_key_fails() {
        let (p, _) = sealed_payload();
        assert_eq!(open(&[0x99; 32], &p), Err(EnvelopeError::CannotOpen));
    }

    #[test]
    fn honest_proof_passes() {
        let (p, s) = sealed_payload();
        let proof = EnvelopeProof {
            env_esk: s.env_esk,
            receiver: RECEIVER,
            bet_salt: SALT,
        };
        assert_eq!(verify_proof(&proof, &p, &envelope_public_key(&SK)), Ok(()));
    }

    #[test]
    fn forged_proofs_fail() {
        let (p, s) = sealed_payload();
        let pk = envelope_public_key(&SK);
        let good = EnvelopeProof {
            env_esk: s.env_esk,
            receiver: RECEIVER,
            bet_salt: SALT,
        };
        let wrong_esk = EnvelopeProof {
            env_esk: [0x01; 32],
            ..good
        };
        assert_eq!(
            verify_proof(&wrong_esk, &p, &pk),
            Err(ProofFailure::EskMismatch)
        );
        let wrong_receiver = EnvelopeProof {
            receiver: [0x45; 43],
            ..good
        };
        assert_eq!(
            verify_proof(&wrong_receiver, &p, &pk),
            Err(ProofFailure::PlaintextMismatch)
        );
        let wrong_salt = EnvelopeProof {
            bet_salt: [0; 8],
            ..good
        };
        assert_eq!(
            verify_proof(&wrong_salt, &p, &pk),
            Err(ProofFailure::PlaintextMismatch)
        );
        assert_eq!(
            verify_proof(&good, &p, &envelope_public_key(&[0x12; 32])),
            Err(ProofFailure::DecryptFailed)
        );
    }
}

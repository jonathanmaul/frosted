pub mod committee;
pub mod config;
pub mod keys;
pub mod market;
pub mod push;
pub mod secrets;
pub mod service;
pub mod settlement;
pub mod store;
pub mod wallet;
pub fn coordinator_pubkey(key: &[u8; 32]) -> String {
    hex::encode(
        ed25519_dalek::SigningKey::from_bytes(key)
            .verifying_key()
            .to_bytes(),
    )
}

pub mod frost_sign;

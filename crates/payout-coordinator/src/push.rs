use anyhow::{Result, bail};
use base64::{Engine, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signer, SigningKey};
use reqwest::{Client, Url};
use sha2::{Digest, Sha256};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub fn signature(key: &SigningKey, path: &str, timestamp: u64, body: &[u8]) -> String {
    let message = format!(
        "POST\n{path}\n{timestamp}\n{}",
        hex::encode(Sha256::digest(body))
    );
    STANDARD.encode(key.sign(message.as_bytes()).to_bytes())
}
pub async fn market(client: &Client, key: &SigningKey, url: &Url, body: &[u8]) -> Result<()> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let response = client
        .post(url.clone())
        .header("content-type", "application/json")
        .header("X-Frosted-Timestamp", timestamp)
        .header(
            "X-Frosted-Signature",
            signature(key, url.path(), timestamp, body),
        )
        .body(body.to_vec())
        .send()
        .await?;
    if !response.status().is_success() {
        bail!("API ingest returned HTTP {}", response.status());
    }
    Ok(())
}
pub fn client() -> Result<Client> {
    Ok(Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()?)
}
#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signature, Verifier};
    #[test]
    fn signed_exact_body() {
        let key = SigningKey::from_bytes(&[5; 32]);
        let body = b"{\"a\":1}";
        let sig = Signature::from_slice(
            &STANDARD
                .decode(signature(&key, "/v1/ingest/markets", 42, body))
                .unwrap(),
        )
        .unwrap();
        let m = format!(
            "POST\n/v1/ingest/markets\n42\n{}",
            hex::encode(Sha256::digest(body))
        );
        key.verifying_key().verify(m.as_bytes(), &sig).unwrap();
        assert!(key.verifying_key().verify(b"wrong", &sig).is_err());
    }
}

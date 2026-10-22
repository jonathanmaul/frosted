use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    body::Bytes,
    http::{HeaderMap, Method, StatusCode},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use ed25519_dalek::Signature;
use sha2::{Digest, Sha256};

use crate::{
    Config,
    error::{ApiError, Result},
};

pub(crate) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}
pub(crate) fn verify(
    config: &Config,
    method: &Method,
    path: &str,
    headers: &HeaderMap,
    body: &Bytes,
) -> Result<()> {
    let timestamp = headers
        .get("x-frosted-timestamp")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(ApiError::unauthorized)?;
    let unix: u64 = timestamp.parse().map_err(|_| ApiError::unauthorized())?;
    let encoded = headers
        .get("x-frosted-signature")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(ApiError::unauthorized)?;
    let signature = STANDARD
        .decode(encoded)
        .map_err(|_| ApiError::unauthorized())?;
    let signature = Signature::from_slice(&signature).map_err(|_| ApiError::unauthorized())?;
    let message = format!(
        "{method}\n{path}\n{timestamp}\n{}",
        hex::encode(Sha256::digest(body))
    );
    config
        .coordinator_pubkey
        .verify_strict(message.as_bytes(), &signature)
        .map_err(|_| ApiError::unauthorized())?;
    if now().abs_diff(unix) > config.ingest_max_skew_secs {
        return Err(ApiError(
            StatusCode::UNAUTHORIZED,
            "STALE_REQUEST",
            "Coordinator timestamp outside allowed skew".into(),
        ));
    }
    Ok(())
}

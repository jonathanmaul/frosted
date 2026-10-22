use std::{env, net::SocketAddr, time::Duration};

use anyhow::{Context, Result, ensure};
use axum::http::HeaderValue;
use ed25519_dalek::VerifyingKey;

#[derive(Clone)]
pub struct Config {
    pub bind: SocketAddr,
    pub public_origin: HeaderValue,
    pub public_db_path: String,
    pub coordinator_pubkey: VerifyingKey,
    pub ingest_max_skew_secs: u64,
    pub zebra_rpc_url: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        ensure!(
            env::var("LOG_IP").unwrap_or_else(|_| "false".into()) == "false",
            "LOG_IP must be false"
        );
        let key = hex::decode(
            env::var("COORDINATOR_PUBKEY")
                .context("COORDINATOR_PUBKEY is required (32-byte hex)")?,
        )?;
        let key: [u8; 32] = key
            .try_into()
            .map_err(|_| anyhow::anyhow!("COORDINATOR_PUBKEY must be 32 bytes"))?;
        let coordinator_pubkey = VerifyingKey::from_bytes(&key)?;
        ensure!(
            !coordinator_pubkey.is_weak(),
            "COORDINATOR_PUBKEY must not be weak"
        );
        let public_origin: HeaderValue = env::var("API_PUBLIC_ORIGIN")
            .unwrap_or_else(|_| "http://localhost:3000".into())
            .parse()?;
        ensure!(
            public_origin != "*" && public_origin != "null",
            "API_PUBLIC_ORIGIN must be an explicit web origin"
        );
        Ok(Self {
            bind: env::var("API_BIND")
                .unwrap_or_else(|_| "0.0.0.0:8080".into())
                .parse()?,
            public_origin,
            public_db_path: env::var("PUBLIC_DB_PATH")
                .unwrap_or_else(|_| "./data/public.db".into()),
            coordinator_pubkey,
            ingest_max_skew_secs: env::var("INGEST_MAX_SKEW_SECS")
                .unwrap_or_else(|_| "60".into())
                .parse()?,
            zebra_rpc_url: env::var("ZEBRA_RPC_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:18232".into()),
        })
    }

    pub(crate) fn rpc_client(&self) -> Result<reqwest::Client> {
        Ok(reqwest::Client::builder()
            .timeout(Duration::from_secs(3))
            .build()?)
    }
}

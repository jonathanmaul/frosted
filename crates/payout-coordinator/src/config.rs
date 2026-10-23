use crate::secrets::Secrets;
use anyhow::{Context, Result, bail};
use reqwest::Url;
use std::{env, net::SocketAddr, path::PathBuf, time::Duration};
pub use zcash_glass::params::SIGNING_SESSION_TIMEOUT_SECS;

#[derive(Clone)]
pub struct Config {
    pub bind: SocketAddr,
    pub network: String,
    pub db: PathBuf,
    pub indexer_db: PathBuf,
    pub secrets_key: PathBuf,
    pub coordinator_key: PathBuf,
    pub envelope_key: PathBuf,
    pub hot_seed: PathBuf,
    pub treasury: String,
    pub rpc_url: String,
    pub zaino_url: String,
    pub ingest_url: Url,
    pub helper: PathBuf,
    pub min_bet_zat: u64,
    pub min_confirmations: u32,
    pub max_mainnet_demo_zat: u64,
    pub sync_interval: Duration,
}
impl Config {
    pub fn from_env() -> Result<Self> {
        let var = |name: &str, default: &str| env::var(name).unwrap_or_else(|_| default.into());
        let network = var("FROSTED_NETWORK", "regtest");
        if !["regtest", "testnet", "mainnet"].contains(&network.as_str()) {
            bail!("invalid network");
        }
        let bind = local_bind(&var("COORDINATOR_BIND", "127.0.0.1:8090"))?;
        let ingest_url = Url::parse(&var("API_INGEST_URL", "http://localhost:8080/v1/ingest"))?;
        validate_ingest_url(&ingest_url, &network)?;
        let config = Self {
            bind,
            network,
            db: var("COORDINATOR_DB_PATH", "./private/coordinator.db").into(),
            indexer_db: var("COORDINATOR_INDEXER_DB_PATH", "./private/indexer.db").into(),
            secrets_key: var("SECRETS_KEY_FILE", "./private/secrets.key").into(),
            coordinator_key: var("COORDINATOR_KEY_FILE", "./private/coordinator.key").into(),
            envelope_key: var("ENVELOPE_SK_FILE", "./private/envelope.sk").into(),
            hot_seed: var("HOT_WALLET_SEED_FILE", "./private/hot-wallet.seed").into(),
            treasury: env::var("TREASURY_ADDRESS").context("TREASURY_ADDRESS required")?,
            rpc_url: var("ZEBRA_RPC_URL", "http://127.0.0.1:18232"),
            zaino_url: var("LIGHTWALLETD_GRPC_URL", "http://127.0.0.1:9067"),
            ingest_url,
            helper: var(
                "WALLET_HELPER_BIN",
                "./wallet-helper/target/release/frosted-wallet-helper",
            )
            .into(),
            min_bet_zat: var("MIN_BET_ZAT", "1000000").parse()?,
            min_confirmations: var("MIN_CONFIRMATIONS", "3").parse()?,
            max_mainnet_demo_zat: var("MAX_MAINNET_DEMO_ZAT", "50000000").parse()?,
            sync_interval: Duration::from_secs(var("ESCROW_SYNC_INTERVAL_SECS", "30").parse()?),
        };
        if config.sync_interval.is_zero() || config.min_confirmations == 0 {
            bail!("sync interval and confirmations must be positive");
        }
        zcash_glass::params::assert_invariants(&zcash_glass::params::Params {
            min_bet_zat: config.min_bet_zat,
            ..Default::default()
        })?;
        zcash_glass::address::parse_payout_ua(&config.treasury, &config.network)
            .context("invalid treasury Orchard address")?;
        Ok(config)
    }
    pub fn secrets(&self) -> Result<Secrets> {
        Secrets::load(&self.secrets_key)
    }
}
pub fn local_bind(value: &str) -> Result<SocketAddr> {
    let a: SocketAddr = value.parse()?;
    if !a.ip().is_loopback() {
        bail!("coordinator must bind loopback");
    }
    Ok(a)
}
pub fn validate_ingest_url(url: &Url, network: &str) -> Result<()> {
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        bail!("invalid ingest URL");
    }
    let loopback = url.host_str().is_some_and(|s| {
        s == "localhost"
            || s.parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
    });
    let local_http = matches!(network, "regtest" | "testnet") && loopback && url.scheme() == "http";
    if url.scheme() != "https" && !local_http {
        bail!("ingest requires HTTPS (a local regtest or testnet run may use HTTP on loopback)");
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn localhost_only_and_https() {
        assert!(local_bind("0.0.0.0:8090").is_err());
        assert!(local_bind("127.0.0.1:8090").is_ok());
        assert!(
            validate_ingest_url(
                &Url::parse("http://example.com/v1/ingest").unwrap(),
                "regtest"
            )
            .is_err()
        );
        assert!(
            validate_ingest_url(
                &Url::parse("http://localhost/v1/ingest").unwrap(),
                "regtest"
            )
            .is_ok()
        );
        assert!(
            validate_ingest_url(
                &Url::parse("http://localhost/v1/ingest").unwrap(),
                "testnet"
            )
            .is_ok()
        );
        assert!(
            validate_ingest_url(
                &Url::parse("http://example.com/v1/ingest").unwrap(),
                "testnet"
            )
            .is_err()
        );
        assert!(
            validate_ingest_url(
                &Url::parse("http://localhost/v1/ingest").unwrap(),
                "mainnet"
            )
            .is_err()
        );
    }
}

//! Minimal zebrad JSON-RPC client (methods used by the indexer, §22.4).

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use zcash_glass::scan::ActivationHeights;

/// Retries on a provider's requests-per-second error before giving up.
const RATE_LIMIT_RETRIES: u32 = 12;

#[derive(Clone)]
pub struct Rpc {
    url: String,
    http: reqwest::Client,
}

/// `getblock(h, 1)`: header fields and txids in block order.
#[derive(Debug, Deserialize)]
pub struct Block {
    pub hash: String,
    pub height: u32,
    #[serde(default, rename = "previousblockhash")]
    pub previous_block_hash: Option<String>,
    pub tx: Vec<String>,
}

impl Rpc {
    pub fn new(url: impl Into<String>) -> Self {
        Rpc {
            url: url.into(),
            http: reqwest::Client::new(),
        }
    }

    async fn call<T: DeserializeOwned>(&self, method: &str, params: Value) -> Result<T> {
        let body = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
        let mut attempt = 0;
        loop {
            let resp: Value = self
                .http
                .post(&self.url)
                .json(&body)
                .send()
                .await
                // The node URL may carry an access key: keep it out of logs.
                .map_err(reqwest::Error::without_url)
                .with_context(|| format!("{method}: node unreachable"))?
                .json()
                .await
                .map_err(reqwest::Error::without_url)
                .with_context(|| format!("{method}: invalid JSON"))?;
            if let Some(err) = resp.get("error").filter(|e| !e.is_null()) {
                // Hosted nodes cap requests per second (-32005); wait and retry instead of failing.
                if err.get("code").and_then(Value::as_i64) == Some(-32005)
                    && attempt < RATE_LIMIT_RETRIES
                {
                    attempt += 1;
                    tokio::time::sleep(std::time::Duration::from_millis(250 * u64::from(attempt)))
                        .await;
                    continue;
                }
                bail!("{method}: {err}");
            }
            return serde_json::from_value(resp.get("result").cloned().unwrap_or(Value::Null))
                .with_context(|| format!("{method}: unexpected result"));
        }
    }

    /// Tip height. (`getbestblockheightandhash` returns the hash as a byte array in Zebra 6.4.2,
    /// so the follower uses `getblockcount` + `getblockhash`.)
    pub async fn block_count(&self) -> Result<u32> {
        self.call("getblockcount", json!([])).await
    }

    pub async fn block_hash(&self, height: u32) -> Result<String> {
        self.call("getblockhash", json!([height])).await
    }

    pub async fn block(&self, height: u32) -> Result<Block> {
        self.call("getblock", json!([height.to_string(), 1])).await
    }

    pub async fn raw_transaction(&self, txid: &str) -> Result<Vec<u8>> {
        let hex: String = self.call("getrawtransaction", json!([txid, 0])).await?;
        hex::decode(hex).map_err(|e| anyhow!("getrawtransaction: bad hex: {e}"))
    }

    /// Activation heights from `getblockchaininfo.upgrades` (validator is authoritative, ADR-0016).
    pub async fn activation_heights(&self) -> Result<(String, ActivationHeights)> {
        let info: Value = self.call("getblockchaininfo", json!([])).await?;
        let chain = info["chain"].as_str().unwrap_or_default().to_string();
        let mut heights = ActivationHeights::default();
        if let Some(upgrades) = info["upgrades"].as_object() {
            for u in upgrades.values() {
                if let (Some(name), Some(h)) = (u["name"].as_str(), u["activationheight"].as_u64())
                {
                    heights.set(name, h as u32);
                }
            }
        }
        Ok((chain, heights))
    }
}

#[cfg(test)]
mod tests {
    use super::Rpc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    async fn serve(replies: Vec<&'static str>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            for body in replies {
                let (mut sock, _) = listener.accept().await.unwrap();
                let mut buf = vec![0u8; 8192];
                let _ = sock.read(&mut buf).await.unwrap();
                let resp = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                sock.write_all(resp.as_bytes()).await.unwrap();
            }
        });
        url
    }

    const LIMITED: &str =
        r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32005,"message":"RPS limit"}}"#;
    const OK: &str = r#"{"jsonrpc":"2.0","id":1,"result":4537016}"#;

    #[tokio::test]
    async fn errors_never_name_the_node_url() {
        // Hosted node URLs carry an access key; a failed call must not print it.
        let err = Rpc::new("http://127.0.0.1:1/secret-access-key")
            .block_count()
            .await
            .unwrap_err();
        assert!(!format!("{err:#}").contains("secret-access-key"), "{err:#}");
    }

    #[tokio::test]
    async fn retries_when_the_provider_rate_limits() {
        let url = serve(vec![LIMITED, LIMITED, OK]).await;
        assert_eq!(Rpc::new(url).block_count().await.unwrap(), 4_537_016);
    }

    #[tokio::test]
    async fn other_errors_are_not_retried() {
        let url = serve(vec![
            r#"{"jsonrpc":"2.0","id":1,"error":{"code":-8,"message":"bad"}}"#,
        ])
        .await;
        assert!(Rpc::new(url).block_count().await.is_err());
    }
}

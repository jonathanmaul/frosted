//! Minimal zebrad JSON-RPC client (methods used by the indexer, §22.4).

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use zcash_glass::scan::ActivationHeights;

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
        let resp: Value = self
            .http
            .post(&self.url)
            .json(&body)
            .send()
            .await
            .with_context(|| format!("{method}: node unreachable"))?
            .json()
            .await
            .with_context(|| format!("{method}: invalid JSON"))?;
        if let Some(err) = resp.get("error").filter(|e| !e.is_null()) {
            bail!("{method}: {err}");
        }
        serde_json::from_value(resp.get("result").cloned().unwrap_or(Value::Null))
            .with_context(|| format!("{method}: unexpected result"))
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

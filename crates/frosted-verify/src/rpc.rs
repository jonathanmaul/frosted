//! Production `Node` (zebrad JSON-RPC) and `Api` (Frosted REST) implementations.

use std::{collections::HashMap, sync::Mutex, time::Duration};

use anyhow::{Context, Result, anyhow, bail};
use futures::{StreamExt, TryStreamExt, stream};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use zcash_glass::{
    hash::{MarketDefinition, note_ref},
    memo::{self, FRSTM1_PREFIX, bet_fingerprint, decode_frstm1},
    scan::{ActivationHeights, MarketViewingKey, ParsedTx, ScanNetwork},
    status::BetStatus,
    validate::{Classification, NoteFacts, classify},
};

use crate::source::{Api, Node, Note, Registration, Scan};

const TX_FETCH_CONCURRENCY: usize = 8;

/// A block as the scanner needs it: raw transactions in block order.
struct RawBlock {
    txs: Vec<(String, Vec<u8>)>,
}

/// zebrad JSON-RPC. Raw blocks are kept in memory for the life of the process, so checking several
/// markets (or a Persona across markets) reads each block from the node once.
pub struct RpcNode {
    url: String,
    http: reqwest::Client,
    blocks: Mutex<HashMap<u32, std::sync::Arc<RawBlock>>>,
}

impl RpcNode {
    pub fn new(url: impl Into<String>) -> Result<Self> {
        Ok(RpcNode {
            url: url.into(),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()?,
            blocks: Mutex::default(),
        })
    }

    async fn call<T: DeserializeOwned>(&self, method: &str, params: Value) -> Result<T> {
        let body = json!({"jsonrpc":"2.0","id":1,"method":method,"params":params});
        let resp: Value = self
            .http
            .post(&self.url)
            .json(&body)
            .send()
            .await
            // The node URL may carry an access key: keep it out of every error message.
            .map_err(reqwest::Error::without_url)
            .with_context(|| format!("{method}: node unreachable"))?
            .json()
            .await
            .map_err(reqwest::Error::without_url)
            .with_context(|| format!("{method}: node returned invalid JSON"))?;
        if let Some(err) = resp.get("error").filter(|e| !e.is_null()) {
            bail!("{method}: {err}");
        }
        serde_json::from_value(resp.get("result").cloned().unwrap_or(Value::Null))
            .with_context(|| format!("{method}: unexpected result"))
    }

    /// Activation heights come from the validator (ADR-0016), never from library defaults.
    async fn network(&self, name: &str) -> Result<ScanNetwork> {
        let info: Value = self.call("getblockchaininfo", json!([])).await?;
        let mut heights = ActivationHeights::default();
        if let Some(upgrades) = info["upgrades"].as_object() {
            for u in upgrades.values() {
                if let (Some(n), Some(h)) = (u["name"].as_str(), u["activationheight"].as_u64()) {
                    heights.set(n, h as u32);
                }
            }
        }
        let chain = info["chain"].as_str().unwrap_or_default();
        // zebrad reports `test` for both testnet and regtest; the activation heights tell them apart.
        let matches = matches!(
            (chain, name),
            ("main", "mainnet") | ("test" | "regtest", "testnet" | "regtest")
        );
        if !matches {
            bail!("this node is on chain {chain:?} but the market is on {name:?}");
        }
        Ok(ScanNetwork::new(name, heights)?)
    }

    async fn block(&self, height: u32) -> Result<std::sync::Arc<RawBlock>> {
        if let Some(b) = self.blocks.lock().expect("block cache").get(&height) {
            return Ok(b.clone());
        }
        let header: Value = self
            .call("getblock", json!([height.to_string(), 1]))
            .await?;
        let txids: Vec<String> = header["tx"]
            .as_array()
            .context("getblock: no tx list")?
            .iter()
            .map(|t| t.as_str().map(str::to_owned).context("getblock: bad txid"))
            .collect::<Result<_>>()?;
        let txs = stream::iter(txids)
            .map(|txid| async move {
                let hex: String = self.call("getrawtransaction", json!([txid, 0])).await?;
                let bytes =
                    hex::decode(hex).map_err(|e| anyhow!("getrawtransaction: bad hex: {e}"))?;
                Ok::<_, anyhow::Error>((txid, bytes))
            })
            .buffered(TX_FETCH_CONCURRENCY)
            .try_collect()
            .await?;
        let block = std::sync::Arc::new(RawBlock { txs });
        self.blocks
            .lock()
            .expect("block cache")
            .insert(height, block.clone());
        Ok(block)
    }
}

impl Node for RpcNode {
    async fn network_name(&self) -> Result<String> {
        let info: Value = self.call("getblockchaininfo", json!([])).await?;
        Ok(match info["chain"].as_str().unwrap_or_default() {
            "main" => "mainnet",
            "test" => "testnet",
            other => other,
        }
        .to_owned())
    }

    async fn tip(&self) -> Result<u32> {
        self.call("getblockcount", json!([])).await
    }

    async fn scan(&self, def: &MarketDefinition, rules_hash: &[u8; 32], upto: u32) -> Result<Scan> {
        let network = self.network(&def.network).await?;
        let key = MarketViewingKey::from_uivk(&network, &def.uivk)?;
        let mut scan = Scan {
            tip: upto,
            ..Scan::default()
        };
        let mut announced = false;
        for height in def.birthday_height..=upto {
            let block = self.block(height).await?;
            for (tx_index, (txid_hex, bytes)) in block.txs.iter().enumerate() {
                let parsed = ParsedTx::parse(bytes, height, &network)
                    .with_context(|| format!("parsing {txid_hex} at {height}"))?;
                let txid = parsed.txid();
                let transparent = parsed.has_transparent_input();
                for n in parsed.decrypt(&key) {
                    let facts = NoteFacts {
                        amount_zat: n.value_zat,
                        block_height: height,
                        diversifier_index: n.diversifier_index,
                        memo_text: n.memo_text(),
                        has_transparent_input: transparent,
                    };
                    let nref = note_ref(&txid, n.action_index);
                    if scan.first_frstm1_hash.is_none()
                        && n.diversifier_index == Some(0)
                        && let Some(h) = n
                            .memo_text()
                            .filter(|m| m.starts_with(FRSTM1_PREFIX))
                            .and_then(|m| decode_frstm1(m).ok())
                    {
                        scan.first_frstm1_hash = Some(h);
                    }
                    let (status, outcome_index, payload, is_announcement) =
                        match classify(&facts, def, rules_hash, announced) {
                            Classification::Announcement => {
                                announced = true;
                                scan.announcement = Some((nref.clone(), height));
                                (BetStatus::InvalidMemo, Some(0), None, true)
                            }
                            Classification::Note {
                                status,
                                outcome_index,
                                payload,
                            } => (status, outcome_index, payload, false),
                        };
                    scan.notes.push(Note {
                        note_ref: nref,
                        outcome_index,
                        amount_zat: n.value_zat,
                        block_height: height,
                        tx_index: tx_index as u32,
                        action_index: n.action_index,
                        status,
                        is_announcement,
                        bet_fingerprint: payload.as_ref().map(bet_fingerprint),
                        payload,
                    });
                }
            }
        }
        Ok(scan)
    }

    async fn resolution_data(&self, height: u32) -> Result<(Value, Value)> {
        let block = self
            .call("getblock", json!([height.to_string(), 1]))
            .await?;
        let info = self.call("getblockchaininfo", json!([])).await?;
        Ok((block, info))
    }

    async fn registrations(
        &self,
        uivk: &str,
        network: &str,
        birthday: u32,
        upto: u32,
    ) -> Result<Vec<Registration>> {
        let network = self.network(network).await?;
        let key = MarketViewingKey::from_uivk(&network, uivk)?;
        let mut out = Vec::new();
        for height in birthday..=upto {
            let block = self.block(height).await?;
            for (txid_hex, bytes) in &block.txs {
                let parsed = ParsedTx::parse(bytes, height, &network)
                    .with_context(|| format!("parsing {txid_hex} at {height}"))?;
                for n in parsed.decrypt(&key) {
                    if let Some(commitment) =
                        n.memo_text().and_then(|m| memo::decode_frstp1(m).ok())
                    {
                        out.push(Registration { commitment, height });
                    }
                }
            }
        }
        Ok(out)
    }
}

/// The public Frosted REST API (`--api`, default `http://127.0.0.1:8080/v1`).
pub struct HttpApi {
    base: String,
    http: reqwest::Client,
}

impl HttpApi {
    pub fn new(base: impl Into<String>) -> Result<Self> {
        Ok(HttpApi {
            base: base.into().trim_end_matches('/').to_owned(),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()?,
        })
    }

    /// `None` on 404, an error for anything else that is not 200.
    async fn get_opt(&self, path: &str) -> Result<Option<Value>> {
        let resp = self
            .http
            .get(format!("{}{path}", self.base))
            .send()
            .await
            .with_context(|| format!("API unreachable at {}", self.base))?;
        match resp.status().as_u16() {
            200 => Ok(Some(
                resp.json()
                    .await
                    .with_context(|| format!("GET {path}: invalid JSON"))?,
            )),
            404 => Ok(None),
            code => bail!("GET {path}: HTTP {code}"),
        }
    }

    async fn get(&self, path: &str) -> Result<Value> {
        self.get_opt(path)
            .await?
            .with_context(|| format!("GET {path}: not found"))
    }
}

impl Api for HttpApi {
    async fn market(&self, id: &str) -> Result<Value> {
        self.get(&format!("/markets/{id}")).await
    }
    async fn odds(&self, id: &str) -> Result<Value> {
        self.get(&format!("/markets/{id}/odds")).await
    }
    async fn resolution(&self, id: &str) -> Result<Value> {
        self.get(&format!("/markets/{id}/resolution")).await
    }
    async fn payouts(&self, id: &str) -> Result<Vec<Value>> {
        let body = self.get(&format!("/markets/{id}/payouts")).await?;
        Ok(body["plans"].as_array().cloned().unwrap_or_default())
    }
    async fn market_ids(&self) -> Result<Vec<String>> {
        let body = self.get("/markets").await?;
        Ok(body["markets"]
            .as_array()
            .map(|m| {
                m.iter()
                    .filter_map(|m| m["market_id"].as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default())
    }
    async fn indexer_height(&self) -> Result<u32> {
        let body = self.get("/health").await?;
        body["indexer_height"]
            .as_u64()
            .map(|h| h as u32)
            .context("health: no indexer_height")
    }
    async fn bets(&self, id: &str) -> Result<Value> {
        self.get(&format!("/markets/{id}/bets?limit=500")).await
    }
    async fn persona(&self, id: &str) -> Result<Option<Value>> {
        self.get_opt(&format!("/personas/{id}")).await
    }
    async fn thaw_report(&self, id: &str) -> Result<Option<Value>> {
        self.get_opt(&format!("/personas/{id}/report")).await
    }
    async fn registry(&self) -> Result<Option<Value>> {
        Ok(self
            .get_opt("/personas/registry")
            .await?
            .filter(|v| !v.is_null()))
    }
}

#[cfg(test)]
mod tests {
    use super::RpcNode;

    #[tokio::test]
    async fn errors_never_name_the_node_url() {
        // Hosted node URLs carry an access key; the public API returns these errors' context.
        let node = RpcNode::new("http://127.0.0.1:1/secret-access-key").unwrap();
        let err = node
            .call::<serde_json::Value>("getblockcount", serde_json::json!([]))
            .await
            .unwrap_err();
        assert!(!format!("{err:#}").contains("secret-access-key"), "{err:#}");
    }
}

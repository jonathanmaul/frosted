//! Glass Indexer jobs J-1..J-4 (docs/PROJECT_SPEC_FROSTED.md §23.3): chain follower with reorg
//! handling, UIVK scanner, confirmation promoter, height-based market lifecycle, Persona Registry
//! scanning, and the event feed read by `frosted-api`.

use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use futures::{StreamExt, TryStreamExt, stream};
use serde_json::json;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::{Row, Sqlite, Transaction};
use tracing::{info, warn};
use zcash_glass::hash::{MarketDefinition, note_ref, rules_hash};
use zcash_glass::memo::{bet_fingerprint, decode_frstp1};
use zcash_glass::params::REORG_DEPTH;
use zcash_glass::scan::{MarketViewingKey, ParsedTx, ScanNetwork};
use zcash_glass::status::BetStatus;
use zcash_glass::thaw;
use zcash_glass::validate::{Classification, NoteFacts, classify, confirmations};

use crate::rpc::Rpc;

/// Parallel `getrawtransaction` calls per block.
const TX_FETCH_CONCURRENCY: usize = 8;
/// Blocks fetched ahead of the one being processed. Fetching is network-bound and stateless, so
/// it overlaps safely; processing stays strictly in height order.
const BLOCK_PREFETCH: usize = 2;

/// A block and its raw transactions, fetched but not yet processed.
struct Fetched {
    block: crate::rpc::Block,
    raw: Vec<(usize, String, Vec<u8>)>,
}

pub struct Indexer {
    pool: SqlitePool,
    rpc: Rpc,
    /// `FROSTED_NETWORK` (`regtest`, `testnet`, `mainnet`). Zebra reports `chain = "test"` for
    /// both testnet and regtest, so the network is configured, not inferred.
    network: String,
}

struct ActiveMarket {
    market_id: String,
    def: MarketDefinition,
    rules_hash: [u8; 32],
    key: MarketViewingKey,
    announcement_seen: bool,
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

fn status_str(s: BetStatus) -> String {
    serde_json::to_value(s)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .expect("BetStatus serializes to a string")
}

async fn emit(
    tx: &mut Transaction<'_, Sqlite>,
    market_id: &str,
    kind: &str,
    payload: serde_json::Value,
    height: u32,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO event (market_id, kind, payload_json, block_height, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(market_id)
    .bind(kind)
    .bind(payload.to_string())
    .bind(height)
    .bind(now())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Registers a published market and schedules its historical scan in the caller's transaction.
/// The API includes replay bookkeeping in this same transaction, so retries never rewind twice.
pub async fn register_market(
    tx: &mut Transaction<'_, Sqlite>,
    definition_json: &str,
) -> Result<String> {
    let def: MarketDefinition =
        serde_json::from_str(definition_json).context("invalid market_definition")?;
    let hash = rules_hash(&def).context("rules_hash")?;
    sqlx::query(
            "INSERT INTO market (market_id, definition_json, rules_hash, uivk, network, birthday_height,
               close_height, resolve_height, min_confirmations, announce_timeout_blocks, status, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'ANNOUNCING', ?)",
        )
        .bind(&def.market_id)
        .bind(definition_json)
        .bind(hex::encode(hash))
        .bind(&def.uivk)
        .bind(&def.network)
        .bind(def.birthday_height)
        .bind(def.close_height)
        .bind(def.resolve_height)
        .bind(def.min_confirmations)
        .bind(def.announce_timeout_blocks)
        .bind(now())
        .execute(&mut **tx)
        .await?;
    for o in &def.outcomes {
        sqlx::query(
            "INSERT INTO outcome (market_id, outcome_index, label, address) VALUES (?, ?, ?, ?)",
        )
        .bind(&def.market_id)
        .bind(o.outcome_index)
        .bind(&o.label)
        .bind(&o.address)
        .execute(&mut **tx)
        .await?;
    }
    rewind(tx, def.birthday_height.saturating_sub(1)).await?;
    Ok(def.market_id)
}

async fn rewind(tx: &mut Transaction<'_, Sqlite>, height: u32) -> Result<()> {
    sqlx::query("DELETE FROM bet WHERE block_height > ?")
        .bind(height)
        .execute(&mut **tx)
        .await?;
    sqlx::query("DELETE FROM event WHERE block_height > ?")
        .bind(height)
        .execute(&mut **tx)
        .await?;
    sqlx::query("DELETE FROM persona WHERE registered_height > ?")
        .bind(height)
        .execute(&mut **tx)
        .await?;
    sqlx::query("UPDATE market SET announce_height = NULL, announce_note_ref = NULL WHERE announce_height > ?")
            .bind(height)
            .execute(&mut **tx)
            .await?;
    sqlx::query("DELETE FROM block WHERE height > ?")
        .bind(height)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

impl Indexer {
    pub async fn open(db_path: &str, rpc_url: &str, network: &str) -> Result<Self> {
        let opts = SqliteConnectOptions::new()
            .filename(db_path)
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(opts)
            .await
            .with_context(|| format!("opening {db_path}"))?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Indexer {
            pool,
            rpc: Rpc::new(rpc_url),
            network: network.to_string(),
        })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Stores a Market from its published `market_definition` (strict JSON). Used by tests and by
    /// the ingest endpoint (M5). Blocks at or after its birthday are rescanned.
    pub async fn add_market(&self, definition_json: &str) -> Result<String> {
        let mut tx = self.pool.begin().await?;
        let id = register_market(&mut tx, definition_json).await?;
        tx.commit().await?;
        Ok(id)
    }

    /// Sets the Persona Registry (single row) and rescans from its birthday.
    pub async fn set_registry(
        &self,
        uivk: &str,
        address: &str,
        network: &str,
        birthday: u32,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO persona_registry (id, uivk, address, network, birthday_height) VALUES (1, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET uivk = excluded.uivk, address = excluded.address,
               network = excluded.network, birthday_height = excluded.birthday_height",
        )
        .bind(uivk)
        .bind(address)
        .bind(network)
        .bind(birthday)
        .execute(&self.pool)
        .await?;
        self.rewind_to(birthday.saturating_sub(1)).await
    }

    async fn last_block(&self) -> Result<Option<(u32, String)>> {
        Ok(
            sqlx::query("SELECT height, hash FROM block ORDER BY height DESC LIMIT 1")
                .fetch_optional(&self.pool)
                .await?
                .map(|r| (r.get::<u32, _>(0), r.get::<String, _>(1))),
        )
    }

    async fn stored_hash(&self, height: u32) -> Result<Option<String>> {
        Ok(
            sqlx::query_scalar("SELECT hash FROM block WHERE height = ?")
                .bind(height)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    /// Lowest height anything must be scanned from, if anything is configured.
    async fn scan_start(&self) -> Result<Option<u32>> {
        Ok(sqlx::query_scalar(
            "SELECT MIN(h) FROM (SELECT birthday_height AS h FROM market
                                 UNION ALL SELECT birthday_height FROM persona_registry)",
        )
        .fetch_one(&self.pool)
        .await?)
    }

    /// Deletes everything derived from blocks above `height` (reorg or rescan).
    async fn rewind_to(&self, height: u32) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        rewind(&mut tx, height).await?;
        tx.commit().await?;
        Ok(())
    }

    /// J-1 reorg detection: walk back while stored hashes differ from the node's.
    async fn reconcile(&self) -> Result<()> {
        let Some((mut height, _)) = self.last_block().await? else {
            return Ok(());
        };
        let top = height;
        loop {
            let stored = self.stored_hash(height).await?;
            let node = self.rpc.block_hash(height).await.ok();
            if stored.is_none() || stored == node {
                break;
            }
            if top - height >= REORG_DEPTH {
                bail!("reorg deeper than REORG_DEPTH ({REORG_DEPTH}) at height {top}");
            }
            if height == 0 {
                break;
            }
            height -= 1;
        }
        if height < top {
            warn!(fork = height, top, "reorg detected, rewinding");
            self.rewind_to(height).await?;
        }
        Ok(())
    }

    /// Processes every block up to the node tip once, then promotes confirmations and updates
    /// market statuses. Returns the tip.
    pub async fn sync_once(&self) -> Result<u32> {
        self.reconcile().await?;
        let tip = self.rpc.block_count().await?;
        let (_chain, heights) = self.rpc.activation_heights().await?;
        let network = ScanNetwork::new(&self.network, heights)?;

        let next = match self.last_block().await? {
            Some((h, _)) => h + 1,
            None => match self.scan_start().await? {
                Some(start) => start,
                None => tip + 1, // nothing configured yet
            },
        };
        let mut fetched = stream::iter(next..=tip)
            .map(|height| async move { self.fetch_block(height).await.map(|f| (height, f)) })
            .buffered(BLOCK_PREFETCH);
        while let Some(item) = fetched.next().await {
            let (height, f) = item?;
            self.process_block(height, &network, f).await?;
        }
        self.promote_and_lifecycle(tip).await?;
        Ok(tip)
    }

    /// J-1 loop.
    pub async fn run(&self, poll: std::time::Duration) -> Result<()> {
        loop {
            match self.sync_once().await {
                Ok(tip) => tracing::debug!(tip, "synced"),
                Err(e) => warn!("sync failed, retrying: {e:#}"),
            }
            tokio::time::sleep(poll).await;
        }
    }

    async fn active_markets(
        &self,
        height: u32,
        network: &ScanNetwork,
    ) -> Result<Vec<ActiveMarket>> {
        let rows = sqlx::query(
            "SELECT market_id, definition_json, rules_hash, announce_height FROM market WHERE birthday_height <= ?",
        )
        .bind(height)
        .fetch_all(&self.pool)
        .await?;
        let mut out = Vec::new();
        for r in rows {
            let market_id: String = r.get(0);
            let def: MarketDefinition = serde_json::from_str(r.get::<&str, _>(1))?;
            if chain_name_matches(network, &def.network) {
                let mut rh = [0u8; 32];
                hex::decode_to_slice(r.get::<&str, _>(2), &mut rh)?;
                let key = MarketViewingKey::from_uivk(network, &def.uivk)?;
                out.push(ActiveMarket {
                    market_id,
                    def,
                    rules_hash: rh,
                    key,
                    announcement_seen: r.get::<Option<u32>, _>(3).is_some(),
                });
            } else {
                warn!(
                    market_id,
                    network = def.network,
                    "market network does not match the node; skipped"
                );
            }
        }
        Ok(out)
    }

    async fn registry_key(
        &self,
        height: u32,
        network: &ScanNetwork,
    ) -> Result<Option<MarketViewingKey>> {
        let row =
            sqlx::query("SELECT uivk FROM persona_registry WHERE id = 1 AND birthday_height <= ?")
                .bind(height)
                .fetch_optional(&self.pool)
                .await?;
        row.map(|r| MarketViewingKey::from_uivk(network, r.get::<&str, _>(0)))
            .transpose()
            .map_err(Into::into)
    }

    /// Network half of J-2: the block and every transaction in it. Touches no database state.
    async fn fetch_block(&self, height: u32) -> Result<Fetched> {
        let block = self.rpc.block(height).await?;
        let raw: Vec<(usize, String, Vec<u8>)> = stream::iter(block.tx.iter().cloned().enumerate())
            .map(|(i, txid)| {
                let rpc = self.rpc.clone();
                async move { rpc.raw_transaction(&txid).await.map(|b| (i, txid, b)) }
            })
            .buffered(TX_FETCH_CONCURRENCY)
            .try_collect()
            .await?;
        Ok(Fetched { block, raw })
    }

    /// J-2 for one block.
    async fn process_block(
        &self,
        height: u32,
        network: &ScanNetwork,
        fetched: Fetched,
    ) -> Result<()> {
        let Fetched { block, raw } = fetched;
        let stored_prev = self.stored_hash(height.saturating_sub(1)).await?;
        if let (Some(prev), Some(stored)) = (&block.previous_block_hash, stored_prev)
            && *prev != stored
        {
            bail!("block {height} does not extend stored chain; reconcile first");
        }
        let mut markets = self.active_markets(height, network).await?;
        let registry = self.registry_key(height, network).await?;

        let mut tx = self.pool.begin().await?;
        for (tx_index, txid_hex, bytes) in raw {
            if markets.is_empty() && registry.is_none() {
                break;
            }
            let parsed = ParsedTx::parse(&bytes, height, network)
                .with_context(|| format!("parsing {txid_hex} at {height}"))?;
            let txid = parsed.txid();
            let transparent = parsed.has_transparent_input();

            for m in markets.iter_mut() {
                for note in parsed.decrypt(&m.key) {
                    let facts = NoteFacts {
                        amount_zat: note.value_zat,
                        block_height: height,
                        diversifier_index: note.diversifier_index,
                        memo_text: note.memo_text(),
                        has_transparent_input: transparent,
                    };
                    let class = classify(&facts, &m.def, &m.rules_hash, m.announcement_seen);
                    let nref = note_ref(&txid, note.action_index);
                    let (status, outcome, payload, is_announcement) = match &class {
                        Classification::Announcement => {
                            m.announcement_seen = true;
                            ("ANNOUNCEMENT".to_string(), Some(0), None, true)
                        }
                        Classification::Note {
                            status,
                            outcome_index,
                            payload,
                        } => (status_str(*status), *outcome_index, *payload, false),
                    };
                    let fingerprint = payload.as_ref().map(bet_fingerprint);
                    let tagged = payload.as_ref().is_some_and(|p| p.header.has_thaw_tag());
                    sqlx::query(
                        "INSERT OR IGNORE INTO bet (note_ref, bet_id, market_id, outcome_index, amount_zat,
                           block_height, block_hash, tx_index, action_index, has_transparent_input,
                           is_announcement, status, bet_fingerprint, tagged, thaw_tag, memo_payload, first_seen_at)
                         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                    )
                    .bind(&nref)
                    .bind(format!("{txid_hex}:{}", note.action_index))
                    .bind(&m.market_id)
                    .bind(outcome)
                    .bind(note.value_zat as i64)
                    .bind(height)
                    .bind(&block.hash)
                    .bind(tx_index as i64)
                    .bind(note.action_index)
                    .bind(transparent)
                    .bind(is_announcement)
                    .bind(&status)
                    .bind(&fingerprint)
                    .bind(tagged)
                    .bind(payload.filter(|p| p.header.has_thaw_tag()).map(|p| p.header.thaw_tag.to_vec()))
                    .bind(payload.map(|p| p.to_bytes().to_vec()))
                    .bind(now())
                    .execute(&mut *tx)
                    .await?;
                    if is_announcement {
                        sqlx::query("UPDATE market SET announce_height = ?, announce_note_ref = ? WHERE market_id = ?")
                            .bind(height)
                            .bind(&nref)
                            .bind(&m.market_id)
                            .execute(&mut *tx)
                            .await?;
                    }
                    emit(
                        &mut tx,
                        &m.market_id,
                        "bet_status_changed",
                        json!({ "note_ref": nref, "bet_fingerprint": fingerprint, "status": status }),
                        height,
                    )
                    .await?;
                }
            }

            if let Some(key) = &registry {
                for note in parsed.decrypt(key) {
                    let Some(commitment) = note.memo_text().and_then(|m| decode_frstp1(m).ok())
                    else {
                        continue;
                    };
                    sqlx::query(
                        "INSERT OR IGNORE INTO persona (persona_id, commitment, registered_height, registered_note_ref)
                         VALUES (?, ?, ?, ?)",
                    )
                    .bind(thaw::persona_id(&commitment))
                    .bind(commitment.to_vec())
                    .bind(height)
                    .bind(note_ref(&txid, note.action_index))
                    .execute(&mut *tx)
                    .await?;
                }
            }
        }
        sqlx::query(
            "INSERT INTO block (height, hash, prev_hash, processed_at) VALUES (?, ?, ?, ?)",
        )
        .bind(height)
        .bind(&block.hash)
        .bind(block.previous_block_hash.unwrap_or_default())
        .bind(now())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// J-3 (PENDING ↔ COUNTED by confirmations) and J-4 (height-based market statuses).
    async fn promote_and_lifecycle(&self, tip: u32) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let markets = sqlx::query(
            "SELECT market_id, min_confirmations, birthday_height, close_height, resolve_height,
                    announce_timeout_blocks, announce_height, status, status_source FROM market",
        )
        .fetch_all(&mut *tx)
        .await?;
        for m in markets {
            let market_id: String = m.get(0);
            let min_conf: u32 = m.get(1);
            let mut odds_changed = false;

            let pending = sqlx::query("SELECT note_ref, bet_fingerprint, block_height FROM bet WHERE market_id = ? AND status = 'PENDING'")
                .bind(&market_id)
                .fetch_all(&mut *tx)
                .await?;
            for b in pending {
                let height: u32 = b.get(2);
                if confirmations(height, tip) >= min_conf {
                    let nref: String = b.get(0);
                    sqlx::query("UPDATE bet SET status = 'COUNTED' WHERE note_ref = ?")
                        .bind(&nref)
                        .execute(&mut *tx)
                        .await?;
                    emit(&mut tx, &market_id, "bet_counted", json!({ "note_ref": nref, "bet_fingerprint": b.get::<Option<String>, _>(1) }), tip).await?;
                    odds_changed = true;
                }
            }
            // After a reorg the tip can drop below a counted bet's confirmation height.
            let demoted = sqlx::query(
                "UPDATE bet SET status = 'PENDING' WHERE market_id = ? AND status = 'COUNTED' AND (? - block_height + 1) < ?",
            )
            .bind(&market_id)
            .bind(tip)
            .bind(min_conf)
            .execute(&mut *tx)
            .await?
            .rows_affected();
            odds_changed |= demoted > 0;
            if odds_changed {
                emit(
                    &mut tx,
                    &market_id,
                    "odds_updated",
                    json!({ "at_height": tip }),
                    tip,
                )
                .await?;
            }

            if m.get::<&str, _>(8) != "indexer" {
                continue;
            }
            let birthday: u32 = m.get(2);
            let close: u32 = m.get(3);
            let resolve: u32 = m.get(4);
            let timeout: u32 = m.get(5);
            let announced = m.get::<Option<u32>, _>(6).is_some();
            let target = if !announced {
                if tip >= birthday + timeout {
                    "VOID"
                } else {
                    "ANNOUNCING"
                }
            } else if tip >= resolve {
                "AWAITING_RESOLUTION"
            } else if tip >= close {
                "CLOSED"
            } else {
                "OPEN"
            };
            let current: String = m.get(7);
            if current != target {
                sqlx::query("UPDATE market SET status = ? WHERE market_id = ?")
                    .bind(target)
                    .bind(&market_id)
                    .execute(&mut *tx)
                    .await?;
                emit(
                    &mut tx,
                    &market_id,
                    "market_status_changed",
                    json!({ "from": current, "to": target }),
                    tip,
                )
                .await?;
                info!(market_id, from = current, to = target, "market status");
            }
        }
        tx.commit().await?;
        Ok(())
    }
}

fn chain_name_matches(network: &ScanNetwork, def_network: &str) -> bool {
    matches!(
        (network, def_network),
        (ScanNetwork::Main, "mainnet")
            | (ScanNetwork::Test, "testnet")
            | (ScanNetwork::Regtest(_), "regtest")
    )
}

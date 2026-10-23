//! `GET /v1/markets/{id}/verify`: the same checks as `frosted-verify all`, run by the server against
//! its own node and database. A convenience for the Verify page, labelled as untrustworthy: a lying
//! server would simply return PASS. Run the CLI on your own node for an independent audit.

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::anyhow;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use frosted_verify::{Api, Command, rpc::RpcNode};
use serde_json::{Value, json};
use sqlx::Row;

use crate::{
    AppState,
    error::{ApiError, Result},
    public,
};

/// A recent report is served again instead of rescanning the chain for every visitor.
const FRESH_FOR: Duration = Duration::from_secs(30);
const BUDGET: Duration = Duration::from_secs(120);
pub const TRUST_NOTICE: &str = "Server-side check, for convenience only. Do not trust it: a dishonest server could print PASS. Run frosted-verify against your own node.";

struct Cached {
    at: Instant,
    indexer_height: u32,
    report: Value,
}

/// One shared node client (its block cache survives requests) and the last report per market.
#[derive(Clone)]
pub(crate) struct Verifier {
    node: Arc<RpcNode>,
    cache: Arc<tokio::sync::Mutex<HashMap<String, Cached>>>,
}

impl Verifier {
    pub(crate) fn new(rpc_url: &str) -> anyhow::Result<Self> {
        Ok(Self {
            node: Arc::new(RpcNode::new(rpc_url)?),
            cache: Arc::default(),
        })
    }
}

/// The server's own database seen through the interface the CLI uses for the HTTP API.
struct Local<'a>(&'a AppState);

fn unsupported<T>() -> anyhow::Result<T> {
    Err(anyhow!("not available in the server-side check"))
}

impl Api for Local<'_> {
    async fn market(&self, id: &str) -> anyhow::Result<Value> {
        Ok(serde_json::to_value(public::market(self.0, id).await?)?)
    }
    async fn odds(&self, id: &str) -> anyhow::Result<Value> {
        let def = public::market(self.0, id).await?.market_definition;
        let at = public::height(self.0).await?;
        Ok(public::odds_at(self.0, &def, at).await?)
    }
    async fn resolution(&self, id: &str) -> anyhow::Result<Value> {
        Ok(public::published(self.0, id, "resolution").await?)
    }
    async fn payouts(&self, id: &str) -> anyhow::Result<Vec<Value>> {
        Ok(public::plans(self.0, id).await?)
    }
    async fn market_ids(&self) -> anyhow::Result<Vec<String>> {
        Ok(
            sqlx::query_scalar("SELECT market_id FROM market ORDER BY created_at, market_id")
                .fetch_all(&self.0.read)
                .await?,
        )
    }
    async fn indexer_height(&self) -> anyhow::Result<u32> {
        Ok(public::height(self.0).await?)
    }
    async fn bets(&self, id: &str) -> anyhow::Result<Value> {
        let rows =
            sqlx::query("SELECT note_ref FROM bet WHERE market_id = ? AND is_announcement = 0")
                .bind(id)
                .fetch_all(&self.0.read)
                .await?;
        Ok(
            json!({"bets": rows.iter().map(|r| json!({"note_ref": r.get::<String, _>(0)})).collect::<Vec<_>>()}),
        )
    }
    async fn persona(&self, _: &str) -> anyhow::Result<Option<Value>> {
        unsupported()
    }
    async fn thaw_report(&self, _: &str) -> anyhow::Result<Option<Value>> {
        unsupported()
    }
    async fn registry(&self) -> anyhow::Result<Option<Value>> {
        unsupported()
    }
}

pub(crate) async fn verify(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    public::market(&state, &id).await?;
    let height = public::height(&state).await?;
    // Holding the lock while computing serializes verification: at most one chain scan runs at a time.
    let mut cache = state.verify.cache.lock().await;
    if let Some(c) = cache.get(&id)
        && c.at.elapsed() < FRESH_FOR
        && c.indexer_height == height
    {
        return Ok(Json(c.report.clone()));
    }
    let unavailable = |why: String| {
        ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "NODE_UNAVAILABLE",
            format!("Server-side verification is unavailable: {why}"),
        )
    };
    let local = Local(&state);
    let run = frosted_verify::run(&local, &*state.verify.node, &id, Command::All);
    let report = match tokio::time::timeout(BUDGET, run).await {
        Err(_) => return Err(unavailable("it took too long".into())),
        Ok(Err(e)) => return Err(unavailable(format!("{e:#}"))),
        Ok(Ok(report)) => report,
    };
    let mut value = serde_json::to_value(&report)?;
    value["trust"] = json!(TRUST_NOTICE);
    cache.insert(
        id,
        Cached {
            at: Instant::now(),
            indexer_height: height,
            report: value.clone(),
        },
    );
    Ok(Json(value))
}

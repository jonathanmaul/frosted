//! Public Glass Indexer: chain follower, UIVK scanner, bet validation, market lifecycle.
//!
//!   glass-indexer run                          follow the chain (J-1..J-4)
//!   glass-indexer once                         process up to the tip once and exit
//!   glass-indexer market-add <definition.json> store a market (dev/test; M5 ingest replaces it)
//!   glass-indexer registry-set <uivk> <address> <birthday>
//!   glass-indexer dump <market_id>             print market status and bets as JSON
//!
//! Config (docs/PROJECT_SPEC_FROSTED.md §27.4): PUBLIC_DB_PATH, ZEBRA_RPC_URL, FROSTED_NETWORK.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use glass_indexer::Indexer;
use sqlx::Row;
use zcash_glass::params::CHAIN_POLL_INTERVAL_SECS;

fn env(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_string())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("LOG_LEVEL")
                .unwrap_or_else(|_| "info".into()),
        )
        .init();
    let db = env("PUBLIC_DB_PATH", "./data/public.db");
    if let Some(dir) = std::path::Path::new(&db).parent() {
        std::fs::create_dir_all(dir).ok();
    }
    let indexer = Indexer::open(
        &db,
        &env("ZEBRA_RPC_URL", "http://127.0.0.1:18232"),
        &env("FROSTED_NETWORK", "regtest"),
    )
    .await?;

    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("run") => {
            indexer
                .run(Duration::from_secs(CHAIN_POLL_INTERVAL_SECS))
                .await
        }
        Some("once") => {
            let tip = indexer.sync_once().await?;
            println!("{tip}");
            Ok(())
        }
        Some("market-add") => {
            let path = args.get(1).context("definition file")?;
            let id = indexer.add_market(&std::fs::read_to_string(path)?).await?;
            println!("{id}");
            Ok(())
        }
        Some("registry-set") => {
            let (uivk, address, birthday) = (args.get(1), args.get(2), args.get(3));
            let (Some(uivk), Some(address), Some(birthday)) = (uivk, address, birthday) else {
                bail!("usage: registry-set <uivk> <address> <birthday>");
            };
            indexer
                .set_registry(
                    uivk,
                    address,
                    &env("FROSTED_NETWORK", "regtest"),
                    birthday.parse()?,
                )
                .await
        }
        Some("dump") => {
            let id = args.get(1).context("market_id")?;
            dump(&indexer, id).await
        }
        _ => bail!(
            "usage: glass-indexer run|once|market-add <file>|registry-set <uivk> <address> <birthday>|dump <market_id>"
        ),
    }
}

async fn dump(indexer: &Indexer, market_id: &str) -> Result<()> {
    let pool = indexer.pool();
    let m = sqlx::query("SELECT status, announce_height FROM market WHERE market_id = ?")
        .bind(market_id)
        .fetch_one(pool)
        .await?;
    let bets = sqlx::query(
        "SELECT note_ref, outcome_index, amount_zat, block_height, status, bet_fingerprint, tagged
         FROM bet WHERE market_id = ? ORDER BY block_height, tx_index, action_index",
    )
    .bind(market_id)
    .fetch_all(pool)
    .await?;
    let bets: Vec<_> = bets
        .iter()
        .map(|b| {
            serde_json::json!({
                "note_ref": b.get::<String, _>(0),
                "outcome_index": b.get::<Option<i64>, _>(1),
                "amount_zat": b.get::<i64, _>(2),
                "block_height": b.get::<i64, _>(3),
                "status": b.get::<String, _>(4),
                "bet_fingerprint": b.get::<Option<String>, _>(5),
                "tagged": b.get::<bool, _>(6),
            })
        })
        .collect();
    let personas: Vec<_> = sqlx::query("SELECT persona_id, registered_height FROM persona ORDER BY registered_height")
        .fetch_all(pool)
        .await?
        .iter()
        .map(|p| serde_json::json!({ "persona_id": p.get::<String, _>(0), "registered_height": p.get::<i64, _>(1) }))
        .collect();
    let events: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM event WHERE market_id = ?")
        .bind(market_id)
        .fetch_one(pool)
        .await?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "status": m.get::<String, _>(0),
            "announce_height": m.get::<Option<i64>, _>(1),
            "bets": bets,
            "personas": personas,
            "events": events,
        }))?
    );
    Ok(())
}

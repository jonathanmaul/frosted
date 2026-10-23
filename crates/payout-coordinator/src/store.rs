use crate::secrets::Secrets;
use anyhow::Result;
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use std::path::Path;

pub async fn open(path: &Path) -> Result<SqlitePool> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if !path.exists() {
        crate::secrets::write_private_new(path, &[])?;
    }
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(false),
        )
        .await?;
    sqlx::raw_sql("PRAGMA journal_mode=DELETE; PRAGMA secure_delete=ON;
 CREATE TABLE IF NOT EXISTS committee(group_id TEXT PRIMARY KEY,public_json TEXT NOT NULL);
 CREATE TABLE IF NOT EXISTS secret(name TEXT PRIMARY KEY,value_enc BLOB NOT NULL);
 CREATE TABLE IF NOT EXISTS market(market_id TEXT PRIMARY KEY,definition_json TEXT NOT NULL,rules_hash TEXT NOT NULL,nk_enc BLOB NOT NULL,rivk_enc BLOB NOT NULL,ufvk_enc BLOB NOT NULL,wallet_enc BLOB NOT NULL,wallet_account_uuid TEXT, balance_zat INTEGER NOT NULL DEFAULT 0, pushed INTEGER NOT NULL DEFAULT 0,announce_txid TEXT,announce_height INTEGER, status TEXT NOT NULL DEFAULT 'ANNOUNCING');").execute(&pool).await?;
    sqlx::raw_sql("CREATE TABLE IF NOT EXISTS resolution(market_id TEXT PRIMARY KEY, body_json TEXT NOT NULL, pushed INTEGER NOT NULL DEFAULT 0); CREATE TABLE IF NOT EXISTS payout(plan_hash TEXT PRIMARY KEY,market_id TEXT NOT NULL,public_json TEXT NOT NULL,private_enc BLOB NOT NULL,status TEXT NOT NULL,cutoff_height INTEGER NOT NULL,created_at INTEGER NOT NULL DEFAULT(unixepoch())); CREATE UNIQUE INDEX IF NOT EXISTS payout_active ON payout(market_id) WHERE status NOT IN ('CONFIRMED','FAILED');").execute(&pool).await?;
    Ok(pool)
}
pub async fn put_secret(pool: &SqlitePool, key: &Secrets, name: &str, value: &[u8]) -> Result<()> {
    sqlx::query("INSERT INTO secret(name,value_enc) VALUES(?,?) ON CONFLICT(name) DO UPDATE SET value_enc=excluded.value_enc").bind(name).bind(key.seal(name,value)?).execute(pool).await?;
    Ok(())
}
pub async fn get_secret(
    pool: &SqlitePool,
    key: &Secrets,
    name: &str,
) -> Result<Option<zeroize::Zeroizing<Vec<u8>>>> {
    let blob: Option<Vec<u8>> = sqlx::query_scalar("SELECT value_enc FROM secret WHERE name=?")
        .bind(name)
        .fetch_optional(pool)
        .await?;
    blob.map(|b| key.open(name, &b)).transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn db_contains_ciphertext_only() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("key");
        crate::secrets::write_private_new(&p, &[8; 32]).unwrap();
        let k = Secrets::load(&p).unwrap();
        let db = d.path().join("private.db");
        let pool = open(&db).await.unwrap();
        put_secret(&pool, &k, "envelope_sk", b"unique-secret-string")
            .await
            .unwrap();
        assert_eq!(
            &**get_secret(&pool, &k, "envelope_sk")
                .await
                .unwrap()
                .as_ref()
                .unwrap(),
            b"unique-secret-string"
        );
        pool.close().await;
        assert!(
            !std::fs::read(db)
                .unwrap()
                .windows(20)
                .any(|s| s == b"unique-secret-string")
        );
    }
}

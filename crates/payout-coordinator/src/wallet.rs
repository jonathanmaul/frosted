//! Stateless pre-release wallet process. No secret is placed in argv, environment or disk.
use crate::config::Config;
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{process::Stdio, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
};

pub async fn call(config: &Config, mut request: Value) -> Result<Value> {
    request["network"] = json!(config.network);
    request["rpc_url"] = json!(config.rpc_url);
    request["zaino_url"] = json!(config.zaino_url);
    let mut child = Command::new(&config.helper)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .context("wallet helper unavailable")?;
    let bytes = zeroize::Zeroizing::new(serde_json::to_vec(&request)?);
    let mut stdin = child.stdin.take().context("wallet helper stdin")?;
    stdin.write_all(&bytes).await?;
    drop(stdin);
    let mut stdout = child.stdout.take().context("wallet helper stdout")?;
    let result = tokio::time::timeout(Duration::from_secs(600), async {
        let mut bytes = zeroize::Zeroizing::new(Vec::new());
        stdout.read_to_end(&mut bytes).await?;
        if !child.wait().await?.success() {
            bail!("wallet helper failed");
        }
        serde_json::from_slice(&bytes).context("wallet helper invalid response")
    })
    .await
    .context("wallet helper timeout")??;
    Ok(result)
}

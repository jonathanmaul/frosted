use frosted_api::{AppState, Config, router};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // No HTTP trace layer: request headers, peer addresses and bodies are never logged.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("LOG_LEVEL")
                .unwrap_or_else(|_| "info".into()),
        )
        .init();
    let config = Config::from_env()?;
    let bind = config.bind;
    let state = AppState::open(config).await?;
    let refresher = state.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            tick.tick().await;
            if let Err(e) = frosted_api::refresh_thaw_reports(&refresher).await {
                tracing::warn!("thaw report refresh failed: {e}");
            }
        }
    });
    let listener = tokio::net::TcpListener::bind(bind).await?;
    tracing::info!("Frosted API ready");
    axum::serve(listener, router(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}

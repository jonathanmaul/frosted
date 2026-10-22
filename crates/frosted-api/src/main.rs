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
    let listener = tokio::net::TcpListener::bind(bind).await?;
    tracing::info!("Frosted API ready");
    axum::serve(listener, router(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}

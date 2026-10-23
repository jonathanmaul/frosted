use anyhow::Result;
#[tokio::main]
async fn main() -> Result<()> {
    let config = payout_coordinator::config::Config::from_env()?;
    let bind = config.bind;
    let listener = tokio::net::TcpListener::bind(bind).await?;
    let app = payout_coordinator::service::App::new(config).await?;
    tokio::spawn(app.clone().work());
    axum::serve(listener, payout_coordinator::service::router(app))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}

//! Public read-only REST/SSE handlers and the coordinator's authenticated write boundary.

mod auth;
pub mod config;
mod error;
mod ingest;
mod public;
mod sse;
mod thaw;
pub mod types;
mod verify;

pub use thaw::refresh_all as refresh_thaw_reports;

use std::time::Duration;

use axum::{
    Router,
    http::{Method, header},
    routing::{get, post},
};
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use tower_http::cors::CorsLayer;

pub use config::Config;

#[derive(Clone)]
pub struct AppState {
    pub(crate) read: SqlitePool,
    pub(crate) write: SqlitePool,
    pub(crate) config: Config,
    pub(crate) http: reqwest::Client,
    pub(crate) verify: verify::Verifier,
    /// Last chain tip and when it was read; every page polls the tip, so the node is asked at most once per
    /// TIP_FRESH_FOR however many visitors there are.
    /// Live SSE streams each poll the database; past this many, new streams get 503 (SSE_MAX_STREAMS).
    pub(crate) sse_slots: std::sync::Arc<tokio::sync::Semaphore>,
    pub(crate) tip_cache:
        std::sync::Arc<tokio::sync::Mutex<Option<(std::time::Instant, serde_json::Value)>>>,
}
impl AppState {
    pub async fn open(config: Config) -> anyhow::Result<Self> {
        let options = SqliteConnectOptions::new()
            .filename(&config.public_db_path)
            .create_if_missing(false)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5));
        let write = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options.clone())
            .await?;
        sqlx::raw_sql(include_str!("../schema.sql"))
            .execute(&write)
            .await?;
        let read = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options.read_only(true))
            .await?;
        let http = config.rpc_client()?;
        let verify = verify::Verifier::new(&config.zebra_rpc_url)?;
        Ok(Self {
            read,
            write,
            config,
            http,
            verify,
            tip_cache: Default::default(),
            sse_slots: std::sync::Arc::new(tokio::sync::Semaphore::new(
                std::env::var("SSE_MAX_STREAMS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(256),
            )),
        })
    }
}

pub fn router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(state.config.public_origin.clone())
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([
            header::HeaderName::from_static("last-event-id"),
            header::CONTENT_TYPE,
        ]);
    let public = Router::new()
        .route("/v1/health", get(public::health))
        .route("/v1/chain/tip", get(public::tip))
        .route("/v1/markets", get(public::markets))
        .route("/v1/markets/{id}", get(public::get_market))
        .route("/v1/markets/{id}/odds", get(public::get_odds))
        .route("/v1/markets/{id}/bets", get(public::bets))
        .route("/v1/markets/{id}/resolution", get(public::resolution))
        .route("/v1/markets/{id}/payout", get(public::payout))
        .route("/v1/markets/{id}/payouts", get(public::payouts))
        .route("/v1/markets/{id}/verify", get(verify::verify))
        .route("/v1/markets/{id}/stream", get(sse::events))
        .route("/v1/personas/registry", get(public::registry))
        .route("/v1/personas/{id}", get(public::persona))
        .route("/v1/personas/{id}/thaw", post(thaw::thaw))
        .route("/v1/personas/{id}/report", get(thaw::report))
        .route("/v1/leaderboard", get(public::leaderboard))
        .layer(cors);
    let ingest = Router::new()
        .route("/v1/ingest/markets", post(ingest::market))
        .route(
            "/v1/ingest/markets/{id}/resolution",
            post(ingest::resolution),
        )
        .route("/v1/ingest/markets/{id}/payout", post(ingest::payout));
    public
        .merge(ingest)
        .with_state(state)
        .layer(axum::middleware::map_response(json_errors))
}

#[cfg(test)]
mod tests;

async fn json_errors(mut response: axum::response::Response) -> axum::response::Response {
    use axum::response::IntoResponse;
    if (response.status().is_client_error() || response.status().is_server_error())
        && response
            .headers()
            .get(header::CONTENT_TYPE)
            .is_none_or(|v| v != "application/json")
    {
        let mapped = error::ApiError(response.status(), "INTERNAL", "Invalid request".into())
            .into_response();
        *response.body_mut() = mapped.into_body();
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/json"),
        );
        response.headers_mut().remove(header::CONTENT_LENGTH);
    }
    response
}

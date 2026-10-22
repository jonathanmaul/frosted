use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;

#[derive(Debug)]
pub struct ApiError(pub StatusCode, pub &'static str, pub String);

impl ApiError {
    pub(crate) fn bad(message: impl Into<String>) -> Self {
        Self(StatusCode::BAD_REQUEST, "INTERNAL", message.into())
    }
    pub(crate) fn market() -> Self {
        Self(
            StatusCode::NOT_FOUND,
            "MARKET_NOT_FOUND",
            "Market does not exist".into(),
        )
    }
    pub(crate) fn unauthorized() -> Self {
        Self(
            StatusCode::UNAUTHORIZED,
            "UNAUTHORIZED",
            "Invalid coordinator signature".into(),
        )
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.0,
            Json(json!({"error": {"code": self.1, "message": self.2}})),
        )
            .into_response()
    }
}
impl From<sqlx::Error> for ApiError {
    fn from(_: sqlx::Error) -> Self {
        Self(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Database operation failed".into(),
        )
    }
}
impl From<serde_json::Error> for ApiError {
    fn from(_: serde_json::Error) -> Self {
        Self::bad("Invalid JSON or schema")
    }
}
pub(crate) type Result<T> = std::result::Result<T, ApiError>;

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.1, self.2)
    }
}
impl std::error::Error for ApiError {}

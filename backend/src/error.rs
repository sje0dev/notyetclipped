use actix_web::{HttpResponse, ResponseError, http::StatusCode};
use serde_json::json;

#[derive(Debug)]
pub struct ApiError(pub StatusCode, pub String);
impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.1)
    }
}
impl std::error::Error for ApiError {}
impl ResponseError for ApiError {
    fn status_code(&self) -> StatusCode {
        self.0
    }
    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.0).json(json!({"detail": self.1}))
    }
}
impl From<sqlx::Error> for ApiError {
    fn from(error: sqlx::Error) -> Self {
        if let sqlx::Error::Database(ref database) = error
            && database.is_unique_violation()
        {
            return Self(
                StatusCode::CONFLICT,
                "Dieser Eintrag existiert bereits.".into(),
            );
        }
        tracing::error!(%error, "Database request failed");
        Self(
            StatusCode::SERVICE_UNAVAILABLE,
            "PostgreSQL ist nicht erreichbar oder noch nicht migriert.".into(),
        )
    }
}
impl From<anyhow::Error> for ApiError {
    fn from(error: anyhow::Error) -> Self {
        tracing::error!(%error, "Request failed");
        Self(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Die Anfrage konnte nicht abgeschlossen werden.".into(),
        )
    }
}
pub fn invalid(message: impl Into<String>) -> ApiError {
    ApiError(StatusCode::UNPROCESSABLE_ENTITY, message.into())
}
pub fn conflict(message: impl Into<String>) -> ApiError {
    ApiError(StatusCode::CONFLICT, message.into())
}
pub fn missing() -> ApiError {
    ApiError(StatusCode::NOT_FOUND, "Eintrag nicht gefunden.".into())
}

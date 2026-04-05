use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Failed to fetch URL: {0}")]
    FetchError(String),

    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    #[error("Timeout fetching URL")]
    Timeout,

    #[error("Content too large: {0} bytes")]
    ContentTooLarge(usize),

    #[error("Blocked by robots.txt or anti-bot protection")]
    Blocked,

    #[error("LLM extraction error: {0}")]
    ExtractionError(String),

    #[error("LLM not configured: set ANTHROPIC_API_KEY")]
    LlmNotConfigured,

    #[error("Unauthorized: invalid or missing API key")]
    Unauthorized,

    #[error("Rate limited: try again later")]
    RateLimited,

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

#[derive(Serialize)]
struct ErrorResponse {
    success: bool,
    error: String,
    code: u16,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::FetchError(msg) => {
                tracing::warn!(error = %msg, "scrape.fetch_error");
                (StatusCode::BAD_GATEWAY, msg.clone())
            }
            AppError::InvalidUrl(msg) => {
                tracing::warn!(error = %msg, "scrape.invalid_url");
                (StatusCode::BAD_REQUEST, msg.clone())
            }
            AppError::Timeout => {
                tracing::warn!("scrape.timeout");
                (StatusCode::GATEWAY_TIMEOUT, self.to_string())
            }
            AppError::ContentTooLarge(size) => {
                tracing::warn!(size, "scrape.content_too_large");
                (StatusCode::PAYLOAD_TOO_LARGE, self.to_string())
            }
            AppError::Blocked => {
                tracing::info!("scrape.blocked");
                (StatusCode::FORBIDDEN, self.to_string())
            }
            AppError::ExtractionError(msg) => {
                tracing::error!(error = %msg, "extract.error");
                (StatusCode::UNPROCESSABLE_ENTITY, msg.clone())
            }
            AppError::LlmNotConfigured => {
                tracing::error!("extract.llm_not_configured");
                (StatusCode::SERVICE_UNAVAILABLE, self.to_string())
            }
            AppError::Unauthorized => {
                tracing::warn!("auth.unauthorized");
                (StatusCode::UNAUTHORIZED, self.to_string())
            }
            AppError::RateLimited => {
                tracing::info!("rate_limit.exceeded");
                (StatusCode::TOO_MANY_REQUESTS, self.to_string())
            }
            AppError::Config(msg) => {
                tracing::error!(error = %msg, "config.error");
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal configuration error".into())
            }
            AppError::Internal(msg) => {
                tracing::error!(error = %msg, "internal.error");
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".into())
            }
        };

        let body = ErrorResponse {
            success: false,
            error: message,
            code: status.as_u16(),
        };

        (status, axum::Json(body)).into_response()
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_timeout() {
            AppError::Timeout
        } else {
            AppError::FetchError(e.to_string())
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Internal(format!("Serialization error: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_messages() {
        assert_eq!(
            AppError::InvalidUrl("bad".into()).to_string(),
            "Invalid URL: bad"
        );
        assert_eq!(
            AppError::Timeout.to_string(),
            "Timeout fetching URL"
        );
        assert_eq!(
            AppError::ContentTooLarge(5000).to_string(),
            "Content too large: 5000 bytes"
        );
        assert_eq!(
            AppError::Blocked.to_string(),
            "Blocked by robots.txt or anti-bot protection"
        );
        assert_eq!(
            AppError::LlmNotConfigured.to_string(),
            "LLM not configured: set ANTHROPIC_API_KEY"
        );
        assert_eq!(
            AppError::Unauthorized.to_string(),
            "Unauthorized: invalid or missing API key"
        );
        assert_eq!(
            AppError::RateLimited.to_string(),
            "Rate limited: try again later"
        );
    }

    #[test]
    fn error_from_serde_json() {
        let json_err: Result<serde_json::Value, _> = serde_json::from_str("not json");
        let app_err: AppError = json_err.unwrap_err().into();
        match app_err {
            AppError::Internal(msg) => assert!(msg.contains("Serialization")),
            _ => panic!("Expected Internal variant"),
        }
    }

    #[test]
    fn internal_error_hides_details_in_response() {
        // Internal errors should not leak implementation details to client
        let err = AppError::Internal("database connection pool exhausted".into());
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn config_error_hides_details_in_response() {
        let err = AppError::Config("missing secret key".into());
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn error_status_codes() {
        assert_eq!(
            AppError::InvalidUrl("x".into()).into_response().status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            AppError::Timeout.into_response().status(),
            StatusCode::GATEWAY_TIMEOUT
        );
        assert_eq!(
            AppError::ContentTooLarge(100).into_response().status(),
            StatusCode::PAYLOAD_TOO_LARGE
        );
        assert_eq!(
            AppError::Blocked.into_response().status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            AppError::Unauthorized.into_response().status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            AppError::RateLimited.into_response().status(),
            StatusCode::TOO_MANY_REQUESTS
        );
        assert_eq!(
            AppError::LlmNotConfigured.into_response().status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(
            AppError::ExtractionError("x".into()).into_response().status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
        assert_eq!(
            AppError::FetchError("x".into()).into_response().status(),
            StatusCode::BAD_GATEWAY
        );
    }
}

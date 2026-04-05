use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::AppError;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct ExtractRequest {
    pub url: String,
    pub schema: Option<Value>,
    pub prompt: Option<String>,
}

#[derive(Serialize)]
pub struct ExtractResponse {
    pub success: bool,
    pub data: Value,
    pub url: String,
    pub elapsed_ms: u64,
}

pub async fn extract_data(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ExtractRequest>,
) -> Result<Json<ExtractResponse>, AppError> {
    if payload.url.is_empty() {
        return Err(AppError::InvalidUrl("URL is required".into()));
    }

    if payload.schema.is_none() && payload.prompt.is_none() {
        return Err(AppError::ExtractionError(
            "Either 'schema' or 'prompt' is required for extraction".into(),
        ));
    }

    let start = std::time::Instant::now();

    let scrape_result = state.scraper.scrape(&payload.url, false).await?;

    let extracted = state
        .extractor
        .extract(
            &scrape_result.markdown,
            payload.schema.as_ref(),
            payload.prompt.as_deref(),
        )
        .await?;

    let elapsed_ms = start.elapsed().as_millis() as u64;

    tracing::info!(
        url = %payload.url,
        elapsed_ms,
        "extract.complete"
    );

    Ok(Json(ExtractResponse {
        success: true,
        data: extracted,
        url: payload.url,
        elapsed_ms,
    }))
}

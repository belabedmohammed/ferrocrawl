use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::scraper::ScrapeResult;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct ScrapeRequest {
    pub url: String,
    #[serde(default)]
    pub include_raw_html: bool,
    #[serde(default)]
    pub formats: Vec<String>,
}

#[derive(Serialize)]
pub struct ScrapeResponse {
    pub success: bool,
    pub data: ScrapeResult,
}

pub async fn scrape_url(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ScrapeRequest>,
) -> Result<Json<ScrapeResponse>, AppError> {
    if payload.url.is_empty() {
        return Err(AppError::InvalidUrl("URL is required".into()));
    }

    let include_raw = payload.include_raw_html
        || payload.formats.iter().any(|f| f == "rawHtml" || f == "html");

    let result = state.scraper.scrape(&payload.url, include_raw).await?;

    tracing::info!(
        url = %payload.url,
        status = result.status_code,
        elapsed_ms = result.elapsed_ms,
        words = result.metadata.word_count,
        "scrape.complete"
    );

    Ok(Json(ScrapeResponse {
        success: true,
        data: result,
    }))
}

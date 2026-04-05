pub mod config;
pub mod error;
pub mod extractor;
pub mod routes;
pub mod scraper;

use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::{get, post};
use axum::Router;
use tower_http::cors::{AllowHeaders, AllowMethods, AllowOrigin, CorsLayer};
use tower_http::trace::TraceLayer;

use config::Config;
use extractor::LlmExtractor;
use scraper::StaticScraper;

/// Central application state, passed to all handlers via Arc.
pub struct AppState {
    pub config: Arc<Config>,
    pub scraper: StaticScraper,
    pub extractor: LlmExtractor,
}

/// Initialize tracing with JSON output and env-based filtering.
pub fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ferrocrawl=info,tower_http=info".into()),
        )
        .json()
        .with_ansi(false)
        .init();
}

/// Build shared application state from config.
pub fn build_state(config: Arc<Config>) -> Arc<AppState> {
    let scraper = StaticScraper::new(&config).expect("Failed to initialize scraper");
    let extractor = LlmExtractor::new(&config.anthropic);

    Arc::new(AppState {
        config,
        scraper,
        extractor,
    })
}

/// Build the Axum router with all routes and middleware.
pub fn build_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::any())
        .allow_methods(AllowMethods::any())
        .allow_headers(AllowHeaders::list([
            "content-type".parse().unwrap(),
            "authorization".parse().unwrap(),
        ]));

    Router::new()
        .route("/health", get(routes::health_check))
        .route("/v1/scrape", post(routes::scrape_url))
        .route("/v1/extract", post(routes::extract_data))
        .layer(middleware::from_fn(security_headers))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}

async fn security_headers(
    request: axum::extract::Request,
    next: Next,
) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    headers.insert(
        "strict-transport-security",
        HeaderValue::from_static("max-age=63072000; includeSubDomains; preload"),
    );
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        "x-frame-options",
        HeaderValue::from_static("DENY"),
    );

    response
}

async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    request: axum::extract::Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if request.uri().path() == "/health" {
        return Ok(next.run(request).await);
    }

    if !state.config.auth.enabled() {
        return Ok(next.run(request).await);
    }

    let auth_header = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let token = auth_header.strip_prefix("Bearer ").unwrap_or(auth_header);

    if state.config.auth.api_keys.contains(&token.to_string()) {
        Ok(next.run(request).await)
    } else {
        tracing::warn!("auth.rejected");
        Err(StatusCode::UNAUTHORIZED)
    }
}

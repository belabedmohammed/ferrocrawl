use std::sync::Arc;

use ferrocrawl::config::Config;
use ferrocrawl::{build_router, build_state, init_tracing};

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();

    init_tracing();

    let config = Arc::new(Config::from_env().expect("Failed to load configuration"));
    tracing::info!(config = ?config, "config.loaded");

    let state = build_state(config.clone());
    let app = build_router(state);

    let addr = format!("{}:{}", config.server.host, config.server.port);
    tracing::info!(
        addr = %addr,
        auth = config.auth.enabled(),
        "ferrocrawl.local.starting"
    );

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind address");

    axum::serve(listener, app).await.expect("Server error");
}

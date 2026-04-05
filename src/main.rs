use std::sync::Arc;

use ferrocrawl::config::Config;
use ferrocrawl::{build_router, build_state, init_tracing};
use lambda_http::Error;

#[tokio::main]
async fn main() -> Result<(), Error> {
    init_tracing();

    let config = Arc::new(Config::from_env().expect("Failed to load configuration"));
    tracing::info!(config = ?config, "config.loaded");

    let state = build_state(config);
    let app = build_router(state);

    tracing::info!("ferrocrawl.lambda.starting");
    lambda_http::run(app).await
}

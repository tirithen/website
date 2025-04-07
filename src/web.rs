use anyhow::Result;
use axum::{
    Json, Router,
    extract::{Path, Query},
    http::StatusCode,
    routing::get,
};
use serde::{Deserialize, Serialize};

use crate::config::load_config;

#[derive(Deserialize, Debug)]
struct QueryParams {
    mode: Option<Mode>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all(deserialize = "lowercase"))]
enum Mode {
    Partial,
    Edit,
}

pub async fn start_server() -> Result<()> {
    let config = load_config();
    let app = Router::new()
        .route("/", get(catch_all_handler))
        .route("/{*path}", get(catch_all_handler));
    let address = format!("0.0.0.0:{}", config.port());
    let listener = tokio::net::TcpListener::bind(&address).await?;

    tracing::info!("🚀 Starting website server at: http://{address}");
    axum::serve(listener, app).await?;

    Ok(())
}

async fn catch_all_handler(path: Option<Path<String>>, Query(query): Query<QueryParams>) -> String {
    format!("Caught path: {:?}, Query parameters: {:?}", path, query)
}

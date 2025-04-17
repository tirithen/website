use anyhow::Result;
use config::load_config;
use logger::init_logging;
use search::spawn_search_indexer;
use web::start_server;

mod assets;
mod config;
mod error_handler;
mod logger;
mod page;
mod search;
mod security;
mod web;

#[tokio::main]
async fn main() -> Result<()> {
    let config = load_config();
    init_logging(&config)?;
    let (search_index, _debouncer, _watcher) = spawn_search_indexer(&config).await?;
    start_server(&config, search_index).await?;
    Ok(())
}

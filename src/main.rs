use anyhow::Result;
use config::load_config;
use logger::init_logging;
use web::start_server;

mod assets;
mod config;
mod logger;
mod page;
mod security;
mod web;

#[tokio::main]
async fn main() -> Result<()> {
    let config = load_config();
    init_logging(&config)?;
    start_server().await?;
    Ok(())
}

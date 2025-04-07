use anyhow::Result;
use cli::Cli;
use config::load_config;
use logger::init_logging;

mod cli;
mod config;
mod logger;
mod page;
mod web;

fn main() -> Result<()> {
    let config = load_config();
    init_logging(&config)?;

    Ok(())
}

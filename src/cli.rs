use anyhow::Result;
use clap::Parser;

use crate::web::start_server;

/// Website server command-line interface
#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about,
    long_about = "Simple to use. High-performance website server with Markdown support",
    propagate_version = true
)]
pub struct Cli {
    
};

impl Cli {
    /// Parse command-line arguments with Clap
    pub fn parse_args() -> Self {
        Self::parse()
    }

    /// Start web server
    pub async fn start(&self) -> Result<()> {
        tracing::info!("🚀 Starting website server in production mode...");
        start_server().await
    }
}

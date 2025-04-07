use clap::Parser;

/// website server command-line interface
#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about,
    long_about = "High-performance personal blog server with Markdown support",
    propagate_version = true
)]
pub struct Cli;

impl Cli {
    /// Parse command-line arguments with Clap
    pub fn parse_args() -> Self {
        Self::parse()
    }

    /// Start production server with configured parameters
    pub fn start_production_server(&self) {
        tracing::info!("🚀 Starting website server in production mode...");
        // Implementation will integrate with Config::load()
    }
}

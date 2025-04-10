use crate::config::Config;
use anyhow::Result;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{filter::EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

pub fn init_logging(config: &Config) -> Result<()> {
    let log_path = config.log_path();
    std::fs::create_dir_all(log_path)?;

    let stdout_log = fmt::layer()
        .with_target(true)
        .with_level(true)
        .with_ansi(atty::is(atty::Stream::Stdout));

    let file_log = fmt::layer()
        .with_ansi(false)
        .with_writer(tracing_appender::rolling::daily(
            config.log_path(),
            "website.log",
        ));

    let log_level = (*config.log_level()).into();
    let level_filter = LevelFilter::from_level(log_level).into();

    tracing_subscriber::registry()
        .with(
            EnvFilter::builder()
                .with_default_directive(level_filter)
                .from_env_lossy(),
        )
        .with(stdout_log)
        .with(file_log)
        .init();

    Ok(())
}

use std::{path::PathBuf, str::FromStr};

use cached::proc_macro::cached;
use derive_getters::Getters;
use serde::{Deserialize, Serialize};
use tracing::Level;

#[cached]
pub fn load_config() -> Config {
    let config_path = dirs::config_local_dir()
        .unwrap_or(PathBuf::from_str("./config").unwrap())
        .join("website/config.toml");

    println!(
        "🔧 Loading config from {} (if exists)",
        config_path.to_string_lossy()
    );

    let mut config = None;
    if let Ok(data) = std::fs::read_to_string(config_path) {
        match toml::from_str::<ConfigParsed>(&data) {
            Ok(parsed) => {
                config = Some(Config::from(parsed));
            }
            Err(error) => {
                eprintln!("💥 Failed to parse config: {error}");
            }
        };
    }

    if config.is_none() {
        config = Some(Config::from(ConfigParsed::default()));
        eprintln!("⚠️ Unable to find config file, falling back on default config");
    }

    config.unwrap()
}

#[derive(Default, Serialize, Deserialize)]
pub struct ConfigParsed {
    title: Option<String>,
    port: Option<u16>,
    data_path: Option<PathBuf>,
    log_level: Option<ConfigLogLevel>,
}

#[derive(Clone, Getters, Serialize)]
pub struct Config {
    title: String,
    port: u16,
    data_path: PathBuf,
    log_level: ConfigLogLevel,
}

impl Config {
    pub fn log_path(&self) -> PathBuf {
        self.data_path.join("logs")
    }
}

impl From<ConfigParsed> for Config {
    fn from(value: ConfigParsed) -> Self {
        Self {
            title: value.title.unwrap_or("Welcome".into()),
            port: 4000,
            data_path: value.data_path.unwrap_or(
                dirs::data_local_dir()
                    .unwrap_or(PathBuf::from_str("./data").unwrap())
                    .join("website/"),
            ),
            log_level: ConfigLogLevel::Info,
        }
    }
}

#[repr(usize)]
#[derive(Default, Copy, Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum ConfigLogLevel {
    #[default]
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl From<Level> for ConfigLogLevel {
    fn from(value: Level) -> Self {
        match value {
            Level::ERROR => ConfigLogLevel::Error,
            Level::WARN => ConfigLogLevel::Warn,
            Level::INFO => ConfigLogLevel::Info,
            Level::DEBUG => ConfigLogLevel::Debug,
            Level::TRACE => ConfigLogLevel::Trace,
        }
    }
}

impl From<ConfigLogLevel> for Level {
    fn from(value: ConfigLogLevel) -> Self {
        match value {
            ConfigLogLevel::Error => Level::ERROR,
            ConfigLogLevel::Warn => Level::WARN,
            ConfigLogLevel::Info => Level::INFO,
            ConfigLogLevel::Debug => Level::DEBUG,
            ConfigLogLevel::Trace => Level::TRACE,
        }
    }
}

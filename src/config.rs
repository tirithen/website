use std::{path::PathBuf, str::FromStr, time::Duration};

use cached::proc_macro::cached;
use derive_getters::Getters;
use duration_str::deserialize_duration;
use serde::{Deserialize, Deserializer, Serialize};
use tracing::Level;

#[cached]
pub fn load_config() -> Config {
    let config_path = dirs::config_local_dir()
        .unwrap_or(PathBuf::from_str("./config").unwrap())
        .join("website/config.toml");

    print!(
        "🔧 Loading config from {} (if exists)...",
        config_path.to_string_lossy()
    );

    let mut config = None;
    if let Ok(data) = std::fs::read_to_string(config_path) {
        match toml::from_str::<ConfigParsed>(&data) {
            Ok(parsed) => {
                println!(" found and loaded!");
                config = Some(Config::from(parsed));
            }
            Err(error) => {
                println!();
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
    #[serde(
        default,
        deserialize_with = "deserialize_option_duration",
        skip_serializing_if = "Option::is_none"
    )]
    search_reindex_interval: Option<Duration>,
}

fn deserialize_option_duration<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    s.map(|s| duration_str::parse(&s).map_err(serde::de::Error::custom))
        .transpose()
}

#[derive(Clone, Getters, Serialize)]
pub struct Config {
    title: String,
    port: u16,
    data_path: PathBuf,
    log_level: ConfigLogLevel,
    search_reindex_interval: Duration,
}

impl Config {
    pub fn log_path(&self) -> PathBuf {
        self.data_path.join("logs")
    }

    pub fn pages_path(&self) -> PathBuf {
        self.data_path.join("pages")
    }

    pub fn search_path(&self) -> PathBuf {
        self.data_path.join("search")
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
            log_level: value.log_level.unwrap_or(ConfigLogLevel::Info),
            search_reindex_interval: value
                .search_reindex_interval
                .unwrap_or(Duration::from_secs(30 * 60)),
        }
    }
}

#[repr(usize)]
#[derive(Default, Copy, Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
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

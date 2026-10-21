// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Runtime configuration, read from environment variables and an optional file.
//!
//! Every setting has a default, so the server starts with no configuration at
//! all. The API key is the one exception that may be left unset: the server
//! then generates one on first start (see [`crate::apikey`]).
//!
//! A TOML file named by `TPT_RUC_CONFIG` can hold the same settings. Each key
//! is the environment variable's name, for example `TPT_RUC_BIND = "..."`, and
//! a list such as `TPT_RUC_CORS_ORIGINS` may be written as a TOML array. An
//! environment variable always wins over the file.

use std::fmt;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

/// Shortest API key accepted. Shorter keys are easy to guess over a network.
pub const MIN_API_KEY_LEN: usize = 32;

/// Where the server listens when `TPT_RUC_BIND` is not set. Loopback only, so
/// nothing outside the machine can connect until the owner opts in.
pub const DEFAULT_BIND: &str = "127.0.0.1:8080";
/// Database used when `TPT_RUC_DATABASE_URL` is not set. Relative to the working directory.
pub const DEFAULT_DATABASE_URL: &str = "sqlite://data/tpt-ruc.db";

/// Settings the server needs to start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Address to listen on.
    pub bind: SocketAddr,
    /// SQLite connection URL, for example `sqlite://data/tpt-ruc.db`.
    pub database_url: String,
    /// The owner's API key from `TPT_RUC_API_KEY`. `None` means the server
    /// should load or generate one from the data folder.
    pub api_key: Option<String>,
    /// Origins allowed to call the API from a browser, comma-separated in `TPT_RUC_CORS_ORIGINS`.
    pub cors_origins: Vec<String>,
    /// Trips older than this many days are deleted. `None` keeps trips forever.
    pub retention_days: Option<u32>,
    /// Folder of the built web app, from `TPT_RUC_PWA_DIR`. `None` serves no pages.
    pub pwa_dir: Option<PathBuf>,
    /// The eRUC push provider named in `TPT_RUC_ERUC_PROVIDER`. `None` sends nothing.
    pub eruc_provider: Option<String>,
    /// Whether users may upload raw GPS points, from `TPT_RUC_ALLOW_RAW_GPS`. Off by default.
    pub allow_raw_gps: bool,
}

/// Why the configuration cannot be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// The API key set in `TPT_RUC_API_KEY` is shorter than [`MIN_API_KEY_LEN`].
    ShortApiKey,
    /// `TPT_RUC_BIND` is not a socket address.
    InvalidBind(String),
    /// `TPT_RUC_RETENTION_DAYS` is not a whole number of at least 1.
    InvalidRetention(String),
    /// A switch such as `TPT_RUC_ALLOW_RAW_GPS` is not `true` or `false`. Holds the name and the value.
    InvalidFlag(String, String),
    /// The file named by `TPT_RUC_CONFIG` cannot be read or parsed, or holds a value of the wrong kind.
    File(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ShortApiKey => write!(
                f,
                "TPT_RUC_API_KEY must be at least {MIN_API_KEY_LEN} characters, or unset to generate one"
            ),
            Self::InvalidBind(value) => write!(f, "TPT_RUC_BIND is not an address: {value}"),
            Self::InvalidRetention(value) => write!(
                f,
                "TPT_RUC_RETENTION_DAYS must be a whole number of days, at least 1, not {value}"
            ),
            Self::InvalidFlag(name, value) => {
                write!(f, "{name} must be true or false, not {value}")
            }
            Self::File(message) => write!(f, "configuration file: {message}"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl Config {
    /// Reads the configuration from the process environment and the file named by `TPT_RUC_CONFIG`, if any.
    pub fn from_env() -> Result<Self, ConfigError> {
        let file = match std::env::var("TPT_RUC_CONFIG")
            .ok()
            .filter(|path| !path.trim().is_empty())
        {
            Some(path) => Some(read_file(Path::new(path.trim()))?),
            None => None,
        };
        Self::from_lookup(|name| {
            std::env::var(name)
                .ok()
                .or_else(|| file.as_ref().and_then(|values| values.get(name).cloned()))
        })
    }

    /// Builds the configuration from any name lookup. Tests use this instead of the environment.
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let api_key = get("TPT_RUC_API_KEY")
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        if let Some(key) = &api_key {
            if key.len() < MIN_API_KEY_LEN {
                return Err(ConfigError::ShortApiKey);
            }
        }

        let bind_text = get("TPT_RUC_BIND").unwrap_or_else(|| DEFAULT_BIND.to_owned());
        let bind = bind_text
            .trim()
            .parse()
            .map_err(|_| ConfigError::InvalidBind(bind_text.clone()))?;

        let database_url =
            get("TPT_RUC_DATABASE_URL").unwrap_or_else(|| DEFAULT_DATABASE_URL.to_owned());

        let cors_origins = get("TPT_RUC_CORS_ORIGINS")
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|origin| !origin.is_empty())
            .map(str::to_owned)
            .collect();

        let retention_days = match get("TPT_RUC_RETENTION_DAYS").map(|v| v.trim().to_owned()) {
            None => None,
            Some(text) if text.is_empty() => None,
            Some(text) => match text.parse::<u32>() {
                Ok(days) if days >= 1 => Some(days),
                _ => return Err(ConfigError::InvalidRetention(text)),
            },
        };

        let eruc_provider = get("TPT_RUC_ERUC_PROVIDER")
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());

        let pwa_dir = get("TPT_RUC_PWA_DIR")
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);

        let allow_raw_gps = match get("TPT_RUC_ALLOW_RAW_GPS").map(|v| v.trim().to_owned()) {
            None => false,
            Some(text) if text.is_empty() => false,
            Some(text) if text.eq_ignore_ascii_case("true") => true,
            Some(text) if text.eq_ignore_ascii_case("false") => false,
            Some(text) => {
                return Err(ConfigError::InvalidFlag(
                    "TPT_RUC_ALLOW_RAW_GPS".to_owned(),
                    text,
                ))
            }
        };

        Ok(Self {
            bind,
            database_url,
            api_key,
            cors_origins,
            retention_days,
            pwa_dir,
            eruc_provider,
            allow_raw_gps,
        })
    }

    /// The folder that holds the database file. The API key file also lives here.
    pub fn data_dir(&self) -> PathBuf {
        data_dir_of(&self.database_url)
    }
}

/// Reads a configuration file into name/value pairs. Lists are joined with commas,
/// so they read the same as the environment variable form.
pub fn read_file(path: &Path) -> Result<std::collections::HashMap<String, String>, ConfigError> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| ConfigError::File(format!("cannot read {}: {error}", path.display())))?;
    parse_file(&text)
}

/// Parses the text of a configuration file. See [`read_file`].
pub fn parse_file(text: &str) -> Result<std::collections::HashMap<String, String>, ConfigError> {
    let table: toml::Table = text
        .parse()
        .map_err(|error: toml::de::Error| ConfigError::File(error.to_string()))?;
    let mut values = std::collections::HashMap::new();
    for (name, value) in table {
        let text = match value {
            toml::Value::String(text) => text,
            toml::Value::Integer(number) => number.to_string(),
            toml::Value::Boolean(flag) => flag.to_string(),
            toml::Value::Array(items) => {
                let mut parts = Vec::with_capacity(items.len());
                for item in items {
                    match item {
                        toml::Value::String(text) => parts.push(text),
                        _ => {
                            return Err(ConfigError::File(format!(
                                "{name} may hold only strings in a list"
                            )))
                        }
                    }
                }
                parts.join(",")
            }
            _ => {
                return Err(ConfigError::File(format!(
                    "{name} must be a string, number, boolean or list of strings"
                )))
            }
        };
        values.insert(name, text);
    }
    Ok(values)
}

/// The folder containing the SQLite file named by `url`. Falls back to the working directory.
pub fn data_dir_of(url: &str) -> PathBuf {
    let path = Path::new(url.strip_prefix("sqlite://").unwrap_or(url));
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "0123456789abcdef0123456789abcdef";

    fn lookup<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        }
    }

    #[test]
    fn no_configuration_at_all_is_valid() {
        let config = Config::from_lookup(lookup(&[])).unwrap();
        assert_eq!(config.bind, "127.0.0.1:8080".parse().unwrap());
        assert_eq!(config.database_url, "sqlite://data/tpt-ruc.db");
        assert_eq!(config.api_key, None);
        assert!(config.cors_origins.is_empty());
    }

    #[test]
    fn blank_key_means_unset() {
        let config = Config::from_lookup(lookup(&[("TPT_RUC_API_KEY", "   ")])).unwrap();
        assert_eq!(config.api_key, None);
    }

    #[test]
    fn short_key_is_refused() {
        assert_eq!(
            Config::from_lookup(lookup(&[("TPT_RUC_API_KEY", "short")])),
            Err(ConfigError::ShortApiKey)
        );
    }

    #[test]
    fn invalid_bind_is_refused() {
        let result = Config::from_lookup(lookup(&[
            ("TPT_RUC_API_KEY", KEY),
            ("TPT_RUC_BIND", "not-an-address"),
        ]));
        assert_eq!(
            result,
            Err(ConfigError::InvalidBind("not-an-address".to_owned()))
        );
    }

    #[test]
    fn cors_origins_are_split_and_trimmed() {
        let config = Config::from_lookup(lookup(&[
            ("TPT_RUC_API_KEY", KEY),
            (
                "TPT_RUC_CORS_ORIGINS",
                " https://a.example , https://b.example,",
            ),
        ]))
        .unwrap();
        assert_eq!(
            config.cors_origins,
            vec!["https://a.example", "https://b.example"]
        );
    }

    #[test]
    fn retention_is_optional_and_must_be_a_positive_whole_number() {
        let unset = Config::from_lookup(lookup(&[])).unwrap();
        assert_eq!(unset.retention_days, None);
        let set = Config::from_lookup(lookup(&[("TPT_RUC_RETENTION_DAYS", "365")])).unwrap();
        assert_eq!(set.retention_days, Some(365));
        for bad in ["0", "-3", "forever"] {
            assert!(matches!(
                Config::from_lookup(lookup(&[("TPT_RUC_RETENTION_DAYS", bad)])),
                Err(ConfigError::InvalidRetention(_))
            ));
        }
    }

    #[test]
    fn a_file_supplies_settings_and_the_environment_overrides_it() {
        let values = parse_file(
            "TPT_RUC_BIND = \"0.0.0.0:9000\"
             TPT_RUC_RETENTION_DAYS = 90
             TPT_RUC_CORS_ORIGINS = [\"https://a.example\", \"https://b.example\"]
",
        )
        .unwrap();
        let from_file = |name: &str| values.get(name).cloned();
        let config = Config::from_lookup(|name| {
            if name == "TPT_RUC_BIND" {
                Some("127.0.0.1:7000".to_owned())
            } else {
                from_file(name)
            }
        })
        .unwrap();
        assert_eq!(config.bind, "127.0.0.1:7000".parse().unwrap());
        assert_eq!(config.retention_days, Some(90));
        assert_eq!(
            config.cors_origins,
            vec!["https://a.example", "https://b.example"]
        );
    }

    #[test]
    fn a_file_with_a_bad_value_kind_is_refused() {
        assert!(matches!(
            parse_file("TPT_RUC_CORS_ORIGINS = [1, 2]"),
            Err(ConfigError::File(_))
        ));
        assert!(matches!(
            parse_file("TPT_RUC_BIND = ["),
            Err(ConfigError::File(_))
        ));
        assert!(matches!(
            parse_file("TPT_RUC_RETENTION_DAYS = { days = 3 }"),
            Err(ConfigError::File(_))
        ));
    }

    #[test]
    fn raw_gps_is_off_unless_switched_on() {
        assert!(!Config::from_lookup(lookup(&[])).unwrap().allow_raw_gps);
        let on = Config::from_lookup(lookup(&[("TPT_RUC_ALLOW_RAW_GPS", "True")])).unwrap();
        assert!(on.allow_raw_gps);
        assert!(matches!(
            Config::from_lookup(lookup(&[("TPT_RUC_ALLOW_RAW_GPS", "yes")])),
            Err(ConfigError::InvalidFlag(_, _))
        ));
    }

    #[test]
    fn data_dir_is_the_folder_of_the_database() {
        assert_eq!(
            data_dir_of("sqlite://data/tpt-ruc.db"),
            PathBuf::from("data")
        );
        assert_eq!(
            data_dir_of("sqlite:///data/tpt-ruc.db"),
            PathBuf::from("/data")
        );
        assert_eq!(data_dir_of("sqlite://tpt.db"), PathBuf::from("."));
    }
}

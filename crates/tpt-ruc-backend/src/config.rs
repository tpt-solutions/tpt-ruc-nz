// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Runtime configuration, read from environment variables.
//!
//! Every setting has a default, so the server starts with no configuration at
//! all. The API key is the one exception that may be left unset: the server
//! then generates one on first start (see [`crate::apikey`]).

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
        }
    }
}

impl std::error::Error for ConfigError {}

impl Config {
    /// Reads the configuration from the process environment.
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|name| std::env::var(name).ok())
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

        Ok(Self {
            bind,
            database_url,
            api_key,
            cors_origins,
            retention_days,
            pwa_dir,
            eruc_provider,
        })
    }

    /// The folder that holds the database file. The API key file also lives here.
    pub fn data_dir(&self) -> PathBuf {
        data_dir_of(&self.database_url)
    }
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

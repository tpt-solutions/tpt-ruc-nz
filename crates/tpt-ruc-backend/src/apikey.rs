// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Finds or creates the owner's API key.
//!
//! The key is taken from `TPT_RUC_API_KEY` when set. Otherwise it is read from
//! `api-key` in the data folder. If that file does not exist, a new key is
//! generated and saved there, so a first-time user needs no setup at all.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::config::MIN_API_KEY_LEN;

/// File name of the saved key, inside the data folder.
pub const KEY_FILE_NAME: &str = "api-key";

/// Where the key came from, so the caller can tell the owner what happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeySource {
    /// Set in `TPT_RUC_API_KEY`.
    Environment,
    /// Read from an existing key file.
    File(PathBuf),
    /// Newly generated and written to this file.
    Generated(PathBuf),
}

/// The key the server will accept, and where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedKey {
    /// The key itself.
    pub key: String,
    /// Where the key came from.
    pub source: KeySource,
}

/// Returns the key to use. Generates and saves one if none exists yet.
pub fn resolve(configured: Option<String>, data_dir: &Path) -> io::Result<ResolvedKey> {
    if let Some(key) = configured {
        return Ok(ResolvedKey {
            key,
            source: KeySource::Environment,
        });
    }

    let path = data_dir.join(KEY_FILE_NAME);
    match fs::read_to_string(&path) {
        Ok(text) => {
            let key = text.trim().to_owned();
            if key.len() < MIN_API_KEY_LEN {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "{} holds a key shorter than {MIN_API_KEY_LEN} characters. Delete it to generate a new one",
                        path.display()
                    ),
                ));
            }
            Ok(ResolvedKey {
                key,
                source: KeySource::File(path),
            })
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let key = generate()?;
            write_new_key(&path, &key)?;
            Ok(ResolvedKey {
                key,
                source: KeySource::Generated(path),
            })
        }
        Err(error) => Err(error),
    }
}

/// A new random key: 32 bytes from the operating system, as 64 hex characters.
fn generate() -> io::Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|error| io::Error::other(error.to_string()))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Writes the key to a new file, creating the folder if needed. The file is
/// readable only by its owner on Unix. It is never overwritten.
fn write_new_key(path: &Path, key: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    writeln!(file, "{key}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh folder under the system temp directory, unique to this test run.
    fn scratch_dir(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "tpt-ruc-apikey-{name}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_configured_key_wins_and_writes_nothing() {
        let dir = scratch_dir("configured");
        let resolved = resolve(Some("x".repeat(32)), &dir).unwrap();
        assert_eq!(resolved.source, KeySource::Environment);
        assert!(!dir.join(KEY_FILE_NAME).exists());
    }

    #[test]
    fn first_start_generates_and_later_starts_reuse_the_key() {
        let dir = scratch_dir("generate");
        let first = resolve(None, &dir).unwrap();
        assert!(matches!(first.source, KeySource::Generated(_)));
        assert_eq!(first.key.len(), 64);

        let second = resolve(None, &dir).unwrap();
        assert!(matches!(second.source, KeySource::File(_)));
        assert_eq!(second.key, first.key);
    }

    #[test]
    fn a_short_saved_key_is_refused_rather_than_replaced() {
        let dir = scratch_dir("short");
        fs::write(dir.join(KEY_FILE_NAME), "too-short\n").unwrap();
        assert!(resolve(None, &dir).is_err());
        assert_eq!(
            fs::read_to_string(dir.join(KEY_FILE_NAME)).unwrap(),
            "too-short\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn the_key_file_is_private_to_its_owner() {
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch_dir("mode");
        resolve(None, &dir).unwrap();
        let mode = fs::metadata(dir.join(KEY_FILE_NAME))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}

//! Non-secret application preferences (TOML).
//!
//! Secrets (Streamlabs token, OBS password, stream keys) are never persisted
//! here; they live in the OS credential store.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::domain::errors::ConfigError;
use crate::domain::models::{ObsPreferences, StreamConfig};

/// Persisted, non-secret preferences.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub stream: StreamConfig,
    #[serde(default)]
    pub obs: ObsPreferences,
}

impl AppConfig {
    /// Load from an explicit path. A missing file yields defaults.
    pub fn load_from(path: &Path) -> Result<Self, ConfigError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path).map_err(|e| ConfigError::Io(e.to_string()))?;
        if text.trim().is_empty() {
            return Ok(Self::default());
        }
        toml::from_str(&text).map_err(|e| ConfigError::Parse(e.to_string()))
    }

    /// Serialize to TOML without writing (used by tests and the UI preview).
    pub fn to_toml(&self) -> Result<String, ConfigError> {
        toml::to_string_pretty(self).map_err(|e| ConfigError::Parse(e.to_string()))
    }

    /// Save to an explicit path, creating parent directories as needed.
    pub fn save_to(&self, path: &Path) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ConfigError::Io(e.to_string()))?;
        }
        let text = self.to_toml()?;
        std::fs::write(path, text).map_err(|e| ConfigError::Io(e.to_string()))
    }

    /// Load from the default per-user location.
    pub fn load() -> Result<Self, ConfigError> {
        match crate::persistence::paths::config_file() {
            Some(path) => Self::load_from(&path),
            None => Err(ConfigError::NoConfigDir),
        }
    }

    /// Save to the default per-user location.
    pub fn save(&self) -> Result<(), ConfigError> {
        match crate::persistence::paths::config_file() {
            Some(path) => self.save_to(&path),
            None => Err(ConfigError::NoConfigDir),
        }
    }

    /// Convenience: the config path if resolvable.
    pub fn path() -> Option<PathBuf> {
        crate::persistence::paths::config_file()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::models::AudienceType;

    #[test]
    fn missing_file_loads_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("does-not-exist.toml");
        let cfg = AppConfig::load_from(&path).unwrap();
        assert_eq!(cfg.obs.host, "127.0.0.1");
        assert_eq!(cfg.obs.port, 4455);
        assert_eq!(cfg.stream.audience, AudienceType::General);
    }

    #[test]
    fn round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join("config.toml");
        let mut cfg = AppConfig::default();
        cfg.stream.title = "Ranked climb".into();
        cfg.stream.game = "VALORANT".into();
        cfg.stream.audience = AudienceType::Mature;
        cfg.obs.port = 4466;
        cfg.save_to(&path).unwrap();

        let loaded = AppConfig::load_from(&path).unwrap();
        assert_eq!(loaded, cfg);
    }

    #[test]
    fn config_never_contains_secret_keys() {
        // No TOML key may be a secret-bearing name. (A boolean such as
        // `remember_password` is fine; only key names are checked.)
        let forbidden = [
            "token",
            "password",
            "key",
            "stream_key",
            "secret",
            "oauth_token",
        ];
        let cfg = AppConfig::default();
        let text = cfg.to_toml().unwrap();
        for line in text.lines() {
            let Some((key, _)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            assert!(
                !forbidden.contains(&key),
                "config TOML has a secret-bearing key '{key}':\n{text}"
            );
        }
    }
}

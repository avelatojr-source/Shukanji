//! Per-user application directory resolution.

use std::path::PathBuf;

const APP_DIR: &str = "TikTokStreamTool";

/// Base config/data directory for the current user.
pub fn config_dir() -> Option<PathBuf> {
    if let Some(appdata) = std::env::var_os("APPDATA") {
        return Some(PathBuf::from(appdata).join(APP_DIR));
    }
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(xdg).join(APP_DIR));
    }
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config").join(APP_DIR))
}

/// Path to the non-secret TOML config file.
pub fn config_file() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join("config.toml"))
}

/// Directory for log files.
pub fn log_dir() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join("logs"))
}

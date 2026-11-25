//! Opt-in, read-only scan of the local Streamlabs LevelDB logs for an apiToken.
//!
//! This is never run automatically. It is narrowly scoped to the documented
//! Streamlabs application path and reads the newest `*.log` files only.

use std::path::{Path, PathBuf};

use crate::domain::models::SecretString;

/// The documented Streamlabs LevelDB directory for the current OS, if known.
pub fn default_leveldb_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA").map(|appdata| {
            PathBuf::from(appdata)
                .join("slobs-client")
                .join("Local Storage")
                .join("leveldb")
        })
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME").map(|home| {
            PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("slobs-client")
                .join("Local Storage")
                .join("leveldb")
        })
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        None
    }
}

/// Extract the last `"apiToken":"<hex>"` occurrence from raw log text.
pub fn extract_api_token(content: &str) -> Option<String> {
    const NEEDLE: &str = "\"apiToken\":\"";
    let mut last: Option<String> = None;
    let mut cursor = 0usize;
    while let Some(pos) = content[cursor..].find(NEEDLE) {
        let value_start = cursor + pos + NEEDLE.len();
        let rest = &content[value_start..];
        let end = rest.find('"').unwrap_or(rest.len());
        let candidate = &rest[..end];
        if !candidate.is_empty() && candidate.chars().all(|c| c.is_ascii_hexdigit()) {
            last = Some(candidate.to_string());
        }
        cursor = value_start;
    }
    last
}

/// Scan a LevelDB directory for the newest token. Returns `None` if not found.
pub fn scan_dir(dir: &Path) -> Option<SecretString> {
    let mut logs: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|entry| {
            entry
                .path()
                .extension()
                .map(|ext| ext.eq_ignore_ascii_case("log"))
                .unwrap_or(false)
        })
        .filter_map(|entry| {
            let meta = entry.metadata().ok()?;
            let modified = meta.modified().ok()?;
            Some((modified, entry.path()))
        })
        .collect();

    // Newest first.
    logs.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));

    for (_, path) in logs {
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let text = String::from_utf8_lossy(&bytes).replace('\0', "");
        if let Some(token) = extract_api_token(&text) {
            return Some(SecretString::new(token));
        }
    }
    None
}

/// Scan the default Streamlabs LevelDB directory.
pub fn find_local_token() -> Result<Option<SecretString>, String> {
    let dir = default_leveldb_dir().ok_or_else(|| {
        "local token retrieval is not supported on this operating system".to_string()
    })?;
    if !dir.exists() {
        return Ok(None);
    }
    Ok(scan_dir(&dir))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_token() {
        let content = r#"junk "apiToken":"deadbeef1234" more"#;
        assert_eq!(extract_api_token(content).as_deref(), Some("deadbeef1234"));
    }

    #[test]
    fn returns_last_match() {
        let content = r#""apiToken":"aaaa" ... "apiToken":"bbbb""#;
        assert_eq!(extract_api_token(content).as_deref(), Some("bbbb"));
    }

    #[test]
    fn ignores_non_hex_values() {
        let content = r#""apiToken":"not-a-token!!""#;
        assert_eq!(extract_api_token(content), None);
    }

    #[test]
    fn no_token_returns_none() {
        assert_eq!(extract_api_token("nothing here"), None);
    }

    #[test]
    fn scans_newest_log_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("000001.log"), r#""apiToken":"aaaa""#).unwrap();
        std::fs::write(dir.path().join("notes.txt"), r#""apiToken":"cccc""#).unwrap();

        let found = scan_dir(dir.path());
        assert!(found.is_some());
        assert_eq!(found.unwrap().expose(), "aaaa");
    }

    #[test]
    fn scan_missing_dir_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope");
        assert!(scan_dir(&missing).is_none());
    }
}

//! Core domain types. No GUI, network, or platform concerns live here.

use std::fmt;

use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};

/// A secret string that redacts itself in `Debug` output and zeroizes its
/// backing buffer on drop (best effort; not a hard guarantee).
#[derive(Clone, Default, Zeroize, ZeroizeOnDrop)]
pub struct SecretString(String);

impl SecretString {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Borrow the raw secret. Callers must not log the returned value.
    pub fn expose(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SecretString({})",
            crate::diagnostics::redact_secret(&self.0)
        )
    }
}

impl From<String> for SecretString {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for SecretString {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

/// A TikTok game/category suggestion from Streamlabs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameCategory {
    pub full_name: String,
    #[serde(default)]
    pub game_mask_id: String,
}

/// Whether a stream is flagged as mature (18+) content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudienceType {
    #[default]
    General,
    Mature,
}

impl AudienceType {
    /// The `audience_type` value the Streamlabs endpoint expects.
    pub fn as_api_str(self) -> &'static str {
        match self {
            AudienceType::General => "0",
            AudienceType::Mature => "1",
        }
    }
}

/// Account information returned by the Streamlabs TikTok info endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountInfo {
    pub username: String,
    pub application_status: String,
    pub can_be_live: bool,
}

/// Where the currently-held RTMP credentials came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialSource {
    Streamlabs,
    Manual,
}

impl CredentialSource {
    pub fn label(self) -> &'static str {
        match self {
            CredentialSource::Streamlabs => "Streamlabs",
            CredentialSource::Manual => "Manual entry",
        }
    }
}

/// RTMP credentials. The key is a secret and must be masked by default in UI.
#[derive(Clone)]
pub struct RtmpCredentials {
    pub server: String,
    pub key: SecretString,
    pub source: CredentialSource,
    /// Present only for credentials obtained from Streamlabs.
    pub stream_id: Option<String>,
}

impl fmt::Debug for RtmpCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RtmpCredentials")
            .field("server", &self.server)
            .field("key", &self.key)
            .field("source", &self.source)
            .field("stream_id", &self.stream_id)
            .finish()
    }
}

/// Non-secret stream configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamConfig {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub game: String,
    #[serde(default)]
    pub game_mask_id: String,
    #[serde(default)]
    pub audience: AudienceType,
}

/// Non-secret OBS preferences. Passwords are never stored here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObsPreferences {
    pub host: String,
    pub port: u16,
    /// Connect automatically when the app starts (only if a password is stored).
    #[serde(default)]
    pub auto_connect: bool,
    /// Remember the OBS password in the OS credential store.
    #[serde(default)]
    pub remember_password: bool,
}

impl Default for ObsPreferences {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 4455,
            auto_connect: false,
            remember_password: false,
        }
    }
}

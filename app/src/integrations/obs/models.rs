//! Typed OBS data used by the UI and coordinator.

use crate::domain::models::SecretString;

/// Result of `GetVersion`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObsVersion {
    pub obs_version: String,
    pub websocket_version: String,
    pub rpc_version: u32,
    pub available_requests: Vec<String>,
}

/// Result of `GetStreamStatus` (subset we act on).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StreamStatus {
    pub output_active: bool,
}

/// Result of `GetStreamServiceSettings`.
#[derive(Clone)]
pub struct StreamServiceSettings {
    pub service_type: String,
    pub server: String,
    pub key: SecretString,
}

impl std::fmt::Debug for StreamServiceSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamServiceSettings")
            .field("service_type", &self.service_type)
            .field("server", &self.server)
            .field("key", &self.key)
            .finish()
    }
}

//! Pure decision logic for OBS synchronization.

use crate::domain::states::{ObsOutputState, may_apply_stream_settings};

/// OBS service type for a user-provided RTMP server + key.
pub const CUSTOM_SERVICE_TYPE: &str = "rtmp_custom";

/// Why applying settings was refused or produced a mismatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyOutcome {
    Match,
    ServiceTypeMismatch {
        expected: String,
        actual: String,
    },
    ServerMismatch {
        expected: String,
        actual: String,
    },
    /// The read-back key did not match. Values are deliberately omitted.
    KeyMismatch,
    /// OBS reported an empty service type, meaning nothing was applied.
    NothingApplied,
}

impl VerifyOutcome {
    pub fn is_match(&self) -> bool {
        matches!(self, VerifyOutcome::Match)
    }

    /// Human-readable detail that never includes the stream key.
    pub fn detail(&self) -> String {
        match self {
            VerifyOutcome::Match => "read-back matches".to_string(),
            VerifyOutcome::ServiceTypeMismatch { expected, actual } => {
                format!("service type mismatch: expected '{expected}', got '{actual}'")
            }
            VerifyOutcome::ServerMismatch { expected, actual } => {
                format!("server mismatch: expected '{expected}', got '{actual}'")
            }
            VerifyOutcome::KeyMismatch => "stream key mismatch after apply".to_string(),
            VerifyOutcome::NothingApplied => "OBS reported no service type after apply".to_string(),
        }
    }
}

/// Guard used before applying. Delegates to the shared state rule so the UI
/// and the coordinator cannot drift.
pub fn preflight(output: ObsOutputState) -> Result<(), &'static str> {
    may_apply_stream_settings(output)
}

/// Compare what we intended to write against what OBS read back.
///
/// Key comparison is constant-content but its values are never returned.
pub fn verify_applied(
    expected_type: &str,
    expected_server: &str,
    expected_key: &str,
    actual_type: &str,
    actual_server: &str,
    actual_key: &str,
) -> VerifyOutcome {
    if actual_type.is_empty() {
        return VerifyOutcome::NothingApplied;
    }
    if actual_type != expected_type {
        return VerifyOutcome::ServiceTypeMismatch {
            expected: expected_type.to_string(),
            actual: actual_type.to_string(),
        };
    }
    if actual_server != expected_server {
        return VerifyOutcome::ServerMismatch {
            expected: expected_server.to_string(),
            actual: actual_server.to_string(),
        };
    }
    if actual_key != expected_key {
        return VerifyOutcome::KeyMismatch;
    }
    VerifyOutcome::Match
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preflight_refuses_when_active() {
        assert!(preflight(ObsOutputState::Active).is_err());
        assert!(preflight(ObsOutputState::Inactive).is_ok());
        assert!(preflight(ObsOutputState::Unknown).is_err());
    }

    #[test]
    fn verify_detects_match() {
        let outcome = verify_applied(
            CUSTOM_SERVICE_TYPE,
            "rtmp://live.tiktok.com/live/",
            "key-123",
            CUSTOM_SERVICE_TYPE,
            "rtmp://live.tiktok.com/live/",
            "key-123",
        );
        assert!(outcome.is_match());
    }

    #[test]
    fn verify_detects_key_mismatch_without_leaking() {
        let outcome = verify_applied(
            "rtmp_custom",
            "srv",
            "key-one",
            "rtmp_custom",
            "srv",
            "key-two",
        );
        assert_eq!(outcome, VerifyOutcome::KeyMismatch);
        let detail = outcome.detail();
        assert!(!detail.contains("key-one"));
        assert!(!detail.contains("key-two"));
    }

    #[test]
    fn verify_detects_server_mismatch() {
        let outcome = verify_applied(
            "rtmp_custom",
            "expected-srv",
            "k",
            "rtmp_custom",
            "other-srv",
            "k",
        );
        assert!(matches!(outcome, VerifyOutcome::ServerMismatch { .. }));
    }

    #[test]
    fn verify_detects_service_type_mismatch() {
        let outcome = verify_applied("rtmp_custom", "srv", "k", "rtmp_common", "srv", "k");
        assert!(matches!(outcome, VerifyOutcome::ServiceTypeMismatch { .. }));
    }

    #[test]
    fn verify_detects_nothing_applied() {
        let outcome = verify_applied("rtmp_custom", "srv", "k", "", "", "");
        assert_eq!(outcome, VerifyOutcome::NothingApplied);
    }
}

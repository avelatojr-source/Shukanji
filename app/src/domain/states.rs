//! Independent state machines. Success in one subsystem must never imply
//! success in another, so each is modeled separately.

use super::models::CredentialSource;

/// Authentication lifecycle.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AuthState {
    #[default]
    Disconnected,
    Authenticating,
    Authenticated {
        username: String,
    },
    Failed {
        message: String,
    },
}

impl AuthState {
    pub fn label(&self) -> &'static str {
        match self {
            AuthState::Disconnected => "Not authenticated",
            AuthState::Authenticating => "Authenticating…",
            AuthState::Authenticated { .. } => "Authenticated",
            AuthState::Failed { .. } => "Authentication failed",
        }
    }

    pub fn is_authenticated(&self) -> bool {
        matches!(self, AuthState::Authenticated { .. })
    }
}

/// Credential retrieval lifecycle, including the ambiguous timeout case.
///
/// `OutcomeUnknown` means a non-idempotent request (stream creation) was sent
/// but no definite response was received. The user must reconcile the session
/// before retrying; it must never be treated as an automatic retry.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum CredentialState {
    #[default]
    Empty,
    Retrieving,
    Retrieved {
        source: CredentialSource,
    },
    OutcomeUnknown {
        message: String,
    },
    Failed {
        message: String,
    },
}

impl CredentialState {
    pub fn label(&self) -> &'static str {
        match self {
            CredentialState::Empty => "No credentials yet",
            CredentialState::Retrieving => "Retrieving credentials…",
            CredentialState::Retrieved { .. } => "Credentials retrieved",
            CredentialState::OutcomeUnknown { .. } => "Request timed out — outcome unknown",
            CredentialState::Failed { .. } => "Credential request failed",
        }
    }

    /// True while a credential request is in flight.
    pub fn is_busy(&self) -> bool {
        matches!(self, CredentialState::Retrieving)
    }
}

/// OBS connection lifecycle.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ObsConnectionState {
    #[default]
    Disconnected,
    Connecting,
    Connected {
        version: String,
        rpc_version: u32,
    },
    Reconnecting {
        attempt: u32,
    },
    Failed {
        message: String,
    },
}

impl ObsConnectionState {
    pub fn is_connected(&self) -> bool {
        matches!(self, ObsConnectionState::Connected { .. })
    }

    pub fn label(&self) -> &'static str {
        match self {
            ObsConnectionState::Disconnected => "Disconnected",
            ObsConnectionState::Connecting => "Connecting…",
            ObsConnectionState::Connected { .. } => "Connected",
            ObsConnectionState::Reconnecting { .. } => "Reconnecting…",
            ObsConnectionState::Failed { .. } => "Connection failed",
        }
    }
}

/// Whether OBS is currently outputting (streaming/recording).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ObsOutputState {
    #[default]
    Unknown,
    Inactive,
    Active,
}

impl ObsOutputState {
    pub fn label(self) -> &'static str {
        match self {
            ObsOutputState::Unknown => "Output state unknown",
            ObsOutputState::Inactive => "Output inactive",
            ObsOutputState::Active => "Output running",
        }
    }
}

impl From<bool> for ObsOutputState {
    fn from(active: bool) -> Self {
        if active {
            ObsOutputState::Active
        } else {
            ObsOutputState::Inactive
        }
    }
}

/// Result of applying stream settings to OBS.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ApplyState {
    #[default]
    Idle,
    Applying,
    Applied {
        server: String,
    },
    /// Settings were written but the read-back did not match what was sent.
    Mismatch {
        detail: String,
    },
    Failed {
        message: String,
    },
    /// Refused because OBS was already outputting.
    RefusedOutputActive,
}

impl ApplyState {
    pub fn label(&self) -> &'static str {
        match self {
            ApplyState::Idle => "Not applied",
            ApplyState::Applying => "Applying…",
            ApplyState::Applied { .. } => "OBS settings applied and confirmed",
            ApplyState::Mismatch { .. } => "Applied, but read-back did not match",
            ApplyState::Failed { .. } => "Apply failed",
            ApplyState::RefusedOutputActive => "Refused: OBS output is running",
        }
    }
}

/// A guarded transition: may we write stream service settings right now?
///
/// Returns `Err(reason)` when applying is unsafe.
pub fn may_apply_stream_settings(output: ObsOutputState) -> Result<(), &'static str> {
    match output {
        ObsOutputState::Active => {
            Err("OBS is currently outputting. Stop the stream in OBS before applying settings.")
        }
        ObsOutputState::Unknown => {
            Err("OBS output state is unknown. Re-read OBS status before applying settings.")
        }
        ObsOutputState::Inactive => Ok(()),
    }
}

/// A guarded transition: may we start OBS streaming?
pub fn may_start_output(
    connected: &ObsConnectionState,
    output: ObsOutputState,
) -> Result<(), &'static str> {
    if !connected.is_connected() {
        return Err("OBS is not connected.");
    }
    match output {
        ObsOutputState::Active => Err("OBS output is already running."),
        _ => Ok(()),
    }
}

/// A guarded transition: may we request new Streamlabs credentials?
pub fn may_request_credentials(state: &CredentialState) -> Result<(), &'static str> {
    match state {
        CredentialState::Retrieving => Err("A credential request is already in progress."),
        CredentialState::OutcomeUnknown { .. } => Err(
            "A previous request timed out with unknown outcome. Reconcile the account/session first.",
        ),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_apply_when_output_active() {
        assert!(may_apply_stream_settings(ObsOutputState::Active).is_err());
    }

    #[test]
    fn refuses_apply_when_output_unknown() {
        assert!(may_apply_stream_settings(ObsOutputState::Unknown).is_err());
    }

    #[test]
    fn allows_apply_when_output_inactive() {
        assert!(may_apply_stream_settings(ObsOutputState::Inactive).is_ok());
    }

    #[test]
    fn output_state_from_bool() {
        assert_eq!(ObsOutputState::from(true), ObsOutputState::Active);
        assert_eq!(ObsOutputState::from(false), ObsOutputState::Inactive);
    }

    #[test]
    fn cannot_start_when_not_connected() {
        assert!(
            may_start_output(&ObsConnectionState::Disconnected, ObsOutputState::Inactive).is_err()
        );
    }

    #[test]
    fn cannot_start_when_already_active() {
        let connected = ObsConnectionState::Connected {
            version: "31.0.0".into(),
            rpc_version: 1,
        };
        assert!(may_start_output(&connected, ObsOutputState::Active).is_err());
        assert!(may_start_output(&connected, ObsOutputState::Inactive).is_ok());
    }

    #[test]
    fn outcome_unknown_blocks_new_requests() {
        let state = CredentialState::OutcomeUnknown {
            message: "timeout".into(),
        };
        assert!(may_request_credentials(&state).is_err());
    }

    #[test]
    fn retrieving_blocks_new_requests() {
        assert!(may_request_credentials(&CredentialState::Retrieving).is_err());
        assert!(may_request_credentials(&CredentialState::Empty).is_ok());
    }

    #[test]
    fn success_in_one_subsystem_does_not_imply_another() {
        // Retrieved credentials must not, by themselves, mark OBS connected.
        let creds = CredentialState::Retrieved {
            source: CredentialSource::Streamlabs,
        };
        let obs = ObsConnectionState::Disconnected;
        assert_eq!(creds.label(), "Credentials retrieved");
        assert!(!obs.is_connected());
    }
}

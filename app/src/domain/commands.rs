//! Commands sent from the UI thread to background tasks.

use super::models::{AudienceType, SecretString};

/// A unit of work requested by the UI.
///
/// `request_id` values correlate asynchronous replies so stale results can be
/// discarded (for example, an obsolete game search).
#[derive(Debug, Clone)]
pub enum Command {
    /// Load any stored token from the OS credential store and refresh account.
    RestoreSession,
    /// Remove stored Streamlabs token and OBS password from the credential store.
    ClearStoredSecrets,
    /// Begin the Streamlabs web (PKCE) authentication flow.
    BeginWebAuth,
    /// Opt-in scan of the local Streamlabs LevelDB logs for an apiToken.
    ScanLocalToken,
    /// Use a token the user pasted manually.
    SubmitToken {
        token: SecretString,
    },
    /// Re-read account info for the current token.
    RefreshAccount {
        token: SecretString,
    },
    /// Search game categories.
    SearchGames {
        token: SecretString,
        query: String,
        request_id: u64,
    },
    /// Request new RTMP credentials (non-idempotent; never auto-retried).
    GenerateCredentials {
        token: SecretString,
        title: String,
        game_mask_id: String,
        audience: AudienceType,
        request_id: u64,
    },
    /// End an existing Streamlabs stream session.
    EndStreamlabsStream {
        token: SecretString,
        stream_id: String,
    },
    /// One-click go-live: ensure OBS is connected, create the Streamlabs/TikTok
    /// session, push the RTMP settings to OBS, verify, then start OBS output.
    ///
    /// The Streamlabs `stream/start` call is non-idempotent: if its outcome is
    /// unknown the flow stops and must be reconciled (never auto-retried).
    GoLive {
        token: SecretString,
        title: String,
        game_mask_id: String,
        audience: AudienceType,
        host: String,
        port: u16,
        password: Option<SecretString>,
        remember_password: bool,
        request_id: u64,
    },

    /// Connect to OBS.
    ObsConnect {
        host: String,
        port: u16,
        password: Option<SecretString>,
        /// Persist the password in the OS credential store when true.
        remember_password: bool,
    },
    /// Connect to OBS using a password previously saved in the credential store.
    ObsConnectStored {
        host: String,
        port: u16,
    },
    ObsDisconnect,
    /// Re-read OBS stream status (output active/inactive).
    ObsRefreshStatus,
    /// Apply custom RTMP settings to OBS, after a read-back verification.
    ObsApplyStreamSettings {
        server: String,
        key: SecretString,
        request_id: u64,
    },
    /// Explicitly start OBS output.
    ObsStartStream,
    /// Explicitly stop OBS output.
    ObsStopStream,

    /// Connect to a TikTok LIVE room and stream chat/gift/activity events.
    /// Connecting again replaces any existing monitor.
    StartLiveMonitor {
        username: String,
    },
    /// Disconnect the TikTok LIVE monitor.
    StopLiveMonitor,
}

//! Events sent from background tasks back to the UI thread.

use super::models::{AccountInfo, GameCategory, RtmpCredentials, SecretString};
use super::monitor::{LiveConnectionState, MonitorEvent};
use super::states::{ApplyState, ObsOutputState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

/// A typed notification for the UI.
#[derive(Debug, Clone)]
pub enum AppEvent {
    WebAuthStarted {
        url: String,
    },
    WebAuthFinished {
        token: Option<SecretString>,
        message: Option<String>,
    },

    LocalTokenScanStarted,
    LocalTokenFound {
        token: SecretString,
    },
    LocalTokenNotFound,

    TokenSubmitted {
        token: SecretString,
    },

    AccountInfoLoaded {
        info: AccountInfo,
    },
    AccountInfoFailed {
        message: String,
    },

    GameSuggestions {
        request_id: u64,
        categories: Vec<GameCategory>,
    },
    GameSearchFailed {
        request_id: u64,
        message: String,
    },

    CredentialsRetrieved {
        request_id: u64,
        credentials: RtmpCredentials,
    },
    CredentialsOutcomeUnknown {
        request_id: u64,
        message: String,
    },
    CredentialsFailed {
        request_id: u64,
        message: String,
    },

    StreamlabsEnded {
        success: bool,
        message: String,
    },

    ObsConnected {
        version: String,
        rpc_version: u32,
    },
    ObsDisconnected {
        reason: String,
    },
    ObsReconnecting {
        attempt: u32,
    },
    ObsConnectionFailed {
        message: String,
    },
    ObsOutputStateChanged {
        state: ObsOutputState,
    },
    ObsApplyFinished {
        request_id: u64,
        result: ApplyState,
    },
    ObsStartFinished {
        ok: bool,
        message: String,
    },
    ObsStopFinished {
        ok: bool,
        message: String,
    },

    /// TikTok LIVE monitor connection state changed.
    LiveMonitorStatus {
        state: LiveConnectionState,
    },
    /// A decoded TikTok LIVE event (chat, gift, activity, counters).
    LiveMonitorEvent {
        event: MonitorEvent,
    },
    /// A stage update from the one-click go-live orchestration.
    GoLiveProgress {
        request_id: u64,
        stage: String,
    },
    /// Terminal result of the one-click go-live orchestration.
    GoLiveFinished {
        request_id: u64,
        ok: bool,
        message: String,
    },

    Log {
        level: LogLevel,
        message: String,
    },
}

/// A synchronous sink background tasks use to publish events to the UI.
///
/// Implementations must be cheap and non-blocking: they enqueue the event and
/// wake the UI thread. Events that cannot be enqueued (channel full/closed)
/// are dropped with a warning rather than blocking a background task.
pub trait EventSink: Send + Sync + 'static {
    fn emit(&self, event: AppEvent);
}

//! Domain layer: pure types, states, commands, events, and errors.

pub mod commands;
pub mod errors;
pub mod events;
pub mod models;
pub mod monitor;
pub mod states;

pub use commands::Command;
pub use events::{AppEvent, LogLevel};
pub use models::{
    AccountInfo, AudienceType, CredentialSource, GameCategory, ObsPreferences, RtmpCredentials,
    SecretString, StreamConfig,
};
pub use monitor::{
    ActivityKind, ActivityLine, ChatEmote, ChatLine, GiftLine, LiveConnectionState,
    LiveMonitorData, MonitorEvent,
};
pub use states::{
    ApplyState, AuthState, CredentialState, ObsConnectionState, ObsOutputState,
    may_apply_stream_settings, may_request_credentials, may_start_output,
};

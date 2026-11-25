//! Typed errors per integration. Messages must never embed secrets.

/// Errors from the Streamlabs HTTP client.
#[derive(Debug, thiserror::Error)]
pub enum StreamlabsError {
    #[error("network error: {0}")]
    Network(String),
    #[error("unexpected HTTP status {status}")]
    Http { status: u16 },
    #[error("malformed response: {0}")]
    Decode(String),
    #[error("{0}")]
    Api(String),
    #[error("provider returned no credentials")]
    NoCredentials,
}

/// Errors from the OAuth / PKCE flow.
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("authorization timed out")]
    Timeout,
    #[error("authorization was denied or failed")]
    Denied,
    #[error("callback validation failed: {0}")]
    Callback(String),
    #[error("token exchange failed: {0}")]
    Exchange(String),
    #[error("network error: {0}")]
    Network(String),
    #[error("could not open the system browser")]
    Browser,
}

/// Errors from the OBS WebSocket client.
#[derive(Debug, thiserror::Error)]
pub enum ObsError {
    #[error("connection failed: {0}")]
    Connect(String),
    #[error("OBS authentication failed")]
    Auth,
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error("request '{request}' failed: {detail}")]
    RequestFailed { request: String, detail: String },
    #[error("request '{0}' timed out")]
    Timeout(String),
    #[error("connection closed")]
    Closed,
    #[error("not connected to OBS")]
    NotConnected,
}

/// Errors from the OS-backed secret store.
#[derive(Debug, thiserror::Error)]
pub enum CredentialStoreError {
    #[error("secret not found")]
    NotFound,
    #[error("secret store unavailable: {0}")]
    Unavailable(String),
    #[error("secret store error: {0}")]
    Other(String),
}

/// Errors reading or writing the non-secret config file.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("could not resolve a config directory")]
    NoConfigDir,
    #[error("io error: {0}")]
    Io(String),
    #[error("parse error: {0}")]
    Parse(String),
}

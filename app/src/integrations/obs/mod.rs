//! OBS WebSocket 5.x integration.

pub mod client;
pub mod models;
pub mod protocol;
pub mod sync;

pub use client::{ObsHandle, ObsSettings};
pub use models::{ObsVersion, StreamServiceSettings, StreamStatus};
pub use protocol::compute_auth;
pub use sync::{CUSTOM_SERVICE_TYPE, VerifyOutcome, preflight, verify_applied};

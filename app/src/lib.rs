//! TikTok Stream Tool — a native Streamlabs + OBS utility.
//!
//! Layers:
//! - [`domain`]: pure types, states, commands, events, errors.
//! - [`integrations`]: Streamlabs HTTP/OAuth and the OBS WebSocket client.
//! - [`platform`]: OS-backed secret storage.
//! - [`persistence`]: non-secret TOML config.
//! - [`application`]: the background coordinator.
//! - [`ui`] / [`app`]: the egui front-end.

pub mod app;
pub mod application;
pub mod diagnostics;
pub mod domain;
pub mod integrations;
pub mod persistence;
pub mod platform;
pub mod ui;

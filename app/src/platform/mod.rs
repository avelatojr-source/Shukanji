//! Platform-specific integrations (kept behind narrow abstractions).

pub mod credentials;
#[cfg(windows)]
pub mod window_fx;

//! Streamlabs integration: typed client, OAuth/PKCE, and an opt-in local scan.

pub mod auth;
pub mod client;
pub mod local_scan;
pub mod models;

pub use client::{
    DEFAULT_BASE_URL, DEFAULT_USER_AGENT, MAX_CATEGORY_LEN, StartStreamResult, StreamlabsClient,
};

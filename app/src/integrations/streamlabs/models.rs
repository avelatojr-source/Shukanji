//! Wire-format types for the (undocumented) Streamlabs internal endpoints.
//!
//! These shapes come from the original Python source. Any field not present
//! there is treated as unverified and is optional/`#[serde(default)]` here.

use serde::Deserialize;

use crate::domain::models::GameCategory;

/// Response from `GET /tiktok/info`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct InfoResponse {
    #[serde(default)]
    pub user: Option<UserInfo>,
    #[serde(default)]
    pub application_status: Option<ApplicationStatus>,
    #[serde(default)]
    pub can_be_live: Option<bool>,
    #[serde(default)]
    pub categories: Vec<GameCategory>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct UserInfo {
    #[serde(default)]
    pub username: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ApplicationStatus {
    #[serde(default)]
    pub status: String,
}

/// Response from `POST /tiktok/stream/start`.
///
/// The original code reads exactly `id`, `rtmp`, and `key`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct StartStreamResponse {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub rtmp: String,
    #[serde(default)]
    pub key: String,
}

/// Response from `POST /tiktok/stream/{id}/end`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct EndStreamResponse {
    #[serde(default)]
    pub success: bool,
}

/// Response from `GET /auth/data` (OAuth code exchange).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct TokenExchangeResponse {
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub data: Option<TokenData>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TokenData {
    #[serde(default)]
    pub oauth_token: Option<String>,
}

//! Typed Streamlabs client.
//!
//! # Compatibility risk
//! These are undocumented Streamlabs Desktop endpoints, not a public API.
//! The original Python application sent a spoofed `StreamlabsDesktop` Electron
//! User-Agent. This port uses a **truthful** application User-Agent by default
//! and documents that as an unverified compatibility risk. If the endpoints
//! reject a truthful UA, call [`StreamlabsClient::with_user_agent`].

use std::time::Duration;

use reqwest::Client;

use crate::domain::errors::StreamlabsError;
use crate::domain::models::{AccountInfo, AudienceType, GameCategory, SecretString};

use super::models::{EndStreamResponse, InfoResponse, StartStreamResponse};

/// Base URL for the internal endpoints (shared by info and auth).
pub const DEFAULT_BASE_URL: &str = "https://streamlabs.com/api/v5/slobs";

/// Truthful application User-Agent (see module docs).
pub const DEFAULT_USER_AGENT: &str = concat!("TikTokStreamTool/", env!("CARGO_PKG_VERSION"));

/// The original client truncates the category query to 25 characters because
/// longer names make the endpoint return HTTP 500.
pub const MAX_CATEGORY_LEN: usize = 25;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(20);
/// Stream creation may take longer; timeouts here are treated as outcome-unknown.
const START_TIMEOUT: Duration = Duration::from_secs(30);

/// Outcome of a stream-start request. Because stream creation is
/// non-idempotent, an ambiguous result is surfaced explicitly rather than
/// retried.
#[derive(Debug)]
pub enum StartStreamResult {
    Started {
        stream_id: String,
        server: String,
        key: SecretString,
    },
    /// The request may or may not have taken effect (e.g. timeout or 5xx).
    OutcomeUnknown { detail: String },
}

/// A typed Streamlabs client bound to a single OAuth token.
pub struct StreamlabsClient {
    http: Client,
    base_url: String,
    token: SecretString,
}

impl std::fmt::Debug for StreamlabsClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamlabsClient")
            .field("base_url", &self.base_url)
            .field("token", &self.token)
            .finish()
    }
}

impl StreamlabsClient {
    /// Build a client with the default base URL and a truthful User-Agent.
    pub fn new(token: SecretString) -> Result<Self, StreamlabsError> {
        Self::with_base_url(token, DEFAULT_BASE_URL)
    }

    /// Build a client pointed at a custom base URL (used by tests).
    pub fn with_base_url(
        token: SecretString,
        base_url: impl Into<String>,
    ) -> Result<Self, StreamlabsError> {
        let http = Client::builder()
            .user_agent(DEFAULT_USER_AGENT)
            .timeout(DEFAULT_TIMEOUT)
            .build()
            .map_err(|e| StreamlabsError::Network(e.to_string()))?;
        Ok(Self::with_http(http, token, base_url))
    }

    /// Build a client that reuses an existing HTTP client.
    ///
    /// Production callers pass the dispatcher's shared client so TCP/TLS
    /// connections and the HTTP/2 pool are reused between calls instead of
    /// paying a fresh handshake per request.
    pub fn with_http(http: Client, token: SecretString, base_url: impl Into<String>) -> Self {
        Self {
            http,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            token,
        }
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }

    /// Search game categories. Empty query returns no suggestions.
    pub async fn search_games(&self, query: &str) -> Result<Vec<GameCategory>, StreamlabsError> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }
        // Truncate on a char boundary to avoid splitting a multi-byte name.
        let truncated: String = query.chars().take(MAX_CATEGORY_LEN).collect();

        let response = self
            .http
            .get(self.endpoint("/tiktok/info"))
            .bearer_auth(self.token.expose())
            .query(&[("category", truncated.as_str())])
            .send()
            .await
            .map_err(network_err)?;

        let status = response.status();
        if !status.is_success() {
            return Err(StreamlabsError::Http {
                status: status.as_u16(),
            });
        }
        let info: InfoResponse = response
            .json()
            .await
            .map_err(|e| StreamlabsError::Decode(e.to_string()))?;

        let mut categories = info.categories;
        // The original always appends an "Other" category with an empty mask id.
        categories.push(GameCategory {
            full_name: "Other".to_string(),
            game_mask_id: String::new(),
        });
        Ok(categories)
    }

    /// Read account information for the bound token.
    pub async fn account_info(&self) -> Result<AccountInfo, StreamlabsError> {
        let response = self
            .http
            .get(self.endpoint("/tiktok/info"))
            .bearer_auth(self.token.expose())
            .send()
            .await
            .map_err(network_err)?;

        let status = response.status();
        if !status.is_success() {
            return Err(StreamlabsError::Http {
                status: status.as_u16(),
            });
        }
        let info: InfoResponse = response
            .json()
            .await
            .map_err(|e| StreamlabsError::Decode(e.to_string()))?;

        Ok(AccountInfo {
            username: info
                .user
                .map(|u| u.username)
                .filter(|u| !u.is_empty())
                .unwrap_or_else(|| "Unknown".to_string()),
            application_status: info
                .application_status
                .map(|s| s.status)
                .unwrap_or_default(),
            can_be_live: info.can_be_live.unwrap_or(false),
        })
    }

    /// Start a stream and obtain RTMP credentials.
    ///
    /// Never retried automatically. A timeout or 5xx is reported as
    /// [`StartStreamResult::OutcomeUnknown`] so the caller can require the user
    /// to reconcile the session instead of blindly retrying.
    pub async fn start_stream(
        &self,
        title: &str,
        game_mask_id: &str,
        audience: AudienceType,
    ) -> Result<StartStreamResult, StreamlabsError> {
        let form = reqwest::multipart::Form::new()
            .part("title", reqwest::multipart::Part::text(title.to_string()))
            .part(
                "device_platform",
                reqwest::multipart::Part::text("win32".to_string()),
            )
            .part(
                "category",
                reqwest::multipart::Part::text(game_mask_id.to_string()),
            )
            .part(
                "audience_type",
                reqwest::multipart::Part::text(audience.as_api_str().to_string()),
            );

        let response = match self
            .http
            .post(self.endpoint("/tiktok/stream/start"))
            .bearer_auth(self.token.expose())
            .timeout(START_TIMEOUT)
            .multipart(form)
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(e) if e.is_timeout() => {
                return Ok(StartStreamResult::OutcomeUnknown {
                    detail: "request timed out".to_string(),
                });
            }
            Err(e) => return Err(network_err(e)),
        };

        let status = response.status();
        if status.as_u16() >= 500 {
            return Ok(StartStreamResult::OutcomeUnknown {
                detail: format!("server returned HTTP {}", status.as_u16()),
            });
        }
        if !status.is_success() {
            return Err(StreamlabsError::Http {
                status: status.as_u16(),
            });
        }

        let body: StartStreamResponse = response
            .json()
            .await
            .map_err(|e| StreamlabsError::Decode(e.to_string()))?;

        if body.id.is_empty() || body.rtmp.is_empty() || body.key.is_empty() {
            return Err(StreamlabsError::NoCredentials);
        }

        Ok(StartStreamResult::Started {
            stream_id: body.id,
            server: body.rtmp,
            key: SecretString::new(body.key),
        })
    }

    /// End an existing stream session.
    pub async fn end_stream(&self, stream_id: &str) -> Result<bool, StreamlabsError> {
        let response = self
            .http
            .post(self.endpoint(&format!("/tiktok/stream/{stream_id}/end")))
            .bearer_auth(self.token.expose())
            .send()
            .await
            .map_err(network_err)?;

        let status = response.status();
        if !status.is_success() {
            return Err(StreamlabsError::Http {
                status: status.as_u16(),
            });
        }
        let body: EndStreamResponse = response
            .json()
            .await
            .map_err(|e| StreamlabsError::Decode(e.to_string()))?;
        Ok(body.success)
    }
}

fn network_err(e: reqwest::Error) -> StreamlabsError {
    StreamlabsError::Network(e.to_string())
}

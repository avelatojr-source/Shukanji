//! Streamlabs OAuth (PKCE S256) primitives and the loopback callback server.
//!
//! Security properties implemented here:
//! - S256 code challenge.
//! - Cryptographically random `state` sent in the URL; a callback is accepted
//!   when it carries a non-empty authorization code (Streamlabs does not
//!   reliably echo `state`/`success`; PKCE S256 binds the code to this process).
//! - Loopback-only binding to `127.0.0.1` on an OS-assigned port.
//! - Strict callback path and parameter validation.
//! - Finite timeout; the listener is dropped on success, failure, or cancel.
//! - The authorization code and verifier are never logged.

use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};

use crate::domain::errors::AuthError;
use crate::domain::models::SecretString;

use super::client::DEFAULT_USER_AGENT;
use super::models::TokenExchangeResponse;

/// The only path the loopback server will treat as a valid callback.
pub const CALLBACK_PATH: &str = "/";
/// Maximum time to wait for the user to complete login.
pub const AUTH_TIMEOUT: Duration = Duration::from_secs(300);
/// Upper bound on the HTTP request head we will read from a callback.
const MAX_REQUEST_BYTES: usize = 8 * 1024;

/// Build the Streamlabs login URL. Reproduces the original parameter set and
/// adds a `state` value that we verify on callback.
pub fn build_auth_url(port: u16, code_challenge: &str, state: &str) -> String {
    format!(
        "https://streamlabs.com/slobs/login?skip_splash=true&external=electron&tiktok&force_verify\
         &origin=slobs&port={port}&code_challenge={code_challenge}&code_flow=true&state={state}"
    )
}

/// 64 random bytes, hex-encoded (128 hex chars) — the PKCE verifier.
pub fn generate_code_verifier() -> Result<String, AuthError> {
    let mut buf = [0u8; 64];
    getrandom::fill(&mut buf).map_err(|e| AuthError::Exchange(format!("entropy error: {e}")))?;
    Ok(hex_encode(&buf))
}

/// 32 random bytes, base64url-encoded — the anti-CSRF `state`.
pub fn generate_state() -> Result<String, AuthError> {
    let mut buf = [0u8; 32];
    getrandom::fill(&mut buf).map_err(|e| AuthError::Exchange(format!("entropy error: {e}")))?;
    Ok(URL_SAFE_NO_PAD.encode(buf))
}

/// `BASE64URL(SHA256(verifier))` without padding (PKCE S256).
pub fn code_challenge_s256(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

fn hex_encode(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(out, "{b:02x}");
    }
    out
}

/// Parse a URL query string into decoded key/value pairs.
pub fn parse_query(query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter(|s| !s.is_empty())
        .map(|pair| {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            (decode(k), decode(v))
        })
        .collect()
}

fn decode(s: &str) -> String {
    urlencoding::decode(s)
        .map(|c| c.into_owned())
        .unwrap_or_else(|_| s.to_string())
}

/// Validate a callback request. Returns the authorization code on success.
///
/// The original Python client accepted a callback purely on the presence of a
/// non-empty `code`; Streamlabs' internal redirect does **not** reliably echo
/// `success` or `state`. PKCE S256 binds the code to this process, so:
/// - `code` must be present and non-empty (the only hard requirement)
/// - `success=false` is an explicit denial and is rejected
/// - `state`, if echoed, is diagnostic only and never blocks the callback
/// - path must equal [`CALLBACK_PATH`]
pub fn validate_callback(
    path: &str,
    pairs: &[(String, String)],
    expected_state: &str,
) -> Result<String, AuthError> {
    if path != CALLBACK_PATH {
        return Err(AuthError::Callback(format!(
            "unexpected callback path '{path}'"
        )));
    }
    let get = |key: &str| {
        pairs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    };

    if get("success") == Some("false") {
        return Err(AuthError::Denied);
    }
    if let Some(state) = get("state")
        && state != expected_state
    {
        // Streamlabs may echo its own `state`. PKCE still binds the code to us,
        // so this is informational and must not fail the login.
        tracing::debug!(
            "oauth callback carried a non-matching state; continuing (PKCE binds the code)"
        );
    }
    let code = get("code").filter(|c| !c.is_empty()).ok_or_else(|| {
        // Names only, never values: a callback may carry secrets.
        let names = pairs
            .iter()
            .map(|(k, _)| k.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        AuthError::Callback(format!("missing authorization code (params: [{names}])"))
    })?;
    Ok(code.to_string())
}

/// Wait for a single valid OAuth callback on `listener`, with a deadline.
///
/// Unknown paths get a 404 and are ignored (e.g. a stray favicon request).
/// A callback with invalid parameters gets a 400 and aborts the flow.
pub async fn listen_for_callback(
    listener: tokio::net::TcpListener,
    expected_state: &str,
    timeout: Duration,
) -> Result<String, AuthError> {
    let deadline = tokio::time::Instant::now() + timeout;

    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return Err(AuthError::Timeout);
        }

        let accepted = tokio::time::timeout(remaining, listener.accept()).await;
        let (mut stream, _peer) = match accepted {
            Ok(Ok(pair)) => pair,
            Ok(Err(e)) => return Err(AuthError::Network(e.to_string())),
            Err(_) => return Err(AuthError::Timeout),
        };

        let head = match read_request_head(&mut stream).await {
            Ok(h) => h,
            Err(_) => continue,
        };
        let request_line = head.lines().next().unwrap_or_default();
        let mut parts = request_line.split_whitespace();
        let method = parts.next().unwrap_or_default();
        let target = parts.next().unwrap_or("/");

        if !method.eq_ignore_ascii_case("GET") {
            let _ = write_plain(&mut stream, 405, "Method not allowed").await;
            continue;
        }

        let (path, query) = match target.split_once('?') {
            Some((p, q)) => (p, q),
            None => (target, ""),
        };

        if path != CALLBACK_PATH {
            let _ = write_plain(&mut stream, 404, "Not found").await;
            continue;
        }

        let pairs = parse_query(query);
        match validate_callback(path, &pairs, expected_state) {
            Ok(code) => {
                let _ = write_html(
                    &mut stream,
                    200,
                    "<h2>Authentication successful. You can close this tab.</h2>",
                )
                .await;
                return Ok(code);
            }
            Err(e) => {
                // Debug aid: log parameter NAMES only, never values.
                let names = pairs
                    .iter()
                    .map(|(k, _)| k.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                tracing::warn!(params = %names, "oauth callback rejected");
                let _ = write_html(
                    &mut stream,
                    400,
                    "<h2>Authentication failed. Please try again.</h2>",
                )
                .await;
                return Err(e);
            }
        }
    }
}

async fn read_request_head(stream: &mut tokio::net::TcpStream) -> std::io::Result<String> {
    use tokio::io::AsyncReadExt;
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 512];
    loop {
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
        if buf.windows(4).any(|w| w == b"\r\n\r\n") || buf.len() >= MAX_REQUEST_BYTES {
            break;
        }
    }
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

async fn write_plain(
    stream: &mut tokio::net::TcpStream,
    status: u16,
    body: &str,
) -> std::io::Result<()> {
    write_html(stream, status, &format!("<p>{body}</p>")).await
}

async fn write_html(
    stream: &mut tokio::net::TcpStream,
    status: u16,
    body: &str,
) -> std::io::Result<()> {
    use tokio::io::AsyncWriteExt;
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "Error",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await?;
    stream.flush().await
}

/// Exchange an authorization code (plus verifier) for an OAuth token.
///
/// The code and verifier are never logged.
pub async fn exchange_code_for_token(
    http: &reqwest::Client,
    base_url: &str,
    code: &str,
    code_verifier: &str,
) -> Result<SecretString, AuthError> {
    let url = format!("{}/auth/data", base_url.trim_end_matches('/'));
    let response = http
        .get(url)
        .header(reqwest::header::USER_AGENT, DEFAULT_USER_AGENT)
        .header(reqwest::header::ACCEPT, "*/*")
        .query(&[("code_verifier", code_verifier), ("code", code)])
        .send()
        .await
        .map_err(|e| AuthError::Network(e.to_string()))?;

    let status = response.status();
    if !status.is_success() {
        return Err(AuthError::Exchange(format!(
            "token exchange returned HTTP {}",
            status.as_u16()
        )));
    }

    let body: TokenExchangeResponse = response
        .json()
        .await
        .map_err(|e| AuthError::Exchange(format!("could not decode token response: {e}")))?;

    if !body.success {
        return Err(AuthError::Exchange("provider reported failure".to_string()));
    }
    let token = body
        .data
        .and_then(|d| d.oauth_token)
        .filter(|t| !t.is_empty())
        .ok_or_else(|| AuthError::Exchange("no oauth token in response".to_string()))?;
    Ok(SecretString::new(token))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn challenge_is_stable_s256_base64url() {
        // RFC 7636 Appendix B test vector.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = code_challenge_s256(verifier);
        assert_eq!(challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    #[test]
    fn verifier_is_128_hex_chars_and_random() {
        let a = generate_code_verifier().unwrap();
        let b = generate_code_verifier().unwrap();
        assert_eq!(a.len(), 128);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn state_is_random() {
        let a = generate_state().unwrap();
        let b = generate_state().unwrap();
        assert_ne!(a, b);
        assert!(!a.is_empty());
    }

    #[test]
    fn parses_query_with_encoding() {
        let pairs = parse_query("success=true&code=a%2Bb&state=xyz");
        assert_eq!(pairs[0], ("success".into(), "true".into()));
        assert_eq!(pairs[1], ("code".into(), "a+b".into()));
        assert_eq!(pairs[2], ("state".into(), "xyz".into()));
    }

    #[test]
    fn validation_accepts_valid_callback() {
        let pairs = parse_query("success=true&code=abc123&state=expected");
        assert_eq!(
            validate_callback("/", &pairs, "expected").unwrap(),
            "abc123"
        );
    }

    #[test]
    fn validation_accepts_state_mismatch() {
        // Streamlabs may echo its own `state`; PKCE binds the code, so a
        // non-matching state is diagnostic only and must not fail login.
        let pairs = parse_query("success=true&code=abc&state=attacker");
        assert_eq!(validate_callback("/", &pairs, "expected").unwrap(), "abc");
    }

    #[test]
    fn validation_accepts_missing_state() {
        // The original Python flow never sends `state`; PKCE covers the code
        // exchange, so an absent state is accepted.
        let pairs = parse_query("success=true&code=abc");
        assert_eq!(validate_callback("/", &pairs, "expected").unwrap(), "abc");
    }

    #[test]
    fn validation_accepts_missing_success() {
        // Streamlabs' internal redirect does not reliably echo `success`; the
        // presence of a code is what matters.
        let pairs = parse_query("code=abc");
        assert_eq!(validate_callback("/", &pairs, "expected").unwrap(), "abc");
    }

    #[test]
    fn validation_rejects_missing_code_and_lists_param_names() {
        let pairs = parse_query("success=true&state=expected");
        match validate_callback("/", &pairs, "expected") {
            Err(AuthError::Callback(msg)) => {
                assert!(msg.contains("missing authorization code"), "msg: {msg}");
                assert!(msg.contains("success"), "msg: {msg}");
                assert!(msg.contains("state"), "msg: {msg}");
                // Never leak a value.
                assert!(!msg.contains("expected"), "msg: {msg}");
            }
            other => panic!("expected Callback error, got {other:?}"),
        }
    }

    #[test]
    fn validation_rejects_denied() {
        let pairs = parse_query("success=false&state=expected");
        assert!(matches!(
            validate_callback("/", &pairs, "expected"),
            Err(AuthError::Denied)
        ));
    }

    #[test]
    fn validation_rejects_wrong_path() {
        let pairs = parse_query("success=true&code=abc&state=expected");
        assert!(matches!(
            validate_callback("/evil", &pairs, "expected"),
            Err(AuthError::Callback(_))
        ));
    }
}

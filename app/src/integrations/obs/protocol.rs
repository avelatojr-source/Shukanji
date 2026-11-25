//! OBS WebSocket 5.x protocol: opcodes, message shapes, and auth.
//!
//! Source of truth: the official OBS WebSocket protocol reference. Only the
//! subset needed for this application is modeled.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// WebSocket opcodes.
pub mod op {
    pub const HELLO: u8 = 0;
    pub const IDENTIFY: u8 = 1;
    pub const IDENTIFIED: u8 = 2;
    pub const REIDENTIFY: u8 = 3;
    pub const EVENT: u8 = 5;
    pub const REQUEST: u8 = 6;
    pub const REQUEST_RESPONSE: u8 = 7;
    pub const REQUEST_BATCH: u8 = 8;
    pub const REQUEST_BATCH_RESPONSE: u8 = 9;
}

/// The RPC version this client speaks.
pub const RPC_VERSION: u32 = 1;

/// The generic envelope for every inbound message.
#[derive(Debug, Clone, Deserialize)]
pub struct Incoming {
    pub op: u8,
    #[serde(default)]
    pub d: Value,
}

/// `Hello` (opcode 0) payload.
#[derive(Debug, Clone, Deserialize)]
pub struct Hello {
    #[serde(rename = "obsWebSocketVersion", default)]
    pub obs_web_socket_version: String,
    #[serde(rename = "rpcVersion", default)]
    pub rpc_version: u32,
    #[serde(default)]
    pub authentication: Option<HelloAuth>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HelloAuth {
    pub challenge: String,
    pub salt: String,
}

/// `Identify` (opcode 1) payload.
#[derive(Debug, Clone, Serialize)]
pub struct Identify {
    #[serde(rename = "rpcVersion")]
    pub rpc_version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authentication: Option<String>,
    #[serde(rename = "eventSubscriptions", skip_serializing_if = "Option::is_none")]
    pub event_subscriptions: Option<u32>,
}

/// `Identified` (opcode 2) payload.
#[derive(Debug, Clone, Deserialize)]
pub struct Identified {
    #[serde(rename = "negotiatedRpcVersion", default)]
    pub negotiated_rpc_version: u32,
}

/// `Request` (opcode 6) payload.
#[derive(Debug, Clone, Serialize)]
pub struct Request {
    #[serde(rename = "requestType")]
    pub request_type: String,
    #[serde(rename = "requestId")]
    pub request_id: String,
    #[serde(rename = "requestData", skip_serializing_if = "Option::is_none")]
    pub request_data: Option<Value>,
}

/// `RequestResponse` (opcode 7) payload.
#[derive(Debug, Clone, Deserialize)]
pub struct RequestResponse {
    #[serde(rename = "requestType", default)]
    pub request_type: String,
    #[serde(rename = "requestId", default)]
    pub request_id: String,
    #[serde(rename = "requestStatus", default)]
    pub request_status: Option<RequestStatus>,
    #[serde(rename = "responseData", default)]
    pub response_data: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RequestStatus {
    #[serde(default)]
    pub result: bool,
    #[serde(default)]
    pub code: i64,
    #[serde(default)]
    pub comment: Option<String>,
}

/// Compute the OBS WebSocket v5 authentication string.
///
/// `base64(sha256(password + salt))` then
/// `base64(sha256(secret + challenge))`, both base64 (standard, padded).
pub fn compute_auth(password: &str, challenge: &str, salt: &str) -> String {
    let secret = Sha256::digest(format!("{password}{salt}").as_bytes());
    let secret_b64 = STANDARD.encode(secret);
    let auth = Sha256::digest(format!("{secret_b64}{challenge}").as_bytes());
    STANDARD.encode(auth)
}

/// Serialize an envelope with an opcode and payload.
pub fn envelope(op: u8, payload: &impl Serialize) -> Result<String, serde_json::Error> {
    #[derive(Serialize)]
    struct Env<'a, T> {
        op: u8,
        d: &'a T,
    }
    serde_json::to_string(&Env { op, d: payload })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Reference values independently computed with an external SHA-256
    // implementation (PowerShell / .NET) from the same inputs.
    const REF_AUTH: &str = "B10L7eSLCldL8PH8s7RXiB8XWMjHBGoMEFyEHuarjYY=";
    const REF_SECRET: &str = "vPWxMrzZregVx2mKHqKZ0+T7BS3vtQ0z1FKb+B+jOsw=";

    #[test]
    fn auth_matches_external_reference() {
        assert_eq!(
            compute_auth("password123", "challenge456", "salt789"),
            REF_AUTH
        );
    }

    #[test]
    fn secret_intermediate_matches_reference() {
        let secret = Sha256::digest(format!("{}{}", "password123", "salt789").as_bytes());
        assert_eq!(STANDARD.encode(secret), REF_SECRET);
    }

    #[test]
    fn auth_is_deterministic_and_password_sensitive() {
        let a = compute_auth("pw", "ch", "salt");
        let b = compute_auth("pw", "ch", "salt");
        let c = compute_auth("other", "ch", "salt");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn identify_serializes_auth_only_when_present() {
        let no_auth = Identify {
            rpc_version: RPC_VERSION,
            authentication: None,
            event_subscriptions: None,
        };
        let json = serde_json::to_string(&no_auth).unwrap();
        assert!(!json.contains("authentication"));
        assert!(json.contains("\"rpcVersion\":1"));

        let with_auth = Identify {
            rpc_version: RPC_VERSION,
            authentication: Some("abc".into()),
            event_subscriptions: None,
        };
        let json = serde_json::to_string(&with_auth).unwrap();
        assert!(json.contains("\"authentication\":\"abc\""));
    }

    #[test]
    fn parses_hello_without_auth() {
        let raw = r#"{"op":0,"d":{"obsWebSocketVersion":"5.5.2","rpcVersion":1}}"#;
        let incoming: Incoming = serde_json::from_str(raw).unwrap();
        assert_eq!(incoming.op, op::HELLO);
        let hello: Hello = serde_json::from_value(incoming.d).unwrap();
        assert_eq!(hello.rpc_version, 1);
        assert!(hello.authentication.is_none());
    }

    #[test]
    fn parses_hello_with_auth() {
        let raw = r#"{"op":0,"d":{"obsWebSocketVersion":"5.5.2","rpcVersion":1,"authentication":{"challenge":"c","salt":"s"}}}"#;
        let incoming: Incoming = serde_json::from_str(raw).unwrap();
        let hello: Hello = serde_json::from_value(incoming.d).unwrap();
        assert_eq!(hello.authentication.unwrap().salt, "s");
    }

    #[test]
    fn parses_failed_request_status() {
        let raw = r#"{"op":7,"d":{"requestType":"GetVersion","requestId":"1","requestStatus":{"result":false,"code":604,"comment":"No source"}}}"#;
        let incoming: Incoming = serde_json::from_str(raw).unwrap();
        let resp: RequestResponse = serde_json::from_value(incoming.d).unwrap();
        let status = resp.request_status.unwrap();
        assert!(!status.result);
        assert_eq!(status.code, 604);
        assert_eq!(status.comment.as_deref(), Some("No source"));
    }
}

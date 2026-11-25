//! OBS WebSocket 5.x client.
//!
//! The connection is owned by a single actor task. Callers hold an
//! [`ObsHandle`] and issue typed requests that are correlated by `requestId`.
//! The actor reconnects with exponential backoff + jitter and re-runs the
//! handshake on every reconnect; it never assumes previous state persisted.
//!
//! Streaming is never started automatically — only `start_stream` does that,
//! and it is invoked exclusively from an explicit UI action.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot, watch};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

use crate::domain::errors::ObsError;
use crate::domain::events::{AppEvent, EventSink};
use crate::domain::models::SecretString;

use super::models::{ObsVersion, StreamServiceSettings, StreamStatus};
use super::protocol::{self, op};
use super::sync::CUSTOM_SERVICE_TYPE;

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Time budget for a single request/response round trip.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
/// Time budget for the Hello/Identified handshake steps.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
/// Command channel depth.
const COMMAND_CAPACITY: usize = 32;

/// Where to reach OBS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObsSettings {
    pub host: String,
    pub port: u16,
}

impl Default for ObsSettings {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 4455,
        }
    }
}

impl ObsSettings {
    pub fn url(&self) -> String {
        format!("ws://{}:{}", self.host, self.port)
    }
}

/// A command for the connection actor.
pub enum ObsCommand {
    Request {
        request_type: &'static str,
        data: Option<Value>,
        reply: oneshot::Sender<Result<Value, ObsError>>,
    },
    Shutdown,
}

struct Pending {
    sent_at: Instant,
    reply: oneshot::Sender<Result<Value, ObsError>>,
}

enum SessionEnd {
    Shutdown,
    Disconnected(String),
}

/// A cheap, cloneable handle used to talk to a running OBS connection.
#[derive(Clone)]
pub struct ObsHandle {
    tx: mpsc::Sender<ObsCommand>,
    request_timeout: Duration,
    /// Latest known connection state. Set by the actor after a successful
    /// handshake and cleared on disconnect, so callers can await readiness.
    connected: watch::Receiver<bool>,
}

impl ObsHandle {
    /// Spawn the connection actor and return a handle to it.
    pub fn spawn(
        settings: ObsSettings,
        password: Option<SecretString>,
        sink: Arc<dyn EventSink>,
    ) -> Self {
        Self::spawn_with_timeout(settings, password, sink, REQUEST_TIMEOUT)
    }

    /// Like [`ObsHandle::spawn`] but with a custom request timeout (tests).
    pub fn spawn_with_timeout(
        settings: ObsSettings,
        password: Option<SecretString>,
        sink: Arc<dyn EventSink>,
        request_timeout: Duration,
    ) -> Self {
        let (tx, rx) = mpsc::channel(COMMAND_CAPACITY);
        let (connected_tx, connected) = watch::channel(false);
        tokio::spawn(run_actor(settings, password, sink, rx, connected_tx));
        Self {
            tx,
            request_timeout,
            connected,
        }
    }

    /// Ask the actor to stop and disconnect.
    pub fn shutdown(&self) {
        let _ = self.tx.try_send(ObsCommand::Shutdown);
    }

    /// Whether the actor currently has a completed handshake.
    pub fn is_connected(&self) -> bool {
        *self.connected.borrow()
    }

    /// Wait until the handshake completes, or the timeout elapses.
    pub async fn wait_until_connected(&self, timeout: Duration) -> Result<(), ObsError> {
        let mut rx = self.connected.clone();
        let wait = async {
            loop {
                if *rx.borrow_and_update() {
                    return;
                }
                if rx.changed().await.is_err() {
                    return;
                }
            }
        };
        match tokio::time::timeout(timeout, wait).await {
            Ok(()) if self.is_connected() => Ok(()),
            Ok(()) => Err(ObsError::Connect("OBS connection ended".to_string())),
            Err(_) => Err(ObsError::Timeout("OBS connect".to_string())),
        }
    }

    async fn request(
        &self,
        request_type: &'static str,
        data: Option<Value>,
    ) -> Result<Value, ObsError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(ObsCommand::Request {
                request_type,
                data,
                reply: reply_tx,
            })
            .await
            .map_err(|_| ObsError::NotConnected)?;
        match tokio::time::timeout(self.request_timeout, reply_rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(ObsError::NotConnected),
            Err(_) => Err(ObsError::Timeout(request_type.to_string())),
        }
    }

    pub async fn get_version(&self) -> Result<ObsVersion, ObsError> {
        let v = self.request("GetVersion", None).await?;
        Ok(ObsVersion {
            obs_version: str_field(&v, "obsVersion"),
            websocket_version: str_field(&v, "obsWebSocketVersion"),
            rpc_version: v.get("rpcVersion").and_then(Value::as_u64).unwrap_or(0) as u32,
            available_requests: v
                .get("availableRequests")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|y| y.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
        })
    }

    pub async fn get_stream_status(&self) -> Result<StreamStatus, ObsError> {
        let v = self.request("GetStreamStatus", None).await?;
        Ok(StreamStatus {
            output_active: v
                .get("outputActive")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        })
    }

    pub async fn get_stream_service_settings(&self) -> Result<StreamServiceSettings, ObsError> {
        let v = self.request("GetStreamServiceSettings", None).await?;
        Ok(parse_service_settings(&v))
    }

    pub async fn set_stream_service_settings(
        &self,
        server: &str,
        key: &SecretString,
    ) -> Result<(), ObsError> {
        let data = json!({
            "streamServiceType": CUSTOM_SERVICE_TYPE,
            "streamServiceSettings": {
                "server": server,
                "key": key.expose(),
            }
        });
        self.request("SetStreamServiceSettings", Some(data))
            .await
            .map(|_| ())
    }

    pub async fn start_stream(&self) -> Result<(), ObsError> {
        self.request("StartStream", None).await.map(|_| ())
    }

    pub async fn stop_stream(&self) -> Result<(), ObsError> {
        self.request("StopStream", None).await.map(|_| ())
    }
}

fn str_field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// Parse `GetStreamServiceSettings` response data.
pub fn parse_service_settings(value: &Value) -> StreamServiceSettings {
    let service_type = str_field(value, "streamServiceType");
    let settings = value
        .get("streamServiceSettings")
        .cloned()
        .unwrap_or(Value::Null);
    StreamServiceSettings {
        service_type,
        server: str_field(&settings, "server"),
        key: SecretString::new(str_field(&settings, "key")),
    }
}

async fn run_actor(
    settings: ObsSettings,
    password: Option<SecretString>,
    sink: Arc<dyn EventSink>,
    mut rx: mpsc::Receiver<ObsCommand>,
    connected_tx: watch::Sender<bool>,
) {
    let mut attempt: u32 = 0;
    loop {
        match connect_and_handshake(&settings.url(), password.as_ref()).await {
            Ok((ws, websocket_version, negotiated_rpc)) => {
                attempt = 0;
                let _ = connected_tx.send(true);
                sink.emit(AppEvent::ObsConnected {
                    version: websocket_version,
                    rpc_version: negotiated_rpc,
                });
                match run_session(ws, &mut rx, &sink).await {
                    SessionEnd::Shutdown => {
                        let _ = connected_tx.send(false);
                        return;
                    }
                    SessionEnd::Disconnected(reason) => {
                        let _ = connected_tx.send(false);
                        sink.emit(AppEvent::ObsDisconnected { reason });
                    }
                }
            }
            Err(err) => {
                let _ = connected_tx.send(false);
                sink.emit(AppEvent::ObsConnectionFailed {
                    message: err.to_string(),
                });
            }
        }

        attempt = attempt.saturating_add(1);
        sink.emit(AppEvent::ObsReconnecting { attempt });
        if wait_or_shutdown(&mut rx, backoff_delay(attempt)).await {
            return;
        }
    }
}

async fn connect_and_handshake(
    url: &str,
    password: Option<&SecretString>,
) -> Result<(Socket, String, u32), ObsError> {
    let (mut ws, _resp) = connect_async(url)
        .await
        .map_err(|e| ObsError::Connect(e.to_string()))?;

    let hello = recv_hello(&mut ws).await?;

    let authentication = match &hello.authentication {
        Some(auth) => {
            let password = password.ok_or(ObsError::Auth)?;
            Some(protocol::compute_auth(
                password.expose(),
                &auth.challenge,
                &auth.salt,
            ))
        }
        None => None,
    };

    let identify = protocol::Identify {
        rpc_version: protocol::RPC_VERSION,
        authentication,
        event_subscriptions: None,
    };
    let text = protocol::envelope(op::IDENTIFY, &identify)
        .map_err(|e| ObsError::Protocol(e.to_string()))?;
    ws.send(Message::Text(text.into()))
        .await
        .map_err(|e| ObsError::Connect(e.to_string()))?;

    let negotiated = recv_identified(&mut ws).await?;
    Ok((ws, hello.obs_web_socket_version, negotiated))
}

async fn recv_hello(ws: &mut Socket) -> Result<protocol::Hello, ObsError> {
    loop {
        let message = tokio::time::timeout(HANDSHAKE_TIMEOUT, ws.next())
            .await
            .map_err(|_| ObsError::Timeout("Hello".to_string()))?
            .ok_or(ObsError::Closed)?
            .map_err(|e| ObsError::Protocol(e.to_string()))?;

        let Some(incoming) = parse_incoming(&message) else {
            continue;
        };
        if incoming.op != op::HELLO {
            return Err(ObsError::Protocol(format!(
                "expected Hello (op 0), got op {}",
                incoming.op
            )));
        }
        return serde_json::from_value(incoming.d)
            .map_err(|e| ObsError::Protocol(format!("bad Hello payload: {e}")));
    }
}

async fn recv_identified(ws: &mut Socket) -> Result<u32, ObsError> {
    loop {
        let message = tokio::time::timeout(HANDSHAKE_TIMEOUT, ws.next())
            .await
            .map_err(|_| ObsError::Timeout("Identified".to_string()))?
            .ok_or(ObsError::Closed)?
            .map_err(|e| ObsError::Protocol(e.to_string()))?;

        let Some(incoming) = parse_incoming(&message) else {
            continue;
        };
        match incoming.op {
            op::IDENTIFIED => {
                let identified: protocol::Identified = serde_json::from_value(incoming.d)
                    .map_err(|e| ObsError::Protocol(format!("bad Identified payload: {e}")))?;
                return Ok(identified.negotiated_rpc_version);
            }
            op::REQUEST_RESPONSE => continue,
            _ => continue,
        }
    }
}

fn parse_incoming(message: &Message) -> Option<protocol::Incoming> {
    let text = match message {
        Message::Text(t) => t.as_str().to_string(),
        Message::Binary(b) => String::from_utf8_lossy(b).into_owned(),
        _ => return None,
    };
    serde_json::from_str(&text).ok()
}

async fn run_session(
    mut ws: Socket,
    rx: &mut mpsc::Receiver<ObsCommand>,
    sink: &Arc<dyn EventSink>,
) -> SessionEnd {
    let mut pending: HashMap<String, Pending> = HashMap::new();

    loop {
        prune_expired(&mut pending);

        tokio::select! {
            command = rx.recv() => {
                match command {
                    None => {
                        fail_all(&mut pending, ObsError::Closed);
                        return SessionEnd::Shutdown;
                    }
                    Some(ObsCommand::Shutdown) => {
                        let _ = ws.close(None).await;
                        fail_all(&mut pending, ObsError::Closed);
                        return SessionEnd::Shutdown;
                    }
                    Some(ObsCommand::Request { request_type, data, reply }) => {
                        let request_id = uuid::Uuid::new_v4().to_string();
                        let request = protocol::Request {
                            request_type: request_type.to_string(),
                            request_id: request_id.clone(),
                            request_data: data,
                        };
                        let text = match protocol::envelope(op::REQUEST, &request) {
                            Ok(t) => t,
                            Err(e) => {
                                let _ = reply.send(Err(ObsError::Protocol(e.to_string())));
                                continue;
                            }
                        };
                        if let Err(e) = ws.send(Message::Text(text.into())).await {
                            let _ = reply.send(Err(ObsError::Connect(e.to_string())));
                            fail_all(&mut pending, ObsError::Closed);
                            return SessionEnd::Disconnected(e.to_string());
                        }
                        pending.insert(request_id, Pending { sent_at: Instant::now(), reply });
                    }
                }
            }
            message = ws.next() => {
                match message {
                    None => {
                        fail_all(&mut pending, ObsError::Closed);
                        return SessionEnd::Disconnected("connection closed".to_string());
                    }
                    Some(Err(e)) => {
                        fail_all(&mut pending, ObsError::Closed);
                        return SessionEnd::Disconnected(e.to_string());
                    }
                    Some(Ok(message)) => {
                        if let Some(incoming) = parse_incoming(&message) {
                            if incoming.op == op::EVENT {
                                let _ = sink; // unsolicited events are ignored (no subscriptions)
                            } else if incoming.op == op::REQUEST_RESPONSE {
                                handle_response(incoming.d, &mut pending);
                            }
                        }
                    }
                }
            }
        }
    }
}

fn handle_response(data: Value, pending: &mut HashMap<String, Pending>) {
    let response: protocol::RequestResponse = match serde_json::from_value(data) {
        Ok(r) => r,
        Err(_) => return,
    };
    let Some(entry) = pending.remove(&response.request_id) else {
        return;
    };
    let result = match response.request_status {
        Some(status) if !status.result => Err(ObsError::RequestFailed {
            request: response.request_type,
            detail: status
                .comment
                .filter(|c| !c.is_empty())
                .unwrap_or_else(|| format!("code {}", status.code)),
        }),
        _ => Ok(response.response_data.unwrap_or(Value::Null)),
    };
    let _ = entry.reply.send(result);
}

fn fail_all(pending: &mut HashMap<String, Pending>, error: ObsError) {
    for (_, entry) in pending.drain() {
        let _ = entry.reply.send(Err(match &error {
            ObsError::Closed => ObsError::Closed,
            other => ObsError::Protocol(other.to_string()),
        }));
    }
}

fn prune_expired(pending: &mut HashMap<String, Pending>) {
    let ttl = REQUEST_TIMEOUT * 2;
    pending.retain(|_, entry| entry.sent_at.elapsed() < ttl);
}

fn backoff_delay(attempt: u32) -> Duration {
    let base_ms: u64 = 500;
    let factor = 1u64 << attempt.min(6);
    let capped = base_ms.saturating_mul(factor).min(30_000);
    let jitter = getrandom::u64().unwrap_or(0) % 500;
    Duration::from_millis(capped + jitter)
}

/// Wait for `duration`, returning `true` if a shutdown was requested first.
async fn wait_or_shutdown(rx: &mut mpsc::Receiver<ObsCommand>, duration: Duration) -> bool {
    let sleep = tokio::time::sleep(duration);
    tokio::pin!(sleep);
    loop {
        tokio::select! {
            _ = &mut sleep => return false,
            command = rx.recv() => match command {
                None => return true,
                Some(ObsCommand::Shutdown) => return true,
                Some(ObsCommand::Request { reply, .. }) => {
                    // Not connected: fail the request rather than leaving it hanging.
                    let _ = reply.send(Err(ObsError::NotConnected));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_service_settings_and_redacts_key() {
        let value = json!({
            "streamServiceType": "rtmp_custom",
            "streamServiceSettings": {
                "server": "rtmp://live.tiktok.com/live/",
                "key": "super-secret-key"
            }
        });
        let settings = parse_service_settings(&value);
        assert_eq!(settings.service_type, "rtmp_custom");
        assert_eq!(settings.server, "rtmp://live.tiktok.com/live/");
        assert_eq!(settings.key.expose(), "super-secret-key");
        assert!(!format!("{settings:?}").contains("super-secret-key"));
    }

    #[test]
    fn parses_missing_service_settings_as_empty() {
        let settings = parse_service_settings(&json!({}));
        assert!(settings.service_type.is_empty());
        assert!(settings.server.is_empty());
        assert!(settings.key.is_empty());
    }

    #[test]
    fn backoff_is_bounded() {
        for attempt in 1..20 {
            let d = backoff_delay(attempt);
            assert!(d <= Duration::from_millis(30_500));
            assert!(d >= Duration::from_millis(500));
        }
    }

    #[test]
    fn default_settings_point_at_loopback() {
        let settings = ObsSettings::default();
        assert_eq!(settings.url(), "ws://127.0.0.1:4455");
    }
}

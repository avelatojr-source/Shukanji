//! Shared test helpers: a mock OBS WebSocket 5.x server, an event sink, and
//! event pumping for the coordinator.
#![allow(dead_code)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use tiktok_stream_tool::application::Coordinator;
use tiktok_stream_tool::domain::events::{AppEvent, EventSink};
use tiktok_stream_tool::integrations::obs::compute_auth;

/// Configuration for the mock OBS server.
#[derive(Clone)]
pub struct MockObsConfig {
    pub password: Option<String>,
    pub output_active: bool,
    /// When false, `SetStreamServiceSettings` is accepted but not persisted,
    /// so a read-back will mismatch (tests verification).
    pub echo_settings: bool,
    /// When false, the server never replies to requests (tests timeouts).
    pub respond: bool,
    /// When true, `GetStreamStatus` returns a failed requestStatus.
    pub fail_stream_status: bool,
    /// When true, the server closes right after Identified (tests reconnect).
    pub close_after_identified: bool,
}

impl Default for MockObsConfig {
    fn default() -> Self {
        Self {
            password: None,
            output_active: false,
            echo_settings: true,
            respond: true,
            fail_stream_status: false,
            close_after_identified: false,
        }
    }
}

/// Handle to a running mock server.
pub struct MockObs {
    pub port: u16,
    /// `Some(true)` = auth accepted, `Some(false)` = rejected, `None` = no
    /// Identify was received.
    pub auth_ok: Arc<Mutex<Option<bool>>>,
}

impl MockObs {
    pub fn auth_ok(&self) -> Option<bool> {
        *self.auth_ok.lock().unwrap()
    }
}

/// Start a mock OBS server on an ephemeral loopback port.
pub async fn spawn_mock_obs(config: MockObsConfig) -> MockObs {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let auth_ok = Arc::new(Mutex::new(None));
    let auth_ok_task = auth_ok.clone();

    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let Ok(mut ws) = tokio_tungstenite::accept_async(stream).await else {
            return;
        };

        const SALT: &str = "mock-salt";
        const CHALLENGE: &str = "mock-challenge";

        let hello = match &config.password {
            Some(_) => json!({
                "op": 0,
                "d": {
                    "obsWebSocketVersion": "5.5.2",
                    "rpcVersion": 1,
                    "authentication": { "challenge": CHALLENGE, "salt": SALT }
                }
            }),
            None => json!({
                "op": 0,
                "d": { "obsWebSocketVersion": "5.5.2", "rpcVersion": 1 }
            }),
        };
        if ws
            .send(Message::Text(hello.to_string().into()))
            .await
            .is_err()
        {
            return;
        }

        // Expect Identify.
        let Some(Ok(identify)) = ws.next().await else {
            return;
        };
        let Ok(identify) = serde_json::from_str::<Value>(identify.to_text().unwrap_or("")) else {
            return;
        };
        if identify["op"] != 1 {
            return;
        }
        let ok = match &config.password {
            Some(password) => {
                identify["d"]["authentication"].as_str()
                    == Some(compute_auth(password, CHALLENGE, SALT).as_str())
            }
            None => identify["d"].get("authentication").is_none(),
        };
        *auth_ok_task.lock().unwrap() = Some(ok);

        if !ok {
            // Real OBS drops the connection when authentication fails.
            let _ = ws.close(None).await;
            return;
        }

        if ws
            .send(Message::Text(
                json!({"op": 2, "d": {"negotiatedRpcVersion": 1}})
                    .to_string()
                    .into(),
            ))
            .await
            .is_err()
        {
            return;
        }

        if config.close_after_identified {
            let _ = ws.close(None).await;
            return;
        }

        let mut applied: Option<Value> = None;

        while let Some(Ok(message)) = ws.next().await {
            let Ok(value) = serde_json::from_str::<Value>(message.to_text().unwrap_or("")) else {
                continue;
            };
            if value["op"] != 6 {
                continue;
            }
            let request_type = value["d"]["requestType"].as_str().unwrap_or("").to_string();
            let request_id = value["d"]["requestId"].clone();

            if !config.respond {
                continue; // emulate a dropped response
            }

            let (response, failed) = match request_type.as_str() {
                "GetVersion" => (
                    json!({
                        "obsVersion": "31.0.0",
                        "obsWebSocketVersion": "5.5.2",
                        "rpcVersion": 1,
                        "availableRequests": [
                            "GetVersion",
                            "GetStreamStatus",
                            "GetStreamServiceSettings",
                            "SetStreamServiceSettings",
                            "StartStream",
                            "StopStream"
                        ]
                    }),
                    false,
                ),
                "GetStreamStatus" => (
                    json!({"outputActive": config.output_active, "outputReconnecting": false}),
                    config.fail_stream_status,
                ),
                "GetStreamServiceSettings" => (
                    applied
                        .clone()
                        .unwrap_or(json!({"streamServiceType": "", "streamServiceSettings": {}})),
                    false,
                ),
                "SetStreamServiceSettings" => {
                    if config.echo_settings {
                        let data = &value["d"]["requestData"];
                        applied = Some(json!({
                            "streamServiceType": data["streamServiceType"].clone(),
                            "streamServiceSettings": data["streamServiceSettings"].clone()
                        }));
                    }
                    (json!({}), false)
                }
                "StartStream" | "StopStream" => (json!({}), false),
                _ => (json!({}), true),
            };

            let payload = if failed {
                json!({
                    "op": 7,
                    "d": {
                        "requestType": request_type,
                        "requestId": request_id,
                        "requestStatus": {"result": false, "code": 604, "comment": "unsupported"}
                    }
                })
            } else {
                json!({
                    "op": 7,
                    "d": {
                        "requestType": request_type,
                        "requestId": request_id,
                        "requestStatus": {"result": true, "code": 100},
                        "responseData": response
                    }
                })
            };
            if ws
                .send(Message::Text(payload.to_string().into()))
                .await
                .is_err()
            {
                break;
            }
        }
    });

    MockObs { port, auth_ok }
}

/// An event sink that records everything it receives.
#[derive(Default)]
pub struct RecordingSink(pub Mutex<Vec<AppEvent>>);

impl RecordingSink {
    pub fn events(&self) -> Vec<AppEvent> {
        self.0.lock().unwrap().clone()
    }

    pub fn has(&self, pred: impl Fn(&AppEvent) -> bool) -> bool {
        self.0.lock().unwrap().iter().any(pred)
    }
}

impl EventSink for RecordingSink {
    fn emit(&self, event: AppEvent) {
        self.0.lock().unwrap().push(event);
    }
}

/// Drain coordinator events until `pred` matches or the timeout elapses.
pub async fn pump_until(
    coordinator: &mut Coordinator,
    seen: &mut Vec<AppEvent>,
    pred: impl Fn(&AppEvent) -> bool,
    timeout: Duration,
) -> bool {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        seen.extend(coordinator.poll_events());
        if seen.iter().any(&pred) {
            return true;
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

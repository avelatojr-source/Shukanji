//! End-to-end tests for the one-click go-live orchestration, driven by a mock
//! OBS WebSocket server and a mock Streamlabs HTTP server.

mod common;

use std::sync::Arc;
use std::time::Duration;

use common::{MockObsConfig, pump_until, spawn_mock_obs};
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use tiktok_stream_tool::application::Coordinator;
use tiktok_stream_tool::domain::Command;
use tiktok_stream_tool::domain::events::AppEvent;
use tiktok_stream_tool::domain::models::{AudienceType, SecretString};
use tiktok_stream_tool::domain::states::ApplyState;
use tiktok_stream_tool::platform::credentials::MemoryStore;

fn coordinator(base_url: String) -> Coordinator {
    let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
    Coordinator::new(wake, Arc::new(MemoryStore::new()), Some(base_url)).expect("coordinator")
}

async fn mount_start_ok(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/tiktok/stream/start"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "stream-1",
            "rtmp": "rtmp://live.tiktok.com/live/",
            "key": "secret-key-xyz"
        })))
        .mount(server)
        .await;
}

fn go_live_command(port: u16) -> Command {
    Command::GoLive {
        token: SecretString::new("test-token"),
        title: "Hello".to_string(),
        game_mask_id: "12345".to_string(),
        audience: AudienceType::General,
        host: "127.0.0.1".to_string(),
        port,
        password: None,
        remember_password: false,
        request_id: 1,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn go_live_connects_applies_and_starts() {
    let mock_obs = spawn_mock_obs(MockObsConfig::default()).await;
    let server = MockServer::start().await;
    mount_start_ok(&server).await;

    let mut coordinator = coordinator(server.uri());
    let mut seen = Vec::new();
    coordinator.send(go_live_command(mock_obs.port));

    assert!(
        pump_until(
            &mut coordinator,
            &mut seen,
            |e| matches!(e, AppEvent::GoLiveFinished { ok: true, .. }),
            Duration::from_secs(10)
        )
        .await,
        "no successful GoLiveFinished: {seen:?}"
    );
    assert!(
        seen.iter()
            .any(|e| matches!(e, AppEvent::CredentialsRetrieved { .. })),
        "credentials were not surfaced: {seen:?}"
    );
    assert!(
        seen.iter().any(|e| matches!(
            e,
            AppEvent::ObsApplyFinished {
                result: ApplyState::Applied { .. },
                ..
            }
        )),
        "settings were not applied: {seen:?}"
    );
    assert!(
        seen.iter()
            .any(|e| matches!(e, AppEvent::ObsStartFinished { ok: true, .. })),
        "OBS output was not started: {seen:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn go_live_outcome_unknown_does_not_touch_obs() {
    let mock_obs = spawn_mock_obs(MockObsConfig::default()).await;
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/tiktok/stream/start"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;

    let mut coordinator = coordinator(server.uri());
    let mut seen = Vec::new();
    coordinator.send(go_live_command(mock_obs.port));

    assert!(
        pump_until(
            &mut coordinator,
            &mut seen,
            |e| matches!(e, AppEvent::GoLiveFinished { ok: false, .. }),
            Duration::from_secs(10)
        )
        .await,
        "expected a failed GoLiveFinished: {seen:?}"
    );
    assert!(
        seen.iter()
            .any(|e| matches!(e, AppEvent::CredentialsOutcomeUnknown { .. })),
        "outcome-unknown was not surfaced: {seen:?}"
    );
    assert!(
        !seen
            .iter()
            .any(|e| matches!(e, AppEvent::ObsApplyFinished { .. })),
        "must not apply settings when the stream start outcome is unknown: {seen:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn go_live_refuses_when_obs_output_already_active() {
    let config = MockObsConfig {
        output_active: true,
        ..Default::default()
    };
    let mock_obs = spawn_mock_obs(config).await;
    let server = MockServer::start().await;
    mount_start_ok(&server).await;

    let mut coordinator = coordinator(server.uri());
    let mut seen = Vec::new();
    coordinator.send(go_live_command(mock_obs.port));

    assert!(
        pump_until(
            &mut coordinator,
            &mut seen,
            |e| matches!(e, AppEvent::GoLiveFinished { ok: false, .. }),
            Duration::from_secs(10)
        )
        .await,
        "expected a failed GoLiveFinished: {seen:?}"
    );
    assert!(
        seen.iter().any(|e| matches!(
            e,
            AppEvent::ObsApplyFinished {
                result: ApplyState::RefusedOutputActive,
                ..
            }
        )),
        "apply should have been refused: {seen:?}"
    );
    assert!(
        !seen
            .iter()
            .any(|e| matches!(e, AppEvent::ObsStartFinished { ok: true, .. })),
        "must not start when output is already active: {seen:?}"
    );
}

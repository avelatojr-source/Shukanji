//! OBS WebSocket client tests against a local mock server.

mod common;

use std::sync::Arc;
use std::time::Duration;

use common::{MockObs, MockObsConfig, RecordingSink, spawn_mock_obs};
use tiktok_stream_tool::domain::errors::ObsError;
use tiktok_stream_tool::domain::events::AppEvent;
use tiktok_stream_tool::integrations::obs::{ObsHandle, ObsSettings};

fn settings(port: u16) -> ObsSettings {
    ObsSettings {
        host: "127.0.0.1".to_string(),
        port,
    }
}

async fn wait_auth(mock: &MockObs, timeout: Duration) -> Option<bool> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if let Some(ok) = mock.auth_ok() {
            return Some(ok);
        }
        if tokio::time::Instant::now() >= deadline {
            return None;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

async fn wait_sink(
    sink: &RecordingSink,
    pred: impl Fn(&AppEvent) -> bool,
    timeout: Duration,
) -> bool {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if sink.has(&pred) {
            return true;
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn handshake_without_auth_then_get_version() {
    let mock = spawn_mock_obs(MockObsConfig::default()).await;
    let sink = Arc::new(RecordingSink::default());
    let handle = ObsHandle::spawn(settings(mock.port), None, sink.clone());

    let version = handle
        .get_version()
        .await
        .expect("GetVersion should succeed");
    assert_eq!(version.obs_version, "31.0.0");
    assert_eq!(version.rpc_version, 1);
    assert!(
        version
            .available_requests
            .iter()
            .any(|r| r == "SetStreamServiceSettings")
    );

    assert_eq!(wait_auth(&mock, Duration::from_secs(2)).await, Some(true));
    assert!(
        wait_sink(
            &sink,
            |e| matches!(e, AppEvent::ObsConnected { .. }),
            Duration::from_secs(2)
        )
        .await
    );

    handle.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn handshake_with_authentication() {
    let config = MockObsConfig {
        password: Some("s3cret-pw".to_string()),
        ..Default::default()
    };
    let mock = spawn_mock_obs(config).await;
    let sink = Arc::new(RecordingSink::default());
    let handle = ObsHandle::spawn(settings(mock.port), Some("s3cret-pw".into()), sink.clone());

    let version = handle
        .get_version()
        .await
        .expect("authenticated GetVersion");
    assert_eq!(version.obs_version, "31.0.0");
    assert_eq!(wait_auth(&mock, Duration::from_secs(2)).await, Some(true));

    handle.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn wrong_password_is_rejected() {
    let config = MockObsConfig {
        password: Some("correct".to_string()),
        ..Default::default()
    };
    let mock = spawn_mock_obs(config).await;
    let sink = Arc::new(RecordingSink::default());
    let handle = ObsHandle::spawn(settings(mock.port), Some("wrong".into()), sink.clone());

    // The client should fail to complete after the server drops the connection.
    let result = handle.get_version().await;
    assert!(result.is_err(), "expected failure with a wrong password");
    assert_eq!(wait_auth(&mock, Duration::from_secs(2)).await, Some(false));
    handle.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn missing_password_when_required_fails_before_identify() {
    let config = MockObsConfig {
        password: Some("needed".to_string()),
        ..Default::default()
    };
    let mock = spawn_mock_obs(config).await;
    let sink = Arc::new(RecordingSink::default());
    let handle = ObsHandle::spawn(settings(mock.port), None, sink.clone());

    // Client has no password but OBS requires one.
    let result = handle.get_version().await;
    assert!(result.is_err());
    // No Identify should have reached the server.
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(mock.auth_ok(), None);
    handle.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn concurrent_requests_are_correlated_by_id() {
    let mock = spawn_mock_obs(MockObsConfig::default()).await;
    let sink = Arc::new(RecordingSink::default());
    let handle = ObsHandle::spawn(settings(mock.port), None, sink.clone());

    let (version, status) = tokio::join!(handle.get_version(), handle.get_stream_status());
    let version = version.expect("version");
    let status = status.expect("status");
    assert_eq!(version.obs_version, "31.0.0");
    assert!(!status.output_active);
    handle.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn failed_request_status_is_surfaced() {
    let config = MockObsConfig {
        fail_stream_status: true,
        ..Default::default()
    };
    let mock = spawn_mock_obs(config).await;
    let sink = Arc::new(RecordingSink::default());
    let handle = ObsHandle::spawn(settings(mock.port), None, sink.clone());

    let err = handle
        .get_stream_status()
        .await
        .expect_err("failed status should error");
    assert!(matches!(err, ObsError::RequestFailed { .. }));
    if let ObsError::RequestFailed { request, detail } = err {
        assert_eq!(request, "GetStreamStatus");
        assert!(!detail.is_empty());
    }
    handle.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn request_times_out_when_no_response() {
    let config = MockObsConfig {
        respond: false,
        ..Default::default()
    };
    let mock = spawn_mock_obs(config).await;
    let sink = Arc::new(RecordingSink::default());
    let handle = ObsHandle::spawn_with_timeout(
        settings(mock.port),
        None,
        sink.clone(),
        Duration::from_millis(300),
    );

    let err = handle.get_version().await.expect_err("should time out");
    assert!(matches!(err, ObsError::Timeout(_)));
    handle.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn disconnect_emits_events() {
    let config = MockObsConfig {
        close_after_identified: true,
        ..Default::default()
    };
    let mock = spawn_mock_obs(config).await;
    let sink = Arc::new(RecordingSink::default());
    let handle = ObsHandle::spawn(settings(mock.port), None, sink.clone());

    let saw = wait_sink(
        &sink,
        |e| {
            matches!(
                e,
                AppEvent::ObsDisconnected { .. } | AppEvent::ObsReconnecting { .. }
            )
        },
        Duration::from_secs(3),
    )
    .await;
    assert!(saw, "expected a disconnect or reconnect event");
    handle.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn service_settings_round_trip() {
    let mock = spawn_mock_obs(MockObsConfig::default()).await;
    let sink = Arc::new(RecordingSink::default());
    let handle = ObsHandle::spawn(settings(mock.port), None, sink.clone());

    let key = "stream-key-abc".into();
    handle
        .set_stream_service_settings("rtmp://live.tiktok.com/live/", &key)
        .await
        .expect("set settings");

    let read_back = handle
        .get_stream_service_settings()
        .await
        .expect("read back");
    assert_eq!(read_back.service_type, "rtmp_custom");
    assert_eq!(read_back.server, "rtmp://live.tiktok.com/live/");
    assert_eq!(read_back.key.expose(), "stream-key-abc");
    handle.shutdown();
}

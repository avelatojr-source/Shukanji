//! End-to-end coordinator tests for the OBS apply flow, driven by a mock OBS.

mod common;

use std::sync::Arc;
use std::time::Duration;

use common::{MockObsConfig, pump_until, spawn_mock_obs};
use tiktok_stream_tool::application::Coordinator;
use tiktok_stream_tool::domain::Command;
use tiktok_stream_tool::domain::events::AppEvent;
use tiktok_stream_tool::domain::states::ApplyState;
use tiktok_stream_tool::platform::credentials::MemoryStore;

fn coordinator() -> Coordinator {
    let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
    Coordinator::new(wake, Arc::new(MemoryStore::new()), None).expect("coordinator")
}

fn apply_result(events: &[AppEvent]) -> Option<ApplyState> {
    events.iter().rev().find_map(|e| match e {
        AppEvent::ObsApplyFinished { result, .. } => Some(result.clone()),
        _ => None,
    })
}

async fn connect(coordinator: &mut Coordinator, seen: &mut Vec<AppEvent>, port: u16) {
    coordinator.send(Command::ObsConnect {
        host: "127.0.0.1".to_string(),
        port,
        password: None,
        remember_password: false,
    });
    assert!(
        pump_until(
            coordinator,
            seen,
            |e| matches!(e, AppEvent::ObsConnected { .. }),
            Duration::from_secs(5)
        )
        .await,
        "did not observe ObsConnected"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn apply_settings_succeeds_and_is_verified() {
    let mock = spawn_mock_obs(MockObsConfig::default()).await;
    let mut coordinator = coordinator();
    let mut seen = Vec::new();
    connect(&mut coordinator, &mut seen, mock.port).await;

    coordinator.send(Command::ObsApplyStreamSettings {
        server: "rtmp://live.tiktok.com/live/".to_string(),
        key: "key-123456".into(),
        request_id: 1,
    });

    let matched = pump_until(
        &mut coordinator,
        &mut seen,
        |e| matches!(e, AppEvent::ObsApplyFinished { .. }),
        Duration::from_secs(5),
    )
    .await;
    assert!(matched, "no apply result observed");
    assert_eq!(
        apply_result(&seen),
        Some(ApplyState::Applied {
            server: "rtmp://live.tiktok.com/live/".to_string()
        })
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn apply_is_refused_while_output_is_active() {
    let config = MockObsConfig {
        output_active: true,
        ..Default::default()
    };
    let mock = spawn_mock_obs(config).await;
    let mut coordinator = coordinator();
    let mut seen = Vec::new();
    connect(&mut coordinator, &mut seen, mock.port).await;

    coordinator.send(Command::ObsApplyStreamSettings {
        server: "rtmp://live.tiktok.com/live/".to_string(),
        key: "key-123456".into(),
        request_id: 1,
    });

    assert!(
        pump_until(
            &mut coordinator,
            &mut seen,
            |e| matches!(e, AppEvent::ObsApplyFinished { .. }),
            Duration::from_secs(5)
        )
        .await
    );
    assert_eq!(apply_result(&seen), Some(ApplyState::RefusedOutputActive));
}

#[tokio::test(flavor = "multi_thread")]
async fn apply_detects_read_back_mismatch() {
    let config = MockObsConfig {
        echo_settings: false,
        ..Default::default()
    };
    let mock = spawn_mock_obs(config).await;
    let mut coordinator = coordinator();
    let mut seen = Vec::new();
    connect(&mut coordinator, &mut seen, mock.port).await;

    coordinator.send(Command::ObsApplyStreamSettings {
        server: "rtmp://live.tiktok.com/live/".to_string(),
        key: "key-123456".into(),
        request_id: 1,
    });

    assert!(
        pump_until(
            &mut coordinator,
            &mut seen,
            |e| matches!(e, AppEvent::ObsApplyFinished { .. }),
            Duration::from_secs(5)
        )
        .await
    );
    assert!(matches!(
        apply_result(&seen),
        Some(ApplyState::Mismatch { .. })
    ));
    // The mismatch detail must never contain the key.
    let detail = seen.iter().find_map(|e| match e {
        AppEvent::ObsApplyFinished {
            result: ApplyState::Mismatch { detail },
            ..
        } => Some(detail.clone()),
        _ => None,
    });
    assert!(!detail.unwrap_or_default().contains("key-123456"));
}

/// The manual RTMP path applies to OBS without any Streamlabs involvement.
#[tokio::test(flavor = "multi_thread")]
async fn manual_credentials_apply_without_streamlabs() {
    let mock = spawn_mock_obs(MockObsConfig::default()).await;
    let mut coordinator = coordinator();
    let mut seen = Vec::new();
    connect(&mut coordinator, &mut seen, mock.port).await;

    coordinator.send(Command::ObsApplyStreamSettings {
        server: "rtmp://my-own-server/live".to_string(),
        key: "manual-key".into(),
        request_id: 7,
    });

    assert!(
        pump_until(
            &mut coordinator,
            &mut seen,
            |e| matches!(
                e,
                AppEvent::ObsApplyFinished {
                    result: ApplyState::Applied { .. },
                    ..
                }
            ),
            Duration::from_secs(5)
        )
        .await
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn apply_without_connection_fails_cleanly() {
    let mut coordinator = coordinator();
    let mut seen = Vec::new();

    coordinator.send(Command::ObsApplyStreamSettings {
        server: "rtmp://live.tiktok.com/live/".to_string(),
        key: "key".into(),
        request_id: 1,
    });

    assert!(
        pump_until(
            &mut coordinator,
            &mut seen,
            |e| matches!(e, AppEvent::ObsApplyFinished { .. }),
            Duration::from_secs(3)
        )
        .await
    );
    assert!(matches!(
        apply_result(&seen),
        Some(ApplyState::Failed { .. })
    ));
}

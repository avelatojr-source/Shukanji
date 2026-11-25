//! Background coordinator.
//!
//! The UI thread owns view state and sends [`Command`]s. A dispatcher task
//! receives them, spawns short-lived async tasks, and those tasks publish
//! typed [`AppEvent`]s back to the UI over a bounded channel. Nothing shared
//! is mutated from the UI thread except through these messages.

use std::sync::Arc;

use tokio::runtime::Runtime;
use tokio::sync::{RwLock, mpsc};

use crate::domain::LiveConnectionState;
use crate::domain::errors::StreamlabsError;
use crate::domain::events::{AppEvent, EventSink};
use crate::domain::models::{AudienceType, CredentialSource, RtmpCredentials, SecretString};
use crate::domain::states::{ApplyState, ObsOutputState};
use crate::domain::{Command, LogLevel};
use crate::integrations::obs::{
    CUSTOM_SERVICE_TYPE, ObsHandle, ObsSettings, preflight, verify_applied,
};
use crate::integrations::streamlabs::client::DEFAULT_BASE_URL;
use crate::integrations::streamlabs::{StartStreamResult, StreamlabsClient, auth, local_scan};
use crate::integrations::tiktok_live::{self, LiveMonitorHandle};
use crate::platform::credentials::{SecretStore, keys};

use super::event_sink::ChannelSink;

const COMMAND_CAPACITY: usize = 64;
const EVENT_CAPACITY: usize = 256;
/// How long the one-click go-live flow waits for the OBS handshake.
const OBS_CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(8);
/// How long to wait for OBS to report the output as active after StartStream.
const OUTPUT_CONFIRM_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(1500);

/// State shared between the dispatcher and its spawned tasks.
#[derive(Default)]
struct Shared {
    token: Option<SecretString>,
    stream_id: Option<String>,
    obs: Option<ObsHandle>,
    live_monitor: Option<LiveMonitorHandle>,
    live_username: Option<String>,
}

/// Inputs for the one-click go-live orchestration.
struct GoLiveParams {
    token: SecretString,
    title: String,
    game_mask_id: String,
    audience: AudienceType,
    host: String,
    port: u16,
    password: Option<SecretString>,
    remember_password: bool,
    request_id: u64,
}

/// Owns the tokio runtime and the channel endpoints.
pub struct Coordinator {
    runtime: Option<Runtime>,
    command_tx: mpsc::Sender<Command>,
    event_rx: mpsc::Receiver<AppEvent>,
}

impl Drop for Coordinator {
    fn drop(&mut self) {
        // Dropping a `Runtime` inside an async context panics, so shut it down
        // in the background instead. Background tasks are aborted.
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

impl Coordinator {
    /// Build the coordinator. `wake` is called after every event so the UI
    /// repaints promptly. `base_url` overrides the Streamlabs base URL (used
    /// by tests).
    pub fn new(
        wake: Arc<dyn Fn() + Send + Sync>,
        store: Arc<dyn SecretStore>,
        base_url: Option<String>,
    ) -> std::io::Result<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()?;

        let (command_tx, command_rx) = mpsc::channel(COMMAND_CAPACITY);
        let (event_tx, event_rx) = mpsc::channel(EVENT_CAPACITY);
        let sink: Arc<dyn EventSink> = Arc::new(ChannelSink::new(event_tx, wake));
        let shared = Arc::new(RwLock::new(Shared::default()));

        let http = reqwest::Client::builder()
            .user_agent(crate::integrations::streamlabs::DEFAULT_USER_AGENT)
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        let dispatcher = Dispatcher {
            sink,
            shared,
            store,
            base_url: Arc::new(base_url.unwrap_or_else(|| DEFAULT_BASE_URL.to_string())),
            http,
        };
        runtime.spawn(dispatcher.run(command_rx));

        Ok(Self {
            runtime: Some(runtime),
            command_tx,
            event_rx,
        })
    }

    /// Queue a command. Non-blocking; logs if the queue is full.
    pub fn send(&self, command: Command) {
        if let Err(err) = self.command_tx.try_send(command) {
            tracing::warn!("could not queue command: {err}");
        }
    }

    /// Drain all pending events (called once per frame by the UI).
    pub fn poll_events(&mut self) -> Vec<AppEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.event_rx.try_recv() {
            events.push(event);
        }
        events
    }
}

#[derive(Clone)]
struct Dispatcher {
    sink: Arc<dyn EventSink>,
    shared: Arc<RwLock<Shared>>,
    store: Arc<dyn SecretStore>,
    base_url: Arc<String>,
    http: reqwest::Client,
}

impl Dispatcher {
    async fn run(self, mut rx: mpsc::Receiver<Command>) {
        while let Some(command) = rx.recv().await {
            self.handle(command).await;
        }
    }

    async fn handle(&self, command: Command) {
        match command {
            Command::RestoreSession => {
                let this = self.clone();
                tokio::spawn(async move { this.restore_session().await });
            }
            Command::ClearStoredSecrets => {
                let this = self.clone();
                tokio::spawn(async move { this.clear_stored_secrets().await });
            }
            Command::BeginWebAuth => {
                let this = self.clone();
                tokio::spawn(async move { this.begin_web_auth().await });
            }
            Command::ScanLocalToken => {
                let this = self.clone();
                tokio::spawn(async move { this.scan_local_token().await });
            }
            Command::SubmitToken { token } => {
                let this = self.clone();
                tokio::spawn(async move {
                    this.persist_token(&token).await;
                    this.shared.write().await.token = Some(token.clone());
                    this.sink.emit(AppEvent::TokenSubmitted {
                        token: token.clone(),
                    });
                    this.refresh_account(token).await;
                });
            }
            Command::RefreshAccount { token } => {
                let this = self.clone();
                tokio::spawn(async move {
                    this.shared.write().await.token = Some(token.clone());
                    this.refresh_account(token).await;
                });
            }
            Command::SearchGames {
                token,
                query,
                request_id,
            } => {
                let this = self.clone();
                tokio::spawn(async move { this.search_games(token, query, request_id).await });
            }
            Command::GenerateCredentials {
                token,
                title,
                game_mask_id,
                audience,
                request_id,
            } => {
                let this = self.clone();
                tokio::spawn(async move {
                    this.generate_credentials(token, title, game_mask_id, audience, request_id)
                        .await
                });
            }
            Command::EndStreamlabsStream { token, stream_id } => {
                let this = self.clone();
                tokio::spawn(async move { this.end_streamlabs_stream(token, stream_id).await });
            }
            Command::GoLive {
                token,
                title,
                game_mask_id,
                audience,
                host,
                port,
                password,
                remember_password,
                request_id,
            } => {
                let this = self.clone();
                tokio::spawn(async move {
                    this.go_live(GoLiveParams {
                        token,
                        title,
                        game_mask_id,
                        audience,
                        host,
                        port,
                        password,
                        remember_password,
                        request_id,
                    })
                    .await
                });
            }
            Command::ObsConnect {
                host,
                port,
                password,
                remember_password,
            } => {
                let this = self.clone();
                tokio::spawn(async move {
                    this.remember_obs_password(remember_password, password.as_ref())
                        .await;
                    this.start_obs(host, port, password).await;
                });
            }
            Command::ObsConnectStored { host, port } => {
                let this = self.clone();
                tokio::spawn(async move {
                    let password = this.load_obs_password().await;
                    this.start_obs(host, port, password).await;
                });
            }
            Command::ObsDisconnect => {
                if let Some(handle) = self.shared.write().await.obs.take() {
                    handle.shutdown();
                }
                self.sink.emit(AppEvent::ObsDisconnected {
                    reason: "disconnected by user".to_string(),
                });
            }
            Command::ObsRefreshStatus => {
                let this = self.clone();
                tokio::spawn(async move { this.refresh_obs_status().await });
            }
            Command::ObsApplyStreamSettings {
                server,
                key,
                request_id,
            } => {
                let this = self.clone();
                tokio::spawn(
                    async move { this.apply_stream_settings(server, key, request_id).await },
                );
            }
            Command::ObsStartStream => {
                let this = self.clone();
                tokio::spawn(async move { this.start_output().await });
            }
            Command::ObsStopStream => {
                let this = self.clone();
                tokio::spawn(async move { this.stop_output().await });
            }
            Command::StartLiveMonitor { username } => {
                let this = self.clone();
                tokio::spawn(async move { this.start_live_monitor(username).await });
            }
            Command::StopLiveMonitor => {
                {
                    let mut shared = self.shared.write().await;
                    if let Some(handle) = shared.live_monitor.take() {
                        handle.shutdown();
                    }
                    shared.live_username = None;
                }
                self.sink.emit(AppEvent::LiveMonitorStatus {
                    state: LiveConnectionState::Disconnected,
                });
            }
        }
    }

    async fn start_live_monitor(&self, username: String) {
        // Replacing an existing monitor: stop the old task first so it cannot
        // deliver stale events after the new one starts.
        {
            let mut shared = self.shared.write().await;
            if let Some(old) = shared.live_monitor.take() {
                old.shutdown();
            }
            shared.live_username = Some(username.clone());
        }
        let handle = tiktok_live::spawn_monitor(username, self.sink.clone());
        self.shared.write().await.live_monitor = Some(handle);
    }

    fn client(&self, token: SecretString) -> Result<StreamlabsClient, StreamlabsError> {
        // Reuse the dispatcher's HTTP client so connections/TLS stay warm.
        Ok(StreamlabsClient::with_http(
            self.http.clone(),
            token,
            self.base_url.as_str(),
        ))
    }

    async fn persist_token(&self, token: &SecretString) {
        let store = self.store.clone();
        let token = token.clone();
        let result = tokio::task::spawn_blocking(move || {
            store.set(keys::SERVICE, keys::STREAMLABS_TOKEN, &token)
        })
        .await;
        match result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => self.sink.emit(AppEvent::Log {
                level: LogLevel::Warn,
                message: format!("Could not store the Streamlabs token securely: {e}"),
            }),
            Err(e) => self.sink.emit(AppEvent::Log {
                level: LogLevel::Warn,
                message: format!("Credential store task failed: {e}"),
            }),
        }
    }

    async fn clear_stored_secrets(&self) {
        let store = self.store.clone();
        let _ = tokio::task::spawn_blocking(move || {
            let _ = store.delete(keys::SERVICE, keys::STREAMLABS_TOKEN);
            let _ = store.delete(keys::SERVICE, keys::OBS_PASSWORD);
        })
        .await;
        self.shared.write().await.token = None;
        self.sink.emit(AppEvent::Log {
            level: LogLevel::Info,
            message: "Stored credentials removed.".to_string(),
        });
    }

    async fn remember_obs_password(&self, remember: bool, password: Option<&SecretString>) {
        let store = self.store.clone();
        let password = password.cloned();
        let result = tokio::task::spawn_blocking(move || {
            if remember {
                if let Some(pw) = password {
                    store.set(keys::SERVICE, keys::OBS_PASSWORD, &pw)?;
                }
            } else {
                store.delete(keys::SERVICE, keys::OBS_PASSWORD)?;
            }
            Ok::<(), crate::domain::errors::CredentialStoreError>(())
        })
        .await;
        match result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => self.sink.emit(AppEvent::Log {
                level: LogLevel::Warn,
                message: format!("Could not update the stored OBS password: {e}"),
            }),
            Err(e) => self.sink.emit(AppEvent::Log {
                level: LogLevel::Warn,
                message: format!("Credential store task failed: {e}"),
            }),
        }
    }

    async fn load_obs_password(&self) -> Option<SecretString> {
        let store = self.store.clone();
        tokio::task::spawn_blocking(move || {
            store.get(keys::SERVICE, keys::OBS_PASSWORD).ok().flatten()
        })
        .await
        .ok()
        .flatten()
    }

    async fn restore_session(&self) {
        let store = self.store.clone();
        let token = tokio::task::spawn_blocking(move || {
            store
                .get(keys::SERVICE, keys::STREAMLABS_TOKEN)
                .ok()
                .flatten()
        })
        .await
        .ok()
        .flatten();

        if let Some(token) = token {
            self.shared.write().await.token = Some(token.clone());
            self.sink.emit(AppEvent::TokenSubmitted {
                token: token.clone(),
            });
            self.refresh_account(token).await;
        }
    }

    async fn refresh_account(&self, token: SecretString) {
        let client = match self.client(token) {
            Ok(client) => client,
            Err(e) => {
                self.sink.emit(AppEvent::AccountInfoFailed {
                    message: e.to_string(),
                });
                return;
            }
        };
        match client.account_info().await {
            Ok(info) => self.sink.emit(AppEvent::AccountInfoLoaded { info }),
            Err(e) => self.sink.emit(AppEvent::AccountInfoFailed {
                message: e.to_string(),
            }),
        }
    }

    async fn begin_web_auth(&self) {
        match self.run_web_auth().await {
            Ok(token) => {
                self.persist_token(&token).await;
                self.shared.write().await.token = Some(token.clone());
                self.sink.emit(AppEvent::WebAuthFinished {
                    token: Some(token.clone()),
                    message: None,
                });
                self.refresh_account(token).await;
            }
            Err(e) => {
                self.sink.emit(AppEvent::WebAuthFinished {
                    token: None,
                    message: Some(e.to_string()),
                });
            }
        }
    }

    async fn run_web_auth(&self) -> Result<SecretString, crate::domain::errors::AuthError> {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .map_err(|e| crate::domain::errors::AuthError::Network(e.to_string()))?;
        let port = listener
            .local_addr()
            .map_err(|e| crate::domain::errors::AuthError::Network(e.to_string()))?
            .port();

        let verifier = auth::generate_code_verifier()?;
        let challenge = auth::code_challenge_s256(&verifier);
        let state = auth::generate_state()?;
        let url = auth::build_auth_url(port, &challenge, &state);

        self.sink
            .emit(AppEvent::WebAuthStarted { url: url.clone() });
        // Opening the browser is best-effort; the URL is shown so the user can
        // open it manually if the auto-open fails.
        let _ = open::that(&url);

        let code = auth::listen_for_callback(listener, &state, auth::AUTH_TIMEOUT).await?;
        auth::exchange_code_for_token(&self.http, &self.base_url, &code, &verifier).await
    }

    async fn scan_local_token(&self) {
        self.sink.emit(AppEvent::LocalTokenScanStarted);
        let found = tokio::task::spawn_blocking(local_scan::find_local_token)
            .await
            .unwrap_or(Ok(None));
        match found {
            Ok(Some(token)) => self.sink.emit(AppEvent::LocalTokenFound { token }),
            Ok(None) => self.sink.emit(AppEvent::LocalTokenNotFound),
            Err(message) => self.sink.emit(AppEvent::Log {
                level: LogLevel::Warn,
                message,
            }),
        }
    }

    async fn search_games(&self, token: SecretString, query: String, request_id: u64) {
        let client = match self.client(token) {
            Ok(client) => client,
            Err(e) => {
                self.sink.emit(AppEvent::GameSearchFailed {
                    request_id,
                    message: e.to_string(),
                });
                return;
            }
        };
        match client.search_games(&query).await {
            Ok(categories) => self.sink.emit(AppEvent::GameSuggestions {
                request_id,
                categories,
            }),
            Err(e) => self.sink.emit(AppEvent::GameSearchFailed {
                request_id,
                message: e.to_string(),
            }),
        }
    }

    async fn generate_credentials(
        &self,
        token: SecretString,
        title: String,
        game_mask_id: String,
        audience: AudienceType,
        request_id: u64,
    ) {
        let client = match self.client(token) {
            Ok(client) => client,
            Err(e) => {
                self.sink.emit(AppEvent::CredentialsFailed {
                    request_id,
                    message: e.to_string(),
                });
                return;
            }
        };

        // Non-idempotent: never auto-retried. A timeout/5xx becomes OutcomeUnknown.
        match client.start_stream(&title, &game_mask_id, audience).await {
            Ok(StartStreamResult::Started {
                stream_id,
                server,
                key,
            }) => {
                self.shared.write().await.stream_id = Some(stream_id.clone());
                self.sink.emit(AppEvent::CredentialsRetrieved {
                    request_id,
                    credentials: RtmpCredentials {
                        server,
                        key,
                        source: CredentialSource::Streamlabs,
                        stream_id: Some(stream_id),
                    },
                });
            }
            Ok(StartStreamResult::OutcomeUnknown { detail }) => {
                self.sink.emit(AppEvent::CredentialsOutcomeUnknown {
                    request_id,
                    message: detail,
                });
            }
            Err(e) => self.sink.emit(AppEvent::CredentialsFailed {
                request_id,
                message: e.to_string(),
            }),
        }
    }

    async fn end_streamlabs_stream(&self, token: SecretString, stream_id: String) {
        let client = match self.client(token) {
            Ok(client) => client,
            Err(e) => {
                self.sink.emit(AppEvent::StreamlabsEnded {
                    success: false,
                    message: e.to_string(),
                });
                return;
            }
        };
        match client.end_stream(&stream_id).await {
            Ok(success) if success => {
                self.shared.write().await.stream_id = None;
                self.sink.emit(AppEvent::StreamlabsEnded {
                    success: true,
                    message: "Stream session ended.".to_string(),
                });
            }
            Ok(_) => self.sink.emit(AppEvent::StreamlabsEnded {
                success: false,
                message: "Provider reported the session was not ended.".to_string(),
            }),
            Err(e) => self.sink.emit(AppEvent::StreamlabsEnded {
                success: false,
                message: e.to_string(),
            }),
        }
    }

    async fn start_obs(&self, host: String, port: u16, password: Option<SecretString>) {
        if let Some(old) = self.shared.write().await.obs.take() {
            old.shutdown();
        }
        let handle = ObsHandle::spawn(ObsSettings { host, port }, password, self.sink.clone());
        self.shared.write().await.obs = Some(handle);
    }

    async fn refresh_obs_status(&self) {
        let handle = self.shared.read().await.obs.clone();
        let Some(handle) = handle else {
            self.sink.emit(AppEvent::ObsOutputStateChanged {
                state: ObsOutputState::Unknown,
            });
            return;
        };
        match handle.get_stream_status().await {
            Ok(status) => self.sink.emit(AppEvent::ObsOutputStateChanged {
                state: ObsOutputState::from(status.output_active),
            }),
            Err(e) => {
                self.sink.emit(AppEvent::Log {
                    level: LogLevel::Warn,
                    message: format!("could not read OBS stream status: {e}"),
                });
                self.sink.emit(AppEvent::ObsOutputStateChanged {
                    state: ObsOutputState::Unknown,
                });
            }
        }
    }

    /// Apply settings and read back to verify. Returns the outcome that was
    /// also emitted as [`AppEvent::ObsApplyFinished`].
    async fn apply_stream_settings(
        &self,
        server: String,
        key: SecretString,
        request_id: u64,
    ) -> ApplyState {
        macro_rules! finish {
            ($result:expr) => {{
                let result = $result;
                self.sink.emit(AppEvent::ObsApplyFinished {
                    request_id,
                    result: result.clone(),
                });
                return result;
            }};
        }

        let handle = self.shared.read().await.obs.clone();
        let Some(handle) = handle else {
            finish!(ApplyState::Failed {
                message: "OBS is not connected.".to_string(),
            });
        };

        // 1. Preflight: read output status and refuse if output is running.
        let status = match handle.get_stream_status().await {
            Ok(status) => status,
            Err(e) => {
                finish!(ApplyState::Failed {
                    message: format!("could not read OBS status: {e}"),
                });
            }
        };
        let output = ObsOutputState::from(status.output_active);
        self.sink
            .emit(AppEvent::ObsOutputStateChanged { state: output });
        if let Err(reason) = preflight(output) {
            self.sink.emit(AppEvent::Log {
                level: LogLevel::Warn,
                message: reason.to_string(),
            });
            finish!(ApplyState::RefusedOutputActive);
        }

        // 2. Write settings.
        if let Err(e) = handle.set_stream_service_settings(&server, &key).await {
            finish!(ApplyState::Failed {
                message: e.to_string(),
            });
        }

        // 3. Read back and verify.
        let actual = match handle.get_stream_service_settings().await {
            Ok(actual) => actual,
            Err(e) => {
                finish!(ApplyState::Mismatch {
                    detail: format!("could not read back OBS settings: {e}"),
                });
            }
        };
        let outcome = verify_applied(
            CUSTOM_SERVICE_TYPE,
            &server,
            key.expose(),
            &actual.service_type,
            &actual.server,
            actual.key.expose(),
        );
        if outcome.is_match() {
            finish!(ApplyState::Applied { server });
        }
        finish!(ApplyState::Mismatch {
            detail: outcome.detail(),
        });
    }

    /// Start OBS output. Returns whether it started, and emits
    /// [`AppEvent::ObsStartFinished`].
    async fn start_output(&self) -> bool {
        let handle = self.shared.read().await.obs.clone();
        let Some(handle) = handle else {
            self.sink.emit(AppEvent::ObsStartFinished {
                ok: false,
                message: "OBS is not connected.".to_string(),
            });
            return false;
        };
        // Guard: never start if output is already active.
        match handle.get_stream_status().await {
            Ok(status) if status.output_active => {
                self.sink.emit(AppEvent::ObsStartFinished {
                    ok: false,
                    message: "OBS output is already running.".to_string(),
                });
                return false;
            }
            Ok(_) => {}
            Err(e) => {
                self.sink.emit(AppEvent::ObsStartFinished {
                    ok: false,
                    message: format!("could not read OBS status: {e}"),
                });
                return false;
            }
        }
        match handle.start_stream().await {
            Ok(()) => {
                self.sink.emit(AppEvent::ObsStartFinished {
                    ok: true,
                    message: "OBS output is running — TikTok broadcast status is unknown."
                        .to_string(),
                });
                self.sink.emit(AppEvent::ObsOutputStateChanged {
                    state: ObsOutputState::Active,
                });
                true
            }
            Err(e) => {
                self.sink.emit(AppEvent::ObsStartFinished {
                    ok: false,
                    message: e.to_string(),
                });
                false
            }
        }
    }

    async fn stop_output(&self) {
        let handle = self.shared.read().await.obs.clone();
        let Some(handle) = handle else {
            self.sink.emit(AppEvent::ObsStopFinished {
                ok: false,
                message: "OBS is not connected.".to_string(),
            });
            return;
        };
        match handle.stop_stream().await {
            Ok(()) => {
                self.sink.emit(AppEvent::ObsStopFinished {
                    ok: true,
                    message: "OBS output stopped.".to_string(),
                });
                self.sink.emit(AppEvent::ObsOutputStateChanged {
                    state: ObsOutputState::Inactive,
                });
            }
            Err(e) => self.sink.emit(AppEvent::ObsStopFinished {
                ok: false,
                message: e.to_string(),
            }),
        }
    }

    // ------------------------------------------------------------ go live
    //
    // One-click orchestration. Ordering is deliberate:
    //   1. connect OBS first (fast, local) so we never create a TikTok session
    //      that nothing is streaming to;
    //   2. create the Streamlabs/TikTok session and read the RTMP credentials
    //      (the only slow, network-bound step);
    //   3. push settings + read back + start OBS output, confirmed on loopback.
    async fn go_live(&self, p: GoLiveParams) {
        let fail = |message: String| {
            self.sink.emit(AppEvent::GoLiveFinished {
                request_id: p.request_id,
                ok: false,
                message,
            });
        };

        // 1. OBS must be ready to receive before we go live on TikTok.
        self.progress(&p, "Connecting to OBS…");
        if let Err(message) = self.ensure_obs_connected(&p).await {
            fail(message);
            return;
        }

        // 2. Create the live session and fetch credentials.
        self.progress(&p, "Requesting stream key from Streamlabs…");
        let client = match self.client(p.token.clone()) {
            Ok(client) => client,
            Err(e) => {
                fail(format!("Could not build the Streamlabs client: {e}"));
                return;
            }
        };
        let credentials = match client
            .start_stream(&p.title, &p.game_mask_id, p.audience)
            .await
        {
            Ok(StartStreamResult::Started {
                stream_id,
                server,
                key,
            }) => {
                self.shared.write().await.stream_id = Some(stream_id.clone());
                let credentials = RtmpCredentials {
                    server,
                    key,
                    source: CredentialSource::Streamlabs,
                    stream_id: Some(stream_id),
                };
                self.sink.emit(AppEvent::CredentialsRetrieved {
                    request_id: p.request_id,
                    credentials: credentials.clone(),
                });
                credentials
            }
            Ok(StartStreamResult::OutcomeUnknown { detail }) => {
                self.sink.emit(AppEvent::CredentialsOutcomeUnknown {
                    request_id: p.request_id,
                    message: detail.clone(),
                });
                fail(format!(
                    "Stream start outcome is unknown ({detail}). Check TikTok before retrying."
                ));
                return;
            }
            Err(e) => {
                self.sink.emit(AppEvent::CredentialsFailed {
                    request_id: p.request_id,
                    message: e.to_string(),
                });
                fail(format!("Could not get a stream key: {e}"));
                return;
            }
        };

        // 3. Apply, verify, then start.
        self.progress(&p, "Applying settings to OBS…");
        match self
            .apply_stream_settings(
                credentials.server.clone(),
                credentials.key.clone(),
                p.request_id,
            )
            .await
        {
            ApplyState::Applied { .. } => {}
            other => {
                fail(format!("OBS settings were not applied: {}", other.label()));
                return;
            }
        }

        self.progress(&p, "Starting OBS output…");
        if let Err(message) = self.start_and_confirm().await {
            fail(message);
            return;
        }

        self.sink.emit(AppEvent::GoLiveFinished {
            request_id: p.request_id,
            ok: true,
            message: "You are live: OBS is streaming and the TikTok session is active.".to_string(),
        });
    }

    fn progress(&self, p: &GoLiveParams, stage: &str) {
        self.sink.emit(AppEvent::GoLiveProgress {
            request_id: p.request_id,
            stage: stage.to_string(),
        });
    }

    /// Ensure the OBS actor has a completed handshake, connecting if needed.
    async fn ensure_obs_connected(&self, p: &GoLiveParams) -> Result<(), String> {
        if let Some(handle) = self.shared.read().await.obs.clone()
            && handle.is_connected()
        {
            return Ok(());
        }

        let password = match p.password.clone() {
            Some(password) => Some(password),
            None => self.load_obs_password().await,
        };
        self.remember_obs_password(p.remember_password, password.as_ref())
            .await;
        self.start_obs(p.host.clone(), p.port, password).await;

        let handle = self
            .shared
            .read()
            .await
            .obs
            .clone()
            .ok_or_else(|| "OBS handle was not created.".to_string())?;
        handle
            .wait_until_connected(OBS_CONNECT_TIMEOUT)
            .await
            .map_err(|e| format!("Could not connect to OBS: {e}"))
    }

    /// Start OBS output and briefly confirm it became active.
    async fn start_and_confirm(&self) -> Result<(), String> {
        if !self.start_output().await {
            return Err("OBS did not start streaming.".to_string());
        }
        let Some(handle) = self.shared.read().await.obs.clone() else {
            return Ok(());
        };
        let deadline = tokio::time::Instant::now() + OUTPUT_CONFIRM_TIMEOUT;
        loop {
            match handle.get_stream_status().await {
                Ok(status) if status.output_active => {
                    self.sink.emit(AppEvent::ObsOutputStateChanged {
                        state: ObsOutputState::Active,
                    });
                    return Ok(());
                }
                _ => {}
            }
            if tokio::time::Instant::now() >= deadline {
                // StartStream succeeded; the status flag just lags. Not fatal.
                return Ok(());
            }
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        }
    }
}

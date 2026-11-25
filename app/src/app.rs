//! The eframe application: owns view state and drives the coordinator.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::application::Coordinator;
use crate::domain::events::{AppEvent, LogLevel};
use crate::domain::models::{
    AccountInfo, AudienceType, GameCategory, RtmpCredentials, SecretString,
};
use crate::domain::monitor::{LiveConnectionState, LiveMonitorData};
use crate::domain::states::{
    ApplyState, AuthState, CredentialState, ObsConnectionState, ObsOutputState,
};
use crate::domain::{Command, ObsPreferences};
use crate::persistence::AppConfig;
use crate::platform::credentials::{KeyringStore, SecretStore};
use crate::ui::theme;

const SEARCH_DEBOUNCE: Duration = Duration::from_millis(300);
const TOAST_LIFETIME: Duration = Duration::from_secs(6);
const MAX_LOG_LINES: usize = 200;

/// A transient notification.
pub struct Toast {
    pub level: LogLevel,
    pub message: String,
    pub until: Instant,
}

/// Top-level pages in the ShuKanji sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Page {
    #[default]
    Setup,
    Live,
    Account,
    Obs,
    Log,
    Settings,
}

impl Page {
    pub fn title(self) -> &'static str {
        match self {
            Page::Setup => "Stream setup",
            Page::Live => "Live dashboard",
            Page::Account => "Streamlabs account",
            Page::Obs => "OBS connection",
            Page::Log => "Activity log",
            Page::Settings => "Settings",
        }
    }
}

/// A confirmation dialog that is currently open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confirm {
    GoLive,
    EndLive,
    RemoveStored,
}

/// One activity-log line, timestamped from the session start.
pub struct LogLine {
    pub at: Instant,
    pub message: String,
}

/// Live-dashboard chat filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LiveTab {
    #[default]
    All,
    Gifts,
    Follows,
}

/// UI-thread-owned view state. Background tasks never touch these directly.
pub struct App {
    // infrastructure
    pub config: AppConfig,
    #[allow(dead_code)]
    pub store: Arc<dyn SecretStore>,
    pub coordinator: Coordinator,

    // auth / account
    pub token_input: String,
    pub show_token: bool,
    pub current_token: Option<SecretString>,
    pub auth: AuthState,
    pub account: Option<AccountInfo>,
    pub auth_message: Option<String>,
    pub auth_busy: bool,
    pub local_scan_busy: bool,
    pub web_auth_url: Option<String>,

    // stream details
    pub title: String,
    pub game: String,
    pub game_mask_id: String,
    pub mature: bool,
    pub suggestions: Vec<GameCategory>,
    pub show_suggestions: bool,
    search_debounce: Option<Instant>,
    last_input_seen: String,
    search_request_id: u64,
    last_search_query: String,
    pending_mask_resolve: bool,

    // credentials
    pub credentials: Option<RtmpCredentials>,
    pub credential_state: CredentialState,
    pub reveal_key: bool,
    pub credentials_request_id: u64,

    // one-click go-live
    pub go_live_busy: bool,
    pub go_live_request_id: u64,
    pub go_live_stage: Option<String>,

    // TikTok LIVE monitor
    pub show_live_monitor: bool,
    pub live_username: String,
    pub live_state: LiveConnectionState,
    pub live: LiveMonitorData,
    pub live_autoscroll: bool,
    pub live_tab: LiveTab,
    /// Cached remote images (avatars, gift art, emotes).
    pub images: crate::ui::image_cache::ImageCache,

    // Live dashboard section visibility / sizes
    pub monitor_expanded: bool,
    /// Live chat height, set by dragging the grip under the chat. `None` means
    /// "size to the window"; it can never go below the automatic height.
    pub chat_height: Option<f32>,
    /// Scratch buffer for the merged chat feed, reused every frame so the UI
    /// never allocates or sorts the whole feed while drawing.
    pub feed_entries: Vec<crate::domain::monitor::FeedEntry>,
    /// Viewer-strip height, set by dragging its grip (never below the default).
    pub viewers_height: f32,
    /// The Go Live bar can be minimized to free vertical space.
    pub go_live_bar_collapsed: bool,

    // OBS
    pub obs_host: String,
    pub obs_port: String,
    pub obs_password: String,
    pub obs_remember: bool,
    pub obs_connection: ObsConnectionState,
    pub obs_output: ObsOutputState,
    pub apply_state: ApplyState,
    pub apply_request_id: u64,

    // shell / theme / navigation
    pub page: Page,
    pub theme_choice: theme::ThemeChoice,
    /// Result of the last font scan (custom + system fallbacks), shown in
    /// Settings → Chat fonts.
    pub font_report: crate::ui::fonts::FontReport,
    pub tebby: crate::ui::tebby::Tebby,
    pub display_name: String,
    pub show_timestamps: bool,
    pub sidebar_minimized: bool,
    /// Set once the user toggles the sidebar themselves (Ctrl+B), which stops
    /// the automatic narrow-window collapse from overriding them.
    sidebar_user_set: bool,
    pub confirm: Option<Confirm>,
    pub started_at: Instant,
    pub live_started_at: Option<Instant>,
    /// Frames left to sanity-check the window geometry on start-up.
    window_check_frames: u8,

    // diagnostics
    pub toast: Option<Toast>,
    pub logs: VecDeque<LogLine>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let font_report = theme::apply(&cc.egui_ctx);
        egui_extras::install_image_loaders(&cc.egui_ctx);

        let config = AppConfig::load().unwrap_or_default();
        let store: Arc<dyn SecretStore> = Arc::new(KeyringStore);

        let ctx = cc.egui_ctx.clone();
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(move || ctx.request_repaint());
        let coordinator = Coordinator::new(wake, store.clone(), None)
            .expect("failed to start background runtime");

        let title = config.stream.title.clone();
        let game = config.stream.game.clone();
        let game_mask_id = config.stream.game_mask_id.clone();
        let mature = config.stream.audience == AudienceType::Mature;
        let obs_prefs: ObsPreferences = config.obs.clone();

        let mut app = Self {
            config,
            store,
            coordinator,
            token_input: String::new(),
            show_token: false,
            current_token: None,
            auth: AuthState::Disconnected,
            account: None,
            auth_message: None,
            auth_busy: false,
            local_scan_busy: false,
            web_auth_url: None,
            title,
            game,
            game_mask_id,
            mature,
            suggestions: Vec::new(),
            show_suggestions: false,
            search_debounce: None,
            last_input_seen: String::new(),
            search_request_id: 0,
            last_search_query: String::new(),
            pending_mask_resolve: false,
            credentials: None,
            credential_state: CredentialState::Empty,
            reveal_key: false,
            credentials_request_id: 0,
            go_live_busy: false,
            go_live_request_id: 0,
            go_live_stage: None,
            show_live_monitor: true,
            live_username: String::new(),
            live_state: LiveConnectionState::Disconnected,
            live: LiveMonitorData::default(),
            live_autoscroll: true,
            live_tab: LiveTab::All,
            images: crate::ui::image_cache::ImageCache::new(),
            monitor_expanded: true,
            chat_height: None,
            feed_entries: Vec::new(),
            viewers_height: 40.0,
            go_live_bar_collapsed: false,
            obs_host: obs_prefs.host,
            obs_port: obs_prefs.port.to_string(),
            obs_password: String::new(),
            obs_remember: obs_prefs.remember_password,
            obs_connection: ObsConnectionState::Disconnected,
            obs_output: ObsOutputState::Unknown,
            apply_state: ApplyState::Idle,
            apply_request_id: 0,
            page: Page::Setup,
            theme_choice: theme::ThemeChoice::System,
            font_report,
            tebby: crate::ui::tebby::Tebby::default(),
            display_name: "your_name".to_string(),
            show_timestamps: true,
            sidebar_minimized: false,
            sidebar_user_set: false,
            confirm: None,
            started_at: Instant::now(),
            live_started_at: None,
            window_check_frames: 10,
            toast: None,
            logs: VecDeque::new(),
        };

        app.log("Ready.");
        app.coordinator.send(Command::RestoreSession);
        if app.config.obs.auto_connect {
            app.coordinator.send(Command::ObsConnectStored {
                host: app.obs_host.clone(),
                port: app.obs_port.parse().unwrap_or(4455),
            });
        }
        app
    }

    pub fn toast(&mut self, level: LogLevel, message: impl Into<String>) {
        let message = message.into();
        self.log(message.clone());
        self.toast = Some(Toast {
            level,
            message,
            until: Instant::now() + TOAST_LIFETIME,
        });
    }

    pub fn log(&mut self, message: impl Into<String>) {
        self.logs.push_back(LogLine {
            at: Instant::now(),
            message: message.into(),
        });
        while self.logs.len() > MAX_LOG_LINES {
            self.logs.pop_front();
        }
    }

    /// Apply a picked game suggestion: set the name and stop further searching.
    pub fn set_game_selection(&mut self, name: String) {
        self.game = name.clone();
        self.last_input_seen = name.clone();
        self.last_search_query = name;
    }

    pub fn save_config(&mut self) {
        self.config.stream.title = self.title.clone();
        self.config.stream.game = self.game.clone();
        self.config.stream.game_mask_id = self.game_mask_id.clone();
        self.config.stream.audience = if self.mature {
            AudienceType::Mature
        } else {
            AudienceType::General
        };
        self.config.obs.host = self.obs_host.clone();
        self.config.obs.port = self.obs_port.parse().unwrap_or(4455);
        self.config.obs.remember_password = self.obs_remember;

        match self.config.save() {
            Ok(()) => self.toast(LogLevel::Info, "Configuration saved."),
            Err(e) => self.toast(LogLevel::Error, format!("Could not save config: {e}")),
        }
    }

    fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::WebAuthStarted { url } => {
                self.auth_busy = true;
                self.web_auth_url = Some(url.clone());
                self.toast(
                    LogLevel::Info,
                    "Browser opened for Streamlabs login. Waiting for callback…",
                );
            }
            AppEvent::WebAuthFinished { token, message } => {
                self.auth_busy = false;
                if let Some(token) = token {
                    self.token_input = token.expose().to_string();
                    self.current_token = Some(token);
                    self.auth = AuthState::Authenticating;
                    self.toast(LogLevel::Info, "Signed in with Streamlabs.");
                } else {
                    self.auth = AuthState::Failed {
                        message: message.clone().unwrap_or_default(),
                    };
                    self.toast(
                        LogLevel::Error,
                        format!(
                            "Web login failed: {}",
                            message.unwrap_or_else(|| "unknown error".to_string())
                        ),
                    );
                }
            }
            AppEvent::LocalTokenScanStarted => {
                self.local_scan_busy = true;
            }
            AppEvent::LocalTokenFound { token } => {
                self.local_scan_busy = false;
                self.coordinator.send(Command::SubmitToken { token });
            }
            AppEvent::LocalTokenNotFound => {
                self.local_scan_busy = false;
                self.toast(
                    LogLevel::Warn,
                    "No local Streamlabs token found. Sign in via the web or paste a token.",
                );
            }
            AppEvent::TokenSubmitted { token } => {
                self.token_input = token.expose().to_string();
                self.current_token = Some(token);
                self.auth = AuthState::Authenticating;
            }
            AppEvent::AccountInfoLoaded { info } => {
                self.account = Some(info.clone());
                self.auth = AuthState::Authenticated {
                    username: info.username.clone(),
                };
                self.auth_message = None;
                // The profile name always mirrors the signed-in account.
                if !info.username.trim().is_empty() {
                    self.display_name = info.username.clone();
                }
                // Default the LIVE monitor to this account's own TikTok handle.
                if self.live_username.trim().is_empty() {
                    self.live_username = info.username.clone();
                }
                if !info.can_be_live {
                    self.toast(
                        LogLevel::Warn,
                        "Streamlabs reports this account cannot go live yet.",
                    );
                }
                self.maybe_resolve_mask_id();
            }
            AppEvent::AccountInfoFailed { message } => {
                self.auth = AuthState::Failed {
                    message: message.clone(),
                };
                self.auth_message = Some(message.clone());
                self.toast(LogLevel::Error, format!("Account info failed: {message}"));
            }
            AppEvent::GameSuggestions {
                request_id,
                categories,
            } => {
                if request_id < self.search_request_id {
                    return; // stale
                }
                if self.pending_mask_resolve {
                    self.pending_mask_resolve = false;
                    if let Some(matched) = categories
                        .iter()
                        .find(|c| c.full_name.eq_ignore_ascii_case(&self.game))
                    {
                        self.game_mask_id = matched.game_mask_id.clone();
                    }
                    return;
                }
                self.suggestions = categories;
                self.show_suggestions = !self.suggestions.is_empty();
            }
            AppEvent::GameSearchFailed {
                request_id,
                message,
            } => {
                if request_id < self.search_request_id {
                    return;
                }
                self.show_suggestions = false;
                self.log(format!("Game search failed: {message}"));
            }
            AppEvent::CredentialsRetrieved {
                request_id,
                credentials,
            } => {
                if request_id < self.credentials_request_id {
                    return;
                }
                self.credential_state = CredentialState::Retrieved {
                    source: credentials.source,
                };
                self.credentials = Some(credentials);
                self.reveal_key = false;
                self.apply_state = ApplyState::Idle;
                self.toast(
                    LogLevel::Info,
                    "Credentials retrieved — not verified as live on TikTok.",
                );
            }
            AppEvent::CredentialsOutcomeUnknown {
                request_id,
                message,
            } => {
                if request_id < self.credentials_request_id {
                    return;
                }
                self.credential_state = CredentialState::OutcomeUnknown {
                    message: message.clone(),
                };
                self.toast(
                    LogLevel::Error,
                    "Request timed out — outcome unknown. Check your account/session before retrying.",
                );
            }
            AppEvent::CredentialsFailed {
                request_id,
                message,
            } => {
                if request_id < self.credentials_request_id {
                    return;
                }
                self.credential_state = CredentialState::Failed {
                    message: message.clone(),
                };
                self.toast(
                    LogLevel::Error,
                    format!("Could not get credentials: {message}"),
                );
            }
            AppEvent::StreamlabsEnded { success, message } => {
                if success {
                    self.credentials = None;
                    self.credential_state = CredentialState::Empty;
                    self.apply_state = ApplyState::Idle;
                }
                self.toast(
                    if success {
                        LogLevel::Info
                    } else {
                        LogLevel::Error
                    },
                    message,
                );
            }
            AppEvent::ObsConnected {
                version,
                rpc_version,
            } => {
                self.obs_connection = ObsConnectionState::Connected {
                    version: version.clone(),
                    rpc_version,
                };
                self.toast(
                    LogLevel::Info,
                    format!("Connected to OBS WebSocket {version}."),
                );
            }
            AppEvent::ObsDisconnected { reason } => {
                self.obs_connection = ObsConnectionState::Disconnected;
                self.obs_output = ObsOutputState::Unknown;
                self.log(format!("OBS disconnected: {reason}"));
            }
            AppEvent::ObsReconnecting { attempt } => {
                self.obs_connection = ObsConnectionState::Reconnecting { attempt };
            }
            AppEvent::ObsConnectionFailed { message } => {
                self.obs_connection = ObsConnectionState::Failed {
                    message: message.clone(),
                };
            }
            AppEvent::ObsOutputStateChanged { state } => {
                match state {
                    ObsOutputState::Active => {
                        if self.live_started_at.is_none() {
                            self.live_started_at = Some(Instant::now());
                        }
                    }
                    _ => self.live_started_at = None,
                }
                self.obs_output = state;
            }
            AppEvent::ObsApplyFinished { request_id, result } => {
                if request_id < self.apply_request_id {
                    return;
                }
                let message = match &result {
                    ApplyState::Applied { .. } => Some("OBS settings applied and confirmed."),
                    ApplyState::Mismatch { detail } => {
                        self.toast(LogLevel::Error, format!("Read-back mismatch: {detail}"));
                        None
                    }
                    ApplyState::Failed { message } => {
                        self.toast(LogLevel::Error, format!("Apply failed: {message}"));
                        None
                    }
                    ApplyState::RefusedOutputActive => {
                        self.toast(
                            LogLevel::Warn,
                            "Refused to apply settings while OBS output is running.",
                        );
                        None
                    }
                    _ => None,
                };
                self.apply_state = result;
                if let Some(message) = message {
                    self.toast(LogLevel::Info, message);
                }
            }
            AppEvent::ObsStartFinished { ok, message } => {
                self.toast(if ok { LogLevel::Info } else { LogLevel::Error }, message);
            }
            AppEvent::ObsStopFinished { ok, message } => {
                self.toast(if ok { LogLevel::Info } else { LogLevel::Error }, message);
            }
            AppEvent::GoLiveProgress { request_id, stage } => {
                if request_id < self.go_live_request_id {
                    return;
                }
                self.go_live_stage = Some(stage.clone());
                self.log(stage);
            }
            AppEvent::GoLiveFinished {
                request_id,
                ok,
                message,
            } => {
                if request_id < self.go_live_request_id {
                    return;
                }
                self.go_live_busy = false;
                self.go_live_stage = None;
                if !ok {
                    // A live session may already exist (non-idempotent start).
                    // Only allow a plain retry when none was created, and never
                    // clear the OutcomeUnknown gate.
                    let has_session = self
                        .credentials
                        .as_ref()
                        .and_then(|c| c.stream_id.as_ref())
                        .is_some();
                    if !has_session
                        && !matches!(
                            self.credential_state,
                            CredentialState::OutcomeUnknown { .. }
                        )
                    {
                        self.credential_state = CredentialState::Failed {
                            message: message.clone(),
                        };
                    }
                }
                self.toast(if ok { LogLevel::Info } else { LogLevel::Error }, message);
            }
            AppEvent::LiveMonitorStatus { state } => {
                match &state {
                    LiveConnectionState::Connecting { .. } => self.live.reset(),
                    LiveConnectionState::Connected { .. } => {
                        self.toast(LogLevel::Info, "TikTok LIVE monitor connected.")
                    }
                    LiveConnectionState::Disconnected => {}
                    LiveConnectionState::Reconnecting { attempt, .. } => self.log(format!(
                        "TikTok LIVE monitor reconnecting (attempt {attempt})…"
                    )),
                    LiveConnectionState::Failed { message } => {
                        self.toast(LogLevel::Error, format!("LIVE monitor: {message}"))
                    }
                }
                self.live_state = state;
            }
            AppEvent::LiveMonitorEvent { event } => {
                self.live.apply(event);
            }
            AppEvent::Log { level, message } => {
                self.toast(level, message);
            }
        }
    }

    fn maybe_resolve_mask_id(&mut self) {
        if !self.game.trim().is_empty()
            && self.game_mask_id.is_empty()
            && self.current_token.is_some()
        {
            self.pending_mask_resolve = true;
            self.spawn_search(self.game.clone());
        }
    }

    fn spawn_search(&mut self, query: String) {
        let Some(token) = self.current_token.clone() else {
            return;
        };
        self.search_request_id += 1;
        self.last_search_query = query.clone();
        self.coordinator.send(Command::SearchGames {
            token,
            query,
            request_id: self.search_request_id,
        });
    }

    fn tick_search(&mut self, ctx: &egui::Context) {
        if self.game != self.last_input_seen {
            self.last_input_seen = self.game.clone();
            if !self.pending_mask_resolve {
                self.search_debounce = Some(Instant::now() + SEARCH_DEBOUNCE);
            }
        }

        if let Some(deadline) = self.search_debounce {
            if Instant::now() >= deadline {
                self.search_debounce = None;
                let query = self.game.clone();
                if self.current_token.is_some()
                    && !query.trim().is_empty()
                    && query != self.last_search_query
                {
                    self.spawn_search(query);
                }
            } else {
                ctx.request_repaint_after(deadline - Instant::now());
            }
        }
    }

    fn expire_toast(&mut self) {
        if let Some(toast) = &self.toast
            && Instant::now() >= toast.until
        {
            self.toast = None;
        }
    }

    pub fn can_request_credentials(&self) -> bool {
        self.current_token.is_some()
            && crate::domain::may_request_credentials(&self.credential_state).is_ok()
    }

    pub fn can_apply_to_obs(&self) -> bool {
        self.credentials.is_some()
            && self.obs_connection.is_connected()
            && !matches!(self.apply_state, ApplyState::Applying)
    }

    /// Whether the one-click "Go Live" action may be started.
    pub fn can_go_live(&self) -> bool {
        self.current_token.is_some()
            && !self.go_live_busy
            && self.obs_output != ObsOutputState::Active
            && self
                .credentials
                .as_ref()
                .is_none_or(|c| c.stream_id.is_none())
            && crate::domain::may_request_credentials(&self.credential_state).is_ok()
    }

    /// Mark the sidebar state as the user's choice, so the automatic
    /// narrow-window collapse stops overriding it.
    pub fn set_sidebar_preference(&mut self) {
        self.sidebar_user_set = true;
    }

    /// Whether the app currently considers itself streaming. Derived from the
    /// real lifecycle — OBS output being active or a Streamlabs session
    /// existing — never from a separate toggle, so the button always reflects
    /// what is actually happening.
    pub fn is_streaming(&self) -> bool {
        self.obs_output == ObsOutputState::Active
            || self
                .credentials
                .as_ref()
                .and_then(|c| c.stream_id.as_ref())
                .is_some()
    }

    /// Whether an active Streamlabs session can be ended.
    pub fn can_end_live(&self) -> bool {
        self.current_token.is_some()
            && self
                .credentials
                .as_ref()
                .and_then(|c| c.stream_id.as_ref())
                .is_some()
    }

    /// Whether the LIVE monitor is connecting/connected/reconnecting.
    pub fn live_monitor_active(&self) -> bool {
        matches!(
            self.live_state,
            LiveConnectionState::Connecting { .. }
                | LiveConnectionState::Connected { .. }
                | LiveConnectionState::Reconnecting { .. }
        )
    }

    /// Whether the monitor can be started (needs a username, and not already up).
    pub fn can_start_live_monitor(&self) -> bool {
        !self.live_username.trim().is_empty() && !self.live_monitor_active()
    }

    /// Open the streamer's own TikTok LIVE page (or the LIVE directory) in the
    /// default browser.
    pub fn open_tiktok_live(&mut self) {
        let handle = self.live_username.trim().trim_start_matches('@');
        let url = if handle.is_empty() {
            "https://www.tiktok.com/live".to_string()
        } else {
            format!("https://www.tiktok.com/@{handle}/live")
        };
        let _ = open::that(url);
    }

    /// One click: sign in to Streamlabs through the browser.
    pub fn load_from_web(&mut self) {
        self.coordinator.send(Command::BeginWebAuth);
        self.log("Load from Web: signing in to Streamlabs through your browser.");
    }
    /// Session-relative timestamp for an activity-log line (HH:MM:SS).
    pub fn log_stamp(&self, at: Instant) -> String {
        let secs = at.duration_since(self.started_at).as_secs();
        format!(
            "{:02}:{:02}:{:02}",
            secs / 3600,
            (secs % 3600) / 60,
            secs % 60
        )
    }

    /// Elapsed time since OBS output went live, formatted HH:MM:SS.
    pub fn live_clock(&self) -> String {
        match self.live_started_at {
            Some(started) => {
                let secs = started.elapsed().as_secs();
                format!(
                    "{:02}:{:02}:{:02}",
                    secs / 3600,
                    (secs % 3600) / 60,
                    secs % 60
                )
            }
            None => "00:00:00".to_string(),
        }
    }

    /// Compact human count (1.2K, 3.4M).
    pub fn compact(n: i64) -> String {
        if n >= 1_000_000 {
            format!("{:.1}M", n as f64 / 1_000_000.0)
        } else if n >= 1_000 {
            format!("{:.1}K", n as f64 / 1_000.0)
        } else {
            n.to_string()
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        theme::set_choice(ctx, self.theme_choice);
        ctx.set_theme(self.theme_choice.egui_pref());
        // The mascot only exists in the Tebby theme, and only when the sidebar
        // is open. It must not drive a repaint loop when it isn't on screen.
        let tebby_visible =
            self.theme_choice == theme::ThemeChoice::Tebby && !self.sidebar_minimized;
        self.tebby
            .animate(ctx, ctx.input(|i| i.stable_dt), tebby_visible);
        // Responsive shell: the sidebar collapses on its own when the window is
        // narrow, and comes back when there is room — unless the user has taken
        // over with Ctrl+B, in which case we leave it alone.
        if !self.sidebar_user_set {
            let width = ctx.screen_rect().width();
            self.sidebar_minimized = width < 900.0;
        }
        // Self-heal a degenerate window: a 0x0 or off-screen window is unusable
        // and the user has no way to recover it.
        if self.window_check_frames > 0 {
            self.window_check_frames -= 1;
            let size = ctx.screen_rect().size();
            let outer = ctx.input(|i| i.viewport().outer_rect);
            let off_screen = outer.is_some_and(|r| r.min.x < -4000.0 || r.min.y < -4000.0);
            if size.x < 480.0 || size.y < 360.0 || off_screen {
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(1280.0, 800.0)));
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(60.0, 60.0)));
            }
        }
        let (toggle_sidebar, escape) = ctx.input(|i| {
            (
                i.modifiers.command && i.key_pressed(egui::Key::B),
                i.key_pressed(egui::Key::Escape),
            )
        });
        if toggle_sidebar {
            self.sidebar_minimized = !self.sidebar_minimized;
            // The user has expressed a preference; stop auto-collapsing.
            self.sidebar_user_set = true;
        }
        if escape {
            self.confirm = None;
            self.show_suggestions = false;
        }

        for event in self.coordinator.poll_events() {
            self.handle_event(event);
        }
        self.images.poll(ctx);
        self.tick_search(ctx);
        self.expire_toast();
        crate::ui::panels::render(self, ctx);
    }
}

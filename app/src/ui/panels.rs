//! ShuKanji shell and pages. Flat layout, two themes, one pink accent.

use egui::{
    Align, Align2, Color32, CornerRadius, FontId, Layout, Margin, Rect, RichText, Sense, Stroke,
    StrokeKind, Vec2,
};

use crate::app::{App, Confirm, LiveTab, Page};
use crate::domain::events::LogLevel;
use crate::domain::models::{AudienceType, GameCategory, SecretString};
use crate::domain::monitor::{
    ActivityKind, ChatEmote, ChatLine, FeedKind, GiftLine, LiveConnectionState,
};
use crate::domain::states::{ApplyState, AuthState, ObsConnectionState, ObsOutputState};

use super::emoji::{self, Piece};
use super::image_cache::ImageCache;
use super::theme::{self, Palette};
use super::widgets::{self, BtnKind};

pub fn render(app: &mut App, ctx: &egui::Context) {
    let p = theme::pal(ctx);
    // Animated minimizer: the width slides and `expand` drives the label
    // fades/icon slide inside the sidebar (1 = expanded, 0 = rail).
    let minimized_t = ctx.animate_value_with_time(
        egui::Id::new("shukanji.sidebar_minimized"),
        if app.sidebar_minimized { 1.0 } else { 0.0 },
        widgets::animation_secs(ctx, 0.18),
    );
    let expand = 1.0 - minimized_t;
    let sidebar_width = 220.0 + (68.0 - 220.0) * minimized_t;
    egui::SidePanel::left("sidebar")
        .exact_width(sidebar_width)
        .resizable(false)
        .frame(
            egui::Frame::new()
                .fill(p.sidebar)
                .inner_margin(Margin::symmetric(12, 20)),
        )
        .show(ctx, |ui| sidebar(ui, app, expand));

    // Two-row header when the status strip cannot sit beside the title.
    let header_height = if ctx.screen_rect().width() < HEADER_TWO_ROW_BELOW {
        HEADER_TWO_ROW_HEIGHT
    } else {
        HEADER_SINGLE_ROW_HEIGHT
    };

    egui::TopBottomPanel::top("header")
        .exact_height(header_height)
        .frame(
            egui::Frame::new()
                .fill(p.sidebar)
                .inner_margin(Margin::symmetric(40, 0)),
        )
        .show(ctx, |ui| header(ui, app));

    // The bar minimizes to a thin strip; its height eases between the two
    // sizes and the content swaps at the midpoint of the animation.
    let collapse_t = ctx.animate_value_with_time(
        egui::Id::new("shukanji.go_live_bar_collapsed"),
        if app.go_live_bar_collapsed { 1.0 } else { 0.0 },
        widgets::animation_secs(ctx, 0.16),
    );
    egui::TopBottomPanel::bottom("go_live_bar")
        .exact_height(76.0 + (34.0 - 76.0) * collapse_t)
        .frame(
            egui::Frame::new()
                .fill(p.sidebar)
                .inner_margin(Margin::symmetric(
                    40,
                    (16.0 + (4.0 - 16.0) * collapse_t).round() as _,
                )),
        )
        .show(ctx, |ui| bottom_bar(ui, app, collapse_t));

    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(p.bg).inner_margin(Margin {
            left: 40,
            right: 0,
            top: 18,
            bottom: 22,
        }))
        .show(ctx, |ui| {
            // Exact visible height, captured before the scroll area: inside one
            // `available_height()` is infinite, which is why the chat used to
            // size itself off-screen.
            let viewport_height = ui.available_height();
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                // Dragging is reserved for the resize grips; the wheel still
                // scrolls.
                .drag_to_scroll(false)
                .show(ui, |ui| {
                    // Keep the content padded on the right while the scroll bar
                    // sits flush against the window edge.
                    ui.set_max_width((ui.available_width() - 40.0).max(120.0));
                    page_body(ui, app, viewport_height);
                });
        });

    confirm_dialog(app, ctx);
    toast(app, ctx);

    // Tebby mascot (bottom-left), only in the Tebby theme.
    if theme::choice(ctx) == theme::ThemeChoice::Tebby {
        app.tebby.draw(ctx, app.sidebar_minimized);
    }
}

// ------------------------------------------------------------------- sidebar

fn sidebar(ui: &mut egui::Ui, app: &mut App, expand: f32) {
    let p = theme::pal(ui.ctx());

    ui.horizontal(|ui| {
        // Avatar shrinks slightly as the rail closes; the wordmark and the
        // display name fade out with the expansion value.
        let avatar_size = 40.0 + 8.0 * expand;
        let tebby_avatar = if theme::choice(ui.ctx()) == theme::ThemeChoice::Tebby {
            app.tebby.avatar_texture().cloned()
        } else {
            None
        };
        match tebby_avatar {
            Some(texture) => widgets::avatar_image(ui, &texture, avatar_size),
            None => widgets::avatar(ui, &app.display_name, avatar_size),
        }
        if expand > 0.01 {
            ui.add_space(4.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.label(
                    RichText::new("ShuKanji")
                        .family(theme::wordmark_family())
                        .color(widgets::with_alpha(p.text, expand))
                        .size(22.0),
                );
                ui.label(
                    RichText::new(&app.display_name)
                        .color(widgets::with_alpha(p.muted, expand))
                        .size(12.0),
                );
            });
        }
    });

    ui.add_space(16.0);
    // The hairline keeps its slot and fades, so nothing jumps mid-animation.
    let width = ui.available_width();
    let (line_rect, _) = ui.allocate_exact_size(Vec2::new(width, 1.0), Sense::hover());
    if expand > 0.01 {
        ui.painter().rect_filled(
            line_rect,
            CornerRadius::ZERO,
            widgets::with_alpha(p.line, expand),
        );
    }
    ui.add_space(10.0);

    group_label(ui, "Stream", expand);
    nav(ui, app, Page::Setup, "setup", "Stream setup", expand);
    nav(ui, app, Page::Live, "live", "Live dashboard", expand);
    nav(
        ui,
        app,
        Page::Account,
        "account",
        "Streamlabs account",
        expand,
    );
    nav(ui, app, Page::Obs, "obs", "OBS connection", expand);

    group_label(ui, "Tools", expand);
    nav(ui, app, Page::Log, "log", "Activity log", expand);
    nav(ui, app, Page::Settings, "settings", "Settings", expand);

    ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
        if widgets::nav_item(ui, "minimize", "Minimize", false, expand)
            .on_hover_text("Minimize / expand sidebar (Ctrl+B)")
            .clicked()
        {
            app.sidebar_minimized = !app.sidebar_minimized;
            // Take over from the automatic narrow-window collapse, otherwise it
            // would immediately undo this click on the next frame.
            app.set_sidebar_preference();
        }
    });
}

fn group_label(ui: &mut egui::Ui, text: &str, expand: f32) {
    let p = theme::pal(ui.ctx());
    // Fixed height so nothing jumps: the expanded caption and the minimized
    // centered rule crossfade inside the same slot.
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 26.0), Sense::hover());
    if expand > 0.01 {
        ui.painter().text(
            egui::pos2(rect.left(), rect.center().y),
            Align2::LEFT_CENTER,
            text.to_uppercase(),
            FontId::proportional(11.0),
            widgets::with_alpha(p.muted, expand),
        );
    }
    let rule = 1.0 - expand;
    if rule > 0.01 {
        let c = rect.center();
        ui.painter().line_segment(
            [egui::pos2(c.x - 14.0, c.y), egui::pos2(c.x + 14.0, c.y)],
            Stroke::new(1.0_f32, widgets::with_alpha(p.line, rule)),
        );
    }
}

fn nav(ui: &mut egui::Ui, app: &mut App, page: Page, icon: &str, label: &str, expand: f32) {
    let active = app.page == page;
    let mut response = widgets::nav_item(ui, icon, label, active, expand);
    // Expanded rows already show their label; only the icon-only rail needs a
    // tooltip. This prevents a second wall of text on every expanded hover.
    if expand <= 0.02 {
        response = response.on_hover_text(label);
    }
    if response.clicked() {
        app.page = page;
    }
    ui.add_space(4.0);
}

// -------------------------------------------------------------------- header

/// Below this window width the status strip is moved onto its own row, so the
/// page title and the connection indicators can never overlap. The strip is
/// wide (four labelled indicators plus the clock and the theme toggle), so the
/// threshold is generous.
const HEADER_TWO_ROW_BELOW: f32 = 1500.0;
/// Header height when the strip needs its own row.
const HEADER_TWO_ROW_HEIGHT: f32 = 98.0;
const HEADER_SINGLE_ROW_HEIGHT: f32 = 54.0;

fn header(ui: &mut egui::Ui, app: &mut App) {
    let p = theme::pal(ui.ctx());
    let rect = ui.max_rect();
    let two_rows = rect.height() > 70.0;

    if two_rows {
        // Narrow: title on top, indicators on their own row underneath.
        ui.vertical(|ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(app.page.title()).size(17.0).strong());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    theme_toggle(ui, app);
                });
            });
            ui.add_space(2.0);
            ui.horizontal_wrapped(|ui| status_strip(ui, app, p, true));
        });
    } else {
        // Fill the header height and center everything vertically, so the
        // status strip sits on the title's baseline instead of hugging the top.
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            ui.label(RichText::new(app.page.title()).size(17.0).strong());
            let remaining = Vec2::new(ui.available_width(), 34.0);
            ui.allocate_ui_with_layout(remaining, Layout::right_to_left(Align::Center), |ui| {
                theme_toggle(ui, app);
                if app.obs_output == ObsOutputState::Active {
                    ui.add_space(6.0);
                    ui.label(RichText::new(app.live_clock()).monospace().color(p.text));
                }
                status_strip(ui, app, p, two_rows);
            });
        });
    }

    ui.painter()
        .hline(rect.x_range(), rect.bottom(), Stroke::new(1.0_f32, p.line));
}

/// The connection indicators. In the single-row header they are added
/// right-to-left (so the caller's right-to-left layout orders them); on their
/// own row they are added left-to-right and can wrap.
fn status_strip(ui: &mut egui::Ui, app: &App, p: Palette, on_own_row: bool) {
    let items: [(&str, String, Option<Color32>); 3] = [
        (
            "Account",
            account_label(&app.auth).to_string(),
            account_dot(p, &app.auth),
        ),
        (
            "OBS",
            app.obs_connection.label().to_string(),
            Some(obs_dot(p, &app.obs_connection)),
        ),
        (
            "Output",
            output_label(app.obs_output).to_string(),
            Some(output_dot(p, app.obs_output)),
        ),
    ];
    // On its own row the list reads left-to-right; in the right-to-left header
    // group it must be added in reverse so "Output" ends up right-most.
    let order: Vec<usize> = if on_own_row {
        (0..items.len()).collect()
    } else {
        (0..items.len()).rev().collect()
    };
    for (n, index) in order.into_iter().enumerate() {
        if n > 0 {
            ui.add_space(16.0);
        }
        let (label, value, dot) = &items[index];
        widgets::status_item(ui, label, value, *dot);
    }
}

fn theme_toggle(ui: &mut egui::Ui, app: &mut App) {
    let p = theme::pal(ui.ctx());
    let dark = ui.ctx().theme() == egui::Theme::Dark;
    let (icon, tip) = if dark {
        ("live", "Switch to light theme")
    } else {
        ("account", "Switch to dark theme")
    };
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(36.0), Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(6), p.hover);
    }
    let icon_rect = Rect::from_center_size(rect.center(), Vec2::splat(18.0));
    widgets::paint_icon(ui, icon, icon_rect, p.text_soft);
    if response.on_hover_text(tip).clicked() {
        let choice = if dark {
            theme::ThemeChoice::Light
        } else {
            theme::ThemeChoice::Dark
        };
        app.change_theme(ui.ctx(), choice);
    }
}

// ---------------------------------------------------------------- bottom bar

fn bottom_bar(ui: &mut egui::Ui, app: &mut App, collapse_t: f32) {
    let p = theme::pal(ui.ctx());
    let rect = ui.max_rect();
    let hint = go_live_hint(app);

    let streaming = app.is_streaming();

    // Swap the contents halfway through the height animation.
    if collapse_t > 0.5 {
        // Minimized: nothing but the restore arrow. The Go Live button is
        // hidden with the rest of the bar — expand it again to go live.
        ui.horizontal(|ui| {
            if widgets::collapse_toggle(ui, true, "Show the Go Live bar") {
                app.go_live_bar_collapsed = false;
            }
            // The empty rest of the strip is a big, obvious restore target.
            let (_, rest) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 22.0), Sense::click());
            if rest.on_hover_text("Show the Go Live bar").clicked() {
                app.go_live_bar_collapsed = false;
            }
        });
        ui.painter()
            .hline(rect.x_range(), rect.top(), Stroke::new(1.0_f32, p.line));
        return;
    }

    ui.horizontal(|ui| {
        // Minimize control at the far left.
        if widgets::collapse_toggle(ui, false, "Minimize the Go Live bar") {
            app.go_live_bar_collapsed = true;
        }
        ui.add_space(10.0);
        // On narrow windows the hint is dropped rather than squeezed: the one
        // control that matters must never be clipped.
        if ui.available_width() >= 760.0 {
            let (text, color) = match &hint {
                Some(reason) => (reason.clone(), p.warn),
                None => (
                    "Go Live will connect OBS and start streaming.".to_string(),
                    p.muted,
                ),
            };
            let reserve = 330.0;
            let hint_width = (ui.available_width() - reserve).max(60.0);
            ui.add_sized(
                [hint_width, 20.0],
                egui::Label::new(RichText::new(text).color(color).size(13.0)).truncate(),
            );
        }
        let remaining = Vec2::new(ui.available_width(), 44.0);
        ui.allocate_ui_with_layout(remaining, Layout::right_to_left(Align::Center), |ui| {
            // Right-to-left: first added is right-most, so the visual order is
            // "apply state, one Go Live / End Live button".
            let enabled = if streaming {
                app.can_end_live()
            } else {
                app.can_go_live()
            };
            if widgets::stream_button(ui, streaming, enabled).clicked() {
                app.confirm = Some(if streaming {
                    Confirm::EndLive
                } else {
                    Confirm::GoLive
                });
            }
            ui.add_space(10.0);
            ui.label(
                RichText::new(app.apply_state.label())
                    .color(p.muted)
                    .size(13.0),
            );
        });
    });

    // No rule above the bar.
    let _ = rect;
}

fn go_live_hint(app: &App) -> Option<String> {
    if app.current_token.is_none() {
        Some("Sign in with a token first.".to_string())
    } else if app.title.trim().is_empty() || app.game.trim().is_empty() {
        Some("Add a stream title and a game category.".to_string())
    } else if app.obs_output == ObsOutputState::Active {
        Some("OBS output is already running.".to_string())
    } else if !app.can_go_live() {
        Some("A live session is already active. Use End Live first.".to_string())
    } else {
        None
    }
}

// --------------------------------------------------------------------- pages

fn page_body(ui: &mut egui::Ui, app: &mut App, viewport_height: f32) {
    match app.page {
        Page::Setup => page_setup(ui, app),
        Page::Live => page_live(ui, app, viewport_height),
        Page::Account => page_account(ui, app),
        Page::Obs => page_obs(ui, app),
        Page::Log => page_log(ui, app),
        Page::Settings => page_settings(ui, app),
    }
}

// -- Stream setup

fn page_setup(ui: &mut egui::Ui, app: &mut App) {
    if ui.available_width() >= 700.0 {
        let gap = 56.0;
        let col_width = ((ui.available_width() - gap) / 2.0).max(120.0);
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(col_width, 0.0),
                Layout::top_down(Align::Min),
                |ui| {
                    setup_token(ui, app);
                    ui.add_space(32.0);
                    setup_credentials(ui, app);
                },
            );
            ui.add_space(gap);
            ui.allocate_ui_with_layout(
                Vec2::new(col_width, 0.0),
                Layout::top_down(Align::Min),
                |ui| setup_details(ui, app),
            );
        });
    } else {
        setup_token(ui, app);
        ui.add_space(32.0);
        setup_credentials(ui, app);
        ui.add_space(26.0);
        setup_details(ui, app);
    }
}

fn setup_token(ui: &mut egui::Ui, app: &mut App) {
    widgets::section(ui, "Token loader", |ui| {
        let p = theme::pal(ui.ctx());
        ui.label(RichText::new("Token").color(p.text_soft).size(13.0));
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let show_btn = 84.0;
            let width = (ui.available_width() - show_btn - 10.0).max(80.0);
            ui.add_sized(
                [width, 40.0],
                egui::TextEdit::singleline(&mut app.token_input)
                    .font(FontId::monospace(13.0))
                    .password(!app.show_token)
                    .margin(Margin::symmetric(12, 10))
                    .hint_text("Paste token here or load below..."),
            );
            if widgets::secondary_button(ui, if app.show_token { "Hide" } else { "Show" }, true)
                .clicked()
            {
                app.show_token = !app.show_token;
            }
        });
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if widgets::button(
                ui,
                "Use token",
                !app.token_input.trim().is_empty(),
                BtnKind::Primary,
            )
            .clicked()
            {
                let token = SecretString::new(app.token_input.trim().to_string());
                app.coordinator
                    .send(crate::domain::Command::RefreshAccount { token });
            }
            ui.add_space(8.0);
            if widgets::secondary_button(ui, "Load from PC", !app.local_scan_busy).clicked() {
                app.coordinator.send(crate::domain::Command::ScanLocalToken);
            }
            ui.add_space(8.0);
            if widgets::secondary_button(
                ui,
                if app.auth_busy {
                    "Waiting…"
                } else {
                    "Load from Web"
                },
                !app.auth_busy,
            )
            .clicked()
            {
                app.load_from_web();
            }
        });
        ui.add_space(12.0);
        ui.horizontal(|ui| match &app.account {
            Some(info) => {
                ui.label(RichText::new("Account").color(p.muted).size(13.0));
                ui.label(
                    RichText::new(&info.username)
                        .color(p.text)
                        .size(13.0)
                        .strong(),
                );
                ui.label(
                    RichText::new(format!("({})", info.application_status))
                        .color(p.muted)
                        .size(13.0),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let (text, color) = if info.can_be_live {
                        ("Can go live", p.ok)
                    } else {
                        ("Cannot go live", p.warn)
                    };
                    ui.label(RichText::new(text).color(color).size(13.0).strong());
                });
            }
            None => {
                ui.label(RichText::new("Not signed in").color(p.muted).size(13.0));
            }
        });
        if let AuthState::Failed { message } = &app.auth {
            ui.add_space(6.0);
            ui.label(RichText::new(message).color(p.err).size(12.0));
        }
    });
}

fn setup_credentials(ui: &mut egui::Ui, app: &mut App) {
    widgets::section(ui, "Stream credentials", |ui| {
        let p = theme::pal(ui.ctx());
        let snapshot = app
            .credentials
            .as_ref()
            .map(|c| (c.server.clone(), c.key.expose().to_string()));

        match snapshot {
            Some((server, key)) => {
                ui.label(RichText::new("Server").color(p.text_soft).size(13.0));
                ui.add_space(6.0);
                let mut server_display = server.clone();
                ui.add_sized(
                    [ui.available_width(), 40.0],
                    egui::TextEdit::singleline(&mut server_display)
                        .font(FontId::monospace(13.0))
                        .margin(Margin::symmetric(12, 10)),
                );
                ui.add_space(12.0);
                ui.label(RichText::new("Stream key").color(p.text_soft).size(13.0));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let show_btn = 84.0;
                    let width = (ui.available_width() - show_btn - 10.0).max(80.0);
                    let mut key_display = if app.reveal_key {
                        key.clone()
                    } else {
                        mask(&key)
                    };
                    ui.add_sized(
                        [width, 40.0],
                        egui::TextEdit::singleline(&mut key_display)
                            .font(FontId::monospace(13.0))
                            .password(!app.reveal_key)
                            .margin(Margin::symmetric(12, 10)),
                    );
                    if widgets::secondary_button(
                        ui,
                        if app.reveal_key { "Hide" } else { "Show" },
                        true,
                    )
                    .clicked()
                    {
                        app.reveal_key = !app.reveal_key;
                    }
                });
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if widgets::secondary_button(ui, "Copy URL", true).clicked() {
                        ui.ctx().copy_text(server.clone());
                    }
                    ui.add_space(8.0);
                    if widgets::secondary_button(ui, "Copy key", true).clicked() {
                        ui.ctx().copy_text(key.clone());
                    }
                });
                ui.add_space(10.0);
                ui.label(
                    RichText::new("Credentials retrieved — not verified as live on TikTok.")
                        .color(p.warn)
                        .size(12.0),
                );
            }
            None => {
                ui.label(
                    RichText::new("No credentials yet")
                        .color(p.text)
                        .size(14.0)
                        .strong(),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new("Click Go Live to get them, then apply to OBS and start.")
                        .color(p.muted)
                        .size(13.0),
                );
            }
        }
    });
}

fn setup_details(ui: &mut egui::Ui, app: &mut App) {
    widgets::section(ui, "Stream details", |ui| {
        let p = theme::pal(ui.ctx());
        widgets::field(ui, "Stream title", &mut app.title, false, false);
        ui.add_space(12.0);
        widgets::field(ui, "Game category", &mut app.game, false, false);
        if app.show_suggestions {
            ui.add_space(4.0);
            suggestions_list(ui, app);
        }
        ui.add_space(12.0);
        let mut mature = app.mature;
        if ui.checkbox(&mut mature, "Mature content (18+)").changed() {
            app.mature = mature;
        }
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Category ID").color(p.muted).size(13.0));
            if app.game_mask_id.is_empty() {
                ui.label(RichText::new("not resolved").color(p.muted).size(13.0));
            } else {
                ui.label(RichText::new(&app.game_mask_id).monospace().color(p.text));
            }
        });
    });
}

fn suggestions_list(ui: &mut egui::Ui, app: &mut App) {
    let p = theme::pal(ui.ctx());
    let mut selected: Option<GameCategory> = None;
    // Match the field above: full available width. Use the remaining visible
    // height instead of a fixed five-row window; the outer page still scrolls
    // normally for the controls below, so nothing overlaps.
    let full_width = ui.available_width();
    let list_height = (ui.clip_rect().bottom() - ui.cursor().top() - 16.0).max(120.0);
    egui::Frame::new()
        .fill(p.sidebar)
        .stroke(Stroke::new(1.0_f32, p.field_border))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::symmetric(4, 4))
        .show(ui, |ui| {
            ui.set_min_width(full_width - 8.0);
            egui::ScrollArea::vertical()
                .id_salt("game_suggestions")
                .max_height(list_height)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for category in &app.suggestions {
                        if suggestion_row(ui, &category.full_name, &category.game_mask_id).clicked()
                        {
                            selected = Some(category.clone());
                        }
                    }
                });
        });
    if let Some(category) = selected {
        app.game = category.full_name.clone();
        app.game_mask_id = category.game_mask_id.clone();
        app.show_suggestions = false;
        app.set_game_selection(category.full_name.clone());
    }
}

/// One suggestion row: name left, category id right, full width.
fn suggestion_row(ui: &mut egui::Ui, name: &str, mask_id: &str) -> egui::Response {
    let p = theme::pal(ui.ctx());
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 30.0), Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(4), p.hover);
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(4),
            Stroke::new(2.0_f32, p.accent),
            StrokeKind::Inside,
        );
    }
    // Long IDs consume most of a narrow column. Keep them on wide rows only;
    // the selected ID is always shown below the list. This lets the game name
    // itself remain readable instead of clipping to a few characters.
    let show_id = !mask_id.is_empty() && width >= 520.0;
    let id_font = FontId::monospace(12.0);
    let id_width = if show_id {
        ui.painter()
            .layout_no_wrap(mask_id.to_owned(), id_font.clone(), p.muted)
            .size()
            .x
            + 12.0
    } else {
        0.0
    };
    let name_right = (rect.right() - 10.0 - id_width).max(rect.left() + 20.0);
    let name_rect = Rect::from_min_max(
        egui::pos2(rect.left() + 10.0, rect.top()),
        egui::pos2(name_right, rect.bottom()),
    );
    ui.painter().with_clip_rect(name_rect).text(
        egui::pos2(rect.left() + 10.0, rect.center().y),
        Align2::LEFT_CENTER,
        name,
        FontId::proportional(14.0),
        p.text,
    );
    if show_id {
        ui.painter().text(
            egui::pos2(rect.right() - 10.0, rect.center().y),
            Align2::RIGHT_CENTER,
            mask_id,
            id_font,
            p.muted,
        );
    }
    response
}

// -- Live dashboard

fn page_live(ui: &mut egui::Ui, app: &mut App, viewport_height: f32) {
    live_monitor_controls(ui, app);
    ui.add_space(4.0);

    let live = app.live_monitor_active() || app.obs_output == ObsOutputState::Active;

    stat_strip(ui, app, live);
    // Drag to resize the viewer strip.
    let mut viewers_height = app.viewers_height.max(STAT_STRIP_MIN_HEIGHT);
    if widgets::vertical_grip(ui, &mut viewers_height, STAT_STRIP_MIN_HEIGHT, 320.0).dragged() {
        app.viewers_height = viewers_height;
    }
    chat_panel(ui, app, live, viewport_height);
}

/// A section whose header is a click target that expands/collapses the body.
/// The current status is always shown on the header, so collapsing hides the
/// controls without hiding the state.
fn collapsible_section(
    ui: &mut egui::Ui,
    title: &str,
    hint: &str,
    hint_color: Color32,
    open: &mut bool,
    add: impl FnOnce(&mut egui::Ui),
) {
    let p = theme::pal(ui.ctx());
    widgets::hairline(ui);
    ui.add_space(4.0);

    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 22.0), Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(4), p.nav_hover);
    }
    let color = if response.hovered() { p.text } else { p.muted };

    // A hand-drawn chevron: no dependency on a glyph the font may lack.
    let center = egui::pos2(rect.left() + 6.0, rect.center().y);
    let s = 3.6;
    let triangle = if *open {
        vec![
            center + egui::vec2(-s, -s * 0.6),
            center + egui::vec2(s, -s * 0.6),
            center + egui::vec2(0.0, s * 0.9),
        ]
    } else {
        vec![
            center + egui::vec2(-s * 0.6, -s),
            center + egui::vec2(-s * 0.6, s),
            center + egui::vec2(s * 0.9, 0.0),
        ]
    };
    ui.painter()
        .add(egui::Shape::convex_polygon(triangle, color, Stroke::NONE));

    ui.painter().text(
        egui::pos2(rect.left() + 18.0, rect.center().y),
        Align2::LEFT_CENTER,
        title.to_uppercase(),
        FontId::proportional(11.0),
        color,
    );
    if !hint.is_empty() {
        ui.painter().text(
            egui::pos2(rect.right(), rect.center().y),
            Align2::RIGHT_CENTER,
            hint,
            FontId::proportional(11.5),
            hint_color,
        );
    }
    if response.clicked() {
        *open = !*open;
    }

    ui.add_space(4.0);
    if *open {
        add(ui);
    }
}

fn live_monitor_controls(ui: &mut egui::Ui, app: &mut App) {
    let p = theme::pal(ui.ctx());
    let (dot, status) = live_status(p, &app.live_state);
    let mut open = app.monitor_expanded;
    collapsible_section(ui, "TikTok monitor", status, dot, &mut open, |ui| {
        // Wraps instead of overflowing when the window is narrow.
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new("TikTok username")
                    .color(p.text_soft)
                    .size(13.0),
            );
            ui.add_sized(
                [180.0, 40.0],
                egui::TextEdit::singleline(&mut app.live_username)
                    .margin(Margin::symmetric(12, 10))
                    .hint_text("your @handle"),
            );
            ui.add_space(8.0);
            if widgets::button(
                ui,
                "Connect",
                app.can_start_live_monitor(),
                BtnKind::Primary,
            )
            .clicked()
            {
                start_live_monitor(app);
            }
            ui.add_space(8.0);
            if widgets::secondary_button(ui, "Disconnect", app.live_monitor_active()).clicked() {
                app.coordinator
                    .send(crate::domain::Command::StopLiveMonitor);
            }
            ui.add_space(8.0);
            if widgets::secondary_button(ui, "Open Live Monitor", true).clicked() {
                let _ = open::that("https://livecenter.tiktok.com/live_monitor?lang=en-US");
            }
            ui.add_space(8.0);
            if widgets::secondary_button(ui, "Open my LIVE page", true).clicked() {
                app.open_tiktok_live();
            }
        });
    });
    app.monitor_expanded = open;
}

/// Smallest the viewer strip may be dragged. This is the original size, so it
/// can never be made smaller than it started.
const STAT_STRIP_MIN_HEIGHT: f32 = 40.0;

/// The viewer counters: one compact row by default. Drag the grip underneath to
/// make it taller — the same four counters, just bigger.
fn stat_strip(ui: &mut egui::Ui, app: &mut App, live: bool) {
    let p = theme::pal(ui.ctx());
    // Height is whatever the user last dragged it to, never below the default.
    let height = app.viewers_height.max(STAT_STRIP_MIN_HEIGHT);
    let full = ui.available_width().max(240.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(full, height), Sense::hover());
    let radius = CornerRadius::same(8);
    ui.painter().rect_filled(rect, radius, p.panel);
    ui.painter().rect_stroke(
        rect,
        radius,
        Stroke::new(1.0_f32, p.line),
        StrokeKind::Inside,
    );

    let cells = [
        ("Current viewers", compact_or_dash(live, app.live.viewers)),
        ("Likes", compact_or_dash(live, app.live.likes)),
        ("New followers", dash_if_not(live, app.live.follows as i64)),
        (
            "Diamonds from gifts",
            compact_or_dash(live, app.live.diamonds),
        ),
    ];

    let grid = rect;
    let cell_width = grid.width() / cells.len() as f32;
    // Below this the label and value cannot share a line without clipping.
    let stacked = cell_width < 150.0;
    // A taller strip just gets bigger numbers; the counters never change, so
    // nothing is ever duplicated.
    let value_size = if height >= 80.0 { 30.0 } else { 17.0 };
    let value_inline = !stacked && height < 80.0;

    for (i, (label, value)) in cells.iter().enumerate() {
        let left = grid.left() + cell_width * i as f32;
        if i > 0 {
            ui.painter().line_segment(
                [
                    egui::pos2(left, grid.top() + 6.0),
                    egui::pos2(left, grid.bottom() - 6.0),
                ],
                Stroke::new(1.0_f32, p.line),
            );
        }
        if stacked || !value_inline {
            ui.painter().text(
                egui::pos2(left + 14.0, grid.top() + 16.0),
                Align2::LEFT_TOP,
                *label,
                FontId::proportional(12.0),
                p.muted,
            );
            ui.painter().text(
                egui::pos2(left + 14.0, grid.top() + 38.0),
                Align2::LEFT_TOP,
                value.as_str(),
                FontId::proportional(value_size),
                p.text,
            );
        } else {
            ui.painter().text(
                egui::pos2(left + 14.0, rect.center().y),
                Align2::LEFT_CENTER,
                *label,
                FontId::proportional(12.0),
                p.muted,
            );
            ui.painter().text(
                egui::pos2(left + cell_width - 14.0, rect.center().y),
                Align2::RIGHT_CENTER,
                value.as_str(),
                FontId::proportional(value_size),
                p.text,
            );
        }
    }
}

/// Everything in the chat panel except the scrolling message list: the "Live
/// chat" header row, the rules, the reply box and the status line beneath it.
/// Everything in the chat panel except the scrolling message list: the header
/// row, the rules and the drag grip.
const CHAT_CHROME_HEIGHT: f32 = 96.0;

fn chat_panel(ui: &mut egui::Ui, app: &mut App, live: bool, viewport_height: f32) {
    let p = theme::pal(ui.ctx());
    // Sized against the whole viewport, not the space left over below the
    // sections: the chat is then nearly a full screen, and scrolling down slides
    // the monitor section out of the way instead of it permanently eating the
    // chat's room.
    let base = (viewport_height - CHAT_CHROME_HEIGHT).max(220.0);
    // Whatever the user dragged it to, never smaller than the automatic size.
    let list_height = app.chat_height.unwrap_or(base).max(base);
    widgets::panel_frame(ui.ctx())
        .inner_margin(Margin::same(0))
        .show(ui, |ui| {
            // header row with tabs
            ui.horizontal(|ui| {
                ui.add_space(20.0);
                ui.label(
                    RichText::new("Live chat")
                        .color(p.muted)
                        .size(11.0)
                        .strong(),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(12.0);
                    for (tab, label) in [
                        (LiveTab::Follows, "Follows"),
                        (LiveTab::Gifts, "Gifts"),
                        (LiveTab::All, "All"),
                    ] {
                        if chat_tab(ui, label, app.live_tab == tab).clicked() {
                            app.live_tab = tab;
                        }
                    }
                });
            });
            widgets::hairline(ui);

            egui::ScrollArea::vertical()
                .id_salt("live_chat")
                .max_height(list_height)
                .auto_shrink([false, false])
                .stick_to_bottom(app.live_autoscroll)
                .show(ui, |ui| {
                    ui.add_space(10.0);
                    if !live {
                        ui.add_space(40.0);
                        ui.vertical_centered(|ui| {
                            ui.label(
                                RichText::new("You are not live.")
                                    .color(p.text_soft)
                                    .size(14.0),
                            );
                            ui.label(
                                RichText::new(
                                    "Use Go Live and connect the monitor to see chat and gifts.",
                                )
                                .color(p.muted)
                                .size(13.0),
                            );
                        });
                        ui.add_space(40.0);
                    } else {
                        chat_rows(ui, app);
                    }
                });

            widgets::hairline(ui);
            // Drag the bottom edge to make the chat bigger or smaller.
            ui.add_space(4.0);
            let mut want = app.chat_height.unwrap_or(base);
            if widgets::vertical_grip(ui, &mut want, base, base + 900.0).dragged() {
                app.chat_height = Some(want);
            }
            ui.add_space(6.0);
        });
}

fn chat_tab(ui: &mut egui::Ui, label: &str, active: bool) -> egui::Response {
    let p = theme::pal(ui.ctx());
    let font = FontId::proportional(13.0);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font.clone(), p.text);
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(galley.size().x + 20.0, 30.0), Sense::click());
    // Ease the label color and the underline in/out with the active tab.
    let at = ui.ctx().animate_bool_with_time(
        response.id.with("active"),
        active,
        widgets::animation_secs(ui.ctx(), widgets::TRANSITION_SECS),
    );
    let ht = widgets::hover_t(ui, &response, response.hovered());
    let color = widgets::lerp_color(p.muted, p.text, at.max(ht));
    ui.painter()
        .text(rect.center(), Align2::CENTER_CENTER, label, font, color);
    let underline = widgets::lerp_color(Color32::TRANSPARENT, p.accent, at);
    if underline.a() > 0 {
        ui.painter()
            .hline(rect.x_range(), rect.bottom(), Stroke::new(2.0_f32, underline));
    }
    response
}

fn chat_rows(ui: &mut egui::Ui, app: &mut App) {
    let p = theme::pal(ui.ctx());
    // Split borrows so we can read the feed while mutating the image cache.
    let App {
        live,
        images,
        live_tab,
        feed_entries,
        ..
    } = &mut *app;
    let (chat, activity, gifts) = match *live_tab {
        LiveTab::All => (true, true, true),
        LiveTab::Follows => (true, true, false),
        LiveTab::Gifts => (false, false, true),
    };
    // One chronological feed: chat, joins, follows, shares, likes and gifts in
    // the order they happened, so the newest line is always at the bottom. The
    // buffer is reused, so this costs no allocation while drawing.
    live.timeline_into(chat, activity, gifts, feed_entries);
    if feed_entries.is_empty() {
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(20.0);
            ui.label(
                RichText::new("Waiting for events…")
                    .color(p.muted)
                    .size(13.0),
            );
        });
        return;
    }

    for entry in feed_entries.iter() {
        match entry.kind {
            FeedKind::Chat => {
                if let Some(line) = live.chat_at(entry.index) {
                    chat_row(ui, images, line, p);
                }
            }
            FeedKind::Activity => {
                if let Some(line) = live.activity_at(entry.index) {
                    activity_row(ui, images, line, p);
                }
            }
            FeedKind::Gift => {
                if let Some(line) = live.gift_at(entry.index) {
                    gift_row(ui, images, line, p);
                }
            }
        }
    }
}

fn chat_row(ui: &mut egui::Ui, images: &mut ImageCache, line: &ChatLine, p: Palette) {
    ui.horizontal_wrapped(|ui| {
        // Tight spacing so inline emoji sit against the text.
        ui.spacing_mut().item_spacing.x = 3.0;
        ui.add_space(20.0);
        if let Some(url) = &line.avatar_url {
            let uri = images.uri(url);
            avatar_img(ui, uri.as_deref(), 28.0);
            ui.add_space(4.0);
        }
        let mut name = RichText::new(format!("{}:", line.user))
            .color(p.accent)
            .size(16.0)
            .strong();
        if line.subscriber {
            name = name.color(p.ok);
        }
        ui.label(name);
        ui.add_space(2.0);
        chat_message(ui, images, line, p);
    });
    ui.add_space(4.0);
}

fn activity_row(
    ui: &mut egui::Ui,
    images: &mut ImageCache,
    line: &crate::domain::monitor::ActivityLine,
    p: Palette,
) {
    // Likes are the one activity worth calling out, and they carry a count.
    let text = match line.kind {
        ActivityKind::Like if line.count > 1 => {
            format!("{} liked ❤️ ×{}", line.user, line.count)
        }
        ActivityKind::Like => format!("{} liked ❤️", line.user),
        kind => format!("{} {}", line.user, kind.label()),
    };
    let color = if line.kind == ActivityKind::Like {
        p.text_soft
    } else {
        p.muted
    };
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        ui.add_space(20.0);
        if let Some(url) = &line.avatar_url {
            let uri = images.uri(url);
            avatar_img(ui, uri.as_deref(), 22.0);
            ui.add_space(4.0);
        }
        let pieces = emoji::split(&text, 17.0);
        emoji::render(ui, images, &pieces, color);
    });
    ui.add_space(2.0);
}

fn gift_row(ui: &mut egui::Ui, images: &mut ImageCache, gift: &GiftLine, p: Palette) {
    let count = if gift.combo {
        format!(" ×{}", gift.count)
    } else {
        String::new()
    };
    let suffix = if gift.final_streak {
        ""
    } else {
        " (sending…)"
    };
    ui.horizontal(|ui| {
        ui.add_space(20.0);
        if let Some(url) = &gift.image_url {
            let uri = images.uri(url);
            gift_img(ui, uri.as_deref(), 32.0);
            ui.add_space(6.0);
        }
        ui.label(
            RichText::new(format!("{} sent {}{}", gift.user, gift.gift, count))
                .color(p.text)
                .size(16.0)
                .strong(),
        );
        ui.label(
            RichText::new(format!("· {} diamonds{suffix}", gift.diamonds))
                .color(p.muted)
                .size(15.0),
        );
    });
    ui.add_space(2.0);
}

/// Render a comment: custom emote images are spliced in at their UTF-16
/// offsets, and colour emoji are drawn as images.
fn chat_message(ui: &mut egui::Ui, images: &mut ImageCache, line: &ChatLine, p: Palette) {
    const EMOJI: f32 = 19.0;
    const CUSTOM_EMOTE: f32 = 26.0;

    let mut pieces: Vec<Piece> = Vec::new();
    if line.emotes.is_empty() {
        pieces = emoji::split(&line.message, EMOJI);
    } else {
        let units: Vec<u16> = line.message.encode_utf16().collect();
        let mut emotes: Vec<&ChatEmote> = line.emotes.iter().collect();
        emotes.sort_by_key(|e| e.position);
        let mut last = 0usize;
        for emote in emotes {
            let pos = emote.position.min(units.len());
            if pos > last {
                pieces.extend(emoji::split(
                    &String::from_utf16_lossy(&units[last..pos]),
                    EMOJI,
                ));
            }
            pieces.push(Piece::Image {
                url: emote.url.clone(),
                fallback: String::new(),
                size: CUSTOM_EMOTE,
            });
            last = pos;
        }
        if last < units.len() {
            pieces.extend(emoji::split(
                &String::from_utf16_lossy(&units[last..]),
                EMOJI,
            ));
        }
    }
    emoji::render(ui, images, &pieces, p.text);
}

// -- Streamlabs account

fn page_account(ui: &mut egui::Ui, app: &mut App) {
    ui.allocate_ui_with_layout(
        Vec2::new(640.0f32.min(ui.available_width()), 0.0),
        Layout::top_down(Align::Min),
        |ui| {
            widgets::section(ui, "Streamlabs account", |ui| {
                let p = theme::pal(ui.ctx());
                ui.horizontal(|ui| {
                    widgets::avatar(ui, &app.display_name, 56.0);
                    ui.add_space(12.0);
                    ui.vertical(|ui| {
                        let name = app
                            .account
                            .as_ref()
                            .map(|a| a.username.clone())
                            .unwrap_or_else(|| "Not signed in".to_string());
                        ui.label(RichText::new(name).size(15.0).strong());
                        let sub = match &app.account {
                            Some(info) => format!("{}, can go live", info.application_status),
                            None => "Load a token on the Stream setup page".to_string(),
                        };
                        ui.label(RichText::new(sub).color(p.muted).size(13.0));
                    });
                });
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    let _sign_out = widgets::secondary_button(ui, "Sign out", false)
                        .on_disabled_hover_text("Sign-out is not available in this build.");
                    ui.add_space(8.0);
                    if widgets::button(ui, "Go to Stream setup", true, BtnKind::Primary).clicked() {
                        app.page = Page::Setup;
                    }
                });
            });
        },
    );
}

// -- OBS connection

fn page_obs(ui: &mut egui::Ui, app: &mut App) {
    ui.allocate_ui_with_layout(
        Vec2::new(640.0f32.min(ui.available_width()), 0.0),
        Layout::top_down(Align::Min),
        |ui| {
            widgets::section(ui, "Connection", |ui| {
                ui.horizontal_top(|ui| {
                    let port_width = 120.0;
                    let host_width = (ui.available_width() - port_width - 10.0).max(120.0);
                    ui.allocate_ui_with_layout(
                        Vec2::new(host_width, 0.0),
                        Layout::top_down(Align::Min),
                        |ui| widgets::field(ui, "Host", &mut app.obs_host, true, false),
                    );
                    ui.add_space(10.0);
                    ui.allocate_ui_with_layout(
                        Vec2::new(port_width, 0.0),
                        Layout::top_down(Align::Min),
                        |ui| widgets::field(ui, "Port", &mut app.obs_port, true, false),
                    );
                });
                ui.add_space(12.0);
                widgets::field(ui, "Password", &mut app.obs_password, true, true);
                ui.add_space(12.0);
                ui.checkbox(&mut app.obs_remember, "Remember securely");
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    let connected = app.obs_connection.is_connected();
                    if widgets::button(ui, "Connect", !connected, BtnKind::Primary).clicked() {
                        connect_obs(app, false);
                    }
                    ui.add_space(8.0);
                    if widgets::secondary_button(ui, "Connect (saved password)", !connected)
                        .clicked()
                    {
                        connect_obs(app, true);
                    }
                    ui.add_space(8.0);
                    if widgets::secondary_button(ui, "Disconnect", connected).clicked() {
                        app.coordinator.send(crate::domain::Command::ObsDisconnect);
                    }
                    ui.add_space(8.0);
                    if widgets::secondary_button(ui, "Refresh status", connected).clicked() {
                        app.coordinator
                            .send(crate::domain::Command::ObsRefreshStatus);
                    }
                });
                if let ObsConnectionState::Failed { message } = &app.obs_connection {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(message)
                            .color(theme::pal(ui.ctx()).err)
                            .size(12.0),
                    );
                }
            });
            ui.add_space(32.0);
            widgets::section(ui, "Output", |ui| {
                let p = theme::pal(ui.ctx());
                ui.horizontal(|ui| {
                    ui.label(RichText::new("State:").color(p.muted).size(13.0));
                    ui.label(
                        RichText::new(output_label(app.obs_output))
                            .color(p.text)
                            .size(13.0)
                            .strong(),
                    );
                });
                ui.add_space(6.0);
                ui.label(
                    RichText::new(
                        "Starting and ending output is done with Go Live and End Live below.",
                    )
                    .color(p.muted)
                    .size(13.0),
                );
            });
        },
    );
}

// -- Activity log

fn page_log(ui: &mut egui::Ui, app: &mut App) {
    // A visible, bordered log that reaches down near the Go Live bar but stays
    // left-aligned (fixed narrow column, never full width).
    let column_width = 760.0_f32.min(ui.available_width());
    let list_height = (ui.available_height() - 200.0).max(240.0);
    ui.allocate_ui_with_layout(
        Vec2::new(column_width, 0.0),
        Layout::top_down(Align::Min),
        |ui| {
            widgets::section(ui, "Activity log", |ui| {
                let p = theme::pal(ui.ctx());
                if widgets::secondary_button(ui, "Clear", !app.logs.is_empty()).clicked() {
                    app.logs.clear();
                }
                ui.add_space(10.0);
                widgets::panel_frame(ui.ctx()).show(ui, |ui| {
                    ui.set_min_height(list_height);
                    egui::ScrollArea::vertical()
                        .id_salt("activity_log")
                        .auto_shrink([false, false])
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            if app.logs.is_empty() {
                                ui.label(RichText::new("Nothing yet.").color(p.muted).size(13.0));
                            }
                            for line in &app.logs {
                                ui.horizontal(|ui| {
                                    if app.show_timestamps {
                                        ui.label(
                                            RichText::new(app.log_stamp(line.at))
                                                .monospace()
                                                .color(p.timestamp),
                                        );
                                    }
                                    ui.label(
                                        RichText::new(&line.message).monospace().color(p.text_soft),
                                    );
                                });
                            }
                        });
                });
            });
        },
    );
}

// -- Settings

fn page_settings(ui: &mut egui::Ui, app: &mut App) {
    ui.allocate_ui_with_layout(
        Vec2::new(640.0f32.min(ui.available_width()), 0.0),
        Layout::top_down(Align::Min),
        |ui| {
            let p = theme::pal(ui.ctx());
            widgets::section(ui, "Profile", |ui| {
                ui.label(RichText::new("Display name").color(p.text_soft).size(13.0));
                ui.add_space(6.0);
                ui.label(
                    RichText::new(&app.display_name)
                        .color(p.text)
                        .size(16.0)
                        .strong(),
                );
                ui.add_space(6.0);
                ui.label(
                    RichText::new(
                        "Taken from your signed-in account (Token loader) and shown under the app name in the sidebar.",
                    )
                    .color(p.muted)
                    .size(13.0),
                );
            });
            ui.add_space(32.0);

            widgets::section(ui, "Activity log", |ui| {
                ui.checkbox(&mut app.show_timestamps, "Show timestamps");
            });
            ui.add_space(32.0);

            widgets::section(ui, "Appearance", |ui| {
                ui.label(
                    RichText::new(
                        "Choose how ShuKanji looks. System follows your device. Tebby is a lavender purple theme with the ShuKanji mascot.",
                    )
                    .color(p.muted)
                    .size(13.0),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    theme_card(ui, app, theme::ThemeChoice::System);
                    ui.add_space(12.0);
                    theme_card(ui, app, theme::ThemeChoice::Dark);
                    ui.add_space(12.0);
                    theme_card(ui, app, theme::ThemeChoice::Light);
                    ui.add_space(12.0);
                    theme_card(ui, app, theme::ThemeChoice::Tebby);
                });
                ui.add_space(12.0);
                let mut optimize = app.optimize_mode;
                if ui
                    .checkbox(&mut optimize, "Optimize Mode")
                    .on_hover_text("Static Tebby mascot and shorter UI transitions")
                    .changed()
                {
                    app.set_optimize_mode(ui.ctx(), optimize);
                }
                ui.label(
                    RichText::new(
                        "Uses static Tebby art and shorter transitions to reduce RAM and rendering work. Live chat monitoring stays active.",
                    )
                    .color(p.muted)
                    .size(12.0),
                );
                ui.add_space(8.0);
                let mut background = app.background_mode;
                if ui
                    .checkbox(&mut background, "Minimize to background (low power)")
                    .on_hover_text("Minimize and release visual resources while monitoring continues")
                    .changed()
                {
                    app.set_background_mode(ui.ctx(), background);
                }
                ui.label(
                    RichText::new(
                        "Pauses visual rendering and releases Tebby/image textures while minimized. Restore the window to resume.",
                    )
                    .color(p.muted)
                    .size(12.0),
                );
            });
            ui.add_space(32.0);

            widgets::section(ui, "Chat fonts", |ui| {
                chat_fonts_section(ui, app);
            });
            ui.add_space(32.0);

            widgets::section(ui, "Stored data", |ui| {
                ui.label(
                    RichText::new("Removes the saved token and OBS password from this app.")
                        .color(p.muted)
                        .size(13.0),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if widgets::danger_button(ui, "Remove stored credentials", true).clicked() {
                        app.confirm = Some(Confirm::RemoveStored);
                    }
                    ui.add_space(8.0);
                    if widgets::secondary_button(ui, "Save config", true).clicked() {
                        app.save_config();
                    }
                });
            });
            ui.add_space(32.0);

            widgets::section(ui, "Shortcuts", |ui| {
                ui.label(
                    RichText::new(
                        "Ctrl+B  minimize the sidebar\nCtrl+T  switch dark / light theme\nEsc  close a dialog",
                    )
                        .color(p.muted)
                        .size(13.0),
                );
            });
        },
    );
}

/// Settings → Chat fonts: custom font folder, reload, scan report and a live
/// multi-script preview so coverage problems are visible immediately.
const FONT_PREVIEW: &str =
    "Preview: English · 日本語 · 中文 · 한국어 · العربية · ไทย · Ελληνικά · Кириллица";

fn chat_fonts_section(ui: &mut egui::Ui, app: &mut App) {
    let p = theme::pal(ui.ctx());
    ui.label(
        RichText::new(
            "Viewer comments can use any language. ShuKanji loads the matching Windows fallback the first time a script appears (Japanese, Chinese, Korean, Arabic, Thai and more), and you can add your own fonts below.",
        )
        .color(p.muted)
        .size(13.0),
    );
    ui.add_space(10.0);

    if let Some(dir) = app.font_report.custom_dir.clone() {
        ui.label(
            RichText::new("Drop .ttf / .otf / .ttc files into this folder, then press Reload fonts:")
                .color(p.text_soft)
                .size(13.0),
        );
        ui.add_space(4.0);
        ui.label(
            RichText::new(dir.display().to_string())
                .color(p.muted)
                .size(12.0)
                .family(egui::FontFamily::Monospace),
        );
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if widgets::secondary_button(ui, "Open fonts folder", true).clicked() {
                match std::fs::create_dir_all(&dir) {
                    Ok(()) => {
                        let _ = open::that(&dir);
                    }
                    Err(e) => {
                        app.toast = Some(crate::app::Toast {
                            level: LogLevel::Error,
                            message: format!("Could not create fonts folder: {e}"),
                            until: std::time::Instant::now()
                                + std::time::Duration::from_secs(6),
                        });
                    }
                }
            }
            ui.add_space(8.0);
            if widgets::secondary_button(ui, "Reload fonts", true).clicked() {
                app.fonts.reload();
                app.toast(LogLevel::Info, "Rescanning fonts…");
            }
        });
    } else {
        ui.label(
            RichText::new("Could not resolve an app-data folder for custom fonts.")
                .color(p.warn)
                .size(13.0),
        );
    }

    ui.add_space(12.0);
    let custom = if app.font_report.custom.is_empty() {
        "Custom fonts: none (system fallbacks active)".to_owned()
    } else {
        format!("Custom fonts: {}", app.font_report.custom.join(", "))
    };
    ui.label(RichText::new(custom).color(p.text_soft).size(12.0));
    ui.label(
        RichText::new(format!(
            "System fallbacks active: {} (other scripts load on demand)",
            app.font_report.system.len()
        ))
        .color(p.muted)
        .size(12.0),
    );
    if !app.font_report.failed.is_empty() {
        ui.label(
            RichText::new(format!(
                "Could not read: {}",
                app.font_report.failed.join(", ")
            ))
            .color(p.warn)
            .size(12.0),
        );
    }

    ui.add_space(12.0);
    // Opening this section is an explicit request to test coverage, so pull
    // every script the preview uses (they stay installed for the session).
    app.fonts.ensure_text(FONT_PREVIEW);
    ui.label(RichText::new(FONT_PREVIEW).color(p.text).size(16.0));
}

fn theme_card(ui: &mut egui::Ui, app: &mut App, choice: theme::ThemeChoice) {
    let p = theme::pal(ui.ctx());
    let selected = app.theme_choice == choice;
    let size = Vec2::new(140.0, 96.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());

    let ht = widgets::hover_t(ui, &response, response.hovered() && !selected);
    let hover_fill = widgets::lerp_color(Color32::TRANSPARENT, p.hover, ht);
    if hover_fill.a() > 0 {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(8), hover_fill);
    }
    let stroke = if selected {
        Stroke::new(2.0_f32, p.accent)
    } else {
        Stroke::new(1.0_f32, p.field_border)
    };
    ui.painter()
        .rect_stroke(rect, CornerRadius::same(8), stroke, StrokeKind::Inside);

    let pad = 8.0;
    let preview = Rect::from_min_size(
        rect.min + Vec2::splat(pad),
        Vec2::new(size.x - pad * 2.0, 56.0),
    );
    draw_preview(ui, preview, choice);
    ui.painter().text(
        egui::pos2(rect.left() + pad, rect.bottom() - pad),
        Align2::LEFT_BOTTOM,
        choice.label(),
        FontId::proportional(13.0),
        p.text,
    );

    if response.clicked() {
        app.change_theme(ui.ctx(), choice);
    }
}

fn draw_preview(ui: &mut egui::Ui, rect: Rect, choice: theme::ThemeChoice) {
    let painter = ui.painter();
    let stripe = |c: theme::Palette, r: Rect| {
        (
            Rect::from_min_size(r.min, Vec2::new(r.width() * 0.34, r.height())),
            c,
        )
    };
    let swatch = |c: theme::Palette, r: Rect| {
        painter.rect_filled(r, CornerRadius::same(5), c.bg);
        let (sb, cc) = stripe(c, r);
        painter.rect_filled(sb, CornerRadius::ZERO, cc.sidebar);
        let bar = Rect::from_min_size(
            r.min + Vec2::new(r.width() * 0.42, 12.0),
            Vec2::new(r.width() * 0.28, 6.0),
        );
        painter.rect_filled(bar, CornerRadius::same(3), cc.accent);
    };
    match choice {
        theme::ThemeChoice::Dark => swatch(theme::DARK, rect),
        theme::ThemeChoice::Light => swatch(theme::LIGHT, rect),
        theme::ThemeChoice::Tebby => {
            theme::vertical_gradient(painter, rect, rgb(0xE2D7F6), rgb(0xB9A6E2));
            let (sb, cc) = stripe(theme::TEBBY, rect);
            painter.rect_filled(
                sb,
                CornerRadius::ZERO,
                egui::Color32::from_rgba_unmultiplied(0xCD, 0xBD, 0xEB, 200),
            );
            let bar = Rect::from_min_size(
                rect.min + Vec2::new(rect.width() * 0.42, 12.0),
                Vec2::new(rect.width() * 0.28, 6.0),
            );
            painter.rect_filled(bar, CornerRadius::same(3), cc.accent);
        }
        theme::ThemeChoice::System => {
            let half = rect.width() / 2.0;
            let left = Rect::from_min_size(rect.min, Vec2::new(half, rect.height()));
            let right = Rect::from_min_size(
                rect.min + Vec2::new(half, 0.0),
                Vec2::new(half, rect.height()),
            );
            swatch(theme::DARK, left);
            swatch(theme::LIGHT, right);
        }
    }
}

fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

// -------------------------------------------------------------------- dialog

fn confirm_dialog(app: &mut App, ctx: &egui::Context) {
    let Some(kind) = app.confirm else {
        return;
    };
    let p = theme::pal(ctx);
    let (title, body, ok_label, danger) = match kind {
        Confirm::GoLive => (
            "Start streaming?",
            "This connects OBS, applies the stream key and starts OBS output.",
            "Go Live",
            false,
        ),
        Confirm::EndLive => (
            "End the stream?",
            "This stops OBS output and ends the Streamlabs session.",
            "End Live",
            true,
        ),
        Confirm::RemoveStored => (
            "Remove stored credentials?",
            "The saved token and OBS password are removed from this computer.",
            "Remove",
            true,
        ),
    };

    let mut activate = false;
    let mut cancel = false;

    egui::Modal::new(egui::Id::new("shukanji_confirm"))
        .backdrop_color(Color32::from_black_alpha(160))
        .frame(
            egui::Frame::new()
                .fill(p.panel)
                .stroke(Stroke::new(1.0_f32, p.field_border))
                .corner_radius(CornerRadius::same(10))
                .inner_margin(Margin::symmetric(24, 22)),
        )
        .show(ctx, |ui| {
            ui.set_width(360.0);
            ui.label(RichText::new(title).size(16.0).strong());
            ui.add_space(6.0);
            ui.label(RichText::new(body).color(p.text_soft).size(13.0));
            ui.add_space(18.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let kind_btn = if danger {
                    BtnKind::Danger
                } else {
                    BtnKind::Primary
                };
                if widgets::button(ui, ok_label, true, kind_btn).clicked() {
                    activate = true;
                }
                ui.add_space(8.0);
                if widgets::secondary_button(ui, "Cancel", true).clicked() {
                    cancel = true;
                }
            });
        });

    if activate {
        app.confirm = None;
        match kind {
            Confirm::GoLive => go_live(app),
            Confirm::EndLive => end_live(app),
            Confirm::RemoveStored => app
                .coordinator
                .send(crate::domain::Command::ClearStoredSecrets),
        }
    } else if cancel {
        app.confirm = None;
    }
}

fn toast(app: &mut App, ctx: &egui::Context) {
    let Some(t) = &app.toast else {
        return;
    };
    let p = theme::pal(ctx);
    let color = match t.level {
        LogLevel::Info => p.text,
        LogLevel::Warn => p.warn,
        LogLevel::Error => p.err,
    };
    let message = t.message.clone();
    egui::Area::new(egui::Id::new("toast"))
        .anchor(Align2::CENTER_BOTTOM, Vec2::new(0.0, -92.0))
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(p.panel)
                .stroke(Stroke::new(1.0_f32, p.field_border))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(Margin::symmetric(14, 9))
                .show(ui, |ui| {
                    ui.label(RichText::new(message).color(color).size(13.0));
                });
        });
}

// ------------------------------------------------------------------- helpers

/// A round remote image (viewer avatar). Loaded asynchronously and cached by
/// the egui_extras HTTP loader; nothing is drawn until it arrives (no
/// placeholder). Animated GIF/WebP are animated by the loaders.
fn avatar_img(ui: &mut egui::Ui, uri: Option<&str>, size: f32) {
    if let Some(uri) = uri {
        ui.add(
            egui::Image::from_uri(uri.to_string())
                .fit_to_exact_size(Vec2::splat(size))
                .corner_radius(CornerRadius::same((size / 2.0).round() as u8))
                .show_loading_spinner(false),
        );
    }
}

/// A square remote image (gift artwork, static or animated).
fn gift_img(ui: &mut egui::Ui, uri: Option<&str>, size: f32) {
    if let Some(uri) = uri {
        ui.add(
            egui::Image::from_uri(uri.to_string())
                .fit_to_exact_size(Vec2::splat(size))
                .corner_radius(CornerRadius::same(4))
                .show_loading_spinner(false),
        );
    }
}

fn mask(key: &str) -> String {
    crate::diagnostics::mask_stream_key(key)
}

fn compact_or_dash(live: bool, n: i64) -> String {
    if live {
        App::compact(n)
    } else {
        "—".to_string()
    }
}

fn dash_if_not(live: bool, n: i64) -> String {
    if live {
        n.to_string()
    } else {
        "—".to_string()
    }
}

fn output_label(state: ObsOutputState) -> &'static str {
    match state {
        ObsOutputState::Active => "Live",
        ObsOutputState::Inactive => "Idle",
        ObsOutputState::Unknown => "State unknown",
    }
}

fn output_dot(p: Palette, state: ObsOutputState) -> Color32 {
    match state {
        ObsOutputState::Active => p.ok,
        ObsOutputState::Inactive => p.dot_idle,
        ObsOutputState::Unknown => p.warn,
    }
}

fn obs_dot(p: Palette, state: &ObsConnectionState) -> Color32 {
    match state {
        ObsConnectionState::Connected { .. } => p.ok,
        ObsConnectionState::Connecting | ObsConnectionState::Reconnecting { .. } => p.warn,
        ObsConnectionState::Failed { .. } => p.err,
        ObsConnectionState::Disconnected => p.dot_idle,
    }
}

fn account_label(state: &AuthState) -> &'static str {
    match state {
        AuthState::Disconnected => "Signed out",
        AuthState::Authenticating => "Signing in…",
        AuthState::Authenticated { .. } => "Authenticated",
        AuthState::Failed { .. } => "Failed",
    }
}

fn account_dot(p: Palette, state: &AuthState) -> Option<Color32> {
    match state {
        AuthState::Authenticated { .. } => Some(p.ok),
        AuthState::Authenticating => Some(p.warn),
        AuthState::Failed { .. } => Some(p.err),
        AuthState::Disconnected => None,
    }
}

fn live_status(pal: Palette, state: &LiveConnectionState) -> (Color32, &'static str) {
    let dot = match state {
        LiveConnectionState::Connected { .. } => pal.ok,
        LiveConnectionState::Connecting { .. } | LiveConnectionState::Reconnecting { .. } => {
            pal.warn
        }
        LiveConnectionState::Failed { .. } => pal.err,
        LiveConnectionState::Disconnected => pal.dot_idle,
    };
    (dot, state.label())
}

// ------------------------------------------------------------------- actions

fn go_live(app: &mut App) {
    let Some(token) = app.current_token.clone() else {
        return;
    };
    app.go_live_request_id += 1;
    let request_id = app.go_live_request_id;
    app.credentials_request_id = request_id;
    app.apply_request_id = request_id;
    app.go_live_busy = true;
    app.go_live_stage = Some("Connecting to OBS…".to_string());
    app.credential_state = crate::domain::states::CredentialState::Retrieving;
    app.apply_state = ApplyState::Idle;
    app.log("Go Live: starting");
    app.coordinator.send(crate::domain::Command::GoLive {
        token,
        title: app.title.clone(),
        game_mask_id: app.game_mask_id.clone(),
        audience: if app.mature {
            AudienceType::Mature
        } else {
            AudienceType::General
        },
        host: app.obs_host.clone(),
        port: app.obs_port.parse().unwrap_or(4455),
        password: if app.obs_password.is_empty() {
            None
        } else {
            Some(SecretString::new(app.obs_password.clone()))
        },
        remember_password: app.obs_remember,
        request_id,
    });
}

fn end_live(app: &mut App) {
    if let Some(stream_id) = app.credentials.as_ref().and_then(|c| c.stream_id.clone())
        && let Some(token) = app.current_token.clone()
    {
        app.coordinator
            .send(crate::domain::Command::EndStreamlabsStream { token, stream_id });
    }
    if app.obs_connection.is_connected() && app.obs_output == ObsOutputState::Active {
        app.coordinator.send(crate::domain::Command::ObsStopStream);
    }
    app.log("End Live requested");
}

fn connect_obs(app: &mut App, stored: bool) {
    let host = app.obs_host.clone();
    let port = app.obs_port.parse().unwrap_or(4455);
    if stored {
        app.coordinator
            .send(crate::domain::Command::ObsConnectStored { host, port });
    } else {
        let password = if app.obs_password.is_empty() {
            None
        } else {
            Some(SecretString::new(app.obs_password.clone()))
        };
        app.coordinator.send(crate::domain::Command::ObsConnect {
            host,
            port,
            password,
            remember_password: app.obs_remember,
        });
    }
}

fn start_live_monitor(app: &mut App) {
    let username = app.live_username.trim().trim_start_matches('@').to_string();
    if username.is_empty() {
        return;
    }
    app.live_username = username.clone();
    app.log(format!("Connecting LIVE monitor for @{username}"));
    app.coordinator
        .send(crate::domain::Command::StartLiveMonitor { username });
}

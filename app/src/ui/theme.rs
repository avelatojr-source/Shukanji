//! ShuKanji theme: dark + light palettes, fonts and spacing.
//!
//! Colors come from `tokens-themes.json`. Flat look: hairline strokes, 6 px
//! radii, no gradients, glows or shadows, one pink accent.
//!
//! Font installation lives in [`super::fonts`] (bundled faces plus user and
//! system fallbacks so chat can render any script).

use egui::{Color32, FontFamily};

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// All theme colors for one mode.
#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Color32,
    pub sidebar: Color32,
    pub line: Color32,
    pub field_border: Color32,
    pub hover: Color32,
    pub nav_hover: Color32,
    pub panel: Color32,
    pub text: Color32,
    pub text_soft: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub accent_text: Color32,
    pub ok: Color32,
    pub warn: Color32,
    pub err: Color32,
    pub dot_idle: Color32,
    pub disabled_fill: Color32,
    pub disabled_text: Color32,
    pub danger_border: Color32,
    pub danger_hover: Color32,
    pub event_pill: Color32,
    pub timestamp: Color32,
}

pub const DARK: Palette = Palette {
    bg: rgb(0x131017),
    sidebar: rgb(0x100E14),
    line: rgb(0x2A2530),
    field_border: rgb(0x38323F),
    hover: rgb(0x221E29),
    nav_hover: rgb(0x1A171F),
    panel: rgb(0x1A171F),
    text: rgb(0xECE8EF),
    text_soft: rgb(0xB8B0C1),
    muted: rgb(0x9A92A3),
    accent: rgb(0xE58FB2),
    accent_hover: rgb(0xEBA0BF),
    accent_text: rgb(0x1A0F16),
    ok: rgb(0x6FCF97),
    warn: rgb(0xE0A84F),
    err: rgb(0xE5736F),
    dot_idle: rgb(0x6B6475),
    disabled_fill: rgb(0x1E1A24),
    disabled_text: rgb(0x7E7788),
    danger_border: rgb(0x6B3A3A),
    danger_hover: rgb(0x2A1A1D),
    event_pill: rgb(0x221C29),
    timestamp: rgb(0x8B8494),
};

pub const LIGHT: Palette = Palette {
    bg: rgb(0xF7F5F9),
    sidebar: rgb(0xFFFFFF),
    line: rgb(0xE4DFE9),
    field_border: rgb(0xCFC8D6),
    hover: rgb(0xEEEAF2),
    nav_hover: rgb(0xF1EEF4),
    panel: rgb(0xFFFFFF),
    text: rgb(0x1C1822),
    text_soft: rgb(0x4A4353),
    muted: rgb(0x6B6475),
    accent: rgb(0xC7467F),
    accent_hover: rgb(0xB53A72),
    accent_text: rgb(0xFFFFFF),
    ok: rgb(0x1F8F55),
    warn: rgb(0xA86400),
    err: rgb(0xC93C37),
    dot_idle: rgb(0xA39BAD),
    disabled_fill: rgb(0xECE8EF),
    disabled_text: rgb(0x9A92A3),
    danger_border: rgb(0xE3A9A6),
    danger_hover: rgb(0xFBECEC),
    event_pill: rgb(0xF5E8EF),
    timestamp: rgb(0x7B7385),
};

/// Tebby: lavender/purple theme (from the supplied HTML `data-theme=tebby`).
pub const TEBBY: Palette = Palette {
    bg: rgb(0xD9CCF2),
    sidebar: rgb(0xCDBDEB),
    line: rgb(0xB3A0DC),
    field_border: rgb(0x9C86CE),
    hover: rgb(0xC4B2E6),
    nav_hover: rgb(0xC9B8EA),
    panel: rgb(0xE6DCF7),
    text: rgb(0x241A3D),
    text_soft: rgb(0x3E3263),
    muted: rgb(0x54487C),
    accent: rgb(0x9B80E0),
    accent_hover: rgb(0x8B6DD6),
    accent_text: rgb(0x1E0F38),
    ok: rgb(0x1B7A4B),
    warn: rgb(0x8F5300),
    err: rgb(0xB3322E),
    dot_idle: rgb(0x8574B0),
    disabled_fill: rgb(0xC7B6E8),
    disabled_text: rgb(0x6F6296),
    danger_border: rgb(0xD49AA6),
    danger_hover: rgb(0xEBD3E0),
    event_pill: rgb(0xD3C2EF),
    timestamp: rgb(0x5E5285),
};

/// The user's theme choice (egui only knows Dark/Light, so Tebby is tracked
/// here and mapped onto the Light base visuals).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ThemeChoice {
    #[default]
    System,
    Dark,
    Light,
    Tebby,
}

impl ThemeChoice {
    pub fn label(self) -> &'static str {
        match self {
            ThemeChoice::System => "System",
            ThemeChoice::Dark => "Dark",
            ThemeChoice::Light => "Light",
            ThemeChoice::Tebby => "Tebby",
        }
    }

    /// The egui preference this choice maps to.
    pub fn egui_pref(self) -> egui::ThemePreference {
        match self {
            ThemeChoice::Dark => egui::ThemePreference::Dark,
            ThemeChoice::Light | ThemeChoice::Tebby => egui::ThemePreference::Light,
            ThemeChoice::System => egui::ThemePreference::System,
        }
    }
}

fn choice_id() -> egui::Id {
    egui::Id::new("shukanji.theme_choice")
}

/// Store the active theme choice for the frame (read back by [`pal`]).
pub fn set_choice(ctx: &egui::Context, choice: ThemeChoice) {
    ctx.data_mut(|d| d.insert_temp(choice_id(), choice));
}

pub fn choice(ctx: &egui::Context) -> ThemeChoice {
    ctx.data(|d| d.get_temp::<ThemeChoice>(choice_id()))
        .unwrap_or_default()
}

/// The palette for whichever theme is active right now.
pub fn pal(ctx: &egui::Context) -> Palette {
    match choice(ctx) {
        ThemeChoice::Tebby => TEBBY,
        ThemeChoice::Dark => DARK,
        ThemeChoice::Light => LIGHT,
        ThemeChoice::System => match ctx.theme() {
            egui::Theme::Dark => DARK,
            egui::Theme::Light => LIGHT,
        },
    }
}

/// Paint a vertical two-color gradient (used by the Tebby background).
pub fn vertical_gradient(painter: &egui::Painter, rect: egui::Rect, top: Color32, bottom: Color32) {
    let mut mesh = egui::Mesh::default();
    for (pos, color) in [
        (rect.left_top(), top),
        (rect.right_top(), top),
        (rect.left_bottom(), bottom),
        (rect.right_bottom(), bottom),
    ] {
        mesh.vertices.push(egui::epaint::Vertex {
            pos,
            uv: egui::Pos2::ZERO,
            color,
        });
    }
    mesh.indices.extend_from_slice(&[0, 1, 2, 1, 3, 2]);
    painter.add(egui::Shape::mesh(mesh));
}

/// The font family name used only for the "ShuKanji" wordmark.
pub fn wordmark_family() -> FontFamily {
    FontFamily::Name("wordmark".into())
}

/// Install the bundled fonts, register both theme visuals and set spacing.
/// Call once. Custom + system fonts are loaded afterwards in the background
/// (see [`super::fonts::spawn_extended`]) so start-up never blocks on disk.
pub fn apply(ctx: &egui::Context) {
    super::fonts::install_bundled(ctx);
    ctx.set_visuals_of(egui::Theme::Dark, visuals(DARK, egui::Visuals::dark()));
    ctx.set_visuals_of(egui::Theme::Light, visuals(LIGHT, egui::Visuals::light()));
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(10.0, 10.0);
        style.spacing.button_padding = egui::vec2(16.0, 10.0);
        style.spacing.interact_size.y = 40.0;
        // Scroll bar flush against the window edge.
        style.spacing.scroll.bar_width = 10.0;
        style.spacing.scroll.bar_inner_margin = 0.0;
        style.spacing.scroll.bar_outer_margin = 0.0;
        style.spacing.scroll.floating = false;
    });
}

fn visuals(p: Palette, mut v: egui::Visuals) -> egui::Visuals {
    v.panel_fill = p.bg;
    v.window_fill = p.sidebar;
    v.extreme_bg_color = p.sidebar;
    v.override_text_color = Some(p.text);
    v.selection.bg_fill = p.accent;
    v.selection.stroke = egui::Stroke::new(2.0_f32, p.accent);
    v.hyperlink_color = p.accent;
    v.window_shadow = egui::Shadow::NONE;
    v.popup_shadow = egui::Shadow::NONE;

    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, p.line);
    v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, p.field_border);
    v.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    v.widgets.hovered.weak_bg_fill = p.hover;
    v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, p.field_border);
    v.widgets.active.weak_bg_fill = p.line;
    v.widgets.active.bg_stroke = egui::Stroke::new(2.0_f32, p.accent);
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = egui::CornerRadius::same(6);
    }
    v
}

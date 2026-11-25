//! ShuKanji flat widget helpers. All colors come from [`super::theme::pal`].

use egui::{
    Align2, Color32, CornerRadius, FontId, Margin, Rect, Response, RichText, Sense, Stroke,
    StrokeKind, Ui, Vec2,
};

use super::theme;

/// Which button style to draw.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BtnKind {
    Primary,
    /// The 44-high Go Live button.
    PrimaryLarge,
    Secondary,
    Danger,
}

const RADIUS: u8 = 6;

/// Seconds for hover/active fades. Short enough to feel instant, long enough
/// to read as a transition rather than a hard flip.
pub const TRANSITION_SECS: f32 = 0.14;

/// Linear RGBA mix between two colors (`t = 0` → `a`, `t = 1` → `b`).
pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(
        mix(a.r(), b.r()),
        mix(a.g(), b.g()),
        mix(a.b(), b.b()),
        mix(a.a(), b.a()),
    )
}

/// Multiply a color's alpha. Used to fade labels in/out while the sidebar
/// width animates between its minimized and expanded states.
pub fn with_alpha(c: Color32, alpha: f32) -> Color32 {
    let a = (c.a() as f32 * alpha.clamp(0.0, 1.0)).round() as u8;
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

/// 0 → 1 hover fade for a custom-painted widget. egui keeps requesting
/// repaints while the value animates, so no timer is needed.
pub fn hover_t(ui: &Ui, response: &Response, active: bool) -> f32 {
    ui.ctx()
        .animate_bool_with_time(response.id.with("hover"), active, TRANSITION_SECS)
}

/// 0 → 1 pressed fade; slightly faster than the hover fade so clicks feel snappy.
pub fn press_t(ui: &Ui, response: &Response, active: bool) -> f32 {
    ui.ctx()
        .animate_bool_with_time(response.id.with("press"), active, 0.08)
}

/// The three looks a button eases between (rest → hover → pressed).
#[derive(Clone, Copy)]
struct BtnState {
    fill: Color32,
    border: Color32,
    text: Color32,
}

fn lerp_state(a: BtnState, b: BtnState, t: f32) -> BtnState {
    BtnState {
        fill: lerp_color(a.fill, b.fill, t),
        border: lerp_color(a.border, b.border, t),
        text: lerp_color(a.text, b.text, t),
    }
}

/// A 1 px horizontal rule across the available width.
pub fn hairline(ui: &mut Ui) {
    let p = theme::pal(ui.ctx());
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 1.0), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::ZERO, p.line);
}

/// A section: 1 px rule, uppercase label, then the content. Never collapsible.
pub fn section(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui)) {
    let p = theme::pal(ui.ctx());
    hairline(ui);
    ui.add_space(14.0);
    ui.label(
        RichText::new(title.to_uppercase())
            .color(p.muted)
            .size(11.0)
            .strong(),
    );
    ui.add_space(6.0);
    add(ui);
}

/// A status item: 7 px dot, muted label, then the value.
pub fn status_item(ui: &mut Ui, label: &str, value: &str, dot: Option<Color32>) {
    let p = theme::pal(ui.ctx());
    let font = FontId::proportional(13.0);
    let label_galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font.clone(), p.muted);
    let value_galley = ui.painter().layout_no_wrap(value.to_owned(), font, p.text);

    // Exact size so this widget never expands to fill a parent row (which is
    // what broke right-alignment inside the header's right-to-left group).
    const DOT: f32 = 7.0;
    const GAP: f32 = 6.0;
    let label_w = label_galley.size().x;
    let value_w = value_galley.size().x;
    let width = DOT + GAP + label_w + GAP + value_w;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 20.0), Sense::hover());

    let cy = rect.center().y;
    let mut x = rect.left();
    ui.painter().circle_filled(
        egui::pos2(x + DOT / 2.0, cy),
        3.5,
        dot.unwrap_or(p.dot_idle),
    );
    x += DOT + GAP;
    let lh = label_galley.size().y;
    ui.painter()
        .galley(egui::pos2(x, cy - lh / 2.0), label_galley, p.muted);
    x += label_w + GAP;
    let vh = value_galley.size().y;
    ui.painter()
        .galley(egui::pos2(x, cy - vh / 2.0), value_galley, p.text);
}

/// Draw a flat button. Returns the response; acts only when `enabled`.
pub fn button(ui: &mut Ui, text: &str, enabled: bool, kind: BtnKind) -> Response {
    let p = theme::pal(ui.ctx());
    let height = if kind == BtnKind::PrimaryLarge {
        44.0
    } else {
        40.0
    };
    let font = FontId::proportional(14.0);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font.clone(), p.text);
    let pad = if kind == BtnKind::PrimaryLarge {
        28.0
    } else {
        16.0
    };
    let size = Vec2::new(galley.size().x + pad * 2.0, height);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);

    if !enabled {
        paint_disabled(ui, rect, text, font, p);
        return response;
    }

    let hovered = response.hovered() || response.has_focus();
    let pressed = response.is_pointer_button_down_on();

    // Rest → hover → pressed, all eased instead of flipping instantly.
    let (normal, over, down) = match kind {
        BtnKind::Primary | BtnKind::PrimaryLarge => (
            BtnState {
                fill: p.accent,
                border: p.accent,
                text: p.accent_text,
            },
            BtnState {
                fill: p.accent_hover,
                border: p.accent_hover,
                text: p.accent_text,
            },
            BtnState {
                fill: p.accent,
                border: p.accent,
                text: p.accent_text,
            },
        ),
        BtnKind::Secondary => (
            BtnState {
                fill: Color32::TRANSPARENT,
                border: p.field_border,
                text: p.text,
            },
            BtnState {
                fill: p.hover,
                border: p.field_border,
                text: p.text,
            },
            BtnState {
                fill: p.line,
                border: p.field_border,
                text: p.text,
            },
        ),
        BtnKind::Danger => (
            BtnState {
                fill: Color32::TRANSPARENT,
                border: p.danger_border,
                text: p.err,
            },
            BtnState {
                fill: p.danger_hover,
                border: p.danger_border,
                text: p.err,
            },
            BtnState {
                fill: lerp_color(p.danger_hover, p.err, 0.18),
                border: p.danger_border,
                text: p.err,
            },
        ),
    };
    let state = lerp_state(
        lerp_state(normal, over, hover_t(ui, &response, hovered)),
        down,
        press_t(ui, &response, pressed),
    );

    let radius = CornerRadius::same(RADIUS);
    if state.fill.a() > 0 {
        ui.painter().rect_filled(rect, radius, state.fill);
    }
    if kind != BtnKind::Primary && kind != BtnKind::PrimaryLarge {
        ui.painter().rect_stroke(
            rect,
            radius,
            Stroke::new(1.0_f32, state.border),
            StrokeKind::Inside,
        );
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            radius,
            Stroke::new(2.0_f32, p.accent),
            StrokeKind::Inside,
        );
    }
    ui.painter()
        .text(rect.center(), Align2::CENTER_CENTER, text, font, state.text);
    response
}

/// Static look for disabled buttons (no hover, nothing to animate).
fn paint_disabled(ui: &Ui, rect: Rect, text: &str, font: FontId, p: theme::Palette) {
    let radius = CornerRadius::same(RADIUS);
    ui.painter().rect_filled(rect, radius, p.disabled_fill);
    ui.painter()
        .text(rect.center(), Align2::CENTER_CENTER, text, font, p.disabled_text);
}

/// The one live-control: a wide rectangle with a smaller square inside it, so
/// the same control reads as "record" when off and "stop" when on. Replaces the
/// old pair of Go Live / End Live buttons.
pub fn stream_button(ui: &mut Ui, streaming: bool, enabled: bool) -> Response {
    let p = theme::pal(ui.ctx());
    let label = if streaming { "End Live" } else { "Go Live" };
    let font = FontId::proportional(14.0);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font.clone(), p.text);

    const SQUARE: f32 = 16.0;
    const PAD: f32 = 24.0;
    const GAP: f32 = 10.0;
    const HEIGHT: f32 = 44.0;
    let size = Vec2::new(PAD * 2.0 + SQUARE + GAP + galley.size().x, HEIGHT);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);

    if !enabled {
        // Static disabled look; the outlined variant keeps its border.
        if streaming {
            ui.painter().rect_stroke(
                rect,
                CornerRadius::same(6),
                Stroke::new(1.0_f32, p.field_border),
                StrokeKind::Inside,
            );
        } else {
            ui.painter()
                .rect_filled(rect, CornerRadius::same(6), p.disabled_fill);
        }
        let square_rect = Rect::from_min_size(
            egui::pos2(rect.left() + PAD, rect.center().y - SQUARE / 2.0),
            Vec2::splat(SQUARE),
        );
        if streaming {
            ui.painter().rect_stroke(
                square_rect,
                CornerRadius::same(3),
                Stroke::new(2.0_f32, p.disabled_text),
                StrokeKind::Inside,
            );
        } else {
            ui.painter()
                .rect_filled(square_rect, CornerRadius::same(3), p.disabled_text);
        }
        ui.painter().text(
            egui::pos2(square_rect.right() + GAP, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            font,
            p.disabled_text,
        );
        return response;
    }

    let hovered = response.hovered() || response.has_focus();
    let pressed = response.is_pointer_button_down_on();

    // Off = solid accent (record); on = outlined danger (stop).
    let (normal, over, down) = if streaming {
        (
            BtnState {
                fill: Color32::TRANSPARENT,
                border: p.danger_border,
                text: p.err,
            },
            BtnState {
                fill: p.danger_hover,
                border: p.danger_border,
                text: p.err,
            },
            BtnState {
                fill: lerp_color(p.danger_hover, p.err, 0.18),
                border: p.danger_border,
                text: p.err,
            },
        )
    } else {
        (
            BtnState {
                fill: p.accent,
                border: p.accent,
                text: p.accent_text,
            },
            BtnState {
                fill: p.accent_hover,
                border: p.accent_hover,
                text: p.accent_text,
            },
            BtnState {
                fill: p.accent,
                border: p.accent,
                text: p.accent_text,
            },
        )
    };
    let state = lerp_state(
        lerp_state(normal, over, hover_t(ui, &response, hovered)),
        down,
        press_t(ui, &response, pressed),
    );
    let fill = state.fill;
    let ink = state.text;

    let radius = CornerRadius::same(6);
    if fill.a() > 0 {
        ui.painter().rect_filled(rect, radius, fill);
    }
    if streaming {
        ui.painter().rect_stroke(
            rect,
            radius,
            Stroke::new(1.0_f32, state.border),
            StrokeKind::Inside,
        );
    }
    if response.has_focus() && enabled {
        ui.painter().rect_stroke(
            rect,
            radius,
            Stroke::new(2.0_f32, p.accent),
            StrokeKind::Inside,
        );
    }

    // The inner square: filled while recording, hollow while stopping.
    let square_rect = Rect::from_min_size(
        egui::pos2(rect.left() + PAD, rect.center().y - SQUARE / 2.0),
        Vec2::splat(SQUARE),
    );
    let square_radius = CornerRadius::same(3);
    if streaming {
        ui.painter().rect_stroke(
            square_rect,
            square_radius,
            Stroke::new(2.0_f32, ink),
            StrokeKind::Inside,
        );
    } else {
        ui.painter().rect_filled(square_rect, square_radius, ink);
    }

    ui.painter().text(
        egui::pos2(square_rect.right() + GAP, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        font,
        ink,
    );
    response
}

/// A small arrow toggle for minimizing/restoring the Go Live bar. Draws a
/// downward arrow while expanded (click = minimize) and an upward one while
/// collapsed (click = restore). Returns true when clicked.
pub fn collapse_toggle(ui: &mut Ui, collapsed: bool, tooltip: &str) -> bool {
    let p = theme::pal(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::click());
    let ht = hover_t(ui, &response, response.hovered());
    let bg = lerp_color(Color32::TRANSPARENT, p.nav_hover, ht);
    if bg.a() > 0 {
        ui.painter().rect_filled(rect, CornerRadius::same(4), bg);
    }
    let ink = lerp_color(p.muted, p.text, ht);
    let c = rect.center();
    let s = 4.2;
    let points = if collapsed {
        vec![
            c + egui::vec2(-s, s * 0.55),
            c + egui::vec2(s, s * 0.55),
            c + egui::vec2(0.0, -s * 0.9),
        ]
    } else {
        vec![
            c + egui::vec2(-s, -s * 0.55),
            c + egui::vec2(s, -s * 0.55),
            c + egui::vec2(0.0, s * 0.9),
        ]
    };
    ui.painter()
        .add(egui::Shape::convex_polygon(points, ink, Stroke::NONE));
    response.on_hover_text(tooltip).clicked()
}

/// A horizontal grip you drag with the mouse to resize the panel it sits under.
/// `value` is the height being resized; it is clamped so the panel can never be
/// made smaller than it originally was.
///
/// Returns the response so the caller can persist the change only while the
/// user is actually dragging.
pub fn vertical_grip(ui: &mut Ui, value: &mut f32, min: f32, max: f32) -> Response {
    let p = theme::pal(ui.ctx());
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 12.0), Sense::drag());
    let hot = response.hovered() || response.dragged();
    let t = ui
        .ctx()
        .animate_bool_with_time(response.id.with("hot"), hot, TRANSITION_SECS);
    let bar_width = 44.0 + (64.0 - 44.0) * t;
    let color = lerp_color(p.line, p.accent, t);
    ui.painter().rect_filled(
        Rect::from_center_size(rect.center(), Vec2::new(bar_width, 3.0)),
        CornerRadius::same(2),
        color,
    );

    if response.dragged() {
        *value = (*value + response.drag_delta().y).clamp(min, max);
    }
    if hot {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
    response.on_hover_text("Drag to resize")
}

pub fn primary_button(ui: &mut Ui, text: &str, enabled: bool) -> Response {
    button(ui, text, enabled, BtnKind::Primary)
}

pub fn secondary_button(ui: &mut Ui, text: &str, enabled: bool) -> Response {
    button(ui, text, enabled, BtnKind::Secondary)
}

pub fn danger_button(ui: &mut Ui, text: &str, enabled: bool) -> Response {
    button(ui, text, enabled, BtnKind::Danger)
}

/// A labelled text field. The caller decides masking via `password`.
pub fn field(ui: &mut Ui, label: &str, value: &mut String, mono: bool, password: bool) {
    let p = theme::pal(ui.ctx());
    ui.label(RichText::new(label).color(p.text_soft).size(13.0));
    ui.add_space(6.0);
    let mut edit = egui::TextEdit::singleline(value)
        .desired_width(f32::INFINITY)
        .margin(Margin::symmetric(12, 10));
    if mono {
        edit = edit.font(FontId::monospace(13.0));
    }
    if password {
        edit = edit.password(true);
    }
    ui.add_sized([ui.available_width(), 40.0], edit);
}

/// A circular avatar: first letter of `name` in pink on the hover color,
/// with a 2 px pink ring.
pub fn avatar(ui: &mut Ui, name: &str, size: f32) {
    let p = theme::pal(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let center = rect.center();
    let radius = size / 2.0;
    ui.painter().circle_filled(center, radius, p.hover);
    ui.painter()
        .circle_stroke(center, radius - 1.0, Stroke::new(2.0_f32, p.accent));
    let letter = name
        .trim()
        .chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "S".to_owned());
    ui.painter().text(
        center,
        Align2::CENTER_CENTER,
        letter,
        FontId::proportional(size * 0.38),
        p.accent,
    );
}

/// A circular avatar backed by a texture (used by the Tebby mascot face).
pub fn avatar_image(ui: &mut Ui, texture: &egui::TextureHandle, size: f32) {
    let p = theme::pal(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let sized = egui::load::SizedTexture::new(texture.id(), Vec2::splat(size));
    let radius = (size / 2.0).round() as u8;
    ui.put(
        rect,
        egui::Image::new(sized).corner_radius(CornerRadius::same(radius)),
    );
    ui.painter().circle_stroke(
        rect.center(),
        size / 2.0 - 1.0,
        Stroke::new(2.0_f32, p.accent),
    );
}

/// A sidebar nav row with an icon. `expand` is the sidebar's animated
/// expansion (1 = fully expanded, 0 = minimized rail): the icon slides from
/// its left slot to the rail center and the label fades with it.
pub fn nav_item(ui: &mut Ui, icon: &str, text: &str, active: bool, expand: f32) -> Response {
    let p = theme::pal(ui.ctx());
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 36.0), Sense::click());
    let radius = CornerRadius::same(RADIUS);
    // Ease both the selection change and the hover highlight.
    let at = ui
        .ctx()
        .animate_bool_with_time(response.id.with("active"), active, TRANSITION_SECS);
    let ht = hover_t(ui, &response, response.hovered());
    let idle_bg = lerp_color(Color32::TRANSPARENT, p.hover, at);
    let over_bg = lerp_color(p.nav_hover, p.hover, at);
    let bg = lerp_color(idle_bg, over_bg, ht);
    if bg.a() > 0 {
        ui.painter().rect_filled(rect, radius, bg);
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            radius,
            Stroke::new(2.0_f32, p.accent),
            StrokeKind::Inside,
        );
    }
    let tint = lerp_color(p.text_soft, p.text, at.max(ht));
    // Expanded: icon in its left slot beside the label. Minimized rail: icon
    // centered. The two positions are interpolated during the transition.
    let icon_x = if text.is_empty() {
        rect.center().x - 8.0
    } else {
        let expanded_x = rect.left() + 10.0;
        let rail_x = rect.center().x - 8.0;
        expanded_x + (rail_x - expanded_x) * (1.0 - expand)
    };
    let icon_rect =
        Rect::from_min_size(egui::pos2(icon_x, rect.center().y - 8.0), Vec2::splat(16.0));
    paint_icon(ui, icon, icon_rect, tint);
    if !text.is_empty() && expand > 0.02 {
        ui.painter().text(
            egui::pos2(rect.left() + 36.0, rect.center().y),
            Align2::LEFT_CENTER,
            text,
            FontId::proportional(14.0),
            with_alpha(tint, expand),
        );
    }
    response
}

/// Draw one of the bundled stroke icons, tinted. `"minimize"` is drawn as a
/// small panel glyph (no matching asset is shipped).
pub fn paint_icon(ui: &mut Ui, name: &str, rect: Rect, tint: Color32) {
    if name == "minimize" {
        let painter = ui.painter();
        let stroke = Stroke::new(1.6_f32, tint);
        painter.rect_stroke(rect, CornerRadius::same(2), stroke, StrokeKind::Inside);
        let x = rect.left() + rect.width() * 0.38;
        painter.line_segment(
            [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
            stroke,
        );
        return;
    }
    let image = egui::Image::new(icon_source(name))
        .tint(tint)
        .fit_to_exact_size(rect.size());
    ui.put(rect, image);
}

fn icon_source(name: &str) -> egui::ImageSource<'static> {
    let bytes: &'static [u8] = match name {
        "setup" => include_bytes!("../../assets/icons/setup.svg"),
        "live" => include_bytes!("../../assets/icons/live.svg"),
        "account" => include_bytes!("../../assets/icons/account.svg"),
        "obs" => include_bytes!("../../assets/icons/obs.svg"),
        "log" => include_bytes!("../../assets/icons/log.svg"),
        "settings" => include_bytes!("../../assets/icons/settings.svg"),
        _ => include_bytes!("../../assets/icons/setup.svg"),
    };
    egui::ImageSource::Bytes {
        uri: format!("bytes://icons/{name}.svg").into(),
        bytes: bytes.into(),
    }
}

/// A bordered panel frame used for the dashboard cards.
pub fn panel_frame(ctx: &egui::Context) -> egui::Frame {
    let p = theme::pal(ctx);
    egui::Frame::new()
        .fill(p.panel)
        .stroke(Stroke::new(1.0_f32, p.line))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(20, 16))
}

/// A small uppercase "sample"/label chip.
pub fn eyebrow(ui: &mut Ui, text: &str) {
    let p = theme::pal(ui.ctx());
    ui.label(
        RichText::new(text.to_uppercase())
            .color(p.muted)
            .size(11.0)
            .strong(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lerp_color_endpoints_and_midpoint() {
        // Opaque colors mix channel-by-channel exactly. (egui Color32 stores
        // premultiplied sRGB, so only opaque colors have exact channel reads.)
        let a = Color32::from_rgb(0, 100, 200);
        let b = Color32::from_rgb(200, 0, 100);
        assert_eq!(lerp_color(a, b, 0.0), a);
        assert_eq!(lerp_color(a, b, 1.0), b);
        assert_eq!(lerp_color(a, b, 0.5), Color32::from_rgb(100, 50, 150));
        // Out-of-range t is clamped, never wraps the channels.
        assert_eq!(lerp_color(a, b, 2.0), b);
    }

    #[test]
    fn transparent_fades_in() {
        // Exact endpoints, exact alpha ramp. (Channel reads of half-alpha
        // colors are premultiplied by egui, so only alpha is asserted here.)
        assert_eq!(
            lerp_color(Color32::TRANSPARENT, Color32::WHITE, 0.0),
            Color32::TRANSPARENT
        );
        assert_eq!(
            lerp_color(Color32::TRANSPARENT, Color32::WHITE, 1.0),
            Color32::WHITE
        );
        assert_eq!(lerp_color(Color32::TRANSPARENT, Color32::WHITE, 0.5).a(), 128);
    }
}

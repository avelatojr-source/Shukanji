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
    let hovered = enabled && response.hovered();

    let (fill, stroke, text_color) = match kind {
        BtnKind::Primary | BtnKind::PrimaryLarge => {
            if !enabled {
                (p.disabled_fill, Stroke::NONE, p.disabled_text)
            } else if hovered {
                (p.accent_hover, Stroke::NONE, p.accent_text)
            } else {
                (p.accent, Stroke::NONE, p.accent_text)
            }
        }
        BtnKind::Secondary => {
            if !enabled {
                (p.disabled_fill, Stroke::NONE, p.disabled_text)
            } else if hovered {
                (p.hover, Stroke::new(1.0_f32, p.field_border), p.text)
            } else {
                (
                    Color32::TRANSPARENT,
                    Stroke::new(1.0_f32, p.field_border),
                    p.text,
                )
            }
        }
        BtnKind::Danger => {
            if !enabled {
                (p.disabled_fill, Stroke::NONE, p.disabled_text)
            } else if hovered {
                (p.danger_hover, Stroke::new(1.0_f32, p.danger_border), p.err)
            } else {
                (
                    Color32::TRANSPARENT,
                    Stroke::new(1.0_f32, p.danger_border),
                    p.err,
                )
            }
        }
    };

    let radius = CornerRadius::same(RADIUS);
    if fill != Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, radius, fill);
    }
    if stroke != Stroke::NONE {
        ui.painter()
            .rect_stroke(rect, radius, stroke, StrokeKind::Inside);
    }
    if response.has_focus() && enabled {
        ui.painter().rect_stroke(
            rect,
            radius,
            Stroke::new(2.0_f32, p.accent),
            StrokeKind::Inside,
        );
    }
    ui.painter()
        .text(rect.center(), Align2::CENTER_CENTER, text, font, text_color);
    response
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
    let hovered = enabled && response.hovered();

    // Off = solid accent (record); on = outlined danger (stop).
    let (fill, stroke, ink) = if streaming {
        if !enabled {
            (
                Color32::TRANSPARENT,
                Stroke::new(1.0_f32, p.field_border),
                p.disabled_text,
            )
        } else if hovered {
            (p.danger_hover, Stroke::new(1.0_f32, p.danger_border), p.err)
        } else {
            (
                Color32::TRANSPARENT,
                Stroke::new(1.0_f32, p.danger_border),
                p.err,
            )
        }
    } else if !enabled {
        (p.disabled_fill, Stroke::NONE, p.disabled_text)
    } else if hovered {
        (p.accent_hover, Stroke::NONE, p.accent_text)
    } else {
        (p.accent, Stroke::NONE, p.accent_text)
    };

    let radius = CornerRadius::same(6);
    if fill != Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, radius, fill);
    }
    if stroke != Stroke::NONE {
        ui.painter()
            .rect_stroke(rect, radius, stroke, StrokeKind::Inside);
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
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(4), p.nav_hover);
    }
    let ink = if response.hovered() { p.text } else { p.muted };
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

    let (bar_width, color) = if hot {
        (64.0, p.accent)
    } else {
        (44.0, p.line)
    };
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

/// A sidebar nav row with an icon. Returns the response.
pub fn nav_item(ui: &mut Ui, icon: &str, text: &str, active: bool) -> Response {
    let p = theme::pal(ui.ctx());
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 36.0), Sense::click());
    let radius = CornerRadius::same(RADIUS);
    if active {
        ui.painter().rect_filled(rect, radius, p.hover);
    } else if response.hovered() {
        ui.painter().rect_filled(rect, radius, p.nav_hover);
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            radius,
            Stroke::new(2.0_f32, p.accent),
            StrokeKind::Inside,
        );
    }
    let tint = if active { p.text } else { p.text_soft };
    // With no label (the minimized rail) the icon is centered; otherwise it
    // sits at the left with the label beside it.
    let icon_x = if text.is_empty() {
        rect.center().x - 8.0
    } else {
        rect.left() + 10.0
    };
    let icon_rect =
        Rect::from_min_size(egui::pos2(icon_x, rect.center().y - 8.0), Vec2::splat(16.0));
    paint_icon(ui, icon, icon_rect, tint);
    if !text.is_empty() {
        ui.painter().text(
            egui::pos2(rect.left() + 36.0, rect.center().y),
            Align2::LEFT_CENTER,
            text,
            FontId::proportional(14.0),
            tint,
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

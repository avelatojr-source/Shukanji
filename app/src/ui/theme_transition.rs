//! Center-out circular reveal for theme changes.
//!
//! A click first captures the *old* theme without changing it. When the
//! screenshot arrives, the new theme is applied and the old frame is drawn on
//! top with an expanding circular hole. The reveal mesh is rebuilt each frame
//! (128 segments), so no shader or per-pixel CPU work is required.

use std::time::{Duration, Instant};

use egui::epaint::{Mesh, Vertex};
use egui::{Color32, Pos2, Rect, TextureHandle, TextureId, Vec2, pos2};

use super::theme::ThemeChoice;

const REVEAL_DURATION: Duration = Duration::from_millis(460);
const SEGMENTS: usize = 128;

#[derive(Default)]
pub struct ThemeTransition {
    next_serial: u64,
    capture_serial: Option<u64>,
    pending_choice: Option<ThemeChoice>,
    queued_choice: Option<ThemeChoice>,
    old_frame: Option<TextureHandle>,
    started_at: Option<Instant>,
}

impl ThemeTransition {
    /// Queue a theme change. During an active reveal, a newer click is applied
    /// as soon as the current reveal completes.
    pub fn request(&mut self, ctx: &egui::Context, choice: ThemeChoice) {
        if self.old_frame.is_some() {
            self.queued_choice = Some(choice);
        } else if self.capture_serial.is_some() {
            // The same old-theme capture can serve the latest requested choice.
            self.pending_choice = Some(choice);
        } else {
            self.start_capture(ctx, choice);
        }
    }

    fn start_capture(&mut self, ctx: &egui::Context, choice: ThemeChoice) {
        self.next_serial = self.next_serial.wrapping_add(1);
        let serial = self.next_serial;
        self.capture_serial = Some(serial);
        self.pending_choice = Some(choice);
        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::new(
            serial,
        )));
    }

    /// Consume screenshot replies and advance/retire the active transition.
    /// Called before the application's theme is set for the current frame.
    pub fn update(&mut self, ctx: &egui::Context, current_choice: &mut ThemeChoice) {
        let screenshots = ctx.input(|input| {
            input
                .events
                .iter()
                .filter_map(|event| match event {
                    egui::Event::Screenshot {
                        user_data, image, ..
                    } => user_data
                        .data
                        .as_ref()
                        .and_then(|data| data.downcast_ref::<u64>())
                        .map(|serial| (*serial, image.clone())),
                    _ => None,
                })
                .collect::<Vec<_>>()
        });

        for (serial, image) in screenshots {
            if self.capture_serial != Some(serial) {
                continue;
            }
            self.capture_serial = None;
            if let Some(choice) = self.pending_choice.take() {
                *current_choice = choice;
            }
            self.old_frame = Some(ctx.load_texture(
                "shukanji-theme-old-frame",
                image.as_ref().clone(),
                egui::TextureOptions::LINEAR,
            ));
            self.started_at = Some(Instant::now());
        }

        let finished = self
            .started_at
            .is_some_and(|started| started.elapsed() >= REVEAL_DURATION);
        if finished {
            self.old_frame = None;
            self.started_at = None;
            if let Some(choice) = self.queued_choice.take() {
                self.start_capture(ctx, choice);
            }
        }
    }

    /// Draw the old frame outside the expanding reveal circle, above the new
    /// theme. Called after all normal panels have been rendered.
    pub fn paint(&self, ctx: &egui::Context) {
        let (Some(texture), Some(started_at)) = (&self.old_frame, self.started_at) else {
            return;
        };
        let t =
            (started_at.elapsed().as_secs_f32() / REVEAL_DURATION.as_secs_f32()).clamp(0.0, 1.0);
        let progress = smoothstep(t);
        let rect = ctx.screen_rect();
        let center = rect.center();
        let radius = max_radius(rect, center) * progress;
        let mesh = reveal_mesh(rect, center, radius, texture.id());

        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Tooltip,
            egui::Id::new("shukanji.theme_reveal"),
        ));
        painter.add(egui::Shape::mesh(mesh));
        if radius > 1.0 && t < 0.98 {
            // A short-lived accent edge makes the circle readable without
            // turning it into a permanent ring.
            let accent = super::theme::pal(ctx).accent;
            let edge = super::widgets::with_alpha(accent, (1.0 - t) * 0.45);
            painter.circle_stroke(center, radius, egui::Stroke::new(1.5_f32, edge));
        }
        ctx.request_repaint();
    }
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn max_radius(rect: Rect, center: Pos2) -> f32 {
    [
        rect.left_top(),
        rect.right_top(),
        rect.left_bottom(),
        rect.right_bottom(),
    ]
    .iter()
    .map(|corner| corner.distance(center))
    .fold(0.0, f32::max)
}

fn ray_to_rect(center: Pos2, direction: Vec2, rect: Rect) -> Pos2 {
    let mut distance = f32::INFINITY;
    if direction.x > 0.0 {
        distance = distance.min((rect.right() - center.x) / direction.x);
    } else if direction.x < 0.0 {
        distance = distance.min((rect.left() - center.x) / direction.x);
    }
    if direction.y > 0.0 {
        distance = distance.min((rect.bottom() - center.y) / direction.y);
    } else if direction.y < 0.0 {
        distance = distance.min((rect.top() - center.y) / direction.y);
    }
    center + direction * distance
}

fn point_toward(center: Pos2, boundary: Pos2, radius: f32) -> Pos2 {
    let delta = boundary - center;
    let distance = delta.length();
    if distance <= radius {
        boundary
    } else {
        center + delta / distance * radius
    }
}

fn uv(rect: Rect, point: Pos2) -> Pos2 {
    pos2(
        (point.x - rect.left()) / rect.width(),
        (point.y - rect.top()) / rect.height(),
    )
}

/// Textured geometry covering `rect` except for a circular hole around
/// `center`. Each segment clamps its inner point to the rectangle boundary so
/// the final frames cannot produce inverted triangles.
fn reveal_mesh(rect: Rect, center: Pos2, radius: f32, texture_id: TextureId) -> Mesh {
    let mut mesh = Mesh::with_texture(texture_id);
    mesh.vertices.reserve(SEGMENTS * 4);
    mesh.indices.reserve(SEGMENTS * 6);

    for segment in 0..SEGMENTS {
        let a0 = segment as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        let a1 = (segment + 1) as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        let outer0 = ray_to_rect(center, Vec2::new(a0.cos(), a0.sin()), rect);
        let outer1 = ray_to_rect(center, Vec2::new(a1.cos(), a1.sin()), rect);
        let inner0 = point_toward(center, outer0, radius);
        let inner1 = point_toward(center, outer1, radius);

        let base = mesh.vertices.len() as u32;
        for point in [inner0, outer0, outer1, inner1] {
            mesh.vertices.push(Vertex {
                pos: point,
                uv: uv(rect, point),
                color: Color32::WHITE,
            });
        }
        mesh.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoothstep_eases_both_ends() {
        assert_eq!(smoothstep(0.0), 0.0);
        assert_eq!(smoothstep(1.0), 1.0);
        assert!(smoothstep(0.1) < 0.1);
        assert!(smoothstep(0.9) > 0.9);
    }

    #[test]
    fn rays_hit_the_rectangle_boundary() {
        let rect = Rect::from_min_size(pos2(10.0, 20.0), Vec2::new(200.0, 100.0));
        let center = rect.center();
        let right = ray_to_rect(center, Vec2::new(1.0, 0.0), rect);
        let top = ray_to_rect(center, Vec2::new(0.0, -1.0), rect);
        assert_eq!(right.x, rect.right());
        assert_eq!(top.y, rect.top());
    }

    #[test]
    fn reveal_mesh_is_valid_at_start_middle_and_end() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0));
        let center = rect.center();
        for radius in [0.0, 100.0, max_radius(rect, center)] {
            let mesh = reveal_mesh(rect, center, radius, TextureId::Managed(1));
            assert!(mesh.is_valid());
            assert_eq!(mesh.vertices.len(), SEGMENTS * 4);
            assert_eq!(mesh.indices.len(), SEGMENTS * 6);
        }
    }
}

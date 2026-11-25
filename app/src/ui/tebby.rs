//! Tebby mascot: sprite-sheet animation extracted from the supplied design
//! (`#mas` / `.pk` in the HTML). Two sequences are used there: `idle` (72
//! frames, loops) and `duck` (40 frames, plays once then returns to idle).
//!
//! The sprite sheets are 12 columns wide; frame size 190x215, 24 fps.

use std::time::Duration;

use egui::{Color32, Id, LayerId, Order, Pos2, Rect, TextureHandle, Vec2};

const FRAME_W: f32 = 190.0;
const FRAME_H: f32 = 215.0;
const COLS: usize = 12;
const MS_PER_FRAME: f32 = 1000.0 / 24.0;
const IDLE_FRAMES: usize = 72;
const DUCK_FRAMES: usize = 40;
const IDLE_ROWS: usize = 6;
const DUCK_ROWS: usize = 4;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Seq {
    #[default]
    Idle,
    Duck,
}

/// Owns the Tebby textures and animation state. One per app.
#[derive(Default)]
pub struct Tebby {
    textures: Option<Textures>,
    seq: Seq,
    frame: usize,
    accum: f32,
    /// Milliseconds until the next idle→duck transition.
    wait: f32,
}

struct Textures {
    idle: TextureHandle,
    duck: TextureHandle,
    avatar: TextureHandle,
}

impl Tebby {
    /// Decode the sprite sheets once. Safe to call every frame; failures are
    /// tolerated (the mascot is simply not drawn).
    pub fn ensure_loaded(&mut self, ctx: &egui::Context) {
        if self.textures.is_some() {
            return;
        }
        let idle = decode(
            ctx,
            "tebby.idle",
            include_bytes!("../../assets/tebby/idle.webp"),
        );
        let duck = decode(
            ctx,
            "tebby.duck",
            include_bytes!("../../assets/tebby/duck.webp"),
        );
        let avatar = decode(
            ctx,
            "tebby.avatar",
            include_bytes!("../../assets/tebby/avatar.webp"),
        );
        if let (Some(idle), Some(duck), Some(avatar)) = (idle, duck, avatar) {
            self.textures = Some(Textures { idle, duck, avatar });
            self.wait = 12_000.0;
        }
    }

    pub fn avatar_texture(&self) -> Option<&TextureHandle> {
        self.textures.as_ref().map(|t| &t.avatar)
    }

    pub fn is_ready(&self) -> bool {
        self.textures.is_some()
    }

    /// Advance the animation. `dt` is seconds since the last frame.
    ///
    /// `visible` must be false whenever the mascot is not on screen (any theme
    /// other than Tebby, or a minimized sidebar). When it is false this method
    /// returns without requesting a repaint: an unconditional
    /// `request_repaint_after` here is what kept the whole app rendering at
    /// ~60 fps — burning CPU — while sitting idle in the dark/light themes.
    pub fn animate(&mut self, ctx: &egui::Context, dt: f32, visible: bool) {
        if !visible {
            return;
        }
        if self.textures.is_none() {
            self.ensure_loaded(ctx);
            ctx.request_repaint();
            return;
        }
        let mut acc = self.accum + dt * 1000.0;
        while acc >= MS_PER_FRAME {
            acc -= MS_PER_FRAME;
            match self.seq {
                Seq::Idle => {
                    self.frame += 1;
                    if self.frame >= IDLE_FRAMES {
                        self.frame = 0;
                    }
                    self.wait -= MS_PER_FRAME;
                    if self.wait <= 0.0 {
                        self.seq = Seq::Duck;
                        self.frame = 0;
                    }
                }
                Seq::Duck => {
                    self.frame += 1;
                    if self.frame >= DUCK_FRAMES {
                        self.seq = Seq::Idle;
                        self.frame = 0;
                        self.wait = 10_000.0 + (self.frame as f32 * 137.0) % 6_000.0;
                    }
                }
            }
        }
        self.accum = acc;
        // Wake at the sprite's own frame rate (24 fps), not 60, and back right
        // off when the window is not focused — the mascot is decorative, and an
        // unconditional repaint loop is what kept the app burning CPU while idle.
        let next_ms = if ctx.input(|i| i.focused) {
            MS_PER_FRAME
        } else {
            400.0
        };
        ctx.request_repaint_after(Duration::from_millis(next_ms as u64));
    }

    /// Draw the mascot at the bottom-left of the window.
    pub fn draw(&self, ctx: &egui::Context, minimized: bool) {
        let Some(textures) = &self.textures else {
            return;
        };
        if minimized {
            return;
        }
        let screen = ctx.screen_rect();
        let pos = Pos2::new(screen.left(), screen.bottom() - 50.0 - FRAME_H);
        let rect = Rect::from_min_size(pos, Vec2::new(FRAME_W, FRAME_H));
        let (sheet, rows, frame) = match self.seq {
            Seq::Idle => (&textures.idle, IDLE_ROWS, self.frame),
            Seq::Duck => (&textures.duck, DUCK_ROWS, self.frame),
        };
        let col = frame % COLS;
        let row = frame / COLS;
        let uv = Rect::from_min_size(
            Pos2::new(col as f32 / COLS as f32, row as f32 / rows as f32),
            Vec2::new(1.0 / COLS as f32, 1.0 / rows as f32),
        );
        ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("tebby.mascot")))
            .image(sheet.id(), rect, uv, Color32::WHITE);
    }
}

fn decode(ctx: &egui::Context, name: &str, bytes: &[u8]) -> Option<TextureHandle> {
    let image = image::load_from_memory(bytes).ok()?;
    let rgba = image.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    Some(ctx.load_texture(name.to_owned(), color, egui::TextureOptions::LINEAR))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drive a headless egui context and collect the repaint delay egui reports
    /// per pass. `Duration::MAX` means "nothing asked for a repaint".
    ///
    /// The first two passes are discarded: egui deliberately runs a couple of
    /// frames at start-up (`outstanding: 1`).
    fn repaint_delays(dt: f32, visible: bool, frames: usize) -> Vec<Duration> {
        let ctx = egui::Context::default();
        // The real GL backend reports a large limit; the headless default is 2048.
        let input = egui::RawInput {
            max_texture_side: Some(8192),
            ..Default::default()
        };
        let mut tebby = Tebby::default();
        let mut delays = Vec::new();
        for _ in 0..frames {
            let output = ctx.run(input.clone(), |ctx| {
                tebby.animate(ctx, dt, visible);
            });
            delays.push(output.viewport_output[&egui::ViewportId::ROOT].repaint_delay);
        }
        delays.split_off(2)
    }

    #[test]
    fn an_offscreen_mascot_never_schedules_a_repaint() {
        let delays = repaint_delays(0.016, false, 6);
        assert!(
            delays.iter().all(|delay| *delay == Duration::MAX),
            "an offscreen mascot must not keep the app rendering, got {delays:?}"
        );
    }

    #[test]
    fn a_visible_mascot_schedules_animation_frames() {
        let delays = repaint_delays(0.016, true, 6);
        assert!(
            delays
                .iter()
                .all(|delay| *delay <= Duration::from_millis(50)),
            "the animating mascot must schedule its next frame, got {delays:?}"
        );
    }
}

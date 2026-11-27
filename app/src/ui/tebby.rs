//! Tebby mascot: sprite-sheet animation extracted from the supplied design
//! (`#mas` / `.pk` in the HTML). Two sequences are used there: `idle` (72
//! frames, loops) and `duck` (40 frames, plays once then returns to idle).
//!
//! The source sheets are much larger than the on-screen mascot. They are
//! decoded and resized on a worker thread for the current display scale, then
//! uploaded once on the UI thread. Optimize Mode keeps only a static first
//! frame and the avatar, so it neither animates nor keeps the sprite sheets in
//! memory. Leaving the Tebby theme drops every Tebby texture.

use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;

use egui::{Color32, ColorImage, Id, LayerId, Order, Pos2, Rect, TextureHandle, Vec2};

const FRAME_W: f32 = 190.0;
const FRAME_H: f32 = 215.0;
const AVATAR_SIZE: f32 = 48.0;
const COLS: usize = 12;
const MS_PER_FRAME: f32 = 1000.0 / 24.0;
const IDLE_FRAMES: usize = 72;
const DUCK_FRAMES: usize = 40;
const IDLE_ROWS: usize = 6;
const DUCK_ROWS: usize = 4;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum Seq {
    #[default]
    Idle,
    Duck,
}

/// Owns the Tebby textures and animation state. One per app.
#[derive(Default)]
pub struct Tebby {
    textures: Option<Textures>,
    load_rx: Option<Receiver<Option<DecodedAssets>>>,
    load_attempted: bool,
    optimized: bool,
    seq: Seq,
    frame: usize,
    accum: f32,
    /// Milliseconds until the next idle→duck transition.
    wait: f32,
}

struct Textures {
    /// Full idle sheet normally; one static frame in Optimize Mode.
    idle: TextureHandle,
    duck: Option<TextureHandle>,
    avatar: TextureHandle,
}

struct DecodedAssets {
    idle: ColorImage,
    duck: Option<ColorImage>,
    avatar: ColorImage,
}

impl Tebby {
    /// Start decoding assets off the UI thread. Called as soon as a Tebby
    /// switch is requested, so the 460 ms reveal usually covers the load.
    pub fn request_load(&mut self, ctx: &egui::Context) {
        if self.textures.is_some() || self.load_rx.is_some() || self.load_attempted {
            return;
        }
        self.load_attempted = true;
        let (tx, rx) = std::sync::mpsc::channel();
        self.load_rx = Some(rx);
        let wake = ctx.clone();
        let optimized = self.optimized;
        let scale = ctx.pixels_per_point();
        let _ = std::thread::Builder::new()
            .name("tebby-load".to_string())
            .spawn(move || {
                let assets = decode_assets(scale, optimized);
                let _ = tx.send(assets);
                wake.request_repaint();
            });
    }

    /// Upload completed assets without ever decoding WebP on the UI thread.
    fn poll(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.load_rx else {
            return;
        };
        match rx.try_recv() {
            Ok(Some(assets)) => {
                let idle = ctx.load_texture(
                    "tebby.idle",
                    assets.idle,
                    egui::TextureOptions::LINEAR,
                );
                let duck = assets.duck.map(|image| {
                    ctx.load_texture("tebby.duck", image, egui::TextureOptions::LINEAR)
                });
                let avatar = ctx.load_texture(
                    "tebby.avatar",
                    assets.avatar,
                    egui::TextureOptions::LINEAR,
                );
                self.textures = Some(Textures {
                    idle,
                    duck,
                    avatar,
                });
                self.wait = 12_000.0;
                self.load_rx = None;
            }
            Ok(None) | Err(TryRecvError::Disconnected) => {
                // A missing/corrupt asset leaves the mascot hidden instead of
                // retrying a costly decode every frame.
                self.load_rx = None;
            }
            Err(TryRecvError::Empty) => {}
        }
    }

    /// Release all theme-owned assets. The receiver is dropped too, so a
    /// worker that finishes after a theme switch cannot resurrect them.
    pub fn unload(&mut self) {
        self.textures = None;
        self.load_rx = None;
        self.load_attempted = false;
        self.seq = Seq::Idle;
        self.frame = 0;
        self.accum = 0.0;
        self.wait = 0.0;
    }

    /// Switch between animated sheets and a single static mascot frame.
    pub fn set_optimized(&mut self, optimized: bool) {
        if self.optimized != optimized {
            self.optimized = optimized;
            self.unload();
        }
    }

    pub fn avatar_texture(&self) -> Option<&TextureHandle> {
        self.textures.as_ref().map(|t| &t.avatar)
    }

    /// Advance the animation. `dt` is seconds since the last frame.
    ///
    /// A hidden mascot does not request repaints. A visible one runs at the
    /// same 24 fps in the background as in the foreground; live monitoring is
    /// owned by the async coordinator and is independent of these repaints.
    pub fn animate(&mut self, ctx: &egui::Context, dt: f32, visible: bool) {
        self.poll(ctx);
        if !visible {
            return;
        }
        if self.textures.is_none() {
            self.request_load(ctx);
            // The worker also requests a repaint on completion; this is only a
            // fallback in case spawning it failed.
            ctx.request_repaint_after(Duration::from_millis(50));
            return;
        }
        if self.optimized {
            // Static Optimize Mode: no perpetual animation/repaint loop.
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
        // Keep the native 24 fps cadence even when unfocused. The old 400 ms
        // background delay made the mascot visibly stutter when tabbing out.
        ctx.request_repaint_after(Duration::from_millis(MS_PER_FRAME as u64));
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
        let (sheet, uv) = if self.optimized {
            (
                &textures.idle,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            )
        } else {
            let (sheet, rows, frame) = match self.seq {
                Seq::Idle => (&textures.idle, IDLE_ROWS, self.frame),
                Seq::Duck => (
                    textures.duck.as_ref().unwrap_or(&textures.idle),
                    DUCK_ROWS,
                    self.frame,
                ),
            };
            let col = frame % COLS;
            let row = frame / COLS;
            let uv = Rect::from_min_size(
                Pos2::new(col as f32 / COLS as f32, row as f32 / rows as f32),
                Vec2::new(1.0 / COLS as f32, 1.0 / rows as f32),
            );
            (sheet, uv)
        };
        ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("tebby.mascot")))
            .image(sheet.id(), rect, uv, Color32::WHITE);
    }
}

/// Frame dimensions that match the actual display instead of decoding the
/// original 304×344 cells. Capped at 2× to avoid extreme DPI allocations.
fn frame_pixels(scale: f32) -> (u32, u32) {
    let scale = scale.clamp(1.0, 2.0);
    (
        (FRAME_W * scale).round() as u32,
        (FRAME_H * scale).round() as u32,
    )
}

fn decode_assets(scale: f32, optimized: bool) -> Option<DecodedAssets> {
    let idle = if optimized {
        decode_static_frame(include_bytes!("../../assets/tebby/idle.webp"), scale)
    } else {
        decode_sheet(
            include_bytes!("../../assets/tebby/idle.webp"),
            IDLE_ROWS,
            scale,
        )
    }?;
    let duck = if optimized {
        None
    } else {
        Some(decode_sheet(
            include_bytes!("../../assets/tebby/duck.webp"),
            DUCK_ROWS,
            scale,
        )?)
    };
    let avatar = decode_avatar(include_bytes!("../../assets/tebby/avatar.webp"), scale)?;
    Some(DecodedAssets { idle, duck, avatar })
}

fn decode_sheet(bytes: &[u8], rows: usize, scale: f32) -> Option<ColorImage> {
    let source = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (frame_w, frame_h) = frame_pixels(scale);
    let width = frame_w * COLS as u32;
    let height = frame_h * rows as u32;
    Some(resize_rgba(source, width, height))
}

fn decode_static_frame(bytes: &[u8], scale: f32) -> Option<ColorImage> {
    let source = image::load_from_memory(bytes).ok()?.to_rgba8();
    let cell_w = source.width() / COLS as u32;
    let cell_h = source.height() / IDLE_ROWS as u32;
    let first = image::imageops::crop_imm(&source, 0, 0, cell_w, cell_h).to_image();
    drop(source);
    let (width, height) = frame_pixels(scale);
    Some(resize_rgba(first, width, height))
}

fn decode_avatar(bytes: &[u8], scale: f32) -> Option<ColorImage> {
    let source = image::load_from_memory(bytes).ok()?.to_rgba8();
    let size = (AVATAR_SIZE * scale.clamp(1.0, 2.0)).round() as u32;
    Some(resize_rgba(source, size, size))
}

fn resize_rgba(
    source: image::ImageBuffer<image::Rgba<u8>, Vec<u8>>,
    width: u32,
    height: u32,
) -> ColorImage {
    // Produce the resized buffer, then release the much larger source *before*
    // allocating egui's Color32 pixel vector. This avoids holding the original
    // RGBA sheet, the resized RGBA sheet and the ColorImage at the same time.
    let resized = image::imageops::resize(
        &source,
        width,
        height,
        image::imageops::FilterType::Triangle,
    );
    drop(source);
    ColorImage::from_rgba_unmultiplied([width as usize, height as usize], resized.as_raw())
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

    #[test]
    fn display_scaling_bounds_sheet_memory() {
        assert_eq!(frame_pixels(1.0), (190, 215));
        let (w, h) = frame_pixels(3.0);
        assert_eq!((w, h), (380, 430));
        // The source cell is 304×344; a 1× sheet is substantially smaller.
        assert!(190 * 215 < 304 * 344);
    }

    #[test]
    fn unload_resets_assets_and_animation() {
        let mut tebby = Tebby::default();
        tebby.load_attempted = true;
        tebby.seq = Seq::Duck;
        tebby.frame = 20;
        tebby.accum = 12.0;
        tebby.unload();
        assert!(tebby.textures.is_none());
        assert!(tebby.load_rx.is_none());
        assert!(!tebby.load_attempted);
        assert_eq!(tebby.seq, Seq::Idle);
        assert_eq!(tebby.frame, 0);
        assert_eq!(tebby.accum, 0.0);
    }
}

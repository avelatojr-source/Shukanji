# ShuKanji update — UI transitions + chat font coverage

This folder (`Desktop\Shukanji-Update`) is an **independent copy** of the
project. The original `Desktop\Shukanji` checkout is untouched.

## What changed

### 0. Smooth window animations, circular theme reveal + performance pass (latest)

- **Real native window animations (Windows):** launch fades in over 240 ms and
  the title-bar X / Alt+F4 fades out over 180 ms before shutdown. The title-bar
  minimize button now fades through a 200 ms `SetTimer` opacity ramp with
  smoothstep easing, and only then enters the real minimized state; restoring
  from the taskbar fades back in over 170 ms. This replaces the earlier
  `AnimateWindow` minimize/restore path, whose hide + show sequence caused a
  visible snap. Restoring during the fade cancels it instead of re-minimizing,
  and un-maximizing does not trigger a restore fade. Close still uses
  `DestroyWindow` after the fade so hidden windows shut down reliably; normal
  non-animated fallbacks remain.
- **Animated theme change (center-out circle):** changing the theme captures
  the previous frame, applies the new theme, and reveals it through a circular
  hole expanding from the center of the window over 460 ms with smoothstep
  easing. The old frame is drawn as a textured 128-segment mesh (no shaders or
  per-pixel CPU work) with a thin accent ring at the edge. Works from the
  Appearance theme cards and the header toggle; rapid clicks are queued and
  replayed after the current reveal.
- **Ctrl+T** now switches dark/light theme (also listed in
  Settings → Shortcuts).
- **Sidebar minimizer is now fully animated** (180 ms): the width slides
  between 220 px and the 68 px rail, the wordmark/display name fade, nav icons
  slide from their left slot to the rail center, labels fade out, and the group
  captions crossfade into the centered rules — no more instant snapping.
- **Go Live bar collapse is animated** (160 ms): height eases between 76 px and
  the 34 px strip (padding too); contents swap at the midpoint.
- **Faster start-up:** the window now appears immediately with the bundled
  fonts; the ~70 MB of custom + Windows fallback fonts are read on a background
  thread (`font-load`) and installed when ready, with the settings report
  delivered over a channel. The "Reload fonts" button still rescans on demand.
- **Repaint scheduling for toasts:** the UI wakes exactly when a toast expires
  instead of waiting for incidental repaints — no continuous repaint loop.

### 1. Smooth transitions on buttons and interactive widgets

Every custom-painted control now eases between states instead of flipping
instantly (140 ms hover/active fades, 80 ms press fade):

- **Primary / Secondary / Danger buttons** — fill, border and text color ease
  through rest → hover → pressed.
- **Go Live / End Live stream button** — same treatment for both modes.
- **Sidebar navigation** — the active-page highlight and hover background fade
  in/out; icon and label tint ease with them.
- **Chat tabs (All / Gifts / Follows)** — label color and the accent underline
  animate when switching tabs.
- **Theme cards, sidebar collapse arrow, chat resize grips** — hover fades and
  an animated grip width/color.

New helpers in `app/src/ui/widgets.rs`: `lerp_color`, `hover_t`, `press_t`,
`TRANSITION_SECS`, built on `egui::Context::animate_bool_with_time` (egui drives
the repaints; no timers or background threads).

### 2. Chat comments now render any script + custom font support

Viewer comments previously showed empty boxes for Japanese, Chinese, Korean,
Arabic, Thai, etc., because only Inter/JetBrains (Latin) were installed.

New `app/src/ui/fonts.rs` builds an ordered fallback chain per family:

1. Bundled primary face (Inter / JetBrains Mono)
2. **Your custom fonts** from `%APPDATA%\TikTokStreamTool\fonts`
   (`.ttf`, `.otf`, `.ttc`, `.otc`; sorted by filename, so `01-...` wins)
3. **Windows system fallbacks** — Segoe UI + Symbol, Yu Gothic, Meiryo,
   Microsoft YaHei/JhengHei, Malgun Gothic, Nirmala UI, Leelawadee UI, SimSun,
   Arial, Tahoma, Segoe UI Emoji (only the ones installed are loaded)
4. Bundled Noto Emoji (unchanged, last)

This fixes chat, usernames, gift names, activity lines — everything drawn with
the proportional or monospace families.

**Settings → Chat fonts** shows the custom-font folder, an **Open fonts
folder** button, a **Reload fonts** button (no restart needed), the scan report
and a live multi-script preview.

### 3. Research: replying to chat with your own TikTok account

See **`docs/CHAT_REPLY_RESEARCH.md`** — sending is possible via Euler Stream's
`POST /webcast/rooms/{room_id}/chat` route with your session cookies and a
premium API key, but carries cost/credential/ToS risks. The official LIVE
Center (the app's **Open Live Monitor** button) remains the safe reply path.

## Files touched

| File | Change |
| --- | --- |
| `app/src/ui/fonts.rs` | **New** — font discovery, fallback chain, scan report, tests; bundled fast path + background extended loader |
| `app/src/ui/theme.rs` | Font install moved to `fonts.rs` (bundled fast path at start-up) |
| `app/src/ui/mod.rs` | Register `fonts` module |
| `app/src/app.rs` | Store `font_report` (+ background channel), toast repaint scheduling; theme transition state + `change_theme`, Ctrl+T shortcut |
| `app/src/ui/widgets.rs` | Animated button/stream-button/nav/toggle/grip states + color-lerp/alpha helpers + tests |
| `app/src/ui/panels.rs` | Animated chat tabs + theme cards; Chat fonts settings section; animated sidebar + Go Live bar minimizers; theme cards/toggle route through the reveal |
| `app/src/ui/theme_transition.rs` | **New** — screenshot capture, expanding circular reveal mesh, queued theme changes + tests |
| `app/src/platform/window_fx.rs` | **New** — Win32 native open/close fades, timer-driven minimize/restore ramps + tests |
| `app/src/platform/mod.rs` | Register the Windows-only native effects module |
| `app/src/main.rs` | Start hidden on Windows for the launch fade |
| `app/Cargo.toml`, `app/Cargo.lock` | Add `raw-window-handle` and Windows API dependencies |
| `docs/CHAT_REPLY_RESEARCH.md` | **New** — chat-reply findings and API contract |
| `docs/UPDATE.md` | **New** — this file |

## Verification

- `cargo check --all-targets` — clean, no warnings
- `cargo test` — 121 passed, 0 failed, 1 ignored (includes new reveal-mesh,
  smoothstep and window-command tests)
- `cargo build --release` — produces the updated `shukanji.exe` in
  `app\target\release\`
- Live Windows probe: minimize alpha steps 247 → 212 → 167 → 62 → 22 → 0
  (25 ms samples), restore climbs back to opaque, close exits the process
- Live Windows probe: restoring 60 ms into the minimize fade leaves the window
  restored and visible
- Live Windows probe: Ctrl+T / theme card capture shows a small center circle
  growing to the full window, ending on the new theme

## Build / run

```powershell
cd Desktop\Shukanji-Update\app
cargo run --release
```

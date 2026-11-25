# ShuKanji update — UI transitions + chat font coverage

This folder (`Desktop\Shukanji-Update`) is an **independent copy** of the
project. The original `Desktop\Shukanji` checkout is untouched.

## What changed

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
| `app/src/ui/fonts.rs` | **New** — font discovery, fallback chain, scan report, tests |
| `app/src/ui/theme.rs` | Font install moved to `fonts.rs`; `apply` returns the report |
| `app/src/ui/mod.rs` | Register `fonts` module |
| `app/src/app.rs` | Store `font_report` for the settings page |
| `app/src/ui/widgets.rs` | Animated button/stream-button/nav/toggle/grip states + color-lerp helpers + tests |
| `app/src/ui/panels.rs` | Animated chat tabs + theme cards; new Chat fonts settings section |
| `docs/CHAT_REPLY_RESEARCH.md` | **New** — chat-reply findings and API contract |
| `docs/UPDATE.md` | **New** — this file |

## Verification

- `cargo check --all-targets` — clean
- `cargo test` — all tests pass (including new font-chain and color-lerp tests)
- `cargo build --release` — produces the updated `shukanji.exe` in
  `app\target\release\`

## Build / run

```powershell
cd Desktop\Shukanji-Update\app
cargo run --release
```

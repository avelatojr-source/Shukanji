# Architecture

## Overview

The application is intended to be stored in two sibling Rust crates:

1. `app/` — the Windows desktop application and executable (`shukanji.exe`).
2. `live-rs/` — the local `piratetok-live-rs` library used to receive and decode TikTok LIVE events.

`app/Cargo.toml` references the library through `path = "../live-rs"`. Keep both directories at the repository root beside one another. The repository is deliberately not a Cargo workspace; build and test each crate from its own directory.

## Desktop application layers

| Area | Responsibility |
| --- | --- |
| `src/ui/`, `src/app.rs` | egui application shell, pages, panels, widgets, theme, image cache, mascot rendering |
| `src/application/` | command coordinator, background task orchestration, event delivery |
| `src/domain/` | app commands, state models, monitor models, events, and domain errors |
| `src/integrations/streamlabs/` | Streamlabs HTTP calls, browser OAuth/PKCE, manual token support, opt-in local token scan |
| `src/integrations/obs/` | OBS WebSocket 5.x connection, authentication, protocol types, settings apply/read-back |
| `src/integrations/tiktok_live/` | conversion and aggregation of `live-rs` events for the app activity feed |
| `src/platform/credentials/` | platform-backed secret storage |
| `src/persistence/` | non-secret TOML configuration and user data paths |
| `src/diagnostics/` | structured logging and secret redaction |

## Event flow

```text
User interaction
      |
      v
 egui UI (app/src/ui, app/src/app.rs)
      |
      v
 Domain Command -----> Application Coordinator
                              |
                +-------------+------------------+
                |                                |
                v                                v
        Streamlabs integration             OBS integration
        OAuth + HTTP client                OBS WebSocket 5.x
                |                                |
                +---------------+----------------+
                                |
                                v
                         App event channel
                                |
                                v
                         UI state / panels
```

The TikTok monitor runs as a separate background task. `live-rs` resolves the room, opens the WebSocket connection, decodes the protobuf event stream, and handles reconnects. The app's TikTok integration translates those events into app-level monitor events, applies gift-streak and like aggregation, and sends them to the chronological feed.

## Persistence and secrets

- Non-secret preferences are serialized as TOML below the user's `TikTokStreamTool` data directory.
- Streamlabs tokens and OBS passwords are managed through the platform credential-store abstraction, which uses Windows Credential Manager on Windows.
- Images are downloaded and cached separately from TOML settings.
- Diagnostics redact sensitive values. Never add credentials, real stream keys, account tokens, or local `config.toml` files to the repository.

## External dependencies and trust boundaries

- Streamlabs endpoints used by the app are undocumented internal endpoints. Treat them as a compatibility risk, not a guaranteed public API.
- OBS communication uses OBS WebSocket 5.x. Mock-server tests do not replace a manual test against the user's installed OBS instance.
- TikTok LIVE monitoring depends on TikTok's room/API and WebSocket behavior. A live stream can disconnect or change protocol behavior independently of the app.

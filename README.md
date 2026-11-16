# ShuKanji

**A native Windows desktop companion for TikTok LIVE streaming.** ShuKanji retrieves TikTok stream credentials through Streamlabs, configures and verifies OBS Studio over OBS WebSocket, can start or stop OBS output with explicit user action, and displays TikTok LIVE chat and audience events in one chronological feed.

Built with Rust and egui, the project consists of a Windows desktop application and a separate Rust TikTok LIVE client.

> [!IMPORTANT]
> This GitHub repository currently contains the project documentation and import helper. The full Rust source tree and embedded binary assets still need to be imported from the organized source archive. Do not treat the repository as buildable until `app/` and `live-rs/` have been pushed and a Windows build has been checked.

## Import the complete source

The source code and embedded binary assets are not yet in this GitHub tree. Download the `ShuKanji-Organized-Source.zip` archive supplied with the project handoff, clone this repository, and run the checked-in [PowerShell importer](scripts/import-organized-source.ps1). It validates the two sibling Cargo crates and required assets, stages only `app/` and `live-rs/`, and asks before committing and pushing. See [the full import instructions](docs/SOURCE_IMPORT.md).

```powershell
git clone https://github.com/avelatojr-source/Shukanji.git
cd Shukanji
powershell -ExecutionPolicy Bypass -File .\scripts\import-organized-source.ps1 -ArchivePath "$HOME\Downloads\ShuKanji-Organized-Source.zip"
```

## Features

### Stream setup and OBS control
- Streamlabs browser-based OAuth PKCE sign-in, pasted-token support, and an opt-in, read-only local token scan.
- Retrieves an RTMP server URL and stream key from Streamlabs, with manual RTMP entry as an alternative.
- Connects to OBS Studio through OBS WebSocket 5.x, reads stream status, applies custom RTMP settings, and reads the settings back to verify them.
- Starts and stops OBS output only after an explicit user action.
- Reconnects with backoff and re-checks OBS state after reconnecting.

### TikTok LIVE activity monitor
- Connects to a TikTok LIVE by username.
- Displays chat, joins, follows, shares, likes, gifts, and viewer information in one chronological feed.
- De-duplicates combo-gift updates and accumulates gift diamond totals from event deltas.
- Coalesces high-volume likes and caches avatars, gift artwork, emotes, and emoji images locally.

The monitor is independent of the Streamlabs credential workflow. ShuKanji reads a LIVE; it does **not** send chat messages.

## Status accuracy

A retrieved stream key does not prove that TikTok has accepted a broadcast. ShuKanji distinguishes between:
1. Stream credentials retrieved.
2. OBS settings applied and verified.
3. OBS output running.
4. TikTok LIVE broadcast status.

**OBS output running does not, by itself, verify that the TikTok LIVE is active.**

## Requirements

- Windows 10 or Windows 11, 64-bit.
- OBS Studio 28 or later, with the OBS WebSocket server enabled (default port `4455`).
- A Streamlabs account linked to TikTok for managed credential retrieval. Manual RTMP entry is also available.
- Rust 1.85 or later for the desktop app (edition 2024).

## Repository layout

The intended repository has two sibling Rust crates because the app uses `../live-rs` as a local Cargo path dependency. This is intentionally **not** a Cargo workspace.

```text
Shukanji/
├── README.md
├── .gitignore
├── LICENSE                 # MIT — desktop application
├── app/                    # Rust + egui Windows desktop app
│   ├── Cargo.toml
│   ├── Cargo.lock
│   ├── build.rs
│   ├── assets/
│   ├── src/
│   └── tests/
├── live-rs/                # Local piratetok-live-rs crate (0BSD)
│   ├── Cargo.toml
│   ├── src/
│   ├── examples/
│   └── tests/
├── scripts/
│   └── import-organized-source.ps1
└── docs/
    ├── ARCHITECTURE.md
    ├── MANUAL_OBS_CHECKLIST.md
    ├── RELEASING.md
    └── SOURCE_IMPORT.md
```

## Import the source

Clone the repository, download the organized source ZIP from the project handoff, and run the PowerShell helper described in [docs/SOURCE_IMPORT.md](docs/SOURCE_IMPORT.md). It checks the archive, copies the two crates into the correct sibling layout, stages the source, and asks before committing and pushing.

## Build and test

After the source directories have been pushed, run the desktop app commands from `app/`:

```powershell
cargo fmt --check
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo build --release
```

The `live-rs` crate is checked separately:

```powershell
cd ..\live-rs
cargo test
```

See [Architecture](docs/ARCHITECTURE.md), [Release checklist](docs/RELEASING.md), and the [manual OBS verification checklist](docs/MANUAL_OBS_CHECKLIST.md).

## Configuration and security

The application stores non-secret settings, logs, and its image cache below:

```text
%APPDATA%\TikTokStreamTool\
```

That directory name is retained for compatibility with existing installations. Streamlabs tokens and OBS passwords are stored through the Windows Credential Manager integration; they are not intended to be written to `config.toml`. The local Streamlabs token scan is opt-in and read-only. Logs redact secret values.

The Streamlabs endpoints used by the app are undocumented internal endpoints, not a stable public API. Treat their continued availability as an external compatibility risk.

## Versioning

The supplied desktop application manifest declares version `0.1.0`; the `live-rs` crate independently declares version `0.5.0`. A Git tag named `v1.0` does not change Cargo package metadata. Before a v1.0 release, update the app manifest, lockfile, app README, and release notes together—or tag the actual app version.

## Releases

Keep `target/`, `dist/`, `Source code/`, and executable build output out of Git. Build the Windows executable from `app/` and attach `shukanji.exe` (or a release ZIP containing it and the user guide) to a GitHub Release.

## Licenses

- Desktop application: MIT, as declared in `app/Cargo.toml`.
- `live-rs/`: 0BSD, as declared in its own crate manifest. Preserve its upstream license file and attribution.

The desktop application's root `LICENSE` does not replace the separate `live-rs` license.

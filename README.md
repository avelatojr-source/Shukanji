# ShuKanji

### Your TikTok LIVE chat, finally in one place.

Going LIVE and tired of jumping between windows just to keep up with your chat? **ShuKanji is a Windows desktop companion that helps you follow what is happening on your TikTok LIVE while you manage your stream.**

See messages and audience activity in one live feed, keep an eye on gifts and follows, and handle key OBS setup from the same app. Spend less time hunting for updates and more time actually streaming.

> **Keep your chat in sight. Catch the moments. Stay with your stream.**

## What you can do with ShuKanji

- **Follow the conversation.** Read TikTok LIVE chat in a chronological feed without constantly checking another screen.
- **See the room come alive.** Keep track of gifts, likes, follows, shares, joins, and viewer information as events arrive.
- **Get your stream ready.** Sign in to Streamlabs to retrieve your RTMP server and stream key, or enter RTMP details manually.
- **Work with OBS.** Connect through OBS WebSocket, apply your stream settings, and read them back to verify the configuration. Start or stop OBS output with an explicit action.
- **Keep busy streams readable.** ShuKanji handles combo-gift updates, totals gift diamond changes, and groups high-volume like activity to make the feed easier to follow.

## How it fits into your stream

1. **Connect your stream details.** Use Streamlabs to retrieve your RTMP server and stream key, or enter them yourself.
2. **Connect OBS Studio.** ShuKanji communicates with OBS over OBS WebSocket and can apply and verify your stream settings.
3. **Open your TikTok LIVE monitor.** Connect using the LIVE creator's username and watch chat and audience events appear in the feed.
4. **Stay in control.** Start or stop OBS output when you're ready.

The TikTok LIVE monitor is separate from the Streamlabs credential workflow. ShuKanji reads the LIVE; it does **not** send chat messages.

## What ShuKanji is — and isn't

ShuKanji brings useful parts of a streaming workflow together, but it doesn't claim to do everything for you.

- Retrieving a stream key does not mean TikTok has accepted a broadcast.
- Seeing OBS output running does not prove that the TikTok LIVE is active.
- ShuKanji distinguishes between credentials being retrieved, OBS settings being applied and verified, OBS output running, and TikTok LIVE broadcast status.

## What you'll need

- Windows 10 or Windows 11 (64-bit)
- OBS Studio 28 or later, with OBS WebSocket enabled (the usual port is `4455`)
- A Streamlabs account linked to TikTok for managed credential retrieval, or your own RTMP server and stream key
- Rust 1.85 or later if you're building the desktop app from source (edition 2024)

## Project status

The repository is being prepared for the complete source release. **At the moment, this GitHub tree contains project documentation and the source-import helper; the `app/` and `live-rs/` source directories have not yet been added.** That means you can't build ShuKanji from this repository just yet.

When the source import is complete, the layout will look like this:

```text
Shukanji/
├── app/       # Windows desktop app (Rust + egui)
├── live-rs/   # TikTok LIVE client library
├── docs/      # Project and release documentation
├── scripts/   # Source import helper
├── README.md
└── LICENSE
```

The two Rust crates are kept side by side because the desktop app uses `../live-rs` as a local Cargo dependency. They are intentionally not a Cargo workspace.

## Building from source

Once `app/` and `live-rs/` are present, run the desktop app checks from `app/`:

```powershell
cargo fmt --check
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo build --release
```

Then check the LIVE client separately from `live-rs/`:

```powershell
cargo test
```

More detail is in [the architecture notes](docs/ARCHITECTURE.md), [the manual OBS checklist](docs/MANUAL_OBS_CHECKLIST.md), and [the release checklist](docs/RELEASING.md).

## Downloading the app

A ready-to-use Windows download has **not yet been published as a GitHub Release**. When a release is available, this section will link to the downloadable app package. The executable belongs in GitHub Releases, not in the source-code history.

## Your settings and privacy

ShuKanji keeps non-secret settings, logs, and its image cache under:

```text
%APPDATA%\\TikTokStreamTool\\
```

That folder name is retained for compatibility with existing installations. Streamlabs tokens and OBS passwords are stored through the Windows Credential Manager integration rather than being written to `config.toml`. The optional local Streamlabs token scan is read-only, and logs redact secret values.

One thing to be aware of: the Streamlabs endpoints used for credential retrieval are undocumented internal endpoints, not a stable public API. Their availability may change outside ShuKanji's control.

## Licenses

- Desktop application: MIT
- `live-rs/` client library: 0BSD

The library keeps its own license; the root MIT license does not replace it.

---

ShuKanji is an independent community-built project. It is not affiliated with, endorsed by, or sponsored by TikTok, Streamlabs, or OBS.

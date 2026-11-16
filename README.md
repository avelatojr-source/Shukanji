# ShuKanji

### Your TikTok LIVE chat, OBS setup, and stream activity — together in one place.

Going LIVE is already a lot to manage. You shouldn't have to keep jumping between windows just to read chat, check gifts, or get your stream running.

**ShuKanji is a Windows desktop companion for TikTok LIVE creators.** It brings your LIVE activity into one organized dashboard and helps connect your stream setup to OBS, so you can spend less time managing windows and more time with your viewers.

Keep an eye on chat, gifts, likes, follows, shares, joins, and viewer activity. When you're ready to stream, use **Go Live** and ShuKanji takes care of the OBS setup flow with you.

> **Your LIVE chat in view. Your OBS setup in sync. Your stream, in your hands.**

## What you can do with ShuKanji

### 💬 Keep up with your chat
Connect to a TikTok LIVE by username and watch incoming messages in one chronological feed. You can follow the conversation without constantly switching back to TikTok in your browser.

### 🎁 Catch the moments that matter
See gifts, likes, follows, shares, joins, and viewer information as events arrive. Filter the feed to focus on **All**, **Gifts**, or **Follows**. Gift combo updates are handled so totals stay useful, and high-volume likes are grouped to keep busy chats easier to read.

### 🎛️ Sync your stream setup with OBS
ShuKanji connects to OBS Studio through OBS WebSocket. It can apply your custom RTMP server and stream key, then read the settings back to make sure OBS received the expected values.

No need to keep copying your stream key into OBS by hand every time you use the normal Go Live flow.

### 🚀 Press Go Live — ShuKanji starts OBS output
Once your Streamlabs details and OBS connection are ready, click **Go Live** in ShuKanji. It retrieves your RTMP server and stream key, applies them to OBS, verifies the settings, and starts the OBS stream output. While you're live, the same control becomes **End Live**.

You stay in control: starting and ending the stream are deliberate actions in the app. ShuKanji does not send chat messages on your behalf.

### 🎨 Pick the look you like
Use the theme selector to switch between **Light**, **Dark**, and **Tebby** themes. Choose the look that feels right for your setup, whether you prefer a bright screen, a darker workspace, or ShuKanji's Tebby style.

## Getting started

For the full walkthrough, see **[Quick Start: connect OBS, go LIVE, and monitor chat](docs/QUICK_START.md)**.

Here's the basic flow:

1. **Set up OBS WebSocket.** In OBS Studio, open **Tools → WebSocket Server Settings** and enable the WebSocket server. The default port is usually `4455`.
2. **Connect ShuKanji to OBS.** Open the OBS connection page in ShuKanji and connect to your local OBS instance. Enter your OBS WebSocket password if you enabled authentication.
3. **Connect Streamlabs.** In ShuKanji's Stream setup page, choose **Load from Web** and sign in to Streamlabs in your browser. Your Streamlabs account needs to be linked to TikTok for managed credential retrieval.
4. **Set up your LIVE.** Enter a stream title and choose a game category.
5. **Go LIVE.** Click **Go Live**. ShuKanji gets the stream details, syncs and verifies the RTMP settings in OBS, then starts OBS output.
6. **Open the LIVE monitor.** On the Live dashboard, enter the TikTok creator's `@username` under TikTok monitor and click **Connect**. Chat and audience events will appear in the feed.

## Make ShuKanji yours

ShuKanji includes three appearance options:

- **Light** — a bright interface.
- **Dark** — a darker interface for low-light setups.
- **Tebby** — ShuKanji's own alternate theme.

Choose your preferred look from the app's theme selector.

## A note about LIVE status

ShuKanji keeps the different parts of your stream setup clear:

1. Stream credentials have been retrieved.
2. OBS settings have been applied and verified.
3. OBS output is running.
4. TikTok has accepted and is showing the LIVE broadcast.

These are not all the same thing. **OBS output running does not, by itself, prove that TikTok has accepted or started showing your LIVE.** Streamlabs provides credentials; TikTok controls whether the broadcast is live.

The TikTok monitor is separate from the Streamlabs credential workflow. It reads the LIVE and displays its events; it does **not** post messages to chat.

## What you'll need

- Windows 10 or Windows 11 (64-bit).
- OBS Studio 28 or later, with OBS WebSocket enabled (default port `4455`).
- A Streamlabs account linked to TikTok for managed stream-key retrieval. Manual RTMP entry is also available.
- Rust 1.85 or later if you want to build the desktop app from source (edition 2024).

## Project status

The repository includes project documentation, the import helper, and downloadable project files. The complete buildable Rust source tree is **not yet present as normal source folders**: the `app/` directory and the `live-rs` crate source still need to be imported and verified. Until then, this repository should not be treated as a complete source build.

## Download the Windows app

A copy of the Windows executable is available here: **[Download shukanji.exe](shukanji.exe)**.

For versioned releases, the recommended place is the repository's **[Releases](https://github.com/avelatojr-source/Shukanji/releases)** page, where the executable can be packaged with the end-user guide.

## Privacy and stored settings

ShuKanji stores non-secret preferences, logs, and its image cache under:

```text
%APPDATA%\TikTokStreamTool\
```

That folder name is retained for compatibility with existing installations. Streamlabs tokens and OBS passwords are stored through the Windows Credential Manager integration, rather than being written to `config.toml`. The optional local Streamlabs token scan is read-only, and logs redact secret values.

One thing to know: the Streamlabs endpoints used for credential retrieval are undocumented internal endpoints, not a stable public API. Their availability may change independently of ShuKanji.

## For developers

The intended source layout keeps two sibling Rust crates:

```text
Shukanji/
├── app/       # Windows desktop app (Rust + egui)
├── live-rs/   # TikTok LIVE client library
├── docs/      # Project and release documentation
└── scripts/   # Source import helper
```

The desktop app uses `../live-rs` as a local Cargo path dependency. Keep the directories side by side; this is intentionally not a Cargo workspace.

See [Architecture](docs/ARCHITECTURE.md), [Manual OBS checklist](docs/MANUAL_OBS_CHECKLIST.md), and [Release checklist](docs/RELEASING.md).

## Licenses

- Desktop application: MIT.
- `live-rs/` client library: 0BSD.

The library keeps its own license; the root MIT license does not replace it.

---

ShuKanji is an independent community-built project. It is not affiliated with, endorsed by, or sponsored by TikTok, Streamlabs, or OBS.

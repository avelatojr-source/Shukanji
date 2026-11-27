# ShuKanji

A native Windows desktop companion for TikTok LIVE streaming. It obtains TikTok
RTMP credentials via Streamlabs, applies them to OBS Studio over OBS WebSocket
5.x, starts the broadcast, and shows the LIVE chat, gifts, likes, follows,
shares and viewer counts while you stream.

This is a from-scratch Rust rewrite of the Python/PySide6
`StreamLabsTikTokStreamKeyGenerator` application, with the ShuKanji interface and
the LIVE monitor added on top. It targets low memory use, a single small binary,
and a correct, verified OBS integration.

> **Status: working, in use.** The Streamlabs sign-in, the OBS apply/verify flow
> and the LIVE monitor have all been exercised against the live services on a
> real broadcast. Automated tests cover the pure logic; the parts that need a
> live account are listed under *Honesty about testing*.

---

## Requirements

- Windows 10/11 (primary target).
- Rust **1.85+** (edition 2024). Verified with `rustc 1.99.1`.
- OBS Studio 28+ with the WebSocket server enabled (Tools → WebSocket Server
  Settings).

## Build and run

```powershell
cargo build --release
.\target\release\shukanji.exe
```

The result is one self-contained executable: fonts, icons and the mascot sprites
are compiled in, so it can be copied anywhere and run on its own. The Windows
icon and version metadata are embedded by `build.rs`.

For development:

```powershell
cargo run
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --check
```

## Features (0.1.0)

- **Streamlabs authentication**: paste a token, sign in via a web PKCE flow
  (loopback callback), or opt in to a read-only local token scan.
- **Account info + category search** with a debounced suggestions list.
- **Credential retrieval** (server URL + stream key) with a masked key and
  explicit reveal/copy. A timeout is reported as *outcome unknown* and is never
  auto-retried.
- **Manual RTMP entry** that applies to OBS without Streamlabs.
- **OBS WebSocket 5.x**: connect (with or without a password), read the stream
  status, apply custom RTMP settings, **read back and verify** them, and
  optionally start/stop output behind an explicit confirmation.
- **Reconnect** with exponential backoff and jitter; the handshake is re-run and
  actual OBS state is re-read after every reconnect.

### Truthful statuses

The UI never conflates credential retrieval with a live broadcast:

- “Credentials retrieved — not verified as live on TikTok.”
- “OBS settings applied and confirmed.”
- “OBS output is running — TikTok broadcast status is unknown.”
- “Request timed out — outcome unknown. Check the account/session before retrying.”

## Out of scope

- **Sending chat.** The app reads your LIVE; it never posts messages. Doing that
  reliably needs the streamer's own browser session, which is not something this
  tool should ask for.
- Overlays, scripting/plugins, multi-account, global hotkeys, stream history,
  Discord webhooks, a local REST API, online update checks, and macOS/Linux builds.
  The downloaded Windows release does include a portable update bootstrap that
  replaces an existing local installation; it does not check for or download
  updates itself.
- Persistence of UI preferences (theme, display name, panel sizes): these reset
  each launch. Only the stream details and OBS connection settings are saved.

## Configuration

Non-secret preferences live in a TOML file. The `TikTokStreamTool` directory name is retained by the current code for compatibility with existing user data:

```
%APPDATA%\TikTokStreamTool\config.toml
```

Only the stream title/category/audience and OBS host/port/remember-password
preference are stored there. **Secrets are never written to this file.**

Logs are written to `%APPDATA%\TikTokStreamTool\logs\`.

## Security notes

- The Streamlabs token and the OBS password are stored in the **Windows
  Credential Manager** (via the `keyring` crate). There is **no plaintext
  fallback**: if the secret store is unavailable, operations fail closed.
- The OAuth flow implements PKCE S256. A callback is accepted when it carries a
  non-empty `code` (an explicit `success=false` denial is still rejected); a
  random `state` is sent and logged if it does not match, but PKCE S256 — not
  `state` — binds the code to this process, matching the original Python flow,
  which neither sent nor checked `state`. It also uses loopback-only binding on
  an OS-assigned port, a strict callback path, a finite timeout, and listener
  cleanup. The authorization code and verifier are never logged.
- Secrets use a `SecretString` wrapper with a redacting `Debug` and best-effort
  zeroization on drop. Logs, errors, and test snapshots never contain secrets.
- Use **Remove stored credentials** to delete the token and OBS password.
- The local Streamlabs token scan is **opt-in**, read-only, and scoped to the
  documented Streamlabs application path. It never runs automatically.

## Streamlabs compatibility risk (important)

The Streamlabs endpoints used here are **undocumented internal endpoints**, not
a public API. Additionally:

- The original Python app sent a **spoofed `StreamlabsDesktop` Electron
  User-Agent**. This port intentionally uses a **truthful** application
  User-Agent and treats acceptance of it as **unverified**. If the endpoints
  reject a truthful UA, change `DEFAULT_USER_AGENT` in
  `src/integrations/streamlabs/client.rs` (documented as a compatibility risk).
- The login endpoint's `state`/`success` echo behavior is **unverified**. The
  callback succeeds whenever it carries an authorization code, so login does not
  depend on Streamlabs echoing either parameter (the reference Python client
  only ever read `code`).

## TikTok LIVE monitor

Watch any TikTok LIVE (chat, gifts, viewer count, likes, follows, shares, joins)
while OBS is streaming — no Streamlabs involvement and no authentication. Enter
the @handle under **TikTok monitor** on the Live dashboard and press Connect.

- Provider: the local checkout of [PirateTok/live-rs](../live-rs), wired in as
  the path dependency `../live-rs`. Keep `live-rs/` at the repository root beside `app/`.
- The username defaults to the TikTok handle from your signed-in Streamlabs
  account; override it in the field if needed.
- The library handles reconnection / stale-socket recovery; the UI surfaces
  `Connecting` / `Monitoring` / `Reconnecting` / failure states.
- **One chronological feed.** Chat, joins, follows, shares, likes and gifts are
  merged by arrival time and shown oldest-to-newest, so the newest line is always
  at the bottom. The three buffers are each already ordered, so the merge is O(n)
  into a buffer the UI reuses — no sorting or allocation while drawing.
- Combo gifts are de-duplicated (an ongoing streak updates in place, keeping its
  position in the feed) and diamond totals accumulate per-event deltas, so totals
  stay correct across gift streaks. Likes are monotonized, and each viewer's taps
  are coalesced into one `X liked ❤️ ×N` line per three seconds.
- Avatars, gift artwork, custom emotes and colour emoji are downloaded through a
  shared on-disk cache (`%APPDATA%\TikTokStreamTool\image-cache`), capped at
  256 MB with the oldest files pruned at start-up.
- **Honesty about testing:** the reducer and the feed ordering are covered by
  automated tests, and the LIVE stream itself has been verified against real
  broadcasts. What has *not* been confirmed live is gift artwork rendering and
  custom (inline) emotes, because they need a specific gift or emote to show up.

## Manual OBS verification

Automated tests use a **mock** OBS server. Before trusting the integration
against a real OBS install, follow
[`docs/MANUAL_OBS_CHECKLIST.md`](../docs/MANUAL_OBS_CHECKLIST.md).

## License

MIT.

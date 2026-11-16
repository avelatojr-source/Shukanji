# Release checklist

## 1. Confirm the version

The supplied desktop app manifest declares version `0.1.0`; the `live-rs` crate has its own independent version (`0.5.0`). Before creating a `v1.0` release, either keep the app version at `0.1.0` and tag accordingly, or bump `app/Cargo.toml` to `1.0.0` and regenerate/update `app/Cargo.lock` with Cargo. Update the app README and release notes in the same change.

## 2. Run checks

From PowerShell:

```powershell
cd app
cargo fmt --check
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo build --release
```

For the included TikTok client, run its checks separately:

```powershell
cd ..\live-rs
cargo test
```

See `live-rs/README.md` for optional capture test data required by replay validation.

## 3. Verify on a real Windows setup

- Follow [MANUAL_OBS_CHECKLIST.md](MANUAL_OBS_CHECKLIST.md) with OBS WebSocket enabled.
- Exercise Streamlabs sign-in and credential retrieval with a linked account.
- Confirm OBS settings are applied and read back correctly before starting output.
- Confirm the app does not imply that OBS output alone proves TikTok is live.
- Connect the TikTok monitor to a live account and check the event feed. Gift artwork and custom emotes require the corresponding events to appear to verify those render paths.

## 4. Package the release

The desktop executable is built at:

```text
app\target\release\shukanji.exe
```

Attach the executable to a GitHub Release, or place it in a release ZIP together with the end-user guide. Keep executable build output and generated packaging outside the Git source tree (`target/`, `dist/`, and `*.exe` are ignored). Do not commit build output to the source repository.

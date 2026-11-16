# Manual OBS verification checklist

Automated tests exercise the OBS client against a **mock** WebSocket server. They do **not** prove behaviour against a real OBS install. Work through the following with OBS Studio 28+ before relying on the OBS path.

## 0. Setup
- [ ] OBS Studio 28 or newer installed.
- [ ] Tools → WebSocket Server Settings → Enable WebSocket server.
- [ ] Note the port (default 4455) and whether authentication is enabled.

## 1. Connect — no password
- [ ] Authentication disabled in OBS.
- [ ] In the app, enter `127.0.0.1` and the port, leave the password empty, and click **Connect**.
- [ ] Expected: status becomes “Connected” with the WebSocket version shown.

## 2. Connect — password enabled
- [ ] Authentication enabled in OBS with a password.
- [ ] Enter the password and connect. Expected: “Connected”.
- [ ] Enter a **wrong** password and connect again. Expected: an error, without a crash.

## 3. Remember password
- [ ] Tick **Remember securely** and connect with a password.
- [ ] Restart the app and click **Connect (saved password)**.
- [ ] Expected: connects without re-typing the password.
- [ ] Confirm the password is **not** present in `%APPDATA%\TikTokStreamTool\config.toml`.

## 4. OBS disconnected
- [ ] Close OBS while the app shows “Connected”.
- [ ] Expected: app shows disconnected/reconnecting; credentials remain available to copy.
- [ ] Reopen OBS. Expected: the app reconnects automatically with backoff/jitter.

## 5. Output inactive — apply
- [ ] Ensure OBS is **not** streaming or recording.
- [ ] Provide credentials (Streamlabs or manual) and click **Apply settings to OBS**.
- [ ] Expected: “OBS settings applied and confirmed.”
- [ ] In OBS, confirm Settings → Stream uses **Custom…** with the expected Server URL and Stream Key.

## 6. Output active — refuse
- [ ] Start streaming in OBS manually.
- [ ] Click **Apply settings to OBS** in the app.
- [ ] Expected: refusal — “Refused: OBS output is running.” OBS settings are unchanged.

## 7. Read-back mismatch
- [ ] If you can make OBS report settings different from the values written, confirm the app shows a mismatch and does **not** claim success.
- [ ] Confirm mismatch details do not contain the stream key.

## 8. Explicit start / stop
- [ ] Click **Start OBS output** once. Expected: the app asks for confirmation, then starts streaming in OBS.
- [ ] Expected status: “OBS output is running — TikTok broadcast status is unknown.”
- [ ] Click **Stop OBS output** with confirmation. Expected: OBS stops.

## 9. Viewport / scaling
- [ ] Test at 100%, 125%, and 150% Windows display scaling.
- [ ] Check the window at 920×620 and larger; no controls should be clipped or unreachable.

## Report

Record the OBS version, WebSocket version, and the pass/fail result for each step in the implementation report or a follow-up issue.

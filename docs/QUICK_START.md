# ShuKanji Quick Start

Want to get your TikTok LIVE chat on screen and have ShuKanji handle the OBS stream setup? Follow these steps the first time you open the app.

## Before you start

Make sure you have:

- A Windows 10 or Windows 11 PC (64-bit).
- OBS Studio 28 or later installed.
- A TikTok account that is going LIVE.
- A Streamlabs account linked to TikTok if you want ShuKanji to retrieve your stream key automatically.
- OBS WebSocket enabled (we'll do that below).

## 1. Turn on OBS WebSocket

1. Open OBS Studio.
2. Go to **Tools → WebSocket Server Settings**.
3. Tick **Enable WebSocket server**.
4. Leave the port at **4455** unless you have a reason to change it.
5. Choose whether to use a password. A password is recommended, especially if OBS may be reachable by other devices on your network.
6. Save or apply the settings.

Keep OBS open while you connect ShuKanji.

## 2. Connect ShuKanji to OBS

1. Open ShuKanji.
2. Go to the **OBS connection** page.
3. For a normal local setup, use host `127.0.0.1` and port `4455` (or the port you chose in OBS).
4. Enter the OBS WebSocket password if authentication is enabled.
5. Click **Connect**.
6. Once connected, you can choose **Remember securely** so the password is stored using Windows Credential Manager. On later launches, use **Connect (saved password)**.

If ShuKanji cannot connect, check that OBS is open and that the WebSocket server is enabled. Make sure the host, port, and password match.

## 3. Connect Streamlabs

1. Open ShuKanji's **Stream setup** page.
2. Click **Load from Web**.
3. Your browser opens the Streamlabs sign-in page. Sign in to the Streamlabs account linked to your TikTok account.
4. Finish the browser sign-in flow and return to ShuKanji.
5. Check that your account appears in the app.
6. Enter your **Stream title** and choose a **Game category**.

Your Streamlabs token is kept in Windows Credential Manager. If you prefer not to use the Streamlabs connection, you can enter RTMP details manually.

## 4. Go LIVE — ShuKanji syncs OBS for you

This is the main part of the setup:

1. Make sure OBS is connected in ShuKanji.
2. Make sure your Streamlabs account or manual RTMP details are ready.
3. Click **Go Live** at the bottom-right of ShuKanji.
4. ShuKanji retrieves the RTMP server URL and stream key.
5. It applies those details to OBS as a custom stream destination.
6. ShuKanji reads the OBS settings back to verify that the server and key match what it intended to apply.
7. Once the settings are confirmed, ShuKanji starts the OBS stream output.

You do not have to manually paste the stream key into OBS during this normal Go Live flow. When the stream is running, the same control changes to **End Live**. Use it when you're ready to stop.

**A useful status detail:** A running OBS output is not the same as TikTok confirming that the broadcast is live. ShuKanji distinguishes between credentials being retrieved, OBS settings being verified, OBS output running, and TikTok accepting the LIVE broadcast.

## 5. Watch your TikTok LIVE chat

1. Open the **Live dashboard** page in ShuKanji.
2. Find **TikTok monitor**.
3. Type the creator's TikTok handle (with or without the `@`, as accepted by the field).
4. Click **Connect**.
5. Incoming chat messages and activity appear in one chronological feed, with the newest items at the bottom.

The monitor can show:

- Chat messages
- Gifts
- Likes
- Follows
- Shares
- Joins
- Viewer information

Use **All**, **Gifts**, and **Follows** to filter the feed. You can also use **Open Live Monitor** or **Open my LIVE page** to open TikTok in your browser.

ShuKanji only reads the LIVE. It does not send messages to your chat.

## 6. Choose your theme

Want a different look while you stream? Open ShuKanji's theme selector and choose one of the available themes:

- **Light** — a bright look.
- **Dark** — a darker look for a low-light room or a less bright screen.
- **Tebby** — ShuKanji's alternate themed look.

Switch between them to find the one you like. Your choice changes the app's appearance, not your OBS or TikTok stream settings.

## Quick troubleshooting

### ShuKanji says it cannot connect to OBS
Open OBS and check **Tools → WebSocket Server Settings**. Confirm that the server is enabled and that the host, port, and password in ShuKanji match OBS.

### Go Live cannot apply settings
Make sure OBS is connected and that OBS is not already streaming or recording while ShuKanji tries to apply the RTMP settings. Check that your Streamlabs account is linked to TikTok, or confirm the manual RTMP server and key are correct.

### The app says credentials were retrieved, but TikTok is not showing a LIVE
That message means ShuKanji received the stream credentials. It does not mean TikTok has confirmed the broadcast. Check the OBS output and TikTok's LIVE status separately.

### The chat feed is empty
Check that the TikTok handle is spelled correctly and that the account is currently LIVE. The LIVE monitor connects separately from the Streamlabs credential workflow.

## Where ShuKanji stores things

- Streamlabs token and OBS password: Windows Credential Manager.
- Preferences: `%APPDATA%\TikTokStreamTool\config.toml`.
- Logs: `%APPDATA%\TikTokStreamTool\logs\`.
- Cached avatars, gift artwork, emotes, and emoji images: `%APPDATA%\TikTokStreamTool\image-cache\`.

The image cache can be deleted if needed; ShuKanji downloads images again as they are needed.

SHUKANJI — QUICK START
======================

Your TikTok LIVE chat, OBS setup, and stream activity — together in one place.

ShuKanji is a Windows desktop companion for TikTok LIVE creators. It helps you follow chat and audience activity while handling the OBS stream setup from one app.

WHAT YOU NEED
-------------
* Windows 10 or Windows 11 (64-bit)
* OBS Studio 28 or later
* OBS WebSocket enabled
* A Streamlabs account linked to TikTok for automatic stream-key retrieval
  (manual RTMP details are also supported)


1. ENABLE OBS WEBSOCKET
-----------------------
1. Open OBS Studio.
2. Go to Tools -> WebSocket Server Settings.
3. Enable the WebSocket server.
4. The default port is 4455. You can keep it unless you changed it.
5. Set a password if you want OBS WebSocket authentication, then save.

Keep OBS open.


2. CONNECT SHUKANJI TO OBS
--------------------------
1. Open ShuKanji and go to the OBS connection page.
2. Use 127.0.0.1 as the host and 4455 as the port for a normal local setup.
3. Enter your OBS WebSocket password if you enabled one.
4. Click Connect.
5. You can choose Remember securely, then use Connect (saved password)
   next time.

If it cannot connect, check that OBS is open and the host, port, and password
match the OBS WebSocket settings.


3. CONNECT STREAMLABS
---------------------
1. Open the Stream setup page in ShuKanji.
2. Click Load from Web.
3. Your browser opens. Sign in to the Streamlabs account linked to TikTok.
4. Finish the sign-in flow and return to ShuKanji.
5. Enter your Stream title and choose a Game category.

The Streamlabs token is stored in Windows Credential Manager. You can also
enter RTMP details manually instead of using Streamlabs.


4. GO LIVE — OBS SYNC HAPPENS AUTOMATICALLY
-------------------------------------------
1. Make sure OBS is connected in ShuKanji.
2. Make sure your Streamlabs account or manual RTMP details are ready.
3. Click Go Live at the bottom-right of ShuKanji.
4. ShuKanji retrieves the RTMP server URL and stream key.
5. It applies those details to OBS as a custom stream destination.
6. ShuKanji reads the settings back to verify they match.
7. After the settings are confirmed, ShuKanji starts the OBS stream output.

You do not need to paste the stream key into OBS by hand during this normal
Go Live flow. While the stream is running, the same control becomes End Live.

Important: OBS output running does not, by itself, prove that TikTok has
accepted or started showing your LIVE. Credentials retrieved, OBS settings
verified, OBS output running, and TikTok LIVE status are separate states.


5. WATCH YOUR TIKTOK LIVE CHAT
------------------------------
1. Open the Live dashboard page.
2. Find TikTok monitor.
3. Type the creator's TikTok @handle and click Connect.
4. Incoming activity appears in one chronological feed, newest at the bottom.

The feed can show chat messages, gifts, likes, follows, shares, joins, and
viewer information. Use All, Gifts, and Follows to filter it.

ShuKanji reads the LIVE; it does not send messages to your chat.


6. CHOOSE YOUR THEME
--------------------
Open ShuKanji's theme selector and choose the look you prefer:

* Light — a bright look.
* Dark — a darker look for low-light setups.
* Tebby — ShuKanji's alternate themed look.

Changing the theme changes the app's appearance, not your OBS or TikTok
stream settings.


TROUBLESHOOTING
---------------
* Cannot connect to OBS:
  Open OBS -> Tools -> WebSocket Server Settings. Make sure the server is
  enabled and the host, port, and password in ShuKanji match.

* Go Live cannot apply settings:
  Make sure OBS is connected and is not already streaming or recording while
  the settings are being applied. Check your Streamlabs link or manual RTMP
  server and key.

* Credentials retrieved, but TikTok is not showing a LIVE:
  ShuKanji has received the stream credentials, but TikTok has not necessarily
  confirmed the broadcast. Check OBS output and TikTok LIVE status separately.

* Chat feed is empty:
  Check the TikTok handle and make sure that account is LIVE. The monitor
  connects separately from the Streamlabs credential workflow.


WHERE THINGS ARE STORED
-----------------------
* Streamlabs token and OBS password: Windows Credential Manager.
* Preferences: %APPDATA%\TikTokStreamTool\config.toml
* Logs: %APPDATA%\TikTokStreamTool\logs\
* Cached images: %APPDATA%\TikTokStreamTool\image-cache\

The image cache can be deleted; ShuKanji downloads images again as needed.


THEMES
------
Light, Dark, and Tebby are available in the app's theme selector.

For more detail, see the repository's README.md and docs/QUICK_START.md.

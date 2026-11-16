ShuKanji
========

A desktop companion for TikTok LIVE streaming. It gets your TikTok stream key
through Streamlabs, configures and starts OBS for you, and shows your LIVE chat,
gifts, likes, follows, shares and viewer counts in real time.


REQUIREMENTS
------------

* Windows 10 or 11 (64-bit)
* OBS Studio, with its WebSocket server turned on:
  OBS -> Tools -> WebSocket Server Settings -> "Enable WebSocket server"
  (the default port 4455 is already filled in for you)
* A Streamlabs account linked to TikTok, for the stream key

Nothing else is needed - the app is a single .exe with everything built in.


RUN
---

Double-click shukanji.exe.


FIRST-TIME SETUP
----------------

1. Stream setup page -> "Load from Web"
   Your browser opens and you sign in to Streamlabs. When it finishes, your
   account name appears and the app keeps the token in the Windows Credential
   Manager.

2. Fill in "Stream title" and pick a "Game category".

3. OBS connection page -> "Connect" (or "Connect (saved password)").

4. Press the "Go Live" button at the bottom right.
   It connects to OBS, requests the stream key from Streamlabs, applies the
   settings to OBS, verifies them, and starts streaming. The same button becomes
   "End Live" while you are live; it always shows what is really happening.

   You can also collapse the bottom bar with the small arrow on its left if you
   want more vertical space.


WATCHING YOUR LIVE
------------------

Live dashboard page:

* Type your TikTok @handle under "TikTok monitor" and press Connect.
  Incoming chat, gifts, likes, follows, shares and joins appear in one
  chronological feed, newest at the bottom.
* "All", "Gifts" and "Follows" filter the feed.
* "Open Live Monitor" and "Open my LIVE page" open TikTok in your browser.
* Drag the grip under the viewer counters, or the grip at the bottom of the
  chat, to resize them. Both can only be made taller, never smaller than the
  default.

This app only *reads* your LIVE. It cannot post messages to your chat.


WHERE THINGS ARE STORED
-----------------------

* Secrets (Streamlabs token, OBS password): Windows Credential Manager.
  Nothing secret is ever written to a plain file.
* Preferences: %APPDATA%\TikTokStreamTool\config.toml
* Logs: %APPDATA%\TikTokStreamTool\logs\
* Cached avatars, gift images, emote and emoji images:
  %APPDATA%\TikTokStreamTool\image-cache\
  Safe to delete at any time; it re-downloads as needed.


TROUBLESHOOTING
---------------

* "Could not connect to OBS" - open OBS and check that the WebSocket server is
  enabled, and that the host/port on the OBS connection page match.
* Nothing appears in the LIVE chat - check that the @handle is spelled exactly
  as it appears on TikTok and that the account is actually live right now.
* "Credentials retrieved - not verified as live on TikTok" - this is normal.
  Streamlabs gives out a key; only TikTok can confirm the broadcast itself.

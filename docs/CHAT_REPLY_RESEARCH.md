# Research: replying to TikTok LIVE chat from ShuKanji with your own account

Researched 2026-11-25. Question: can the app send (reply to) LIVE chat messages
as the signed-in creator, instead of only reading them?

## TL;DR

- **There is no official TikTok API for LIVE chat** — neither reading nor
  sending. TikTok's own developer platform does not expose it.
- **Sending is technically possible today** through the third-party
  **Euler Stream** sign/chat service, authenticated with **your own TikTok
  session cookies**. The Node.js `TikTok-Live-Connector` library added exactly
  this (`sendMessage`) in its `ts-rewrite` branch.
- It is **not free of consequences**: the route requires a `WEBCAST_PREMIUM`
  scoped API key (paid add-on territory), your session cookies are sent to a
  third party, and automated sending can violate TikTok's Terms of Service
  (account-restriction risk).
- **The safe, supported way to reply as yourself is TikTok's own LIVE
  Center**, which ShuKanji already opens via the **Open Live Monitor** button
  on the Live dashboard.

## What the community libraries do

| Library | Read chat | Send chat |
| --- | --- | --- |
| `isaackogan/TikTokLive` (Python) | Yes | **No** — no send method exists |
| `zerodytrash/TikTok-Live-Connector` (Node.js, `ts-rewrite`) | Yes | **Yes** — `sendMessage(content, [roomId])`, via Euler Stream |
| ShuKanji's `live-rs` | Yes | No (read-only by design) |

The Node connector's docs state the requirements plainly: sending needs a
`signApiKey` **and** an authenticated session (`sessionid` + `tt-target-idc`
cookies, or an OAuth token). Reading needs neither.

## The exact HTTP contract (from Euler Stream's public OpenAPI spec)

Source: `EulerStream/TikTok-Api-Sdk`, `spec/openapi.json`
(route operation id `SendRoomChat`).

```
POST https://api.eulerstream.com/webcast/rooms/{room_id}/chat
```

Headers:

| Header | Required | Meaning |
| --- | --- | --- |
| `x-api-key` | Yes (or `?api_key=` query) | Euler Stream API key, scope **`WEBCAST_PREMIUM`** |
| `x-cookie-header` | One of the two | Full cookie string containing `sessionid` and `tt-target-idc` copied from your logged-in TikTok browser session |
| `x-oauth-token` | One of the two | OAuth access token; the session is resolved from the stored OAuth session |

JSON body (schema `WebcastRoomChatPayload`):

```json
{
  "content": "Thanks for the follow! ❤️",
  "targetUniqueId": "some_creator"
}
```

- `content` is the only required field.
- `targetUniqueId` = the creator's username; when used, pass `room_id = 0` in
  the path. `targetRoomId` (the numeric room id ShuKanji already receives on
  `TikTokLiveEvent::Connected`) also works but is marked deprecated in favor of
  the username flow.
- Error responses: `429` rate limit, `503` "No agents are currently available"
  (Euler sends through its own agent pool), `500` internal.

Getting your cookies (manual, per the connector docs): log into tiktok.com in
your browser → DevTools → Application → Cookies → copy `sessionid` (secret!)
and `tt-target-idc` (datacenter region, e.g. `useast1a`). Both are required.

## How this could integrate with ShuKanji (if ever enabled)

1. **Settings → TikTok account (experimental):** paste `sessionid` +
   `tt-target-idc`, stored via the existing Windows Credential Manager store —
   never in `config.toml`. Show an explicit warning that these cookies are
   sent to Euler Stream, a third party.
2. **Reply box under the Live chat** (the UI already reserves space for it in
   `CHAT_CHROME_HEIGHT`), enabled only while the monitor is connected.
3. **New `Command::SendLiveChat { content }`** handled by a background task
   using `reqwest` against the route above; the connected monitor already
   supplies `room_id`.
4. **Client-side rate limiting** (e.g. 1 message / 3 s, 200-char cap), send
   status in the chat line, `429`/`503` surfaced as toasts.

Estimated effort: 1–2 days including the credential UI. The blocker is not
code — it is cost (`WEBCAST_PREMIUM`), trust (cookies to a third party), and
account risk.

## Risks (read before building this)

- **Terms of Service / bans.** Automated chat posting through unofficial
  channels can get the *sending account* restricted. TikTok enforces this
  independently of which library is used.
- **Credential exposure.** `sessionid` is full account access. With
  `x-cookie-header` it travels to Euler Stream; anyone holding it can act as
  you. Never log it (ShuKanji's redaction layer would need to cover it).
- **Session churn.** Cookies expire and rotate; expect periodic re-pasting.
- **Paid scope & availability.** The route's security scope is
  `WEBCAST_PREMIUM`; free community keys may be rejected. `503` means Euler
  had no agent available — sending is best-effort, not guaranteed.
- **Project stance.** ShuKanji's README currently states it does not post
  messages on your behalf. Enabling this changes that promise and should come
  with an explicit opt-in and README update.

## Recommendation

Keep **Open Live Monitor** (TikTok LIVE Center) as the supported reply path —
it is official, free, and zero-risk. If in-app replying is still wanted,
implement the Euler Stream route as a clearly-labeled experimental opt-in with
the warnings above, and expect it to break when TikTok changes its protocol.

## Sources

- Node.js connector (sendMessage, authenticated session, signing):
  <https://github.com/zerodytrash/TikTok-Live-Connector>
- Python library (read-only reference):
  <https://github.com/isaackogan/TikTokLive>
- Euler Stream SDK + OpenAPI spec (`SendRoomChat` route, payload, security
  scope, plans): <https://github.com/EulerStream/TikTok-Api-Sdk> and
  <https://www.eulerstream.com/pricing>

# Relay protocol

How the extension hands a webtiles game to the app (ADR 0001). The integer **relay protocol version** is the only compatibility contract between the two: the app accepts the current version and the previous one. Bump it whenever a frame below changes shape.

Current version: **2**. Version 2 added the `window` and `resize-window` frames, so the app can dock its panel beside the browser.

## Capture

A MAIN-world script wraps the page's `comm.handle_message`, through which every decoded webtiles message passes, and wraps each one in an **envelope**:

```json
{ "origin": "https://crawl.akrasiac.org", "tab": 12, "ts": 1760486400000, "gameVersion": "Dungeon Crawl Stone Soup 0.34.1", "msg": { "msg": "player", "hp": 20 } }
```

| Field | Meaning |
|---|---|
| `origin` | The game server's page origin. |
| `tab` | The browser tab the game runs in. |
| `ts` | When the page received the message, in Unix milliseconds. |
| `gameVersion` | The `text` of the most recent webtiles `version` message the page saw, verbatim, or `null` before one arrives. |
| `msg` | The webtiles message, unchanged. |

The extension parses nothing beyond this. It skips only `multi` messages, whose children pass through `handle_message` on their own.

The page keeps every envelope since it loaded, up to about 25 MB, and replays them all when the app connects, so the app always sees a game from its first message. Past the limit the buffer is dropped and the tab is marked overflowed: live messages still flow, but a later connect can't replay the history, and the app tells the player to reload the game page.

## Transport

A websocket from the extension's background to `ws://127.0.0.1:47821`. The app checks the HTTP `Origin` header before upgrading: only browser extension origins (`chrome-extension://`, `moz-extension://`) are accepted, so web pages (`http(s)://`, `null`) are rejected with 403.

Every frame is one JSON text message tagged by `type`. The Rust side of each frame lives in `copilot_webtiles::relay_protocol`, shared by the app and the fake extension.

### Extension → app

| Frame | When |
|---|---|
| `{"type":"hello","protocol":2}` | First frame on every connection. |
| `{"type":"envelope","envelope":{…}}` | Each captured message, replayed buffer first. |
| `{"type":"overflow","origin":"…","tab":12}` | A tab's buffer overflowed before it could be replayed. |
| `{"type":"window","bounds":{"left":0,"top":25,"width":1920,"height":1055},"state":"maximized"}` | Where the game window is: on welcome, then each time it moves or resizes, or the player turns to a webtiles tab in another window. |
| `{"type":"ping"}` | Every 20 s, to keep the extension's service worker alive. |

### App → extension

| Frame | When |
|---|---|
| `{"type":"welcome","protocol":2}` | The browser is allowed and the protocol is supported. Envelopes may follow. |
| `{"type":"awaiting-approval"}` | First connect from this browser: the app is asking the player "Allow this browser?". `welcome` or `refused` follows, unless a newer connection is asked instead: then the app closes this one without a frame and the extension retries. |
| `{"type":"resize-window","bounds":{"left":0,"top":25,"width":1500,"height":1055}}` | Make Room is on and the panel doesn't fit beside the game window: the extension moves and resizes that window to these bounds, in its normal state. |
| `{"type":"refused","reason":"unsupported-protocol"}` | The extension's protocol is neither the app's current nor previous version. The app closes the connection. |
| `{"type":"refused","reason":"browser-denied"}` | The player answered no, now or earlier. The app closes the connection. |

The player's answer is remembered per browser origin.

## The game window

The **game window** is the browser window holding the webtiles tab the player turned to last. Its `bounds` are the outer bounds the browser's `windows` API gives, in screen pixels, which the app takes as logical pixels; its `state` is `normal`, `minimized`, `maximized` or `fullscreen` (Chrome's kiosk `locked-fullscreen` is sent as `fullscreen`). Chrome reports each move once it ends (`windows.onBoundsChanged`); Firefox has no such event, so the extension reads the window every second and sends it only when it changed.

These frames carry no game data: the extension still reads nothing of a game message.

Each connection starts afresh: every tab replays its buffer, so the app rebuilds each game from its first message and a dev build starts a new Recording.

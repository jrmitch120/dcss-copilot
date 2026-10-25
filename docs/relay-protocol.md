# Relay protocol

How the extension hands a webtiles game to the app, and shows the app's panel in the browser's Side Panel (ADR 0001, ADR 0004). The integer **relay protocol version** is the only compatibility contract between the two: the app accepts the current version and the previous one. Bump it whenever a frame below changes shape.

Current version: **4**. Version 2 added the `side-panel-opened`, `panel-command`, `view` and `panel-command-done` frames, so the panel shows in the browser's Side Panel; version 3 added the `set-assistance` command, so the player sets their Assistance Level there too; version 4 added the `new-game` command, so the player can say there that a game the Advisor picked up again is a new one.

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
| `{"type":"hello","protocol":4}` | First frame on every connection. |
| `{"type":"envelope","envelope":{…}}` | Each captured message, replayed buffer first. |
| `{"type":"overflow","origin":"…","tab":12}` | A tab's buffer overflowed before it could be replayed. |
| `{"type":"side-panel-opened"}` | A Side Panel opened while none was, or one is open as the app welcomes the browser. The app's own window gives way to it. |
| `{"type":"panel-command","id":4,"command":{"kind":"set-appearance","appearance":"glyphs"}}` | The player pressed something in a Side Panel. `command` is `set-appearance`, `set-assistance` (with `"assistance":"info"`, `"hints"` or `"coach"`), `new-game` or `open-settings`; `id` is the extension's, for the answer. |
| `{"type":"ping"}` | Every 20 s, to keep the extension's service worker alive. |

### App → extension

| Frame | When |
|---|---|
| `{"type":"welcome","protocol":4}` | The browser is allowed and the protocol is supported. Envelopes may follow. |
| `{"type":"awaiting-approval"}` | First connect from this browser: the app is asking the player "Allow this browser?". `welcome` or `refused` follows, unless a newer connection is asked instead: then the app closes this one without a frame and the extension retries. |
| `{"type":"view","view":{…}}` | The panel's view model, for the Side Panels and the Toolbar Badge: after `welcome`, then on every change. |
| `{"type":"panel-command-done","id":4,"error":null}` | The command with this `id` was done, or `error` says why it couldn't be. |
| `{"type":"refused","reason":"unsupported-protocol"}` | The extension's protocol is neither the app's current nor previous version. The app closes the connection. |
| `{"type":"refused","reason":"browser-denied"}` | The player answered no, now or earlier. The app closes the connection. |

The player's answer is remembered per browser origin.

## Side Panels

The extension shows the app's panel in the browser's Side Panel (Firefox calls it the sidebar), opened from the extension's toolbar button. A `view` is the same view model the app's own panel draws, in the shape of `app/src/lib/bindings.ts`, and the Side Panel draws it with the app's Svelte components. While no Side Panel is open, the extension shows the Toolbar Badge from the latest view instead.

A Side Panel can't answer "Allow this browser?": there is no frame for it, so a browser can never allow itself. These frames carry no game messages: the extension still reads nothing of a game.

Each connection starts afresh: every tab replays its buffer, so the app rebuilds each game from its first message and a dev build starts a new Recording.

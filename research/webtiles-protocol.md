# Webtiles protocol: what a player and a spectator can see, and how to capture it

Resolves [#2](https://github.com/jrmitch120/dcss-copilot/issues/2). Researched 2026-10-13.

Sources are pinned to crawl trunk commit
[`28a137d`](https://github.com/crawl/crawl/tree/28a137decf34033cc50b797d3677e82da05aba5d)
(2026-10-13). Stable-version comparisons use tags `0.34.1`, `0.30.0` and `0.25.0`.
Short link prefixes used below:

- `TW` = [`crawl-ref/source/tileweb.cc`](https://github.com/crawl/crawl/blob/28a137decf34033cc50b797d3677e82da05aba5d/crawl-ref/source/tileweb.cc)
- `PH` = [`webserver/webtiles/process_handler.py`](https://github.com/crawl/crawl/blob/28a137decf34033cc50b797d3677e82da05aba5d/crawl-ref/source/webserver/webtiles/process_handler.py)
- `WS` = [`webserver/webtiles/ws_handler.py`](https://github.com/crawl/crawl/blob/28a137decf34033cc50b797d3677e82da05aba5d/crawl-ref/source/webserver/webtiles/ws_handler.py)
- `CJ` = [`webserver/static/scripts/client.js`](https://github.com/crawl/crawl/blob/28a137decf34033cc50b797d3677e82da05aba5d/crawl-ref/source/webserver/static/scripts/client.js)
- `CM` = [`webserver/static/scripts/comm.js`](https://github.com/crawl/crawl/blob/28a137decf34033cc50b797d3677e82da05aba5d/crawl-ref/source/webserver/static/scripts/comm.js)
- `GD` = [`webserver/game_data/static/`](https://github.com/crawl/crawl/tree/28a137decf34033cc50b797d3677e82da05aba5d/crawl-ref/source/webserver/game_data/static), the per-version game client JS

## TL;DR

1. **The player and spectators get the same game stream.** The crawl process writes one
   JSON stream to its Unix socket. The Python server copies every game message to every
   receiver, player and spectators alike. The only differences are (a) a spectator
   gets a one-off full resync when it joins, (b) a few non-game messages (wrapper-script
   stdout, lobby, rc editing) go only to the player, and (c) spectators can't send input.
   So a capture method works the same in either role.
2. **What the stream carries:** the player's stats, statuses, place, gold and XL; the full
   inventory with names; the **current level's** map, including remembered cells,
   with basic monster info per cell (name, type, attitude, threat, average HP); the
   message log, sent as deltas; every menu, prompt and description popup the player
   opens, including the dungeon overview, skills and `%` screens while they are open; and
   game-end, dump and milestone events. Things it does **not** stream continuously: skills,
   spells, mutations, other levels' maps, individual monster HP, and message history from
   before a spectator joined.
3. **Stability:** the outer framing (JSON over websocket, `msg` type field, delta updates)
   has been stable for 10+ releases. The payload is not. Keys get added and removed between
   versions (0.34.1 to trunk: 4 added, 1 removed), and features, monster types and item
   types are sent as **version-specific numeric enums**. The server is version-independent,
   but it serves a **different client JS per game version**. The Advisor needs a
   per-version decoding table. That ties capture to Game Knowledge being per-version.
4. **Capture:** the best option is a browser extension content script running in the
   page's `MAIN` world that wraps the page's `comm.handle_message`, which every decoded
   message passes through. The alternative is wrapping `WebSocket` at `document_start` and
   inflating the stateful deflate stream yourself. The extension passes messages to the
   desktop app through either a localhost websocket from the extension service worker
   (Chrome 116+) or native messaging, which has a 1 MB host-to-extension limit and
   64 MiB extension-to-host. Local webtiles uses the same server code and client JS, so
   it works the same way.
5. **Backfill on mid-game join:** a resync gives the full current Visible State, but only
   the current level's map and no message backlog. Accumulated State for other levels
   can't come from the stream. The realistic backfill sources are the character dump
   (overview, skills, notes, recent messages), fetched from the server's morgue URL after
   the player saves or presses `#`, and capturing the overview menu when the player opens
   it.

## 1. Architecture of the stream

```
crawl process --(Unix datagram socket, JSON lines)--> webtiles server (Python/Tornado)
     ^                                                 | fan-out to all receivers
     | control msgs (key, spectator_joined, ...)       v
     +------------------------------------------ browser(s): player + spectators
```

- The crawl process builds JSON in `m_msg_buf` and `finish_message()` `sendto`s each
  fragment to **every** address in `m_dest_addrs`. In practice that is the one webserver
  socket ([TW L192-290](https://github.com/crawl/crawl/blob/28a137decf34033cc50b797d3677e82da05aba5d/crawl-ref/source/tileweb.cc#L192-L290)).
  Messages starting with `*` are addressed to the server, not to clients (`client_path`,
  `flush_messages`, `dump`, `exit_reason`, `milestone`;
  [PH L1156-1212](https://github.com/crawl/crawl/blob/28a137decf34033cc50b797d3677e82da05aba5d/crawl-ref/source/webserver/webtiles/process_handler.py#L1156-L1212)).
- `CrawlProcessHandlerBase.handle_process_message` appends every game message to every
  receiver in `self._receivers`. That set holds the player's socket and every spectator's
  socket. The one exception is the full-map heuristic below
  ([PH L196-213](https://github.com/crawl/crawl/blob/28a137decf34033cc50b797d3677e82da05aba5d/crawl-ref/source/webserver/webtiles/process_handler.py#L196-L213)).
- Server to browser: each websocket gets its **own stateful zlib stream**. The server
  deflates, sync-flushes, strips the trailing `00 00 FF FF`, and sends a binary frame.
  The client asks for `no-compression` as a subprotocol if it can't inflate
  ([WS L403-420](https://github.com/crawl/crawl/blob/28a137decf34033cc50b797d3677e82da05aba5d/crawl-ref/source/webserver/webtiles/ws_handler.py#L403-L420),
  [WS L1357-1371](https://github.com/crawl/crawl/blob/28a137decf34033cc50b797d3677e82da05aba5d/crawl-ref/source/webserver/webtiles/ws_handler.py#L1357-L1371)).
  The client re-appends `00 00 FF FF` and feeds a long-lived `Inflater` from
  `contrib/inflate.js`
  ([CJ L1683-1745](https://github.com/crawl/crawl/blob/28a137decf34033cc50b797d3677e82da05aba5d/crawl-ref/source/webserver/static/scripts/client.js#L1683-L1745)).
- Each decoded frame is either one JSON object or a `{"msgs":[...]}` batch. Each message
  is dispatched by its `msg` field through `comm.handle_message`
  ([CJ L42-90](https://github.com/crawl/crawl/blob/28a137decf34033cc50b797d3677e82da05aba5d/crawl-ref/source/webserver/static/scripts/client.js#L42-L90),
  [CM L25-52](https://github.com/crawl/crawl/blob/28a137decf34033cc50b797d3677e82da05aba5d/crawl-ref/source/webserver/static/scripts/comm.js#L25-L52)).
  The client parses with `eval`, not `JSON.parse`, because "0.11 and older send invalid
  JSON messages" (CJ L62-64).

## 2. What the stream carries

All game-state messages are **deltas**. Each sender keeps a copy of what it last sent and
emits only changed fields, unless `force_full` is set. A consumer has to keep its own
merged model, as the page's `GD` modules do.

| `msg` | Source | Contents |
|---|---|---|
| `version` | [TW L777-786](https://github.com/crawl/crawl/blob/28a137decf34033cc50b797d3677e82da05aba5d/crawl-ref/source/tileweb.cc#L777-L786) | `"text": "Crawl <Version::Long>"`. Sent at startup (TW L163) and on spectator join. |
| `player` | [TW L1132-1335](https://github.com/crawl/crawl/blob/28a137decf34033cc50b797d3677e82da05aba5d/crawl-ref/source/tileweb.cc#L1132-L1335) | name, title, species, god, piety rank, penance, form, hp/hp_max/real_hp_max, mp, ac/ev/sh (+temp mods), str/int/dex, doom, contam, xl, progress, gold, noise, time/turn, place/depth, pos, `status[]` (light text, description, colour), `inv` (52 slots, see below), quiver, weapon/offhand indices |
| inventory items (inside `player.inv`) | [TW L1383-1505](https://github.com/crawl/crawl/blob/28a137decf34033cc50b797d3677e82da05aba5d/crawl-ref/source/tileweb.cc#L1383-L1505) | `base_type`, `sub_type` (numeric enums), quantity, plus/plus2, flags, inscription, letter, `useless`, **`name`** (full display name), tile, action_panel_order |
| `map` | [TW L1902-2037](https://github.com/crawl/crawl/blob/28a137decf34033cc50b797d3677e82da05aba5d/crawl-ref/source/tileweb.cc#L1902-L2037), cell fields [TW L1603+](https://github.com/crawl/crawl/blob/28a137decf34033cc50b797d3677e82da05aba5d/crawl-ref/source/tileweb.cc#L1603) | `cells[]` for dirty cells, or all cells when `clear:true`. Per cell: `f` (dungeon_feature_type enum), `mf` (map feature), glyph `g`, colour, tile data `t`, and `mon`. Comes from `env.map_knowledge`, so it covers **remembered (out-of-LOS) cells of the current level**. Also `vgrdc`, `player_on_level`, `invis_mon_desc`. |
| monster (inside a cell) | [TW L2039-2103](https://github.com/crawl/crawl/blob/28a137decf34033cc50b797d3677e82da05aba5d/crawl-ref/source/tileweb.cc#L2039-L2103) | `id`, `name`, `plural`, `type` (monster_type enum), `typedata.avghp`, `no_exp`, `att` (attitude), `btype`, `threat`. **No current HP, spells or statuses.** The page builds its monster list from these (`GD/monster_list.js`). |
| `msgs` | [`message.cc` L900-950](https://github.com/crawl/crawl/blob/28a137decf34033cc50b797d3677e82da05aba5d/crawl-ref/source/message.cc#L900-L950) | new message lines (`text` with colour tags, `turn`, `channel`), `rollback`, `more` |
| `menu`, `update_menu`, `update_menu_items`, `menu_scroll`, `close_menu` | `menu.cc`, TW L857-900 | full menu contents: inventory screens, the `^O` dungeon overview, `m` skills, `I` spells, `A` abilities, `%` and so on, **only while the player has them open** |
| `ui-push` / `ui-pop` / `ui-state` / `ui-stack` | TW L921-960 | newer UI layouts: `describe-monster`, `describe-item`, `describe-spell`, `describe-god`, `formatted-scroller`, `game-over`, and so on (handlers in `GD/ui-layouts.js`) |
| `txt` | `m_text_menu` / `tileweb-text.cc` | text regions, e.g. the CRT layer |
| `options`, `layout`, `input_mode`, `ui_state`, `cursor`, `text_cursor`, `delay` | various | client config and UI mode |
| `game_client`, `game_started`, `watching_started`, `game_ended`, `dump`, `update_spectators`, `chat`, `lobby_*` | server ([PH](https://github.com/crawl/crawl/blob/28a137decf34033cc50b797d3677e82da05aba5d/crawl-ref/source/webserver/webtiles/process_handler.py), [WS](https://github.com/crawl/crawl/blob/28a137decf34033cc50b797d3677e82da05aba5d/crawl-ref/source/webserver/webtiles/ws_handler.py)) | session events. `dump` carries a URL to the char dump when the player runs `#` and the server has `morgue_url` set (PH L1180-1187). |

Not streamed except through a menu or dump: **skills and training, memorised spells
and fail rates, mutations, abilities, other levels' maps, the dungeon overview, notes,
and individual monster HP and status details.** A full-map send happens on level change
(`clear_minimap()` sets `m_need_full_map`, TW L2344-2349), so the stream only ever
describes the current level.

## 3. Player connection vs. spectator connection

| Aspect | Player | Spectator | Source |
|---|---|---|---|
| Game messages (`player`, `map`, `msgs`, menus, UI) | yes | **same bytes**, copied from the same process stream | PH L196-213 |
| Initial state | built up from game start | `add_watcher` sends `{"msg":"spectator_joined"}` to the process, which runs `_send_everything()`: version, options, layout, full `player`, full `map` (flagged `spect_only` and sent only to fresh watchers), the whole open `ui-stack`, unsent messages, input mode | PH L1058-1063, [TW L470-475](https://github.com/crawl/crawl/blob/28a137decf34033cc50b797d3677e82da05aba5d/crawl-ref/source/tileweb.cc#L470-L475), [TW L2193-2267](https://github.com/crawl/crawl/blob/28a137decf34033cc50b797d3677e82da05aba5d/crawl-ref/source/tileweb.cc#L2193-L2267) |
| Message history | everything since the game loaded | **only messages after joining**. `_send_messages()` sends only `unsent` lines, and there is no backlog replay | message.cc L900-921, TW L2181-2188 |
| `time_last_input` | not sent | sent in the forced-full `player` | TW L1236-1240 |
| Options | the player's rc options | the process's options, plus the spectator's own JSON options marked `watcher:true` | [WS L1015-1024](https://github.com/crawl/crawl/blob/28a137decf34033cc50b797d3677e82da05aba5d/crawl-ref/source/webserver/webtiles/ws_handler.py#L1015-L1024) |
| Wrapper-script stdout | yes | no ("send messages from wrapper scripts only to the player") | PH L1104-1116 |
| Input | `key`/`input` go to the process | dropped. Only receivers with `self.process` forward input | WS L1305-1316, PH L1065-1086 |
| Visibility to others | n/a | listed in `update_spectators`. The player can block spectators or disable anonymous spectating (`/block`, `[anon]`, `[all]`) | PH L228-292, L541-574 |

Implications:

- **Read-only by construction:** a spectator connection can't send keys, which matches the
  Advisor's never-send-input rule.
- Spectating is a second viable Game Source. A headless spectator client, such as a
  desktop app opening its own websocket to the server, would get the same data without a
  browser extension. The costs: it shows up publicly in the spectator list, it can be
  blocked, it needs the server's websocket URL and its own inflater, and it has the same
  no-history-on-join limit. Server operators may also regard automated spectator bots as
  unwelcome. That is a policy question, not a protocol one.

## 4. Stability across versions

- The **server** is version-independent: the README says its "code should work with any
  version of DCSS that supports webtiles"
  ([webserver/README.md L9](https://github.com/crawl/crawl/blob/28a137decf34033cc50b797d3677e82da05aba5d/crawl-ref/source/webserver/README.md)).
  Public servers run one trunk server for many game versions.
- The **game client JS is per version.** The process reports `client_path` and `version`.
  The server sends a `game_client` message with that version's `game.html` and serves
  that version's `game_data/static/*.js`
  ([PH L602-614](https://github.com/crawl/crawl/blob/28a137decf34033cc50b797d3677e82da05aba5d/crawl-ref/source/webserver/webtiles/process_handler.py#L602-L614)).
  So the protocol payload is whatever that version's C++ and JS agree on. There is no
  versioned schema.
- **Measured key drift:** comparing the JSON key strings written by `tileweb.cc`:
  - 0.34.1 to trunk: added `invis_mon_desc`, `offhand_weapon_colour`,
    `species_display_name`, `weapon_colour`; removed `unarmed_attack_colour`.
  - 0.25.0 to trunk: about 30 keys added (e.g. `ac_mod`/`ev_mod`/`sh_mod`, `doom`,
    `contam`, `offhand_*`, `quiver_desc`, `useless`) and removed (`str_max`,
    `int_max`, `dex_max`, `equip`).
  - The framing and the core message types (`player`, `map`, `msgs`, `menu`) are
    unchanged across this range.
- **Numeric enums are the bigger risk.** `f` (dungeon_feature_type), monster `type`, and
  item `base_type`/`sub_type` are raw C++ enum values. These are renumbered between
  versions, and trunk can renumber them at any commit. Monster and item `name` strings
  are sent alongside and are more robust. Features have glyphs and tiles but no name.
  The Advisor should decode with a table generated from the **exact** game version
  (read from the `version` message, or the lobby `game_id`), and fall back to names
  and glyphs.

## 5. Capturing from a browser extension

### Hook points in the page (most robust first)

1. **Wrap `comm.handle_message`** (MAIN-world content script). `client.js` calls
   `comm.handle_message(msg)` through the module object for every queued message, both
   lobby and game (CJ L42-52). `comm` is a require.js module (`define([...])`, CM L1),
   loaded by `require.js` (`templates/client.html` L11). A MAIN-world script can do
   `require(["comm"], c => { const orig = c.handle_message; c.handle_message = m => { forward(structuredClone(m)); return orig(m); }; })`.
   Results:
   - messages arrive already inflated, UTF-8 decoded, parsed and un-batched;
   - the hook doesn't depend on compression;
   - `multi` wrappers show up as well as their children, so skip `multi`;
   - `ping` and `close` bypass it as immediate handlers, which doesn't matter.

   Risk: this relies on the page's module layout (`comm`, `handle_message`). That layout
   is in the version-independent `static/scripts`, not `game_data`, so it changes rarely.
   It has the same shape at 0.25-era tags.
2. **Wrap `window.WebSocket`** at `run_at: document_start` and add a `message` listener.
   You must run your own inflater over the **whole** connection, because the zlib stream
   is stateful from the first frame. That means appending `00 00 FF FF` per frame, as
   CJ L1721-1725 does. You could use `DecompressionStream("deflate-raw")` or a bundled
   inflate library. It also catches lobby traffic. It is more fragile if injection is
   late, and is only needed if the require.js hook breaks.
3. Don't set the `no-compression` cookie to simplify capture. It changes the server's
   behaviour for the user's session, and the Advisor should be invisible.

Chrome docs: `world: "MAIN"` "will share the execution environment with the host
page's JavaScript", and `document_start` injects before the DOM loads
([content_scripts manifest](https://developer.chrome.com/docs/extensions/reference/manifest/content-scripts)).
A MAIN-world script has no `chrome.runtime`. It has to `window.postMessage` to an
ISOLATED-world content script, which relays to the service worker.

### Getting data to the desktop app

| Channel | Notes | Source |
|---|---|---|
| Localhost websocket from the extension service worker | MV3 service workers support WebSockets "starting with Chrome 116". Keep the worker alive by exchanging messages within the 30 s activity window, e.g. a 20 s ping. Simple, cross-platform. The desktop app must listen on a port and should authenticate the extension, e.g. with a shared token and an Origin check, because any local process or web page could connect. | [Chrome: WebSockets in service workers](https://developer.chrome.com/docs/extensions/how-to/web-platform/websockets) |
| Native messaging | Chrome launches the host over stdio with JSON frames, each preceded by a 32-bit length in native byte order. The limit is 1 MB per host-to-Chrome message and 64 MiB Chrome-to-host, so capture-to-app traffic is fine. Needs a host manifest installed per browser: a registry key on Windows, or `~/Library/Application Support/Google/Chrome/NativeMessagingHosts/` on macOS, etc. Chrome **spawns** the host process, which fits poorly with a long-running desktop app unless the host is a small relay. | [Chrome: native messaging](https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging) |

Full-map messages can exceed 100 kB (PH L185 comment: "map bundles can be very large
(100k+)"), which is well within both channels.

### Local webtiles

Local webtiles runs the same `server.py` with `dgl_mode = False`
(`DGLLessCrawlProcessHandler`, PH L1215-1232; default `bind_port = 8080` in
`webserver/config.py`). It uses the same `static/scripts` and per-version `game_data`. The
same content script works if its `matches` include `http://localhost:*/*` and
`http://127.0.0.1:*/*`. Webtiles is Linux, macOS or WSL only, not native Windows
(README L33). Local **SDL tiles and console** builds don't speak this protocol at all,
so they would need a different Game Source.

## 6. Backfilling Accumulated State on a mid-game join

What a late joiner gets for free:
- the forced full `player` and full current-level `map`, including remembered cells;
- any open menu or UI stack.

That is a complete current Visible State. Even for the player's own page, an
extension loaded mid-session can trigger the same thing by reloading the page. A reload
reconnects and replays `_send_everything` as a watcher-style join for the player, but
this needs verification (see Open risks).

What it can't get from the stream: other levels' maps, items seen elsewhere, uniques met,
skills, spells, and message history.

Backfill options:
1. **Character dump.** The `#` command (or save or death) writes a dump. When the server
   has `morgue_url` set, a `dump` message with a public URL goes to all receivers
   (TW L766-775, PH L1180-1187; e.g. CAO's `http://crawl.akrasiac.org/rawdata/%n/`,
   config.py L200). The dump has the dungeon overview (levels visited, altars, shops,
   portals), skills, spells, mutations, inventory, notes (uniques met, items found) and
   recent messages. That covers most of the Accumulated State. The player has to press
   `#`, which is allowed under the Advisor rule, or the Advisor can fetch the last
   dump on load if the server's URL pattern is known.
2. **Overview, skills and spells menus.** When the player opens `^O`, `m`, `I` or `A`, the
   full contents arrive as `menu`/`ui-push` messages and can be parsed opportunistically.
3. **Lobby / `where` info.** This gives coarse place, XL and turn for any game
   (`lobby_entry`, PH L694-710), plus `milestone` events (TW L788-795). It is useful for
   cross-checking, but not for backfill.

## Open risks / unverified

- **Reload replay for the player:** for the player's own connection, it is unverified
  whether a page reload mid-game re-attaches with a full resync or restarts the session
  (`attach`/`primary` handling, TW L455-461). This needs a live test.
- `DecompressionStream("deflate-raw")` streaming across frames with sync-flush trailers
  hasn't been tested. It only matters for hook option 2.
- Per-server `morgue_url` patterns and whether dumps are public vary by server (CAO, CBRO,
  CDO, etc.). There is no protocol-level guarantee.
- Server policy and etiquette for automated spectators and for extensions reading game
  state is not covered by code. Check with server operators and the community rules.
- Monster details beyond the per-cell summary (HP, spells, resistances) aren't in the
  stream. Tactical Advice has to combine `type` with version-matched Game Knowledge, or
  parse `describe-monster` popups when the player opens them.

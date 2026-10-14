# Standalone app with a relay-only browser extension

The Advisor is a standalone Tauri app with a Rust core, docked beside the browser. A browser extension captures webtiles traffic and relays raw messages to it over a localhost websocket. We chose this over showing the Advisor in the extension's side panel, because only a standalone app can serve Game Sources that have no browser (console, local tiles), run local models, and keep API keys out of web contexts. We kept the extension relay-only, rather than having it decode game state, so all version-specific decoding sits behind the Game Source seam next to Game Knowledge, and the raw relay stream doubles as a recording format for the dev harness.

## Considered Options

- **Extension side panel, no app:** one install and no transport to build, but it would only ever support webtiles.
- **Thick extension that sends a normalized Visible State:** would split decoding between the extension and the app, mean more extension updates and store reviews, and leave nothing raw to replay.
- **Native messaging instead of a localhost websocket:** secure by default, but each browser needs its own host manifest and the browser launches the host process, so it would need a bridge binary. We use a localhost websocket that rejects web page origins and has the user approve each browser on first connect.

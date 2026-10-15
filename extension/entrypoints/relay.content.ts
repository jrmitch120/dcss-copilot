// Bridges the page script, which can't reach extension APIs, to the background.

import {
  type ExtensionMessage,
  FROM_EXTENSION,
  FROM_PAGE,
  type PageMessage,
  RELAY_PORT_NAME,
} from "../lib/bridge";
import { WEBTILES_MATCHES } from "../lib/servers";

/** How long to wait before reconnecting to a background that went away. */
const RECONNECT_MS = 1_000;

export default defineContentScript({
  matches: WEBTILES_MATCHES,
  runAt: "document_start",
  async main() {
    let port = connect();

    function connect() {
      const opened = browser.runtime.connect({ name: RELAY_PORT_NAME });
      opened.onMessage.addListener((message: { type?: string }) => {
        if (message.type !== "replay") return;
        const request: ExtensionMessage = { source: FROM_EXTENSION, type: "replay" };
        window.postMessage(request, window.location.origin);
      });
      // A service worker that went idle drops its ports; a new one asks this tab to replay.
      opened.onDisconnect.addListener(() => {
        setTimeout(() => {
          port = connect();
        }, RECONNECT_MS);
      });
      return opened;
    }

    window.addEventListener("message", (event: MessageEvent<Partial<PageMessage>>) => {
      if (event.source !== window || event.data?.source !== FROM_PAGE) return;
      port.postMessage(event.data.report);
    });

    await injectScript("/relay-page.js", { keepInDom: true });
  },
});

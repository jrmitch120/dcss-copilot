// Injected into the webtiles page (MAIN world) by the relay content script.

import { type ExtensionMessage, FROM_EXTENSION, FROM_PAGE, type PageMessage } from "../lib/bridge";
import { type Comm, wrapComm } from "../lib/page-relay";

type Require = (modules: string[], loaded: (comm: Comm) => void) => void;

/** How often to look for the page's require.js before it has loaded. */
const REQUIRE_POLL_MS = 20;

export default defineUnlistedScript(() => {
  waitForRequire((require) => {
    require(["comm"], (comm) => {
      const relay = wrapComm(comm, {
        origin: window.location.origin,
        now: Date.now,
        report: (report) => {
          const message: PageMessage = { source: FROM_PAGE, report };
          window.postMessage(message, window.location.origin);
        },
      });
      window.addEventListener("message", (event: MessageEvent<Partial<ExtensionMessage>>) => {
        if (event.source !== window || event.data?.source !== FROM_EXTENSION) return;
        if (event.data.type === "replay") relay.replay();
      });
    });
  });
});

function waitForRequire(found: (require: Require) => void): void {
  const require = (window as unknown as { require?: Require }).require;
  if (typeof require === "function") {
    found(require);
  } else {
    setTimeout(() => waitForRequire(found), REQUIRE_POLL_MS);
  }
}

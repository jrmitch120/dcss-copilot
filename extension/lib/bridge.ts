// How the page script and the isolated content script talk, over `window.postMessage`.

import type { PageReport } from "./protocol";

/** Tags the page's reports, so the bridge ignores the page's own window messages. */
export const FROM_PAGE = "dcss-copilot/page";
/** Tags the bridge's replay requests. */
export const FROM_EXTENSION = "dcss-copilot/extension";

export interface PageMessage {
  source: typeof FROM_PAGE;
  report: PageReport;
}

export interface ExtensionMessage {
  source: typeof FROM_EXTENSION;
  type: "replay";
}

/** The port name the content script connects to the background with. */
export const RELAY_PORT_NAME = "relay";

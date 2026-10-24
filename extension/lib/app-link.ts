// Runs in the extension's background: the one connection to the app, fed by every webtiles tab.

import {
  type AppFrame,
  type ExtensionFrame,
  type PageEnvelope,
  type PageReport,
  type PanelCommand,
  type PanelFrame,
  RELAY_PROTOCOL_VERSION,
  type Refusal,
} from "./protocol";

/** The part of `WebSocket` the link uses. */
export interface Socket {
  send(data: string): void;
  close(): void;
  onopen: (() => void) | null;
  onmessage: ((event: { data: unknown }) => void) | null;
  onclose: (() => void) | null;
}

/** Where this browser stands with the app: what a side panel tells the player until it's connected. */
export type LinkStatus = "no-app" | "awaiting-approval" | Refusal | "connected";

export interface AppLink {
  /** Connect to the app, unless already connected or connecting. */
  start(): void;
  /** Pass on what a tab's page reported. */
  report(tab: number, report: PageReport): void;
  /** Forget a tab that closed or navigated away. */
  tabClosed(tab: number): void;
  /** Whether any side panel is open: the app's window gives way to one opening. */
  sidePanel(open: boolean): void;
  /** Pass on a command from a side panel; its answer comes back as a `panel-command-done`. */
  panelCommand(id: number, command: PanelCommand): void;
  /** Ping the app, so an idle connection keeps the service worker alive. */
  keepAlive(): void;
}

/**
 * Links the tabs' pages to the app. Once the app welcomes this browser, every known tab replays its
 * buffer, and a tab's live envelopes are forwarded only after its replay, so nothing arrives twice or
 * out of order.
 */
export function createAppLink(options: {
  connect(): Socket;
  /** Ask a tab's page to report its buffer. */
  requestReplay(tab: number): void;
  /** Call `reconnect` after a pause. */
  retryLater(reconnect: () => void): void;
  /** Where this browser stands with the app, each time that changes. */
  status(status: LinkStatus): void;
  /** A frame from the app for the side panels and the Toolbar Badge. */
  toSidePanels(frame: PanelFrame): void;
}): AppLink {
  let socket: Socket | null = null;
  let status: LinkStatus = "no-app";
  /** Whether any side panel is open, told to each connection once it is welcomed. */
  let sidePanelOpen = false;
  /** Every tab with a webtiles page, and whether its replay has been forwarded on this connection. */
  const tabs = new Map<number, { replayed: boolean }>();

  function send(frame: ExtensionFrame): void {
    socket?.send(JSON.stringify(frame));
  }

  function forward(tab: number, envelope: PageEnvelope): void {
    send({ type: "envelope", envelope: { ...envelope, tab } });
  }

  function askForReplay(tab: number): void {
    tabs.set(tab, { replayed: false });
    options.requestReplay(tab);
  }

  function welcomed(): boolean {
    return status === "connected";
  }

  function changeStatus(next: LinkStatus): void {
    if (next === status) return;
    status = next;
    options.status(next);
  }

  function onFrame(frame: AppFrame): void {
    if (frame.type === "welcome") {
      changeStatus("connected");
      if (sidePanelOpen) send({ type: "side-panel-opened" });
      for (const tab of tabs.keys()) askForReplay(tab);
    } else if (frame.type === "awaiting-approval") {
      changeStatus("awaiting-approval");
    } else if (frame.type === "refused") {
      changeStatus(frame.reason);
      socket?.close();
    } else if (frame.type === "view" || frame.type === "panel-command-done") {
      options.toSidePanels(frame);
    } else {
      console.error("DCSS Copilot sent a frame this extension doesn't know", frame);
    }
  }

  function start(): void {
    if (socket) return;
    const opened = options.connect();
    socket = opened;
    opened.onopen = () => send({ type: "hello", protocol: RELAY_PROTOCOL_VERSION });
    opened.onmessage = (event) => {
      if (typeof event.data === "string") onFrame(JSON.parse(event.data) as AppFrame);
    };
    opened.onclose = () => {
      socket = null;
      // A refusal stands until the app answers otherwise on a later connection.
      if (status !== "browser-denied" && status !== "unsupported-protocol") changeStatus("no-app");
      options.retryLater(start);
    };
  }

  return {
    start,

    report(tab, report) {
      const known = tabs.get(tab);
      if (!known) {
        tabs.set(tab, { replayed: false });
        if (welcomed()) askForReplay(tab);
        return;
      }
      if (!welcomed()) return;
      if (report.kind === "replay") {
        if (report.overflowed) send({ type: "overflow", origin: report.origin, tab });
        for (const envelope of report.envelopes) forward(tab, envelope);
        known.replayed = true;
      } else if (known.replayed) {
        forward(tab, report.envelope);
      }
    },

    tabClosed(tab) {
      tabs.delete(tab);
    },

    sidePanel(open) {
      const opened = open && !sidePanelOpen;
      sidePanelOpen = open;
      if (opened && welcomed()) send({ type: "side-panel-opened" });
    },

    panelCommand(id, command) {
      if (welcomed()) send({ type: "panel-command", id, command });
    },

    keepAlive() {
      send({ type: "ping" });
    },
  };
}

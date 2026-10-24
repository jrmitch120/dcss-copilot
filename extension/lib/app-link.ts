// Runs in the extension's background: the one connection to the app, fed by every webtiles tab.

import {
  type AppFrame,
  type ExtensionFrame,
  type GameWindow,
  type PageEnvelope,
  type PageReport,
  RELAY_PROTOCOL_VERSION,
  type WindowBounds,
} from "./protocol";

/** The part of `WebSocket` the link uses. */
export interface Socket {
  send(data: string): void;
  close(): void;
  onopen: (() => void) | null;
  onmessage: ((event: { data: unknown }) => void) | null;
  onclose: (() => void) | null;
}

export interface AppLink {
  /** Connect to the app, unless already connected or connecting. */
  start(): void;
  /** Pass on what a tab's page reported. */
  report(tab: number, report: PageReport): void;
  /** Forget a tab that closed or navigated away. */
  tabClosed(tab: number): void;
  /** Tell the app where the game window is now, so it can dock beside it. */
  windowMoved(window: GameWindow): void;
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
  /** Move and resize the game window, as the app asks to make room for its panel. */
  resizeWindow(bounds: WindowBounds): void;
}): AppLink {
  let socket: Socket | null = null;
  let welcomed = false;
  /** Every tab with a webtiles page, and whether its replay has been forwarded on this connection. */
  const tabs = new Map<number, { replayed: boolean }>();
  /** Where the game window was last reported, told to each connection once it is welcomed. */
  let gameWindow: GameWindow | null = null;

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

  function onFrame(frame: AppFrame): void {
    if (frame.type === "welcome") {
      welcomed = true;
      if (gameWindow) send({ type: "window", ...gameWindow });
      for (const tab of tabs.keys()) askForReplay(tab);
    } else if (frame.type === "resize-window") {
      options.resizeWindow(frame.bounds);
    } else if (frame.type === "refused") {
      socket?.close();
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
      welcomed = false;
      options.retryLater(start);
    };
  }

  return {
    start,

    report(tab, report) {
      const known = tabs.get(tab);
      if (!known) {
        tabs.set(tab, { replayed: false });
        if (welcomed) askForReplay(tab);
        return;
      }
      if (!welcomed) return;
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

    windowMoved(window) {
      gameWindow = window;
      if (welcomed) send({ type: "window", ...window });
    },

    keepAlive() {
      send({ type: "ping" });
    },
  };
}

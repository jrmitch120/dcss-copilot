import { createAppLink, type Socket } from "../lib/app-link";
import { RELAY_PORT_NAME } from "../lib/bridge";
import { watchGameWindow } from "../lib/game-window";
import { APP_URL, type PageReport } from "../lib/protocol";

/** How long to wait before trying the app again. */
const RETRY_MS = 5_000;
/** Under the 30 s a service worker may sit idle. */
const KEEP_ALIVE_MS = 20_000;

export default defineBackground(() => {
  const ports = new Map<number, Browser.runtime.Port>();

  const link = createAppLink({
    connect: () => openSocket(APP_URL),
    requestReplay: (tab) => ports.get(tab)?.postMessage({ type: "replay" }),
    retryLater: (reconnect) => setTimeout(reconnect, RETRY_MS),
    resizeWindow: (bounds) => {
      const id = watch.following();
      // A window must be in its normal state to take bounds.
      if (id !== null) void browser.windows.update(id, { state: "normal", ...bounds });
    },
  });
  setInterval(() => link.keepAlive(), KEEP_ALIVE_MS);

  const boundsChanged = browser.windows.onBoundsChanged;
  const watch = watchGameWindow({
    read: (id) => browser.windows.get(id),
    onBoundsChanged: boundsChanged
      ? (listener) => boundsChanged.addListener((window) => window.id && listener(window.id))
      : null,
    every: (ms, poll) => setInterval(poll, ms),
    report: (window) => link.windowMoved(window),
  });

  /** Follow the game window whenever the player turns to a webtiles tab. */
  function turnedTo(tab: number, window: number): void {
    if (ports.has(tab)) watch.follow(window);
  }
  browser.tabs.onActivated.addListener(({ tabId, windowId }) => turnedTo(tabId, windowId));
  browser.windows.onFocusChanged.addListener(async (window) => {
    // Focus left the browser for another app; the game window stays where it was.
    if (window === browser.windows.WINDOW_ID_NONE) return;
    const [active] = await browser.tabs.query({ active: true, windowId: window }).catch(() => []);
    if (active?.id !== undefined) turnedTo(active.id, window);
  });

  browser.runtime.onConnect.addListener((port) => {
    const tab = port.sender?.tab;
    if (port.name !== RELAY_PORT_NAME || tab?.id === undefined) return;
    const id = tab.id;
    ports.set(id, port);
    port.onMessage.addListener((report: PageReport) => link.report(id, report));
    port.onDisconnect.addListener(() => {
      ports.delete(id);
      link.tabClosed(id);
    });
    if (tab.active || watch.following() === null) watch.follow(tab.windowId);
    link.start();
  });
});

function openSocket(url: string): Socket {
  const ws = new WebSocket(url);
  const socket: Socket = {
    send: (data) => ws.send(data),
    close: () => ws.close(),
    onopen: null,
    onmessage: null,
    onclose: null,
  };
  ws.onopen = () => socket.onopen?.();
  ws.onmessage = (event) => socket.onmessage?.({ data: event.data });
  ws.onclose = () => socket.onclose?.();
  return socket;
}

import { createAppLink, type Socket } from "../lib/app-link";
import { RELAY_PORT_NAME } from "../lib/bridge";
import { APP_URL, type PageReport } from "../lib/protocol";
import {
  type Badge,
  createSidePanels,
  type FromSidePanel,
  NO_BADGE,
  SIDE_PANEL_PORT_NAME,
  type SidePanelPort,
} from "../lib/side-panels";

/** How long to wait before trying the app again. */
const RETRY_MS = 5_000;
/** Under the 30 s a service worker may sit idle. */
const KEEP_ALIVE_MS = 20_000;

/** Firefox's sidebar, which the Chrome typings don't describe. */
interface FirefoxSidebar {
  toggle(): Promise<void>;
}

export default defineBackground(() => {
  const ports = new Map<number, Browser.runtime.Port>();
  // Firefox's MV2 build has only the older name for the toolbar button.
  const button = browser.action ?? browser.browserAction;

  const link = createAppLink({
    connect: () => openSocket(APP_URL),
    requestReplay: (tab) => ports.get(tab)?.postMessage({ type: "replay" }),
    retryLater: (reconnect) => setTimeout(reconnect, RETRY_MS),
    status: (status) => sidePanels.status(status),
    toSidePanels: (frame) => sidePanels.received(frame),
  });
  setInterval(() => link.keepAlive(), KEEP_ALIVE_MS);

  const sidePanels = createSidePanels({ link, badge: showBadge });
  // The browser keeps the last badge across service worker restarts, but not the view it showed.
  showBadge(NO_BADGE);

  function showBadge(badge: Badge): void {
    void button.setBadgeText({ text: badge.text });
    void button.setBadgeBackgroundColor({ color: badge.colour });
    void button.setTitle({ title: badge.title });
  }

  // The toolbar button opens the side panel: Chrome does it itself, Firefox on its click.
  void browser.sidePanel?.setPanelBehavior({ openPanelOnActionClick: true });
  const sidebar = (browser as unknown as { sidebarAction?: FirefoxSidebar }).sidebarAction;
  if (sidebar) button.onClicked.addListener(() => void sidebar.toggle());

  browser.runtime.onConnect.addListener((port) => {
    if (port.name === SIDE_PANEL_PORT_NAME) {
      // Only the extension's own side panel page may send the app commands.
      if (port.sender?.url?.startsWith(browser.runtime.getURL("/sidepanel.html"))) {
        sidePanelConnected(port);
      }
      return;
    }
    const tab = port.sender?.tab;
    if (port.name !== RELAY_PORT_NAME || tab?.id === undefined) return;
    const id = tab.id;
    ports.set(id, port);
    port.onMessage.addListener((report: PageReport) => link.report(id, report));
    port.onDisconnect.addListener(() => {
      ports.delete(id);
      link.tabClosed(id);
    });
    link.start();
  });

  function sidePanelConnected(port: Browser.runtime.Port): void {
    const panel: SidePanelPort = { post: (message) => port.postMessage(message) };
    port.onMessage.addListener((message: FromSidePanel) => sidePanels.asked(panel, message));
    port.onDisconnect.addListener(() => sidePanels.closed(panel));
    sidePanels.opened(panel);
    // A side panel wants to know if the app is there, game or not.
    link.start();
  }
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

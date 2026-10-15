import { createAppLink, type Socket } from "../lib/app-link";
import { RELAY_PORT_NAME } from "../lib/bridge";
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
  });
  setInterval(() => link.keepAlive(), KEEP_ALIVE_MS);

  browser.runtime.onConnect.addListener((port) => {
    const tab = port.sender?.tab?.id;
    if (port.name !== RELAY_PORT_NAME || tab === undefined) return;
    ports.set(tab, port);
    port.onMessage.addListener((report: PageReport) => link.report(tab, report));
    port.onDisconnect.addListener(() => {
      ports.delete(tab);
      link.tabClosed(tab);
    });
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

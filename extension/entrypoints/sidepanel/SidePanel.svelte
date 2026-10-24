<script lang="ts">
import "$lib/panel/theme.css";
import type { ViewModel } from "$lib/bindings";
import type { PanelActions } from "$lib/panel/actions";
import Panel from "$lib/panel/Panel.svelte";
import type { LinkStatus } from "../../lib/app-link";
import type { PanelCommand } from "../../lib/protocol";
import { type FromSidePanel, SIDE_PANEL_PORT_NAME, type ToSidePanel } from "../../lib/side-panels";

/** How long to wait before reaching the background again, after its service worker stopped. */
const RECONNECT_MS = 1_000;

const SAYS: Record<Exclude<LinkStatus, "connected">, string> = {
  "no-app": "DCSS Copilot isn't running. Start it, and the panel shows here.",
  "awaiting-approval":
    "Allow this browser in the DCSS Copilot app: its window is asking now. Can't see it? Open it from DCSS Copilot's icon in the menu bar or system tray.",
  "browser-denied": "DCSS Copilot was told not to allow this browser.",
  "unsupported-protocol":
    "This extension and the DCSS Copilot app are different versions. Update both to the latest.",
};

let status = $state<LinkStatus>("no-app");
let view = $state<ViewModel | null>(null);

/** Each command sent, by its id, waiting for the app's answer. */
const waiting = new Map<number, (error: string | null) => void>();
let nextId = 1;
let port: Browser.runtime.Port | null = null;

function connect(): void {
  const connected = browser.runtime.connect({ name: SIDE_PANEL_PORT_NAME });
  port = connected;
  connected.onMessage.addListener((message: ToSidePanel) => {
    if (message.type === "status") {
      status = message.status;
      if (message.status !== "connected") view = null;
    } else if (message.type === "view") {
      view = message.view;
    } else {
      waiting.get(message.id)?.(message.error);
      waiting.delete(message.id);
    }
  });
  connected.onDisconnect.addListener(() => {
    port = null;
    for (const answer of waiting.values()) answer("DCSS Copilot's extension restarted.");
    waiting.clear();
    setTimeout(connect, RECONNECT_MS);
  });
}

function ask(command: PanelCommand): Promise<string | null> {
  const id = nextId++;
  const message: FromSidePanel = { type: "command", id, command };
  return new Promise((answer) => {
    if (!port) {
      answer("DCSS Copilot's extension is restarting.");
      return;
    }
    waiting.set(id, answer);
    port.postMessage(message);
  });
}

const ACTIONS: PanelActions = {
  setAppearance: (appearance) => ask({ kind: "set-appearance", appearance }),
  openSettings: () => ask({ kind: "open-settings" }),
  answerBrowser: null,
};

connect();
</script>

{#if status === "connected" && view}
  <Panel {view} actions={ACTIONS} />
{:else if status !== "connected"}
  <p class="status">{SAYS[status]}</p>
{/if}

<style>
.status {
  margin: 0;
  padding: 16px 14px;
  font-size: 12.5px;
  color: var(--text-muted);
}
</style>

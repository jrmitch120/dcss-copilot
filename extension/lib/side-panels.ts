// Runs in the extension's background: the browser's open side panels, each showing the panel the
// app sends, and the toolbar button standing in for them while none is open.

import type { NowPill, ViewModel } from "$lib/bindings";
import { nowLabel, nowLine } from "$lib/panel/tone";
import type { AppLink, LinkStatus } from "./app-link";
import type { PanelCommand, PanelFrame } from "./protocol";

/** The port name a side panel connects to the background with. */
export const SIDE_PANEL_PORT_NAME = "side-panel";

/** What the background tells a side panel. */
export type ToSidePanel =
  | { type: "status"; status: LinkStatus }
  | { type: "view"; view: ViewModel }
  | { type: "done"; id: number; error: string | null };

/** What a side panel asks the background: a command, with an id of the panel's choosing. */
export interface FromSidePanel {
  type: "command";
  id: number;
  command: PanelCommand;
}

/** One open side panel. */
export interface SidePanelPort {
  post(message: ToSidePanel): void;
}

/** The Toolbar Badge: the panel's stand-in on the toolbar button while no Side Panel is open. */
export interface ToolbarBadge {
  /** The badge's text: empty for none. */
  text: string;
  colour: string;
  /** The button's tooltip. */
  title: string;
}

/** The button as it is with no game to stand in for. */
export const NO_TOOLBAR_BADGE: ToolbarBadge = {
  text: "",
  colour: "#000000",
  title: "DCSS Copilot",
};

/** theme.css's `--danger`, for a moment that calls for the panel. The background service worker
    has no stylesheet to read it from. */
const DANGER = "#ff3b5c";
/** theme.css's `--info`, for Decision Points waiting. */
const INFO = "#74b9ff";

export interface SidePanels {
  opened(port: SidePanelPort): void;
  closed(port: SidePanelPort): void;
  /** A side panel asked for `message`'s command. */
  asked(port: SidePanelPort, message: FromSidePanel): void;
  /** Where this browser stands with the app changed. */
  status(status: LinkStatus): void;
  /** The app sent the side panels a frame. */
  received(frame: PanelFrame): void;
}

/**
 * Keeps every open side panel showing the app's latest view, routes each panel's commands to the
 * app and their answers back, and badges the toolbar button while no side panel is open.
 */
export function createSidePanels(options: {
  link: Pick<AppLink, "sidePanel" | "panelCommand">;
  badge(badge: ToolbarBadge): void;
}): SidePanels {
  const open = new Set<SidePanelPort>();
  let status: LinkStatus = "no-app";
  /** The app's latest view, while connected. */
  let view: ViewModel | null = null;
  /** Each command sent to the app, by the id the app answers to, with its panel and the panel's id. */
  const waiting = new Map<number, { port: SidePanelPort; id: number }>();
  let nextId = 1;

  function rebadge(): void {
    options.badge(open.size === 0 && view ? toolbarBadgeFor(view) : NO_TOOLBAR_BADGE);
  }

  function broadcast(message: ToSidePanel): void {
    for (const port of open) port.post(message);
  }

  return {
    opened(port) {
      open.add(port);
      if (open.size === 1) options.link.sidePanel(true);
      port.post({ type: "status", status });
      if (view) port.post({ type: "view", view });
      rebadge();
    },

    closed(port) {
      open.delete(port);
      for (const [id, command] of waiting) if (command.port === port) waiting.delete(id);
      if (open.size === 0) options.link.sidePanel(false);
      rebadge();
    },

    asked(port, message) {
      if (status !== "connected") {
        port.post({ type: "done", id: message.id, error: "DCSS Copilot isn't connected." });
        return;
      }
      const id = nextId++;
      waiting.set(id, { port, id: message.id });
      options.link.panelCommand(id, message.command);
    },

    status(next) {
      status = next;
      if (next !== "connected") {
        view = null;
        for (const { port, id } of waiting.values()) {
          port.post({ type: "done", id, error: "DCSS Copilot disconnected before it answered." });
        }
        waiting.clear();
      }
      broadcast({ type: "status", status: next });
      rebadge();
    },

    received(frame) {
      if (frame.type === "view") {
        view = frame.view;
        broadcast({ type: "view", view: frame.view });
        rebadge();
        return;
      }
      const command = waiting.get(frame.id);
      waiting.delete(frame.id);
      command?.port.post({ type: "done", id: command.id, error: frame.error });
    },
  };
}

/** The Toolbar Badge for `view`: the threat in the tooltip, and a call for the panel or the
    Decision Points waiting on the badge. */
function toolbarBadgeFor(view: ViewModel): ToolbarBadge {
  const game = view.game;
  if (!game?.live) return NO_TOOLBAR_BADGE;
  const title = `DCSS Copilot: ${summary(game.now)}`;
  if (game.badge.callsForPanel) return { text: "!", colour: DANGER, title };
  const waiting = game.badge.decisionsWaiting;
  return { text: waiting > 0 ? String(waiting) : "", colour: INFO, title };
}

/** The now pill's label, any Alert and its line, as one tooltip. */
function summary(now: NowPill): string {
  // The Alert and the line are each a sentence of their own.
  return `${nowLabel(now)}. ${now.alert ? `${now.alert} ` : ""}${nowLine(now)}`;
}

import { describe, expect, it } from "vitest";
import type { GameView, ViewModel } from "$lib/bindings";
import type { PanelCommand } from "../lib/protocol";
import {
  type Badge,
  createSidePanels,
  NO_BADGE,
  type SidePanelPort,
  type ToSidePanel,
} from "../lib/side-panels";

class FakePanel implements SidePanelPort {
  received: ToSidePanel[] = [];

  post(message: ToSidePanel): void {
    this.received.push(message);
  }
}

function harness() {
  const sidePanelOpen: boolean[] = [];
  const commands: [number, PanelCommand][] = [];
  const badges: Badge[] = [];
  const sidePanels = createSidePanels({
    link: {
      sidePanel: (open) => sidePanelOpen.push(open),
      panelCommand: (id, command) => commands.push([id, command]),
    },
    badge: (badge) => badges.push(badge),
  });
  return { sidePanels, sidePanelOpen, commands, badges };
}

/** A view of a live game, at `now`, with `badge`. */
function gameView(now: GameView["now"], badge: Partial<GameView["badge"]> = {}): ViewModel {
  const game = {
    now,
    live: true,
    badge: { decisionsWaiting: 0, callsForPanel: false, ...badge },
  } as GameView;
  return { appVersion: "1.2.3", game } as ViewModel;
}

const DANGEROUS = gameView({
  state: "rated",
  level: "dangerous",
  line: "The ogre hits hard.",
  detail: null,
  alert: null,
});

describe("side panels", () => {
  it("shows every open side panel the app's latest view, and tells the link whether any is open", () => {
    const { sidePanels, sidePanelOpen } = harness();
    sidePanels.status("connected");
    sidePanels.received({ type: "view", view: DANGEROUS });
    const first = new FakePanel();
    const second = new FakePanel();

    sidePanels.opened(first);
    sidePanels.opened(second);
    sidePanels.closed(first);
    sidePanels.closed(second);

    expect(first.received).toEqual([
      { type: "status", status: "connected" },
      { type: "view", view: DANGEROUS },
    ]);
    expect(second.received).toEqual(first.received);
    expect(sidePanelOpen).toEqual([true, false]);
  });

  it("answers each command to the side panel that sent it, under its own id", () => {
    const { sidePanels, commands } = harness();
    sidePanels.status("connected");
    const first = new FakePanel();
    const second = new FakePanel();
    sidePanels.opened(first);
    sidePanels.opened(second);

    sidePanels.asked(first, { type: "command", id: 1, command: { kind: "open-settings" } });
    sidePanels.asked(second, { type: "command", id: 1, command: { kind: "open-settings" } });
    sidePanels.received({ type: "panel-command-done", id: 2, error: "No window." });

    expect(commands.map(([id]) => id)).toEqual([1, 2]);
    expect(first.received.filter((m) => m.type === "done")).toEqual([]);
    expect(second.received.filter((m) => m.type === "done")).toEqual([
      { type: "done", id: 1, error: "No window." },
    ]);
  });

  it("answers commands with an error when the app isn't there to do them", () => {
    const { sidePanels, commands } = harness();
    const panel = new FakePanel();
    sidePanels.opened(panel);
    sidePanels.asked(panel, { type: "command", id: 1, command: { kind: "open-settings" } });
    sidePanels.status("connected");
    sidePanels.asked(panel, { type: "command", id: 2, command: { kind: "open-settings" } });
    sidePanels.status("no-app");

    expect(commands).toHaveLength(1);
    expect(panel.received.filter((m) => m.type === "done")).toEqual([
      { type: "done", id: 1, error: "DCSS Copilot isn't connected." },
      { type: "done", id: 2, error: "DCSS Copilot disconnected before it answered." },
    ]);
  });
});

describe("toolbar badge", () => {
  it("stands in for the panel only while no side panel is open", () => {
    const { sidePanels, badges } = harness();
    sidePanels.status("connected");
    sidePanels.received({ type: "view", view: DANGEROUS });
    sidePanels.opened(new FakePanel());

    const standingIn: Badge = {
      text: "",
      colour: "#74b9ff",
      title: "DCSS Copilot: DANGEROUS. The ogre hits hard.",
    };
    expect(badges.slice(-2)).toEqual([standingIn, NO_BADGE]);
  });

  it("calls for the panel only at a LETHAL moment with no trusted Way Out, and counts Decision Points otherwise", () => {
    const { sidePanels, badges } = harness();
    sidePanels.status("connected");
    const lethal = {
      state: "rated",
      level: "lethal",
      line: "Three jackals flank you.",
      detail: null,
      alert: null,
    } as const;

    sidePanels.received({ type: "view", view: gameView(lethal, { callsForPanel: true }) });
    sidePanels.received({ type: "view", view: gameView(lethal, { decisionsWaiting: 2 }) });
    sidePanels.status("no-app");

    expect(badges.slice(-3)).toEqual([
      { text: "!", colour: "#ff3b5c", title: "DCSS Copilot: LETHAL. Three jackals flank you." },
      { text: "2", colour: "#74b9ff", title: "DCSS Copilot: LETHAL. Three jackals flank you." },
      NO_BADGE,
    ]);
  });

  it("goes when the game stops being live", () => {
    const { sidePanels, badges } = harness();
    sidePanels.status("connected");
    const ended = gameView(DANGEROUS.game?.now as GameView["now"], { callsForPanel: true });
    if (ended.game) ended.game.live = false;

    sidePanels.received({ type: "view", view: ended });

    expect(badges.at(-1)).toEqual(NO_BADGE);
  });
});

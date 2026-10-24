import { describe, expect, it } from "vitest";
import { type BrowserWindowInfo, watchGameWindow } from "../lib/game-window";
import type { GameWindow } from "../lib/protocol";

/** The browser's windows, by id, as its `windows` API would give them. */
function browserHarness(options: { boundsEvent: boolean }) {
  const windows = new Map<number, BrowserWindowInfo>([
    [1, { left: 0, top: 25, width: 1920, height: 1055, state: "maximized" }],
    [2, { left: -1440, top: 0, width: 1200, height: 900, state: "normal" }],
  ]);
  const reported: GameWindow[] = [];
  let boundsChanged: ((id: number) => void) | null = null;
  let tick: (() => void) | null = null;

  const watch = watchGameWindow({
    read: async (id) => windows.get(id),
    onBoundsChanged: options.boundsEvent
      ? (listener) => {
          boundsChanged = listener;
        }
      : null,
    every: (_ms, poll) => {
      tick = poll;
    },
    report: (window) => reported.push(window),
  });

  return {
    watch,
    reported,
    /** The player moves or resizes window `id`. */
    async move(id: number, change: Partial<BrowserWindowInfo>) {
      windows.set(id, { ...windows.get(id), ...change } as BrowserWindowInfo);
      boundsChanged?.(id);
      await settle();
    },
    /** Firefox's polling interval passes. */
    async poll() {
      tick?.();
      await settle();
    },
  };
}

/** Lets the watch's reads of the windows API resolve. */
async function settle(): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, 0));
}

describe("game window", () => {
  it("reports the window a game's tab is in, and each move of it Chrome tells of", async () => {
    const browser = browserHarness({ boundsEvent: true });

    browser.watch.follow(1);
    await settle();
    await browser.move(2, { left: -1400 });
    await browser.move(1, { state: "normal", width: 1500 });

    expect(browser.reported).toEqual([
      { bounds: { left: 0, top: 25, width: 1920, height: 1055 }, state: "maximized" },
      { bounds: { left: 0, top: 25, width: 1500, height: 1055 }, state: "normal" },
    ]);
  });

  it("polls the game window where the browser has no bounds event, as Firefox has none", async () => {
    const browser = browserHarness({ boundsEvent: false });

    browser.watch.follow(2);
    await settle();
    await browser.poll();
    await browser.move(2, { top: 40 });
    await browser.poll();

    expect(browser.reported).toEqual([
      { bounds: { left: -1440, top: 0, width: 1200, height: 900 }, state: "normal" },
      { bounds: { left: -1440, top: 40, width: 1200, height: 900 }, state: "normal" },
    ]);
  });

  it("follows the window of the game tab the player turned to last", async () => {
    const browser = browserHarness({ boundsEvent: true });

    browser.watch.follow(1);
    await settle();
    browser.watch.follow(2);
    await settle();
    browser.watch.follow(2);
    await settle();

    expect(browser.reported.map((window) => window.bounds.left)).toEqual([0, -1440]);
    expect(browser.watch.following()).toBe(2);
  });

  it("names a state the app doesn't know by the nearest it does", async () => {
    const browser = browserHarness({ boundsEvent: true });

    browser.watch.follow(1);
    await settle();
    await browser.move(1, { state: "locked-fullscreen" });

    expect(browser.reported.at(-1)?.state).toBe("fullscreen");
  });
});

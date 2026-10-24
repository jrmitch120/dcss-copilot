// Watches the browser window holding the game, so the app can dock its panel beside it.

import type { GameWindow, WindowState } from "./protocol";

/** How often to read the game window's bounds where the browser has no event for them. */
const POLL_MS = 1_000;

/** The part of a `windows.Window` the watch reads. */
export interface BrowserWindowInfo {
  left?: number;
  top?: number;
  width?: number;
  height?: number;
  state?: string;
}

export interface GameWindowWatch {
  /** Follow window `id`, the one holding the game tab the player turned to last. */
  follow(id: number): void;
  /** The window followed, if any. */
  following(): number | null;
}

/**
 * Reports where the game window is each time it moves, resizes or another takes its place. Chrome
 * tells of each move once it ends; Firefox has no such event, so its window is polled.
 */
export function watchGameWindow(options: {
  /** Read window `id`, or nothing if it has closed. */
  read(id: number): Promise<BrowserWindowInfo | undefined>;
  /** Call `listener` with the id of each window whose bounds changed, or null if the browser can't. */
  onBoundsChanged: ((listener: (id: number) => void) => void) | null;
  /** Call `poll` every `ms` milliseconds. */
  every(ms: number, poll: () => void): void;
  report(window: GameWindow): void;
}): GameWindowWatch {
  let followed: number | null = null;
  let last: string | null = null;

  async function check(id: number): Promise<void> {
    const info = await options.read(id).catch(() => undefined);
    if (id !== followed || !info) return;
    const window = gameWindow(info);
    if (!window) return;
    const key = JSON.stringify(window);
    if (key === last) return;
    last = key;
    options.report(window);
  }

  if (options.onBoundsChanged) {
    options.onBoundsChanged((id) => {
      if (id === followed) void check(id);
    });
  } else {
    options.every(POLL_MS, () => {
      if (followed !== null) void check(followed);
    });
  }

  return {
    follow(id) {
      if (id === followed) return;
      followed = id;
      last = null;
      void check(id);
    },
    following: () => followed,
  };
}

/** `info` as the app reads it, or null if the browser left out its bounds. */
function gameWindow(info: BrowserWindowInfo): GameWindow | null {
  const { left, top, width, height } = info;
  if (left === undefined || top === undefined || width === undefined || height === undefined) {
    return null;
  }
  return { bounds: { left, top, width, height }, state: windowState(info.state) };
}

/** The app's name for a browser's window state, such as Chrome's kiosk `locked-fullscreen`. */
function windowState(state: string | undefined): WindowState {
  switch (state) {
    case "minimized":
    case "maximized":
    case "fullscreen":
      return state;
    case "locked-fullscreen":
      return "fullscreen";
    default:
      return "normal";
  }
}

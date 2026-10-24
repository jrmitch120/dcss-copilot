// The relay protocol between the extension and the app. See docs/relay-protocol.md.

/** The only compatibility contract with the app, which accepts this version and the previous one. */
export const RELAY_PROTOCOL_VERSION = 2;

/** Where the app listens. */
export const APP_URL = "ws://127.0.0.1:47821";

/** One webtiles message as the page received it, before the extension knows its tab. */
export interface PageEnvelope {
  origin: string;
  /** Unix milliseconds. */
  ts: number;
  /** The text of the latest webtiles `version` message, verbatim. */
  gameVersion: string | null;
  msg: unknown;
}

/** A webtiles message on its way to the app. */
export interface Envelope extends PageEnvelope {
  tab: number;
}

/** What a page tells the extension. */
export type PageReport =
  | { kind: "live"; envelope: PageEnvelope }
  | { kind: "replay"; origin: string; envelopes: PageEnvelope[]; overflowed: boolean };

/** A browser window's outer bounds, in screen pixels as the `windows` API gives them. */
export interface WindowBounds {
  left: number;
  top: number;
  width: number;
  height: number;
}

/** A browser window's state, as the app knows them. */
export type WindowState = "normal" | "minimized" | "maximized" | "fullscreen";

/** The browser window holding the game, where the app docks its panel. */
export interface GameWindow {
  bounds: WindowBounds;
  state: WindowState;
}

export type ExtensionFrame =
  | { type: "hello"; protocol: number }
  | { type: "envelope"; envelope: Envelope }
  | { type: "overflow"; origin: string; tab: number }
  | ({ type: "window" } & GameWindow)
  | { type: "ping" };

export type AppFrame =
  | { type: "welcome"; protocol: number }
  | { type: "awaiting-approval" }
  | { type: "resize-window"; bounds: WindowBounds }
  | { type: "refused"; reason: "unsupported-protocol" | "browser-denied" };

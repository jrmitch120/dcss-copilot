// The relay protocol between the extension and the app. See docs/relay-protocol.md.

/** The only compatibility contract with the app, which accepts this version and the previous one. */
export const RELAY_PROTOCOL_VERSION = 1;

/** Where the app listens. */
export const APP_URL = "ws://127.0.0.1:47821";

/** One webtiles message as the page received it, before the extension knows its tab. */
export interface Capture {
  origin: string;
  /** Unix milliseconds. */
  ts: number;
  /** The text of the latest webtiles `version` message, verbatim. */
  gameVersion: string | null;
  msg: unknown;
}

/** A captured message on its way to the app. */
export interface Envelope extends Capture {
  tab: number;
}

/** What a page tells the extension. */
export type PageReport =
  | { kind: "live"; capture: Capture }
  | { kind: "replay"; origin: string; captures: Capture[]; overflowed: boolean };

export type ExtensionFrame =
  | { type: "hello"; protocol: number }
  | { type: "envelope"; envelope: Envelope }
  | { type: "overflow"; origin: string; tab: number }
  | { type: "ping" };

export type AppFrame =
  | { type: "welcome"; protocol: number }
  | { type: "awaiting-approval" }
  | { type: "refused"; reason: "unsupported-protocol" | "browser-denied" };

// The relay protocol between the extension and the app. See docs/relay-protocol.md.

import type { Appearance, AssistanceLevel, ViewModel } from "$lib/bindings";

/** The only compatibility contract with the app, which accepts this version and the previous one. */
export const RELAY_PROTOCOL_VERSION = 4;

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

/** What the player can do from the side panel. Only the app answers "Allow this browser?". */
export type PanelCommand =
  | { kind: "set-appearance"; appearance: Appearance }
  | { kind: "set-assistance"; assistance: AssistanceLevel }
  | { kind: "answer-same-game"; same: boolean }
  | { kind: "close-join-card" }
  | { kind: "open-settings" };

export type ExtensionFrame =
  | { type: "hello"; protocol: number }
  | { type: "envelope"; envelope: Envelope }
  | { type: "overflow"; origin: string; tab: number }
  | { type: "side-panel-opened" }
  | { type: "panel-command"; id: number; command: PanelCommand }
  | { type: "ping" };

export type AppFrame =
  | { type: "welcome"; protocol: number }
  | { type: "awaiting-approval" }
  | { type: "view"; view: ViewModel }
  | { type: "panel-command-done"; id: number; error: string | null }
  | { type: "refused"; reason: Refusal };

/** Why the app refused this browser. */
export type Refusal = "unsupported-protocol" | "browser-denied";

/** The frames the app sends for the side panels and the Toolbar Badge. */
export type PanelFrame = Extract<AppFrame, { type: "view" | "panel-command-done" }>;

import type { NowPill, ThreatLevel } from "$lib/bindings";

/** The colour a moment is drawn in: its Threat Level's, or a neutral grey while there is none. */
export function threatTone(now: NowPill): ThreatLevel | "unrated" {
  return now.state === "rated" ? now.level : "unrated";
}

/** The now pill's label: the Threat Level, or UNRATED while there is none. */
export function nowLabel(now: NowPill): string {
  return now.state === "rated" ? now.level.toUpperCase() : "UNRATED";
}

/** The now pill's one line about this moment, after any Alert. */
export function nowLine(now: NowPill): string {
  return now.state === "rated" ? now.line : "The Advisor can't judge danger yet.";
}

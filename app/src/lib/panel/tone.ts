import type { NowPill, ThreatLevel } from "$lib/bindings";

/** The colour a moment is drawn in: its Threat Level's, or a neutral grey while there is none. */
export function threatTone(now: NowPill): ThreatLevel | "unrated" {
  return now.state === "rated" ? now.level : "unrated";
}

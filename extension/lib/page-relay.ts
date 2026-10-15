// Runs in the webtiles page itself (MAIN world): captures every message the page handles.

import type { Capture, PageReport } from "./protocol";

/** About 25 MB of messages: enough for a long session, small enough to sit in a tab. */
export const PAGE_BUFFER_LIMIT_BYTES = 25 * 1024 * 1024;

/** The page's require.js `comm` module: the one function every decoded webtiles message passes through. */
export interface Comm {
  handle_message(msg: unknown): unknown;
}

export interface PageRelay {
  /** Report every buffered capture, so a newly connected app sees the game from the start. */
  replay(): void;
}

/**
 * Wraps `comm.handle_message` so each message is reported unchanged, then handled by the page as before.
 */
export function wrapComm(
  comm: Comm,
  options: {
    origin: string;
    now(): number;
    report(report: PageReport): void;
    bufferLimitBytes?: number | undefined;
  },
): PageRelay {
  const limit = options.bufferLimitBytes ?? PAGE_BUFFER_LIMIT_BYTES;
  let buffer: Capture[] = [];
  let bufferedBytes = 0;
  let overflowed = false;
  let gameVersion: string | null = null;

  const handle = comm.handle_message.bind(comm);
  comm.handle_message = (msg: unknown) => {
    try {
      capture(msg);
    } catch (error) {
      // The relay must never break the game page.
      console.error("DCSS Copilot could not capture a message", error);
    }
    return handle(msg);
  };

  function capture(msg: unknown): void {
    const type = (msg as { msg?: unknown } | null)?.msg;
    // A `multi` message's children pass through handle_message on their own.
    if (type === "multi") return;
    if (type === "version") gameVersion = String((msg as { text?: unknown }).text ?? "");

    const captured: Capture = { origin: options.origin, ts: options.now(), gameVersion, msg };
    if (!overflowed) {
      bufferedBytes += JSON.stringify(msg).length;
      if (bufferedBytes > limit) {
        overflowed = true;
        buffer = [];
      } else {
        buffer.push(captured);
      }
    }
    options.report({ kind: "live", capture: captured });
  }

  return {
    replay() {
      options.report({ kind: "replay", origin: options.origin, captures: [...buffer], overflowed });
    },
  };
}

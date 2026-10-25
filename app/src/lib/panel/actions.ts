import type { Appearance, AssistanceLevel } from "$lib/bindings";

/**
 * What the panel's buttons do, wherever it shows: the app's window calls the app's commands, a
 * side panel asks the app through the extension. Each resolves to why it failed, or `null`.
 */
export interface PanelActions {
  setAppearance(appearance: Appearance): Promise<string | null>;
  setAssistance(assistance: AssistanceLevel): Promise<string | null>;
  /** Answers "Is this the same game?", of a game picked up again on a later turn. */
  answerSameGame(same: boolean): Promise<string | null>;
  /** Puts the join card away: "Not now" while it asks for the dump, "OK" once it has read it. */
  closeJoinCard(): Promise<string | null>;
  openSettings(): Promise<string | null>;
  /**
   * Answers "Allow this browser?", or `null` in a side panel: a browser could otherwise allow
   * itself.
   */
  answerBrowser: ((allow: boolean) => Promise<string | null>) | null;
}

/** What the panel says when the player's choice couldn't be saved, or `null` when it was. */
export function unsaved(error: string | null): string | null {
  return error === null ? null : `This choice lasts only until DCSS Copilot closes: ${error}`;
}

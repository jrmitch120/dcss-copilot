import type { Appearance, Assistance } from "$lib/bindings";

/**
 * What the panel's buttons do, wherever it shows: the app's window calls the app's commands, a
 * side panel asks the app through the extension. Each resolves to why it failed, or `null`.
 */
export interface PanelActions {
  setAppearance(appearance: Appearance): Promise<string | null>;
  setAssistance(assistance: Assistance): Promise<string | null>;
  openSettings(): Promise<string | null>;
  /**
   * Answers "Allow this browser?", or `null` in a side panel: a browser could otherwise allow
   * itself.
   */
  answerBrowser: ((allow: boolean) => Promise<string | null>) | null;
}

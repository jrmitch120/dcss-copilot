/** What a command sent back: its data, or why it failed. */
type CommandResult<T, E> = { status: "ok"; data: T } | { status: "error"; error: E };

/** Why the command failed, or `null` if it succeeded. */
export function failure<T, E>(result: CommandResult<T, E>): E | null {
  return result.status === "error" ? result.error : null;
}

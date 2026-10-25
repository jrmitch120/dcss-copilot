<script lang="ts">
import { commands, type HistoryView } from "$lib/bindings";
import { failure } from "$lib/command";

let { history }: { history: HistoryView } = $props();

let error = $state<string | null>(null);
let confirming = $state(false);

async function forgetOldGames(on: boolean) {
  error = failure(await commands.setForgetOldGames(on));
}

async function clear() {
  confirming = false;
  const failed = failure(await commands.clearHistory());
  error = failed === null ? null : `The history couldn't be cleared: ${failed}`;
}

function remembered(games: number): string {
  return games === 1 ? "1 game remembered" : `${games} games remembered`;
}
</script>

<p class="small muted">
  DCSS Copilot remembers each game you play, on this computer only, so it picks up where you left
  off. Games you watch are never saved.
</p>
<div class="rows">
  <label class="row">
    <span>
      <b>Forget games not played for {history.keepDays} days</b>
      <small>Checked each time your browser connects.</small>
    </span>
    <input
      type="checkbox"
      checked={history.forgetAfterDays !== null}
      onchange={(e) => forgetOldGames(e.currentTarget.checked)}
    >
  </label>
  <div class="row">
    <span>
      <b>Clear history</b>
      <small>
        {remembered(history.games)}. Clearing forgets every one but the game you're playing.
      </small>
    </span>
    {#if confirming}
      <button type="button" class="btn danger" onclick={clear}>Forget them</button>
      <button type="button" class="btn link" onclick={() => (confirming = false)}>Keep</button>
    {:else}
      <button type="button" class="btn" onclick={() => (confirming = true)}>Clear history…</button>
    {/if}
  </div>
</div>
{#if error}
  <p class="error small">{error}</p>
{/if}

<style>
p {
  margin: 0 0 8px;
}

.rows {
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--surface-low);
}

.row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 12px;
  font-size: 12.5px;
}

.row + .row {
  border-top: 1px solid var(--divider);
}

.row > span {
  flex: 1;
}

.row small {
  display: block;
  font-size: 11.5px;
  color: var(--text-muted);
}

.btn {
  padding: 5px 11px;
  border: 0;
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.07);
  font-size: 12px;
  color: var(--text-soft);
  white-space: nowrap;
  cursor: pointer;
}

.btn:hover {
  background: rgba(255, 255, 255, 0.12);
}

.btn.danger {
  background: rgba(255, 59, 92, 0.1);
  color: #ff8da1;
}

.btn.link {
  background: none;
  color: var(--text-muted);
}

.error {
  margin: 8px 0 0;
  color: var(--error);
}
</style>

<script lang="ts">
import { commands, type WindowView } from "$lib/bindings";
import { failure } from "$lib/command";

let { window }: { window: WindowView } = $props();

let error = $state<string | null>(null);

async function startAtLogin(on: boolean) {
  error = failure(await commands.setStartAtLogin(on));
}
</script>

<p class="small muted">
  The panel shows in your browser's side panel: open it from the DCSS Copilot button in the toolbar.
</p>
<div class="rows">
  <label class="row">
    <span>
      <b>Start DCSS Copilot when I log in</b>
      <small>It waits in the tray, ready for your browser.</small>
    </span>
    <input
      type="checkbox"
      checked={window.startAtLogin}
      onchange={(e) => startAtLogin(e.currentTarget.checked)}
    >
  </label>
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

.row > span {
  flex: 1;
}

.row small {
  display: block;
  font-size: 11.5px;
  color: var(--text-muted);
}

.error {
  margin: 8px 0 0;
  color: var(--error);
}
</style>

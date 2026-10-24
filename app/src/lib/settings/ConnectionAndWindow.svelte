<script lang="ts">
import { commands, type DockSide, type WindowView } from "$lib/bindings";
import { failure } from "$lib/command";

let { window }: { window: WindowView } = $props();

let error = $state<string | null>(null);

const SIDES: { side: DockSide; label: string }[] = [
  { side: "left", label: "Left" },
  { side: "right", label: "Right" },
];

const floats = $derived(window.docking.state === "floats");

async function chooseSide(side: DockSide) {
  error = failure(await commands.setDockSide(side));
}

async function makeRoom(on: boolean) {
  error = failure(await commands.setMakeRoom(on));
}

async function startAtLogin(on: boolean) {
  error = failure(await commands.setStartAtLogin(on));
}
</script>

{#if window.docking.state === "floats"}
  <p class="small muted">Floating window. {window.docking.reason}</p>
{/if}
<div class="rows">
  <div class="row" class:off={floats}>
    <span><b>Dock side</b><small>The edge of the browser the panel sits against.</small></span>
    <fieldset class="seg" aria-label="Dock side" disabled={floats}>
      {#each SIDES as { side, label } (side)}
        <button
          type="button"
          class:on={window.dockSide === side}
          aria-pressed={window.dockSide === side}
          onclick={() => chooseSide(side)}
        >
          {label}
        </button>
      {/each}
    </fieldset>
  </div>
  <label class="row" class:off={floats}>
    <span>
      <b>Make room</b>
      <small>Shrink the browser so the panel doesn't cover the game.</small>
    </span>
    <input
      type="checkbox"
      checked={window.makeRoom}
      disabled={floats}
      onchange={(e) => makeRoom(e.currentTarget.checked)}
    >
  </label>
  <label class="row">
    <span>
      <b>Start DCSS Copilot when I log in</b>
      <small>It waits in the tray and shows the panel when a game starts.</small>
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

.row + .row {
  border-top: 1px solid var(--divider);
}

.row.off {
  opacity: 0.5;
}

.row > span {
  flex: 1;
}

.row small {
  display: block;
  font-size: 11.5px;
  color: var(--text-muted);
}

.seg {
  display: flex;
  margin: 0;
  border: 0;
  padding: 2px;
  border-radius: 8px;
  background: var(--surface);
}

.seg button {
  padding: 3px 9px;
  border: 0;
  border-radius: 6px;
  background: none;
  font-size: 11.5px;
  color: var(--text-muted);
  cursor: pointer;
}

.seg button.on {
  background: rgba(255, 255, 255, 0.9);
  color: #111111;
  font-weight: 600;
}

.error {
  margin: 8px 0 0;
  color: var(--error);
}
</style>

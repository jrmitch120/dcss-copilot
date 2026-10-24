<script lang="ts">
import type { Appearance } from "$lib/bindings";
import type { PanelActions } from "./actions";

let {
  appVersion,
  appearance,
  actions,
}: { appVersion: string; appearance: Appearance; actions: PanelActions } = $props();

let saveError = $state<string | null>(null);

const CHOICES: [Appearance, string][] = [
  ["tiles", "Tiles"],
  ["glyphs", "Glyphs"],
];

async function choose(choice: Appearance) {
  const error = await actions.setAppearance(choice);
  saveError = error === null ? null : `This choice lasts only until DCSS Copilot closes: ${error}`;
}

async function openSettings() {
  const error = await actions.openSettings();
  saveError = error === null ? null : `Settings couldn't open: ${error}`;
}
</script>

<footer>
  <span class="version">v{appVersion}</span>
  <span class="sp"></span>
  <fieldset class="seg" aria-label="Draw monsters and items as">
    {#each CHOICES as [choice, label] (choice)}
      <button
        type="button"
        class:on={appearance === choice}
        aria-pressed={appearance === choice}
        onclick={() => choose(choice)}
      >
        {label}
      </button>
    {/each}
  </fieldset>
  <button type="button" class="gear" title="Settings" aria-label="Settings" onclick={openSettings}>
    ⚙
  </button>
</footer>
{#if saveError}
  <p class="error small">{saveError}</p>
{/if}

<style>
footer {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 14px 10px;
}

.version {
  font-size: 11px;
  color: var(--text-faint);
}

.sp {
  flex: 1;
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
  padding: 2px 8px;
  border: 0;
  border-radius: 6px;
  background: none;
  font-size: 11px;
  color: var(--text-muted);
  cursor: pointer;
}

.seg button.on {
  background: rgba(255, 255, 255, 0.9);
  color: #111111;
  font-weight: 600;
}

.gear {
  padding: 0 4px;
  border: 0;
  background: none;
  font-size: 14px;
  color: var(--text-muted);
  cursor: pointer;
}

.gear:hover {
  color: var(--text-strong);
}

.error {
  margin: 0;
  padding: 0 14px 10px;
  color: var(--error);
}
</style>

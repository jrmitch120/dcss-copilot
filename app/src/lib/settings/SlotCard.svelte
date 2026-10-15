<script lang="ts">
import { commands, type SlotView, type ThinkingOption } from "$lib/bindings";

let { title, use, slot }: { title: string; use: string; slot: SlotView } = $props();

let error = $state<string | null>(null);

const THINKING: Record<ThinkingOption, string> = {
  off: "Off",
  low: "Adaptive · low",
  medium: "Adaptive · medium",
  high: "Adaptive · high",
};

const current = $derived(slot.models.find((m) => m.id === slot.model));

async function choose(model: string, thinking: ThinkingOption) {
  const result = await commands.setModel(slot.slot, model, thinking);
  error = result.status === "error" ? result.error : null;
}

function chooseModel(id: string) {
  const model = slot.models.find((m) => m.id === id);
  if (!model) return;
  // Keep the thinking setting if the new model takes it.
  const thinking = model.thinking.includes(slot.thinking) ? slot.thinking : model.suggested;
  choose(id, thinking);
}
</script>

<div class="slot">
  <div class="head"><b>{title}</b><span class="tiny muted">{use}</span></div>
  <div class="grid">
    <span>Provider</span>
    <span>{slot.provider.name}</span>
    <label for="{slot.slot}-model">Model</label>
    <select
      id="{slot.slot}-model"
      class="sel"
      value={slot.model}
      onchange={(e) => chooseModel(e.currentTarget.value)}
    >
      {#each slot.models as model (model.id)}
        <option value={model.id}>{model.name}</option>
      {/each}
    </select>
    <span>Thinking</span>
    <fieldset class="seg" aria-label="{title} thinking">
      {#each current?.thinking ?? [] as thinking (thinking)}
        <button
          type="button"
          class:on={slot.thinking === thinking}
          aria-pressed={slot.thinking === thinking}
          onclick={() => choose(slot.model, thinking)}
        >
          {THINKING[thinking]}
        </button>
      {/each}
    </fieldset>
  </div>
  {#if error}
    <p class="error small">{error}</p>
  {/if}
</div>

<style>
.slot {
  margin-top: 8px;
  padding: 10px 12px;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--surface-low);
}

.head {
  display: flex;
  align-items: baseline;
  gap: 8px;
  margin-bottom: 8px;
}

.head b {
  font-size: 13px;
}

.tiny {
  font-size: 11.5px;
}

.grid {
  display: grid;
  grid-template-columns: 70px 1fr;
  gap: 6px 10px;
  align-items: center;
  font-size: 12px;
}

.grid > span:nth-child(odd),
.grid > label {
  color: var(--text-muted);
}

.sel {
  justify-self: start;
  padding: 4px 8px;
  border: 1px solid var(--border-strong);
  border-radius: 8px;
  background: var(--inset);
  font-size: 12px;
  color: var(--text);
}

.seg {
  display: flex;
  justify-self: start;
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

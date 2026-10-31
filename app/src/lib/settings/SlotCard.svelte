<script lang="ts">
import {
  commands,
  type KeyView,
  type ProviderOption,
  type SlotView,
  type ThinkingOption,
} from "$lib/bindings";
import { failure } from "$lib/command";

let {
  title,
  use,
  slot,
  providers,
  keys,
}: {
  title: string;
  use: string;
  slot: SlotView;
  providers: ProviderOption[];
  keys: KeyView[];
} = $props();

/** What the model list holds to let the player name a model the app doesn't list. */
const OTHER = "other";

let error = $state<string | null>(null);
/** A provider the player picked that has no model to choose yet, such as a server with none. */
let picked = $state<string | null>(null);
let naming = $state(false);
let named = $state("");

const THINKING: Record<ThinkingOption, string> = {
  off: "Off",
  low: "Adaptive · low",
  medium: "Adaptive · medium",
  high: "Adaptive · high",
};

const providerId = $derived(picked ?? slot.provider.id);
const option = $derived(providers.find((p) => p.provider.id === providerId));
const inSlot = $derived(picked === null);
const models = $derived.by(() => {
  const offered = option?.models ?? [];
  // A model the app doesn't list, or a server no longer has, stays on show while it is chosen.
  return inSlot && !offered.some((m) => m.id === slot.model.id)
    ? [slot.model, ...offered]
    : offered;
});
const server = $derived(option?.provider.local ?? false);
const needsKey = $derived(
  keys.find((k) => k.provider.id === providerId)?.key.state === "stored" ? false : !server,
);

async function choose(provider: string, model: string, thinking: ThinkingOption) {
  error = failure(await commands.setModel(slot.slot, provider, model, thinking));
  if (error === null) {
    picked = null;
    naming = false;
    named = "";
  }
}

/** The slot's thinking setting if `accepted` takes it, else `otherwise`. */
function keptThinking(accepted: ThinkingOption[], otherwise: ThinkingOption): ThinkingOption {
  return accepted.includes(slot.thinking) ? slot.thinking : otherwise;
}

function chooseProvider(id: string) {
  naming = false;
  if (id === slot.provider.id) {
    picked = null;
    return;
  }
  const first = providers.find((p) => p.provider.id === id)?.models[0];
  if (first) {
    choose(id, first.id, first.suggested);
  } else {
    picked = id;
  }
}

function chooseModel(id: string) {
  if (id === OTHER) {
    naming = true;
    return;
  }
  naming = false;
  const model = models.find((m) => m.id === id);
  if (!model) return;
  choose(providerId, id, keptThinking(model.thinking, model.suggested));
}

function nameModel() {
  choose(providerId, named, keptThinking(option?.unlistedThinking ?? [], "off"));
}
</script>

<div class="slot">
  <div class="head"><b>{title}</b><span class="tiny muted">{use}</span></div>
  <div class="grid">
    <label for="{slot.slot}-provider">Provider</label>
    <select
      id="{slot.slot}-provider"
      class="sel"
      value={providerId}
      onchange={(e) => chooseProvider(e.currentTarget.value)}
    >
      {#each providers as { provider } (provider.id)}
        <option value={provider.id}>
          {provider.name}{provider.local ? " (experimental · $0)" : ""}
        </option>
      {/each}
    </select>
    <label for="{slot.slot}-model">Model</label>
    <div class="model">
      <select
        id="{slot.slot}-model"
        class="sel"
        value={naming ? OTHER : inSlot ? slot.model.id : ""}
        onchange={(e) => chooseModel(e.currentTarget.value)}
      >
        {#if !inSlot}
          <option value="" disabled>Choose a model</option>
        {/if}
        {#each models as model (model.id)}
          <option value={model.id}>{model.name}</option>
        {/each}
        {#if option?.unlistedThinking}
          <option value={OTHER}>Another model…</option>
        {/if}
      </select>
      {#if naming}
        <form
          class="named"
          onsubmit={(e) => {
            e.preventDefault();
            nameModel();
          }}
        >
          <input
            class="field"
            spellcheck="false"
            placeholder="The model's id at {option?.provider.name}"
            aria-label="{title} model id"
            bind:value={named}
          >
          <button type="submit" class="use" disabled={named.trim() === ""}>Use</button>
        </form>
      {/if}
    </div>
    {#if inSlot}
      <span>Thinking</span>
      <fieldset class="seg" aria-label="{title} thinking">
        {#each slot.model.thinking as thinking (thinking)}
          <button
            type="button"
            class:on={slot.thinking === thinking}
            aria-pressed={slot.thinking === thinking}
            onclick={() => choose(slot.provider.id, slot.model.id, thinking)}
          >
            {THINKING[thinking]}
          </button>
        {/each}
      </fieldset>
    {/if}
  </div>
  {#if !inSlot}
    <p class="tiny muted">
      {title}
      stays on {slot.model.name} until you choose a model from {option?.provider.name}. Is it
      running? See Local models below.
    </p>
  {:else if needsKey}
    <p class="tiny warning">This needs a {option?.provider.name} key: add one below.</p>
  {/if}
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

.grid > span,
.grid > label {
  color: var(--text-muted);
}

.model {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  align-items: center;
}

.named {
  display: flex;
  flex: 1;
  gap: 6px;
  min-width: 200px;
}

.field {
  flex: 1;
  min-width: 0;
  padding: 4px 8px;
  border: 1px solid var(--border-strong);
  border-radius: 8px;
  background: var(--inset);
  font-size: 12px;
  outline: none;
}

.use {
  padding: 4px 10px;
  border: 0;
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.9);
  font-size: 12px;
  font-weight: 600;
  color: #111111;
  cursor: pointer;
}

.use:disabled {
  cursor: default;
  opacity: 0.5;
}

p {
  margin: 8px 0 0;
}

.warning {
  color: var(--warning);
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

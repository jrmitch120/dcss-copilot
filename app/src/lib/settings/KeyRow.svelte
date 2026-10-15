<script lang="ts">
import { commands, type KeyView } from "$lib/bindings";
import { failure } from "$lib/command";

let { key }: { key: KeyView } = $props();

let replacing = $state(false);
let pasted = $state("");
let error = $state<string | null>(null);

const provider = $derived(key.provider);
const entering = $derived(key.key.state === "missing" || replacing);
const ownKey = $derived(key.key.state === "stored" && key.key.place.place !== "environment");

async function save() {
  const saved = failure(await commands.saveKey(provider.id, pasted));
  // The paste is never kept once it has been handed over.
  pasted = "";
  error = saved;
  if (saved !== null) {
    return;
  }
  replacing = false;
  await test();
}

async function test() {
  error = failure(await commands.testKey(provider.id));
}

async function remove() {
  error = failure(await commands.removeKey(provider.id));
}

function seconds(millis: number): string {
  return `${(millis / 1000).toFixed(1)} s`;
}
</script>

<div class="key">
  <div class="row">
    <span class="what">
      <b class="small">{provider.name}</b>
      {#if key.key.state === "stored"}
        <span class="mono">{key.key.masked}</span>
        {#if key.key.place.place === "keychain"}
          <span class="tiny muted">keychain</span>
        {:else if key.key.place.place === "environment"}
          <span class="tiny muted">from {key.key.place.variable} in your environment</span>
        {:else}
          <span class="tiny warning">this session only</span>
        {/if}
      {:else}
        <span class="tiny muted">no key</span>
      {/if}
    </span>
    {#if key.key.state === "stored" && !replacing}
      <span class="actions">
        <button type="button" class="btn" disabled={key.test.state === "testing"} onclick={test}>
          Test
        </button>
        <button type="button" class="btn" onclick={() => (replacing = true)}>
          {ownKey ? "Replace" : "Use another"}
        </button>
        {#if ownKey}
          <button type="button" class="btn danger" onclick={remove}>Remove</button>
        {/if}
      </span>
    {/if}
  </div>

  {#if entering}
    <form
      class="entry"
      onsubmit={(e) => {
        e.preventDefault();
        save();
      }}
    >
      <input
        class="field mono"
        type="password"
        autocomplete="off"
        spellcheck="false"
        placeholder="Paste your {provider.name} API key"
        aria-label="{provider.name} API key"
        bind:value={pasted}
      >
      <button type="submit" class="btn primary" disabled={pasted.trim() === ""}>
        Save and test
      </button>
      {#if replacing}
        <button type="button" class="btn link" onclick={() => (replacing = false)}>Cancel</button>
      {/if}
    </form>
    {#if key.key.state === "missing"}
      <p class="tiny muted">
        DCSS Copilot also uses {key.key.variable} if it is set in your environment.
      </p>
    {/if}
  {/if}

  {#if key.test.state === "testing"}
    <p class="small muted">Testing…</p>
  {:else if key.test.state === "works"}
    <p class="small ok">✓ Works · {key.test.model} answered in {seconds(key.test.millis)}</p>
  {:else if key.test.state === "rejected"}
    <p class="small bad">✕ {provider.name} rejected that key: {key.test.message}</p>
  {:else if key.test.state === "failed"}
    <p class="small bad">Couldn't test the key: {key.test.message}</p>
  {/if}

  {#if key.key.state === "stored" && key.key.place.place === "session"}
    <p class="tiny warning">
      No system keychain could be used ({key.key.place.reason}), so the key is kept in memory and
      asked for again next launch.
    </p>
  {/if}
  {#if error}
    <p class="small bad">{error}</p>
  {/if}
</div>

<style>
.key {
  margin-top: 6px;
  padding: 8px 10px;
  border-radius: 10px;
  background: rgba(0, 0, 0, 0.25);
}

.row {
  display: grid;
  grid-template-columns: 1fr auto;
  gap: 8px;
  align-items: center;
}

.what {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 8px;
}

.mono {
  font-family: var(--mono);
  color: var(--text-soft);
}

.tiny {
  font-size: 11.5px;
}

.actions {
  display: flex;
  gap: 4px;
}

.entry {
  display: flex;
  gap: 6px;
  margin-top: 8px;
}

.field {
  flex: 1;
  min-width: 0;
  padding: 6px 10px;
  border: 1px solid var(--border-strong);
  border-radius: 8px;
  background: var(--inset);
  font-size: 12px;
  outline: none;
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

.btn:disabled {
  cursor: default;
  opacity: 0.5;
}

.btn.primary {
  background: rgba(255, 255, 255, 0.92);
  color: #111111;
  font-weight: 600;
}

.btn.danger {
  background: rgba(255, 59, 92, 0.1);
  color: #ff8da1;
}

.btn.link {
  background: none;
  color: var(--text-muted);
}

p {
  margin: 6px 0 0;
}

.ok {
  color: var(--calm);
}

.bad {
  color: var(--error);
}

.warning {
  color: var(--warning);
}
</style>

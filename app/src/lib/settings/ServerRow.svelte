<script lang="ts">
import { commands, type ServerView } from "$lib/bindings";
import { failure } from "$lib/command";

let { server }: { server: ServerView } = $props();

let error = $state<string | null>(null);

const provider = $derived(server.provider);

async function move(address: string) {
  error = failure(await commands.setServerAddress(provider.id, address));
}

async function detect() {
  error = failure(await commands.detectModels(provider.id));
}
</script>

<div class="server">
  <div class="row">
    <span class="what">
      <b class="small">{provider.name}</b>
      <span class="tiny warning">Experimental · $0</span>
      <span class="tiny muted">no key needed</span>
    </span>
    <button
      type="button"
      class="btn"
      disabled={server.detection.state === "checking"}
      onclick={detect}
    >
      Check again
    </button>
  </div>
  <input
    class="field mono"
    type="url"
    spellcheck="false"
    aria-label="{provider.name} address"
    value={server.address}
    onchange={(e) => move(e.currentTarget.value)}
  >
  {#if server.detection.state === "checking"}
    <p class="small muted">Asking {provider.name}…</p>
  {:else if server.detection.state === "found"}
    {#if server.detection.models.length === 0}
      <p class="small warning">{provider.name} is running, but has no models yet.</p>
    {:else}
      <p class="small ok">✓ Found {server.detection.models.join(", ")}</p>
    {/if}
  {:else if server.detection.state === "unreachable"}
    <p class="small muted">Not found at this address: {server.detection.message}</p>
  {/if}
  {#if provider.id === "ollama"}
    <p class="tiny muted">
      The context length is set for you: Ollama's 4k default cuts prompts off.
    </p>
  {/if}
  {#if error}
    <p class="small bad">{error}</p>
  {/if}
</div>

<style>
.server {
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

.field {
  width: 100%;
  margin-top: 8px;
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

<script lang="ts">
import type { AskBox } from "$lib/bindings";

let { ask }: { ask: AskBox } = $props();
</script>

<div class="ask">
  {#if ask.state === "needsModel"}
    <input
      placeholder="Ask about your run…"
      disabled
      title="Asking the Advisor needs a model, and none is set up yet."
    >
  {/if}
</div>

<style>
.ask {
  display: flex;
  gap: 6px;
  padding: 8px 14px 10px;
  border-top: 1px solid var(--divider);
}

input {
  flex: 1;
  min-width: 0;
  padding: 7px 10px;
  border: 1px solid var(--border-strong);
  border-radius: 10px;
  background: var(--inset);
  outline: none;
  font-size: 13px;
}

input:disabled {
  cursor: not-allowed;
  opacity: 0.6;
}
</style>

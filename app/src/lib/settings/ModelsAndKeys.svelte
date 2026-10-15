<script lang="ts">
import type { ModelsAndKeysView } from "$lib/bindings";
import KeyRow from "./KeyRow.svelte";
import SlotCard from "./SlotCard.svelte";

let { models }: { models: ModelsAndKeysView } = $props();
</script>

<SlotCard title="Quick Model" use="everyday advice and questions" slot={models.quick} />
<SlotCard title="Deep Model" use="only when you tick “Think deeper”" slot={models.deep} />

<div class="lbl keys">API keys</div>
{#each models.keys as key (key.provider.id)}
  <KeyRow {key} />
{/each}
<p class="tiny muted">
  Keys are shown only masked, and never written to settings, logs, Recordings or crash files.
</p>

<style>
.keys {
  margin-top: 18px;
}

.tiny {
  margin: 8px 0 0;
  font-size: 11.5px;
}
</style>

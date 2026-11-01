<script lang="ts">
import { commands, type ModelsAndKeysView } from "$lib/bindings";
import KeyRow from "./KeyRow.svelte";
import ServerRow from "./ServerRow.svelte";
import SlotCard from "./SlotCard.svelte";

let { models }: { models: ModelsAndKeysView } = $props();

$effect(() => {
  // Each server is asked once it has an address nobody has asked yet: as the window opens, and
  // when the player moves it.
  for (const server of models.servers) {
    if (server.detection.state === "unchecked") {
      commands.detectModels(server.provider.id);
    }
  }
});
</script>

<SlotCard
  title="Quick Model"
  use="everyday advice and questions"
  slot={models.quick}
  providers={models.providers}
  keys={models.keys}
/>
<SlotCard
  title="Deep Model"
  use="only when you tick “Think deeper”"
  slot={models.deep}
  providers={models.providers}
  keys={models.keys}
/>

<div class="lbl keys">API keys</div>
{#each models.keys as key (key.provider.id)}
  <KeyRow {key} />
{/each}
<p class="tiny muted">
  Keys are shown only masked, and never written to settings, logs, Recordings or crash files.
</p>

<div class="lbl keys">Local models</div>
{#each models.servers as server (server.provider.id)}
  <ServerRow {server} />
{/each}
<p class="tiny muted">
  Local models run on your own server, for free. They are experimental: quality varies, and a slow
  answer is dropped once the game has moved on.
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

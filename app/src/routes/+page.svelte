<script lang="ts">
import { commands, type ViewModel } from "$lib/bindings";

let view = $state<ViewModel | null>(null);

$effect(() => {
  commands
    .viewModel()
    .then((v) => {
      view = v;
    })
    .catch((e: unknown) => {
      console.error("Could not load the view model", e);
    });
});
</script>

<main>
  <h1>DCSS Copilot</h1>
  {#if view}
    <p>v{view.appVersion}</p>
  {/if}
</main>

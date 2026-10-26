<script lang="ts">
import type { GapsChip } from "$lib/bindings";
import type { PanelActions } from "./actions";
import GapsList from "./GapsList.svelte";

/** The Gap prompt card: once a game, at an All Clear, what the Advisor can't see yet. */
let { gaps, actions }: { gaps: GapsChip; actions: PanelActions } = $props();

let dismissError = $state<string | null>(null);

async function dismiss() {
  const error = await actions.dismissGapPrompt();
  dismissError = error === null ? null : `The card couldn't be put away: ${error}`;
}
</script>

<section class="card prompt">
  <div class="lbl">Gaps <em>asked once</em></div>
  <p class="small">
    While it's quiet: I can't see these yet. Open them in the game when you can, and I'll judge with
    them. Nothing is sent to the game by DCSS Copilot.
  </p>
  <GapsList {gaps} />
  <div class="acts"><button type="button" onclick={dismiss}>Don't ask again</button></div>
</section>
{#if dismissError}
  <p class="error small">{dismissError}</p>
{/if}

<style>
.prompt :global(ul) {
  margin-top: 8px;
}

.error {
  margin: 0;
  color: var(--error);
}
</style>

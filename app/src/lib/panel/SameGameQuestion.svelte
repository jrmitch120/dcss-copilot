<script lang="ts">
import type { PickedUp } from "$lib/bindings";
import type { PanelActions } from "./actions";

let {
  character,
  pickedUp,
  actions,
}: { character: string; pickedUp: PickedUp; actions: PanelActions } = $props();

let answerError = $state<string | null>(null);

let lastSeen = $derived(
  new Date(pickedUp.lastSeenMs).toLocaleString(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }),
);

async function answer(same: boolean) {
  const error = await actions.answerSameGame(same);
  answerError = error === null ? null : `Your answer couldn't be taken: ${error}`;
}
</script>

<section class="card prompt">
  <div class="lbl">Is this the same game?</div>
  <p class="small">
    DCSS Copilot last saw {character}
    {#if pickedUp.turn !== null}
      on turn {pickedUp.turn},
    {/if}
    {lastSeen}, and missed the turns since. If this is another game of {character}, what it
    remembers is wrong for it.
  </p>
  <p class="small muted">Until you answer, it shows and saves nothing of this game.</p>
  <div class="acts">
    <button type="button" class="pri" onclick={() => answer(true)}>Same game</button>
    <button type="button" onclick={() => answer(false)}>A new game</button>
  </div>
  {#if answerError}
    <p class="error small">{answerError}</p>
  {/if}
</section>

<style>
.error {
  color: var(--error);
}
</style>

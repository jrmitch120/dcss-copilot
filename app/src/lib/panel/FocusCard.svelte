<script lang="ts">
import type { Appearance, FocusCard } from "$lib/bindings";
import ListingRow from "./ListingRow.svelte";

let { focus, appearance }: { focus: FocusCard; appearance: Appearance } = $props();
</script>

{#if focus.card === "inView"}
  <section class="card">
    <div class="lbl">In view</div>
    {#if focus.monsters === null}
      <p class="muted small">
        The Advisor can't read monsters yet, so it can't list what's in view.
      </p>
    {:else if focus.monsters.length === 0}
      <p class="muted small">No monsters in view.</p>
    {:else}
      <ul>
        {#each focus.monsters as listing, i (i)}
          <ListingRow {listing} {appearance} />
        {/each}
      </ul>
    {/if}
  </section>
{/if}

<style>
p {
  margin: 0;
}

ul {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin: 0;
  padding: 0;
  list-style: none;
}
</style>

<script lang="ts">
import type { Appearance, FocusCard, Listing } from "$lib/bindings";
import ListingRow from "./ListingRow.svelte";

let { focus, appearance }: { focus: FocusCard; appearance: Appearance } = $props();
</script>

{#snippet inView(
  monsters: Listing[] | null,
)}
  {#if monsters === null}
    <p class="muted small">The Advisor can't read monsters yet, so it can't list what's in view.</p>
  {:else if monsters.length === 0}
    <p class="muted small">No monsters in view.</p>
  {:else}
    <ul>
      {#each monsters as listing, i (i)}
        <ListingRow {listing} {appearance} />
      {/each}
    </ul>
  {/if}
{/snippet}

{#if focus.card === "inView"}
  <section class="card">
    <div class="lbl">In view</div>
    {@render inView(focus.monsters)}
  </section>
{:else if focus.card === "fight"}
  <section class="card">
    <div class="lbl">Fight <em>computed</em></div>
    {@render inView(focus.monsters)}
    <p class="line">{focus.line}</p>
    {#each focus.others as other, i (i)}
      <p class="muted small">{other}</p>
    {/each}
    {#if focus.notModelled}
      <p class="muted small not-modelled">{focus.notModelled}</p>
    {/if}
    {#each focus.gaps as gap, i (i)}
      <p class="muted small">{gap}</p>
    {/each}
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

.line {
  margin-top: 8px;
  font-size: 13px;
  color: var(--text-strong);
}

.not-modelled {
  margin-top: 6px;
}
</style>

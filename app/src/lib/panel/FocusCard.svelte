<script lang="ts">
import type { Appearance, FactCard, FocusCard, Listing } from "$lib/bindings";
import type { PanelActions } from "./actions";
import JoinCard from "./JoinCard.svelte";
import ListingRow from "./ListingRow.svelte";
import Marked from "./Marked.svelte";
import Tile from "./Tile.svelte";

let {
  focus,
  factCards,
  appearance,
  actions,
}: {
  focus: FocusCard;
  factCards: FactCard[];
  appearance: Appearance;
  actions: PanelActions;
} = $props();
</script>

{#snippet facts()}
  {#each factCards as card, i (i)}
    <div class="fact">
      <Tile depiction={card.depiction} {appearance} size={32} />
      <div>
        <div class="name">{card.name}</div>
        <div class="muted small">{card.why}</div>
      </div>
      {#if card.alone}
        <span class="alone {card.alone}">alone: {card.alone.toUpperCase()}</span>
      {/if}
    </div>
  {/each}
{/snippet}

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
    {@render facts()}
    <div class="lbl">In view</div>
    {@render inView(focus.monsters)}
  </section>
{:else if focus.card === "fight"}
  <section class="card">
    {@render facts()}
    <div class="lbl">Fight <em>computed</em></div>
    {@render inView(focus.monsters)}
    <p class="line"><Marked value={focus.line} /></p>
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
{:else if focus.card === "join"}
  <JoinCard join={focus.join} {actions} />
{/if}

<style>
p {
  margin: 0;
}

.fact {
  display: grid;
  grid-template-columns: 32px 1fr auto;
  gap: 8px;
  align-items: center;
  margin-bottom: 8px;
  padding: 6px 8px;
  border: 1px solid var(--border-strong);
  border-radius: 10px;
  background: var(--surface-low);
}

.fact .name {
  font-size: 13px;
  font-weight: 700;
  color: var(--text-strong);
}

.alone {
  font-size: 10.5px;
  font-weight: 800;
  letter-spacing: 0.08em;
}

.alone.calm {
  color: var(--calm);
}

.alone.dangerous {
  color: var(--warning);
}

.alone.lethal {
  color: var(--danger);
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

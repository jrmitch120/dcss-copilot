<script lang="ts">
import type { GapsChip } from "$lib/bindings";

/** Each Gap, why it matters and the keys that fill it, as the Gaps chip and prompt card list them. */
let { gaps }: { gaps: GapsChip } = $props();

/** Joins a list as the panel's sentences do, with no comma before the last: `a, b and c`. */
const sentenceList = new Intl.ListFormat("en-GB", { type: "conjunction" });
</script>

<ul>
  {#each gaps.gaps as gap, i (i)}
    <li>
      <span class="what">{gap.what}</span>
      {#if gap.keys.length > 0}
        <span class="keys">
          {#each gap.keys as key, k (k)}
            {#if k > 0}
              then
            {/if}
            <kbd>{key}</kbd>
          {/each}
        </span>
      {/if}
      <span class="why muted small">{gap.why}</span>
    </li>
  {/each}
</ul>
{#if gaps.dumpFills.length > 0}
  <p class="small muted">
    Or press <kbd>?</kbd> then <kbd>#</kbd> for the character dump: it fills
    {sentenceList.format(gaps.dumpFills)}
    at once.
  </p>
{/if}

<style>
ul {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin: 0;
  padding: 0;
  list-style: none;
}

li {
  display: grid;
  grid-template-columns: 1fr auto;
  column-gap: 8px;
  font-size: 12.5px;
}

.what {
  color: var(--text-strong);
}

.keys {
  justify-self: end;
  color: var(--text-muted);
  font-size: 11.5px;
}

.why {
  grid-column: 1 / -1;
}

p {
  margin: 8px 0 0;
}
</style>

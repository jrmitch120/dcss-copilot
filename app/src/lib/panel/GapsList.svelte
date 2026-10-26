<script lang="ts">
import type { GapsChip } from "$lib/bindings";

/** What the Gaps chip opens: each Gap, why it matters and the keys that fill it. */
let { gaps }: { gaps: GapsChip } = $props();

/** Joins a list as the panel's sentences do, with no comma before the last: `a, b and c`. */
const sentenceList = new Intl.ListFormat("en-GB", { type: "conjunction" });
</script>

<section class="card gaps">
  <div class="lbl">Gaps <em>what the Advisor can't see</em></div>
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
</section>

<style>
.gaps {
  margin: 8px 14px 0;
}

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

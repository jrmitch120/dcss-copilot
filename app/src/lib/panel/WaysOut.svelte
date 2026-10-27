<script lang="ts">
import type { WaysOutList } from "$lib/bindings";
import MarkedValue from "./MarkedValue.svelte";

let { waysOut }: { waysOut: WaysOutList } = $props();
</script>

<section class="card">
  <div class="lbl">Ways out <em>computed</em></div>
  {#if waysOut.rows.length === 0}
    <p class="muted small">No Ways Out known.</p>
  {:else}
    <ul>
      {#each waysOut.rows as row, i (i)}
        <li>
          <span
            class="mark"
            class:ok={row.usable === true}
            class:no={row.usable === false}
            class:unknown={row.usable === null}
          >
            {row.usable === null ? "?" : row.usable ? "✓" : "✕"}
          </span>
          <span>
            {row.name}
            {#if row.detail}
              <span class="muted small"><MarkedValue value={row.detail} /></span>
            {/if}
            {#if row.whyNot}
              <span class="why small"><MarkedValue value={row.whyNot} /></span>
            {/if}
          </span>
        </li>
      {/each}
    </ul>
  {/if}
  {#if waysOut.ground}
    <p class="small ground">{waysOut.ground}</p>
  {/if}
  {#each waysOut.gaps as gap, i (i)}
    <p class="muted small">{gap}</p>
  {/each}
</section>

<style>
p {
  margin: 6px 0 0;
}

ul {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin: 0;
  padding: 0;
  list-style: none;
}

li {
  display: grid;
  grid-template-columns: 14px 1fr;
  gap: 6px;
  font-size: 12.5px;
}

.mark {
  font-weight: 700;
}

.ok {
  color: var(--calm);
}

.no {
  color: var(--danger);
}

.unknown {
  color: var(--unrated);
}

.why {
  display: block;
  color: var(--text-soft);
}

.ground {
  color: var(--text-soft);
}
</style>

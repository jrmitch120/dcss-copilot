<script lang="ts">
import type { Appearance, Listing } from "$lib/bindings";
import Tile from "./Tile.svelte";

let { listing, appearance }: { listing: Listing; appearance: Appearance } = $props();
</script>

<li>
  <Tile depiction={listing.depiction} {appearance} size={32} />
  <span>
    {listing.name}
    {#if listing.count > 1}
      <span class="muted">×{listing.count}</span>
    {/if}
  </span>
  {#if listing.detail}
    <span class="muted small">{listing.detail}</span>
  {/if}
</li>

<style>
li {
  display: grid;
  grid-template-columns: 32px 1fr auto;
  gap: 8px;
  align-items: center;
  font-size: 12.5px;
}
</style>

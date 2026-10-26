<script lang="ts">
import type { Value } from "$lib/bindings";

/**
 * A value as the panel shows it: marked in place, with a dotted underline and a tooltip naming
 * the screen that fills it, while a Gap leaves it unknown or out of date.
 */
let { value }: { value: Value } = $props();
</script>

{#if value.gap}
  <span class="gap" title={value.gap}>{value.text}</span>
{:else}
  {value.text}
{/if}

<style>
.gap {
  text-decoration: underline dotted;
  text-underline-offset: 3px;
  cursor: help;
}
</style>

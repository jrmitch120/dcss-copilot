<script lang="ts">
import type { SuggestedActionCard } from "$lib/bindings";
import Marked from "./Marked.svelte";

/** The Suggested Action, named by the rule that picked it. */
let { suggested }: { suggested: SuggestedActionCard } = $props();
</script>

<section class="card {suggested.level}">
  <div class="lbl">Suggested <em>{suggested.level.toUpperCase()} rule · no LLM</em></div>
  <p class="rule">{suggested.rule}</p>
  <p class="action"><Marked value={suggested.action} /></p>
  {#if suggested.ifLethal}
    <p class="muted small">If it turns LETHAL: <Marked value={suggested.ifLethal} /></p>
  {/if}
</section>

<style>
p {
  margin: 0;
}

.card.dangerous {
  border-color: rgba(255, 178, 107, 0.45);
}

.card.lethal {
  border-color: rgba(255, 59, 92, 0.45);
}

.rule {
  font-size: 15px;
  font-weight: 700;
  color: var(--text-strong);
}

.action {
  margin: 2px 0 4px;
  font-size: 13px;
  color: var(--text-soft);
}
</style>

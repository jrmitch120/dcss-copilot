<script lang="ts">
import type { SuggestedActionCard } from "$lib/bindings";
import MarkedValue from "./MarkedValue.svelte";

/** The Suggested Action, named by the rule that picked it, and at Coach what the Quick Model makes of it. */
let { suggested }: { suggested: SuggestedActionCard } = $props();
</script>

<section class="card {suggested.level}">
  <div class="lbl">Suggested <em>{suggested.level.toUpperCase()} rule · no LLM</em></div>
  <p class="rule">{suggested.rule}</p>
  <p class="action"><MarkedValue value={suggested.action} /></p>
  {#if suggested.ifLethal}
    <p class="muted small">If it turns LETHAL: <MarkedValue value={suggested.ifLethal} /></p>
  {/if}
  {#if suggested.coach}
    <div class="coach">
      <div class="lbl">Coach <em>LLM</em></div>
      {#if suggested.coach.state === "asking"}
        <p class="muted small">Weighing the options…</p>
      {:else}
        {#if suggested.coach.agrees}
          <p class="small">Agrees.</p>
        {:else}
          <p class="pick">Instead: <MarkedValue value={suggested.coach.action} /></p>
        {/if}
        {#if suggested.coach.reason}
          <p class="reason">{suggested.coach.reason}</p>
        {/if}
      {/if}
    </div>
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

.coach {
  margin-top: 8px;
  padding-top: 8px;
  border-top: 1px solid var(--divider);
}

.pick {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-strong);
}

.reason {
  margin-top: 2px;
  font-size: 13px;
  color: var(--text-soft);
}
</style>

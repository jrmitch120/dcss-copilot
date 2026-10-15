<script lang="ts">
import type { RunPlanRail } from "$lib/bindings";

let { runPlan }: { runPlan: RunPlanRail } = $props();
</script>

<section class="rail">
  {#if runPlan.state === "notPlanned"}
    <div class="lbl">Run Plan <em>not planned yet</em></div>
    <p class="muted small">There's no Run Plan for this game yet.</p>
  {/if}
</section>

<style>
.rail {
  display: flex;
  flex-direction: column;
  gap: 5px;
  padding: 9px 14px 10px;
  border-top: 1px solid var(--border);
  background: var(--inset);
}

.lbl {
  margin: 0;
}

p {
  margin: 0;
}
</style>

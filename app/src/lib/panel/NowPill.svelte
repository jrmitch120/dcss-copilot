<script lang="ts">
import type { NowPill } from "$lib/bindings";

let { now }: { now: NowPill } = $props();
</script>

<div class="now {now.state}">
  {#if now.state === "unrated"}
    <div class="lab">UNRATED</div>
    <div class="line">
      The Advisor can't judge danger yet.
      <small>No Threat Level is shown until it can be computed.</small>
    </div>
  {/if}
</div>

<style>
.now {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 14px;
  border-radius: var(--radius-pill);
}

.now.unrated {
  background: rgba(255, 255, 255, 0.04);
  border: 1px solid var(--border-strong);
}

.lab {
  flex: none;
  font-size: 11px;
  font-weight: 800;
  letter-spacing: 0.14em;
}

.unrated .lab {
  color: var(--unrated);
}

.line {
  flex: 1;
  font-size: 14px;
}

.line small {
  display: block;
  font-size: 11.5px;
  color: var(--text-muted);
}
</style>

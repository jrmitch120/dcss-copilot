<script lang="ts">
import type { NowPill } from "$lib/bindings";
import { threatTone } from "./tone";

let { now }: { now: NowPill } = $props();

let tone = $derived(threatTone(now));
</script>

<div class="now {tone}">
  {#if now.state === "unrated"}
    <div class="lab">UNRATED</div>
    <div class="line">
      {#if now.alert}
        <strong class="alert">{now.alert}</strong>
      {/if}
      The Advisor can't judge danger yet.
      <small>No Threat Level is shown until it can be computed.</small>
    </div>
  {:else}
    <div class="lab">{now.level.toUpperCase()}</div>
    <div class="line">
      {#if now.alert}
        <strong class="alert">{now.alert}</strong>
      {/if}
      {now.line}
      {#if now.detail}
        <small>{now.detail}</small>
      {/if}
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

.now.calm {
  background: rgba(46, 213, 115, 0.08);
  border: 1px solid rgba(46, 213, 115, 0.25);
}

.now.dangerous {
  background: linear-gradient(90deg, rgba(255, 178, 107, 0.2), rgba(255, 178, 107, 0.04));
  border: 1px solid rgba(255, 178, 107, 0.5);
}

.now.lethal {
  background: linear-gradient(90deg, rgba(255, 59, 92, 0.22), rgba(255, 59, 92, 0.05));
  border: 1px solid rgba(255, 59, 92, 0.5);
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

.calm .lab {
  color: var(--calm);
}

.dangerous .lab {
  color: var(--warning);
}

.lethal .lab {
  color: var(--danger);
}

.line {
  flex: 1;
  font-size: 14px;
}

.alert {
  display: block;
  font-weight: 700;
  color: var(--text-strong);
}

.line small {
  display: block;
  font-size: 11.5px;
  color: var(--text-muted);
}
</style>

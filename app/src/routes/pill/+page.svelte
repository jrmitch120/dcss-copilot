<script lang="ts">
import "$lib/panel/theme.css";
import { commands, type ViewModel } from "$lib/bindings";
import { threatTone } from "$lib/panel/tone";
import { followViewModel } from "$lib/view-model";

let view = $state<ViewModel | null>(null);

$effect(() =>
  followViewModel((v) => {
    view = v;
  }),
);

let game = $derived(view?.game ?? null);
let tone = $derived(game ? threatTone(game.now) : "unrated");
let label = $derived(
  game?.now.state === "rated" ? game.now.level.toUpperCase() : game ? "UNRATED" : "NO GAME",
);
let line = $derived.by(() => {
  if (!game) return "Waiting for a game.";
  if (game.now.alert) return game.now.alert;
  return game.now.state === "rated" ? game.now.line : "The Advisor can't judge danger yet.";
});
let waiting = $derived(game?.pill.decisionsWaiting ?? 0);
</script>

<div class="pill {tone}">
  <span class="grip" data-tauri-drag-region title="Drag to move">⋮⋮</span>
  <button type="button" title="Expand the panel" onclick={() => commands.expandPanel()}>
    <span class="lab">{label}</span>
    <span class="line">{line}</span>
    {#if waiting > 0}
      <span class="badge" title="Decisions waiting">{waiting}</span>
    {/if}
    <span class="expand" aria-hidden="true">⤢</span>
  </button>
</div>

<style>
:global(body) {
  margin: 0;
  overflow: hidden;
  background: var(--bg);
}

.pill {
  display: flex;
  align-items: stretch;
  box-sizing: border-box;
  width: 100vw;
  height: 100vh;
  border: 1px solid var(--border-strong);
  background: rgba(16, 19, 27, 0.96);
}

.grip {
  display: flex;
  align-items: center;
  padding: 0 4px 0 8px;
  font-size: 11px;
  letter-spacing: -2px;
  color: var(--text-faint);
  cursor: grab;
  user-select: none;
}

.pill button {
  display: flex;
  flex: 1;
  min-width: 0;
  align-items: center;
  gap: 10px;
  padding: 0 14px 0 6px;
  border: 0;
  background: none;
  font-size: 13px;
  color: var(--text);
  text-align: left;
  cursor: pointer;
}

.pill.calm {
  border-color: rgba(46, 213, 115, 0.4);
}

.pill.dangerous {
  border-color: rgba(255, 178, 107, 0.5);
}

.pill.lethal {
  border-color: var(--danger);
  box-shadow: inset 0 0 0 3px rgba(255, 59, 92, 0.18);
}

.lab {
  flex: none;
  font-size: 10.5px;
  font-weight: 800;
  letter-spacing: 0.14em;
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
  min-width: 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.badge {
  flex: none;
  min-width: 18px;
  height: 18px;
  padding: 0 5px;
  border-radius: 9px;
  background: var(--find);
  color: #111111;
  font: 700 11px / 18px system-ui;
  text-align: center;
}

.expand {
  flex: none;
  font-size: 12px;
  color: var(--text-muted);
}
</style>

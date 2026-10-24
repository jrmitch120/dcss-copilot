<script lang="ts">
import type { AssistanceLevel, Character, NowPill } from "$lib/bindings";
import { type PanelActions, unsaved } from "./actions";
import Segmented from "./Segmented.svelte";
import { threatTone } from "./tone";

let {
  character,
  now,
  assistance,
  actions,
}: { character: Character; now: NowPill; assistance: AssistanceLevel; actions: PanelActions } =
  $props();

let tone = $derived(threatTone(now));
let saveError = $state<string | null>(null);

const LEVELS: [AssistanceLevel, string][] = [
  ["info", "Info"],
  ["hints", "Hints"],
  ["coach", "Coach"],
];

async function choose(level: AssistanceLevel) {
  const error = await actions.setAssistance(level);
  saveError = unsaved(error);
}

function stat(value: number | null): string {
  return value === null ? "?" : String(value);
}
</script>

<header>
  <span class="dot {tone}"></span>
  <span class="who">
    <b>{character.name}</b>
    · {character.species} · HP {stat(character.hp)}/{stat(character.hpMax)}
    · T {stat(character.turn)}
  </span>
  <span class="sp"></span>
  <Segmented label="Assistance level" choices={LEVELS} chosen={assistance} {choose} />
</header>
{#if saveError}
  <p class="error small">{saveError}</p>
{/if}

<style>
header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 11px 14px 9px;
  border-bottom: 1px solid var(--divider);
}

.dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex: none;
}

.dot.unrated {
  background: var(--unrated);
}

.dot.calm {
  background: var(--calm);
  box-shadow: 0 0 8px var(--calm);
}

.dot.dangerous {
  background: var(--warning);
  box-shadow: 0 0 8px var(--warning);
}

.dot.lethal {
  background: var(--danger);
  box-shadow: 0 0 8px var(--danger);
}

.who {
  font-size: 12.5px;
  color: var(--unrated);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  font-variant-numeric: tabular-nums;
}

.who b {
  color: var(--text-strong);
}

.sp {
  flex: 1;
}

.error {
  margin: 0;
  padding: 6px 14px 0;
  color: var(--error);
}
</style>

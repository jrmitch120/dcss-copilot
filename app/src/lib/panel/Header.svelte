<script lang="ts">
import type { AssistanceLevel, Character, GapsChip, NowPill } from "$lib/bindings";
import { type PanelActions, unsaved } from "./actions";
import GapsList from "./GapsList.svelte";
import Segmented from "./Segmented.svelte";
import { threatTone } from "./tone";

let {
  character,
  now,
  assistance,
  gaps,
  actions,
}: {
  character: Character;
  now: NowPill;
  assistance: AssistanceLevel;
  gaps: GapsChip;
  actions: PanelActions;
} = $props();

let tone = $derived(threatTone(now));
let saveError = $state<string | null>(null);
/** The player opened the Gaps chip's list. */
let gapsOpen = $state(false);

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
  <span class="levels">
    {#if gaps.gaps.length > 0}
      <button
        type="button"
        class="chip"
        aria-expanded={gapsOpen}
        title="What the Advisor can't see, and the keys that show it"
        onclick={() => (gapsOpen = !gapsOpen)}
      >
        Gaps {gaps.gaps.length}
      </button>
    {/if}
    <Segmented label="Assistance level" choices={LEVELS} chosen={assistance} {choose} />
  </span>
</header>
{#if gapsOpen && gaps.gaps.length > 0}
  <section class="card gaps">
    <div class="lbl">Gaps <em>what the Advisor can't see</em></div>
    <GapsList {gaps} />
  </section>
{/if}
{#if saveError}
  <p class="error small">{saveError}</p>
{/if}

<style>
/* The toggle wraps below rather than crowd out the character's hit points and turn. */
header {
  display: flex;
  flex-wrap: wrap;
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
  flex: 1 0 auto;
  /* As wide as the header beside the dot, past which it is cut short. */
  max-width: calc(100% - 16px);
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

.levels {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-left: auto;
}

.gaps {
  margin: 8px 14px 0;
}

.chip {
  padding: 2px 8px;
  border: 1px dashed var(--border-strong);
  border-radius: var(--radius-pill);
  background: none;
  color: var(--text-muted);
  font-size: 11px;
  cursor: pointer;
}

.chip[aria-expanded="true"] {
  border-style: solid;
  color: var(--text-strong);
}

.error {
  margin: 0;
  padding: 6px 14px 0;
  color: var(--error);
}
</style>

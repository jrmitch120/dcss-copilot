<script lang="ts">
import type { AssistanceLevel, Character, NowPill } from "$lib/bindings";
import { type PanelActions, unsaved } from "./actions";
import Segmented from "./Segmented.svelte";
import { threatTone } from "./tone";

let {
  character,
  now,
  assistance,
  pickedUpAgain,
  actions,
}: {
  character: Character;
  now: NowPill;
  assistance: AssistanceLevel;
  pickedUpAgain: boolean;
  actions: PanelActions;
} = $props();

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

async function newGame() {
  const error = await actions.newGame();
  saveError = error === null ? null : `It couldn't be made a new game: ${error}`;
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
    <Segmented label="Assistance level" choices={LEVELS} chosen={assistance} {choose} />
  </span>
</header>
{#if pickedUpAgain}
  <p class="picked-up small muted">
    Picked up where you left off.
    <button
      type="button"
      title="Follow this as a new game, remembering nothing of it yet. The game DCSS Copilot took it for keeps what it remembered."
      onclick={newGame}
    >
      This is a new game
    </button>
  </p>
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
  margin-left: auto;
}

.picked-up {
  margin: 0;
  padding: 6px 14px;
  border-bottom: 1px solid var(--divider);
}

.picked-up button {
  padding: 0;
  border: 0;
  background: none;
  color: var(--info);
  text-decoration: underline;
  cursor: pointer;
}

.error {
  margin: 0;
  padding: 6px 14px 0;
  color: var(--error);
}
</style>

<script lang="ts">
import type { Character, NowPill } from "$lib/bindings";

let { character, now }: { character: Character; now: NowPill } = $props();

function stat(value: number | null): string {
  return value === null ? "?" : String(value);
}
</script>

<header>
  <span class="dot {now.state}"></span>
  <span class="who">
    <b>{character.name}</b>
    · {character.species} · HP {stat(character.hp)}/{stat(character.hpMax)}
    · T {stat(character.turn)}
  </span>
</header>

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
</style>

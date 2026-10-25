<script lang="ts">
import type { ChecklistRow, JoinCard } from "$lib/bindings";
import type { PanelActions } from "./actions";

let { join, actions }: { join: JoinCard; actions: PanelActions } = $props();

/** The player chose the lighter dungeon overview over the dump: only what the card asks for. */
let overviewOnly = $state(false);
let closeError = $state<string | null>(null);

async function close() {
  const error = await actions.closeJoinCard();
  closeError = error === null ? null : `The card couldn't be put away: ${error}`;
}
</script>

{#snippet checklist(
  rows: ChecklistRow[],
)}
  <ul class="small">
    {#each rows as row, i (i)}
      <li class:read={row.read} class:muted={!row.read}>{row.read ? "✓" : "○"} {row.line}</li>
    {/each}
  </ul>
{/snippet}

{#if join.step === "asking"}
  <section class="card prompt">
    <div class="lbl">
      Joined mid-game
      {#if join.turn !== null}
        <em>T {join.turn.toLocaleString("en-US")}</em>
      {/if}
    </div>
    <p class="small">
      I can see the map, stats and inventory. I <b>can't</b> see your skills, spells, mutations or
      the other levels yet.
    </p>
    {#if overviewOnly}
      <p class="keys">In the game, press <kbd>Ctrl</kbd><kbd>O</kbd></p>
      <p class="small muted">
        That opens your dungeon overview, which shows the other levels but not your skills, spells
        or mutations; press <kbd>Esc</kbd> to close it. Nothing is sent to the game by DCSS Copilot.
      </p>
    {:else}
      <p class="keys">In the game, press <kbd>?</kbd> then <kbd>#</kbd></p>
      <p class="small muted">
        That opens your character dump; press <kbd>Esc</kbd> to close it. Nothing is sent to the
        game by DCSS Copilot.
      </p>
    {/if}
    <div class="acts">
      <button type="button" class="link" onclick={() => (overviewOnly = !overviewOnly)}>
        {overviewOnly ? "Use ? # instead (everything)" : "Use Ctrl-O instead (levels only)"}
      </button>
      <span class="sp"></span>
      <button type="button" onclick={close}>Not now</button>
    </div>
  </section>
{:else if join.step === "reading"}
  <section class="card prompt">
    <div class="lbl">Reading your character dump</div>
    {@render checklist(join.checklist)}
    <p class="small muted">You can close the dump now.</p>
  </section>
{:else if join.step === "caughtUpFromDump"}
  <section class="card">
    <div class="lbl">Caught up <em>from your dump</em></div>
    {@render checklist(join.checklist)}
    <div class="acts"><button type="button" onclick={close}>OK</button></div>
  </section>
{:else if join.step === "caughtUpFromOverview"}
  <section class="card">
    <div class="lbl">Caught up <em>from your dungeon overview</em></div>
    <p class="small">
      Levels: {join.levels}. Your skills, spells and mutations aren't read yet: press <kbd>?</kbd>
      then <kbd>#</kbd> in the game when you can.
    </p>
    <div class="acts"><button type="button" onclick={close}>OK</button></div>
  </section>
{/if}
{#if closeError}
  <p class="error small">{closeError}</p>
{/if}

<style>
p {
  margin: 0;
}

p + p {
  margin-top: 8px;
}

.keys {
  margin: 10px 0;
  font-size: 14px;
}

ul {
  display: grid;
  gap: 3px;
  margin: 0;
  padding: 0;
  list-style: none;
}

ul + p {
  margin-top: 6px;
}

.read {
  color: var(--calm);
}

.sp {
  flex: 1;
}

.error {
  color: var(--error);
}
</style>

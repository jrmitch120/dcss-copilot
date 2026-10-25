<script lang="ts">
import type { ViewModel } from "$lib/bindings";
import AskBox from "./AskBox.svelte";
import type { PanelActions } from "./actions";
import ConnectionNotice from "./ConnectionNotice.svelte";
import FocusCard from "./FocusCard.svelte";
import Footer from "./Footer.svelte";
import Header from "./Header.svelte";
import NowPill from "./NowPill.svelte";
import RunPlanRail from "./RunPlanRail.svelte";
import SameGameQuestion from "./SameGameQuestion.svelte";
import SuggestedAction from "./SuggestedAction.svelte";
import WaysOut from "./WaysOut.svelte";

/** The whole panel for `view`, in the app's window or a browser's side panel. */
let { view, actions }: { view: ViewModel; actions: PanelActions } = $props();
</script>

<main class="panel">
  {#if view.game}
    <Header
      character={view.game.character}
      now={view.game.now}
      assistance={view.assistance}
      {actions}
    />
  {/if}
  <div class="body">
    <ConnectionNotice connection={view.connection} following={view.game !== null} {actions} />
    {#if view.unreadableGameKnowledge}
      <p class="status error">
        DCSS Copilot can't read its Game Knowledge, so it can't name or judge monsters:
        {view.unreadableGameKnowledge}
      </p>
    {/if}
    {#if view.historyUnsaved}
      <p class="status error">
        DCSS Copilot's game history isn't working, so a game may not be remembered the next time it
        starts: {view.historyUnsaved}
      </p>
    {/if}
    {#if view.game?.pickedUp}
      <SameGameQuestion
        character={view.game.character.name}
        pickedUp={view.game.pickedUp}
        {actions}
      />
    {:else if view.game}
      {#if view.game.memoryUnreadable}
        <p class="status error">
          What DCSS Copilot remembered of this game couldn't be read, so it starts over on it:
          {view.game.memoryUnreadable}
        </p>
      {/if}
      <NowPill now={view.game.now} />
      {#if view.game.suggestedAction}
        <SuggestedAction suggested={view.game.suggestedAction} />
      {/if}
      <FocusCard
        focus={view.game.focus}
        factCards={view.game.factCards}
        appearance={view.appearance}
      />
      {#if view.game.waysOut}
        <WaysOut waysOut={view.game.waysOut} />
      {/if}
    {/if}
  </div>
  {#if view.game && !view.game.pickedUp}
    <RunPlanRail runPlan={view.game.runPlan} />
    <AskBox ask={view.game.ask} />
  {/if}
  <Footer appVersion={view.appVersion} appearance={view.appearance} {actions} />
</main>

<style>
.panel {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background:
    radial-gradient(600px 400px at 100% 0%, var(--bg-glow) 0%, transparent 60%), var(--bg);
}

.status {
  margin: 0;
  font-size: 12.5px;
}

.error {
  color: var(--error);
}

.body {
  flex: 1;
  min-height: 0;
  overflow: auto;
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 12px 14px 16px;
}
</style>

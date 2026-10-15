<script lang="ts">
import "$lib/panel/theme.css";
import { commands, events, type ViewModel } from "$lib/bindings";
import AskBox from "$lib/panel/AskBox.svelte";
import ConnectionNotice from "$lib/panel/ConnectionNotice.svelte";
import FocusCard from "$lib/panel/FocusCard.svelte";
import Footer from "$lib/panel/Footer.svelte";
import Header from "$lib/panel/Header.svelte";
import NowPill from "$lib/panel/NowPill.svelte";
import RunPlanRail from "$lib/panel/RunPlanRail.svelte";

let view = $state<ViewModel | null>(null);

$effect(() => {
  // Listen before asking, so no push between the two is missed.
  const unlisten = events.viewModelChanged.listen((event) => {
    view = event.payload;
  });
  commands
    .viewModel()
    .then((v) => {
      view ??= v;
    })
    .catch((e: unknown) => {
      console.error("Could not load the view model", e);
    });
  return () => {
    unlisten.then((stop) => stop());
  };
});
</script>

<main class="panel">
  {#if view}
    {#if view.game}
      <Header character={view.game.character} now={view.game.now} />
    {/if}
    <div class="body">
      <ConnectionNotice connection={view.connection} following={view.game !== null} />
      {#if view.unreadableGameKnowledge}
        <p class="status error">
          DCSS Copilot can't read its Game Knowledge, so it can't name or judge monsters:
          {view.unreadableGameKnowledge}
        </p>
      {/if}
      {#if view.game}
        <NowPill now={view.game.now} />
        <FocusCard focus={view.game.focus} appearance={view.appearance} />
      {/if}
    </div>
    {#if view.game}
      <RunPlanRail runPlan={view.game.runPlan} />
      <AskBox ask={view.game.ask} />
    {/if}
    <Footer appVersion={view.appVersion} appearance={view.appearance} />
  {/if}
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

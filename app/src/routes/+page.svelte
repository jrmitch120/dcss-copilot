<script lang="ts">
import "$lib/panel/theme.css";
import { commands, type ViewModel } from "$lib/bindings";
import { failure } from "$lib/command";
import type { PanelActions } from "$lib/panel/actions";
import Panel from "$lib/panel/Panel.svelte";
import { followViewModel } from "$lib/view-model";

const ACTIONS: PanelActions = {
  setAppearance: async (appearance) => failure(await commands.setAppearance(appearance)),
  setAssistance: async (assistance) => failure(await commands.setAssistance(assistance)),
  openSettings: async () => failure(await commands.openSettings()),
  answerBrowser: async (allow) => failure(await commands.answerBrowser(allow)),
};

let view = $state<ViewModel | null>(null);

$effect(() =>
  followViewModel((v) => {
    view = v;
  }),
);
</script>

{#if view}
  <Panel {view} actions={ACTIONS} />
{/if}

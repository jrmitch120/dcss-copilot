<script lang="ts">
import type { Connection } from "$lib/bindings";
import type { PanelActions } from "./actions";

let {
  connection,
  following,
  actions,
}: { connection: Connection; following: boolean; actions: PanelActions } = $props();

let answerError = $state<string | null>(null);

async function answer(answerBrowser: (allow: boolean) => Promise<string | null>, allow: boolean) {
  answerError = await answerBrowser(allow);
}
</script>

{#if connection.state === "allowBrowser"}
  <section class="card prompt">
    <div class="lbl">Allow this browser?</div>
    <p class="small">
      A browser extension wants to relay your games to DCSS Copilot. Allow it only if you just
      installed the DCSS Copilot extension.
    </p>
    <p class="origin">{connection.browser}</p>
    {#if actions.answerBrowser}
      {@const answerBrowser = actions.answerBrowser}
      <div class="acts">
        <button type="button" class="pri" onclick={() => answer(answerBrowser, true)}>Allow</button>
        <button type="button" onclick={() => answer(answerBrowser, false)}>Don't allow</button>
      </div>
    {:else}
      <p class="small">Answer in the DCSS Copilot app.</p>
    {/if}
    {#if answerError}
      <p class="error small">
        Your answer couldn't be saved, so you'll be asked again: {answerError}
      </p>
    {/if}
  </section>
{:else if connection.state === "relayUnavailable"}
  <p class="status error">Browsers can't connect: {connection.reason}</p>
{:else if connection.state === "noBrowser"}
  <p class="status">
    Waiting for your browser. Open a webtiles game with the DCSS Copilot extension installed.
  </p>
{:else if connection.bufferOverflowed}
  <p class="status warning">
    This game started too long ago to catch up on. Reload the game page to resync: reloading sends
    no game input.
  </p>
{:else if !following}
  <p class="status">Connected. Waiting for a game.</p>
{/if}

<style>
p {
  margin: 0;
}

.prompt {
  border-color: rgba(162, 155, 254, 0.45);
  background: linear-gradient(180deg, rgba(162, 155, 254, 0.1), var(--surface-low));
}

.prompt p + p {
  margin-top: 8px;
}

.origin {
  font-family: var(--mono);
  font-size: 0.8rem;
  color: var(--text-muted);
  word-break: break-all;
}

.acts {
  display: flex;
  gap: 6px;
  margin-top: 8px;
}

.acts button {
  padding: 3px 9px;
  border: 0;
  border-radius: 7px;
  background: rgba(255, 255, 255, 0.07);
  color: var(--text-soft);
  font-size: 11.5px;
  cursor: pointer;
}

.acts button.pri {
  background: rgba(255, 255, 255, 0.9);
  color: #111111;
  font-weight: 600;
}

.status {
  color: var(--text-muted);
  font-size: 12.5px;
}

.warning {
  color: var(--warning);
}

.error {
  color: var(--error);
}
</style>

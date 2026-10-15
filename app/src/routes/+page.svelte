<script lang="ts">
import { commands, events, type ViewModel } from "$lib/bindings";

let view = $state<ViewModel | null>(null);
let answerError = $state<string | null>(null);

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

async function answerBrowser(allow: boolean) {
  const result = await commands.answerBrowser(allow);
  answerError = result.status === "error" ? result.error : null;
}

function stat(value: number | null): string {
  return value === null ? "?" : String(value);
}
</script>

<main>
  {#if view}
    {@const connection = view.connection}
    {#if connection.state === "allowBrowser"}
      <section class="card prompt">
        <h2>Allow this browser?</h2>
        <p>
          A browser extension wants to relay your games to DCSS Copilot. Allow it only if you just
          installed the DCSS Copilot extension.
        </p>
        <p class="origin">{connection.browser}</p>
        <div class="actions">
          <button type="button" onclick={() => answerBrowser(true)}>Allow</button>
          <button type="button" class="secondary" onclick={() => answerBrowser(false)}>
            Don't allow
          </button>
        </div>
        {#if answerError}
          <p class="error">
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
        This game started too long ago to catch up on. Reload the game page to resync: reloading
        sends no game input.
      </p>
    {/if}

    {#if view.character}
      <section class="card character">
        <h1>{view.character.name}</h1>
        <p class="species">{view.character.species}</p>
        <dl>
          <dt>HP</dt>
          <dd>{stat(view.character.hp)}/{stat(view.character.hpMax)}</dd>
          <dt>Turn</dt>
          <dd>{stat(view.character.turn)}</dd>
        </dl>
      </section>
    {:else if connection.state === "connected"}
      <p class="status">Connected. Waiting for a game.</p>
    {/if}

    <footer>v{view.appVersion}</footer>
  {/if}
</main>

<style>
:global(body) {
  margin: 0;
  background: #14161a;
  color: #e6e6e6;
  font-family: system-ui, sans-serif;
}

main {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 12px;
}

.card {
  background: #1e2127;
  border-radius: 8px;
  padding: 12px 16px;
}

h1,
h2 {
  margin: 0;
}

h1 {
  font-size: 1.25rem;
}

h2 {
  font-size: 1rem;
}

.species {
  margin: 2px 0 8px;
  color: #a0a4ab;
}

dl {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 4px 12px;
  margin: 0;
}

dt {
  color: #a0a4ab;
}

dd {
  margin: 0;
  font-variant-numeric: tabular-nums;
}

.origin {
  font-family: ui-monospace, monospace;
  font-size: 0.8rem;
  color: #a0a4ab;
  word-break: break-all;
}

.actions {
  display: flex;
  gap: 8px;
}

button {
  border: none;
  border-radius: 6px;
  padding: 6px 12px;
  background: #3d7be0;
  color: white;
  cursor: pointer;
}

button.secondary {
  background: #2c3038;
}

.status {
  margin: 0;
  color: #a0a4ab;
}

.warning {
  color: #e0b23d;
}

.error {
  color: #e0563d;
}

footer {
  color: #6b7079;
  font-size: 0.75rem;
}
</style>

<script lang="ts">
import "$lib/panel/theme.css";
import { commands, events, type SettingsView } from "$lib/bindings";
import About from "$lib/settings/About.svelte";
import ConnectionAndWindow from "$lib/settings/ConnectionAndWindow.svelte";
import History from "$lib/settings/History.svelte";
import ModelsAndKeys from "$lib/settings/ModelsAndKeys.svelte";

type SectionId = "models" | "window" | "history" | "about";

/** Each section, with the words a search finds it by. Layout follows round2.html
    (prototype/setup-settings). */
const SECTIONS: {
  id: SectionId;
  group: string;
  icon: string;
  colour: string;
  title: string;
  words: string;
}[] = [
  {
    id: "models",
    group: "Advice",
    icon: "✦",
    colour: "var(--info)",
    title: "Models and keys",
    words: "quick deep model thinking provider anthropic api key keychain test replace remove",
  },
  {
    id: "window",
    group: "Setup",
    icon: "⇄",
    colour: "#fd79a8",
    title: "Connection and window",
    words: "side panel sidebar toolbar start login tray",
  },
  {
    id: "history",
    group: "Trust",
    icon: "⟲",
    colour: "var(--calm)",
    title: "History",
    words: "game history remember forget prune days clear delete saved games",
  },
  {
    id: "about",
    group: "Trust",
    icon: "i",
    colour: "var(--text-label)",
    title: "About",
    words: "version licence license gpl crawl devteam source",
  },
];

let view = $state<SettingsView | null>(null);
let open = $state<SectionId>("models");
let query = $state("");

const matches = $derived.by(() => {
  const q = query.trim().toLowerCase();
  return q === ""
    ? SECTIONS
    : SECTIONS.filter((s) => `${s.title} ${s.words}`.toLowerCase().includes(q));
});
const groups = $derived([...new Set(matches.map((s) => s.group))]);
const shown = $derived(query.trim() === "" ? SECTIONS.filter((s) => s.id === open) : matches);

$effect(() => {
  // Listen before asking, so no push between the two is missed.
  const unlisten = events.settingsChanged.listen((event) => {
    view = event.payload;
  });
  commands
    .settingsView()
    .then((v) => {
      view ??= v;
    })
    .catch((e: unknown) => {
      console.error("Could not load the settings", e);
    });
  return () => {
    unlisten.then((stop) => stop());
  };
});

function choose(id: SectionId) {
  open = id;
  query = "";
}
</script>

<div class="window">
  <nav class="side">
    <input
      class="search"
      type="search"
      placeholder="Search: key, model, login, history, version…"
      bind:value={query}
    >
    {#each groups as group (group)}
      <div class="group">{group}</div>
      {#each matches.filter((s) => s.group === group) as section (section.id)}
        <button
          type="button"
          class:on={query.trim() === "" && section.id === open}
          onclick={() => choose(section.id)}
        >
          <span class="icon" style:color={section.colour}>{section.icon}</span>
          <span>{section.title}</span>
        </button>
      {/each}
    {/each}
  </nav>
  <main>
    {#if view}
      {#each shown as section (section.id)}
        <section>
          <h2>{section.title}</h2>
          {#if section.id === "models"}
            <ModelsAndKeys models={view.models} />
          {:else if section.id === "window"}
            <ConnectionAndWindow window={view.window} />
          {:else if section.id === "history"}
            <History history={view.history} />
          {:else}
            <About appVersion={view.appVersion} />
          {/if}
        </section>
      {:else}
        <p class="muted">Nothing matches “{query}”.</p>
      {/each}
    {/if}
  </main>
</div>

<style>
.window {
  display: grid;
  grid-template-columns: 230px 1fr;
  height: 100vh;
  background: var(--bg);
}

.side {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 18px 12px;
  overflow: auto;
  background: rgba(255, 255, 255, 0.025);
  border-right: 1px solid var(--divider);
}

.search {
  margin-bottom: 8px;
  padding: 6px 10px;
  border: 1px solid var(--border-strong);
  border-radius: 8px;
  background: var(--inset);
  font-size: 12.5px;
  outline: none;
}

.group {
  padding: 10px 8px 4px;
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 0.14em;
  text-transform: uppercase;
  color: var(--text-faint);
}

.side button {
  display: grid;
  grid-template-columns: 22px 1fr;
  gap: 8px;
  align-items: center;
  padding: 7px 8px;
  border: 0;
  border-radius: 8px;
  background: none;
  text-align: left;
  font-size: 13px;
  color: #b9bdc9;
  cursor: pointer;
}

.side button:hover {
  background: rgba(255, 255, 255, 0.04);
}

.side button.on {
  background: rgba(255, 255, 255, 0.08);
  color: var(--text-strong);
}

.icon {
  text-align: center;
}

main {
  display: flex;
  flex-direction: column;
  gap: 26px;
  padding: 22px 28px;
  overflow: auto;
}

section {
  max-width: 600px;
}

h2 {
  margin: 0 0 12px;
  font-size: 18px;
  font-weight: 600;
}
</style>

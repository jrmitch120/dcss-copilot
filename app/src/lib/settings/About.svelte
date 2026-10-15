<script lang="ts">
let { appVersion }: { appVersion: string } = $props();
</script>

<p class="small">
  DCSS Copilot {appVersion} · GPL-3.0-or-later. Monster, spell and item text and several formulas
  come from Dungeon Crawl Stone Soup, © the DCSS devteam (GPL-2.0-or-later).
</p>
<p class="small muted">Source: <span class="mono">github.com/jrmitch120/dcss-copilot</span></p>

<style>
p {
  margin: 0 0 8px;
}

.mono {
  font-family: var(--mono);
}
</style>

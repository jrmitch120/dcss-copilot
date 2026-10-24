import { readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { svelte, vitePreprocess } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "wxt";
import { WEBTILES_MATCHES } from "./lib/servers";

/** The app's panel, which the side panel shows: its components, types and theme. */
const APP_LIB = resolve(import.meta.dirname, "../app/src/lib");
/** The app's DCSS tiles, which the panel draws from `/tiles/…`. */
const APP_TILES = resolve(import.meta.dirname, "../app/static/tiles");

// See https://wxt.dev/api/config.html
export default defineConfig({
  alias: { $lib: APP_LIB },
  vite: () => ({ plugins: [svelte({ preprocess: vitePreprocess() })] }),
  manifest: {
    name: "DCSS Copilot",
    description:
      "Shows DCSS Copilot's advice beside your Dungeon Crawl Stone Soup webtiles game. Read-only: it never sends input.",
    // A toolbar button, which opens the side panel.
    action: { default_title: "DCSS Copilot" },
    // The page script is injected by the relay content script on webtiles pages.
    web_accessible_resources: [{ resources: ["relay-page.js"], matches: WEBTILES_MATCHES }],
  },
  hooks: {
    "build:publicAssets": (_, files) => {
      for (const tile of readdirSync(APP_TILES, { recursive: true, encoding: "utf8" })) {
        if (tile.endsWith(".png")) {
          files.push({ absoluteSrc: join(APP_TILES, tile), relativeDest: join("tiles", tile) });
        }
      }
    },
  },
});

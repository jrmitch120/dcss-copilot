import { defineConfig } from "wxt";

// See https://wxt.dev/api/config.html
export default defineConfig({
  manifest: {
    name: "DCSS Copilot",
    description:
      "Relays your Dungeon Crawl Stone Soup webtiles game to the DCSS Copilot app. Read-only: it never sends input.",
  },
});

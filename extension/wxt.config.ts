import { defineConfig } from "wxt";
import { WEBTILES_MATCHES } from "./lib/servers";

// See https://wxt.dev/api/config.html
export default defineConfig({
  manifest: {
    name: "DCSS Copilot",
    description:
      "Relays your Dungeon Crawl Stone Soup webtiles game to the DCSS Copilot app. Read-only: it never sends input.",
    // The page script is injected by the relay content script on webtiles pages.
    web_accessible_resources: [{ resources: ["relay-page.js"], matches: WEBTILES_MATCHES }],
  },
});

# GPL-3.0-or-later to ship crawl data and ported formulas

DCSS Copilot is licensed GPL-3.0-or-later, not MIT, because it is built on crawl (GPL-2.0-or-later). Game Knowledge ships crawl's description text verbatim, and the deterministic analysis ports crawl's combat and spell formulas closely so its numbers match the game exactly and new versions can be diffed mechanically. Both are derivative works, so the distributed app has to be GPL. We picked v3 over crawl's own v2-or-later so we can depend on Apache-2.0-only libraries, and one license covers the whole repo, including the relay-only extension that has no crawl code in it. The project will never be closed or sold, so we gave up nothing by doing this.

## Considered Options

- **Keep the app MIT, with extracted data in a separate GPL package:** works for data loaded at runtime, but not for ported formula code linked into the Rust core, so we'd have needed a clean-room path for the formulas anyway.
- **Facts and clean-room reimplementation only:** no crawl prose (thinner Information cards, relying on CrawlWiki links), and formulas rebuilt from descriptions by someone who has read the source, which is legally weak, drifts from the real numbers, and turns each version update into guesswork.
- **GPL-2.0-or-later, matching crawl:** would let crawl upstream our code unchanged, but rules out Apache-2.0-only dependencies. If crawl ever wants the extractor, we can dual-license that piece.

## Consequences

- No CLA or DCO. Contributions come in under GPL-3.0-or-later.
- Each Game Knowledge bundle records the crawl tag or commit it was extracted from, along with crawl's copyright and license. Its corresponding source is our extractor plus that tag.
- The About screen credits the DCSS devteam and the tile artists, and states our license. `NOTICE` ships in the repo and in installers, and binaries link to the public source.
- qw (no license) remains a design reference only, and CrawlWiki (GFDL) is linked or paraphrased, never bulk-copied. GPL doesn't change either.

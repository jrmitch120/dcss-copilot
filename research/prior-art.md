# Prior art: DCSS tools, bots and community data we can reuse

Ticket: #5. Researched 2026-10-13 against primary sources: GitHub repos via the GitHub API, live server logfiles, and the CrawlWiki MediaWiki API. Licenses come from each repo's GitHub license detection and LICENSE file. "none" means the repo has no license file, so by default all rights are reserved.

Related tickets that cover adjacent ground, so this file stays brief on them:
- #2: the Webtiles protocol
- #3: server policy
- #4: extracting Game Knowledge from crawl source

## Answer in brief

- **Game Knowledge:** crawl's own repo is the authoritative source. Monster, species, job and form data are now YAML files. The dcss-stats extractor already turns monster data into a JSON catalog. Crawl is GPL-2.0+.
- **Tactical Advice:** qw is the only maintained bot with a real threat model. It has per-monster threat, an XL-scaled "scary monsters" table with resist factors, and emergency plans. The repo has **no license**, so treat it as a design reference only. Don't copy its code.
- **Strategic Advice and death statistics:** public server **logfiles** record every game end (killer, XL, place). **Milestone** files record events such as branch entry and unique kills. They are plain HTTP files, listed in Sequell's `sources.yml`. go-sequell (MIT) is a ready-made loader into Postgres. dcss-stats (MIT) is a second, simpler ingest pipeline. It stores games only, not milestones.
- **Information:** CrawlWiki text is GFDL. It has a working MediaWiki API, but its license is awkward for redistribution. Treat it as a link target or a source to paraphrase, not one to copy.
- **Game Source and capture:** dcss-api (MPL-2.0, Rust/Python) and DWEM (no license) are the useful prior art for reading the Webtiles websocket. See #2.
- **Gaps:** no existing tool is a read-only advisor or companion. There is no public LearnDB dump, no public bulk morgue corpus (the 1.5M-morgue S3 tarball now returns 403), and no aggregated "deaths by XL" or "branch order" dataset. We would compute these ourselves from logfiles and milestones.

## Catalogue

| Asset | What is reusable | Access | License | Use for |
|---|---|---|---|---|
| crawl/crawl | Monster YAML (`crawl-ref/source/dat/mons/*.yaml`, 689 files); `dat/species`, `dat/jobs`, `dat/forms`; `spl-data.h`, `art-data.txt`, `mutation-data.h`, `feature-data.h`; descriptions in `dat/descript/*.txt`; `util/monster` CLI (the `monster` lookup tool) | git, tagged per version | GPL-2.0-or-later; most tiles are CC0 (crawl/tiles) | Game Knowledge (#4) |
| crawl/qw | Lua bot: threat model (`source/monsters.lua`), emergency, retreat and kiting plans (`plans-emergency.lua`, `move-retreat.lua`, `move-kiting.lua`, `move-flee.lua`), goal and branch-order config (`qw.rc`), god plans | git | **none** (no LICENSE) | Design reference for Tactical and Strategic Advice |
| crawl/sequell + crawl/go-sequell | Logfile and milestone schema and loader (`seqdb fetch && seqdb load`); `config/sources.yml` lists every public server's logfile, milestone, morgue and ttyrec URLs | git; live bot `Sequell` in ##crawl on Libera IRC | sequell: **none**; go-sequell: MIT | Death and branch statistics pipeline |
| Public server logfiles and milestones | One line per game end or milestone, colon-separated `key=value` | Plain HTTP from each server (see the server list below) | No stated license (public server data) | Strategic Advice priors, threat calibration |
| Public morgue files | Full end-of-game character dumps | Per player, e.g. `https://crawl.akrasiac.org/rawdata/<name>/` (the root listing returns 403) | No stated license | Parser test fixtures, deeper analysis |
| O4epegb/dcss-stats (dcss-stats.com) | TS/Prisma logfile ingest; `Game` model (killer, xl, branch, lvl, god, skills...); API routes (route modules `games`, `matrix`, `suggest`, `monsters`, `players`...); **monster catalog extractor** that patches crawl's `util/monster` to export JSON | git; public site and API | MIT | Ingest code to reuse; monster JSON schema |
| EricFecteau/dcss-api | Rust and Python Webtiles websocket client (0.29-0.34); experimental `dcss-data` model for Webtiles messages; protocol docs at ericfecteau.ca/dcss/dcss-api-docs | crates.io, PyPI, git | MPL-2.0 | Game Source client, message model (#2, #8) |
| refracta/dcss-webtiles-extension-module (DWEM) | Userscript framework that hooks Webtiles RequireJS; `io-hook` module intercepts every websocket message before and after the client handles it; also a `spectator-module` and `map-predictor` | git, Greasy Fork; preinstalled on the CNC server | **none** | Proof that in-browser read-only capture works (#2, #8) |
| dtdannen/dcss-ai-wrapper | Python agent API with vector and symbolic state from Webtiles; research papers (AAAI 2019, ICAPS PRL 2021) | git, PyPI | MIT | State-representation ideas; dated (recommends 0.23) |
| CrawlWiki (crawl.chaosforge.org) | About 3,200 articles and still edited daily; MediaWiki 1.30 API (`api.php`) | HTTP API | GFDL; source excerpts are GPL | Information links and paraphrase |
| LearnDB | Community term and definition knowledge base, queried with `??term` | Only through Sequell/IRC (or beem in Webtiles chat); repo has the schema only | Unstated | Hints and lore; gap: no dump |
| gammafunk/beem | Relays Sequell, Gretell and Cheibriados queries into Webtiles chat | git | GPL-2.0 | Pattern for querying Sequell without IRC |
| colinmorris/crawl-coroner | Morgue parser and pandas notebooks over 1.5M CAO morgues | git; tarball `s3.amazonaws.com/dcss/morgues.tar.gz` now **403** | **none** | Parsing ideas only |
| rwbarton/rw | Older DCSS bot (2016) | git | BSD-3-Clause | Low value |
| crawl/dcss_tourney, zxc23/dcss-scoreboard | Logfile-driven scoring scripts | git | none | Low value |
| Player rc files (gammafunk/dcss-rc, etc.) | Lua UI helpers (force_more on dangerous monsters, and so on) | git | none | Ideas for "dangerous monster" lists |

## Details

### qw (crawl/qw): the tactical prior art

- The README calls it "the first bot to win DCSS with no human assistance". It is "developed and maintained by the DCSS devteam" and runs as clua inside crawl through an rcfile. Its source tree is `source/*.lua`. Neither crawl/qw nor the original elliptic/qw has a license file.
- **Threat model** (`source/monsters.lua`):
  - The base is `mons.minfo:threat()`, crawl's own monster_info threat level, which clua exposes.
  - A `scary_monsters` table adds `entry.xl - you.xl()` when the player is below a threshold XL, then scales by the player's resist percentage. Examples: `["ice beast"] = { xl = 7, resists = { rC = 0.75 } }`, `["fire giant"] = { xl = 24, resists = { rF = 0.75 } }`.
  - Multipliers: berserk 3.375, strong or hasted 1.5, slowed 2/3. Player durations (heroism, finesse, haste, slow, might, weak) also adjust the result.
  - `assess_enemies` sums threat over enemies in LOS that are ranged or have a melee path. The thresholds are moderate 5, high 10 and extreme 20, and are lowered for XL < 10.
  - This scheme maps almost directly onto the Advisor's "how dangerous is this screen" Tactical Advice.
- **Emergency plans** (`plans-emergency.lua`): `want_to_teleport`, `want_to_heal_wounds`, `want_to_haste`, `want_to_berserk`, `want_resistance`, `plan_cure_bad_poison`, `want_to_trogs_hand`, plus god-ability triggers. This is a ready checklist of "which consumable or ability now" decisions.
- **Strategic config** (`qw.rc`): `GOALS` strings such as `"Normal, Zot:1-4, Slime:5, God:TSO, Crypt, Tomb, Pan, Hells, Abyss, Win"`, plus `RUNE_PREFERENCE` (`"smart"` = Spider > Snake/Swamp > Shoals), `LATE_ORC`, `EARLY_SECOND_RUNE` and `ALLOWED_PORTALS`. This is a compact, expert-authored branch-order heuristic.
- **Reuse stance:** qw has no license, and the Advisor is MIT (this repo's LICENSE). Re-implement the ideas, don't copy the code, unless the devteam adds a license. Weights that qw borrows from crawl (`minfo:threat()`) are better pulled from crawl itself (#4).

### Logfiles and milestones: deaths by XL and branch order

- Sequell's `config/sources.yml` is the canonical list of public servers and their file globs. The current entries include:
  - cao: `crawl.akrasiac.org`
  - cdo: `crawl.develz.org`
  - cue: `underhound.eu`
  - cwz: `webzook.net`
  - cbr2: `cbro.berotato.org`
  - cxc: `crawl.xtahua.com`
  - lld: `lazy-life.ddo.jp`
  - cpo: `crawl.project357.org`
  - cko: `crawl.kelbi.org`
  - cdi: `crawl.dcss.io/crawl/meta/`
  - cnc: `archive.nemelex.cards/meta/`

  The same file also has morgue and ttyrec URL rules for each server.
- Live check: `https://crawl.akrasiac.org/logfile-git` is about 1.2 GB (Content-Length 1198201713, modified 2026-10-14). CDI publishes per-version directories (`crawl-0.30/` … `crawl-0.34/`, `crawl-git/`, and bot variants).
- **Logfile line** (one per finished game), sampled from CAO:
  `...:xl=3:...:place=D::3:br=D:lvl=3:...:ktyp=mon:killer=an orc:dam=...:kaux=...:tmsg=slain by a kobold:vmsg=...`
  The fields needed for "what kills characters at which XL" are present: `killer`, `ktyp`, `xl`, `place`/`br`/`lvl`, `race`, `cls`, `god` and `v`.
- **Milestone line** has the same prefix plus `type=` and `milestone=`, e.g. `type=uniq:milestone=killed Menkaure.` and `type=death`. Branch entry is recorded as a milestone `type` (Sequell queries it as `br.enter`). That makes **branch order** data derivable per game by ordering milestones on `start` + `turn`.
- **Ways to reuse:**
  1. Run go-sequell (MIT) to fetch and load everything into Postgres.
  2. Reuse dcss-stats' TS ingest (MIT). It stores games only (`Game` model: `killer`, `killerType`, `xl`, `branch`, `lvl`, `god`, `fifteenskills`...), with no milestones.
  3. Write a small loader. The format is trivial.

  Querying the live Sequell bot over IRC (`!lg`) works for ad-hoc checks but is not a data feed.
- **Gap:** nobody publishes precomputed "death rate by monster × XL" or "branch order vs. win" tables. We would build them, ideally per version.

### Game Knowledge sources (overlap with #4)

- crawl's `dat/mons/README.md` documents the YAML monster schema (ac, ev, hd, hp_10x, attacks, flags, spells, resists, will, speed/energy...). `mon-data.h` is generated from it. A sample `orc-warrior.yaml` has `hd: 4`, `hp_10x: 280`, `attacks: [{type: hit, damage: 12}]` and `will: 20`.
- dcss-stats `apps/extractor` checks out crawl at a pinned commit, builds a patched `util/monster/monster-export`, and writes `monsters.json`. The `MonsterData` interface has hp, ac, ev, attacks with flavour, spells with level and frequency, resistances, flags, willpower and tile path. The extractor is MIT, but the data comes from GPL crawl. This is the closest existing "Game Knowledge per version" pipeline.

### Webtiles capture prior art (overlap with #2 and #8)

- **dcss-api (MPL-2.0):** Rust/Python client covering login, starting a game and sending commands for 0.29-0.34, plus an experimental `dcss-data` message model. Public servers need `speed_ms` ≥ 100. It is built to play games. The Advisor would use only its read path, and its parsing of `map`/`player`/`msgs` messages is the reusable part.
- **DWEM:** its `io-hook` wraps the Webtiles client's `handle_message` and `send_message` with before/after hooks. That is exactly the read-only interception point a browser-extension Game Source needs. It ships on the CNC server already. It has no license, so use it as a reference.
- **dcss-ai-wrapper (MIT):** Webtiles agent API with symbolic state. It targets old versions (0.23), so it is mainly of academic interest.

### Information content

- **CrawlWiki:**
  - API: `http://crawl.chaosforge.org/api.php`
  - Stats: 12,408 pages, 3,235 articles; last edit 2026-10-13
  - License (CrawlWiki:Copyrights): "Most pages of CrawlWiki are licensed under the GNU Free Documentation License … source code and screenshots … GNU GPL V2 or later."

  GFDL redistribution needs the license text and history. Prefer linking to the wiki, or generating our own text from crawl data.
- **LearnDB:** Sequell's repo has only the schema (`config/learndb.sql`, tables `terms`/`definitions`). The content lives on the Sequell host and is reachable only through `??term` queries on IRC or in Webtiles chat via beem. There is no public dump and no stated license.

## Things that don't exist (opportunity or gap)

- No public read-only companion, overlay or advisor for DCSS was found. GitHub search for "dcss" and "dungeon crawl stone soup" turned up bots, rc files, ports, stats sites and an AI-research wrapper. `powerbf/crawl-helper` ("Assistant for DCSS") shows in search results but returns 404 on the API, so it is private or deleted.
- No LLM-based DCSS advisor or player was found as open source.
- No maintained public morgue corpus: the colinmorris tarball returns 403. Morgues are only reachable per player on each server.

## Not verified

- The dcss-stats.com public API responses. TLS verification failed from this environment, so only the code was read. Rate limits and terms are unknown; see #3.
- Whether the milestone `type` values (`br.enter`, `br.end`, `rune`, `orb`…) are identical across all current versions. Only CAO samples from an old version were read. Check against crawl's `source/` milestone writer when building the loader.
- Whether the devteam would license qw if asked.

## Sources

- crawl/crawl: https://github.com/crawl/crawl (LICENSE; `crawl-ref/source/dat/mons/README.md`; `crawl-ref/source/util/`)
- crawl/qw: https://github.com/crawl/qw (README.md, `qw.rc`, `source/monsters.lua`, `source/plans-emergency.lua`)
- crawl/sequell: https://github.com/crawl/sequell (README.md, `config/sources.yml`, `config/learndb.sql`, CREDITS)
- crawl/go-sequell: https://github.com/crawl/go-sequell (MIT)
- CAO logfile and milestones: https://crawl.akrasiac.org/logfile-git, https://crawl.akrasiac.org/milestones-git
- CDI meta: https://crawl.dcss.io/crawl/meta/
- O4epegb/dcss-stats: https://github.com/O4epegb/dcss-stats (`apps/api/prisma/schema.prisma`, `apps/api/src/app/routes/`, `apps/extractor/src/monsterCatalog.ts`)
- EricFecteau/dcss-api: https://github.com/EricFecteau/dcss-api
- refracta/dcss-webtiles-extension-module: https://github.com/refracta/dcss-webtiles-extension-module (`modules/io-hook/index.js`)
- dtdannen/dcss-ai-wrapper: https://github.com/dtdannen/dcss-ai-wrapper
- CrawlWiki copyrights: http://crawl.chaosforge.org/CrawlWiki:Copyrights (via `api.php?action=parse`)
- colinmorris/crawl-coroner: https://github.com/colinmorris/crawl-coroner
- gammafunk/beem: https://github.com/gammafunk/beem
- rwbarton/rw: https://github.com/rwbarton/rw

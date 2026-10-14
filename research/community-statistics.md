# Community statistics: what public logfiles and milestones can yield

Ticket: "Community statistics sources: what public logfiles and milestones can yield" (child of the map). Researched 2026-10-14.

Primary sources:
- Sequell's server list, `config/sources.yml` in crawl/sequell (last changed 2025-05-02), and its field and milestone config, `config/crawl-data.yml`.
- crawl's own writer for both files, at tag `0.34.1`: `crawl-ref/source/hiscores.cc`, `ouch.cc` and `tags.cc` (`make_date_string`).
- go-sequell (MIT), the loader: `httpfetch/httpfetch.go`, `loader/loader.go` and `crawl/ctime/ctime.go`.
- dcss-stats (MIT): `apps/api/prisma/seedData.ts`, its server list and bot list.
- The live servers. I sent a HEAD request to every logfile and milestone URL that `sources.yml` expands to (1,173 URLs). I also made Range reads of 1 MB or less from the ends of about 20 active files. No whole file was downloaded.

## Answer in brief

- **Size.** The 12 servers that still answer publish about **62 GB**: 10.2 GB of logfiles (about 16M finished games) and 51.9 GB of milestones. Normal games are 9.5 GB of the logfiles and 50.5 GB of the milestones; Sprint and ZotDef make up the rest.
- **Growth.** Together the servers log about **5,000 finished games a day**. That is about 3 MB a day of logfile and roughly 15–20 MB a day of milestones, or about 7 GB a year.
- **Feasible tables.** Death share by XL and place, death share by species and background, top killers per XL, god choice, branch order and rune order can all be computed. Logfiles alone cover the death and final-god tables. Branch order, rune order and the god the player first worshipped need milestones.
- **Splitting by version works.** Since 0.30 each stable version has 0.2–0.4 GB of logfile, which is 350k–650k games. That is plenty for XL × branch and for species × XL. It is thin for species × background × XL, which needs pooling or minimum counts.
- **Cost.** Downloading is the bottleneck. A full rebuild means about 62 GB at the 3–5 MB/s each server gave us. It takes about 2 hours with one connection per server, because CAO alone holds 19 GB. A rebuild from logfiles only is about 10 GB, roughly 20 minutes. Parsing is cheap: a plain Python parser ran at about 53 MB/s on an M1 Pro, so all 62 GB parse in about 20 minutes of CPU time. An incremental refresh is a Range request from the last byte offset per file (this is what go-sequell does), about 20 MB a day.
- **Shipped size.** Aggregated tables are small, from kilobytes to low megabytes. Raw data should never ship.
- **Pitfalls.** Sequell's list is **stale**: it has no 0.34 files and no CRG servers, and 7 of its servers are dead. Trunk (`git`) files mix many versions, so split by the `v` field and not by file name. The month in timestamps is **0-based**. Bots (qw and others) play under known names. Wizard and explore games carry flags. Quits and "leaving" are not deaths. Custom seeds cannot be told apart from random seeds. Old versions have species, branches and fields that no longer exist.
- **Terms of use.** No server we checked publishes terms for these files. Their robots.txt files do not block them; CAO blocks only `/rawdata`, `/scoring-data` and player pages. The lines contain player names, so ship aggregates only.

## 1. Sources

### Servers that publish logfiles and milestones

From Sequell `config/sources.yml`, plus two servers listed only by dcss-stats (CRG and CRG-BR). Sizes are the HEAD `Content-Length` summed over every URL that answered on 2026-10-14. They include all game modes.

| Server | Base URL | Live? | Logfiles | Milestones | Versions |
|---|---|---|---|---|---|
| CAO | `http://crawl.akrasiac.org/` (`logfile33`, `logfile-git`, `milestones34`, ...) | yes | 3.40 GB | 16.02 GB | 0.1–0.34 + git (also `allgames.txt`, pre-0.4 scoring, frozen 2014) |
| CUE | `https://underhound.eu/crawl/meta/<ver>/{logfile,milestones}` | yes | 1.43 GB | 7.80 GB | 0.10–0.34 + git |
| CXC | `https://crawl.xtahua.com/crawl/meta/<ver>/...` | yes | 1.16 GB | 6.93 GB | 0.14–0.34 + git |
| CBR2 | `https://cbro.berotato.org/meta/<ver>/...` | yes | 1.17 GB | 5.98 GB | 0.24–0.34 + git |
| CBRO (old) | `http://crawl.berotato.org/crawl/meta/<ver>/...` | frozen (last write 2020-12-31) | 1.10 GB | 5.42 GB | 0.13–0.25 + git + experimental branches |
| CNC | `https://archive.nemelex.cards/meta/crawl-<ver>/...` | yes | 0.47 GB | 3.05 GB | 0.11–0.34 + git |
| CPO | `https://crawl.project357.org/dcss-{logfile,milestones}-<ver>` | yes | 0.40 GB | 2.31 GB | 0.15–0.34 + trunk |
| CDI (official) | `https://crawl.dcss.io/crawl/meta/crawl-<ver>/...` | yes | 0.42 GB | 2.09 GB | 0.30–0.34 + git; also `crawl-bot-*` (qw only) |
| CDO | `http://crawl.develz.org/{allgames,milestones}-<ver>.txt` | trunk only (`-svn`) | 0.56 GB | 1.46 GB | 0.3–0.26 + svn |
| LLD | `http://lazy-life.ddo.jp/mirror/meta/<ver>/...` | yes (keeps only the newest stable) | 0.12 GB | 0.69 GB | 0.14–0.34 + trunk |
| CRG | `https://crawl.roguelikes.gg/meta/<ver>/...` | yes, **not in Sequell** | 0.02 GB | 0.08 GB | 0.30–0.34 + git |
| CRG-BR | `https://crawl-br.roguelikes.gg/meta/<ver>/...` | yes, **not in Sequell** | 0.01 GB | 0.06 GB | 0.34 + git |
| CSZO, CSN, RHF, CKR, CWZ, CKO | various | **dead** (DNS failure or timeout) | – | – | historical only |
| CJR | S3 bucket | **dead** (404) | – | – | – |

Notes:
- **Sequell's list is out of date.** No server in `sources.yml` lists 0.34, although 0.34.0 was released on 2026-02-06 and every live server has 0.34 files. CRG and CRG-BR are missing entirely. Build our own list: start from `sources.yml`, add 0.34 and CRG, and check for 0.35 when it ships. dcss-stats keeps a newer list (`trunkGameVersion = 34`, CRG added 2026-07-12) but covers logfiles only.
- dcss-stats fetches the dead servers' files from `loom.shalott.org/server-xlogs/`. That host does not resolve today, so the CSZO, RHF and CJR history is not publicly reachable.
- Every live server we tried answered Range requests with `206 Partial Content`. All are plain nginx or Apache static files with `Last-Modified` headers.

### Format

Each line is one record of `key=value` fields separated by `:`. A literal `:` inside a value is written as `::`, as in `place=D::8`. Empty fields are left out. The writer is `xlog_fields::xlog_line()` in `hiscores.cc`.

**Logfile** (one line per finished game, from `scorefile_entry::set_base_xlog_fields` and `set_score_fields`):
- `v`, `vlong`: version, such as `0.34.1` or `0.35-a0`.
- `name`, `race`, `cls`, `char`: player, species, background.
- `xl`.
- `place`, `br`, `lvl`, `absdepth`.
- `god`, `piety`, `pen`.
- `start`, `end`, `dur`, `turn`, `aut`.
- `urune`, `nrune`, `fgem`, `igem`.
- `kills`, `sc`.
- `ktyp`: kill type, such as `mon`, `beam`, `pois`, `quitting`, `leaving` or `winning`. The full list is `kill_method_names` in `hiscores.cc`.
- `killer`, `kaux`, `ikiller`, `killer_flags`, `dam`, `sdam`, `tdam`.
- `tmsg`, `vmsg`.
- `map`, `killermap`.
- `seed`, written since 0.23.
- `wiz=1`, `explore=1`: written only when set.
- `tiles=1` for Webtiles games.
- Stats, skills and status.

**Milestones** (one line per event, from `mark_milestone`): the same character fields, plus `time`, `type`, `milestone` (free text) and sometimes `oplace`. Types listed in Sequell's `milestone-types` include:
- branches: `br.enter`, `br.end`, `br.mid`, `br.exit`
- runes and the orb: `rune`, `orb`, `orb.destroy`
- uniques: `uniq`, `uniq.ban`, `uniq.pac`, `uniq.ens`, `uniq.slime`
- ghosts: `ghost`, `ghost.ban`, `ghost.pac`
- gods: `god.worship`, `god.renounce`, `god.mollify`, `god.ecumenical`, `god.maxpiety`
- Abyss and ziggurats: `abyss.enter`, `abyss.exit`, `zig`, `zig.enter`, `zig.exit`
- gems: `gem.found`, `gem.lost`
- the tesseract: `tesseract.activate`, `tesseract.kill`
- other: `begin`, `death`, `shaft`, `sacrifice`, `ancestor.*`, `monstrous`, `mark`, `crash`

0.34 also writes **`zotorb`** ("will face orbs of fire"). It comes from `dgn-overview.cc` and is not in Sequell's list. Old files use aliases such as `type=unique`.

**Time** (`make_date_string` in `tags.cc`): `YYYYMMDDhhmmss` plus `S` or `D`, but the month is `tm_mon`, which is **0-based**. `20260914053237S` means 14 October 2026. go-sequell adds 1 in `ctime.NormalizeUnixTime`. Games before 2008-08-07 on CAO and CDO used local time; Sequell keeps per-server time zones (`timezones`, `utc-epoch`).

**Game identity.** A game is identified by server, `name` and `start`. Milestones join to their logfile line on that key. A game still in progress has milestones but no logfile line yet.

### Volume by version (normal games, logfiles only, all servers)

| Version | Logfile MB | Version | Logfile MB |
|---|---|---|---|
| 0.4–0.9 | 30–84 each | 0.25 | 145 |
| 0.10–0.15 | 32–109 each | 0.26 | 115 |
| 0.16 | 127 | 0.27 | 146 |
| 0.17 | 141 | 0.28 | 158 |
| 0.18 | 166 | 0.29 | 230 |
| 0.19 | 194 | 0.30 | 207 |
| 0.20 | 208 | 0.31 | 232 |
| 0.21 | 243 | 0.32 | 333 |
| 0.22 | 178 | 0.33 | 419 |
| 0.23 | 217 | 0.34 | 384 (8 months in) |
| 0.24 | 157 | git/trunk/svn (all versions mixed) | 4,711 |

At about 600 bytes a line, 1 MB is about 1,650 games. Milestone files are about 5× the size of the matching logfile, at about 500 bytes a line.

### Growth

From the last 1 MB of each active file, measured as lines per day between the first and last timestamps in the sample:

| File | Games/day |
|---|---|
| CAO 0.34 / git | 642 / 357 |
| CDI 0.34 / git | 354 / 153 |
| CUE 0.34 / git | 178 / 417 |
| CBR2 0.34 / git | 250 / 525 |
| CXC 0.34 / git | 171 / 410 |
| CNC 0.34 / git | 183 / 820 |
| CPO 0.34 / trunk | 81 / 190 |
| CDO svn | ~5 |

The total is about 4,900 games a day, before LLD and CRG are added. Milestones run at about 3,000–4,500 lines a day on a busy file, about 2 MB a day each. Altogether the data grows by about **3 MB a day of logfiles and 15–20 MB a day of milestones**.

### Terms of use

- None of the front pages we checked (CAO, CDI, CUE) states terms for the logfiles, and no license file sits next to the logfiles.
- robots.txt blocks none of the `meta/`, logfile or milestone paths. CAO blocks `/rawdata` (morgues) and `/scoring-data`; CUE blocks `/ghost/` and `/p/`.
- The data is public and has been mirrored by Sequell, dcss-stats and others for years. Still, each line carries a player name, so we should ship only aggregates without names.
- Fetch politely: incremental Range requests, one connection per host. Both go-sequell and dcss-stats behave this way.

## 2. Which tables are feasible

| Table | Source | Feasible? | Notes |
|---|---|---|---|
| Death share by XL | logfile `xl`, `ktyp` | **Yes, high confidence** | Keep only real deaths. Exclude `quitting`, `leaving`, `winning` and `wizmode` (see Pitfalls). Millions of rows per version group. |
| Death share by place | logfile `br`, `lvl`, `place` | **Yes** | Place is where the character died. Branch lengths and the set of branches change between versions, so compare places within a version group. |
| Top killers by XL or place | logfile `killer`, `ktyp`, `kaux`, `ikiller` | **Yes** | `killer` holds names with articles ("a two-headed ogre") and unique names. Normalize them as go-sequell's `crawl/killer` does. Uniques and player ghosts need grouping. |
| Death share by species, background or combo | logfile `race`, `cls` | **Yes** for species × XL and background × XL per version. **Thin** for species × background × XL | About 27 species × 27 backgrounds = 729 combos. 400k games spread over that gives a few hundred games per combo. Pool versions or set minimum counts. |
| God choice (final god) | logfile `god` | **Yes** | This is the god at death or win. It is not the first god or every god. |
| God choice (first god, switches) | milestones `god.worship`, `god.renounce` | **Yes**, needs milestones | Join to the game key. |
| Branch order | milestones `br.enter` (plus `turn`, `xl`) | **Yes**, needs milestones | Order of first entry per game. Portals (sewer, bailey, ossuary...) show up as `br.enter` too, so filter to real branches. Branch sets differ by version. |
| Rune order | milestones `rune` ("found a gossamer rune of Zot") | **Yes**, needs milestones | Map the rune name to its branch, such as gossamer → Spider and barnacled → Shoals. `urune` and `nrune` on later lines give counts. |
| Win rate by combo, god or version | logfile `ktyp=winning` | **Yes** | Remove bots first; they skew this table. |
| Turn or XL at which a branch is entered | milestones `br.enter` with `xl`, `turn` | **Yes** | Good input for "you are early or late for Lair" Strategic Advice. |

Splitting by version: split on the logfile's `v` field, cut to major.minor ("0.34"), and treat `-a0` builds as trunk of the next version. **Do not split by file name**: `logfile-git` on CAO holds games from 0.8-a0 in 2011 to today. The 0.30–0.34 range gives 0.2–0.4 GB (350k–650k games) per version. That is enough for every table above except fine-grained species × background × XL.

## 3. Processing cost

- **Full rebuild, everything:** about 62 GB to download. Servers gave 3–5 MB/s on a single 20 MB Range read (CAO 3.0, CDI 4.2, CUE 4.9). The servers can be fetched in parallel, one connection each. CAO dominates at 19.4 GB, about 1.8 hours, so a full rebuild takes about 2 hours wall time.
- **Full rebuild, logfiles only:** 10.2 GB, about 20 minutes (CAO 3.4 GB).
- **Full rebuild, normal games since 0.30:** the per-version files total about 1.6 GB of logfiles, but trunk files must also be read and filtered by `v`. Milestones for the same range are roughly 8 GB plus trunk.
- **Parsing:** a plain single-thread Python parser with a group-by aggregate ran at **88k lines/s, about 53 MB/s** on an Apple M1 Pro. All 62 GB is about 20 minutes of CPU; logfiles alone are about 3 minutes. go-sequell (Go, loading into Postgres) is the existing heavier option. A stream-and-aggregate pass needs no database.
- **Incremental refresh:** store the byte offset reached in each file. Request `Range: bytes=<offset>-` and parse only the new lines. This is exactly what go-sequell does: `httpfetch.go` resumes with `Range` and expects `206`, and `loader.go` keeps `file_offset` per file. The daily delta is about 20 MB in total, which takes seconds. Frozen files (old versions, dead servers) never change after the first fetch. Sequell marks live ones with `*` in `sources.yml`.
- **Shipped table size:** take death share by version group × XL (27) × branch (about 30) × ktyp class. That is at most about 25k rows per version group, around 1 MB of CSV before compression and around 100–300 KB gzipped. Species × background × XL for 3 version groups is about 60k rows. Top killers per XL (top 20) is about 540 rows per version group. A full statistics bundle should fit within a few MB.

## 4. Pitfalls

- **Bots.**
  - qw and others play under fixed names. In the first line of CAO `logfile-git`, `xw` (a bot) dies to a rat.
  - CDI keeps separate `crawl-bot-{0.29,0.30,0.31,git}` files for qw only. Leave them out.
  - dcss-stats maintains a 50-name bot list in `seedData.ts` (`qw`, `xw`, `rw`, `gw`, `jw`, `ow`, `ew`, `autorobin`, `qwrobin`, `hyperqwbe`, `gaubot`, ...). Use it and filter on `name`.
  - Our 1 MB samples from 2016, 2023 and 2026 had no bot lines. Bots play in bursts, though, and they mostly affect win and combo tables.
- **Not deaths.** `ktyp=quitting` is 4–15% of lines. Quick quits at XL 1 are "start-scumming". `leaving` (left the dungeon) reached 16% in a 2016 CAO sample. `winning` is a win. Exclude all three from death tables, and report quit and win rates separately.
- **Wizard and explore games.**
  - The logfile writes `wiz=1` or `explore=1` when set. Wizard deaths may use `ktyp=wizmode`. Drop these lines.
  - Milestones from wizard or explore games are not written to the DGL milestone file at all, unless the build sets `SCORE_WIZARD_CHARACTERS` (`hiscores.cc`, `mark_milestone`).
- **Seeded games.** `seed` is written for every game since 0.23. Nothing marks a player-chosen seed as different from a random one. Repeated seeds across games are the only signal: seed challenges, or many players on one seed. Dedupe or down-weight repeats if they matter.
- **Duplicates across servers.**
  - Each game is written only on the server where it was played, so the same game does not appear on two live servers.
  - Duplicates come from mirrors and alternate paths: `loom.shalott.org` copies of dead servers, CDI `crawl-bot-*` versus `crawl-git`, and old CBRO versus CBR2 (different hosts, separate games).
  - Dedupe on server + `name` + `start` + `end`. go-sequell drops only rows that are exactly identical (`ErrDuplicateRow`).
- **Versions.**
  - Trunk files mix versions over years.
  - Old versions have species and backgrounds that were later removed (`Ogre-Mage` in 0.1.7), different branch layouts and lengths, older milestone type names (`unique`), and missing fields (`aut`, `seed`, `absdepth`).
  - Before 2008-08-07, CAO and CDO times are in local time.
  - Statistics for the Advisor should use 0.30+ (or the current version and trunk). Older data serves only as a long-run prior.
- **Game modes and experimental branches.** Sprint, ZotDef, Nostalgia and the experimental builds live in separate files, listed with an alias in `sources.yml`. Some lines carry `explbr`. Use normal-game files only.
- **Free text.** `milestone`, `killer` and `tmsg` are English text whose wording changes between versions. Parse them with care. Prefer structured fields such as `br`, `type`, `ktyp` and `urune`.
- **Timestamps.** The month is 0-based (see Format). Naive parsing shifts every date back one month, and January (`00`) fails to parse at all.
- **Sequell's list is stale and some servers are dead** (see Sources). CBRO, CSZO and others hold historical data that is either frozen or gone.

## Gaps

- We did not measure the exact share of bot games over the full history. That needs the full download.
- We did not confirm whether the live Sequell deployment fetches 0.34 from some source other than the public `sources.yml`.
- The 3–5 MB/s download rate came from a single 20 MB read per server; sustained throughput and any per-IP limits are unknown.
- We did not check whether CRG-BR overlaps CRG accounts. They are different hosts, so the games should be separate.

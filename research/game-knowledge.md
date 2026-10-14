# Game Knowledge: what can be extracted from crawl source per version?

Research for issue #4 (map #1). Date: 2026-10-13.

**Sources read:** `crawl/crawl` at tag `0.34.1` (latest stable) and `master` (trunk), both downloaded as
GitHub tarballs on 2026-10-13. All paths below are relative to `crawl-ref/source/` unless noted.
Older tags were checked through the GitHub contents API. Prior art: `crawl/gretell`, `crawl/sequell`,
`Predelnik/dcss-wiki-scripts`.

## Answer in brief

- Almost all **Game Knowledge** for a version lives in that version's source tree. It takes three forms:
  1. **YAML** under `dat/` for monsters, species, backgrounds (jobs) and forms. This is the easiest form to parse.
  2. **C++ static tables** in `*-data.h` headers and `item-prop.cc`. These cover spells, zaps, mutations, branches,
     spawn tables, item bases, monster spell sets, spellbooks and god powers.
  3. **Text databases** under `dat/descript/` and `dat/database/`. These hold the flavour and description text
     shown in game, in a simple `%%%%`-delimited format.
- The yaml coverage depends on the version. Monsters and jobs moved to yaml in **0.32**, and forms in **0.33**.
  Species yaml dates from at least **0.23**. Before those versions, the same data is a C++ initializer (for example the
  hand-written `mon-data.h` in 0.31). An extractor that covers many versions has to handle both forms.
- The crawl binary can print a few datasets as JSON: `--playable-json` (species, jobs, combos) and `--branches-json`.
  The repo also ships `util/monster`, the per-version monster stat tool that the IRC bots use.
- I found no maintained, multi-version, machine-readable dataset (JSON or similar) for DCSS. The existing tools are the
  IRC bots (one `monster-<ver>` binary per version) and wiki update scripts that read the yaml.
- The combat formulas are short and spread across a few files: `attack.cc`, `melee-attack.cc`,
  `ranged-attack.cc`, `fight.cc`, `actor.cc`, `player.cc`, `monster.cc`, `beam.cc` with `zap-data.h`, `spl-cast.cc`
  and `mon-cast.cc`. Spell and zap damage is mostly **data** (templated dice calculators), not code.
- **Licence:** crawl is **GPL-2.0-or-later**, with some parts under other GPL-compatible licences. This repo is currently
  **MIT**. Shipping crawl's description text, or code ported line by line from crawl, inside an MIT app is the risky case.
  See the Licensing section.

## 1. Where each data category lives (0.34.1)

| Category | Source (0.34.1) | Form | Notes |
|---|---|---|---|
| Monsters (stats) | `dat/mons/*.yaml` (672 files; 690 on trunk) → generated `mon-data.h` via `util/mon-gen.py` | YAML | Fields include `hd`, `hp_10x`, `ac`, `ev`, `will`, `attacks` (type and damage), `resists`, `flags`, `spells`, `speed`/`energy`, `size`, `intelligence`, `holiness`, `shape`, `exp`. Introduced in 0.32; 0.31 and earlier have a hand-written `mon-data.h`. |
| Monster spell sets | `mon-spell.h` (`mspell_list[]`) | C++ table | The yaml `spells:` key references these sets by name. |
| Monster spawn tables per branch | `mon-pick-data.h` (`POP_<BRANCH>` macros) | C++ macros | Depth range, weight and distribution per branch. Vault-placed monsters come from `dat/des/**`. |
| Monster descriptions and quotes | `dat/descript/monsters.txt`, `dat/descript/quotes.txt` | text DB | |
| Species | `dat/species/*.yaml` (48) → `species-data.h`, `aptitudes.h` via `util/species-gen.py` | YAML | Aptitudes, stats, mutations by XL, difficulty. yaml since at least 0.23. |
| Backgrounds (jobs) | `dat/jobs/*.yaml` (27; 29 on trunk) → `job-data.h` via `util/job-gen.py` | YAML | Starting stats, equipment, skills, recommended species. Introduced in 0.32. |
| Forms (transformations) | `dat/forms/*.yaml` → `form-data.h` via `util/form-gen.py` | YAML | Introduced in 0.33. |
| Spells | `spl-data.h` (`spelldata[]`) | C++ table | Name, schools, flags, level, power cap, range, noise, tile. |
| Spell and zap damage and to-hit | `zap-data.h` | C++ table | For example `new dicedef_calculator<2, 4, 1, 10>` means 2 dice of (4 + pow×1/10). Templates are in `random.h`. Player and monster versions are separate entries. |
| Spellbooks | `book-data.h` (`spellbook_templates[]`) | C++ table | |
| Spell descriptions | `dat/descript/spells.txt` | text DB | |
| Item bases (weapons, armour, missiles, staves, gems) | `item-prop.cc` (`Weapon_prop[]`, `Armour_prop[]`, `Missile_prop[]`, `Staff_prop[]`, `Gem_prop[]`, brand weight vectors) | C++ tables inside a .cc file | Potion and scroll rarity are also maps here. |
| Unrandarts | `art-data.txt` (142 entries) → `art-data.h` via `util/art-data.pl` | key: value text | Properties, base type, flavour text. Special effects are code in `art-func.h`. |
| Item, ego and unrand descriptions | `dat/descript/items.txt`, `egos.txt`, `unrand.txt`, `unident.txt` | text DB | |
| Gods | `religion.cc` (`get_all_god_powers()`: powers by piety rank), `ability.cc` (`Ability_List`: costs and failure), `god-conduct.cc`, `god-passive.cc` | C++ (mostly code) | There is no single data table. Conducts, likes and dislikes are mostly code. Descriptions are in `dat/descript/gods.txt`. |
| Mutations | `mutation-data.h` (`mut_data[]`) | C++ table | Includes the text for each level, for example "(AC + 5)". Descriptions are in `dat/descript/mutations.txt`. |
| Branches | `branch-data.h` (`branches[]`); JSON via `branch-data-json.cc` | C++ table, JSON | Depth, entry, rune, ambient data. Descriptions are in `dat/descript/branches.txt`. |
| Features, clouds, statuses, durations | `feature-data.h`, `duration-data.h`, `dat/descript/{features,clouds,status,monstatus}.txt` | C++ and text | |
| Skills, abilities, backgrounds, species text | `dat/descript/{skills,ability,backgrounds,species,passives}.txt` | text DB | |
| Banes, sacrifices, dominoes, xp evokers | `bane-data.h`, `sacrifice-data.h`, `domino-data.h`, `xp-evoker-data.h` | C++ tables | |
| Level layouts and vaults | `dat/des/**/*.des` | DES (Lua-embedded) | Useful for "what can appear here". |
| Translations | `dat/descript/<lang>/`, `dat/database/<lang>/` | text DB | |

**Text DB format.** Entries are separated by `%%%%`. The first line is the key and the rest is the body; `#` starts a
comment. `database.cc` builds DBM files from them at runtime (`TextDB("descriptions", "descript/", ...)`).
The format is easy to parse directly.

**Trunk compared with 0.34.1.** The layout is the same, with more yaml files. The data-category layout has not changed since 0.33.

## 2. Extraction approaches

| Option | How | Pros | Cons |
|---|---|---|---|
| **A. Parse yaml and text DBs directly** | Read `dat/mons`, `dat/species`, `dat/jobs`, `dat/forms`, `dat/descript` at a tag, with no build. | Simple and robust. No untrusted build. Works offline per tag. | Covers only part of the data. Monsters and jobs are yaml only from 0.32. |
| **B. Parse C++ tables** | Write a small parser for struct-initializer arrays (`spl-data.h`, `zap-data.h`, `mutation-data.h`, `item-prop.cc` arrays, `mon-spell.h`, `book-data.h`, `branch-data.h`, `mon-pick-data.h`). crawl's own `util/gather_*` Perl scripts do this kind of regex extraction and work as a reference. | Covers everything that is table-shaped. | Brittle across versions: enum names and macros (`#if TAG_MAJOR_VERSION`) need resolving, and field order changes. |
| **C. Build crawl and dump** | Run `crawl --playable-json`, `--branches-json`, and build `util/monster` (`make monster`) for each version. Or write a small dump harness that links crawl's objects and serializes the in-memory tables (`get_monster_data`, `get_spell_*`, `get_item_*`) to JSON. | Exact, because the game's own code computes derived values such as average HP and spell power at HD. | Needs a full C++ build for every version, run in a sandbox. The harness is GPL code. |
| **D. Read it live from the game** | The webtiles describe screens and the client Lua API (`l-moninf.cc`, `l-you.cc`) expose the game's own computed numbers. | Always matches the running version. | This belongs to the Game Source question, not static Game Knowledge, and only covers what the UI shows. |

**Recommendation.** Use A and B as the default per-tag pipeline, written as our own extractor that outputs one JSON
bundle per version. Use C as an oracle in CI to check the parsed results for selected versions. Run any crawl build in a
sandbox, because it means executing a downloaded build system.

### Existing tools and datasets (prior art)

- **`util/monster`** in crawl, `monster-main.cc`: a CLI that prints a stat line for a monster for the version it was
  built from. `util/gather_mons` provides vault-monster data for it.
- **`crawl/gretell`**, the IRC bot: `gretell.pl` maps `@??` (trunk), `@?` and `@0.34?` … `@0.10?` to commands that
  shell out to one prebuilt `monster-<ver>` binary per version. This is the community's "per-version Game Knowledge"
  in practice, but it outputs text lines, not data.
- **`crawl/sequell`**: the IRC stats bot. Its data is game logs (xlog/milestones), not Game Knowledge.
- **`Predelnik/dcss-wiki-scripts`**: Lua scripts (`update-from-yaml.lua`) that update the CrawlWiki from the crawl yaml.
  This confirms that the yaml is the practical source for monsters, species and jobs.
- I found **no published multi-version JSON dataset**. GitHub searches for dcss, stone soup and spoiler data found
  nothing maintained. The wiki is secondary and lags behind trunk.

## 3. Formulas deterministic analysis needs (0.34.1 locations)

| Formula | Location | Notes |
|---|---|---|
| Hit or miss roll | `attack.cc:984` `attack::test_hit` | Defender EV is randomised as `random2avg(2*ev, 2)`. A `MIN_HIT_MISS_PERCENTAGE` auto hit/miss chance applies. Blindness adds a miss chance. |
| To-hit, generic | `attack.cc:160` `calc_pre_roll_to_hit`, `attack.cc:288` `calc_to_hit` | Players roll `random2(mhit)` before post-roll modifiers. Monsters roll after. |
| Melee to-hit and damage (player) | `melee-attack.cc:3486` `calc_to_hit`, `:5098` `apply_damage_modifiers`, `:5156` `calc_damage`; `attack.cc:918` `calc_damage`, `:840` slaying, `:899` unarmed base | |
| Stat and skill damage scaling | `fight.cc:1695` `stat_modify_damage`, `:1710` `apply_weapon_skill`, `:1679` `weapon_uses_strength` | |
| Ranged | `ranged-attack.cc:369` `apply_damage_modifiers` | |
| Monster to-hit and shield bypass | `fight.cc:232` `mon_to_hit_base` = `18 + hd*(skilled?5:3)/2`; `fight.cc:238` `mon_shield_bypass` = `15 + hd*2/3` | |
| Hit percentage helpers (the same numbers the game shows the player) | `fight.cc:202` `to_hit_pct`, `:216` `to_hit_pct_aux`, `:251` `mon_to_hit_pct`, `:286` `mon_beat_sh_pct` | These are the best functions to mirror for "chance to be hit". |
| Damage reduction from AC (including GDR) | `actor.cc:343` `actor::apply_ac` (rules: none, proportional, normal, half, triple); `fight.cc:906` `apply_chunked_AC` | Normal rule: `saved = random2(1+ac)`, with a floor of `min(gdr*max_dmg/100, ac/2)`. |
| Shield block | `attack.cc` `attack_shield_blocked`; `player.cc:6106`, `monster.cc:3078` `shield_bonus`; `player.cc:2281` `player_shield_class` | |
| Player AC and EV | `player.cc:6533` `player::armour_class`, `:6635` `player::evasion`, `:2167` `_player_evasion`; armour and shield penalties at `:6178` and `:6193` | |
| Monster AC and EV | `monster.cc:3206` `monster::armour_class`, `:3316` `monster::evasion` | Base values come from the yaml and are modified by items and statuses. |
| Attack speed | `fight.cc:1259` `weapon_min_delay`, `:1294` `weapon_adjust_delay`; `player-act.cc:237`, `monster.cc:439` `attack_delay` | |
| Resistances | `fight.cc:853` `resist_adjust_damage` | |
| Beam and zap damage and to-hit | `zap-data.h` together with `beam.cc:385` `zap_to_hit` and `:401` `zap_damage`; calculator templates in `random.h:239-270` | |
| Spell power and failure (player) | `spl-cast.cc:550` `calc_spell_power`, `:455` `raw_spell_fail`, `:2989` `failure_rate_to_int`; damage strings at `:3167` and `:3194` | |
| Monster spell power and range | `mon-cast.cc:2158` `mons_power_for_hd` (`hd * hd_factor(spell)`), `:2174` `mons_spellpower` | |
| Monster average HP | `mon-util.cc:2324` `mons_avg_hp` (from `hp_10x`) | |
| Stealth, regeneration, XP | `player.cc:3329` `player_stealth`, `:1258` `player_regen`, `:3667` `exp_needed` | |
| Dice | `random.cc:289` `calc_dice`; `random2avg` and related functions in `random.cc` | |

Also useful: `crawl-ref/docs/fight_simulator.txt` describes the wizard-mode fight simulator, which can serve as a
validation oracle. `describe.cc` `_monster_attacks_description` (`:5622`) shows how the game turns these formulas
into player-facing numbers.

**Caveat.** These are code, not data. Deterministic analysis means re-implementing about 20 functions per version and
tracking their changes with `git log -L` across tags. Many inputs, such as statuses, brands and mutations, are special
cases inside the functions.

## 4. Licensing

- crawl `LICENSE` (0.34.1): "GNU General Public License ... either version 2 of the License, or (at your option) any
  later version". It lists exceptions that are all GPL-compatible: BSD, MIT, zlib, Apache-2 and Boost for some files.
  Most tiles and art are **CC0**, with older tiles varying (see `crawl/tiles`).
- **dcss-copilot is MIT** (repo `LICENSE`).
- Implications (this is not legal advice):
  - **Numeric facts** such as HD, AC and spell levels are facts. Extracting them into our own format is low risk,
    but the generated dataset is safest distributed under **GPL-2.0-or-later** with attribution.
  - **Description and flavour text** (`dat/descript`, `dat/database`) is copyrighted expression under the GPL.
    Redistributing it within a bundle means GPL terms apply to that text.
  - **Ported formula code.** A line-by-line translation of crawl functions is likely a derivative work, so it would
    need the GPL. Clean-room implementations written from documented formulas are much less clear-cut as derivatives.
  - **Linking crawl code** (a dump harness, or embedding `util/monster`) produces GPL code. Keep it a separate build-time
    tool whose output is data.
  - GPLv2 has **no network clause**, so a hosted service using GPL code does not by itself trigger source-distribution
    duties. Distributing a client app does.
  - **Simplest path:** license DCSS Copilot (or at least its Game Knowledge package) as GPL-2.0-or-later or GPL-3.0. Or keep the
    extracted dataset and any ported formulas in a separately licensed GPL package that the MIT app loads as data.
    This is a decision for the project owner.

## 5. Open points for downstream tickets

- Supported version range. Supporting versions before 0.32 means a C++ table parser for monsters and jobs.
- Whether to read computed numbers live from the game (option D) instead of re-implementing formulas. This depends on
  what webtiles exposes, which is a Game Source question.
- How to handle the licence for the MIT repository (see Licensing).

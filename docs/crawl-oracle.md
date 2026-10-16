# The crawl oracle

The crawl oracle proves the engine's ported formulas against real crawl, so the engine's numbers can be trusted for each shipped Game Knowledge bundle. It is seam 2 in `CODING_STANDARDS.md`. It covers a monster's melee attack on the player: the monster's to-hit, the roll against the player's evasion, the shield block, and damage after armour. Spell failure comes with #34.

## How it works

`tools/crawl-oracle/harness/melee.cc` is compiled against a crawl build's objects in place of `main.o`, as crawl's own `util/monster` is. It sets crawl up without a dungeon, then for each case:

- dresses a human player at random (Strength, Dexterity, Dodging and Shields skill, body armour and a shield with random enchantment) until their AC, EV and SH are exact on the display, since the display is all the engine sees;
- places a random monster next to them with no items and no statuses, and picks one of its damaging attacks;
- runs that attack many times through crawl's own `melee_attack`, step by step, as `melee_attack::attack` does: `calc_to_hit`, `test_hit` against `player::evasion`, `attack_shield_blocked`, and `calc_damage`, which applies armour. It skips the effects of a hit, and resets the shield's blocks before each attack.

It prints a JSON line per case with the inputs the ported formulas take and the tallies: how often each to-hit came up, hits, blocks, and how often each amount of damage was taken.

`crawl-oracle` reads that, runs each case as many times through `copilot_core::oracle::melee`, using the formula variants named in the bundle for that crawl version, and compares each tally with a chi-squared test. A tally more than five standard deviations past chance is a mismatch, and any mismatch fails the run. Both sides are seeded, so the verdict is the same every run. An off-by-one in any of the four formulas fails dozens of the 500 cases.

The harness leaves out what the engine doesn't model: monsters that wield items, are animated objects, or carry a halo or umbra, which changes their to-hit against the player (#78). It also leaves out players whose AC, EV or SH carries a fraction the display drops (#77).

## Running it

```sh
git clone --depth 1 --branch 0.34.1 https://github.com/crawl/crawl /tmp/crawl
git -C /tmp/crawl submodule update --init --depth 1 \
  crawl-ref/source/contrib/lua crawl-ref/source/contrib/pcre \
  crawl-ref/source/contrib/sqlite crawl-ref/source/contrib/zlib
tools/crawl-oracle/oracle.sh /tmp/crawl
```

The checkout must be at the bundle's commit (`crawl.commit` in `crates/gk/bundles/<tag>.json`). Crawl builds as webtiles and needs crawl's Linux build dependencies (`crawl-ref/INSTALL.md`). The first build takes a few minutes; later runs rebuild only the harness.

The `Crawl oracle` workflow runs this for each stable bundle whenever the engine, the bundles or the oracle change, and caches the crawl build by commit.

`cargo test -p crawl-oracle` diffs a small dump the harness printed (`tools/crawl-oracle/tests/fixtures/melee-0.34.1.jsonl`), so every engine change is checked against crawl even without a crawl build. Regenerate the dump whenever the harness or its output changes:

```sh
/tmp/crawl/crawl-ref/source/crawl-oracle-melee 34 24 20000 > tools/crawl-oracle/tests/fixtures/melee-0.34.1.jsonl
```

## Adding a bundle

Add its tag to the workflow's `bundle` matrix. If its crawl changed the melee code, port a new formula variant first; the oracle shows where.

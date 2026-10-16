# The crawl oracle

The crawl oracle proves the engine's ported formulas against real crawl, so the engine's numbers can be trusted for each shipped Game Knowledge bundle. It is seam 2 in `CODING_STANDARDS.md`. It covers:

- a monster's melee attack on the player: the monster's to-hit, the roll against the player's evasion, the shield block, and damage after armour;
- a monster's spell cast at the player: which spell it picks, at what distance, the power and range it casts with, and the beam's accuracy, the shield block, the roll against evasion and damage after armour;
- the gear crawl makes monsters with: the launcher or throwing missile, and the wand, each kind is given, against the odds in Game Knowledge;
- a monster's missile at the player: whether it shoots or throws from a distance and the energy that takes, and the missile's to-hit, the roll against evasion, the shield block and damage after armour;
- a monster's wand: whether it zaps it and the energy that takes, and the beam at the wand's power.

The engine's other formulas aren't diffed yet: melee reach, the shield's block limit and monster energy outside firing and zapping. Spell failure, which only the player has, comes with the first player spell the engine ports (#35).

## How it works

### Melee

`tools/crawl-oracle/harness/melee.cc` is compiled against a crawl build's objects in place of `main.o`, as crawl's own `util/monster` is. It sets crawl up without a dungeon, then for each case:

- dresses a human player at random (Strength, Dexterity, Dodging and Shields skill, body armour and a shield with random enchantment) until their AC, EV and SH are exact on the display, since the display is all the engine sees;
- places a random monster next to them with no items and no statuses, and picks one of its damaging attacks;
- runs that attack many times through crawl's own `melee_attack`, step by step, as `melee_attack::attack` does: `calc_to_hit`, `test_hit` against `player::evasion`, `attack_shield_blocked`, and `calc_damage`, which applies armour. It skips the effects of a hit, and resets the shield's blocks before each attack.

It prints a JSON line per case with the inputs the ported formulas take and the tallies: how often each to-hit came up, hits, blocks, and how often each amount of damage was taken.

It leaves out what the engine doesn't model: monsters that wield items, are animated objects, or carry a halo or umbra, which changes their to-hit against the player (#78). It also leaves out players whose AC, EV or SH carries a fraction the display drops (#77).

### Spells

`tools/crawl-oracle/harness/spells.cc` is built the same way, but includes crawl's `mon-cast.cc`, whose spell choice is static, and links without `mon-cast.o`. It draws its cases from the spells the engine ports (`crawl-oracle ported-spells <crawl version>` lists them), alternating two kinds:

- **beam**: a randomly dressed player, and a random monster whose book holds a ported spell, casting it many times. Each cast is set up by crawl's `setup_mons_cast` and rolled as `bolt::affect_player` rolls it: `bolt::misses_player` for the shield and evasion, then the damage dice, `bolt::apply_AC` and `check_your_resists` for a human without resistances. It prints the power, range and accuracy, and tallies hits, blocks and damage.
- **choice**: a random monster with a ported spell, a few squares from the player, picking a spell many times through crawl's `_choose_spell_to_cast`, from its book cut down to the ported spells (the engine assumes a monster doesn't cast the others). It tallies which spell, or none, was picked.

It leaves out what the engine doesn't model: draconians, whose breath their book doesn't hold; monsters whose spells crawl picks when they are made; and the beam's flight (`bolt::fire`), so a bounce or a chaos beam's flavour isn't diffed. A monster with an explosion or a cloud among its spells is kept three squares or more from the player in a choice case: up close, crawl's tracer weighs the harm the caster does itself against the harm to the player (`mons_should_fire`), which the engine doesn't port; it assumes the caster casts anyway.

### Missiles and wands

`tools/crawl-oracle/harness/missiles.cc` is built the same way, but includes crawl's `mon-act.cc`, whose `_handle_wand` is static, and links without `mon-act.o`. It runs crawl's console headless, as firing redraws the view. Five kinds of case take turns:

- **gear**: a random kind of monster that uses its starting equipment, three times in four one that a few tries show may be made with something to shoot or zap, is given its items by crawl's `give_item` many times, on D:5 in the Dungeon and not in a band, with the game's item sets picked afresh each time. It tallies the launcher, or else the throwing missile, it got, and the wand it got and will zap.
- **missile**: a monster made with a launcher or throwing missile, made +0 and unbranded, fires it at a randomly dressed player many times, rolled as `ranged_attack::attack` rolls it: the to-hit, `attack::test_hit`, `attack_shield_blocked` and `calc_damage`, which applies armour, without the effects of a hit.
- **fire**: such a monster, its launcher wielded, one to seven squares from the player, decides many times whether to shoot or throw through crawl's `handle_throw`, which fires for real at a player whose hit points and statuses are restored each time. It tallies the energy each action took, 0 when it didn't fire. Nets are left out: one that hits catches the player.
- **wand**: a random monster zaps a wand of a spell the engine ports at a randomly dressed player many times, set up by `setup_mons_cast` as a wand zap and rolled as a spell's beam is.
- **zap**: a monster given such a wand, which it likes, decides many times whether to zap it through crawl's `_handle_wand`, which zaps for real. It tallies the energy each action took. A wand of an explosion is zapped from three squares or more, as in the spell harness.

It leaves out what the engine doesn't model: a launcher or missile's enchantment and brand, a monster swapping its launcher in from its off hand, which costs it a fifth of an attack's energy (`monster::swap_weapons`), the flight of a missile or beam past the player, and a monster weighing the harm a blast does itself.

### The diff

`crawl-oracle` reads any harness's dump, runs each case as many times through `copilot_core::oracle::melee`, `oracle::spell`, `oracle::spell_choice`, `oracle::gear`, `oracle::missile`, `oracle::fire`, `oracle::wand` or `oracle::zap`, using the formula variants named in the bundle for that crawl version, and compares each tally with a chi-squared test. What the formulas fix, a spell's or wand's power, range and accuracy, must be equal. A tally more than five standard deviations past chance is a mismatch, and any mismatch fails the run. Both sides are seeded, so the verdict is the same every run. When it was built, an off-by-one put into each of the four melee formulas in turn failed between 11 and 175 of the 500 cases. In the spell formulas, one put into the beam's accuracy, the evasion roll, the shield's block, damage, armour or a spell's power failed between 22 and 261 of 500; two were too small to show at 20,000 trials, a block roll one higher and the spell roll taking a slot's frequency in 200 as `>` rather than `>=`, each shifting a chance by half a point. In the missile harness, 1,500 cases over three seeds match. An off-by-one put into a missile's accuracy, its damage or an archer's bonus damage, the chance a launcher holds fire (one in six for one in five) or a thrown missile is thrown (one in four for one in three), the chance a wand is zapped (two in three given up for three in four), a wand's power, or the draw of a monster's gear failed between 6 and 106 of 500; the archer's bonus, which only archers roll, was the smallest.

`cargo test` checks melee to-hit, beam damage, spell power, spell choice, missile damage, the gear a kind is made with and how often a launcher fires.

## Running it

```sh
git clone --depth 1 --branch 0.34.1 https://github.com/crawl/crawl /tmp/crawl
git -C /tmp/crawl submodule update --init --depth 1 \
  crawl-ref/source/contrib/lua crawl-ref/source/contrib/pcre \
  crawl-ref/source/contrib/sqlite crawl-ref/source/contrib/zlib
tools/crawl-oracle/oracle.sh /tmp/crawl
```

The checkout must be at the bundle's commit (`crawl.commit` in `crates/gk/bundles/<tag>.json`). Crawl builds as webtiles and needs crawl's Linux build dependencies (`crawl-ref/INSTALL.md`). The first build takes a few minutes; later runs rebuild only the harnesses.

The `Crawl oracle` workflow runs this for each stable bundle whenever the engine, the bundles or the oracle change, and caches the crawl build by commit and runner OS image, saving it before the diff runs so a failing diff doesn't rebuild crawl next time.

`cargo test -p crawl-oracle` diffs a small dump from each harness (`tools/crawl-oracle/tests/fixtures/`), so every engine change is checked against crawl even without a crawl build. Regenerate a dump whenever its harness, its output or the ported spells change:

```sh
/tmp/crawl/crawl-ref/source/crawl-oracle-melee 34 24 20000 > tools/crawl-oracle/tests/fixtures/melee-0.34.1.jsonl
cargo run -q -p crawl-oracle -- ported-spells 0.34.1 > /tmp/ported-spells
/tmp/crawl/crawl-ref/source/crawl-oracle-spells 34 24 20000 /tmp/ported-spells > tools/crawl-oracle/tests/fixtures/spells-0.34.1.jsonl
/tmp/crawl/crawl-ref/source/crawl-oracle-missiles 34 25 20000 /tmp/ported-spells > tools/crawl-oracle/tests/fixtures/missiles-0.34.1.jsonl
```

## Adding a bundle

Add its tag to the workflow's `bundle` matrix. If its crawl changed the melee, spell, missile or wand code, port a new formula variant first, and if it changed how monsters are given their gear, its series' gear rules in `tools/gk-extract`; the oracle shows where.

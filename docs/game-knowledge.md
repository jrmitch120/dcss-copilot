# Game Knowledge bundles

A Game Knowledge bundle is the static facts about DCSS for one Crawl Version, as one JSON file: monsters with their melee attacks and action costs, item classes and sub-types, spells, species, dungeon features, the values crawl gives a monster's attitude and threat rating, and which variant of each ported crawl formula the version uses. `crates/gk` defines the format and `tools/gk-extract` builds it from crawl's source at a release tag. The app ships the bundles in `crates/gk/bundles/`, built into the binary.

Webtiles sends monster, item and dungeon feature types, and a monster's attitude and threat rating, as crawl's C++ enum values, which crawl renumbers between versions. A bundle keys everything by its own version's values, so the webtiles Game Source decodes a game's monsters and items through the bundle for that game's Crawl Version. With no bundle for the version, nothing is decoded: monsters keep the name the game sends, and the Advisor knows nothing more about them.

## What the extractor reads

The extractor reads the source as a webtiles server's build compiles it: `TAG_MAJOR_VERSION` from `tag-version.h`, with `USE_TILE` and `USE_TILE_WEB` defined.

| Bundle part | From |
|---|---|
| monsters | `dat/mons/*.yaml`, numbered by `monster_type` in `monster-type.h`, described by `dat/descript/monsters.txt`; attacks, the `fighter` flag, energy costs, spellbook and tile read as `util/mon-gen.py` reads them |
| monster spellbooks | `mspell_list` in `mon-spell.h`: each book's spells, how often each is picked and its slot flags. A book of `MST_GHOST` means each monster of the kind is given its own spells when made, which the bundle can't know |
| monster tiles | the tile lists in `rltiles/`, from `dc-player.txt`, read as crawl's tile build (`rltiles/tool`) reads them |
| item classes | `object_class_type` in `object-class-type.h` |
| item sub-types | the sub-type enums in `item-prop-enum.h`, `potion-type.h` and `book-type.h`, named by the tables and name functions in `item-prop.cc` and `item-name.cc` |
| spells | `spelldata` in `spl-data.h`, numbered by `spell_type` in `spell-type.h`, with each spell's range and power cap; constants such as `LOS_RADIUS` from `defines.h` |
| zaps | the zap a spell fires from `_spl_zaps` in `spl-zap.cc` (its first, for a spell listed twice), and that zap's flavour, flags and a monster's damage dice and accuracy by power from `zap_data` in `zap-data.h` |
| species | `dat/species/*.yaml` |
| dungeon features | `dungeon_feature_type` in `dungeon-feature-type.h` |
| monster attitudes | `mon_attitude_type` in `mon-attitude-type.h` |
| monster threat ratings | `mon_threat_level_type` in `mon-util.h` |

A bundle also names the variant of each ported crawl formula its version uses, such as `crawl-0.34` for a monster's to-hit, so the engine picks its code by name and never compares version strings. The extractor picks them by the release's series, so `0.34.1` gets the variants ported from crawl 0.34. A release in a series it has no variants for, such as a newer stable, a beta or trunk, stops the extraction rather than being read as 0.34, and so does a crawl whose monster data has changed shape, such as an attack with a field `mon-gen.py` 0.34 doesn't know. Adding a series means porting its formulas and naming them in `FORMULA_VARIANTS` in `tools/gk-extract`.

It understands only what those files use: `#if TAG_MAJOR_VERSION` conditionals, function-like macros that build table entries, enum initializers that are literals or earlier enumerators, table entries built by `_mon_hex_zap`, and damage and accuracy calculators whose template arguments are literals or integer `#define`s. Anything else stops the extraction with an error naming the file, rather than producing a wrong bundle.

## Building a bundle

```sh
git clone --depth 1 --branch 0.34.1 https://github.com/crawl/crawl /tmp/crawl
cargo run -p gk-extract -- /tmp/crawl --tag 0.34.1 --commit "$(git -C /tmp/crawl rev-parse HEAD)" \
  --tiles app/static/tiles > crates/gk/bundles/0.34.1.json
```

`--tiles` writes the image of every tile the bundle names into the app's `static/tiles/`, where the panel draws them from.

The extractor checks `--tag` and `--commit` against the checkout with git: the tag must point at the commit, which must be the one checked out, with no changes to tracked files. Each bundle records the crawl tag and commit it came from and crawl's license (ADR 0002). A new bundle also needs adding to `Library::shipped` in `crates/gk`.

## Tiles

A monster's tile is the image webtiles draws it with, named by its path in `rltiles` without the extension, such as `mon/animals/jackal`. Its tile enum is the one `util/mon-gen.py` gives it, and the tile lists say which image file that enum is drawn from. A tile crawl's build makes, by recolouring, mirroring, composing onto a background or turning into a corpse, has no file of its own, so the bundle names no tile for it.

Every tile file the bundle names ships, whatever its license. Most are CC0; the ones crawl's tile project lists as under an unknown license, from NetHack, or not agreed by their artist (`TILES_UNDER_UNKNOWN_LICENSE.md` in [crawl/tiles](https://github.com/crawl/tiles)) are not, and ship as part of crawl under its GPL (ADR 0002, `NOTICE`).

Crawl's build outlines the tiles listed under `%rim 1` in black: every transparent pixel beside a drawn one, up, down, left or right, becomes opaque black (`tile::add_rim` in `rltiles/tool/tile.cc`). The extractor does the same to those tiles as it writes them, so they look as they do in webtiles, and copies the rest unchanged. A file drawn both with and without the outline for different monsters stops the extraction, since the app ships one image per file. The panel draws a monster without a tile as its glyph.

Crawl's data gives some monsters the program bug's error tile (`misc/error`), which isn't a picture of them, so the extractor gives them no tile and doesn't ship the error image. No tile yet: crawl composes or recolours them at runtime. At 0.34.1 they are:

- program bug;
- black, yellow, green, purple, red, white, grey and pale draconian;
- tentacle and tentacle segment (the kraken's), eldritch tentacle and eldritch tentacle segment, starspawn tentacle and starspawn tentacle segment, snaplasher vine and snaplasher vine segment;
- hell lord and god wrath avatar.

## Tests

The tests in `tools/gk-extract/tests/extract.rs` cover the extractor (seam 3):

- A snapshot of the bundle built from `tests/crawl-0.34.1/`, crawl's files at 0.34.1 trimmed to a few entries each, and the tiles written from it, the outlined ones checked against `tests/outlined-by-crawl/`, made by crawl's own `rltiles/tool` code. They run everywhere, as do the checks of a release against a throwaway git checkout and of a release with no known formula variants.
- The shipped bundle and tiles are exactly what the extractor makes of crawl at its tag. It needs a crawl checkout, so it is ignored unless run with one; CI checks out crawl at the pinned commit and runs it:

```sh
DCSS_CRAWL_SOURCE=/tmp/crawl cargo test -p gk-extract -- --ignored
```

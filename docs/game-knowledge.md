# Game Knowledge bundles

A Game Knowledge bundle is the static facts about DCSS for one Crawl Version, as one JSON file: monsters with their melee attacks and action costs, item classes and sub-types, spells, species, dungeon features, the values crawl gives a monster's attitude and threat rating, and which variant of each ported crawl formula the version uses. `crates/gk` defines the format and `tools/gk-extract` builds it from crawl's source at a release tag. The app ships the bundles in `crates/gk/bundles/`, built into the binary.

Webtiles sends monster, item and dungeon feature types, and a monster's attitude and threat rating, as crawl's C++ enum values, which crawl renumbers between versions. A bundle keys everything by its own version's values, so the webtiles Game Source decodes a game's monsters and items through the bundle for that game's Crawl Version. With no bundle for the version, nothing is decoded: monsters keep the name the game sends, and the Advisor knows nothing more about them.

## What the extractor reads

The extractor reads the source as a webtiles server's build compiles it: `TAG_MAJOR_VERSION` from `tag-version.h`, with `USE_TILE` and `USE_TILE_WEB` defined.

| Bundle part | From |
|---|---|
| monsters | `dat/mons/*.yaml`, numbered by `monster_type` in `monster-type.h`, described by `dat/descript/monsters.txt`; attacks, the `fighter` flag, energy costs and tile read as `util/mon-gen.py` reads them |
| monster tiles | the tile lists in `rltiles/`, from `dc-player.txt`, read as crawl's tile build (`rltiles/tool`) reads them |
| item classes | `object_class_type` in `object-class-type.h` |
| item sub-types | the sub-type enums in `item-prop-enum.h`, `potion-type.h` and `book-type.h`, named by the tables and name functions in `item-prop.cc` and `item-name.cc` |
| spells | `spelldata` in `spl-data.h`, numbered by `spell_type` in `spell-type.h` |
| species | `dat/species/*.yaml` |
| dungeon features | `dungeon_feature_type` in `dungeon-feature-type.h` |
| monster attitudes | `mon_attitude_type` in `mon-attitude-type.h` |
| monster threat ratings | `mon_threat_level_type` in `mon-util.h` |

A bundle also names the variant of each ported crawl formula its version uses, such as `crawl-0.34` for a monster's to-hit, so the engine picks its code by name and never compares version strings. The extractor picks them by the release's series, so `0.34.1` gets the variants ported from crawl 0.34. A release in a series it has no variants for, such as a newer stable, a beta or trunk, stops the extraction rather than being read as 0.34, and so does a crawl whose monster data has changed shape, such as an attack with a field `mon-gen.py` 0.34 doesn't know. Adding a series means porting its formulas and naming them in `FORMULA_VARIANTS` in `tools/gk-extract`.

It understands only what those files use: `#if TAG_MAJOR_VERSION` conditionals, function-like macros that build table entries, and enum initializers that are literals or earlier enumerators. Anything else stops the extraction with an error naming the file, rather than producing a wrong bundle.

## Building a bundle

```sh
git clone --depth 1 --branch 0.34.1 https://github.com/crawl/crawl /tmp/crawl
cargo run -p gk-extract -- /tmp/crawl --tag 0.34.1 --commit "$(git -C /tmp/crawl rev-parse HEAD)" \
  --tiles app/static/tiles > crates/gk/bundles/0.34.1.json
```

`--tiles` copies the image of every tile the bundle names into the app's `static/tiles/`, where the panel draws them from.

The extractor checks `--tag` and `--commit` against the checkout with git: the tag must point at the commit, which must be the one checked out, with no changes to tracked files. Each bundle records the crawl tag and commit it came from and crawl's license (ADR 0002). A new bundle also needs adding to `Library::shipped` in `crates/gk`.

## Tiles

A monster's tile is the image webtiles draws it with, named by its path in `rltiles` without the extension, such as `mon/animals/jackal`. Its tile enum is the one `util/mon-gen.py` gives it, and the tile lists say which image file that enum is drawn from. A bundle names a tile only when the app can ship its file:

- a tile crawl's build makes, by recolouring, mirroring, composing onto a background or turning into a corpse, has no file of its own, so it has no tile;
- a file that crawl's tile project lists as under an unknown license, from NetHack, or not agreed by its artist (`tools/gk-extract/tiles-under-unknown-license.txt`, from `TILES_UNDER_UNKNOWN_LICENSE.md` in [crawl/tiles](https://github.com/crawl/tiles)) is not shipped, so it has no tile either. The rest are CC0 (`NOTICE`).

Crawl's build draws a black rim around some tiles (`%rim 1`); the shipped files don't have it. The panel draws a monster without a tile as its glyph.

## Tests

The tests in `tools/gk-extract/tests/extract.rs` cover the extractor (seam 3):

- A snapshot of the bundle built from `tests/crawl-0.34.1/`, crawl's files at 0.34.1 trimmed to a few entries each, and the tiles copied from it. They run everywhere, as do the checks of a release against a throwaway git checkout and of a release with no known formula variants.
- The shipped bundle and tiles are exactly what the extractor makes of crawl at its tag. It needs a crawl checkout, so it is ignored unless run with one; CI checks out crawl at the pinned commit and runs it:

```sh
DCSS_CRAWL_SOURCE=/tmp/crawl cargo test -p gk-extract -- --ignored
```

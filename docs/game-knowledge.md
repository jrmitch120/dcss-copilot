# Game Knowledge bundles

A Game Knowledge bundle is the static facts about DCSS for one Crawl Version, as one JSON file: monsters, item classes and sub-types, spells, species, dungeon features, and the values crawl gives a monster's attitude and threat rating. `crates/gk` defines the format and `tools/gk-extract` builds it from crawl's source at a release tag. The app ships the bundles in `crates/gk/bundles/`, built into the binary.

Webtiles sends monster, item and dungeon feature types, and a monster's attitude and threat rating, as crawl's C++ enum values, which crawl renumbers between versions. A bundle keys everything by its own version's values, so the webtiles Game Source decodes a game's monsters and items through the bundle for that game's Crawl Version. With no bundle for the version, nothing is decoded: monsters keep the name the game sends, and the Advisor knows nothing more about them.

## What the extractor reads

The extractor reads the source as a webtiles server's build compiles it: `TAG_MAJOR_VERSION` from `tag-version.h`, with `USE_TILE` and `USE_TILE_WEB` defined.

| Bundle part | From |
|---|---|
| monsters | `dat/mons/*.yaml`, numbered by `monster_type` in `monster-type.h`, described by `dat/descript/monsters.txt` |
| item classes | `object_class_type` in `object-class-type.h` |
| item sub-types | the sub-type enums in `item-prop-enum.h`, `potion-type.h` and `book-type.h`, named by the tables and name functions in `item-prop.cc` and `item-name.cc` |
| spells | `spelldata` in `spl-data.h`, numbered by `spell_type` in `spell-type.h` |
| species | `dat/species/*.yaml` |
| dungeon features | `dungeon_feature_type` in `dungeon-feature-type.h` |
| monster attitudes | `mon_attitude_type` in `mon-attitude-type.h` |
| monster threat ratings | `mon_threat_level_type` in `mon-util.h` |

It understands only what those files use: `#if TAG_MAJOR_VERSION` conditionals, function-like macros that build table entries, and enum initializers that are literals or earlier enumerators. Anything else stops the extraction with an error naming the file, rather than producing a wrong bundle.

## Building a bundle

```sh
git clone --depth 1 --branch 0.34.1 https://github.com/crawl/crawl /tmp/crawl
cargo run -p gk-extract -- /tmp/crawl --tag 0.34.1 --commit "$(git -C /tmp/crawl rev-parse HEAD)" \
  > crates/gk/bundles/0.34.1.json
```

Each bundle records the crawl tag and commit it came from and crawl's license (ADR 0002). A new bundle also needs adding to `Library::shipped` in `crates/gk`.

## Tests

Two tests in `tools/gk-extract/tests/extract.rs` cover the extractor (seam 3):

- A snapshot of the bundle built from `tests/crawl-0.34.1/`, crawl's files at 0.34.1 trimmed to a few entries each. It runs everywhere.
- The shipped bundle is exactly what the extractor makes of crawl at its tag. It needs a crawl checkout, so it is ignored unless run with one; CI checks out crawl at the pinned commit and runs it:

```sh
DCSS_CRAWL_SOURCE=/tmp/crawl cargo test -p gk-extract -- --ignored
```

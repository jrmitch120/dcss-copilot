#!/usr/bin/env bash
# Runs the crawl oracle: builds its harness in a crawl checkout, runs crawl's melee code for
# randomised monster attackers and player defenders, and diffs that against the engine. Exits
# non-zero if they differ.
#
#   tools/crawl-oracle/oracle.sh <crawl checkout>
#
# The checkout must be at a shipped Game Knowledge bundle's commit, with the contrib submodules
# lua, pcre, sqlite and zlib. Crawl is built as webtiles; the melee code is the same in every
# build. The first build takes a few minutes; later ones only rebuild the harness.
set -euo pipefail

crawl=$(realpath "$1")
oracle=$(dirname "$(realpath "$0")")
repo=$(realpath "$oracle/../..")
source="$crawl/crawl-ref/source"

# Fixed, so the verdict is the same every run: a case is redrawn only when the harness changes.
seed=34
cases=500
trials=20000

make -C "$source" -f Makefile -f "$oracle/harness/oracle.mk" -j"$(nproc)" \
    WEBTILES=y USE_DGAMELAUNCH=y crawl-oracle-melee >&2
# Cargo runs from the repository, so it uses the pinned toolchain in rust-toolchain.toml.
"$source/crawl-oracle-melee" "$seed" "$cases" "$trials" \
    | (cd "$repo" && cargo run --quiet --release -p crawl-oracle -- -)

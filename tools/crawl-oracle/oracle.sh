#!/usr/bin/env bash
# Runs the crawl oracle: builds its harnesses in a crawl checkout, runs crawl's melee, monster
# spell, missile and wand code, its monster gear, and its monster turns before the player's next
# action, for randomised monsters and player defenders, and diffs that against the engine.
# Exits non-zero if they differ.
#
#   tools/crawl-oracle/oracle.sh <crawl checkout>
#
# The checkout must be at a shipped Game Knowledge bundle's commit, with the contrib submodules
# lua, pcre, sqlite and zlib. Crawl is built as webtiles; the code diffed is the same in every
# build. The first build takes a few minutes; later ones only rebuild the harnesses.
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
    WEBTILES=y USE_DGAMELAUNCH=y crawl-oracle-melee crawl-oracle-spells crawl-oracle-missiles \
    crawl-oracle-turns >&2
# Cargo runs from the repository, so it uses the pinned toolchain in rust-toolchain.toml.
diff_engine() {
    (cd "$repo" && cargo run --quiet --release -p crawl-oracle -- "$@")
}
version=$("$source/crawl-oracle-melee" "$seed" 0 0 | sed -n 's/.*"crawl":"\([^"]*\)".*/\1/p')
ported=$(mktemp)
trap 'rm -f "$ported"' EXIT
diff_engine ported-spells "$version" > "$ported"

status=0
"$source/crawl-oracle-melee" "$seed" "$cases" "$trials" | diff_engine - || status=1
"$source/crawl-oracle-spells" "$seed" "$cases" "$trials" "$ported" | diff_engine - || status=1
"$source/crawl-oracle-missiles" "$seed" "$cases" "$trials" "$ported" | diff_engine - || status=1
"$source/crawl-oracle-turns" "$seed" "$cases" "$trials" "$ported" | diff_engine - || status=1
exit "$status"

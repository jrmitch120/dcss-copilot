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
verdicts=$(mktemp -d)
trap 'rm -rf "$ported" "$verdicts"' EXIT
diff_engine ported-spells "$version" > "$ported"

# The harnesses are independent and each runs on one core, so they run side by side, each
# verdict held back and printed in this order once all are done.
harnesses=(melee spells missiles turns)
pids=()
for harness in "${harnesses[@]}"; do
    args=("$seed" "$cases" "$trials")
    [[ $harness == melee ]] || args+=("$ported")
    ("$source/crawl-oracle-$harness" "${args[@]}" | diff_engine -) > "$verdicts/$harness" 2>&1 &
    pids+=($!)
done

status=0
for i in "${!harnesses[@]}"; do
    wait "${pids[$i]}" || status=1
    cat "$verdicts/${harnesses[$i]}"
done
exit "$status"

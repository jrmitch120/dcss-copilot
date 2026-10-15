# The test corpus

Tests and evals replay real games. Short, hand-made Recordings live in the main repo next to the tests that use them. Full games are too big for git, so they live in the corpus repo, and the main repo pins each one by hash. Everything here is built and fetched with `tools/corpus`.

| What | Where | Tagged with its Crawl Version in |
|---|---|---|
| Test Recordings: small, often hand-built | `crates/replay/tests/recordings/` | the test that replays it |
| Scenarios: wizard-mode games set up to reach one moment | `crates/replay/tests/scenarios/` | its manifest, `crawl_version` |
| Full games: about 20 seeded qw games per stable release, plus our own | release assets of [`jrmitch120/dcss-copilot-corpus`](https://github.com/jrmitch120/dcss-copilot-corpus), listed in `crates/replay/corpus.toml` | its manifest entry, `crawl_version`, and its release, `crawl-<version>` |

A manifest's `crawl_version` is checked whenever the Recording is replayed: every game in the Recording must report exactly that Crawl Version, or the replay fails.

## Full games

`crates/replay/corpus.toml` lists every corpus game:

```toml
repository = "jrmitch120/dcss-copilot-corpus"

[[game]]
file = "qw-0.34.1-seed-7.dcssrec"
sha256 = "…"                 # of the file; a changed asset is refused
crawl_version = "0.34.1"     # kept in the release crawl-0.34.1
played_by = "qw"             # or "player"
seed = 7                     # for a seeded game
```

Fetch them into the cache before replaying them:

```sh
cargo run -p corpus -- fetch                  # into target/corpus, or $DCSS_COPILOT_CORPUS
cargo test -p copilot-replay --test corpus -- --ignored
```

`fetch` downloads only what isn't cached already, and refuses a download whose SHA-256 isn't the pinned one. No Git LFS is involved: the files are ordinary release assets. The corpus tests are `#[ignore]`d, so `cargo test` never needs the network.

To add a game, record it, upload it to the release for its Crawl Version, and commit the manifest entry `record-qw` prints:

```sh
gh release create crawl-0.34.1 --repo jrmitch120/dcss-copilot-corpus --title "Crawl 0.34.1" --notes ""   # once per version
gh release upload crawl-0.34.1 --repo jrmitch120/dcss-copilot-corpus target/corpus/recorded/qw-0.34.1-seed-7.dcssrec
```

## Scenarios

A scenario is a Recording plus a manifest beside it, `<name>.toml`:

```toml
description = "A Minotaur Berserker enters wizard mode and creates an ogre beside them"
recording = "ogre-beside-a-new-minotaur.dcssrec"
crawl_version = "0.34.1"
snapshot_turns = [0]       # snapshot these turns as they ended, besides the end

[setup]                    # how to record it again
rc = """
combo = MiBe.handaxe
game_seed = 1
"""
keys = ["&", "wiz\r", "m", "ogre\r", ".", "."]

[[alerts]]                 # expected, in order
turn = 2
cause = "threat-level-rose"

[[suggested_actions]]      # expected, in order
turn = 2
rule = "leave-by-stairs"
```

- `rc` is the new account's rc file. `combo` picks the character, weapon included so crawl asks nothing, and `game_seed` picks the dungeon.
- Each entry of `keys` is typed once the game has settled from the one before, so a prompt it opens has been drawn. `"&", "wiz\r"` enters wizard mode; crawl then asks straight away for a wizard command, such as `m` to create a monster by name.
- `alerts` and `suggested_actions` are refused until the Advisor raises them (#37, #38), rather than being silently ignored.

`crates/replay/tests/scenarios.rs` plays every scenario in the directory and snapshots each with `insta`, so a new scenario needs no new test. Review its snapshot like any other.

## Recording on local webtiles

Both scenarios and qw games are recorded by playing a game on a local webtiles server from `tools/corpus`, which captures each message the way the extension does: every message the page hands `comm.handle_message`, `multi` messages split into their children, and `ping` and `close` left out. Each recording uses a fresh account, so no save carries over.

Build crawl at the release's tag with webtiles, and run its server from the source directory:

```sh
git clone --branch 0.34.1 --depth 1 https://github.com/crawl/crawl.git && cd crawl
git submodule update --init --depth 1 crawl-ref/source/contrib/{sqlite,lua,pcre,zlib}
cd crawl-ref/source && make -j4 WEBTILES=y USE_DGAMELAUNCH=y
pip install -r webserver/requirements/base.py3.txt
cat > webserver/games.d/copilot.yaml <<'EOF'
games:
  - id: copilot-qw
    template: default
    name: qw
    options: [-lua-max-memory, "128", -no-throttle]
  - id: copilot-wizard
    template: default
    name: Wizard
    options: [-wizard]
EOF
mkdir -p rcs/running rcs/ttyrecs && python3 webserver/server.py
```

`-wizard` lets the account enter wizard mode on a dgamelaunch build; qw needs the extra Lua memory and no throttle.

Then, from this repo:

```sh
# Records the scenario's setup over its Recording, then checks its crawl_version.
cargo run -p corpus -- record-scenario crates/replay/tests/scenarios/ogre-beside-a-new-minotaur.toml

# Records qw playing seed 7 until the game ends (2 hours at most), into target/corpus/recorded/.
cargo run -p corpus -- record-qw path/to/qw/qw-final.rc --seed 7
```

`--server` points at another server (default `ws://localhost:8080/socket`), and `--game` at another game entry.

### qw

qw is run, never copied: clone [crawl/qw](https://github.com/crawl/qw) and build its rc with `./make-qw.sh -r qw.rc`, which writes `qw-final.rc`. `record-qw` adds `AUTO_START`, turns off qw's delay, and sets `game_seed`. qw keeps its own `combo`. While the game is quiet it presses Escape to clear a `--more--` qw left open, such as after it dies, and it stops with an error the first time crawl reports a Lua error, since qw doesn't recover from one.

**qw does not run on 0.34.1 yet.** Its last commit (July 2024) relies on Lua 5.1's implicit `arg` table, and crawl 0.34 runs Lua 5.4 with no LuaJIT option. It also expects crawl APIs that have since changed, such as `you.strength()` returning a maximum as its second value. Until qw supports 0.34, `record-qw` fails on its first turn, and the corpus has no qw games.

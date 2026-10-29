# The test corpus

Tests and evals replay real games. Short Recordings live in the main repo next to the tests that use them. Full games are too big for git, so they live in the corpus repo, and the main repo pins each one by hash.

| What | Where | Tagged with its Crawl Version in |
|---|---|---|
| Test Recordings: small, often hand-built | `crates/replay/tests/recordings/` | `TAGGED` in `crates/replay/tests/replay.rs`, which must list every one |
| Scenarios: wizard-mode games set up to reach one moment, the main regression set | `crates/replay/tests/scenarios/` | its manifest, `crawl_version` |
| Full games: seeded games played by people, about 10 per stable release, the realism set | release assets of [`jrmitch120/dcss-copilot-corpus`](https://github.com/jrmitch120/dcss-copilot-corpus), listed in `crates/replay/corpus.toml` | its manifest entry, `crawl_version`, and its release, `crawl-<version>` |

The tag is checked whenever the Recording is replayed: its games must report exactly the Crawl Versions it is tagged with, or the replay fails.

## Full games

`crates/replay/corpus.toml` lists every corpus game:

```toml
repository = "jrmitch120/dcss-copilot-corpus"

[[game]]
file = "player-0.34.1-seed-7.dcssrec"
sha256 = "…"                 # of the file; a changed asset is refused
crawl_version = "0.34.1"     # kept in the release crawl-0.34.1
seed = 7                     # for a seeded game
```

Fetch them into the cache before replaying them:

```sh
cargo run -p corpus -- fetch                  # into target/corpus, or $DCSS_COPILOT_CORPUS
cargo test --release -p copilot-replay --test corpus -- --ignored
```

`fetch` downloads only what isn't cached already, and refuses a download whose SHA-256 isn't the pinned one. No Git LFS is involved: the files are ordinary release assets. The corpus tests are `#[ignore]`d, so `cargo test` never needs the network. They check each game replays on the Crawl Version it is tagged with, and snapshot what the detectors noticed across each, `corpus__timeline-<game>.snap`: which Encounters, All Clears, Fact Cards, Alerts, Suggested Actions and Gap Prompts came on which turn. That judges a Fact Sheet at every Settled Update of every game, so run them with `--release`.

### Recording a game

Play on local webtiles (below) with the extension and a dev build of the app, which writes a Recording for each browser connection:

1. Run the app with `pnpm tauri dev`, and load the extension with `pnpm --filter ./extension dev`.
2. Open `http://localhost:8080` in that browser and register an account. Its name is in the Recording, so pick one you're happy to make public.
3. In the lobby, **Edit rc** and set a seed, such as `game_seed = 7`, so the dungeon can be played again.
4. Play one game, then quit the app (Cmd+Q, its menu's Quit, or close its window). Quitting finishes the Recording; stopping `pnpm tauri dev` with Ctrl-C leaves it cut short, and a cut-short game isn't accepted into the corpus. The Recording is named for when it started, and is in the app's data directory under `recordings/`: `~/Library/Application Support/io.github.jrmitch120.dcss-copilot/` on macOS, `~/.local/share/io.github.jrmitch120.dcss-copilot/` on Linux, `%APPDATA%\io.github.jrmitch120.dcss-copilot\` on Windows.

A Recording holds everything the server sent, chat included, so keep the server to yourself.

### Adding a game

Name it `player-<version>-seed-<seed>.dcssrec`, upload it to the release for its Crawl Version, and add its entry to `corpus.toml` (`shasum -a 256 <file>` gives its hash):

```sh
gh release create crawl-0.34.1 --repo jrmitch120/dcss-copilot-corpus --title "Crawl 0.34.1" --notes ""   # once per version
gh release upload crawl-0.34.1 --repo jrmitch120/dcss-copilot-corpus player-0.34.1-seed-7.dcssrec
```

Then write its timeline snapshot, which the corpus test fails without, and review it like any other:

```sh
cargo run -p corpus -- fetch
INSTA_UPDATE=always cargo test --release -p copilot-replay --test corpus -- --ignored
```

Commit `corpus.toml` with the new `corpus__timeline-<game>.snap`; no other snapshot should change.

Never replace an asset in place: a game that changes is a new file with a new hash.

## Scenarios

A scenario is a Recording plus a manifest beside it, `<name>.toml`:

```toml
description = "A Minotaur Berserker enters wizard mode and creates an ogre beside them"
recording = "ogre-beside-a-new-minotaur.dcssrec"
crawl_version = "0.34.1"
snapshot_turns = [0]       # snapshot these turns as they ended

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
- Each entry of `keys` is typed once the game has settled from the one before, so a prompt it opens has been drawn. `"&", "wiz\r"` enters wizard mode; crawl then asks straight away for a wizard command, such as `m` to create a monster by name or `d` to take the stairs down. `l` sets the experience level, so a character can survive a threat that would otherwise kill it outright. A `--more--` swallows the next key, so follow a move that may raise one with `"\u001b"` (Escape), which is harmless when there is none.
- The game may stop after the last of `keys`, as when it is saved (`"S", "y"`) or the character dies, but not before: the Recording then ends as the stream does.
- `before_joining`, if given, is typed like `keys` before the Recording starts: the game is played that far unrecorded and saved, then recorded from when it is played again, so the Advisor joins it part way through, as it does a game the player opens the app in the middle of. A wizard command crawl answers with a prompt of its own, such as `]` for a mutation, which then asks how many levels, needs each answer as an entry of its own.
- `alerts` are the Alerts the Advisor must raise, in order: exactly these, each on its turn, with its `cause` as the timeline writes it, such as `threat-level-rose`, `unique-in-view`, `out-of-depth`, `hit-points-below-half`, `hit-points-below-quarter`, `last-way-out-lost` or `dangerous-status`. No `alerts` means none may be raised.
- `suggested_actions` are the Suggested Actions the Advisor must show, in order: exactly these, each on the turn its rules pick it anew, with its `rule` as the timeline writes it: `leave-by-stairs`, `heal-out-of-it`, `best-escape` or `fight` at LETHAL, and `fight-on`, `fall-back-to-a-corridor`, `fall-back-to-stairs` or `fight-carefully` at DANGEROUS. No `suggested_actions` means none may be shown.

Playing a scenario snapshots the Fact Sheet and the panel at each turn in `snapshot_turns` as it ended, at each level the game arrives on as its first turn there ended, and at the end of the Recording, with what the detectors noticed along the way. `crates/replay/tests/scenarios.rs` plays every scenario in the directory and snapshots each with `insta`, so a new scenario needs no new test. Review its snapshot like any other.

### Recording a scenario

`tools/corpus` plays the scenario's setup on local webtiles and records it over the scenario's Recording, then checks its `crawl_version`:

```sh
cargo run -p corpus -- record-scenario crates/replay/tests/scenarios/ogre-beside-a-new-minotaur.toml
```

It captures each message the way the extension does: every message the page hands `comm.handle_message`, `multi` messages split into their children, and `ping` and `close` left out. Each recording uses a fresh account, so no save carries over. `--server` points at another server (default `ws://localhost:8080/socket`), and `--game` at another game entry (default `copilot-wizard`).

## Local webtiles

Build crawl at the release's tag with webtiles, and run its server from the source directory. Install the server's Python requirements first: the build itself needs PyYAML.

```sh
set -e
git clone --branch 0.34.1 --depth 1 https://github.com/crawl/crawl.git && cd crawl
git submodule update --init --depth 1 crawl-ref/source/contrib/{sqlite,lua,pcre,zlib,libpng}
cd crawl-ref/source
python3 -m venv ~/crawl-venv
~/crawl-venv/bin/pip install -r webserver/requirements/base.py3.txt
PATH="$HOME/crawl-venv/bin:$PATH" make -j4 WEBTILES=y USE_DGAMELAUNCH=y
cat > webserver/games.d/copilot.yaml <<'EOF'
games:
  - id: copilot-wizard
    template: default
    name: Wizard
    options: [-wizard]
EOF
mkdir -p rcs/running rcs/ttyrecs
~/crawl-venv/bin/python webserver/server.py
```

- On macOS, install Xcode's command line tools first (`xcode-select --install`). libpng is only needed there; Linux builds use the system's.
- On Windows, run all of this inside WSL: the webtiles server needs Unix sockets. A Windows browser still reaches it at `http://localhost:8080`.
- `-wizard` lets an account enter wizard mode on a dgamelaunch build. Play your own games on any entry.

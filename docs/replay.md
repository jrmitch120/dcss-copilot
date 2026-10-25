# Replaying Recordings

A Recording (`*.dcssrec`) is everything a Game Source sent during a session: zstd-compressed JSON Lines, a header line then the relay envelopes in the order they arrived. Dev builds of the app write one per browser connection into `recordings/` in the app data directory. Two replayers play them back, both in `crates/replay`.

## In process: tests (seam 1)

`Replayer` feeds a Recording's envelopes straight to the webtiles Game Source and the core. The envelope timestamps are its only clock, so a replay always comes out the same. It moves:

| Move | Lands |
|---|---|
| `step()` | just after the next envelope |
| `step_turn()` | on the next envelope that changes the followed game's turn |
| `seek_turn(t)` | on the first envelope at which the followed game's turn is at least `t`; `None` if the Recording never gets there |
| `seek(n)` | just after envelope `n` (`0` is the start) |

Turns are the followed game's, so on a Recording with several games or tabs, `step_turn()` also stops where the followed game changes, and `seek_turn(t)` finds the first game to reach `t`; `seek(n)` reaches any envelope.

Any backward move rebuilds the game from the start at full speed, because the core only moves forward. `replay()` runs a whole Recording and returns every distinct view model, the identity and Visible State at the end, and the timeline: each Screen Read and Session Event, and what the detectors noticed at each Settled Update (Encounters starting, All Clears, Fact Cards, Alerts and Suggested Actions), in order, with the envelope it came from and the turn just after it (`Replayer::timeline()` gives it so far). The timeline is the main regression signal: scenario snapshots hold it, and so does a snapshot of each corpus game's (`docs/corpus.md`).

`Replayer::remembering(recording, store)` replays with a game history, as the app runs: each game is recalled from the store as it is identified and saved as it changes, so Replayers over one store file are successive runs of the app (`tests/persistence.rs`). A backward move then takes the Recording in again from its start with what the store remembers, as the app does when a browser reconnects. `Replayer` and the app share one wiring of Game Source, Advisor and store, `Following`, so a replay sees a game as the app does.

Tests replay committed Recordings from `crates/replay/tests/recordings/` and Scenarios from `crates/replay/tests/scenarios/`, and assert with `insta` snapshots. `docs/corpus.md` covers Scenarios, recording them on local webtiles, and the corpus of full games.

## Over the websocket: designing the panel

The fake extension plays a Recording to the running app as the browser extension would: it says hello, waits through "Allow this browser?", replays everything the page has received so far, then relays live.

```sh
pnpm tauri dev   # the app, in one terminal
cargo run -p copilot-replay --bin fake-extension -- path/to/game.dcssrec --speed 4 --play
```

`--speed` is `1` for real time, any positive multiple such as `4` or `0.5`, or `max` for as fast as possible. Then type one command per line:

| Command | Does |
|---|---|
| `p` or Enter | play or pause |
| `s` | step one message |
| `t` | step one turn |
| `j <turn>` | jump to the first message at that turn or later |
| `g <n>` | go to just after message `n` |
| `x <speed>` | change the speed |
| `c` | connect again, replaying everything so far |
| `h` | show the commands |
| `q` | quit |

Jumps forward relay at full speed. A backward move is a page reload: the fake extension reconnects, and the app, with no other browser connected, starts afresh from what its game history remembers, which keeps what it read later in the Recording, and takes the replay from the start at full speed. Moves made while the app isn't connected only move the page along; connecting replays them.

The fake extension connects from `chrome-extension://dcss-copilot-fake-extension`, so the app asks once whether to allow it, like any new browser. `--url` and `--origin` override where it connects and from where.

## Reading and writing Recordings

`dcssrec` turns a Recording into JSON Lines and back, so a Recording can be read, and a scenario hand-built or edited:

```sh
cargo run -p copilot-replay --bin dcssrec -- unpack game.dcssrec > game.jsonl
cargo run -p copilot-replay --bin dcssrec -- pack game.jsonl game.dcssrec
```

`pack` takes one envelope per line. A leading header line, as `unpack` prints, keeps its app version.

To see a Recording's lines in `git diff` (`.gitattributes` names the `dcssrec` diff driver):

```sh
cargo build -p copilot-replay --bin dcssrec
git config diff.dcssrec.textconv "$PWD/target/debug/dcssrec unpack"
```

## The LLM sidecar

Tests never call a live model. Every LLM call a dev build makes while it records goes into the Recording's LLM sidecar, `game.llm.jsonl` beside `game.dcssrec`, and tests answer calls from a sidecar instead of a provider. Both sides live in `crates/llm`:

| Type | Does |
|---|---|
| `SidecarRecorder` | wraps a live provider and appends each call as it ends |
| `SidecarReplayer` | answers each call with what was recorded for it, and never calls a model |

A sidecar is JSON Lines, one call per line: the request's hash, the request itself, everything the call streamed and, if it failed, its error. Answers are looked up by the request's hash, so a changed prompt fails with `NotRecorded` and the hash it wanted, rather than replaying a stale answer: record that call again in a dev build. A line whose request was edited by hand no longer matches its hash and is refused. A request asked more often than it was recorded gets its last answer again.

Committed sidecars sit beside the tests that replay them, such as `crates/llm/tests/sidecars/`. They never hold a key: providers keep it out of the request.

## Snapshots

Snapshots are golden files in `tests/snapshots/`, committed and reviewed in diffs like code. A test whose output changed fails, and `insta` writes the new output beside the old as `*.snap.new`. CI never writes snapshots; it only fails.

To update them after an intended change:

```sh
cargo install cargo-insta      # once
cargo insta test --review      # run the tests, then accept or reject each change
```

Without `cargo-insta`, run `INSTA_UPDATE=always cargo test` and read `git diff` on the snapshots. Either way, inspect every changed snapshot and explain it in the commit message: never accept in bulk.

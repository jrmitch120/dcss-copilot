# Coding standards

How code in this repo is laid out and written. Vocabulary comes from `GLOSSARY.md`; architecture decisions live in `docs/adr/`.

## Craftsmanship

You are the steward of this codebase. Build it thoughtfully and purposefully, with a clear vision towards maintenance and longevity. Write for the next reader: someone arriving cold, with only the code, the glossary and the ADRs.

- **Types carry the rules.** Encode invariants so the compiler enforces them: a Fact Sheet number always has its source; a Stale fact can't be read as Known.
- **Fail loudly, label approximations.** Every fallback, unknown and approximation is visible to the caller and, where it matters, the player. The code is as honest as the Advisor.
- **Crawl changes land as data.** Version-specific knowledge lives in Game Knowledge bundles and formula variants.
- **Every dependency earns its place.** Prefer the standard library and what the workspace already uses; justify a new dependency in its commit message.
- **Delete what's dead** in the code you touch: unused code, flags and stale comments.
- **File improvements outside the change's subject as issues.**

## Layout

| Path | What lives there |
|---|---|
| `crates/core` | Visible State, Accumulated State, the engine, detectors and the renderer. Pure: no IO, no clocks, no network. |
| `crates/webtiles` | The webtiles Game Source adapter. The only code that decodes the webtiles protocol. |
| `crates/gk` | The Game Knowledge bundle schema, shared by the extractor and `core`. |
| `crates/llm` | The `LlmProvider` trait, provider adapters, and LLM sidecar record and replay. |
| `crates/store` | SQLite persistence. |
| `crates/replay` | The Recording format, the in-process replayer and the fake-extension front end. |
| `app/src-tauri` | The Tauri shell. Wires the crates together and owns windows, the tray and the keychain. |
| `app/src` | The Svelte 5 panel. Renders the view model; holds no game logic. |
| `extension` | The WXT extension. A pure relay: forwards raw messages, parses and stores nothing (ADR 0001). |
| `tools/*` | Offline binaries: the Game Knowledge extractor, the crawl oracle, and the corpus recorder and fetcher. |

Each crate is a deep module: a small public surface over a lot of behaviour. Add a new crate only when it brings a separate binary or a dependency that must stay out of `core`; grow existing modules otherwise.

The webtiles protocol stays behind the Game Source seam: everything past `crates/webtiles` speaks source-neutral types. The extension and `tools/corpus` stand where a browser does: they know how the page frames messages, and the corpus recorder logs in, starts a game and types as a player would, but neither reads what a game message means.

## Domain language

Name types and functions with glossary terms: `FactSheet`, `ThreatLevel`, `WayOut`, `CandidateAction`, `DecisionPoint`, `RunPlan`, `Gap`. When code needs a concept the glossary lacks, add the term to `GLOSSARY.md` in the same change.

## Prefer the cleaner design over the smaller diff

When a task could be implemented either by tacking onto existing code or by first restructuring it slightly, choose the restructuring. "Minimal change" is not a goal in itself; a readable final state is. Make the change easy, then make the easy change: land the refactor as its own commit, then the behaviour change on top of it.

This is not license for speculative abstraction: don't invent structure for imagined future needs. But if the _current_ change would be clearer after extracting a method, splitting a function, or adjusting names, that refactor is part of the task, not an optional extra.

If you catch yourself thinking any of these, stop and refactor first:

- "This does a bit of wasted work, but it's harmless."
- "I'll just add the new behavior alongside the old."
- "The existing method does more than I need, but calling it is fine."

_Adapted from [lazygit's AGENTS.md](https://github.com/jesseduffield/lazygit/blob/master/AGENTS.md#prefer-the-cleaner-design-over-the-smaller-diff) (MIT)._

## Testing

Tests drive the system through its seams and assert on what comes out. The seams, in order of preference:

1. **Recording replay**: a Recording (plus its LLM sidecar) goes in; the view model, timeline, Fact Sheet snapshots and rendered prompts come out. Most behaviour is tested here.
2. **Crawl oracle**: ported formulas diffed against a crawl build (`docs/crawl-oracle.md`).
3. **Offline builders**: fixed input files in, output files out.
4. **Extension relay**: a fake page `comm` in, envelopes out.

Write a unit test only for logic a seam test can't reach precisely, such as a probability distribution. Snapshots use `insta` and are golden files reviewed in diffs; `docs/replay.md` covers the replayers, committed Recordings and how to update snapshots; `docs/corpus.md` covers Scenarios and the corpus of full games. Tests never call a live LLM; they replay the sidecar.

## Rust

- The pinned toolchain, `rustfmt` defaults, and the workspace lint set in the root `Cargo.toml` apply to every crate (`[lints] workspace = true`).
- Libraries return typed errors with `thiserror`. Binaries use `anyhow`.
- Handle every `Option` and `Result` outside tests; `unwrap` and `expect` are denied by lint. The one allowed `expect` is Tauri's startup in `app/src-tauri`, marked with `#[allow]`.
- Doc comments go on public items, saying what the item is for. Inline comments explain why, in one line.
- Constants tuned against the corpus (Threat Level thresholds, word limits) are named constants with a comment pointing at the decision that set them.

## TypeScript and Svelte

- Strict TypeScript everywhere.
- Svelte 5 runes (`$state`, `$derived`, `$effect`, `$props`) for all reactivity.
- The view-model types in `app/src/lib/bindings.ts` are generated from Rust by `tauri-specta`. Change the Rust type, then run `cargo test -p copilot-app bindings` to regenerate; CI fails if the committed file is stale. The file is never edited by hand.
- Biome does both lint and format. Vitest runs the extension's tests.

## Before you're done

Every changed snapshot is intended: inspect each snapshot diff and explain it in the commit message, never accept in bulk. `GLOSSARY.md` and docs match the code, and every one of these passes:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm lint
pnpm check
pnpm test
```

The lefthook pre-commit hook formats staged files; CI runs everything, plus app builds on macOS, Windows and Linux on `main`.

//! The corpus: full games kept as release assets of the corpus repo, pinned by hash in
//! `corpus.toml` and replayed from a local cache.

// Helpers outside #[test] functions panic on bad fixtures too.
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use copilot_replay::{Corpus, CorpusError, Recording, Replayer};

const ONE_GAME: &str = r#"
repository = "jrmitch120/dcss-copilot-corpus"

[[game]]
file = "player-0.34.1-seed-7.dcssrec"
sha256 = "2c26b46b68ffc68ff99b453c1d30413413422d706483bfa0f98a5e886266e7ae"
crawl_version = "0.34.1"
seed = 7
"#;

/// A fresh, empty cache directory for one test.
fn empty_cache(test: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("dcss-copilot-corpus-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn the_committed_corpus_manifest_reads() {
    Corpus::committed().unwrap();
}

#[test]
fn a_game_is_fetched_from_the_release_named_for_its_crawl_version() {
    let corpus = Corpus::parse(ONE_GAME).unwrap();

    assert_eq!(
        corpus.url(&corpus.games[0]),
        "https://github.com/jrmitch120/dcss-copilot-corpus/releases/download/crawl-0.34.1/player-0.34.1-seed-7.dcssrec"
    );
}

#[test]
fn a_cached_game_is_read_only_if_it_matches_its_pinned_hash() {
    let corpus = Corpus::parse(ONE_GAME).unwrap();
    let game = &corpus.games[0];
    let cache = empty_cache("hash");

    assert!(matches!(
        game.read_cached(&cache),
        Err(CorpusError::NotFetched { .. })
    ));

    std::fs::write(game.cached_path(&cache), b"bar").unwrap();
    assert!(matches!(
        game.read_cached(&cache),
        Err(CorpusError::HashMismatch { .. })
    ));

    // The pinned hash is sha256("foo").
    std::fs::write(game.cached_path(&cache), b"foo").unwrap();
    assert_eq!(game.read_cached(&cache).unwrap(), b"foo");
}

#[test]
#[ignore = "needs the corpus: cargo run -p corpus -- fetch"]
fn every_corpus_game_replays_on_the_crawl_version_it_is_tagged_with() {
    let corpus = Corpus::committed().unwrap();
    let cache = Corpus::cache_dir();
    for game in &corpus.games {
        let bytes = game
            .read_cached(&cache)
            .unwrap_or_else(|error| panic!("{error}"));
        let recording = Recording::read(&bytes[..]).unwrap();
        assert!(!recording.truncated, "{} is cut short", game.file);
        let replayer = Replayer::new(recording).unwrap();

        if let Err(mismatch) = replayer.check_crawl_version(&game.crawl_version) {
            panic!("{} is {mismatch}", game.file);
        }
    }
}

//! The corpus: full games kept out of the main repo as release assets of the corpus repo, listed
//! and pinned by hash in `corpus.toml`, and read from a local cache once fetched.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};

/// The committed corpus manifest.
const COMMITTED: &str = include_str!("../corpus.toml");

/// Overrides where fetched corpus games are cached.
pub const CORPUS_DIR_VAR: &str = "DCSS_COPILOT_CORPUS";

/// The corpus manifest: every game in the corpus and where it is kept.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Corpus {
    /// The GitHub repository whose releases hold the games, as `owner/name`.
    pub repository: String,
    /// The games.
    #[serde(default, rename = "game")]
    pub games: Vec<CorpusGame>,
}

/// One full game in the corpus: a Recording kept as a release asset.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorpusGame {
    /// The asset's file name.
    pub file: String,
    /// The SHA-256 of the file, in lowercase hex: a changed asset is refused.
    pub sha256: String,
    /// The Crawl Version the game was played on, which its Recording must report. The asset is
    /// kept in the release `crawl-<version>`.
    pub crawl_version: String,
    /// Who played it.
    pub played_by: PlayedBy,
    /// The dungeon seed, for a seeded game.
    #[serde(default)]
    pub seed: Option<u64>,
}

/// Who played a corpus game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PlayedBy {
    /// The bot qw, on local webtiles.
    Qw,
    /// A person.
    Player,
}

/// Why the corpus couldn't be read.
#[derive(Debug, thiserror::Error)]
pub enum CorpusError {
    /// The manifest isn't a corpus manifest.
    #[error("corpus manifest is malformed: {0}")]
    Malformed(#[from] Box<toml::de::Error>),
    /// The game hasn't been fetched into the cache.
    #[error("corpus game {file} isn't in {dir}: fetch it with `cargo run -p corpus -- fetch`")]
    NotFetched {
        /// The game's file name.
        file: String,
        /// The cache.
        dir: PathBuf,
    },
    /// The cached file couldn't be read.
    #[error("corpus game {path} couldn't be read: {source}")]
    Io {
        /// The cached file.
        path: PathBuf,
        /// Why.
        source: std::io::Error,
    },
    /// The file's hash isn't the one the manifest pins.
    #[error("corpus game {file} has SHA-256 {actual}, but the manifest pins {pinned}")]
    HashMismatch {
        /// The game's file name.
        file: String,
        /// The manifest's hash.
        pinned: String,
        /// The file's hash.
        actual: String,
    },
}

impl Corpus {
    /// The corpus manifest committed with this build.
    ///
    /// # Errors
    /// If it isn't a corpus manifest.
    pub fn committed() -> Result<Self, CorpusError> {
        Self::parse(COMMITTED)
    }

    /// Parses a corpus manifest.
    ///
    /// # Errors
    /// If `text` isn't a corpus manifest.
    pub fn parse(text: &str) -> Result<Self, CorpusError> {
        toml::from_str(text).map_err(|error| CorpusError::Malformed(Box::new(error)))
    }

    /// Where fetched games are cached: `$DCSS_COPILOT_CORPUS`, or `target/corpus` in the
    /// workspace.
    #[must_use]
    pub fn cache_dir() -> PathBuf {
        std::env::var_os(CORPUS_DIR_VAR).map_or_else(
            || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/corpus"),
            PathBuf::from,
        )
    }

    /// Where `game` is downloaded from.
    #[must_use]
    pub fn url(&self, game: &CorpusGame) -> String {
        format!(
            "https://github.com/{}/releases/download/{}/{}",
            self.repository,
            game.release(),
            game.file
        )
    }
}

impl CorpusGame {
    /// The release that holds the game: one per Crawl Version, so the tag names it.
    #[must_use]
    pub fn release(&self) -> String {
        format!("crawl-{}", self.crawl_version)
    }

    /// Where the game is cached in `dir`.
    #[must_use]
    pub fn cached_path(&self, dir: &Path) -> PathBuf {
        dir.join(&self.file)
    }

    /// The cached game's bytes, once they match the pinned hash.
    ///
    /// # Errors
    /// If it isn't cached, can't be read, or isn't what the manifest pins.
    pub fn read_cached(&self, dir: &Path) -> Result<Vec<u8>, CorpusError> {
        let path = self.cached_path(dir);
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(CorpusError::NotFetched {
                    file: self.file.clone(),
                    dir: dir.to_owned(),
                });
            }
            Err(source) => return Err(CorpusError::Io { path, source }),
        };
        self.check(&bytes)?;
        Ok(bytes)
    }

    /// Checks `bytes` are the file the manifest pins.
    ///
    /// # Errors
    /// If their hash isn't the pinned one.
    pub fn check(&self, bytes: &[u8]) -> Result<(), CorpusError> {
        let actual = sha256_hex(bytes);
        if actual == self.sha256 {
            Ok(())
        } else {
            Err(CorpusError::HashMismatch {
                file: self.file.clone(),
                pinned: self.sha256.clone(),
                actual,
            })
        }
    }
}

/// The SHA-256 of `bytes` in lowercase hex, as a corpus manifest pins it.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

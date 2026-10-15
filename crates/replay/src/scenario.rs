//! Scenarios: wizard-mode games set up by hand to reach one moment, each a Recording plus a
//! manifest saying how it was set up and what the Advisor should make of it.

use std::fs::File;
use std::path::{Path, PathBuf};

use copilot_core::{FactSheet, GameIdentity, GameView};
use serde::{Deserialize, Serialize};

use crate::{Recording, ReplayError, Replayer};

/// The file extension scenario manifests are saved with, beside their Recording.
pub const SCENARIO_EXTENSION: &str = "toml";

/// A scenario's manifest: its Recording, the Crawl Version it was recorded on, the keystrokes that
/// set it up, and what the Advisor is expected to make of it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    /// What the scenario sets up, in a sentence.
    pub description: String,
    /// The Recording, resolved against the manifest's directory.
    pub recording: PathBuf,
    /// The Crawl Version the Recording was made on, which its game must report.
    pub crawl_version: String,
    /// How to record the scenario again.
    pub setup: Setup,
    /// Turns to snapshot as they ended, besides the end of the Recording.
    #[serde(default)]
    pub snapshot_turns: Vec<u32>,
    /// The Alerts the Advisor should raise, in order.
    #[serde(default)]
    pub alerts: Vec<ExpectedAlert>,
    /// The Suggested Actions the Advisor should show, in order.
    #[serde(default)]
    pub suggested_actions: Vec<ExpectedSuggestedAction>,
}

/// How a scenario is recorded on local webtiles: a new character on a fresh account, so no save
/// carries over, played by typing `keys`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Setup {
    /// Crawl options for the account's rc file, such as `combo` and `game_seed`, which pick the
    /// character and the dungeon.
    pub rc: String,
    /// What to type, in order. Each entry is sent as one input once the game has settled from
    /// the last, so a prompt it opens has been drawn.
    pub keys: Vec<String>,
}

/// An Alert a scenario expects on a turn.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedAlert {
    /// The turn it fires on.
    pub turn: u32,
    /// What raised it, such as `threat-level-rose`.
    pub cause: String,
}

/// A Suggested Action a scenario expects on a turn.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedSuggestedAction {
    /// The turn it is shown on.
    pub turn: u32,
    /// The rule that picked it, such as `leave-by-stairs`.
    pub rule: String,
}

/// What the Advisor made of a scenario.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioReplay {
    /// The followed game's identity at the end.
    pub identity: Option<GameIdentity>,
    /// Each turn the manifest lists as it ended, then the end of the Recording.
    pub snapshots: Vec<Snapshot>,
}

/// What the Advisor computed and showed at one point of a scenario.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// The followed game's turn.
    pub turn: Option<u32>,
    /// The Fact Sheet.
    pub fact_sheet: FactSheet,
    /// The panel's view of the game.
    pub game: Option<GameView>,
}

/// Why a scenario couldn't be loaded or played.
#[derive(Debug, thiserror::Error)]
pub enum ScenarioError {
    /// The manifest couldn't be read.
    #[error("scenario {path} couldn't be read: {source}")]
    Io {
        /// The manifest.
        path: PathBuf,
        /// Why.
        source: std::io::Error,
    },
    /// The manifest isn't a scenario manifest.
    #[error("scenario {path} is malformed: {source}")]
    Malformed {
        /// The manifest.
        path: PathBuf,
        /// What was wrong with it.
        source: Box<toml::de::Error>,
    },
    /// The Recording couldn't be replayed.
    #[error("scenario recording {path} couldn't be replayed: {source}")]
    Replay {
        /// The Recording.
        path: PathBuf,
        /// Why.
        source: ReplayError,
    },
    /// The Recording's games report another Crawl Version than the manifest is tagged with.
    #[error("scenario is tagged with Crawl Version {tagged} but its games report {reported:?}")]
    CrawlVersionMismatch {
        /// The manifest's tag.
        tagged: String,
        /// What the Recording's games reported.
        reported: Vec<String>,
    },
    /// The manifest asks for a snapshot on a turn the followed game never reported.
    #[error("scenario asks for a snapshot on turn {0}, which the game never reported")]
    TurnNeverReached(u32),
    /// The manifest expects Alerts or Suggested Actions, which the Advisor can't raise yet.
    #[error(
        "scenario expects Alerts or Suggested Actions, but the Advisor raises none yet (#37, #38)"
    )]
    AdviceNotYetRaised,
}

impl Scenario {
    /// Reads the manifest at `path`.
    ///
    /// # Errors
    /// If it can't be read or isn't a scenario manifest.
    pub fn load(path: &Path) -> Result<Self, ScenarioError> {
        let text = std::fs::read_to_string(path).map_err(|source| ScenarioError::Io {
            path: path.to_owned(),
            source,
        })?;
        Self::parse(&text, path)
    }

    /// Parses the manifest `text`, read from `path`.
    ///
    /// # Errors
    /// If it isn't a scenario manifest.
    pub fn parse(text: &str, path: &Path) -> Result<Self, ScenarioError> {
        let mut scenario: Self =
            toml::from_str(text).map_err(|source| ScenarioError::Malformed {
                path: path.to_owned(),
                source: Box::new(source),
            })?;
        if let Some(dir) = path.parent() {
            scenario.recording = dir.join(&scenario.recording);
        }
        Ok(scenario)
    }

    /// Replays the scenario's Recording, checks it against its manifest and snapshots it.
    ///
    /// # Errors
    /// If the Recording can't be replayed, its games report another Crawl Version than its tag, a
    /// snapshot turn is never reached, or the manifest expects advice the Advisor can't raise.
    pub fn play(&self) -> Result<ScenarioReplay, ScenarioError> {
        if !self.alerts.is_empty() || !self.suggested_actions.is_empty() {
            return Err(ScenarioError::AdviceNotYetRaised);
        }
        let replay_error = |source| ScenarioError::Replay {
            path: self.recording.clone(),
            source,
        };
        let file = File::open(&self.recording)
            .map_err(|error| replay_error(ReplayError::Recording(error.into())))?;
        let recording = Recording::read(file).map_err(|error| replay_error(error.into()))?;
        let mut replayer = Replayer::new(recording).map_err(|error| replay_error(error.into()))?;

        let reported: Vec<String> = replayer
            .crawl_versions()
            .iter()
            .map(|version| version.0.clone())
            .collect();
        if reported != [self.crawl_version.clone()] {
            return Err(ScenarioError::CrawlVersionMismatch {
                tagged: self.crawl_version.clone(),
                reported,
            });
        }

        let mut by_turn: Vec<Option<Snapshot>> = vec![None; self.snapshot_turns.len()];
        while !replayer.at_end() {
            replayer.step();
            // The last snapshot taken on a turn is that turn as it ended.
            for (wanted, snapshot) in self.snapshot_turns.iter().zip(&mut by_turn) {
                if replayer.turn() == Some(*wanted) {
                    *snapshot = Some(Snapshot::of(&replayer));
                }
            }
        }
        let mut snapshots = Vec::with_capacity(by_turn.len() + 1);
        for (wanted, snapshot) in self.snapshot_turns.iter().zip(by_turn) {
            snapshots.push(snapshot.ok_or(ScenarioError::TurnNeverReached(*wanted))?);
        }
        snapshots.push(Snapshot::of(&replayer));
        Ok(ScenarioReplay {
            identity: replayer.advisor().identity().cloned(),
            snapshots,
        })
    }
}

impl Snapshot {
    fn of(replayer: &Replayer) -> Self {
        Self {
            turn: replayer.turn(),
            fact_sheet: replayer.advisor().fact_sheet().clone(),
            game: replayer.view_model().game,
        }
    }
}

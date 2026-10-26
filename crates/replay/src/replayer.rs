//! The in-process replayer: a Recording's envelopes fed straight to the webtiles Game Source and
//! the core, one message, one turn or one jump at a time.

use std::ops::Range;

use copilot_core::{
    Advisor, Appearance, AssistanceLevel, Connection, CrawlVersion, SourceEvent, ViewModel,
};
use copilot_gk::{BundleError, Library};
use copilot_store::Store;
use copilot_webtiles::{Envelope, WebtilesSource};
use serde::Serialize;

use crate::Recording;
use crate::following::{Following, Happened};

/// Replays a Recording through the webtiles Game Source and the core, as if a browser had relayed
/// it live.
///
/// The envelope timestamps are its only clock: nothing reads the wall clock, so a replay always
/// comes out the same.
pub struct Replayer {
    recording: Recording,
    /// The followed game's turn once that many envelopes have been applied: `turns[0]` is before
    /// any.
    turns: Vec<Option<u32>>,
    /// Every Crawl Version the followed games report, in the order first seen.
    crawl_versions: Vec<CrawlVersion>,
    position: usize,
    game: Game,
}

/// A Recording's games report another Crawl Version than the one it is tagged with, or more than
/// one.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "tagged with Crawl Version {tagged} but its games report [{}]",
    list(reported)
)]
pub struct CrawlVersionMismatch {
    /// The tag.
    pub tagged: CrawlVersion,
    /// What the Recording's games reported, in the order first seen.
    pub reported: Vec<CrawlVersion>,
}

fn list(versions: &[CrawlVersion]) -> String {
    versions
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

/// What a move applied: the envelopes delivered, in Recording order, and whether the game was
/// rebuilt from the start first, as every backward move is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    /// The game was rebuilt from nothing before `envelopes` were applied.
    pub restarted: bool,
    /// The indexes of the envelopes applied.
    pub envelopes: Range<usize>,
}

/// Something that happened, rather than what the game now shows: a screen read or a session
/// event the Game Source reported, or what the Advisor's detectors noticed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Moment {
    /// The envelope it came from, counting from 0.
    pub envelope: usize,
    /// The followed game's turn just after it.
    pub turn: Option<u32>,
    /// What happened.
    #[serde(flatten)]
    pub event: Happened,
}

/// The Game Source and the core, fed together, and what has happened so far.
struct Game {
    following: Following,
    /// What has happened so far, in order.
    timeline: Vec<Moment>,
    /// How many envelopes have been applied.
    applied: usize,
}

impl Game {
    fn new(following: Following) -> Self {
        Self {
            following,
            timeline: Vec::new(),
            applied: 0,
        }
    }

    /// Takes the Recording in again from its start, as the app does when a browser connects:
    /// keeping only what the store remembers, if it has one.
    fn restart(&mut self) {
        self.following.restart();
        self.timeline.clear();
        self.applied = 0;
    }

    fn apply(&mut self, envelope: &Envelope) {
        let happened = self.following.ingest(envelope);
        let turn = self.following.advisor().visible().turn;
        self.timeline
            .extend(happened.into_iter().map(|event| Moment {
                envelope: self.applied,
                turn,
                event,
            }));
        self.applied += 1;
    }
}

impl Replayer {
    /// Starts before the Recording's first envelope, with the Game Knowledge the app ships.
    ///
    /// # Errors
    /// If a shipped Game Knowledge bundle doesn't read, rather than replay without it.
    pub fn new(recording: Recording) -> Result<Self, BundleError> {
        Ok(Self::with_knowledge(recording, &Library::shipped()?))
    }

    /// Starts before the Recording's first envelope, with the Game Knowledge the app ships,
    /// remembering games in `store` as the app does, across Replayers as across runs of the app.
    /// Moving backward then takes the Recording in again from its start with what the store
    /// remembers, as the app does when a browser reconnects.
    ///
    /// # Errors
    /// If a shipped Game Knowledge bundle doesn't read, rather than replay without it.
    pub fn remembering(recording: Recording, store: Store) -> Result<Self, BundleError> {
        let knowledge = Library::shipped()?;
        let following = Following::remembering(&knowledge, store);
        Ok(Self::following(recording, &knowledge, following))
    }

    /// Starts before the Recording's first envelope, with only the Game Knowledge in
    /// `knowledge`, such as none at all.
    #[must_use]
    pub fn with_knowledge(recording: Recording, knowledge: &Library) -> Self {
        let following = Following::new(knowledge);
        Self::following(recording, knowledge, following)
    }

    fn following(recording: Recording, knowledge: &Library, following: Following) -> Self {
        // The Game Source alone tells each envelope's turn: the Advisor would judge every
        // Settled Update on the way.
        let mut source = WebtilesSource::new(knowledge.clone());
        let mut turn = None;
        let mut turns = Vec::with_capacity(recording.envelopes.len() + 1);
        turns.push(turn);
        let mut crawl_versions = Vec::new();
        for envelope in &recording.envelopes {
            for event in source.ingest(envelope) {
                match event {
                    SourceEvent::Visible(visible) => turn = visible.turn,
                    SourceEvent::Identified(identity) => {
                        if let Some(version) = identity
                            .crawl_version
                            .filter(|version| !crawl_versions.contains(version))
                        {
                            crawl_versions.push(version);
                        }
                    }
                    SourceEvent::ScreenRead(_) | SourceEvent::Session(_) | SourceEvent::Settled => {
                    }
                }
            }
            turns.push(turn);
        }
        Self {
            recording,
            turns,
            crawl_versions,
            position: 0,
            game: Game::new(following),
        }
    }

    /// Every Crawl Version the Recording's followed games report, in the order first seen: what
    /// a manifest tags the Recording with.
    #[must_use]
    pub fn crawl_versions(&self) -> &[CrawlVersion] {
        &self.crawl_versions
    }

    /// Checks the Recording's games all report `tagged`, the Crawl Version a manifest tags it
    /// with.
    ///
    /// # Errors
    /// If they report any other, or none.
    pub fn check_crawl_version(&self, tagged: &CrawlVersion) -> Result<(), CrawlVersionMismatch> {
        if self.crawl_versions == [tagged.clone()] {
            Ok(())
        } else {
            Err(CrawlVersionMismatch {
                tagged: tagged.clone(),
                reported: self.crawl_versions.clone(),
            })
        }
    }

    /// The Recording being replayed.
    #[must_use]
    pub fn recording(&self) -> &Recording {
        &self.recording
    }

    /// How many envelopes have been applied.
    #[must_use]
    pub fn position(&self) -> usize {
        self.position
    }

    /// Every envelope has been applied.
    #[must_use]
    pub fn at_end(&self) -> bool {
        self.position == self.recording.envelopes.len()
    }

    /// The envelope the next step applies.
    #[must_use]
    pub fn next_envelope(&self) -> Option<&Envelope> {
        self.recording.envelopes.get(self.position)
    }

    /// The replay's clock: when the last applied envelope was received, in Unix milliseconds.
    #[must_use]
    pub fn clock_ms(&self) -> Option<u64> {
        let last = self.position.checked_sub(1)?;
        self.recording
            .envelopes
            .get(last)
            .map(|envelope| envelope.ts)
    }

    /// The followed game's turn, once a game has reported one.
    #[must_use]
    pub fn turn(&self) -> Option<u32> {
        self.turns.get(self.position).copied().flatten()
    }

    /// The core, as the applied envelopes left it.
    #[must_use]
    pub fn advisor(&self) -> &Advisor {
        self.game.following.advisor()
    }

    /// The screen reads, session events and what the detectors noticed so far, in order.
    #[must_use]
    pub fn timeline(&self) -> &[Moment] {
        &self.game.timeline
    }

    /// What the detectors noticed so far, in order: the timeline without the Game Source's own
    /// events.
    pub fn detections(&self) -> impl Iterator<Item = &Moment> {
        self.timeline()
            .iter()
            .filter(|moment| matches!(moment.event, Happened::Detected(_)))
    }

    /// What the panel would show, with the browser connected, the default Appearance and the
    /// default Assistance Level.
    #[must_use]
    pub fn view_model(&self) -> ViewModel {
        self.view_model_at(AssistanceLevel::default())
    }

    /// What the panel would show at the player's `assistance` level, with the browser connected
    /// and the default Appearance.
    #[must_use]
    pub fn view_model_at(&self, assistance: AssistanceLevel) -> ViewModel {
        ViewModel {
            app_version: self.recording.header.app_version.clone(),
            connection: Connection::Connected {
                buffer_overflowed: false,
            },
            appearance: Appearance::default(),
            assistance,
            // A replay fails rather than run without the shipped Game Knowledge.
            unreadable_game_knowledge: None,
            history_unsaved: self.game.following.history_unsaved().map(str::to_owned),
            game: self.game.following.game_view(assistance),
        }
    }

    /// The player answered whether the followed game is the one its game process picked up
    /// again on a later turn, as just after the envelopes applied so far. A backward move forgets
    /// the answer, as it rebuilds the game from the start, unless the store remembers it.
    pub fn answer_same_game(&mut self, same: bool) {
        let happened = self.game.following.answer_same_game(same);
        let turn = self.game.following.advisor().visible().turn;
        let envelope = self.game.applied.saturating_sub(1);
        self.game
            .timeline
            .extend(happened.into_iter().map(|event| Moment {
                envelope,
                turn,
                event,
            }));
    }

    /// The player put the join card away, as just after the envelopes applied so far. A backward
    /// move forgets it, as it rebuilds the game from the start.
    pub fn close_join_card(&mut self) {
        self.game.following.close_join_card();
    }

    /// Applies the next envelope, if any.
    pub fn step(&mut self) -> Applied {
        self.seek(self.position + 1)
    }

    /// Applies envelopes up to and including the next one that changes the followed game's turn,
    /// or to the end.
    pub fn step_turn(&mut self) -> Applied {
        let now = self.turn();
        let target = (self.position + 1..self.turns.len())
            .find(|&position| self.turns[position] != now)
            .unwrap_or(self.recording.envelopes.len());
        self.seek(target)
    }

    /// Moves to just after the first envelope at which the followed game's turn is at least
    /// `turn`, or stays put and returns `None` if the Recording never gets there.
    pub fn seek_turn(&mut self, turn: u32) -> Option<Applied> {
        let target = self
            .turns
            .iter()
            .position(|reached| reached.is_some_and(|reached| reached >= turn))?;
        Some(self.seek(target))
    }

    /// Moves to just after `position` envelopes, or to the end. Moving backward rebuilds the game
    /// from the start, because the core only moves forward.
    pub fn seek(&mut self, position: usize) -> Applied {
        let target = position.min(self.recording.envelopes.len());
        let restarted = target < self.position;
        if restarted {
            self.game.restart();
            self.position = 0;
        }
        let start = self.position;
        for envelope in &self.recording.envelopes[start..target] {
            self.game.apply(envelope);
        }
        self.position = target;
        Applied {
            restarted,
            envelopes: start..target,
        }
    }
}

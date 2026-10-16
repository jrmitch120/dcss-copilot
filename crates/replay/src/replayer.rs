//! The in-process replayer: a Recording's envelopes fed straight to the webtiles Game Source and
//! the core, one message, one turn or one jump at a time.

use std::ops::Range;

use copilot_core::{Advisor, Appearance, Connection, CrawlVersion, SourceEvent, ViewModel};
use copilot_gk::{BundleError, Library};
use copilot_webtiles::{Envelope, WebtilesSource};
use serde::Serialize;

use crate::Recording;

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
    knowledge: Library,
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

/// Something the Game Source reported that happened rather than what the game now shows: a
/// screen read, or a session event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Moment {
    /// The envelope it came from, counting from 0.
    pub envelope: usize,
    /// The followed game's turn just after it.
    pub turn: Option<u32>,
    /// What happened.
    #[serde(flatten)]
    pub event: SourceEvent,
}

/// The Game Source and the core, fed together.
struct Game {
    source: WebtilesSource,
    advisor: Advisor,
    /// What has happened so far, in order.
    timeline: Vec<Moment>,
    /// How many envelopes have been applied.
    applied: usize,
}

impl Game {
    fn new(knowledge: &Library) -> Self {
        Self {
            source: WebtilesSource::new(knowledge.clone()),
            advisor: Advisor::new(knowledge.clone()),
            timeline: Vec::new(),
            applied: 0,
        }
    }

    fn apply(&mut self, envelope: &Envelope) {
        let events = self.source.ingest(envelope);
        for event in &events {
            self.advisor.apply(event.clone());
        }
        let turn = self.advisor.visible().turn;
        self.timeline.extend(
            events
                .into_iter()
                .filter(|event| {
                    matches!(event, SourceEvent::ScreenRead(_) | SourceEvent::Session(_))
                })
                .map(|event| Moment {
                    envelope: self.applied,
                    turn,
                    event,
                }),
        );
        self.applied += 1;
    }
}

impl Replayer {
    /// Starts before the Recording's first envelope, with the Game Knowledge the app ships.
    ///
    /// # Errors
    /// If a shipped Game Knowledge bundle doesn't read, rather than replay without it.
    pub fn new(recording: Recording) -> Result<Self, BundleError> {
        Ok(Self::with_knowledge(recording, Library::shipped()?))
    }

    /// Starts before the Recording's first envelope, with only the Game Knowledge in
    /// `knowledge`, such as none at all.
    #[must_use]
    pub fn with_knowledge(recording: Recording, knowledge: Library) -> Self {
        let mut game = Game::new(&knowledge);
        let mut turns = Vec::with_capacity(recording.envelopes.len() + 1);
        turns.push(None);
        let mut crawl_versions = Vec::new();
        for envelope in &recording.envelopes {
            game.apply(envelope);
            turns.push(game.advisor.visible().turn);
            if let Some(version) = game
                .advisor
                .identity()
                .and_then(|identity| identity.crawl_version.as_ref())
                .filter(|version| !crawl_versions.contains(*version))
            {
                crawl_versions.push(version.clone());
            }
        }
        Self {
            recording,
            turns,
            crawl_versions,
            position: 0,
            game: Game::new(&knowledge),
            knowledge,
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
        &self.game.advisor
    }

    /// The screen reads and session events so far, in order.
    #[must_use]
    pub fn timeline(&self) -> &[Moment] {
        &self.game.timeline
    }

    /// What the panel would show, with the browser connected and the default Appearance.
    #[must_use]
    pub fn view_model(&self) -> ViewModel {
        ViewModel {
            app_version: self.recording.header.app_version.clone(),
            connection: Connection::Connected {
                buffer_overflowed: false,
            },
            appearance: Appearance::default(),
            // A replay fails rather than run without the shipped Game Knowledge.
            unreadable_game_knowledge: None,
            game: self.game.advisor.game_view(),
        }
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
            self.game = Game::new(&self.knowledge);
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

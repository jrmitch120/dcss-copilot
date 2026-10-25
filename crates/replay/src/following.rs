//! A Game Source's games, followed by the Advisor and remembered in the store: the wiring the app
//! runs live and the replayer replays, so a replay sees a game as the app does.

use copilot_core::{Advisor, Detection, GameIdentity, KnownGames, Role, SourceEvent};
use copilot_gk::Library;
use copilot_store::Store;
use copilot_webtiles::{Envelope, WebtilesSource};
use serde::Serialize;

/// The webtiles Game Source and the Advisor it feeds, always built together from the same Game
/// Knowledge, and the store that remembers the games they follow between runs of the app.
///
/// A game is recalled from the store as the Game Source identifies it, and saved after each
/// envelope that changes it. What a browser replays of a page the store has seen already is
/// taken in again, so the Advisor follows the game through it, but isn't saved again. A game
/// watched rather than played is never saved.
#[derive(Debug)]
pub struct Following {
    knowledge: Library,
    source: WebtilesSource,
    advisor: Advisor,
    /// Where games are remembered between runs of the app, if anywhere.
    store: Option<Store>,
    /// What went wrong with the game history, the last time anything did.
    trouble: Option<Trouble>,
}

/// What went wrong with the game history.
#[derive(Debug)]
enum Trouble {
    /// What was saved couldn't be read, so it is left out of this run of the app.
    Reading(String),
    /// A game couldn't be saved, so it is remembered only until the app closes, unless a later
    /// save works.
    Saving(String),
}

/// Something that happened, rather than what the game now shows: a screen read or a session
/// event the Game Source reported, or what the Advisor's detectors noticed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum Happened {
    /// The Game Source reported a screen read or a session event.
    Source(SourceEvent),
    /// The Advisor's detectors noticed something at a Settled Update.
    Detected(Detection),
}

impl Following {
    /// Nothing followed yet, decoding and judging games through `knowledge`, and remembering
    /// none of them once dropped.
    #[must_use]
    pub fn new(knowledge: &Library) -> Self {
        Self {
            knowledge: knowledge.clone(),
            source: WebtilesSource::new(knowledge.clone()),
            advisor: Advisor::new(knowledge.clone()),
            store: None,
            trouble: None,
        }
    }

    /// Nothing followed yet in this run of the app, but every game in `store` remembered, and
    /// every game followed from now on saved there.
    #[must_use]
    pub fn remembering(knowledge: &Library, store: Store) -> Self {
        let mut following = Self {
            store: Some(store),
            ..Self::new(knowledge)
        };
        following.restart();
        following
    }

    /// Forgets everything relayed so far, as a browser connecting relays every tab again from
    /// its first message, keeping only what the store remembers.
    pub fn restart(&mut self) {
        let known = match self.store.as_ref().map(Store::known_games) {
            Some(Ok(games)) => KnownGames::new(games),
            Some(Err(error)) => {
                self.trouble = Some(Trouble::Reading(error.to_string()));
                KnownGames::default()
            }
            None => KnownGames::default(),
        };
        self.source = WebtilesSource::knowing(self.knowledge.clone(), known);
        self.advisor = Advisor::new(self.knowledge.clone());
    }

    /// Decodes one relayed envelope, tells the Advisor what the Game Source learned, saves what
    /// changed, and returns what happened, in order.
    pub fn ingest(&mut self, envelope: &Envelope) -> Vec<Happened> {
        let events = self.source.ingest(envelope);
        let happened = self.take_in(events);
        self.save(envelope.ts);
        happened
    }

    /// The player says the followed game is a new game rather than the one its game process
    /// picked up again. It is followed from now on as a new game, first seen as the process
    /// named it, remembering nothing yet; the game it was taken for keeps what it remembered.
    /// Returns what happened, which is nothing for a game the process joined, new already.
    pub fn new_game(&mut self) -> Vec<Happened> {
        let events = self.source.start_afresh();
        let happened = self.take_in(events);
        let last_seen = self
            .advisor
            .identity()
            .and_then(|identity| self.source.known_game(&identity.key()))
            .map(|game| game.last_seen_ms);
        if let Some(last_seen) = last_seen {
            self.save(last_seen);
        }
        happened
    }

    /// Tells the Advisor what the Game Source learned, and returns what happened, in order.
    fn take_in(&mut self, events: Vec<SourceEvent>) -> Vec<Happened> {
        let mut happened = Vec::new();
        for event in events {
            if let SourceEvent::Identified(identity) = &event {
                self.recall(identity);
            }
            let detections = self.advisor.apply(event.clone());
            if matches!(event, SourceEvent::ScreenRead(_) | SourceEvent::Session(_)) {
                happened.push(Happened::Source(event));
            }
            happened.extend(detections.into_iter().map(Happened::Detected));
        }
        happened
    }

    /// The Advisor, as the envelopes taken in so far left it.
    #[must_use]
    pub fn advisor(&self) -> &Advisor {
        &self.advisor
    }

    /// Why the game history couldn't be read or saved, the last time it couldn't, for the
    /// player.
    #[must_use]
    pub fn history_unsaved(&self) -> Option<&str> {
        match self.trouble.as_ref()? {
            Trouble::Reading(why) | Trouble::Saving(why) => Some(why),
        }
    }

    /// Hands the Advisor what the store remembers of the game `identity` names, the first time it
    /// is identified in this run of the app.
    fn recall(&mut self, identity: &GameIdentity) {
        let key = identity.key();
        let Some(store) = &mut self.store else { return };
        if self.advisor.knows(&key) || identity.role == Some(Role::Spectating) {
            return;
        }
        match store.saved(&key) {
            Ok(Some(saved)) => self.advisor.recall(key, saved.turn, saved.accumulated),
            Ok(None) => {}
            Err(error) => self.trouble = Some(Trouble::Reading(error.to_string())),
        }
    }

    /// Saves the followed game as an envelope received at `received_ms` left it, unless it is
    /// watched rather than played, or the envelope is from before the game was last seen: one
    /// the store has taken in already, replayed.
    fn save(&mut self, received_ms: u64) {
        let Some(store) = &mut self.store else { return };
        let Some(identity) = self.advisor.identity() else {
            return;
        };
        if identity.role == Some(Role::Spectating) {
            return;
        }
        let (Some(game), Some(accumulated)) = (
            self.source.known_game(&identity.key()),
            self.advisor.accumulated(),
        ) else {
            return;
        };
        if received_ms < game.last_seen_ms {
            return;
        }
        match store.save(game, accumulated) {
            Ok(()) => {
                if matches!(self.trouble, Some(Trouble::Saving(_))) {
                    self.trouble = None;
                }
            }
            Err(error) => self.trouble = Some(Trouble::Saving(error.to_string())),
        }
    }
}

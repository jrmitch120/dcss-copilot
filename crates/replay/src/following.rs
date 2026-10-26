//! A Game Source's games, followed by the Advisor and remembered in the store: the wiring the app
//! runs live and the replayer replays, so a replay sees a game as the app does.

use std::collections::HashMap;

use copilot_core::{
    Advisor, AssistanceLevel, Detection, GameIdentity, GameKey, GameView, KnownGames, PickedUp,
    Role, SourceEvent,
};
use copilot_gk::Library;
use copilot_store::{Store, StoreError};
use copilot_webtiles::{Envelope, WebtilesSource};
use serde::Serialize;

/// The webtiles Game Source and the Advisor it feeds, always built together from the same Game
/// Knowledge, and the store that remembers the games they follow between runs of the app.
///
/// A game is recalled from the store as the Game Source identifies it, and saved after each
/// envelope that changes it. What a browser replays of a page the store has seen already is
/// taken in again, so the Advisor follows the game through it, but isn't saved again. Only a
/// game the Game Source saw start as played is saved: never one watched. Nor is one picked up
/// again on a later turn, until the player says it is the same game.
#[derive(Debug)]
pub struct Following {
    knowledge: Library,
    source: WebtilesSource,
    advisor: Advisor,
    /// Where games are remembered between runs of the app, if anywhere.
    store: Option<Store>,
    /// What went wrong with the game history, the last time anything did.
    trouble: Option<Trouble>,
    /// Why what was saved of each game was unreadable, the last time it was read in this run of
    /// the app.
    unreadable: HashMap<GameKey, String>,
}

/// What went wrong with the game history.
#[derive(Debug)]
enum Trouble {
    /// Which games were saved couldn't be read, so they are left out of this run of the app.
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
            unreadable: HashMap::new(),
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
            Some(Ok(games)) => {
                if matches!(self.trouble, Some(Trouble::Reading(_))) {
                    self.trouble = None;
                }
                KnownGames::new(games)
            }
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

    /// The player answered whether the followed game is the one its game process picked up again
    /// on a later turn. The same game is saved from now on. A new game is followed from now on as
    /// one first seen as the process named it, remembering nothing yet, and the game it was
    /// taken for is left as it was last saved. Returns what happened, which is nothing unless the
    /// player was asked.
    pub fn answer_same_game(&mut self, same: bool) -> Vec<Happened> {
        if self.source.unconfirmed().is_none() {
            return Vec::new();
        }
        let happened = if same {
            self.source.confirm_same_game();
            Vec::new()
        } else {
            let taken_for = self.advisor.identity().map(GameIdentity::key);
            let events = self.source.start_afresh();
            let happened = self.take_in(events);
            // What the Advisor learned of it since it was picked up is of the new game.
            if let Some(key) = taken_for {
                self.advisor.forget(&key);
            }
            happened
        };
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

    /// The player put the join card away. It comes back after the app restarts if the game's
    /// Backfill still awaits the character dump, so nothing is saved.
    pub fn close_join_card(&mut self) {
        self.advisor.close_join_card();
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

    /// How many games the store remembers.
    ///
    /// # Errors
    /// If the store can't be read.
    pub fn games_remembered(&self) -> Result<usize, StoreError> {
        match &self.store {
            Some(store) => Ok(store.known_games()?.len()),
            None => Ok(0),
        }
    }

    /// Forgets every game last seen before `before_ms`, in Unix milliseconds, and says how many
    /// it forgot. Call it before a browser connects: what the Game Source already knows of a game
    /// is forgotten only at the next [`Following::restart`].
    ///
    /// # Errors
    /// If the store can't forget them.
    pub fn forget_games_seen_before(&mut self, before_ms: u64) -> Result<usize, StoreError> {
        match &mut self.store {
            Some(store) => store.forget_seen_before(before_ms),
            None => Ok(0),
        }
    }

    /// Forgets every game but the one followed, as the player clears their game history: what
    /// is remembered of each, and its Change Log. A game forgotten that is played again is
    /// picked up remembering nothing.
    ///
    /// # Errors
    /// If the store can't forget them. The Advisor forgets them all the same.
    pub fn forget_all_but_followed(&mut self) -> Result<(), StoreError> {
        self.advisor.forget_all_but_followed();
        let followed = self.advisor.identity().map(GameIdentity::key);
        match &mut self.store {
            Some(store) => store.forget_all_but(followed.as_ref()),
            None => Ok(()),
        }
    }

    /// The panel for the followed game, at the player's `assistance` level, asking the player
    /// whether a game picked up again on a later turn is the same game, and saying when what was
    /// saved of it couldn't be read.
    #[must_use]
    pub fn game_view(&self, assistance: AssistanceLevel) -> Option<GameView> {
        let mut view = self.advisor.game_view(assistance)?;
        view.picked_up = self.source.unconfirmed().map(|game| PickedUp {
            turn: game.turn,
            last_seen_ms: game.last_seen_ms,
        });
        view.accumulated_unreadable = self
            .advisor
            .identity()
            .and_then(|identity| self.unreadable.get(&identity.key()))
            .cloned();
        Some(view)
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
    /// is identified in this run of the app. What can't be read, the Advisor starts over without,
    /// and saves over: the Change Log keeps what was saved.
    fn recall(&mut self, identity: &GameIdentity) {
        let key = identity.key();
        let Some(store) = &mut self.store else { return };
        if self.advisor.knows(&key) || identity.role != Some(Role::Playing) {
            return;
        }
        match store.saved(&key) {
            Ok(saved) => {
                self.unreadable.remove(&key);
                if let Some(saved) = saved {
                    self.advisor.recall(key, saved.turn, saved.accumulated);
                }
            }
            Err(error) => {
                self.unreadable.insert(key, error.to_string());
            }
        }
    }

    /// Saves the followed game as an envelope received at `received_ms` left it, unless it is
    /// watched rather than played, the player is yet to say whether it is the game it was picked
    /// up as, or the envelope is from before the game was last seen: one the store has taken in
    /// already, replayed.
    fn save(&mut self, received_ms: u64) {
        let Some(store) = &mut self.store else { return };
        let Some(identity) = self.advisor.identity() else {
            return;
        };
        // A game not known to be played, such as one joined mid-page, may be one watched. One
        // picked up on a later turn may be another game, until the player says.
        if identity.role != Some(Role::Playing) || self.source.unconfirmed().is_some() {
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

//! Coach's Tactical refinement: at a Tactical Alert with a Suggested Action, the Quick Model picks
//! one of the Candidate Actions with a short reason, which the Suggested Action card shows beside
//! the rules' own pick. Only the latest question is in flight: an answer to any other, or one that
//! arrives after the rules picked anew, is dropped, and the Suggested Action stands alone. Turns
//! passing don't move the game on (#24: nothing goes stale from turns passing): a pick holds as
//! long as the Suggested Action it judged, marked with the turn it was asked on once that has
//! passed.

use serde::{Deserialize, Serialize};

use crate::candidate_action::Candidate;
use crate::fight_outcome::Fight;
use crate::gaps::Reader;
use crate::grounding::{EntityNames, kept_reason};
use crate::known_games::GameKey;
use crate::prompt::Prompt;
use crate::suggested_action::SuggestedAction;
use crate::view_model::{CoachLine, MarkedValue};

/// A question to the Coach, unique within a run of the app: the caller numbers them, so an answer
/// to a question asked before the Advisor was rebuilt is never taken for a later one's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct QuestionId(pub u64);

/// A Tactical question for the Quick Model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TacticalQuestion {
    /// Which question it is, which its answer is handed back with.
    pub id: QuestionId,
    /// What to ask.
    pub prompt: Prompt,
    /// The most words its reason may have, as the prompt says.
    pub words: u32,
}

/// What the Quick Model answered, in the shape the prompt asks for.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TacticalAnswer {
    /// The id of the Candidate Action it picked.
    pub pick: String,
    /// Why, in a sentence or two.
    pub reason: String,
}

/// What the Coach's lane did, as the timeline records it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "coach",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Coached {
    /// A question was asked, and is the one in flight.
    Asked {
        /// Which.
        question: QuestionId,
    },
    /// An answer was kept, and the card shows it until the rules pick anew.
    Picked {
        /// The question it answered.
        question: QuestionId,
        /// The id of the Candidate Action it picked.
        pick: String,
        /// That Candidate Action, in one line.
        line: String,
        /// It is the Suggested Action.
        agrees: bool,
        /// What was kept of its reason: the sentences that passed the grounding checks, within
        /// the word limit.
        reason: Option<String>,
    },
    /// An answer was dropped, and the Suggested Action stands alone.
    Dropped {
        /// The question it answered.
        question: QuestionId,
        /// Why.
        why: Dropped,
    },
}

/// Why an answer was dropped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "why", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Dropped {
    /// A newer question was asked while it was in flight.
    Superseded,
    /// The rules picked another Suggested Action, or none, or another game is followed, before it
    /// arrived.
    GameMovedOn,
    /// It picked an id the prompt never listed.
    UnknownPick {
        /// What it picked.
        pick: String,
    },
    /// The call failed, or its answer wasn't what the prompt asks for.
    Failed {
        /// Why.
        error: String,
    },
}

/// The Coach's one lane of Tactical questions.
#[derive(Debug, Clone, Default)]
pub(crate) struct Lane {
    /// The question in flight, the latest asked.
    in_flight: Option<Asked>,
    /// The last answer kept.
    picked: Option<Picked>,
}

/// A question in flight, and the moment it was asked at.
#[derive(Debug, Clone)]
struct Asked {
    id: QuestionId,
    moment: Moment,
    /// All the prompt says, which an answer may quote.
    said: String,
    words: u32,
    /// The Candidate Actions the prompt listed, in one line each.
    listed: Vec<Listed>,
}

/// A Candidate Action a prompt listed.
#[derive(Debug, Clone)]
struct Listed {
    id: String,
    line: MarkedValue,
    suggested: bool,
}

/// An answer kept, for the moment it was asked at.
#[derive(Debug, Clone)]
struct Picked {
    moment: Moment,
    line: CoachLine,
}

/// The moment a question was asked at: the followed game, its turn, and the Suggested Action the
/// Coach judges. The rules picking anew moves the game on from it; turns passing don't.
#[derive(Debug, Clone)]
pub(crate) struct Moment {
    pub(crate) game: GameKey,
    pub(crate) turn: Option<u32>,
    pub(crate) suggested: SuggestedAction,
}

impl Moment {
    /// Whether `now` is still this moment: the same game, and the same pick.
    fn holds_at(&self, now: &Self) -> bool {
        self.game == now.game && self.suggested.same_pick(&now.suggested)
    }

    /// The turn this moment was asked on, once `now` has passed it.
    fn passed_by(&self, now: &Self) -> Option<u32> {
        self.turn.filter(|_| self.turn != now.turn)
    }
}

impl Lane {
    /// Asks `prompt` as question `id` at `moment`, listing `candidates`, each as of `fight`: the
    /// question in flight from now on, in place of any other.
    pub(crate) fn ask(
        &mut self,
        id: QuestionId,
        moment: Moment,
        prompt: Prompt,
        words: u32,
        candidates: &[Candidate],
        fight: &Fight,
    ) -> TacticalQuestion {
        self.in_flight = Some(Asked {
            id,
            moment,
            said: format!("{}\n{}", prompt.prefix, prompt.moment),
            words,
            listed: candidates
                .iter()
                .map(|candidate| Listed {
                    id: candidate.id.clone(),
                    line: candidate.action.value(fight, Reader::Player),
                    suggested: candidate.suggested,
                })
                .collect(),
        });
        self.picked = None;
        TacticalQuestion { id, prompt, words }
    }

    /// Takes the answer to question `id`, given the game is now at `now`: kept only if it is the
    /// question in flight, the game hasn't moved on, and it picks a Candidate Action its prompt
    /// listed, with what of its reason says no name or number of `names` the prompt doesn't.
    pub(crate) fn answer(
        &mut self,
        id: QuestionId,
        answer: Result<TacticalAnswer, String>,
        now: Option<&Moment>,
        names: &EntityNames,
    ) -> Coached {
        let dropped = |why| Coached::Dropped { question: id, why };
        let Some(asked) = self.in_flight.take_if(|asked| asked.id == id) else {
            return dropped(Dropped::Superseded);
        };
        if !now.is_some_and(|now| asked.moment.holds_at(now)) {
            return dropped(Dropped::GameMovedOn);
        }
        let answer = match answer {
            Ok(answer) => answer,
            Err(error) => return dropped(Dropped::Failed { error }),
        };
        let pick = answer.pick.trim();
        let Some(listed) = asked.listed.iter().find(|listed| listed.id == pick) else {
            return dropped(Dropped::UnknownPick {
                pick: answer.pick.clone(),
            });
        };
        let reason = kept_reason(&answer.reason, &asked.said, names, asked.words);
        self.picked = Some(Picked {
            moment: asked.moment.clone(),
            line: CoachLine::Picked {
                action: listed.line.clone(),
                agrees: listed.suggested,
                reason: reason.clone(),
                as_of_turn: None,
            },
        });
        Coached::Picked {
            question: id,
            pick: listed.id.clone(),
            line: listed.line.text.clone(),
            agrees: listed.suggested,
            reason,
        }
    }

    /// What the Suggested Action card says of the Coach at `now`: its pick for the Suggested
    /// Action it judged, as of the turn it was asked on, or that it is being asked.
    pub(crate) fn line(&self, now: &Moment) -> Option<CoachLine> {
        if let Some(picked) = self
            .picked
            .as_ref()
            .filter(|picked| picked.moment.holds_at(now))
        {
            let mut line = picked.line.clone();
            if let CoachLine::Picked { as_of_turn, .. } = &mut line {
                *as_of_turn = picked.moment.passed_by(now);
            }
            return Some(line);
        }
        self.in_flight
            .as_ref()
            .filter(|asked| asked.moment.holds_at(now))
            .map(|_| CoachLine::Asking)
    }
}

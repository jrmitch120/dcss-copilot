//! Gaps: the facts the Advisor needs about a game but doesn't have, or has only an out-of-date
//! copy of, and how the panel words them.

use std::collections::BTreeSet;

use copilot_gk::Bundle;
use serde::{Deserialize, Serialize};

use crate::accumulated_state::{AccumulatedState, Remembered};
use crate::fight_outcome;
use crate::view_model::{GapList, GapRow, capitalised};
use crate::visible_state::{Religion, VisibleState};

/// A fact the Advisor needs about the game but doesn't have, or has only an out-of-date copy of.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(tag = "gap", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Gap {
    /// The player's hit points haven't been reported.
    HitPoints,
    /// The player's AC, EV or SH haven't been reported.
    Defences,
    /// The game hasn't sent its map, so the monsters in view aren't known.
    MonstersInView,
    /// The game hasn't sent its map, so the level's paths aren't known.
    Map,
    /// Where the player stands hasn't been reported.
    Position,
    /// There is no Game Knowledge for the game's Crawl Version, so monsters can't be judged.
    GameKnowledge,
    /// A monster in view isn't in the Game Knowledge for the game's Crawl Version.
    MonsterKnowledge {
        /// The monster, as the game names it.
        monster: String,
    },
    /// The game hasn't sent the inventory.
    Inventory,
    /// The game hasn't said what the player holds.
    Wielded,
    /// The player's Strength, Dexterity or magic points haven't been reported.
    Attributes,
    /// The player hasn't opened the screen that shows `fact`, so what depends on it isn't
    /// judged, or is judged as [`ScreenFact`] says for each.
    Unread {
        /// What isn't read.
        fact: ScreenFact,
    },
    /// `fact` was read on `turn`, and something seen since may have changed it: it is used all
    /// the same.
    Stale {
        /// What is out of date.
        fact: ScreenFact,
        /// The game's turn when it was read, if it had reported one.
        turn: Option<u32>,
    },
    /// The player memorised a spell the game's message didn't name, a Blind Spot until the
    /// spell list (`I`) is read.
    SpellMemorised {
        /// The game's turn when they did, if it had reported one.
        turn: Option<u32>,
    },
    /// The player follows a god who shifts their skills from their base levels with piety,
    /// without a message, a Blind Spot: they may differ from what the skills screen showed.
    SkillsShifting {
        /// The god, as the game names them, such as `Ashenzari`.
        god: String,
    },
}

/// A Screen Fact: a fact about the character the Advisor reads off a screen the player opens,
/// such as their skills, so a Gap in it is filled by opening that screen. The Gap prompt asks for
/// Screen Facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScreenFact {
    /// The skills, off the skills screen (`m`). Unread, no fight can be judged.
    Skills,
    /// The spells known and their failure rates, off the spell list (`I`). Unread, the spells
    /// aren't counted among the Ways Out or the Attack Plans.
    SpellList,
    /// The spells' power and range, off the spell list switched to them (`I`, then `!`). Unread,
    /// the attack spells aren't played in a fight.
    SpellPower,
    /// The abilities and their failure rates, off the ability menu (`a`). Unread, the abilities
    /// aren't counted among the Ways Out.
    AbilityMenu,
    /// The resistances, off the character overview (`%`). Unread, monsters' damage they would
    /// change is played as if the player resists nothing.
    Resistances,
}

impl ScreenFact {
    /// The keys that open the screen showing it, such as `I` then `!`.
    fn keys(self) -> &'static [&'static str] {
        match self {
            Self::Skills => &["m"],
            Self::SpellList => &["I"],
            Self::SpellPower => &["I", "!"],
            Self::AbilityMenu => &["a"],
            Self::Resistances => &["%"],
        }
    }

    /// Its name, as a line of the panel starts, such as `Spell list`.
    fn name(self) -> &'static str {
        match self {
            Self::Skills => "Skills",
            Self::SpellList => "Spell list",
            Self::SpellPower => "Spell power",
            Self::AbilityMenu => "Ability menu",
            Self::Resistances => "Resistances",
        }
    }

    /// The word a sentence goes on to name it by.
    fn pronoun(self) -> &'static str {
        match self {
            Self::Skills | Self::Resistances => "them",
            Self::SpellList | Self::SpellPower | Self::AbilityMenu => "it",
        }
    }

    /// The screen showing it, as a sentence names it, such as `the spell list`, and whether its
    /// name names that screen already.
    fn screen(self) -> (&'static str, bool) {
        match self {
            Self::Skills => ("the skills screen", false),
            Self::SpellList => ("the spell list", true),
            Self::SpellPower => ("the spell list", false),
            Self::AbilityMenu => ("the ability menu", true),
            Self::Resistances => ("the character overview", false),
        }
    }

    /// How to open the screen showing it, as a sentence goes on, such as `open the spell list (I)
    /// and press !`, or, when the sentence has named the screen already, `open it (I)`.
    pub(crate) fn how_to_open(self, named: bool) -> String {
        let screen = if named { "it" } else { self.screen().0 };
        match self.keys() {
            [key, then @ ..] => {
                let then: String = then.iter().map(|key| format!(" and press {key}")).collect();
                format!("open {screen} ({key}){then}")
            }
            [] => format!("open {screen}"),
        }
    }

    /// The Gap in what is `remembered` of it: unread, out of date, or none while it is Known.
    pub(crate) fn gap<T, S>(self, remembered: &Remembered<T, S>) -> Option<Gap> {
        match remembered {
            Remembered::Missing => Some(Gap::Unread { fact: self }),
            Remembered::Stale(read) => Some(Gap::Stale {
                fact: self,
                turn: read.turn,
            }),
            Remembered::Known(_) => None,
        }
    }

    /// It, as a sentence names it, when the character dump (`?` then `#`) shows it too.
    fn in_dump(self) -> Option<&'static str> {
        match self {
            Self::Skills => Some("skills"),
            Self::SpellList => Some("the spell list"),
            Self::Resistances => Some("resistances"),
            Self::SpellPower | Self::AbilityMenu => None,
        }
    }

    /// Why it matters while unread, in one sentence.
    fn why_unread(self) -> &'static str {
        match self {
            Self::Skills => "No fight can be judged without them.",
            Self::SpellList => "Your spells aren't counted among your Ways Out or attacks.",
            Self::SpellPower => "Your attack spells aren't played in a fight.",
            Self::AbilityMenu => "Your abilities aren't counted among your Ways Out.",
            Self::Resistances => {
                "Monsters' fire, cold, poison and the like are played as if you resist none."
            }
        }
    }

    /// Why it matters while out of date, in one sentence.
    fn why_stale(self) -> &'static str {
        match self {
            Self::Skills => "Fights are played with them as they were.",
            Self::SpellList => {
                "A spell learned since isn't counted, and failure rates may have changed."
            }
            Self::SpellPower => "Fights play it as it was.",
            Self::AbilityMenu => {
                "An ability gained since isn't counted, and failure rates may have changed."
            }
            Self::Resistances => "Monsters' damage is played with them as they were.",
        }
    }

    /// What reading it lets the Advisor do, as a sentence that says to open its screen ends,
    /// such as `to judge your spells.`
    fn to_judge(self) -> &'static str {
        match self {
            Self::Skills => "to judge your attacks.",
            Self::SpellList => "to judge your spells.",
            Self::SpellPower => "to judge your spells' power.",
            Self::AbilityMenu => "to judge your abilities.",
            Self::Resistances => {
                "to read your resistances: until then you're taken to resist nothing."
            }
        }
    }
}

impl Gap {
    /// The Screen Fact it is about, if a screen the player can open fills it.
    pub(crate) fn screen_fact(&self) -> Option<ScreenFact> {
        match self {
            Self::Unread { fact } | Self::Stale { fact, .. } => Some(*fact),
            Self::SpellMemorised { .. } => Some(ScreenFact::SpellList),
            Self::HitPoints
            | Self::Defences
            | Self::MonstersInView
            | Self::Map
            | Self::Position
            | Self::GameKnowledge
            | Self::MonsterKnowledge { .. }
            | Self::Inventory
            | Self::Wielded
            | Self::Attributes
            | Self::SkillsShifting { .. } => None,
        }
    }

    /// What is missing or out of date, as the panel names it, such as `~Skills as of turn 12`.
    fn what(&self) -> String {
        match self {
            Self::HitPoints | Self::Defences | Self::Attributes => "Stats".to_owned(),
            Self::MonstersInView | Self::Map => "Map".to_owned(),
            Self::Position => "Where you stand".to_owned(),
            Self::GameKnowledge => "Game Knowledge".to_owned(),
            Self::MonsterKnowledge { monster } => capitalised(monster),
            Self::Inventory | Self::Wielded => "Inventory".to_owned(),
            Self::Unread { fact } => fact.name().to_owned(),
            Self::Stale { fact, turn } => out_of_date(fact.name(), *turn),
            Self::SkillsShifting { .. } => ScreenFact::Skills.name().to_owned(),
            Self::SpellMemorised { turn: Some(turn) } => {
                format!("A spell memorised on turn {turn}")
            }
            Self::SpellMemorised { turn: None } => "A spell memorised".to_owned(),
        }
    }

    /// Why it matters, in one sentence.
    fn why(&self) -> String {
        match self {
            Self::HitPoints
            | Self::Defences
            | Self::Attributes
            | Self::Inventory
            | Self::Wielded => "The game hasn't sent them yet.".to_owned(),
            Self::MonstersInView | Self::Map => "The game hasn't sent it yet.".to_owned(),
            Self::Position => "The game hasn't said yet.".to_owned(),
            Self::GameKnowledge => {
                "There is none for this version, so nothing can be judged.".to_owned()
            }
            Self::MonsterKnowledge { .. } => {
                "It isn't in the Game Knowledge for this version, so it isn't counted.".to_owned()
            }
            Self::Unread { fact } => fact.why_unread().to_owned(),
            Self::Stale { fact, .. } => fact.why_stale().to_owned(),
            Self::SpellMemorised { .. } => {
                "The game didn't name it, so it isn't counted.".to_owned()
            }
            Self::SkillsShifting { god } => {
                format!(
                    "{god} shifts them with your piety, so they may differ from the skills screen's."
                )
            }
        }
    }

    /// Its row in the panel's list, with the keys that fill it unless the player is watching a
    /// game not theirs, whose screens they can't open.
    fn row(&self, watching: bool) -> GapRow {
        let keys = match self.screen_fact() {
            Some(fact) if !watching => fact.keys().iter().map(|&key| key.to_owned()).collect(),
            _ => Vec::new(),
        };
        GapRow {
            what: self.what(),
            why: self.why(),
            keys,
        }
    }

    /// What it keeps the Advisor from judging, or how out of date it is, and how to fill it where
    /// the player can, in one sentence.
    pub(crate) fn sentence(&self) -> String {
        match self {
            Self::HitPoints | Self::Defences | Self::Attributes => {
                "The game hasn't sent your stats yet.".to_owned()
            }
            Self::MonstersInView | Self::Map => "The game hasn't sent its map yet.".to_owned(),
            Self::Position => "The game hasn't said where you stand yet.".to_owned(),
            Self::GameKnowledge => {
                "Nothing can be judged without Game Knowledge for this version.".to_owned()
            }
            Self::MonsterKnowledge { monster } => format!(
                "{} isn't in the Game Knowledge, so it isn't counted.",
                capitalised(monster)
            ),
            Self::Inventory | Self::Wielded => {
                "The game hasn't sent your inventory yet.".to_owned()
            }
            Self::Unread { fact } => open_to(*fact, fact.to_judge()),
            Self::Stale { fact, turn } => format!(
                "{}: {} to bring {} up to date.",
                out_of_date(fact.name(), *turn),
                fact.how_to_open(fact.screen().1),
                fact.pronoun()
            ),
            Self::SpellMemorised { turn } => format!(
                "{}: {} to judge it.",
                memorised_unnamed(*turn),
                ScreenFact::SpellList.how_to_open(false)
            ),
            Self::SkillsShifting { god } => format!(
                "Your skills may differ from the skills screen's: {god} shifts them with your piety."
            ),
        }
    }
}

/// A sentence saying to open the screen showing `fact`, `to` do what, such as `Open the skills
/// screen (m) to judge a fight.`
pub(crate) fn open_to(fact: ScreenFact, to: &str) -> String {
    format!("{} {to}", capitalised(&fact.how_to_open(false)))
}

/// The Gaps in what the Advisor remembers of the game, whatever is in view: each fact the engine
/// reads off a screen that is unread or out of date, and each Blind Spot, as `visible` shows the
/// game, judged through `knowledge` for its Crawl Version where it needs it.
pub(crate) fn of_the_game(
    visible: &VisibleState,
    remembered: &AccumulatedState,
    knowledge: Option<&Bundle>,
) -> BTreeSet<Gap> {
    let mut gaps = BTreeSet::new();
    gaps.extend(ScreenFact::Skills.gap(&remembered.skills));
    gaps.extend(ScreenFact::AbilityMenu.gap(&remembered.abilities));
    gaps.extend(ScreenFact::Resistances.gap(&remembered.resistances));
    if let Some(bundle) = knowledge {
        let mp_max = visible.stats.mp_max.unwrap_or(0);
        gaps.extend(fight_outcome::spell_gaps(Some(remembered), mp_max, bundle));
        if let Some(Religion::God { name, .. }) = &visible.religion
            && bundle.character.skills.shifting_gods.contains(name)
        {
            gaps.insert(Gap::SkillsShifting { god: name.clone() });
        }
    }
    gaps
}

/// The panel's list of `gaps`, without keys to press for a player `watching` a game not
/// theirs.
pub(crate) fn list(gaps: &BTreeSet<Gap>, watching: bool) -> GapList {
    let mut rows: Vec<GapRow> = Vec::new();
    for gap in gaps {
        let row = gap.row(watching);
        if !rows.contains(&row) {
            rows.push(row);
        }
    }
    let in_dump: BTreeSet<ScreenFact> = gaps
        .iter()
        .filter_map(Gap::screen_fact)
        .filter(|screen_fact| screen_fact.in_dump().is_some())
        .collect();
    GapList {
        gaps: rows,
        dump_fills: in_dump
            .into_iter()
            .filter_map(ScreenFact::in_dump)
            .filter(|_| !watching)
            .map(str::to_owned)
            .collect(),
    }
}

/// What was read of `what` on `turn` and may have changed since, such as `~Skills as of turn
/// 12`: `~` before it, as for any value out of date.
fn out_of_date(what: &str, turn: Option<u32>) -> String {
    match turn {
        Some(turn) => format!("~{what} as of turn {turn}"),
        None => format!("~{what} as last read"),
    }
}

/// When the player memorised a spell the game didn't name, as a sentence starts, such as `You
/// memorised a spell on turn 14 that the game didn't name`.
pub(crate) fn memorised_unnamed(turn: Option<u32>) -> String {
    match turn {
        Some(turn) => format!("You memorised a spell on turn {turn} that the game didn't name"),
        None => "You memorised a spell that the game didn't name".to_owned(),
    }
}

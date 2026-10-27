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
    /// The player hasn't opened the ability menu (`a`), so which of their abilities are Ways Out
    /// isn't known.
    AbilityMenu,
    /// The player hasn't opened the spell list (`I`), so which of their spells are Ways Out, and
    /// how often they fail, isn't known.
    SpellList,
    /// The player hasn't opened the skills screen (`m`), so their attacks can't be played out.
    Skills,
    /// The player hasn't switched the spell list to its power and range (`I`, then `!`), so their
    /// attack spells can't be played out.
    SpellPower,
    /// The player hasn't opened the character overview (`%`), so monsters' damage that their
    /// resistances would change is played as if they resist nothing.
    Resistances,
    /// The game hasn't sent the inventory.
    Inventory,
    /// The game hasn't said what the player holds.
    Wielded,
    /// The player's Strength, Dexterity or magic points haven't been reported.
    Attributes,
    /// The skills were read on `turn`, and something seen since may have changed them: they
    /// are played all the same.
    StaleSkills {
        /// The game's turn when they were read, if it had reported one.
        turn: Option<u32>,
    },
    /// The spell list was read on `turn`, and something seen since may have changed which
    /// spells the player knows or how often they fail: it is played all the same.
    StaleSpellList {
        /// The game's turn when it was read, if it had reported one.
        turn: Option<u32>,
    },
    /// The spells' power and range were read on `turn`, and something seen since may have
    /// changed them: they are played all the same.
    StaleSpellPower {
        /// The game's turn when they were read, if it had reported one.
        turn: Option<u32>,
    },
    /// The ability menu was read on `turn`, and something seen since may have changed which
    /// abilities the player has, what they cost or how often they fail: it is judged all the
    /// same.
    StaleAbilityMenu {
        /// The game's turn when it was read, if it had reported one.
        turn: Option<u32>,
    },
    /// The resistances were read on `turn`, and something seen since may have changed them:
    /// they are applied all the same.
    StaleResistances {
        /// The game's turn when they were read, if it had reported one.
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
    /// The skills, off the skills screen (`m`).
    Skills,
    /// The spells known and their failure rates, off the spell list (`I`).
    SpellList,
    /// The spells' power and range, off the spell list switched to them (`I`, then `!`).
    SpellPower,
    /// The abilities and their failure rates, off the ability menu (`a`).
    AbilityMenu,
    /// The resistances, off the character overview (`%`).
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

    /// It, as a sentence names it, when the character dump (`?` then `#`) shows it too.
    fn in_dump(self) -> Option<&'static str> {
        match self {
            Self::Skills => Some("skills"),
            Self::SpellList => Some("the spell list"),
            Self::Resistances => Some("resistances"),
            Self::SpellPower | Self::AbilityMenu => None,
        }
    }
}

impl Gap {
    /// The Screen Fact it is about, if a screen the player can open fills it.
    pub(crate) fn screen_fact(&self) -> Option<ScreenFact> {
        match self {
            Self::Skills | Self::StaleSkills { .. } => Some(ScreenFact::Skills),
            Self::SpellList | Self::StaleSpellList { .. } | Self::SpellMemorised { .. } => {
                Some(ScreenFact::SpellList)
            }
            Self::SpellPower | Self::StaleSpellPower { .. } => Some(ScreenFact::SpellPower),
            Self::AbilityMenu | Self::StaleAbilityMenu { .. } => Some(ScreenFact::AbilityMenu),
            Self::Resistances | Self::StaleResistances { .. } => Some(ScreenFact::Resistances),
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

    /// What is missing or out of date, as the Gaps chip names it, such as `~Skills as of turn
    /// 12`.
    fn what(&self) -> String {
        match self {
            Self::HitPoints | Self::Defences | Self::Attributes => "Stats".to_owned(),
            Self::MonstersInView | Self::Map => "Map".to_owned(),
            Self::Position => "Where you stand".to_owned(),
            Self::GameKnowledge => "Game Knowledge".to_owned(),
            Self::MonsterKnowledge { monster } => capitalised(monster),
            Self::AbilityMenu => "Ability menu".to_owned(),
            Self::SpellList => "Spell list".to_owned(),
            Self::Skills | Self::SkillsShifting { .. } => "Skills".to_owned(),
            Self::SpellPower => "Spell power".to_owned(),
            Self::Resistances => "Resistances".to_owned(),
            Self::Inventory | Self::Wielded => "Inventory".to_owned(),
            Self::StaleSkills { turn } => out_of_date("Skills", *turn),
            Self::StaleSpellList { turn } => out_of_date("Spell list", *turn),
            Self::StaleSpellPower { turn } => out_of_date("Spell power", *turn),
            Self::StaleAbilityMenu { turn } => out_of_date("Ability menu", *turn),
            Self::StaleResistances { turn } => out_of_date("Resistances", *turn),
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
            Self::AbilityMenu => "Your abilities aren't counted among your Ways Out.".to_owned(),
            Self::SpellList => {
                "Your spells aren't counted among your Ways Out or attacks.".to_owned()
            }
            Self::Skills => "No fight can be judged without them.".to_owned(),
            Self::SpellPower => "Your attack spells aren't played in a fight.".to_owned(),
            Self::Resistances => {
                "Monsters' fire, cold, poison and the like are played as if you resist none."
                    .to_owned()
            }
            Self::StaleSkills { .. } => "Fights are played with them as they were.".to_owned(),
            Self::StaleSpellList { .. } => {
                "A spell learned since isn't counted, and failure rates may have changed."
                    .to_owned()
            }
            Self::StaleSpellPower { .. } => "Fights play it as it was.".to_owned(),
            Self::StaleAbilityMenu { .. } => {
                "An ability gained since isn't counted, and failure rates may have changed."
                    .to_owned()
            }
            Self::StaleResistances { .. } => {
                "Monsters' damage is played with them as they were.".to_owned()
            }
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

    /// Its row in the Gaps chip, with the keys that fill it unless the player is watching a game
    /// not theirs, whose screens they can't open.
    fn row(&self, watching: bool) -> GapRow {
        let keys = match self.screen_fact() {
            Some(screen_fact) if !watching => screen_fact
                .keys()
                .iter()
                .map(|&key| key.to_owned())
                .collect(),
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
            Self::AbilityMenu => "Open the ability menu (a) to judge your abilities.".to_owned(),
            Self::SpellList => "Open the spell list (I) to judge your spells.".to_owned(),
            Self::Skills => "Open the skills screen (m) to judge your attacks.".to_owned(),
            Self::SpellPower => {
                "Open the spell list (I) and press ! to judge your spells' power.".to_owned()
            }
            Self::Resistances => "Open the character overview (%) to read your resistances: \
                until then you're taken to resist nothing."
                .to_owned(),
            Self::Inventory | Self::Wielded => {
                "The game hasn't sent your inventory yet.".to_owned()
            }
            Self::StaleSkills { turn } => format!(
                "{}: open the skills screen (m) to bring them up to date.",
                out_of_date("Skills", *turn)
            ),
            Self::StaleSpellList { turn } => format!(
                "{}: open it (I) to bring it up to date.",
                out_of_date("Spell list", *turn)
            ),
            Self::StaleSpellPower { turn } => format!(
                "{}: open the spell list (I) and press ! to bring it up to date.",
                out_of_date("Spell power", *turn)
            ),
            Self::StaleAbilityMenu { turn } => format!(
                "{}: open it (a) to bring it up to date.",
                out_of_date("Ability menu", *turn)
            ),
            Self::StaleResistances { turn } => format!(
                "{}: open the character overview (%) to bring them up to date.",
                out_of_date("Resistances", *turn)
            ),
            Self::SpellMemorised { turn } => format!(
                "{}: open the spell list (I) to judge it.",
                memorised_unnamed(*turn)
            ),
            Self::SkillsShifting { god } => format!(
                "Your skills may differ from the skills screen's: {god} shifts them with your piety."
            ),
        }
    }
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
    gaps.extend(match &remembered.skills {
        Remembered::Missing => Some(Gap::Skills),
        Remembered::Stale(read) => Some(Gap::StaleSkills { turn: read.turn }),
        Remembered::Known(_) => None,
    });
    gaps.extend(match &remembered.abilities {
        Remembered::Missing => Some(Gap::AbilityMenu),
        Remembered::Stale(read) => Some(Gap::StaleAbilityMenu { turn: read.turn }),
        Remembered::Known(_) => None,
    });
    gaps.extend(match &remembered.resistances {
        Remembered::Missing => Some(Gap::Resistances),
        Remembered::Stale(read) => Some(Gap::StaleResistances { turn: read.turn }),
        Remembered::Known(_) => None,
    });
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

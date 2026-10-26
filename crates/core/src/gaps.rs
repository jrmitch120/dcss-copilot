//! Gaps: the facts the Advisor needs about a game but doesn't have, or has only an out-of-date
//! copy of, and how the panel words them.

use serde::Serialize;

use crate::view_model::capitalised;

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

impl Gap {
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

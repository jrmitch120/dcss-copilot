//! Accumulated State: what the Advisor remembers of one game that the game no longer shows, each
//! fact with where it came from and how recent it is.

use copilot_gk::ItemKind;
use serde::Serialize;

use crate::screen::{Resistances, Screen, ScreenRead};
use crate::visible_state::VisibleState;

/// What the Advisor remembers of one game.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccumulatedState {
    /// The character's resistances, as the character overview or dump last showed them. Nothing
    /// the Advisor follows yet tells it they changed, so once read they stay Known: they are
    /// labelled with the turn they were read on, never presented as current.
    pub resistances: Remembered<Resistances>,
    /// The shield the character wears, if any, as the character overview or dump last showed it
    /// with the inventory decoded. It goes Stale when the SH the game reports changes.
    pub worn_shield: Remembered<Option<Shield>>,
}

/// A fact the Advisor remembers, and how current it is: an engine input is Known, Stale or
/// Missing.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Remembered<T> {
    /// The Advisor has never read it.
    #[default]
    Missing,
    /// Read, and nothing the Advisor has seen since changes it.
    Known(AsOf<T>),
    /// Read, but something the Advisor has seen since may have changed it, so it is only an
    /// out-of-date copy.
    Stale(AsOf<T>),
}

/// A remembered value, with where and when it was read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AsOf<T> {
    /// What was read.
    pub value: T,
    /// The game's turn when it was read, if the game had reported one.
    pub turn: Option<u32>,
    /// The screen it was read off.
    pub screen: Screen,
}

/// A shield the character can wear, by how many attacks it can block before their next action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Shield {
    /// `ARM_BUCKLER`.
    Buckler,
    /// `ARM_KITE_SHIELD`.
    KiteShield,
    /// `ARM_TOWER_SHIELD`.
    TowerShield,
}

impl Shield {
    /// The shield an item is, if it is one: crawl's `is_shield` (`item-prop.cc`), which leaves
    /// out orbs, the other armour worn in the offhand.
    fn of(kind: &ItemKind) -> Option<Self> {
        if kind.class.0 != "OBJ_ARMOUR" {
            return None;
        }
        match kind.sub_type.as_ref()?.0.as_str() {
            "ARM_BUCKLER" => Some(Self::Buckler),
            "ARM_KITE_SHIELD" => Some(Self::KiteShield),
            "ARM_TOWER_SHIELD" => Some(Self::TowerShield),
            _ => None,
        }
    }
}

impl<T> Remembered<T> {
    /// The value, when it is Known.
    #[must_use]
    pub fn known(&self) -> Option<&T> {
        match self {
            Self::Known(fact) => Some(&fact.value),
            Self::Missing | Self::Stale(_) => None,
        }
    }

    /// `value`, Known as of now: read off `read` on `visible`'s turn.
    fn read(value: T, read: &ScreenRead, visible: &VisibleState) -> Self {
        Self::Known(AsOf {
            value,
            turn: visible.turn,
            screen: read.screen.clone(),
        })
    }

    /// Something seen since may have changed it.
    fn go_stale(&mut self) {
        if let Self::Known(fact) = std::mem::take(self) {
            *self = Self::Stale(fact);
        }
    }
}

impl AccumulatedState {
    /// Remembers what a screen the player opened shows, read on `visible`'s turn.
    pub(crate) fn read(&mut self, read: &ScreenRead, visible: &VisibleState) {
        if let Some(resistances) = read.resistances() {
            self.resistances = Remembered::read(resistances, read, visible);
        }
        if let Some(shield) = read
            .equipped()
            .and_then(|letters| worn_shield(&letters, visible))
        {
            self.worn_shield = Remembered::read(shield, read, visible);
        }
    }

    /// Notes what changed between the Visible State the game reported before and what it
    /// reports now.
    pub(crate) fn update(&mut self, before: &VisibleState, now: &VisibleState) {
        if before.stats.sh != now.stats.sh {
            self.worn_shield.go_stale();
        }
    }
}

/// The shield among the items at the `equipped` inventory letters, or `None` for none, when the
/// inventory tells what each of them is.
fn worn_shield(equipped: &[char], visible: &VisibleState) -> Option<Option<Shield>> {
    let inventory = visible.inventory.as_ref()?;
    let mut worn = None;
    for &letter in equipped {
        let item = inventory.iter().find(|item| item.letter == letter)?;
        let kind = item.kind.as_ref()?;
        worn = worn.or(Shield::of(kind));
    }
    Some(worn)
}

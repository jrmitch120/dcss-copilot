//! Accumulated State: what the Advisor remembers of one game that the game no longer shows, each
//! fact with where it came from and how recent it is.

use copilot_gk::{EnumId, ItemKind, WayOutSigns};
use serde::Serialize;

use crate::screen::{KnownAbility, KnownSpell, Resistances, Screen, ScreenRead};
use crate::visible_state::{GameMessage, VisibleState};

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
    /// The inventory letters of what the character has equipped and not melded, as the
    /// character overview or dump last showed them. It goes Stale when the inventory, AC, EV or
    /// SH changes, or the light on the character's square, as putting on or taking off an item
    /// with a halo does. A change the game shows no sign of, such as wielding one weapon for
    /// another inside someone else's halo, goes unnoticed.
    pub equipped: Remembered<Vec<char>>,
    /// The spells the character knows, with their failure rates, as the spell list (`I`) last
    /// showed them. It goes Stale when the experience level, Intelligence, inventory, AC, EV or
    /// SH changes, as failure rates change with them. Skills trained between levels, which
    /// lower failure rates bit by bit, go unnoticed, and so does a spell learned or forgotten
    /// without any of those changing.
    pub spells: Remembered<Vec<KnownSpell>>,
    /// The abilities the character has, with their costs and failure rates, as the ability
    /// menu (`a`) last showed them. It goes Stale when the experience level, god, piety,
    /// maximum magic points or inventory changes, as abilities come and go with them.
    pub abilities: Remembered<Vec<KnownAbility>>,
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
        Self::of_armour(kind.sub_type.as_ref()?)
    }

    /// The shield a kind of armour is, by its `armour_type` identifier, if it is one.
    pub(crate) fn of_armour(sub_type: &EnumId) -> Option<Self> {
        match sub_type.0.as_str() {
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

    /// Something seen since may have changed it. A Stale fact stays Stale, and a Missing one
    /// Missing.
    fn go_stale(&mut self) {
        *self = match std::mem::take(self) {
            Self::Known(fact) | Self::Stale(fact) => Self::Stale(fact),
            Self::Missing => Self::Missing,
        };
    }
}

impl AccumulatedState {
    /// Remembers what a screen the player opened shows, read on `visible`'s turn.
    pub(crate) fn read(&mut self, read: &ScreenRead, visible: &VisibleState) {
        if let Some(resistances) = read.resistances() {
            self.resistances = Remembered::read(resistances, read, visible);
        }
        if let Some(letters) = read.equipped() {
            if let Some(shield) = worn_shield(&letters, visible) {
                self.worn_shield = Remembered::read(shield, read, visible);
            }
            self.equipped = Remembered::read(letters, read, visible);
        }
        if let Some(spells) = read.spells() {
            self.spells = Remembered::read(spells, read, visible);
        }
        if let Some(abilities) = read.abilities() {
            self.abilities = Remembered::read(abilities, read, visible);
        }
    }

    /// Notes what changed between the Visible State the game reported before and what it
    /// reports now, reading through `signs` the messages crawl gives instead of the spell list
    /// or the ability menu when there is nothing to list.
    pub(crate) fn update(
        &mut self,
        before: &VisibleState,
        now: &VisibleState,
        signs: Option<&WayOutSigns>,
    ) {
        if before.stats.sh != now.stats.sh {
            self.worn_shield.go_stale();
        }
        let (was, is) = (&before.stats, &now.stats);
        if before.inventory != now.inventory
            || (was.ac, was.ev, was.sh) != (is.ac, is.ev, is.sh)
            || before.light_at_player() != now.light_at_player()
        {
            self.equipped.go_stale();
        }
        if before.inventory != now.inventory
            || (was.xl, was.intelligence) != (is.xl, is.intelligence)
            || (was.ac, was.ev, was.sh) != (is.ac, is.ev, is.sh)
        {
            self.spells.go_stale();
        }
        if before.inventory != now.inventory
            || (was.xl, was.mp_max) != (is.xl, is.mp_max)
            || before.religion != now.religion
        {
            self.abilities.go_stale();
        }
        let Some(signs) = signs else { return };
        for message in new_messages(before, now) {
            let turn = message.turn.or(now.turn);
            if message.text == signs.no_spells {
                self.spells = Remembered::nothing_on(Screen::Spells, turn);
            }
            if message.text == signs.no_abilities {
                self.abilities = Remembered::nothing_on(Screen::Abilities, turn);
            }
        }
    }
}

impl<T> Remembered<Vec<T>> {
    /// Known as of `turn` to list nothing, as crawl says in a message instead of opening
    /// `screen`.
    fn nothing_on(screen: Screen, turn: Option<u32>) -> Self {
        Self::Known(AsOf {
            value: Vec::new(),
            turn,
            screen,
        })
    }
}

/// The messages `now` shows that `before` didn't: those after the last one `before` showed, or
/// all of them when it is gone.
fn new_messages<'a>(before: &VisibleState, now: &'a VisibleState) -> &'a [GameMessage] {
    let after = before
        .messages
        .last()
        .and_then(|last| now.messages.iter().rposition(|message| message == last))
        .map_or(0, |at| at + 1);
    now.messages.get(after..).unwrap_or_default()
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

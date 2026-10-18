//! Accumulated State: what the Advisor remembers of one game that the game no longer shows, each
//! fact with where it came from and how recent it is.

use std::collections::BTreeMap;

use copilot_gk::{Bundle, EnumId, ItemKind};
use serde::Serialize;

use crate::carried_gear::{self, CarriedGear, LoneSighting, Said};
use crate::screen::{KnownAbility, KnownSpell, Resistances, Screen, ScreenRead};
use crate::visible_state::{GameMessage, InventoryItem, Sighting, VisibleMonster, VisibleState};

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
    ///
    /// For each of these, the inventory changes when what is carried does: crawl greying an
    /// item out or not, which follows the character's statuses, isn't a change.
    pub abilities: Remembered<Vec<KnownAbility>>,
    /// The Carried Gear of each monster in view the Advisor has read it for, but uniques', by
    /// its Sighting: forgotten once it leaves view. It goes Stale when the game shows a monster of
    /// its name picking up, dropping or wielding something.
    pub carried_gear: BTreeMap<Sighting, Remembered<CarriedGear, GearSource>>,
    /// The Carried Gear of each unique the Advisor has read it for, by name, kept for the rest of
    /// the game. It goes Stale as a monster's does, and when the unique comes back into view.
    pub uniques_gear: BTreeMap<String, Remembered<CarriedGear, GearSource>>,
    /// Sighting messages whose monsters the game hadn't yet shown in view: crawl sends the
    /// message before the map that shows them. Each is dropped once the game's turn moves on.
    #[serde(skip)]
    unplaced: Vec<Unplaced>,
}

/// What the game said a monster's Carried Gear is, in which message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GearSource {
    /// The sighting message as it came into view.
    Sighting,
    /// The look text, as the look cursor rested on it.
    Look,
}

/// A sighting message's read of one monster's gear, waiting for the monster to be in view.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Unplaced {
    /// The monster's name, as the message gave it.
    name: String,
    /// What it was listed with.
    gear: CarriedGear,
    /// The turn the message was given on.
    turn: Option<u32>,
}

/// A fact the Advisor remembers, and how current it is: an engine input is Known, Stale or
/// Missing.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Remembered<T, S = Screen> {
    /// The Advisor has never read it.
    #[default]
    Missing,
    /// Read, and nothing the Advisor has seen since changes it.
    Known(AsOf<T, S>),
    /// Read, but something the Advisor has seen since may have changed it, so it is only an
    /// out-of-date copy.
    Stale(AsOf<T, S>),
}

/// A remembered value, with where and when it was read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AsOf<T, S = Screen> {
    /// What was read.
    pub value: T,
    /// The game's turn when it was read, if the game had reported one.
    pub turn: Option<u32>,
    /// What it was read off: the screen the player opened, or, for Carried Gear, the message.
    pub source: S,
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

impl<T, S> Remembered<T, S> {
    /// The value, when it is Known.
    #[must_use]
    pub fn known(&self) -> Option<&T> {
        match self {
            Self::Known(fact) => Some(&fact.value),
            Self::Missing | Self::Stale(_) => None,
        }
    }

    /// The value, with when and where it was read, when it is Known or Stale.
    #[must_use]
    pub fn read_as_of(&self) -> Option<&AsOf<T, S>> {
        match self {
            Self::Known(fact) | Self::Stale(fact) => Some(fact),
            Self::Missing => None,
        }
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

impl<T> Remembered<T> {
    /// `value`, Known as of now: read off `read` on `visible`'s turn.
    fn read(value: T, read: &ScreenRead, visible: &VisibleState) -> Self {
        Self::Known(AsOf {
            value,
            turn: visible.turn,
            source: read.screen.clone(),
        })
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
    /// reports now, reading through `knowledge` the messages crawl gives instead of the spell
    /// list or the ability menu when there is nothing to list, and what its messages say of the
    /// gear of monsters in view.
    pub(crate) fn update(
        &mut self,
        before: &VisibleState,
        now: &VisibleState,
        knowledge: Option<&Bundle>,
    ) {
        if before.stats.sh != now.stats.sh {
            self.worn_shield.go_stale();
        }
        let (was, is) = (&before.stats, &now.stats);
        let carried_changed = !carries_the_same(before, now);
        if carried_changed
            || (was.ac, was.ev, was.sh) != (is.ac, is.ev, is.sh)
            || before.light_at_player() != now.light_at_player()
        {
            self.equipped.go_stale();
        }
        if carried_changed
            || (was.xl, was.intelligence) != (is.xl, is.intelligence)
            || (was.ac, was.ev, was.sh) != (is.ac, is.ev, is.sh)
        {
            self.spells.go_stale();
        }
        if carried_changed
            || (was.xl, was.mp_max) != (is.xl, is.mp_max)
            || before.religion != now.religion
        {
            self.abilities.go_stale();
        }
        let Some(bundle) = knowledge else { return };
        let signs = &bundle.ways_out;
        self.notice_comings_and_goings(before, now, bundle);
        for message in new_messages(before, now) {
            let turn = message.turn.or(now.turn);
            if message.text == signs.no_spells {
                self.spells = Remembered::nothing_on(Screen::Spells, turn);
            }
            if message.text == signs.no_abilities {
                self.abilities = Remembered::nothing_on(Screen::Abilities, turn);
            }
            for said in carried_gear::said(&message.text, bundle) {
                self.take_in(said, turn, now, bundle);
            }
        }
        self.place_sightings(now, bundle);
    }

    /// The Carried Gear the Advisor has read for `monster`, by its Sighting, or by name for a
    /// unique, judged by `bundle`.
    #[must_use]
    pub fn gear_of(
        &self,
        monster: &VisibleMonster,
        bundle: &Bundle,
    ) -> Option<&Remembered<CarriedGear, GearSource>> {
        if is_unique(monster, bundle) {
            self.uniques_gear.get(&monster.name)
        } else {
            self.carried_gear.get(&monster.sighting?)
        }
    }

    /// Forgets the Carried Gear of monsters that left view, and makes a unique's Stale as it
    /// comes back into view.
    fn notice_comings_and_goings(
        &mut self,
        before: &VisibleState,
        now: &VisibleState,
        bundle: &Bundle,
    ) {
        let Some(in_view) = &now.monsters else { return };
        self.carried_gear.retain(|sighting, _| {
            in_view
                .iter()
                .any(|monster| monster.sighting == Some(*sighting))
        });
        let Some(was_in_view) = &before.monsters else {
            return;
        };
        for monster in in_view {
            let came_back = !was_in_view
                .iter()
                .any(|was| was.name == monster.name && (was.sighting == monster.sighting));
            if came_back
                && is_unique(monster, bundle)
                && let Some(gear) = self.uniques_gear.get_mut(&monster.name)
            {
                gear.go_stale();
            }
        }
    }

    /// Takes in what one message on `turn` said of monsters' gear, as `now` shows them.
    fn take_in(&mut self, said: Said, turn: Option<u32>, now: &VisibleState, bundle: &Bundle) {
        match said {
            Said::Sighting(sighting) => {
                let reads = sighting
                    .lone
                    .map(|LoneSighting { name, gear }| (name, gear))
                    .into_iter()
                    .chain(sighting.listed);
                self.unplaced
                    .extend(reads.map(|(name, gear)| Unplaced { name, gear, turn }));
            }
            Said::Look { name, gear } => {
                let in_view = now.monsters.iter().flatten();
                let under_cursor = in_view
                    .clone()
                    .find(|monster| Some(monster.position) == now.look_cursor)
                    .filter(|monster| names_match(&name, &monster.name));
                let mut named = in_view.filter(|monster| names_match(&name, &monster.name));
                let only_named = match (named.next(), named.next()) {
                    (Some(only), None) => Some(only),
                    _ => None,
                };
                if let Some(monster) = under_cursor.or(only_named) {
                    self.remember(monster, gear, GearSource::Look, turn, bundle);
                }
            }
            Said::Changed { name } => {
                for monster in now.monsters.iter().flatten() {
                    if !names_match(&name, &monster.name) {
                        continue;
                    }
                    let gear = if is_unique(monster, bundle) {
                        self.uniques_gear.get_mut(&monster.name)
                    } else {
                        monster
                            .sighting
                            .and_then(|sighting| self.carried_gear.get_mut(&sighting))
                    };
                    if let Some(gear) = gear {
                        gear.go_stale();
                    }
                }
            }
        }
    }

    /// Gives each sighting message's read the monster in `now` it names, once there is just one
    /// of that name whose gear the Advisor hasn't read in this Sighting. A read whose name fits
    /// several such monsters is dropped, as which of them it was can't be told; one whose
    /// monster isn't in view yet waits until the game's turn moves on.
    fn place_sightings(&mut self, now: &VisibleState, bundle: &Bundle) {
        for unplaced in std::mem::take(&mut self.unplaced) {
            let mut fits = now.monsters.iter().flatten().filter(|monster| {
                names_match(&unplaced.name, &monster.name)
                    && (is_unique(monster, bundle)
                        || monster
                            .sighting
                            .is_some_and(|sighting| !self.carried_gear.contains_key(&sighting)))
            });
            match (fits.next(), fits.next()) {
                (Some(monster), None) => {
                    let monster = monster.clone();
                    self.remember(
                        &monster,
                        unplaced.gear,
                        GearSource::Sighting,
                        unplaced.turn,
                        bundle,
                    );
                }
                (None, _) if unplaced.turn.is_none() || unplaced.turn == now.turn => {
                    self.unplaced.push(unplaced);
                }
                _ => {}
            }
        }
    }

    /// Remembers `gear` as `monster`'s, read off `source` on `turn`.
    fn remember(
        &mut self,
        monster: &VisibleMonster,
        gear: CarriedGear,
        source: GearSource,
        turn: Option<u32>,
        bundle: &Bundle,
    ) {
        let read = Remembered::Known(AsOf {
            value: gear,
            turn,
            source,
        });
        if is_unique(monster, bundle) {
            self.uniques_gear.insert(monster.name.clone(), read);
        } else if let Some(sighting) = monster.sighting {
            self.carried_gear.insert(sighting, read);
        }
    }
}

/// Whether `monster` is a unique, whose Carried Gear is its own across sightings.
fn is_unique(monster: &VisibleMonster, bundle: &Bundle) -> bool {
    monster
        .id
        .as_ref()
        .and_then(|id| bundle.monster_by_id(id))
        .is_some_and(|kind| kind.unique)
}

/// Whether a message's `said` name, without its article, names the monster called `name` in
/// view: as it is, or a unique by its title, as `Boris, Master of Life and Death` or `Agnes the
/// Wanderer` name `Boris` and `Agnes` (`dat/database/montitle.txt`).
fn names_match(said: &str, name: &str) -> bool {
    said.eq_ignore_ascii_case(name)
        || said
            .strip_prefix(name)
            .is_some_and(|title| title.starts_with(", ") || title.starts_with(" the "))
}

impl<T> Remembered<Vec<T>> {
    /// Known as of `turn` to list nothing, as crawl says in a message instead of opening
    /// `screen`.
    fn nothing_on(screen: Screen, turn: Option<u32>) -> Self {
        Self::Known(AsOf {
            value: Vec::new(),
            turn,
            source: screen,
        })
    }
}

/// Whether the player carries the same in `before` and `now`, by letters, names, quantities and
/// kinds: not whether crawl greys an item out, which changes with their statuses rather than
/// what they carry.
fn carries_the_same(before: &VisibleState, now: &VisibleState) -> bool {
    let key = |item: &InventoryItem| {
        (
            item.letter,
            item.name.clone(),
            item.quantity,
            item.kind.clone(),
        )
    };
    match (&before.inventory, &now.inventory) {
        (Some(was), Some(is)) => was.iter().map(key).eq(is.iter().map(key)),
        (was, is) => was.is_none() && is.is_none(),
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

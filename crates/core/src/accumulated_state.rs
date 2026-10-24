//! Accumulated State: what the Advisor remembers of one game that the game no longer shows, each
//! fact with where it came from and how recent it is.

use std::collections::BTreeMap;

use copilot_gk::{Bundle, EnumId, ItemKind};
use serde::{Deserialize, Serialize};

use crate::carried_gear::{self, CarriedGear, LoneSighting, Said};
use crate::crawl_ids::{armour_types, object_classes};
use crate::screen::{
    KnownAbility, KnownSpell, Resistances, Screen, ScreenRead, ShownMutations, Skills, SpellReach,
};
use crate::view_model::Colour;
use crate::visible_state::{
    GameMessage, InventoryItem, Sighting, Status, VisibleMonster, VisibleState,
};
use crate::ways_out;

/// What the Advisor remembers of one game.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccumulatedState {
    /// The character's resistances, as the character overview or dump last showed them, with
    /// those temporary effects give. They go Stale when anything that could change them does:
    /// what is carried or equipped (as [`AccumulatedState::equipped`] tells it), the experience
    /// level, the god or piety, or a status starting or ending. A change with no such sign, such
    /// as a mutation from a potion the inventory still shows another of, goes unnoticed.
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
    /// The power and range of the spells the character knows, as the spell list (`I`) last
    /// showed them switched to its other columns (`!`). It goes Stale as
    /// [`AccumulatedState::spells`] does, as power changes with the same.
    pub spell_reaches: Remembered<Vec<SpellReach>>,
    /// The character's skills, as the skills screen (`m`) or the character dump last showed
    /// them. They go Stale when the experience level or the statuses change, as a level brings
    /// skill points and a status such as heroism raises them for a while. Skills trained between
    /// levels, a little with each kill, go unnoticed.
    pub skills: Remembered<Skills>,
    /// The abilities the character has, with their costs and failure rates, as the ability
    /// menu (`a`) last showed them. It goes Stale when the experience level, god, piety,
    /// maximum magic points or inventory changes, as abilities come and go with them.
    ///
    /// For each of these, the inventory changes when what is carried does: crawl greying an
    /// item out or not, which follows the character's statuses, isn't a change.
    ///
    /// A Way Out the menu greyed out is remembered so only when nothing the Advisor saw as it
    /// was read explains it: a status that stops it or too little magic, which the Ways Out
    /// judge afresh every turn.
    pub abilities: Remembered<Vec<KnownAbility>>,
    /// The mutations and banes the character has, as the mutations screen (`A`) last showed
    /// them, with the experience level they had then. It goes Stale when the experience level
    /// changes, as banes lift and a species grows mutations with it, or the game gives a message
    /// of a mutation or bane that changes the character's speed coming or going. Any other
    /// mutation coming or going goes unnoticed.
    pub mutations: Remembered<MutationsRead>,
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

/// Where the game listed a monster's Carried Gear: in which message, or on the full-view list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GearSource {
    /// The sighting message as it came into view.
    Sighting,
    /// The look text, as the look cursor rested on it.
    Look,
    /// The full-view list (`Ctrl-X`), which lists every monster in view.
    FullView,
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
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AsOf<T, S = Screen> {
    /// What was read.
    pub value: T,
    /// The game's turn when it was read, if the game had reported one.
    pub turn: Option<u32>,
    /// What it was read off: the screen the player opened, or, for Carried Gear, the message.
    pub source: S,
}

/// What the mutations screen showed, and the experience level the character had then, which
/// tells which of the mutations it lists their species gave them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MutationsRead {
    /// What it showed.
    pub shown: ShownMutations,
    /// The character's experience level, if the game had reported it.
    pub xl: Option<u32>,
}

/// A shield the character can wear, by how many attacks it can block before their next action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
        if kind.class.0 != object_classes::ARMOUR {
            return None;
        }
        Self::of_armour(kind.sub_type.as_ref()?)
    }

    /// The shield a kind of armour is, by its `armour_type` identifier, if it is one.
    pub(crate) fn of_armour(sub_type: &EnumId) -> Option<Self> {
        match sub_type.0.as_str() {
            armour_types::BUCKLER => Some(Self::Buckler),
            armour_types::KITE_SHIELD => Some(Self::KiteShield),
            armour_types::TOWER_SHIELD => Some(Self::TowerShield),
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

    /// The value as it was read, Known or Stale: a Stale reading is applied all the same.
    #[must_use]
    pub fn read_value(&self) -> Option<&T> {
        match self {
            Self::Known(fact) | Self::Stale(fact) => Some(&fact.value),
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
    /// Remembers what a screen the player opened shows, read on `visible`'s turn through
    /// `knowledge`, the Game Knowledge for the game's Crawl Version, if the Advisor has it: on
    /// the full-view list, the Carried Gear of each monster it names that no other in view
    /// shares the name of.
    pub(crate) fn read(
        &mut self,
        read: &ScreenRead,
        visible: &VisibleState,
        knowledge: Option<&Bundle>,
    ) {
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
        if let Some(reaches) = read.spell_reaches() {
            self.spell_reaches = Remembered::read(reaches, read, visible);
        }
        if let Some(skills) = read.skills() {
            self.skills = Remembered::read(skills, read, visible);
        }
        let greyed_out =
            knowledge.and_then(|bundle| Colour::from_crawl(bundle.ways_out.inapplicable_colour));
        if let Some(mut abilities) = read.abilities(greyed_out) {
            if let Some(bundle) = knowledge {
                for ability in &mut abilities {
                    ability.greyed_out &= ways_out::greyed_out_unseen(visible, bundle, ability);
                }
            }
            self.abilities = Remembered::read(abilities, read, visible);
        }
        if let Some(shown) = read.mutations() {
            let mutations = MutationsRead {
                shown,
                xl: visible.stats.xl,
            };
            self.mutations = Remembered::read(mutations, read, visible);
        }
        if let (Some(bundle), Some(monsters)) = (knowledge, read.full_view_monsters()) {
            for description in monsters {
                let Some((name, gear)) = carried_gear::full_view(&description, bundle) else {
                    continue;
                };
                if let Some(monster) = only_named(&name, visible) {
                    self.remember(monster, gear, GearSource::FullView, visible.turn, bundle);
                }
            }
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
        let equipment_may_have_changed = carried_changed
            || (was.ac, was.ev, was.sh) != (is.ac, is.ev, is.sh)
            || before.light_at_player() != now.light_at_player();
        if equipment_may_have_changed {
            self.equipped.go_stale();
        }
        if equipment_may_have_changed
            || was.xl != is.xl
            || before.religion != now.religion
            || !lists_the_same_statuses(before, now)
        {
            self.resistances.go_stale();
        }
        if carried_changed
            || (was.xl, was.intelligence) != (is.xl, is.intelligence)
            || (was.ac, was.ev, was.sh) != (is.ac, is.ev, is.sh)
        {
            self.spells.go_stale();
            self.spell_reaches.go_stale();
        }
        if was.xl != is.xl || !lists_the_same_statuses(before, now) {
            self.skills.go_stale();
        }
        if carried_changed
            || (was.xl, was.mp_max) != (is.xl, is.mp_max)
            || before.religion != now.religion
        {
            self.abilities.go_stale();
        }
        if was.xl != is.xl {
            self.mutations.go_stale();
        }
        let Some(bundle) = knowledge else { return };
        let signs = &bundle.ways_out;
        self.notice_comings_and_goings(before, now, bundle);
        for message in new_messages(before, now) {
            let turn = message.turn.or(now.turn);
            if changes_speed(&message.text, bundle) {
                self.mutations.go_stale();
            }
            if message.text == signs.no_spells {
                self.spells = Remembered::nothing_on(Screen::Spells, turn);
                self.spell_reaches = Remembered::nothing_on(Screen::Spells, turn);
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
        match GearKey::of(monster, bundle)? {
            GearKey::Sighting(sighting) => self.carried_gear.get(&sighting),
            GearKey::Unique(name) => self.uniques_gear.get(name),
        }
    }

    /// The Carried Gear remembered under `key`, to change.
    fn gear_mut(&mut self, key: GearKey<'_>) -> Option<&mut Remembered<CarriedGear, GearSource>> {
        match key {
            GearKey::Sighting(sighting) => self.carried_gear.get_mut(&sighting),
            GearKey::Unique(name) => self.uniques_gear.get_mut(name),
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
                && let Some(key @ GearKey::Unique(_)) = GearKey::of(monster, bundle)
                && let Some(gear) = self.gear_mut(key)
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
                let under_cursor = now
                    .monsters
                    .iter()
                    .flatten()
                    .find(|monster| Some(monster.position) == now.look_cursor)
                    .filter(|monster| names_match(&name, &monster.name));
                if let Some(monster) = under_cursor.or_else(|| only_named(&name, now)) {
                    self.remember(monster, gear, GearSource::Look, turn, bundle);
                }
            }
            Said::Changed { name } => {
                for monster in now.monsters.iter().flatten() {
                    if !names_match(&name, &monster.name) {
                        continue;
                    }
                    if let Some(gear) =
                        GearKey::of(monster, bundle).and_then(|key| self.gear_mut(key))
                    {
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
                    && match GearKey::of(monster, bundle) {
                        Some(GearKey::Sighting(sighting)) => {
                            !self.carried_gear.contains_key(&sighting)
                        }
                        Some(GearKey::Unique(_)) => true,
                        None => false,
                    }
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
        match GearKey::of(monster, bundle) {
            Some(GearKey::Sighting(sighting)) => {
                self.carried_gear.insert(sighting, read);
            }
            Some(GearKey::Unique(name)) => {
                self.uniques_gear.insert(name.to_owned(), read);
            }
            None => {}
        }
    }
}

/// Whose Carried Gear a monster's is: its Sighting's, or, for a unique, its own across
/// sightings, by name.
#[derive(Debug, Clone, Copy)]
enum GearKey<'a> {
    Sighting(Sighting),
    Unique(&'a str),
}

impl<'a> GearKey<'a> {
    /// `monster`'s, its kind judged by `bundle`: none for a monster that isn't a unique and that
    /// the Game Source can't tell apart from others.
    fn of(monster: &'a VisibleMonster, bundle: &Bundle) -> Option<Self> {
        let unique = monster
            .id
            .as_ref()
            .and_then(|id| bundle.monster_by_id(id))
            .is_some_and(|kind| kind.unique);
        if unique {
            Some(Self::Unique(&monster.name))
        } else {
            monster.sighting.map(Self::Sighting)
        }
    }
}

/// The one monster in `now` that the `said` name fits, if no other in view fits it too.
fn only_named<'a>(said: &str, now: &'a VisibleState) -> Option<&'a VisibleMonster> {
    let mut named = now
        .monsters
        .iter()
        .flatten()
        .filter(|monster| names_match(said, &monster.name));
    match (named.next(), named.next()) {
        (Some(only), None) => Some(only),
        _ => None,
    }
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

/// Whether `message` tells of a mutation or bane that changes the character's speed coming or
/// going, as `bundle` gives their messages.
fn changes_speed(message: &str, bundle: &Bundle) -> bool {
    let speed = &bundle.speed;
    speed
        .mutations
        .iter()
        .flat_map(|mutation| &mutation.messages)
        .chain(speed.banes.iter().flat_map(|bane| &bane.messages))
        .any(|said| said == message)
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

/// Whether the status pane lists the same statuses in `before` and `now`, by their lights and
/// words: not whether one's colour changed, as it does when it starts to run out.
fn lists_the_same_statuses(before: &VisibleState, now: &VisibleState) -> bool {
    let key = |status: &Status| (status.light.clone(), status.text.clone());
    match (&before.statuses, &now.statuses) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_model::Colour;
    use crate::visible_state::{Religion, Stats};

    /// A character with one potion, a status lit `Pois`, XL 5 and AC 3, who worships Trog at two
    /// stars.
    fn character() -> VisibleState {
        VisibleState {
            turn: Some(40),
            stats: Stats {
                xl: Some(5),
                ac: Some(3),
                ev: Some(10),
                sh: Some(0),
                ..Stats::default()
            },
            religion: Some(Religion::God {
                name: "Trog".to_owned(),
                piety_rank: Some(2),
                penance: false,
            }),
            statuses: Some(vec![Status {
                light: Some("Pois".to_owned()),
                text: Some("poisoned".to_owned()),
                colour: Some(Colour::Yellow),
            }]),
            inventory: Some(vec![InventoryItem {
                letter: 'a',
                name: "a potion of curing".to_owned(),
                quantity: 1,
                kind: None,
                useless: false,
            }]),
            ..VisibleState::default()
        }
    }

    /// Whether the resistances read off the character overview in `before` are Stale once the
    /// game reports `now`.
    fn goes_stale(before: &VisibleState, now: &VisibleState) -> bool {
        let overview = ScreenRead {
            screen: Screen::CharacterOverview,
            title: None,
            lines: vec!["rFire   + . .".to_owned()],
            colours: BTreeMap::new(),
        };
        let mut remembered = AccumulatedState::default();
        remembered.read(&overview, before, None);
        assert!(remembered.resistances.known().is_some());
        remembered.update(before, now, None);
        matches!(remembered.resistances, Remembered::Stale(_))
    }

    #[test]
    fn resistances_stay_known_while_nothing_that_changes_them_does() {
        let before = character();
        let now = VisibleState {
            turn: Some(41),
            ..character()
        };
        assert!(!goes_stale(&before, &now));
    }

    #[test]
    fn resistances_stay_known_as_a_status_changes_colour_running_out() {
        let mut now = character();
        if let Some(status) = now
            .statuses
            .as_mut()
            .and_then(|statuses| statuses.first_mut())
        {
            status.colour = Some(Colour::DarkGrey);
        }
        assert!(!goes_stale(&character(), &now));
    }

    #[test]
    fn resistances_go_stale_when_a_status_starts_or_ends() {
        let mut started = character();
        started.statuses.get_or_insert_with(Vec::new).push(Status {
            light: Some("Resist".to_owned()),
            text: Some("resistant".to_owned()),
            colour: Some(Colour::LightBlue),
        });
        assert!(goes_stale(&character(), &started));
        let ended = VisibleState {
            statuses: Some(Vec::new()),
            ..character()
        };
        assert!(goes_stale(&character(), &ended));
    }

    #[test]
    fn resistances_go_stale_when_the_inventory_or_equipment_changes() {
        let mut picked_up = character();
        picked_up
            .inventory
            .get_or_insert_with(Vec::new)
            .push(InventoryItem {
                letter: 'b',
                name: "a ring of fire".to_owned(),
                quantity: 1,
                kind: None,
                useless: false,
            });
        assert!(goes_stale(&character(), &picked_up));
        let mut put_on = character();
        put_on.stats.ac = Some(4);
        assert!(goes_stale(&character(), &put_on));
    }

    #[test]
    fn resistances_go_stale_when_the_experience_level_changes() {
        let mut levelled = character();
        levelled.stats.xl = Some(6);
        assert!(goes_stale(&character(), &levelled));
    }

    /// The mutations screen as a naga with a lethargy bane shows it.
    fn mutations_screen() -> ScreenRead {
        ScreenRead {
            screen: Screen::Mutations,
            title: Some("Innate Abilities, Weirdness & Mutations".to_owned()),
            lines: vec![
                " a - Your snake-like lower body moves slowly.".to_owned(),
                " b - You cover ground slowly. [3.5]".to_owned(),
            ],
            colours: BTreeMap::new(),
        }
    }

    /// The shipped 0.34.1 bundle.
    fn bundle() -> &'static Bundle {
        static LIBRARY: std::sync::OnceLock<copilot_gk::Library> = std::sync::OnceLock::new();
        LIBRARY
            .get_or_init(|| copilot_gk::Library::shipped().unwrap())
            .for_version("0.34.1")
            .unwrap()
    }

    /// The mutations read off the mutations screen in `before`, once the game reports `now`.
    fn mutations_after(before: &VisibleState, now: &VisibleState) -> Remembered<MutationsRead> {
        let mut remembered = AccumulatedState::default();
        remembered.read(&mutations_screen(), before, None);
        remembered.update(before, now, Some(bundle()));
        remembered.mutations
    }

    #[test]
    fn the_mutations_screen_is_known_as_of_the_turn_and_level_it_was_read() {
        let mut remembered = AccumulatedState::default();
        assert_eq!(remembered.mutations, Remembered::Missing);
        remembered.read(&mutations_screen(), &character(), None);
        let Remembered::Known(read) = &remembered.mutations else {
            panic!("{:?}", remembered.mutations);
        };
        assert_eq!(read.turn, Some(40));
        assert_eq!(read.value.xl, Some(5));
        assert_eq!(read.value.shown.banes, ["You cover ground slowly."]);
        let next_turn = VisibleState {
            turn: Some(41),
            ..character()
        };
        assert!(matches!(
            mutations_after(&character(), &next_turn),
            Remembered::Known(_)
        ));
    }

    #[test]
    fn the_mutations_go_stale_when_the_experience_level_changes() {
        let mut levelled = character();
        levelled.stats.xl = Some(6);
        assert!(matches!(
            mutations_after(&character(), &levelled),
            Remembered::Stale(_)
        ));
    }

    #[test]
    fn the_mutations_go_stale_when_the_game_says_a_mutation_or_bane_that_changes_speed_came_or_went()
     {
        for message in [
            "You feel sluggish.",
            "Your lower body returns to normal.",
            "The Bane of Lethargy upon you is lifted.",
        ] {
            let mut said = character();
            said.messages.push(GameMessage {
                text: message.to_owned(),
                turn: Some(40),
            });
            assert!(
                matches!(mutations_after(&character(), &said), Remembered::Stale(_)),
                "{message}"
            );
        }
        let mut other = character();
        other.messages.push(GameMessage {
            text: "You feel a little pissed off.".to_owned(),
            turn: Some(40),
        });
        assert!(matches!(
            mutations_after(&character(), &other),
            Remembered::Known(_)
        ));
    }

    #[test]
    fn resistances_go_stale_when_the_god_or_piety_changes() {
        let pious = VisibleState {
            religion: Some(Religion::God {
                name: "Trog".to_owned(),
                piety_rank: Some(3),
                penance: false,
            }),
            ..character()
        };
        assert!(goes_stale(&character(), &pious));
        let converted = VisibleState {
            religion: Some(Religion::God {
                name: "the Shining One".to_owned(),
                piety_rank: Some(0),
                penance: false,
            }),
            ..character()
        };
        assert!(goes_stale(&character(), &converted));
    }
}

//! Accumulated State: what the Advisor remembers of one game that the game no longer shows, each
//! fact with where it came from and how recent it is.

use std::collections::{BTreeMap, BTreeSet};

use copilot_gk::{Bundle, EnumId, ItemKind, Mutation};
use serde::{Deserialize, Serialize};

use crate::carried_gear::{self, CarriedGear, LoneSighting, Said};
use crate::crawl_ids::{armour_types, bane_types, object_classes};
use crate::explored::Explored;
use crate::gaps::ScreenFact;
use crate::screen::{
    DungeonOverview, KnownAbility, KnownSpell, Resistances, Screen, ScreenRead, ShownMutations,
    Skills, SpellReach,
};
use crate::told::{MutationChange, Told, told};
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
    /// level, the god or piety, a status starting or ending, or the game telling of a mutation
    /// coming or going.
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
    /// showed them, less those the game has said they forgot since. It goes Stale when the
    /// experience level, Intelligence, inventory, AC, EV or SH changes, or the game tells of a
    /// skill's level or a mutation changing, as failure rates change with them, and when it
    /// tells of a spell memorised or learned, as it doesn't say how often that fails. Skills
    /// trained within a level, which lower failure rates bit by bit, go unnoticed.
    pub spells: Remembered<Vec<KnownSpell>>,
    /// The power and range of the spells the character knows, as the spell list (`I`) last
    /// showed them switched to its other columns (`!`). It goes Stale as
    /// [`AccumulatedState::spells`] does, as power changes with the same.
    pub spell_reaches: Remembered<Vec<SpellReach>>,
    /// The character's skills, as the skills screen (`m`) or the character dump last showed
    /// them, each at the level the game has said it reached since, taken at the whole level, as
    /// the message tells no fraction. They go Stale when the statuses change, as a status such
    /// as heroism raises them for a while, when the game says a base level changed, which
    /// differs from the level in effect, and when the Bane of the Dilettante, which lowers some,
    /// comes or goes. Skills trained within a level, a little with each kill, and those
    /// crosstraining raises as a related skill trains, go unnoticed; so do those of a follower of
    /// a god who shifts them with piety, a Blind Spot.
    pub skills: Remembered<Skills>,
    /// The abilities the character has, with their costs and failure rates, as the ability
    /// menu (`a`) last showed them. It goes Stale when the experience level, god, piety,
    /// maximum magic points or inventory changes, as abilities come and go with them, and when
    /// the game tells of a god's power coming or going, or of a skill's level or a mutation
    /// changing.
    ///
    /// For each of these, the inventory changes when what is carried does: crawl greying an
    /// item out or not, which follows the character's statuses, isn't a change.
    ///
    /// A Way Out the menu greyed out is remembered so only when nothing the Advisor saw as it
    /// was read explains it: a status that stops it or too little magic, which the Ways Out
    /// judge afresh every turn.
    pub abilities: Remembered<Vec<KnownAbility>>,
    /// The mutations and banes the character has, as the mutations screen (`A`) or the
    /// character dump last showed them, with the experience level they had then: the dump
    /// leaves the banes unread. Each mutation or bane the game has said came or went since is
    /// changed to match, as Game Knowledge words their messages. It goes Stale when the
    /// experience level changes, as banes lift and a species grows mutations with it, and when
    /// a message doesn't fit what was read, or fits more than one mutation it could tell of. A
    /// mutation whose message crawl writes by hand, such as one of strength, goes unnoticed.
    pub mutations: Remembered<MutationsRead>,
    /// The levels the character has found, as the dungeon overview (`Ctrl-O`) or the character
    /// dump last showed them. It goes Stale when the character changes level, as the level they
    /// come to may be explored deeper than any before, or hold an altar or a shop. One found on a
    /// level they were already on goes unnoticed. The dungeon overview doesn't tell which altars
    /// were found, so reading it leaves them unread even after the dump told them.
    #[serde(default)]
    pub dungeon: Remembered<DungeonOverview>,
    /// Whether the Advisor has caught up on what it missed of a game it joined part way through.
    #[serde(default)]
    pub backfill: Backfill,
    /// The character memorised a spell the game's message didn't name since the spell list or
    /// the dump last listed their spells: a Blind Spot until one of them is read again.
    #[serde(default)]
    pub spell_memorised: Option<Memorised>,
    /// The facts the game's Gap prompt card asked the player to fill: none until it has shown.
    /// It shows once a game, whether the player filled them, put it away or an Encounter did.
    #[serde(default)]
    pub gaps_prompted: BTreeSet<ScreenFact>,
    /// The Carried Gear of each monster in view the Advisor has read it for, but uniques', by
    /// its Sighting: forgotten once it leaves view. It goes Stale when the game shows a monster of
    /// its name picking up, dropping or wielding something.
    pub carried_gear: BTreeMap<Sighting, Remembered<CarriedGear, GearSource>>,
    /// The Carried Gear of each unique the Advisor has read it for, by name, kept for the rest of
    /// the game. It goes Stale as a monster's does, and when the unique comes back into view.
    pub uniques_gear: BTreeMap<String, Remembered<CarriedGear, GearSource>>,
    /// The levels the player has explored, and the uniques they met.
    #[serde(flatten)]
    pub explored: Explored,
    /// Sighting messages whose monsters the game hadn't yet shown in view: crawl sends the
    /// message before the map that shows them. Each is dropped once the game's turn moves on.
    #[serde(skip)]
    unplaced: Vec<Unplaced>,
}

/// A spell memorised that the game's message didn't name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Memorised {
    /// The game's turn when the message said so, if the game had reported one.
    pub turn: Option<u32>,
}

/// Whether the Advisor has caught up on what it missed of a game: its Backfill.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Backfill {
    /// Nothing it missed remains to be read: it followed the game from its first turn, or has
    /// read the character dump since it joined.
    #[default]
    Complete,
    /// It joined the game part way through and hasn't read the character dump since.
    Awaiting {
        /// The game's turn when it joined, if the game had reported one.
        joined_on: Option<u32>,
    },
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
pub enum Remembered<T, S = Source> {
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
pub struct AsOf<T, S = Source> {
    /// What was read.
    pub value: T,
    /// The game's turn when it was read, if the game had reported one.
    pub turn: Option<u32>,
    /// What it was read off: the screen the player opened or the message the game gave, or,
    /// for Carried Gear, where the game listed it.
    pub source: S,
}

/// Where a fact about the character was last read: a screen the player opened, or a message the
/// game gave as the fact changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    /// A message the game gave.
    Message,
    /// A screen the player opened, written as the screen alone, as every fact was before
    /// messages changed them, so what was saved then reads the same.
    #[serde(untagged)]
    Screen(Screen),
}

impl From<Screen> for Source {
    fn from(screen: Screen) -> Self {
        Self::Screen(screen)
    }
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
    /// Changes the value as a message on `turn` tells, by `change`, which says whether the
    /// message fits the value, and leaves it untouched when it doesn't. A Known fact that fits
    /// is Known as of the message, and one that doesn't goes Stale; a Stale fact stays Stale as
    /// of when it was read, as the rest of it is no more current, and a Missing one Missing.
    fn amend(&mut self, turn: Option<u32>, change: impl FnOnce(&mut T) -> bool) {
        *self = match std::mem::take(self) {
            Self::Known(mut fact) => {
                if change(&mut fact.value) {
                    Self::Known(AsOf {
                        value: fact.value,
                        turn,
                        source: Source::Message,
                    })
                } else {
                    Self::Stale(fact)
                }
            }
            Self::Stale(mut fact) => {
                change(&mut fact.value);
                Self::Stale(fact)
            }
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
            source: read.screen.clone().into(),
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
        if let Some(mut spells) = read.spells() {
            if read.screen == Screen::CharacterDump
                && let Some(bundle) = knowledge
            {
                for spell in &mut spells {
                    spell.name = whole_spell_name(&spell.name, bundle);
                }
            }
            self.spells = Remembered::read(spells, read, visible);
            self.spell_memorised = None;
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
        if let Some(overview) = read.dungeon_overview() {
            self.dungeon = Remembered::read(overview, read, visible);
        }
        if read.screen == Screen::CharacterDump {
            self.backfill = Backfill::Complete;
        }
        if let Some(bundle) = knowledge {
            self.explored.read_shop(read, visible, bundle);
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
        if !lists_the_same_statuses(before, now) {
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
        if before.place != now.place {
            self.dungeon.go_stale();
        }
        let Some(bundle) = knowledge else { return };
        let signs = &bundle.ways_out;
        self.notice_comings_and_goings(before, now, bundle);
        self.explored.hear(new_messages(before, now), now, bundle);
        for message in new_messages(before, now) {
            let turn = message.turn.or(now.turn);
            for news in told(&message.text, bundle) {
                self.hear(news, turn);
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

    /// Takes in what a message on `turn` told of the character.
    fn hear(&mut self, news: Told<'_>, turn: Option<u32>) {
        match news {
            Told::Skill { name, level, base } => {
                // Failure rates and power follow skills.
                self.spells.go_stale();
                self.spell_reaches.go_stale();
                self.abilities.go_stale();
                if base {
                    self.skills.go_stale();
                } else {
                    self.skills.amend(turn, |skills| {
                        skills.tenths.insert(name, level * 10);
                        true
                    });
                }
            }
            Told::SpellForgotten(name) => {
                self.spells
                    .amend(turn, |spells| removed(spells, |spell| spell.name == name));
                self.spell_reaches
                    .amend(turn, |reaches| removed(reaches, |reach| reach.name == name));
            }
            Told::SpellMemorised => {
                self.spells.go_stale();
                self.spell_reaches.go_stale();
                self.spell_memorised = Some(Memorised { turn });
            }
            Told::SpellLearned => {
                self.spells.go_stale();
                self.spell_reaches.go_stale();
            }
            Told::Power => self.abilities.go_stale(),
            Told::Mutation(changes) => {
                // A mutation may change resistances, abilities and failure rates.
                self.resistances.go_stale();
                self.abilities.go_stale();
                self.spells.go_stale();
                self.spell_reaches.go_stale();
                self.mutations
                    .amend(turn, |read| changed(&mut read.shown.mutations, &changes));
            }
            Told::Bane { bane, lifted } => {
                if bane.id.0 == bane_types::DILETTANTE {
                    self.skills.go_stale();
                }
                // Banes unread stay unread.
                if self
                    .mutations
                    .read_value()
                    .is_some_and(|read| read.shown.banes.is_some())
                {
                    self.mutations.amend(turn, |read| {
                        let banes = read.shown.banes.get_or_insert_with(Vec::new);
                        let had = banes.iter().position(|shown| *shown == bane.shown);
                        match (had, lifted) {
                            (Some(at), true) => {
                                banes.remove(at);
                                true
                            }
                            (None, false) => {
                                banes.push(bane.shown.clone());
                                true
                            }
                            (Some(_), false) => true,
                            (None, true) => false,
                        }
                    });
                }
            }
        }
    }

    /// Takes up again a game picked up again, last seen on `last_turn`, as `now` shows it. Its
    /// game process is new, and with it every Sighting, so no monster's Carried Gear but a
    /// unique's carries over. The rest is kept as it was while the game is no further on, as when
    /// a page reloads or is replayed; once it is, the turns the Advisor didn't see may have
    /// changed any of it, so all of it goes Stale.
    pub(crate) fn pick_up(&mut self, last_turn: Option<u32>, now: &VisibleState) {
        self.carried_gear.clear();
        self.explored.pick_up();
        let moved_on = match (last_turn, now.turn) {
            (Some(last), Some(turn)) => turn > last,
            _ => true,
        };
        if moved_on {
            self.go_stale();
        }
    }

    /// Takes in a Settled Update, as `visible` shows the game, through `knowledge`: what the
    /// player has explored, with whether a hostile is in view, as `hostile_in_view` tells.
    pub(crate) fn settle(
        &mut self,
        visible: &VisibleState,
        knowledge: Option<&Bundle>,
        hostile_in_view: bool,
    ) {
        self.explored.settle(visible, knowledge, hostile_in_view);
    }

    /// The facts whose values are collections the Change Log logs entry by entry, as each grows
    /// large: each level the player has explored.
    pub const LOGGED_BY_ENTRY: &'static [&'static str] = &["levels"];

    /// The Advisor joined the game as `now` shows it, having seen none of it before: part way
    /// through, unless on its first turn, so it has missed what only the character dump tells.
    pub(crate) fn join(&mut self, now: &VisibleState) {
        if now.turn != Some(0) {
            self.backfill = Backfill::Awaiting {
                joined_on: now.turn,
            };
        }
    }

    /// Every fact remembered goes Stale.
    fn go_stale(&mut self) {
        let Self {
            resistances,
            worn_shield,
            equipped,
            spells,
            spell_reaches,
            skills,
            abilities,
            mutations,
            dungeon,
            backfill: _,
            spell_memorised: _,
            gaps_prompted: _,
            carried_gear,
            uniques_gear,
            explored: _,
            unplaced: _,
        } = self;
        resistances.go_stale();
        worn_shield.go_stale();
        equipped.go_stale();
        spells.go_stale();
        spell_reaches.go_stale();
        skills.go_stale();
        abilities.go_stale();
        mutations.go_stale();
        dungeon.go_stale();
        for gear in carried_gear.values_mut().chain(uniques_gear.values_mut()) {
            gear.go_stale();
        }
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
            source: screen.into(),
        })
    }
}

/// The whole name of the spell the character dump names `shown`, which it cuts short when it is
/// long: the one spell in `bundle` named so, or else the one whose name starts so. `shown` when
/// no spell or several fit.
fn whole_spell_name(shown: &str, bundle: &Bundle) -> String {
    let names = || bundle.spells.values().map(|spell| spell.name.as_str());
    if names().any(|name| name == shown) {
        return shown.to_owned();
    }
    let mut starting = names().filter(|name| name.starts_with(shown));
    match (starting.next(), starting.next()) {
        (Some(whole), None) => whole.to_owned(),
        _ => shown.to_owned(),
    }
}

/// Removes the first of `items` that `is`, if any: whether there was one.
fn removed<T>(items: &mut Vec<T>, is: impl Fn(&T) -> bool) -> bool {
    match items.iter().position(is) {
        Some(at) => {
            items.remove(at);
            true
        }
        None => false,
    }
}

/// Changes `shown`, the descriptions of the mutations read, by the one of `changes` whose
/// mutation is at the level it starts from: whether exactly one was. Crawl gives some messages
/// for more than one change, such as a mutation gained or its opposite lost, and for each level
/// of some mutations alike, so which it was is told by the level `shown` describes.
fn changed(shown: &mut Vec<String>, changes: &[MutationChange<'_>]) -> bool {
    let mut fitting = changes.iter().filter_map(|change| {
        let (at, level) = level_shown(shown, change.mutation)?;
        (level == change.from).then_some((change, at))
    });
    let (Some((change, at)), None) = (fitting.next(), fitting.next()) else {
        return false;
    };
    let description = change
        .to
        .checked_sub(1)
        .and_then(|level| change.mutation.shown.get(level))
        .cloned();
    match (at, description) {
        (Some(at), Some(description)) => {
            if let Some(entry) = shown.get_mut(at) {
                *entry = description;
            }
        }
        (Some(at), None) => {
            shown.remove(at);
        }
        (None, Some(description)) => shown.push(description),
        (None, None) => return false,
    }
    true
}

/// Where among `shown` the mutation's description is, and the level of it that describes, 0
/// where none does; `None` when more than one does.
fn level_shown(shown: &[String], mutation: &Mutation) -> Option<(Option<usize>, usize)> {
    let mut found = shown.iter().enumerate().filter_map(|(at, entry)| {
        (0..mutation.shown.len())
            .find(|&level| mutation.describes(entry, level))
            .map(|level| (at, level + 1))
    });
    match (found.next(), found.next()) {
        (None, _) => Some((None, 0)),
        (Some((at, level)), None) => Some((Some(at), level)),
        (Some(_), Some(_)) => None,
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

/// Whether the status pane lists the same statuses in `before` and `now`, by their lights and
/// words: not whether one's colour changed, as it does when it starts to run out.
fn lists_the_same_statuses(before: &VisibleState, now: &VisibleState) -> bool {
    let key = |status: &Status| (status.light.clone(), status.text.clone());
    match (&before.statuses, &now.statuses) {
        (Some(was), Some(is)) => was.iter().map(key).eq(is.iter().map(key)),
        (was, is) => was.is_none() && is.is_none(),
    }
}

/// The messages `now` shows that `before` didn't. The game keeps only its latest messages, and
/// takes back the last lines it rewrites, such as a prompt it echoes with the answer, so `now`
/// starts with the longest run of `before`'s messages it still shows and what follows is new:
/// all of it when it shows none of them. A line rewritten is new again, as crawl rewrites a
/// message it repeats, as `You feel quick. x2`.
fn new_messages<'a>(before: &VisibleState, now: &'a VisibleState) -> &'a [GameMessage] {
    let (was, is) = (&before.messages, &now.messages);
    let kept = (0..was.len())
        .map(|from| {
            was.iter()
                .skip(from)
                .zip(is)
                .take_while(|(was, is)| was == is)
                .count()
        })
        .max()
        .unwrap_or(0);
    is.get(kept..).unwrap_or_default()
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
    use crate::screen::DUMP_SPELL_NAME_LETTERS;
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

    /// The mutations screen as [`mutations_screen`] shows it, read at experience level 5.
    fn mutations_screen_read() -> MutationsRead {
        MutationsRead {
            shown: mutations_screen().mutations().unwrap(),
            xl: Some(5),
        }
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
        assert_eq!(
            read.value.shown.banes,
            Some(vec!["You cover ground slowly.".to_owned()])
        );
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

    /// What the mutations screen in `character` shows once the game says `message`.
    fn mutations_told(message: &str) -> Remembered<MutationsRead> {
        let mut said = character();
        said.messages.push(GameMessage {
            text: message.to_owned(),
            turn: Some(40),
        });
        mutations_after(&character(), &said)
    }

    /// A character remembered with each fact Known as of turn 40, once the game says
    /// `message`: none of the Scenarios hears it.
    fn told_unrecorded(message: &str) -> AccumulatedState {
        let known = |value| AsOf {
            value,
            turn: Some(40),
            source: Source::Screen(Screen::Spells),
        };
        let mut remembered = AccumulatedState {
            skills: Remembered::Known(AsOf {
                value: Skills::default(),
                turn: Some(40),
                source: Source::Screen(Screen::Skills),
            }),
            spells: Remembered::Known(known(Vec::new())),
            abilities: Remembered::Known(AsOf {
                value: Vec::new(),
                turn: Some(40),
                source: Source::Screen(Screen::Abilities),
            }),
            ..AccumulatedState::default()
        };
        remembered.read(&mutations_screen(), &character(), None);
        let mut said = character();
        said.messages.push(GameMessage {
            text: message.to_owned(),
            turn: Some(40),
        });
        remembered.update(&character(), &said, Some(bundle()));
        remembered
    }

    #[test]
    fn messages_no_scenario_hears_put_what_they_change_out_of_date() {
        let learned = told_unrecorded("The power to cast Foxfire wells up from within.");
        assert!(matches!(learned.spells, Remembered::Stale(_)));
        let gifted = told_unrecorded("(You can now cast Foxfire.)");
        assert!(matches!(gifted.spells, Remembered::Stale(_)));
        let lost =
            told_unrecorded("You can no longer call upon Trog for regeneration and willpower.");
        assert!(matches!(lost.abilities, Remembered::Stale(_)));
        let base = told_unrecorded("Your base Fighting skill increases to level 6!");
        assert!(matches!(base.skills, Remembered::Stale(_)));
        let dilettante = told_unrecorded("You are stricken with the Bane of the Dilettante.");
        assert!(matches!(dilettante.skills, Remembered::Stale(_)));
        let Remembered::Known(mutations) = &dilettante.mutations else {
            panic!("{:?}", dilettante.mutations);
        };
        assert!(
            mutations
                .value
                .shown
                .banes
                .iter()
                .flatten()
                .any(|bane| bane == "You are less proficient with several skills.")
        );
        let told = told_unrecorded("Your Fighting skill increases to level 6!");
        assert_eq!(
            told.skills.known().map(|skills| skills.tenths("Fighting")),
            Some(60)
        );
    }

    /// Crawl gives `You feel sluggish.` as slowness is gained and as speed is lost: no speed
    /// is shown, so it was slowness.
    #[test]
    fn a_message_two_mutations_share_changes_the_one_at_the_level_it_starts_from() {
        let Remembered::Known(read) = mutations_told("You feel sluggish.") else {
            panic!("not Known");
        };
        assert_eq!(read.source, Source::Message);
        assert_eq!(
            read.value.shown.mutations,
            [
                "Your snake-like lower body moves slowly.",
                "You cover ground slowly."
            ]
        );
        let Remembered::Known(lifted) = mutations_told("The Bane of Lethargy upon you is lifted.")
        else {
            panic!("not Known");
        };
        assert_eq!(lifted.value.shown.banes, Some(Vec::new()));
    }

    #[test]
    fn a_mutation_message_that_doesnt_fit_the_mutations_read_puts_them_out_of_date() {
        let Remembered::Stale(read) = mutations_told("You feel less cold resistant.") else {
            panic!("not Stale");
        };
        assert_eq!(read.value, mutations_screen_read());
        assert!(matches!(
            mutations_told("You feel a little pissed off."),
            Remembered::Known(read) if read.value == mutations_screen_read()
        ));
    }

    /// A fact's source is saved with the game: a screen as it always was, so games saved before
    /// messages changed facts read the same, or a message.
    #[test]
    fn a_facts_source_is_saved_and_read_back_whether_a_screen_or_a_message() {
        for (source, saved) in [
            (Source::Screen(Screen::Skills), r#"{"kind":"skills"}"#),
            (Source::Message, r#""message""#),
        ] {
            assert_eq!(serde_json::to_string(&source).unwrap(), saved);
            assert_eq!(serde_json::from_str::<Source>(saved).unwrap(), source);
        }
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

    /// A spell name the dump cuts short, and trims should the cut fall on a space, is completed
    /// only when one spell starts so: each spell in a shipped bundle must come back whole.
    #[test]
    fn every_spell_in_a_shipped_bundle_is_remembered_whole_as_the_dump_cuts_its_name() {
        let library = copilot_gk::Library::shipped().unwrap();
        let mut bundles = 0;
        for bundle in library.bundles() {
            bundles += 1;
            for spell in bundle.spells.values() {
                let shown: String = spell.name.chars().take(DUMP_SPELL_NAME_LETTERS).collect();
                assert_eq!(
                    whole_spell_name(shown.trim_end(), bundle),
                    spell.name,
                    "{}: the dump's {shown:?}",
                    bundle.crawl.tag
                );
            }
        }
        assert!(bundles > 0, "no shipped bundles");
    }

    /// The dump cuts a spell's name to fit its column; no Scenario knows a spell named so long.
    #[test]
    fn a_spell_whose_name_the_character_dump_cuts_short_is_remembered_by_its_whole_name() {
        let dump = ScreenRead {
            screen: Screen::CharacterDump,
            title: None,
            lines: [
                "You know the following spells:",
                "",
                " Your Spells              Type           Power      Damage    Failure   Level",
                "a - Iskenderun's Mystic   Conj/Tloc      12 (100)   2d5 (+2d2)13%         4",
                "b - Magic Dart            Conj           22 (25)    1d7       1%          1",
                "",
            ]
            .map(str::to_owned)
            .to_vec(),
            colours: BTreeMap::new(),
        };
        let mut remembered = AccumulatedState::default();
        remembered.read(&dump, &character(), Some(bundle()));
        let names: Vec<_> = remembered
            .spells
            .known()
            .unwrap()
            .iter()
            .map(|spell| (spell.name.as_str(), spell.failure_percent, spell.level))
            .collect();
        assert_eq!(
            names,
            [("Iskenderun's Mystic Blast", 13, 4), ("Magic Dart", 1, 1)]
        );
    }
}

//! The Threat Level's Monte Carlo: every monster that can reach the player before their next
//! standard-speed action, casting its spells, zapping its wand, shooting and fighting in melee
//! through crawl's own dice, many times over.

use copilot_gk::{
    Bundle, EnumId, Formulas, ItemUse, MeleeWeapon, MissileKind, Monster, MonsterSpells, Odds, Wand,
};

use crate::accumulated_state::Shield;
use crate::carried_gear::{
    CarriedGear, CarriedItem, GearSlot, ItemBrand, Unlisted, Unplayed, WAND_OF,
};
use crate::crawl_ids::{missile_types, monster_types, object_classes};

use super::brands::{Brand, Necromancy};
use super::lighting::Lighting;
use super::melee::{
    self, Attack, Defences, DisplayedDefences, ENERGY_THRESHOLD, Fighter, Hands, Target, Wielded,
    attack_reach, energy_gained, starting_energy,
};
use super::missiles::{self, MISSILE_RANGE, Missile, Missiles, Shooter};
use super::rng::Rng;
use super::spells::{self, DamagingSpell, Firing, Landing, Prospect, Reach, Slot};
use super::wands;

/// How many times the next standard-speed action is played out. At 10,000 the chance of death
/// is within about 0.15 percentage points of its true value near the DANGEROUS threshold.
pub(crate) const TRIALS: u32 = 10_000;

/// The seed the Monte Carlo always starts from, so the same Game Situation always gives the same
/// Fact Sheet: the engine reads no clock and no entropy.
const SEED: (u64, u64) = (0x00dc_55c0_9170_7000, 0x0000_0000_0000_0034);

/// The worst-case hit is the total damage at this percentile of trials (spec #24, Engine).
const WORST_HIT_PERCENTILE: u32 = 99;

/// The player's defences, as the game reports them.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Defender {
    /// Current hit points.
    pub(crate) hp: i32,
    /// AC, EV and SH, as displayed.
    pub(crate) defences: DisplayedDefences,
    /// How the light falls on them.
    pub(crate) lighting: Lighting,
    /// The most attacks the shield blocks before the player's next action.
    pub(crate) shield_blocks: u32,
}

impl Defender {
    /// The player as one trial's attacks meet them: in their light, with the fraction of their
    /// defences the display drops drawn afresh ([`Defences::drawn_behind`]).
    fn target(&self, rng: &mut Rng) -> Target {
        Target {
            defences: Defences::drawn_behind(self.defences, rng),
            lighting: self.lighting,
        }
    }
}

/// The most attacks the player's shield blocks before their next action, wearing `shield`: the
/// fewest, as without one, when it isn't known.
pub(crate) fn shield_blocks_per_action(formulas: Formulas, shield: Option<Shield>) -> u32 {
    melee::shield_block_limit(formulas.shield_block, shield)
}

/// A monster that may attack the player, how far away it is, the melee weapons it fights with,
/// and the spells, wands and missiles it may use that the engine ports.
#[derive(Debug, Clone)]
pub(crate) struct Attacker<'a> {
    /// Its Game Knowledge.
    pub(crate) monster: &'a Monster,
    /// Squares between it and the player, counting diagonals as one, as crawl's `grid_distance`.
    pub(crate) distance: i32,
    /// The damaging spells of its book that the engine ports, in book order, as it casts them.
    pub(crate) spells: Vec<Cast<'a>>,
    /// What it may shoot or throw: its Carried Gear's, or by the odds crawl makes its kind with.
    pub(crate) missiles: Draw<Missiles<'a>>,
    /// The wands it may carry whose spells the engine ports: its Carried Gear's, or by the odds
    /// crawl makes its kind with. A wand whose spell isn't ported counts as none: it is assumed
    /// never to zap it.
    pub(crate) wands: Draw<WandZap<'a>>,
    /// It may carry a wand whose spell the engine doesn't port.
    pub(crate) unported_wand: bool,
    /// What it may shoot, throw or zap that is played out, by name: `shortbow` or `wand of
    /// flame` for its kind's, and as the game named them for its Carried Gear's.
    pub(crate) shoots: Vec<String>,
    /// It firing what it shoots or throws.
    shooter: Shooter,
    /// It fighting in melee, with the melee weapons its Carried Gear shows, if read.
    pub(crate) fighter: Fighter,
}

/// A damaging spell as a particular monster casts it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Cast<'a> {
    /// The spell.
    pub(crate) spell: DamagingSpell<'a>,
    /// How its book holds it.
    slot: Slot,
    /// The power the monster casts it with.
    power: i32,
    /// How far it reaches.
    pub(crate) reach: Reach,
}

/// A wand as a particular monster zaps it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct WandZap<'a> {
    /// The wand.
    pub(crate) wand: &'a Wand,
    /// The spell it zaps.
    pub(crate) spell: DamagingSpell<'a>,
    /// The power the monster zaps it with.
    power: i32,
    /// How far it reaches.
    pub(crate) reach: Reach,
}

/// Which of some items a monster carries, drawn afresh for every trial: by the odds crawl makes
/// its kind with when its Carried Gear isn't read, or what it was seen with.
#[derive(Debug, Clone)]
pub(crate) struct Draw<T> {
    out_of: u32,
    items: Vec<(T, u32)>,
}

impl<T: Copy> Draw<T> {
    /// The items of `odds`, each as `item` makes it: one it makes nothing of counts as none.
    pub(crate) fn new<'o>(odds: &'o Odds, item: impl FnMut(&'o EnumId) -> Option<T>) -> Self {
        Self::given(odds, |_| true, item)
    }

    /// The items of `odds` that `may_be` holds for, each as `item` makes it, given that it isn't
    /// any of the others: their chances shared out again among those left and none. Never any
    /// item, when it can only be one of the others.
    pub(crate) fn given<'o>(
        odds: &'o Odds,
        mut may_be: impl FnMut(&EnumId) -> bool,
        mut item: impl FnMut(&'o EnumId) -> Option<T>,
    ) -> Self {
        let (kept, ruled_out): (Vec<_>, Vec<_>) = odds
            .items
            .iter()
            .partition(|weighted| may_be(&weighted.item));
        let ruled_out: u32 = ruled_out.iter().map(|weighted| weighted.weight).sum();
        let Some(out_of) = odds.out_of.checked_sub(ruled_out).filter(|&left| left > 0) else {
            return Self::none();
        };
        Self {
            out_of,
            items: kept
                .into_iter()
                .filter_map(|weighted| Some((item(&weighted.item)?, weighted.weight)))
                .collect(),
        }
    }

    /// The items of `odds`, each as `item` makes it, given that it is one of them.
    pub(crate) fn one_of<'o>(
        odds: &'o Odds,
        mut item: impl FnMut(&'o EnumId) -> Option<T>,
    ) -> Self {
        let out_of: u32 = odds.items.iter().map(|weighted| weighted.weight).sum();
        if out_of == 0 {
            return Self::none();
        }
        Self {
            out_of,
            items: odds
                .items
                .iter()
                .filter_map(|weighted| Some((item(&weighted.item)?, weighted.weight)))
                .collect(),
        }
    }

    /// `item`, always.
    pub(crate) fn always(item: T) -> Self {
        Self {
            out_of: 1,
            items: vec![(item, 1)],
        }
    }

    /// No item, ever.
    pub(crate) fn none() -> Self {
        Self {
            out_of: 1,
            items: Vec::new(),
        }
    }

    /// Whether it is never any item.
    pub(crate) fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Every item it may be, whatever its chance.
    pub(crate) fn items(&self) -> impl Iterator<Item = &T> {
        self.items.iter().map(|(item, _)| item)
    }

    /// The item carried this trial, if any.
    pub(crate) fn draw(&self, rng: &mut Rng) -> Option<T> {
        let mut roll =
            u32::try_from(rng.random2(i32::try_from(self.out_of).unwrap_or(i32::MAX))).unwrap_or(0);
        for &(item, weight) in &self.items {
            if roll < weight {
                return Some(item);
            }
            roll -= weight;
        }
        None
    }
}

impl<'a> Attacker<'a> {
    /// `monster`, `distance` squares from the player, with the spells of its book and the gear
    /// of its kind the engine ports, its Possible Gear, looked up in `bundle`. A spell, missile or
    /// wand the bundle lacks is left out.
    pub(crate) fn new(bundle: &'a Bundle, monster: &'a Monster, distance: i32) -> Self {
        let formulas = bundle.formulas;
        let hit_dice = hit_dice(monster);
        let book = match &monster.spells {
            MonsterSpells::Book { spells } => spells.as_slice(),
            MonsterSpells::PerMonster => &[],
        };
        let spells = book
            .iter()
            .filter_map(|slot| {
                let spell = bundle.spell_by_id(&slot.spell)?;
                let damaging = spells::damaging(formulas.monster_spell_beam, spell)?;
                let power = spells::power(formulas.monster_spell_power, &spell.id, hit_dice);
                let range = spells::range(formulas.monster_spell_power, spell.range?, power);
                Some(Cast {
                    spell: damaging,
                    slot: Slot::of(slot),
                    power,
                    reach: spells::reach(formulas.monster_spell_beam, &damaging, range),
                })
            })
            .collect();
        let missiles = Draw::new(&monster.gear.missile, |id| plain_missile(bundle, id));
        let wands = Draw::new(&monster.gear.wand, |id| wand_zap(bundle, monster, id));
        let mut attacker = Self {
            monster,
            distance,
            spells,
            missiles,
            wands,
            unported_wand: unported(bundle, monster, &monster.gear.wand),
            shoots: Vec::new(),
            shooter: Shooter::of(bundle, monster, hit_dice),
            fighter: Fighter {
                hit_dice,
                skilled: monster.fighter,
                nightvision: monster.nightvision,
                necromancy: Necromancy::of(bundle, monster, hit_dice),
                draugr: monster.id.0 == monster_types::DRAUGR,
                hands: unread_hands(monster),
            },
        };
        attacker.shoots = attacker.possible_names();
        attacker
    }

    /// The same monster, playing its Carried Gear, `gear`, in place of its kind's Possible Gear:
    /// the melee weapons and what it shoots, throws or zaps that the game named, and, where the
    /// listing would leave out what it carries, its kind's Possible Gear given that it is nothing
    /// the listing would have named. With what of the gear isn't played as it is.
    pub(crate) fn carrying(self, bundle: &'a Bundle, gear: &CarriedGear) -> (Self, Vec<Unplayed>) {
        let mut unplayed = Vec::new();
        let hands = carried_hands(bundle, self.monster, gear, &mut unplayed);
        let (missiles, mut shoots) = carried_missiles(bundle, self.monster, gear, &mut unplayed);
        let (wands, unported_wand, zaps) = carried_wand(bundle, self.monster, gear, &mut unplayed);
        shoots.extend(zaps);
        let attacker = Self {
            missiles,
            wands,
            unported_wand,
            shoots,
            fighter: Fighter {
                hands,
                ..self.fighter
            },
            ..self
        };
        (attacker, unplayed)
    }

    /// The same monster, carrying nothing at all: an animated object still fights as the item it
    /// is, which stays unknown.
    pub(crate) fn without_gear(self) -> Self {
        let hands = (!animated_object(self.monster)).then(Hands::default);
        Self {
            missiles: Draw::none(),
            wands: Draw::none(),
            unported_wand: false,
            shoots: Vec::new(),
            fighter: Fighter {
                hands,
                ..self.fighter
            },
            ..self
        }
    }

    /// Whether it may fight in melee with a weapon the Advisor doesn't know: its Carried Gear
    /// isn't read, or doesn't show what it holds.
    pub(crate) fn weapons_unread(&self, formulas: Formulas) -> bool {
        self.fighter.hands.is_none() && melee_reach(formulas, &self.fighter, self.monster) > 0
    }

    /// The names of the launchers, missiles and wands of its Possible Gear that are played out.
    fn possible_names(&self) -> Vec<String> {
        kinds_of_missile(&self.missiles)
            .chain(kinds_of_wand(&self.wands))
            .collect()
    }

    /// Whether it may shoot a launcher, or fight with a melee weapon, whose brand's damage the
    /// player's resistances would cut.
    pub(crate) fn has_resisted_brand(&self) -> bool {
        let launchers = self
            .missiles
            .items()
            .filter_map(|missiles| missiles.launcher)
            .map(|launcher| launcher.brand);
        let hands = self.fighter.hands.unwrap_or_default();
        let held = [hands.main, hands.off]
            .into_iter()
            .flatten()
            .map(|wielded| wielded.weapon.brand);
        launchers.chain(held).any(Brand::resisted)
    }

    /// Whether it can strike the player from `distance` squares in melee, with a spell, a wand
    /// or a missile, if it may carry them.
    fn strikes_from(&self, formulas: Formulas, distance: i32) -> bool {
        let monster = self.monster;
        let shoots = !self.missiles.is_empty()
            && distance <= MISSILE_RANGE
            && (distance > 1 || monster.prefers_ranged);
        distance >= 1
            && (distance <= melee_reach(formulas, &self.fighter, monster)
                || self
                    .spells
                    .iter()
                    .any(|cast| cast.reach.strikes >= distance)
                || self.wands.items().any(|zap| zap.reach.strikes >= distance)
                || shoots)
    }
}

/// Whether `monster` is an animated object, such as a dancing weapon, which fights as the item it
/// is (`mons_class_is_animated_object`): its Carried Gear never lists that item.
fn animated_object(monster: &Monster) -> bool {
    [
        monster_types::DANCING_WEAPON,
        monster_types::SPECTRAL_WEAPON,
        monster_types::ARMOUR_ECHO,
        monster_types::HAUNTED_ARMOUR,
    ]
    .contains(&monster.id.0.as_str())
}

/// The melee weapons `monster` holds before its Carried Gear is read: none for a kind that uses no
/// items, which never holds one, and not known for any other, or for an animated object.
fn unread_hands(monster: &Monster) -> Option<Hands> {
    (monster.item_use < ItemUse::StartingEquipment && !animated_object(monster))
        .then(Hands::default)
}

/// The melee weapons `monster` holds as its Carried Gear, `gear`, shows them, once it has wielded
/// the one it fights with, noting in `unplayed` what of them isn't played as the game named it.
/// Not known when the listing leaves out a plain weapon in its main hand and names nothing there,
/// or when it names something in its hands the Game Knowledge doesn't know as a weapon, such as a
/// magical staff or an artefact whose name doesn't show its kind.
///
/// A monster wielding a launcher, or nothing, swaps in a melee weapon it carries
/// (`monster::wield_melee_weapon`, `monster.cc:1083`). One that wields two weapons is listed with
/// its main hand's first, and with a pair alike as one counted in words, such as `two +0 clubs`
/// (`_describe_monster_weapon`, `directn.cc`).
fn carried_hands(
    bundle: &Bundle,
    monster: &Monster,
    gear: &CarriedGear,
    unplayed: &mut Vec<Unplayed>,
) -> Option<Hands> {
    if animated_object(monster) || monster.item_use < ItemUse::StartingEquipment {
        return unread_hands(monster);
    }
    let mut wielded = Vec::new();
    let mut carried = Vec::new();
    for item in &gear.items {
        if !matches!(item.slot, GearSlot::Wielding | GearSlot::Carrying) {
            continue;
        }
        let kind = item.kind.as_ref();
        if kind.is_some_and(|kind| kind.class.0 == object_classes::WANDS) {
            continue;
        }
        let sub_type = kind.and_then(|kind| kind.sub_type.as_ref());
        if let Some(known) = sub_type.and_then(|id| bundle.melee_weapons.get(id)) {
            match item.slot {
                GearSlot::Wielding => wielded.push((item, known)),
                _ => carried.push((item, known)),
            }
        } else if !sub_type.is_some_and(|id| bundle.missile_weapons.contains_key(id)) {
            return None;
        }
    }
    let wields = gear
        .items
        .iter()
        .any(|item| item.slot == GearSlot::Wielding);
    if !wields && gear.unlisted == Unlisted::PlainWeaponsAndLargeRocks {
        return None;
    }
    let mut hold = |(item, known): (&CarriedItem, &MeleeWeapon)| {
        let plus = weapon_plus(item, unplayed);
        Wielded::of(known, plus, weapon_brand(bundle, item, unplayed))
    };
    if monster.two_weapons {
        let pair = wielded
            .first()
            .is_some_and(|(item, _)| item.name.starts_with(PAIR));
        let mut hands = wielded.into_iter().map(&mut hold);
        let main = hands.next();
        let off = if pair { main } else { hands.next() };
        Some(Hands { main, off })
    } else {
        let main = wielded.into_iter().chain(carried).next().map(hold);
        Some(Hands { main, off: None })
    }
}

/// How crawl starts the name of a pair of weapons alike that a monster wields two of.
const PAIR: &str = "two ";

/// What `monster` shoots or throws of its Carried Gear, `gear`, with the names of what is played:
/// the launcher and the missiles to throw the game named, or, when it named neither, its kind's
/// Possible Gear given that it is nothing the listing would have named. What isn't played as the
/// game named it goes in `unplayed`.
fn carried_missiles<'a>(
    bundle: &'a Bundle,
    monster: &Monster,
    gear: &CarriedGear,
    unplayed: &mut Vec<Unplayed>,
) -> (Draw<Missiles<'a>>, Vec<String>) {
    let variant = bundle.formulas.missile_attack;
    let mut names = Vec::new();
    let (mut launcher, mut thrown) = (None, None);
    for item in &gear.items {
        let Some((id, weapon)) = item
            .kind
            .as_ref()
            .and_then(|kind| kind.sub_type.as_ref())
            .and_then(|id| bundle.missile_weapons.get_key_value(id))
        else {
            continue;
        };
        match (weapon.kind, item.slot) {
            (MissileKind::Launcher, GearSlot::Wielding | GearSlot::Carrying)
                if launcher.is_none() =>
            {
                let plus = weapon_plus(item, unplayed);
                let brand = weapon_brand(bundle, item, unplayed);
                launcher = Some(Missile::of(variant, id, weapon, plus, brand));
                names.push(item.name.clone());
            }
            (MissileKind::Thrown, GearSlot::Quivering) if thrown.is_none() => {
                if let ItemBrand::Unnamed(brand) = &item.brand {
                    unplayed.push(Unplayed::BrandNotPorted {
                        item: item.name.clone(),
                        brand: brand.clone(),
                    });
                }
                thrown = Some(Missile::plain(variant, id, weapon));
                names.push(item.name.clone());
            }
            _ => {}
        }
    }
    if launcher.is_some() || thrown.is_some() {
        return (Draw::always(Missiles { launcher, thrown }), names);
    }
    let wields = gear
        .items
        .iter()
        .any(|item| item.slot == GearSlot::Wielding);
    let may_be = |id: &EnumId| match gear.unlisted {
        Unlisted::Nothing => false,
        Unlisted::PlainLargeRocks => id.0 == missile_types::LARGE_ROCK,
        // An ordinary weapon goes unlisted only in the main hand.
        Unlisted::PlainWeaponsAndLargeRocks => {
            id.0 == missile_types::LARGE_ROCK
                || !wields
                    && bundle
                        .missile_weapons
                        .get(id)
                        .is_some_and(|weapon| weapon.kind == MissileKind::Launcher)
        }
    };
    let missiles = Draw::given(&monster.gear.missile, may_be, |id| {
        plain_missile(bundle, id)
    });
    let names = kinds_of_missile(&missiles).collect();
    (missiles, names)
}

/// The wand `monster` zaps of its Carried Gear, `gear`, whether it may carry one whose spell
/// isn't ported, and the names of what is played: the wand the game named, or, for one it
/// couldn't name, one of its kind's wands by their odds. What isn't played goes in `unplayed`.
fn carried_wand<'a>(
    bundle: &'a Bundle,
    monster: &Monster,
    gear: &CarriedGear,
    unplayed: &mut Vec<Unplayed>,
) -> (Draw<WandZap<'a>>, bool, Vec<String>) {
    let Some(item) = gear.items.iter().find(|item| {
        item.kind
            .as_ref()
            .is_some_and(|kind| kind.class.0 == object_classes::WANDS)
    }) else {
        return (Draw::none(), false, Vec::new());
    };
    match item.kind.as_ref().and_then(|kind| kind.sub_type.as_ref()) {
        Some(id) => match wand_zap(bundle, monster, id) {
            Some(zap) => (Draw::always(zap), false, vec![item.name.clone()]),
            None => {
                unplayed.push(Unplayed::WandNotPorted {
                    item: item.name.clone(),
                });
                (Draw::none(), true, Vec::new())
            }
        },
        // An unidentified wand, which can't be told apart from its kind's others: one of those,
        // by their odds. A named wand the Game Knowledge lacks isn't one of them.
        None if !item.name.starts_with(WAND_OF) && monster.gear.wand.is_possible() => {
            let wands = Draw::one_of(&monster.gear.wand, |id| wand_zap(bundle, monster, id));
            let names = kinds_of_wand(&wands).collect();
            (wands, unported(bundle, monster, &monster.gear.wand), names)
        }
        None => {
            unplayed.push(Unplayed::UnnamedWand {
                item: item.name.clone(),
            });
            (Draw::none(), false, Vec::new())
        }
    }
}

/// The names of the kinds of launcher and missile `missiles` may be, such as `shortbow`.
fn kinds_of_missile<'d>(missiles: &'d Draw<Missiles<'_>>) -> impl Iterator<Item = String> + 'd {
    missiles
        .items()
        .flat_map(|missiles| missiles.each())
        .map(|missile| missile.weapon.name.clone())
}

/// The names of the kinds of wand `wands` may be, such as `wand of flame`.
fn kinds_of_wand<'d>(wands: &'d Draw<WandZap<'_>>) -> impl Iterator<Item = String> + 'd {
    wands
        .items()
        .map(|zap| format!("{WAND_OF}{}", zap.wand.name))
}

/// The launcher or throwing missile `id`, +0 and unbranded, if the bundle has it.
fn plain_missile<'a>(bundle: &'a Bundle, id: &EnumId) -> Option<Missiles<'a>> {
    let weapon = bundle.missile_weapons.get(id)?;
    Some(Missiles::of(Missile::plain(
        bundle.formulas.missile_attack,
        id,
        weapon,
    )))
}

/// The wand `id` as `monster` zaps it, if the bundle has it and the engine ports its spell.
fn wand_zap<'a>(bundle: &'a Bundle, monster: &Monster, id: &EnumId) -> Option<WandZap<'a>> {
    let formulas = bundle.formulas;
    let wand = bundle.wands.get(id)?;
    let spell = bundle.spell_by_id(&wand.spell)?;
    let damaging = spells::damaging(formulas.monster_spell_beam, spell)?;
    let power = wands::power(
        formulas.monster_wand,
        formulas.monster_spell_power,
        &spell.id,
        hit_dice(monster),
    );
    let range = spells::range(formulas.monster_spell_power, spell.range?, power);
    Some(WandZap {
        wand,
        spell: damaging,
        power,
        reach: spells::reach(formulas.monster_spell_beam, &damaging, range),
    })
}

/// Whether `odds` may give `monster` a wand whose spell the engine doesn't port.
fn unported(bundle: &Bundle, monster: &Monster, odds: &Odds) -> bool {
    odds.items
        .iter()
        .any(|weighted| weighted.weight > 0 && wand_zap(bundle, monster, &weighted.item).is_none())
}

/// The enchantment a launcher or melee weapon seen as `item` is played with, noting in `unplayed`
/// one its name doesn't show, which is played as +0.
fn weapon_plus(item: &CarriedItem, unplayed: &mut Vec<Unplayed>) -> i32 {
    item.plus.unwrap_or_else(|| {
        unplayed.push(Unplayed::UnknownPlus {
            item: item.name.clone(),
        });
        0
    })
}

/// The brand a launcher or melee weapon seen as `item` is played with, noting in `unplayed` one
/// its name doesn't show or whose effect isn't ported.
fn weapon_brand(bundle: &Bundle, item: &CarriedItem, unplayed: &mut Vec<Unplayed>) -> Brand {
    match &item.brand {
        ItemBrand::None => Brand::Plain,
        ItemBrand::Weapon(id) => {
            let brand = Brand::of(bundle.formulas.damage_brand, id);
            if brand == Brand::Other {
                unplayed.push(Unplayed::BrandNotPorted {
                    item: item.name.clone(),
                    brand: bundle
                        .weapon_brands
                        .get(id)
                        .map_or_else(|| id.0.clone(), |brand| brand.name.clone()),
                });
            }
            brand
        }
        ItemBrand::Unnamed(_) | ItemBrand::Unknown => {
            unplayed.push(Unplayed::UnknownBrand {
                item: item.name.clone(),
            });
            Brand::Plain
        }
    }
}

/// What playing out the player's next action many times showed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Outcome {
    /// Trials in which the player died.
    pub(crate) deaths: u32,
    /// Trials played out.
    pub(crate) trials: u32,
    /// The total damage taken at the 99th percentile of trials.
    pub(crate) worst_hit: i32,
}

/// Whether `attacker` can attack before the player's next standard-speed action at all: with the
/// most energy it could have, it closes the distance, unless it never moves, and still has an
/// action left to strike the player in melee, with a spell or wand, or with a missile. Where a
/// spell lands its caster ([`spells::Landing`]) never changes the answer: a blinkbolt strikes the
/// player from wherever it is cast at them.
pub(crate) fn can_attack(formulas: Formulas, attacker: &Attacker<'_>) -> bool {
    let monster = attacker.monster;
    let mut energy = melee::most_starting_energy(formulas.monster_energy) + speed(monster);
    let mut distance = attacker.distance;
    while energy >= ENERGY_THRESHOLD {
        if attacker.strikes_from(formulas, distance) {
            return true;
        }
        if monster.stationary || distance <= 1 {
            return false;
        }
        distance -= 1;
        energy -= cost(monster.energy.walk);
    }
    false
}

/// Plays out the player's next standard-speed action [`TRIALS`] times against `attackers`. Each
/// trial draws the fraction of the player's defences the display drops: it is fixed for the
/// player at any moment, and crawl rounds it afresh on every roll.
pub(crate) fn simulate(
    formulas: Formulas,
    defender: Defender,
    attackers: &[Attacker<'_>],
) -> Outcome {
    let mut rng = Rng::new(SEED.0, SEED.1);
    let mut totals: Vec<i32> = (0..TRIALS)
        .map(|_| {
            let target = defender.target(&mut rng);
            one_action(
                formulas,
                &mut rng,
                target,
                defender.shield_blocks,
                attackers,
            )
        })
        .collect();
    totals.sort_unstable();
    let deaths = totals.iter().filter(|&&total| total >= defender.hp).count();
    // The nearest-rank percentile: the smallest total at least 99% of trials stay within.
    let rank = (TRIALS * WORST_HIT_PERCENTILE).div_ceil(100);
    Outcome {
        deaths: u32::try_from(deaths).unwrap_or(TRIALS),
        trials: TRIALS,
        worst_hit: totals
            .get(usize::try_from(rank).unwrap_or(0).saturating_sub(1))
            .copied()
            .unwrap_or(0),
    }
}

/// The damage `target` takes in one trial of their next action, with a shield that blocks up to
/// `shield_blocks` attacks.
fn one_action(
    formulas: Formulas,
    rng: &mut Rng,
    target: Target,
    shield_blocks: u32,
    attackers: &[Attacker<'_>],
) -> i32 {
    let mut blocks_left = shield_blocks;
    attackers
        .iter()
        .map(|attacker| turn(formulas, rng, attacker, target, &mut blocks_left).damage)
        .sum()
}

/// What one monster did before the player's next standard-speed action, in one trial.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Turn {
    /// The damage it dealt the player.
    pub(crate) damage: i32,
    /// The melee actions it took against the player.
    pub(crate) melee: i32,
    /// The spells it cast.
    pub(crate) casts: i32,
    /// The squares it closed on the player.
    pub(crate) closed: i32,
}

/// Plays out `attacker`'s actions before the player's next standard-speed action against
/// `target`, whose shield has `blocks_left` blocks left this action.
pub(crate) fn turn(
    formulas: Formulas,
    rng: &mut Rng,
    attacker: &Attacker<'_>,
    target: Target,
    blocks_left: &mut u32,
) -> Turn {
    let monster = attacker.monster;
    let fighter = &attacker.fighter;
    let reach = melee_reach(formulas, fighter, monster);
    let mut distance = attacker.distance;
    let mut energy = starting_energy(formulas.monster_energy, rng)
        + energy_gained(formulas.monster_energy, rng, speed(monster));
    let missiles = attacker.missiles.draw(rng);
    let wand = attacker.wands.draw(rng);
    let shooter = attacker.shooter;
    let mut turn = Turn::default();
    // The actions left before it can breathe again.
    let mut breath_timeout: u32 = 0;
    let mut first_action = true;
    while energy >= ENERGY_THRESHOLD
        && (reach > 0 || !attacker.spells.is_empty() || missiles.is_some() || wand.is_some())
    {
        // It catches its breath after each action (`_post_monster_move`, `mon-act.cc:2569`).
        if !first_action {
            breath_timeout = breath_timeout.saturating_sub(1);
        }
        first_action = false;
        // A monster tries its spells before anything else, then its wand, then its missiles
        // (`_mons_take_special_action`, `mon-act.cc:1840`).
        let can_breathe = breath_timeout == 0;
        if let Some(cast) = choose_cast(formulas, rng, &attacker.spells, distance, can_breathe) {
            turn.casts += 1;
            if cast.reach.strikes >= distance {
                let firing = Firing {
                    spell: &cast.spell,
                    power: cast.power,
                    nightvision: monster.nightvision,
                };
                turn.damage += fire_beam(formulas, rng, firing, target, blocks_left);
                land(&cast.spell, &mut distance, &mut turn);
            }
            if cast.slot.breath {
                breath_timeout = spells::breath_timeout(formulas.monster_spell_choice, rng);
            }
            // An instant spell takes no time, and the monster goes on to act
            // (`handle_mon_spell`, `mon-cast.cc:5228`).
            if !cast.slot.instant {
                energy -= cost(monster.energy.spell);
                continue;
            }
        }
        // Its tracer must find the player (`_handle_wand`).
        if let Some(zap) = wand
            && wands::zaps(formulas.monster_wand, rng)
            && zap.reach.casts >= distance
        {
            if zap.reach.strikes >= distance {
                let firing = Firing {
                    spell: &zap.spell,
                    power: zap.power,
                    nightvision: monster.nightvision,
                };
                turn.damage += fire_beam(formulas, rng, firing, target, blocks_left);
                land(&zap.spell, &mut distance, &mut turn);
            }
            energy -= wands::energy(formulas.monster_wand);
            continue;
        }
        if let Some(missiles) = missiles
            && let Some(missile) = missiles::fires(
                formulas.monster_fire,
                rng,
                missiles,
                monster.prefers_ranged,
                distance,
            )
        {
            let shot = missiles::shoot(formulas, rng, shooter, missile, target, *blocks_left > 0);
            if shot.blocked {
                *blocks_left -= 1;
            }
            turn.damage += shot.damage;
            energy -= missiles::fire_energy(formulas, rng, missile, cost(monster.energy.missile));
            continue;
        }
        if reach == 0 || distance > reach {
            // It walks straight at the player, unless it never moves: walls and other
            // monsters aren't modelled.
            if !monster.stationary && distance > 1 {
                distance -= 1;
                turn.closed += 1;
            }
            energy -= cost(monster.energy.walk);
            continue;
        }
        turn.melee += 1;
        // Every attack it makes, as one melee action makes them
        // (`melee_attack::run_monster_attack_set`, `melee-attack.cc:1569`), each with its weapon.
        for (_, attack, weapon) in attacks_from(formulas, fighter, monster, distance) {
            let weapon = weapon.map(|wielded| wielded.weapon);
            let swing = melee::swing(
                formulas,
                rng,
                fighter,
                attack,
                weapon,
                target,
                *blocks_left > 0,
            );
            if swing.blocked {
                *blocks_left -= 1;
            }
            turn.damage += swing.damage;
        }
        energy -= melee::melee_energy(
            formulas,
            rng,
            fighter,
            first_attack(monster),
            cost(monster.energy.attack),
        );
    }
    turn
}

/// Moves a monster `distance` squares from the player to where casting `spell` at them leaves
/// it ([`spells::Landing`]), counting the squares it closed in `turn`. Only a beam that reaches
/// the player lands its caster by them; one that falls short would carry it to its end.
fn land(spell: &DamagingSpell<'_>, distance: &mut i32, turn: &mut Turn) {
    match spell.landing {
        Landing::Stays => {}
        Landing::BesideThePlayer => {
            turn.closed += *distance - 1;
            *distance = 1;
        }
    }
}

/// The damage the beam `firing` deals `target`, using up a block of their shield when it blocks
/// it.
fn fire_beam(
    formulas: Formulas,
    rng: &mut Rng,
    firing: Firing<'_, '_>,
    target: Target,
    blocks_left: &mut u32,
) -> i32 {
    let bolt = spells::bolt(formulas.beam_damage, rng, firing);
    let shot = spells::shoot(
        formulas.beam_hit,
        formulas.beam_damage,
        rng,
        bolt,
        target,
        *blocks_left > 0,
    );
    if shot.blocked {
        *blocks_left -= 1;
    }
    shot.damage
}

/// The spell `attacker` casts this action, if any, `distance` squares from the player. A breath
/// can't be cast while it is catching its breath: crawl drops it from the usable spells
/// (`_find_usable_spells`).
fn choose_cast<'c, 'a>(
    formulas: Formulas,
    rng: &mut Rng,
    casts: &'c [Cast<'a>],
    distance: i32,
    can_breathe: bool,
) -> Option<&'c Cast<'a>> {
    let usable: Vec<&Cast<'a>> = casts
        .iter()
        .filter(|cast| can_breathe || !cast.slot.breath)
        .collect();
    // With no usable spell, crawl rolls for none (`handle_mon_spell`).
    if usable.is_empty() {
        return None;
    }
    let prospects: Vec<Prospect> = usable
        .iter()
        .map(|cast| Prospect {
            frequency: cast.slot.frequency,
            when: cast.slot.when,
            in_range: cast.reach.casts >= distance,
        })
        .collect();
    spells::choose(formulas.monster_spell_choice, rng, &prospects, distance)
        .and_then(|i| usable.get(i).copied())
}

/// How far `monster`'s melee reaches, fighting as `fighter`: its farthest-reaching attack that
/// deals damage, or the weapon of its first attack if that reaches farther, as
/// `monster::reach_range` (`monster.cc:6224`). 0 when it has no melee attack.
pub(crate) fn melee_reach(formulas: Formulas, fighter: &Fighter, monster: &Monster) -> i32 {
    let by_flavour = attacks(monster)
        .filter(|(_, attack)| attack.damage > 0)
        .map(|(_, attack)| attack_reach(formulas.monster_reach, attack))
        .max();
    by_flavour.map_or(0, |reach| {
        let primary = first_attack(monster)
            .and_then(|attack| fighter.weapon(formulas.monster_weapon, 0, attack));
        melee::reach_with(formulas.monster_reach, reach, primary)
    })
}

/// The attacks `monster`, fighting as `fighter`, makes in one melee action from `distance`
/// squares, in crawl's order, each with its slot among the monster's attacks and the weapon it
/// makes it with: none from beyond its reach.
pub(crate) fn attacks_from<'m>(
    formulas: Formulas,
    fighter: &'m Fighter,
    monster: &'m Monster,
    distance: i32,
) -> impl Iterator<Item = (usize, Attack, Option<Wielded>)> + 'm {
    let in_reach = distance <= melee_reach(formulas, fighter, monster);
    attacks(monster)
        .filter(move |&(_, attack)| in_reach && fighter.makes(formulas.monster_weapon, attack))
        .map(move |(slot, attack)| {
            (
                slot,
                attack,
                fighter.weapon(formulas.monster_weapon, slot, attack),
            )
        })
        .filter(move |&(_, attack, weapon)| {
            melee::reaches_from(formulas.monster_reach, attack, weapon, distance)
        })
}

/// The monster's first attack, unless its first slot is empty.
fn first_attack(monster: &Monster) -> Option<Attack> {
    monster.attacks.first().and_then(Attack::of)
}

/// The monster's attacks, each with its slot, in crawl's order.
fn attacks(monster: &Monster) -> impl Iterator<Item = (usize, Attack)> + '_ {
    monster
        .attacks
        .iter()
        .enumerate()
        .filter_map(|(slot, attack)| Some((slot, Attack::of(attack)?)))
}

fn hit_dice(monster: &Monster) -> i32 {
    i32::try_from(monster.hd).unwrap_or(i32::MAX)
}

fn speed(monster: &Monster) -> i32 {
    i32::try_from(monster.speed).unwrap_or(i32::MAX)
}

/// An action's energy cost, never zero (as `monster::action_energy`, `monster.cc:5897`).
fn cost(energy: u32) -> i32 {
    i32::try_from(energy).unwrap_or(i32::MAX).max(1)
}

#[cfg(test)]
mod tests {
    use copilot_gk::{ItemKind, Library};

    use super::*;
    use crate::carried_gear::CarriedItem;

    /// Two ogres beside a player on 15 HP who shows AC 6, EV 8 and SH 4: their true defences lie
    /// somewhere from exactly that to one point more, and so does their chance of death.
    #[test]
    fn the_hidden_fraction_puts_the_chance_of_death_between_the_displayed_defences_and_one_more() {
        let library = Library::shipped().unwrap();
        let bundle = library.for_version("0.34.1").unwrap();
        let formulas = bundle.formulas;
        let ogre = bundle.monsters.values().find(|m| m.name == "ogre").unwrap();
        let attackers = vec![Attacker::new(bundle, ogre, 1).without_gear(); 2];
        let hp = 15;
        let exact_deaths = |ac, ev, sh| {
            let mut rng = Rng::new(SEED.0, SEED.1);
            (0..TRIALS)
                .filter(|_| {
                    let target = Target {
                        defences: Defences::exact(ac, ev, sh),
                        lighting: Lighting::Plain,
                    };
                    one_action(formulas, &mut rng, target, 1, &attackers) >= hp
                })
                .count()
        };
        let defender = Defender {
            hp,
            defences: DisplayedDefences {
                ac: 6,
                ev: 8,
                sh: 4,
            },
            lighting: Lighting::Plain,
            shield_blocks: 1,
        };
        let displayed = usize::try_from(simulate(formulas, defender, &attackers).deaths).unwrap();
        let (most, fewest) = (exact_deaths(6, 8, 4), exact_deaths(7, 9, 5));
        assert!(
            fewest < displayed && displayed < most,
            "{fewest} < {displayed} < {most} deaths in {TRIALS}"
        );
    }

    /// The turns `name`, `distance` squares from an undefended player, plays out over 1,000
    /// trials, casting a spell of its book on every action it can.
    fn turns_always_casting(name: &str, distance: i32) -> Vec<Turn> {
        let library = Library::shipped().unwrap();
        let bundle = library.for_version("0.34.1").unwrap();
        let monster = bundle.monsters.values().find(|m| m.name == name).unwrap();
        let mut attacker = Attacker::new(bundle, monster, distance).without_gear();
        assert!(!attacker.spells.is_empty(), "{name} casts no ported spell");
        for cast in &mut attacker.spells {
            cast.slot.frequency = 200;
        }
        let target = Target {
            defences: Defences::exact(0, 0, 0),
            lighting: Lighting::Plain,
        };
        let mut rng = Rng::new(SEED.0, SEED.1);
        (0..1_000)
            .map(|_| turn(bundle.formulas, &mut rng, &attacker, target, &mut 0))
            .collect()
    }

    #[test]
    fn a_blinkbolt_lands_its_caster_beside_the_player_to_fight_on_its_next_action() {
        // A spark wasp, at speed 15, has a second action in some trials; its blinkbolt is only
        // cast from afar, so beside the player it stings.
        let turns = turns_always_casting("spark wasp", 7);
        assert!(turns.iter().all(|turn| turn.casts == 1 && turn.closed == 6));
        assert!(turns.iter().any(|turn| turn.melee == 1));
        assert!(turns.iter().all(|turn| turn.melee <= 1));
    }

    #[test]
    fn a_caster_of_any_other_bolt_keeps_its_distance() {
        // A shock serpent, at speed 15, casts its electrical bolt again from where it stood.
        let turns = turns_always_casting("shock serpent", 4);
        assert!(turns.iter().all(|turn| turn.closed == 0 && turn.melee == 0));
        assert!(turns.iter().any(|turn| turn.casts == 2));
    }

    /// The look text's listing of a monster wielding these weapons, each as
    /// `(name, weapon_type, plus, brand)`, the plus and brand as its name shows them.
    fn wielding(weapons: &[(&str, &str, Option<i32>, ItemBrand)]) -> CarriedGear {
        CarriedGear {
            items: weapons
                .iter()
                .map(|(name, weapon, plus, brand)| CarriedItem {
                    slot: GearSlot::Wielding,
                    name: (*name).to_owned(),
                    kind: Some(ItemKind {
                        class: EnumId(object_classes::WEAPONS.to_owned()),
                        sub_type: Some(EnumId((*weapon).to_owned())),
                    }),
                    plus: *plus,
                    brand: brand.clone(),
                })
                .collect(),
            unlisted: Unlisted::Nothing,
        }
    }

    /// A plain +0 weapon of `weapon_type`, named `name`.
    fn plain(name: &str, weapon: &str) -> CarriedGear {
        wielding(&[(name, weapon, Some(0), ItemBrand::None)])
    }

    /// The slots of the attacks `attacker` makes from `distance`, each with the damage of the
    /// weapon it makes it with, if any.
    fn attacks_with_weapons(attacker: &Attacker<'_>, distance: i32) -> Vec<(usize, Option<i32>)> {
        attacks_from(
            Formulas::CRAWL_0_34,
            &attacker.fighter,
            attacker.monster,
            distance,
        )
        .map(|(slot, _, weapon)| (slot, weapon.map(|wielded| wielded.weapon.damage)))
        .collect()
    }

    #[test]
    fn a_monster_wielding_two_weapons_makes_its_odd_attacks_with_its_off_hands() {
        let library = Library::shipped().unwrap();
        let bundle = library.for_version("0.34.1").unwrap();
        let ettin = bundle
            .monsters
            .values()
            .find(|m| m.name == "ettin")
            .unwrap();
        let gear = wielding(&[
            ("+0 spear", "WPN_SPEAR", Some(0), ItemBrand::None),
            ("+0 dagger", "WPN_DAGGER", Some(0), ItemBrand::None),
        ]);
        let (armed, unplayed) = Attacker::new(bundle, ettin, 1).carrying(bundle, &gear);
        assert!(unplayed.is_empty(), "{unplayed:?}");
        let spear = bundle.melee_weapons[&EnumId("WPN_SPEAR".to_owned())].damage;
        let dagger = bundle.melee_weapons[&EnumId("WPN_DAGGER".to_owned())].damage;
        assert_eq!(
            attacks_with_weapons(&armed, 1),
            [(0, Some(spear)), (1, Some(dagger))]
        );
        // The spear in its main hand reaches two squares; the dagger in its off hand doesn't.
        assert_eq!(attacks_with_weapons(&armed, 2), [(0, Some(spear))]);
    }

    #[test]
    fn a_pair_of_weapons_alike_is_held_one_in_each_hand() {
        let library = Library::shipped().unwrap();
        let bundle = library.for_version("0.34.1").unwrap();
        let ettin = bundle
            .monsters
            .values()
            .find(|m| m.name == "ettin")
            .unwrap();
        let gear = plain("two +0 spears", "WPN_SPEAR");
        let (armed, _) = Attacker::new(bundle, ettin, 2).carrying(bundle, &gear);
        let spear = bundle.melee_weapons[&EnumId("WPN_SPEAR".to_owned())].damage;
        assert_eq!(
            attacks_with_weapons(&armed, 2),
            [(0, Some(spear)), (1, Some(spear))]
        );
    }

    #[test]
    fn only_the_weapon_of_a_monsters_first_attack_sets_how_far_it_reaches() {
        // A spear in the ettin's off hand would reach two squares with its second attack, but
        // crawl's reach is its main hand's (`monster::reach_range`).
        let library = Library::shipped().unwrap();
        let bundle = library.for_version("0.34.1").unwrap();
        let ettin = bundle
            .monsters
            .values()
            .find(|m| m.name == "ettin")
            .unwrap();
        let gear = wielding(&[
            ("+0 dagger", "WPN_DAGGER", Some(0), ItemBrand::None),
            ("+0 spear", "WPN_SPEAR", Some(0), ItemBrand::None),
        ]);
        let (armed, _) = Attacker::new(bundle, ettin, 2).carrying(bundle, &gear);
        assert_eq!(melee_reach(Formulas::CRAWL_0_34, &armed.fighter, ettin), 1);
        assert!(attacks_with_weapons(&armed, 2).is_empty());
    }

    #[test]
    fn a_draugr_fights_with_its_weapon_only_in_its_first_attack() {
        // A draugr takes its attacks from the monster it was made from, which its Game Knowledge
        // doesn't hold: give it two hits.
        let library = Library::shipped().unwrap();
        let bundle = library.for_version("0.34.1").unwrap();
        let ettin = bundle
            .monsters
            .values()
            .find(|m| m.name == "ettin")
            .unwrap();
        let mut draugr = bundle
            .monsters
            .values()
            .find(|m| m.id.0 == monster_types::DRAUGR)
            .unwrap()
            .clone();
        draugr.attacks.clone_from(&ettin.attacks);
        draugr.item_use = ItemUse::StartingEquipment;
        let (armed, _) =
            Attacker::new(bundle, &draugr, 1).carrying(bundle, &plain("+0 spear", "WPN_SPEAR"));
        let spear = bundle.melee_weapons[&EnumId("WPN_SPEAR".to_owned())].damage;
        assert_eq!(
            attacks_with_weapons(&armed, 1),
            [(0, Some(spear)), (1, None)]
        );
        assert_eq!(attacks_with_weapons(&armed, 2), [(0, Some(spear))]);
    }

    #[test]
    fn a_monster_known_to_wield_nothing_makes_no_attack_only_a_weapon_makes() {
        let library = Library::shipped().unwrap();
        let bundle = library.for_version("0.34.1").unwrap();
        let statue = bundle
            .monsters
            .values()
            .find(|m| m.name == "statue")
            .unwrap();
        let unread = Attacker::new(bundle, statue, 1);
        assert_eq!(attacks_with_weapons(&unread, 1), [(0, None)]);
        assert!(unread.weapons_unread(Formulas::CRAWL_0_34));
        let empty_handed = CarriedGear {
            items: Vec::new(),
            unlisted: Unlisted::Nothing,
        };
        let (unarmed, _) = Attacker::new(bundle, statue, 1).carrying(bundle, &empty_handed);
        assert!(attacks_with_weapons(&unarmed, 1).is_empty());
        assert!(!unarmed.weapons_unread(Formulas::CRAWL_0_34));
        let (armed, _) =
            Attacker::new(bundle, statue, 1).carrying(bundle, &plain("+0 spear", "WPN_SPEAR"));
        let spear = bundle.melee_weapons[&EnumId("WPN_SPEAR".to_owned())].damage;
        assert_eq!(attacks_with_weapons(&armed, 2), [(0, Some(spear))]);
    }

    #[test]
    fn a_group_sighting_that_names_nothing_in_hand_leaves_the_weapon_unread() {
        // A group's sighting message leaves out a plain weapon in the main hand.
        let library = Library::shipped().unwrap();
        let bundle = library.for_version("0.34.1").unwrap();
        let orc = bundle.monsters.values().find(|m| m.name == "orc").unwrap();
        let unlisted = CarriedGear {
            items: Vec::new(),
            unlisted: Unlisted::PlainWeaponsAndLargeRocks,
        };
        let (attacker, _) = Attacker::new(bundle, orc, 1).carrying(bundle, &unlisted);
        assert!(attacker.weapons_unread(Formulas::CRAWL_0_34));
    }

    #[test]
    fn a_monster_known_to_wield_a_polearm_strikes_from_two_squares() {
        let library = Library::shipped().unwrap();
        let bundle = library.for_version("0.34.1").unwrap();
        let formulas = bundle.formulas;
        let orc = bundle.monsters.values().find(|m| m.name == "orc").unwrap();
        let target = Target {
            defences: Defences::exact(0, 0, 0),
            lighting: Lighting::Plain,
        };
        let turns = |attacker: &Attacker<'_>| {
            let mut rng = Rng::new(SEED.0, SEED.1);
            (0..1_000)
                .map(|_| turn(formulas, &mut rng, attacker, target, &mut 0))
                .collect::<Vec<_>>()
        };
        let unarmed = Attacker::new(bundle, orc, 2).without_gear();
        assert!(turns(&unarmed).iter().all(|turn| turn.closed == 1));
        let (armed, _) = Attacker::new(bundle, orc, 2)
            .without_gear()
            .carrying(bundle, &plain("+0 spear", "WPN_SPEAR"));
        let armed_turns = turns(&armed);
        assert!(armed_turns.iter().all(|turn| turn.closed == 0));
        assert!(armed_turns.iter().all(|turn| turn.melee >= 1));
    }

    #[test]
    fn a_weapons_unknown_plus_and_unported_brand_are_named_as_unplayed() {
        let library = Library::shipped().unwrap();
        let bundle = library.for_version("0.34.1").unwrap();
        let orc = bundle.monsters.values().find(|m| m.name == "orc").unwrap();
        let gear = wielding(&[(
            "spear of venom",
            "WPN_SPEAR",
            None,
            ItemBrand::Weapon(EnumId("SPWPN_VENOM".to_owned())),
        )]);
        let (_, unplayed) = Attacker::new(bundle, orc, 1).carrying(bundle, &gear);
        assert_eq!(
            unplayed,
            [
                Unplayed::UnknownPlus {
                    item: "spear of venom".to_owned()
                },
                Unplayed::BrandNotPorted {
                    item: "spear of venom".to_owned(),
                    brand: "venom".to_owned(),
                },
            ]
        );
    }
}

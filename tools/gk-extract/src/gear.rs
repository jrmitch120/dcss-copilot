//! What monsters are made with to shoot, throw or zap, from crawl's `mon-gear.cc`: the weapon
//! specs `make_mons_weapon` draws from, read from its tables, and the code around them, ported by
//! hand for each release series ([`GearRules`]). Also the launchers and missiles themselves, from
//! `Weapon_prop` and `Missile_prop` in `item-prop.cc`, the melee weapons monsters fight with,
//! from `Weapon_prop`, and the spell each wand zaps, from `_wand_spells` in `spl-book.cc`.
//!
//! A kind's gear is given as crawl's `give_item` gives it to a monster made on D:3 or deeper
//! (`level` at least 2), outside the Orcish Mines and not in a band, which is what
//! [`GearRules`] assume where crawl's code asks.

use std::collections::{BTreeMap, HashMap};

use copilot_gk::{
    EnumId, ItemUse, MeleeWeapon, MissileKind, MissileWeapon, MonsterGear, Odds, Wand, Weighted,
};

use crate::ExtractError;
use crate::cpp::{Source, Token, number, split_top_level};

/// The skill `Weapon_prop` gives launchers: crawl's `is_range_weapon`.
pub const LAUNCHER_SKILL: &str = "SK_RANGED_WEAPONS";

/// The skill `Weapon_prop` gives the retired blowgun, which shoots needles rather than striking in
/// melee, and whose enchantment adds nothing to its damage (`attack::get_weapon_plus`,
/// `attack.cc:815`): crawl's `SK_THROWING`.
pub const THROWING_SKILL: &str = "SK_THROWING";

/// The skill `Weapon_prop` gives polearms, which reach two squares (`weapon_reach`,
/// `item-prop.cc:2323`): crawl's `SK_POLEARMS`.
pub const POLEARM_SKILL: &str = "SK_POLEARMS";

/// How far a polearm reaches, and any other melee weapon that isn't an artefact
/// (`weapon_reach`).
const POLEARM_REACH: i32 = 2;
const MELEE_REACH: i32 = 1;

/// The weapon type a weapon spec picks for no weapon at all.
pub const NO_WEAPON: &str = "NUM_WEAPONS";

/// The `object_class_type` of wands, whose item sets are the ones read.
pub const WANDS: &str = "OBJ_WANDS";

/// The code around `mon-gear.cc`'s tables that decides a monster's missiles and wand, ported by
/// hand from one release series. The crawl oracle's gear cases check the outcome against crawl's
/// own `give_item` (`docs/crawl-oracle.md`).
pub struct GearRules {
    /// The cases of `make_mons_weapon`'s `switch (type)` that change whether a monster is made
    /// with a launcher or a throwing missile, overriding what its weapon specs gave.
    weapon_cases: &'static [WeaponCase],
    /// `_give_ammo`: the throwing missiles a monster without a launcher is given, by monster,
    /// each with its chance. A monster's chances are of one outcome each, so they sum to at
    /// most 1.
    ammo: &'static [(&'static str, &'static [(&'static str, Chance)])],
    /// `_give_wand` and what it calls.
    wands: WandRules,
}

/// One case of `make_mons_weapon`'s `switch (type)`: with `chance`, the monster's first weapon is
/// `item` instead of what its specs gave, or a melee weapon when `item` is `None`.
struct WeaponCase {
    monster: &'static str,
    item: Option<&'static str>,
    chance: Chance,
}

/// `_give_wand` (`mon-gear.cc`) and what it calls in `makeitem.cc`, `item-prop.cc` and
/// `monster.cc`.
struct WandRules {
    /// A unique without crawl's `no_wand` flag is given a wand this often (`one_chance_in(5)`).
    unique_chance: Chance,
    /// `_random_wand_subtype`: each wand, or each item set crawl picks one wand of per game, with
    /// its weight.
    weights: &'static [(WandPick, u64)],
    /// The wand `_give_wand` rerolls until it gets another.
    rerolled: &'static str,
    /// `is_high_tier_wand`: wands a monster below [`WandRules::high_tier_hd`] or with crawl's
    /// `no_ht_wand` flag refuses, unless it always has a wand.
    high_tier: &'static [&'static str],
    /// The hit dice from which a monster takes a high-tier wand.
    high_tier_hd: u32,
    /// `wand_charge_value` at item level 1, as `monster::likes_wand` calls it: each wand's
    /// value, and [`WandRules::default_charge`] for the rest.
    charge_values: &'static [(&'static str, u32)],
    /// See `charge_values`.
    default_charge: u32,
    /// `monster::likes_wand`: a monster uses a wand while its charge value plus this many per
    /// hit die stays within [`WandRules::liking_limit`].
    liking_per_hd: u32,
    /// See `liking_per_hd`.
    liking_limit: u32,
}

/// One choice of `_random_wand_subtype`.
enum WandPick {
    /// This wand.
    Wand(&'static str),
    /// The wand of the item set of this name, in `item_sets` (`item-prop.cc`), that the game
    /// picked: any of them alike (`initialise_item_sets`).
    Set(&'static str),
}

/// A chance as a fraction, `.0` in `.1`.
type Chance = (u64, u64);

/// Crawl 0.34's gear code, ported from 0.34.1.
pub const GEAR_RULES_0_34: GearRules = GearRules {
    weapon_cases: &[
        // A kobold's weapon is always a melee one.
        WeaponCase {
            monster: "MONS_KOBOLD",
            item: None,
            chance: (1, 1),
        },
        WeaponCase {
            monster: "MONS_KOBOLD_BRIGAND",
            item: None,
            chance: (1, 1),
        },
        // `one_chance_in(12) && level`.
        WeaponCase {
            monster: "MONS_GOBLIN",
            item: Some("WPN_SLING"),
            chance: (1, 12),
        },
        WeaponCase {
            monster: "MONS_CYCLOPS",
            item: Some("MI_LARGE_ROCK"),
            chance: (1, 1),
        },
        WeaponCase {
            monster: "MONS_STONE_GIANT",
            item: Some("MI_LARGE_ROCK"),
            chance: (1, 1),
        },
        WeaponCase {
            monster: "MONS_TAINTED_LEVIATHAN",
            item: Some("MI_LARGE_ROCK"),
            chance: (1, 1),
        },
        WeaponCase {
            monster: "MONS_PARGHIT",
            item: Some("MI_LARGE_ROCK"),
            chance: (1, 1),
        },
    ],
    ammo: &[
        // Darts `one_chance_in(10) && level > 1`, else stones `x_chance_in_y(2, 5)`.
        (
            "MONS_KOBOLD",
            &[("MI_DART", (1, 10)), ("MI_STONE", (9, 25))],
        ),
        ("MONS_KOBOLD_BRIGAND", &[("MI_DART", (1, 1))]),
        ("MONS_SONJA", &[("MI_DART", (1, 1))]),
        ("MONS_SPRIGGAN_RIDER", &[("MI_DART", (1, 15))]),
        // One in 20 outside the Orcish Mines.
        ("MONS_ORC_WARRIOR", &[("MI_BOOMERANG", (1, 20))]),
        ("MONS_ORC", &[("MI_BOOMERANG", (1, 20))]),
        ("MONS_URUG", &[("MI_JAVELIN", (1, 1))]),
        ("MONS_CHUCK", &[("MI_LARGE_ROCK", (1, 1))]),
        ("MONS_POLYPHEMUS", &[("MI_LARGE_ROCK", (1, 1))]),
        ("MONS_MERFOLK_JAVELINEER", &[("MI_JAVELIN", (1, 1))]),
        ("MONS_MINOTAUR", &[("MI_JAVELIN", (1, 1))]),
        // A boomerang one in 3, then a net one in 8 in place of whatever came before.
        (
            "MONS_MERFOLK",
            &[("MI_THROWING_NET", (1, 8)), ("MI_BOOMERANG", (7, 24))],
        ),
        // `!level || !one_chance_in(20)` gives none, then as Harold.
        ("MONS_DRACONIAN_KNIGHT", &[("MI_THROWING_NET", (1, 20))]),
        ("MONS_GNOLL", &[("MI_THROWING_NET", (1, 20))]),
        ("MONS_HAROLD", &[("MI_THROWING_NET", (1, 1))]),
    ],
    wands: WandRules {
        unique_chance: (1, 5),
        weights: &[
            (WandPick::Wand("WAND_FLAME"), 14),
            (WandPick::Set("blast wand"), 14),
            (WandPick::Set("hex wand"), 14),
            (WandPick::Set("beam wand"), 9),
            (WandPick::Wand("WAND_POLYMORPH"), 7),
            (WandPick::Wand("WAND_MINDBURST"), 7),
            (WandPick::Wand("WAND_DIGGING"), 5),
        ],
        rerolled: "WAND_DIGGING",
        high_tier: &[
            "WAND_CHARMING",
            "WAND_PARALYSIS",
            "WAND_ACID",
            "WAND_LIGHT",
            "WAND_QUICKSILVER",
            "WAND_ICEBLAST",
            "WAND_ROOTS",
            "WAND_MINDBURST",
        ],
        high_tier_hd: 5,
        charge_values: &[
            ("WAND_DIGGING", 9),
            ("WAND_ICEBLAST", 15),
            ("WAND_ROOTS", 15),
            ("WAND_WARPING", 15),
            ("WAND_ACID", 15),
            ("WAND_LIGHT", 15),
            ("WAND_QUICKSILVER", 15),
            ("WAND_CHARMING", 15),
            ("WAND_PARALYSIS", 15),
            ("WAND_POLYMORPH", 15),
            ("WAND_FLAME", 32),
        ],
        default_charge: 24,
        liking_per_hd: 6,
        liking_limit: 72,
    },
};

impl GearRules {
    /// The `monster_type` identifiers the rules name.
    pub fn monsters(&self) -> Vec<&'static str> {
        let cases = self.weapon_cases.iter().map(|case| case.monster);
        let ammo = self.ammo.iter().map(|&(monster, _)| monster);
        cases.chain(ammo).collect()
    }

    /// The launchers and throwing missiles the rules name, by `weapon_type` or `missile_type`
    /// identifier.
    pub fn missile_weapons(&self) -> Vec<&'static str> {
        let cases = self.weapon_cases.iter().filter_map(|case| case.item);
        let ammo = self
            .ammo
            .iter()
            .flat_map(|&(_, missiles)| missiles.iter().map(|&(missile, _)| missile));
        cases.chain(ammo).collect()
    }

    /// The `wand_type` identifiers the rules name.
    pub fn wands(&self) -> Vec<&'static str> {
        let wands = &self.wands;
        let picked = wands.weights.iter().filter_map(|(pick, _)| match pick {
            WandPick::Wand(wand) => Some(*wand),
            WandPick::Set(_) => None,
        });
        let charged = wands.charge_values.iter().map(|&(wand, _)| wand);
        picked
            .chain([wands.rerolled])
            .chain(wands.high_tier.iter().copied())
            .chain(charged)
            .collect()
    }
}

/// What the gear of a kind of monster is decided from, read from one crawl release.
pub struct Gear {
    rules: &'static GearRules,
    /// Weapon types `make_mons_weapon` draws a monster's first weapon from when it has no
    /// secondary spec or that picks none, by monster: each type and its weight.
    primary: HashMap<String, Vec<(String, u64)>>,
    /// Weapon types it draws from first, usually ranged, by monster.
    secondary: HashMap<String, Vec<(String, u64)>>,
    /// Every launcher and throwing missile, by sub-type identifier.
    missile_weapons: BTreeMap<EnumId, MissileWeapon>,
    /// Each item set of wands, by name: its wands.
    wand_sets: HashMap<String, Vec<String>>,
}

/// A monster's fields its gear depends on.
pub struct GearOf<'a> {
    /// Its `monster_type` identifier.
    pub id: &'a str,
    /// Its hit dice.
    pub hd: u32,
    /// Which items it uses: only a monster that uses its starting equipment is given any.
    pub item_use: ItemUse,
    /// Its `dat/mons` flags.
    pub flags: &'a [String],
}

impl Gear {
    /// The gear rules `rules`, with the tables they read from `mon-gear.cc` and `item-prop.cc`.
    pub fn read(
        rules: &'static GearRules,
        mon_gear: &Source,
        item_prop: &Source,
        missile_weapons: BTreeMap<EnumId, MissileWeapon>,
    ) -> Result<Self, ExtractError> {
        Ok(Self {
            rules,
            primary: weapon_specs(mon_gear, "mon-gear.cc", "primary_weapon_specs")?,
            secondary: weapon_specs(mon_gear, "mon-gear.cc", "secondary_weapon_specs")?,
            missile_weapons,
            wand_sets: wand_sets(item_prop, "item-prop.cc")?,
        })
    }

    /// The gear crawl makes a monster of this kind with.
    pub fn of(&self, monster: &GearOf<'_>) -> Result<MonsterGear, ExtractError> {
        if monster.item_use < ItemUse::StartingEquipment {
            return Ok(MonsterGear {
                missile: Odds::none(),
                wand: Odds::none(),
            });
        }
        Ok(MonsterGear {
            missile: self.missile(monster.id)?,
            wand: self.wand(monster)?,
        })
    }

    /// `_give_weapon`'s first weapon, when it is a launcher or throwing missile, then
    /// `_give_ammo` for a monster left without a launcher.
    fn missile(&self, id: &str) -> Result<Odds, ExtractError> {
        let mut odds: BTreeMap<String, Ratio> = BTreeMap::new();
        // The secondary spec first; on none, the primary spec (`make_mons_weapon`).
        let mut primary_share = Ratio::ONE;
        if let Some(types) = self.secondary.get(id) {
            let total: u64 = types.iter().map(|(_, weight)| weight).sum();
            primary_share = Ratio::new(0, 1);
            for (weapon, weight) in types {
                let share = Ratio::new(*weight, total);
                if weapon == NO_WEAPON {
                    primary_share = primary_share.add(share);
                } else if self.is_missile_weapon(weapon) {
                    add(&mut odds, weapon, share);
                }
            }
        }
        if let Some(types) = self.primary.get(id) {
            let total: u64 = types.iter().map(|(_, weight)| weight).sum();
            for (weapon, weight) in types {
                if weapon != NO_WEAPON && self.is_missile_weapon(weapon) {
                    add(
                        &mut odds,
                        weapon,
                        primary_share.mul(Ratio::new(*weight, total)),
                    );
                }
            }
        }
        for case in self
            .rules
            .weapon_cases
            .iter()
            .filter(|case| case.monster == id)
        {
            let kept = Ratio::ONE.sub(Ratio::new(case.chance.0, case.chance.1));
            for share in odds.values_mut() {
                *share = share.mul(kept);
            }
            if let Some(item) = case.item {
                self.missile_weapon(item)?;
                add(&mut odds, item, Ratio::new(case.chance.0, case.chance.1));
            }
        }
        // `_give_ammo` gives nothing to a monster with a launcher.
        let mut launchers = Ratio::new(0, 1);
        let mut thrown = false;
        for (item, share) in &odds {
            match self.missile_weapon(item)?.kind {
                MissileKind::Launcher => launchers = launchers.add(*share),
                MissileKind::Thrown => thrown = true,
            }
        }
        if let Some((_, ammo)) = self.rules.ammo.iter().find(|(monster, _)| *monster == id) {
            if thrown {
                // `_give_ammo` would give a second throwing missile, which the bundle can't say.
                return Err(ExtractError::Unreadable {
                    file: "mon-gear.cc".to_owned(),
                    what: format!("the missiles of {id}, given both as a weapon and as ammunition"),
                });
            }
            let without_launcher = Ratio::ONE.sub(launchers);
            for (item, chance) in *ammo {
                self.missile_weapon(item)?;
                add(
                    &mut odds,
                    item,
                    without_launcher.mul(Ratio::new(chance.0, chance.1)),
                );
            }
        }
        Ok(odds_of(odds))
    }

    /// `_give_wand`: the wand a monster is made with, when it will zap it.
    fn wand(&self, monster: &GearOf<'_>) -> Result<Odds, ExtractError> {
        let rules = &self.rules.wands;
        let flag = |name: &str| monster.flags.iter().any(|flag| flag == name);
        let always = flag("always_wand");
        let chance = if always {
            Ratio::ONE
        } else if flag("unique") && !flag("no_wand") {
            Ratio::new(rules.unique_chance.0, rules.unique_chance.1)
        } else {
            return Ok(Odds::none());
        };
        // `_random_wand_subtype`, rerolled until it isn't the rerolled wand.
        let mut weights: Vec<(String, Ratio)> = Vec::new();
        for (pick, weight) in rules.weights {
            match pick {
                WandPick::Wand(wand) => weights.push(((*wand).to_owned(), Ratio::new(*weight, 1))),
                WandPick::Set(set) => {
                    let wands = self
                        .wand_sets
                        .get(*set)
                        .ok_or_else(|| ExtractError::Missing {
                            file: "item-prop.cc".to_owned(),
                            what: format!("the item set {set}"),
                        })?;
                    let each = Ratio::new(*weight, u64::try_from(wands.len()).unwrap_or(1));
                    weights.extend(wands.iter().map(|wand| (wand.clone(), each)));
                }
            }
        }
        weights.retain(|(wand, _)| wand != rules.rerolled);
        let total = weights
            .iter()
            .fold(Ratio::new(0, 1), |sum, (_, weight)| sum.add(*weight));
        let charge = |wand: &str| {
            rules
                .charge_values
                .iter()
                .find(|(known, _)| *known == wand)
                .map_or(rules.default_charge, |&(_, value)| value)
        };
        // `monster::likes_wand`: it zaps only a wand it likes, and refuses one it doesn't unless
        // it always has a wand, so either way it zaps only those.
        let likes =
            |wand: &str| charge(wand) + monster.hd * rules.liking_per_hd <= rules.liking_limit;
        let refuses_high_tier = monster.hd < rules.high_tier_hd || flag("no_ht_wand");
        let mut odds = BTreeMap::new();
        for (wand, weight) in weights {
            let refused = !always && refuses_high_tier && rules.high_tier.contains(&wand.as_str());
            if refused || !likes(&wand) {
                continue;
            }
            add(&mut odds, &wand, chance.mul(weight.div(total)));
        }
        Ok(odds_of(odds))
    }

    fn missile_weapon(&self, item: &str) -> Result<&MissileWeapon, ExtractError> {
        self.missile_weapons
            .get(&EnumId(item.to_owned()))
            .ok_or_else(|| ExtractError::Missing {
                file: "item-prop.cc".to_owned(),
                what: format!("{item}, a monster's missile weapon"),
            })
    }

    fn is_missile_weapon(&self, item: &str) -> bool {
        self.missile_weapons.contains_key(&EnumId(item.to_owned()))
    }
}

/// Every launcher in `Weapon_prop` and throwing missile in `Missile_prop` (`item-prop.cc`).
///
/// A `Weapon_prop` entry's fields are, in order: identifier, name, damage, accuracy, speed and
/// skill (`struct weapon_def`); a `Missile_prop` entry's: identifier, name, damage, mulch rate
/// and price (`struct missile_def`). A launcher keeps its speed, a thrown missile its mulch
/// rate.
pub fn missile_weapons(
    item_prop: &Source,
    file: &str,
) -> Result<BTreeMap<EnumId, MissileWeapon>, ExtractError> {
    let mut weapons = BTreeMap::new();
    for fields in item_prop.entries("Weapon_prop")? {
        let [id, name, damage, accuracy, speed, skill, ..] = fields.as_slice() else {
            return Err(unreadable(file, "an entry of Weapon_prop"));
        };
        if identifier(skill) != Some(LAUNCHER_SKILL) {
            continue;
        }
        let (id, name) = id_and_name(id, name).ok_or_else(|| unreadable(file, "a launcher"))?;
        let what = format!("the damage, accuracy or speed of {id}");
        weapons.insert(
            EnumId(id.to_owned()),
            MissileWeapon {
                name: name.to_owned(),
                kind: MissileKind::Launcher,
                damage: integer(damage).ok_or_else(|| unreadable(file, &what))?,
                accuracy: integer(accuracy).ok_or_else(|| unreadable(file, &what))?,
                delay: Some(integer(speed).ok_or_else(|| unreadable(file, &what))?),
                mulch: None,
            },
        );
    }
    for fields in item_prop.entries("Missile_prop")? {
        let [id, name, damage, mulch, ..] = fields.as_slice() else {
            return Err(unreadable(file, "an entry of Missile_prop"));
        };
        let (id, name) = id_and_name(id, name).ok_or_else(|| unreadable(file, "a missile"))?;
        weapons.insert(
            EnumId(id.to_owned()),
            MissileWeapon {
                name: name.to_owned(),
                kind: MissileKind::Thrown,
                damage: integer(damage)
                    .ok_or_else(|| unreadable(file, &format!("the damage of {id}")))?,
                accuracy: 0,
                delay: None,
                mulch: Some(
                    integer(mulch)
                        .ok_or_else(|| unreadable(file, &format!("the mulch rate of {id}")))?,
                ),
            },
        );
    }
    Ok(weapons)
}

/// Every melee weapon in `Weapon_prop` (`item-prop.cc`): those trained by neither
/// [`LAUNCHER_SKILL`] nor [`THROWING_SKILL`], with their damage, accuracy, reach, skill and
/// speed. The entry's fields are read as [`missile_weapons`] reads them.
pub fn melee_weapons(
    item_prop: &Source,
    file: &str,
) -> Result<BTreeMap<EnumId, MeleeWeapon>, ExtractError> {
    let mut weapons = BTreeMap::new();
    for fields in item_prop.entries("Weapon_prop")? {
        let [id, name, damage, accuracy, speed, skill, ..] = fields.as_slice() else {
            return Err(unreadable(file, "an entry of Weapon_prop"));
        };
        let skill = identifier(skill).ok_or_else(|| unreadable(file, "a weapon's skill"))?;
        if skill == LAUNCHER_SKILL || skill == THROWING_SKILL {
            continue;
        }
        let (id, name) = id_and_name(id, name).ok_or_else(|| unreadable(file, "a weapon"))?;
        let what = format!("the damage, accuracy or speed of {id}");
        weapons.insert(
            EnumId(id.to_owned()),
            MeleeWeapon {
                name: name.to_owned(),
                damage: integer(damage).ok_or_else(|| unreadable(file, &what))?,
                accuracy: integer(accuracy).ok_or_else(|| unreadable(file, &what))?,
                reach: if skill == POLEARM_SKILL {
                    POLEARM_REACH
                } else {
                    MELEE_REACH
                },
                skill: EnumId(skill.to_owned()),
                delay: integer(speed).ok_or_else(|| unreadable(file, &what))?,
            },
        );
    }
    Ok(weapons)
}

/// Every wand in `_wand_spells` (`spl-book.cc`), named by `names`, with the spell it zaps.
pub fn wands(
    spl_book: &Source,
    file: &str,
    names: &BTreeMap<String, String>,
) -> Result<BTreeMap<EnumId, Wand>, ExtractError> {
    let mut wands = BTreeMap::new();
    for fields in spl_book.entries("_wand_spells")? {
        let [wand, spell] = fields.as_slice() else {
            return Err(unreadable(file, "an entry of _wand_spells"));
        };
        let (Some(wand), Some(spell)) = (identifier(wand), identifier(spell)) else {
            return Err(unreadable(file, "an entry of _wand_spells"));
        };
        let name = names.get(wand).ok_or_else(|| ExtractError::Missing {
            file: "item-name.cc".to_owned(),
            what: format!("the name of {wand}"),
        })?;
        wands.insert(
            EnumId(wand.to_owned()),
            Wand {
                name: name.clone(),
                spell: EnumId(spell.to_owned()),
            },
        );
    }
    Ok(wands)
}

/// The weapon types each monster's spec in the map `<name>` (`make_mons_weapon`) picks from,
/// with their weights: `{ MONS_X, spec }`, where a spec is a named `mon_weapon_spec` or
/// `{ types, plusses, brands, good chance }`, and its types a named `weapon_list` or
/// `{ { WPN_X, weight }, ... }`.
fn weapon_specs(
    source: &Source,
    file: &str,
    name: &str,
) -> Result<HashMap<String, Vec<(String, u64)>>, ExtractError> {
    let mut specs = HashMap::new();
    for entry in split_top_level(source.initializer(name)?) {
        let what = || unreadable(file, &format!("an entry of {name}"));
        let fields = braced_fields(&entry).ok_or_else(what)?;
        let [monster, spec] = fields.as_slice() else {
            return Err(what());
        };
        let monster = identifier(monster).ok_or_else(what)?;
        let spec_fields = match identifier(spec) {
            Some(named) => split_top_level(source.initializer(named)?),
            None => braced_fields(spec).ok_or_else(what)?,
        };
        let types = spec_fields.first().ok_or_else(what)?;
        let list = match identifier(types) {
            Some(named) => split_top_level(source.initializer(named)?),
            None => braced_fields(types).ok_or_else(what)?,
        };
        let types = list
            .iter()
            .map(|pair| match braced_fields(pair)?.as_slice() {
                [weapon, weight] => Some((
                    identifier(weapon)?.to_owned(),
                    u64::try_from(integer(weight)?).ok()?,
                )),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| unreadable(file, &format!("the weapons of {monster} in {name}")))?;
        specs.insert(monster.to_owned(), types);
    }
    Ok(specs)
}

/// Each item set of wands in `item_sets` (`item-prop.cc`), by name: `{ "name", OBJ_WANDS,
/// { WAND_X, ... } }`.
fn wand_sets(item_prop: &Source, file: &str) -> Result<HashMap<String, Vec<String>>, ExtractError> {
    let mut sets = HashMap::new();
    for fields in item_prop.entries("item_sets")? {
        let what = || unreadable(file, "an entry of item_sets");
        let [name, class, members] = fields.as_slice() else {
            return Err(what());
        };
        let ([Token::Str(name)], Some(class)) = (name.as_slice(), identifier(class)) else {
            return Err(what());
        };
        if class != WANDS {
            continue;
        }
        let members = braced_fields(members)
            .ok_or_else(what)?
            .iter()
            .map(|member| identifier(member).map(str::to_owned))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(what)?;
        sets.insert(name.clone(), members);
    }
    Ok(sets)
}

/// The top-level fields of `{ ... }`.
fn braced_fields(tokens: &[Token]) -> Option<Vec<Vec<Token>>> {
    match tokens {
        [Token::Punct('{'), inner @ .., Token::Punct('}')] => Some(split_top_level(inner)),
        _ => None,
    }
}

fn id_and_name<'a>(id: &'a [Token], name: &'a [Token]) -> Option<(&'a str, &'a str)> {
    match (identifier(id), name) {
        (Some(id), [Token::Str(name)]) => Some((id, name)),
        _ => None,
    }
}

fn identifier(tokens: &[Token]) -> Option<&str> {
    match tokens {
        [Token::Ident(ident)] => Some(ident),
        _ => None,
    }
}

/// A literal integer, or a negative one.
fn integer(tokens: &[Token]) -> Option<i32> {
    let value = match tokens {
        [Token::Number(n)] => number(n),
        [Token::Punct('-'), Token::Number(n)] => number(n).map(|n| -n),
        _ => None,
    }?;
    i32::try_from(value).ok()
}

fn unreadable(file: &str, what: &str) -> ExtractError {
    ExtractError::Unreadable {
        file: file.to_owned(),
        what: what.to_owned(),
    }
}

fn add(odds: &mut BTreeMap<String, Ratio>, item: &str, share: Ratio) {
    let entry = odds.entry(item.to_owned()).or_insert(Ratio::new(0, 1));
    *entry = entry.add(share);
}

/// `odds` over their least common denominator, leaving out items with no chance.
fn odds_of(odds: BTreeMap<String, Ratio>) -> Odds {
    let odds: Vec<(String, Ratio)> = odds
        .into_iter()
        .filter(|(_, share)| share.num > 0)
        .collect();
    let out_of = odds
        .iter()
        .fold(1, |lcm, (_, share)| lcm / gcd(lcm, share.den) * share.den);
    Odds {
        out_of: u32::try_from(out_of).unwrap_or(u32::MAX),
        items: odds
            .into_iter()
            .map(|(item, share)| Weighted {
                item: EnumId(item),
                weight: u32::try_from(share.num * (out_of / share.den)).unwrap_or(u32::MAX),
            })
            .collect(),
    }
}

/// An exact chance, `num / den` in lowest terms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Ratio {
    num: u64,
    den: u64,
}

impl Ratio {
    const ONE: Self = Self { num: 1, den: 1 };

    fn new(num: u64, den: u64) -> Self {
        let divisor = gcd(num, den).max(1);
        Self {
            num: num / divisor,
            den: (den / divisor).max(1),
        }
    }

    fn add(self, other: Self) -> Self {
        Self::new(
            self.num * other.den + other.num * self.den,
            self.den * other.den,
        )
    }

    fn sub(self, other: Self) -> Self {
        Self::new(
            (self.num * other.den).saturating_sub(other.num * self.den),
            self.den * other.den,
        )
    }

    fn mul(self, other: Self) -> Self {
        Self::new(self.num * other.num, self.den * other.den)
    }

    fn div(self, other: Self) -> Self {
        Self::new(self.num * other.den, self.den * other.num.max(1))
    }
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { gcd(b, a % b) }
}

//! The Fight Outcome: for a small fight, each Attack Plan the player could fight by, played out
//! many times against the hostiles in view, with its chance to win and the hit points it costs.

use std::collections::BTreeSet;

use copilot_gk::{Bundle, EnumId, ItemUse, MissileKind};
use serde::Serialize;

use crate::accumulated_state::{AccumulatedState, Remembered};
use crate::carried_gear::{self, ItemBrand};
use crate::crawl_ids::{missile_types, object_classes, skill_types};
use crate::engine::fight::{self, Base, Fighter, Foe, Opening, Power};
use crate::engine::player_attacks::{AttackSpell, Defence, Heft, PlayerZap, Skill};
use crate::engine::step::Offset;
use crate::engine::threat::{Attacker, Defender};
use crate::fact_sheet::{Chance, NotModelled};
use crate::gaps::{self, Gap, Reader, ScreenFact, memorised_unnamed};
use crate::screen::{KnownSpell, Skills, SpellPower, SpellReach};
use crate::view_model::{FocusCard, Listing, MarkedValue, capitalised};
use crate::visible_state::{InventoryItem, SquareLight, VisibleState, Wielding};

/// The most hostiles in view the Fight Outcome plays out: a small fight, about three monsters or
/// fewer (spec #24, "Engine (ADR 0003)"). A bigger fight isn't judged.
const MOST_HOSTILES: usize = 3;

/// Whether the Advisor judged a fight with the hostiles in view, and how it came out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Fight {
    /// Nothing in view to fight.
    NoFight,
    /// More hostiles are in view than the Fight Outcome plays out.
    TooMany {
        /// How many.
        hostiles: u32,
    },
    /// The Advisor lacks facts it needs to play the fight out.
    Unjudged {
        /// What it lacks.
        gaps: Vec<Gap>,
    },
    /// The Advisor played the fight out.
    Judged(Box<FightOutcome>),
}

impl Fight {
    /// The Gaps that kept it from being judged, or that may hide a better plan.
    pub(crate) fn gaps(&self) -> &[Gap] {
        match self {
            Self::Judged(outcome) => &outcome.gaps,
            Self::Unjudged { gaps } => gaps,
            Self::NoFight | Self::TooMany { .. } => &[],
        }
    }

    /// `text`, said of this fight, marked with the Gaps that keep it from being judged or may
    /// hide a better plan, as its card words them for `reader`.
    pub(crate) fn marked(&self, text: String, reader: Reader) -> MarkedValue {
        MarkedValue::marked(text, self.gaps().iter().map(|gap| fight_gap(gap, reader)))
    }

    /// The best plan's outcome, when the fight was judged.
    #[must_use]
    pub fn best(&self) -> Option<&PlanOutcome> {
        match self {
            Self::Judged(outcome) => outcome.plans.first(),
            Self::NoFight | Self::TooMany { .. } | Self::Unjudged { .. } => None,
        }
    }
}

/// Each Attack Plan played out against the hostiles in view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FightOutcome {
    /// The player's hit points the fights started from.
    pub hp: i32,
    /// Each plan, the best first: the likeliest to win, then the one costing the fewest hit
    /// points.
    pub plans: Vec<PlanOutcome>,
    /// The player's spells and wands no plan uses, by name: those the engine doesn't port as
    /// attacks, such as a hex, a summons or Blink, and those it can't play as the game shows
    /// them, such as a wand whose charges aren't shown.
    pub not_played: Vec<String>,
    /// Facts the Advisor lacks that may hide a better plan, such as the spell list unread.
    pub gaps: Vec<Gap>,
    /// What the fights leave out, so the numbers are read for what they are. The hostiles fight
    /// as the Threat Level plays them, and what it leaves out it leaves out here too.
    pub not_modelled: Vec<NotModelled>,
}

/// One Attack Plan, played out many times.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanOutcome {
    /// The plan.
    pub plan: AttackPlan,
    /// The chance every hostile falls before the player does, within the longest fight played
    /// out.
    pub win: Chance,
    /// The chance the player dies.
    pub death: Chance,
    /// The hit points the player loses, on average: all they have in a fight they die in.
    pub hp_lost: i32,
}

impl PlanOutcome {
    /// The plan and its odds as one line, against the `hp` its fights started from, such as
    /// `Melee: ~60% win, ~70% HP lost`.
    pub(crate) fn summary(&self, hp: i32) -> String {
        format!("{}: {}", capitalised(&self.plan.name()), odds(self, hp))
    }

    /// The hit points lost as a whole percentage of those the fights started from.
    #[must_use]
    pub fn hp_lost_percent(&self, hp: i32) -> i32 {
        if hp <= 0 {
            return 100;
        }
        (self.hp_lost * 100 + hp / 2) / hp
    }
}

/// A fixed way to fight: open with one attack while it lasts, then fall back on what the player
/// holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttackPlan {
    /// What it opens with, if anything other than its base attack.
    pub opening: Option<PlanOpening>,
    /// What it attacks with once the opening runs out, or throughout.
    pub base: PlanBase,
}

/// What an Attack Plan opens with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "with",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PlanOpening {
    /// A spell, while there is the magic for it.
    Spell {
        /// Its name, such as `Fireball`.
        name: String,
    },
    /// A wand, while it has charges.
    Wand {
        /// Its name, such as `wand of flame`.
        name: String,
        /// Its charges.
        charges: u32,
    },
    /// Thrown missiles, while there are any.
    Thrown {
        /// Their name, such as `javelins`.
        name: String,
        /// How many.
        count: u32,
    },
}

/// What an Attack Plan falls back on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "with",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PlanBase {
    /// Melee, with the weapon held or unarmed.
    Melee,
    /// The launcher held.
    Launcher {
        /// Its name, such as `shortbow`.
        name: String,
    },
    /// Nothing the engine can attack with: the player waits.
    Nothing,
}

impl AttackPlan {
    /// The plan in words, such as `Fireball, then melee`.
    #[must_use]
    pub fn name(&self) -> String {
        let base = match &self.base {
            PlanBase::Melee => "melee",
            PlanBase::Launcher { name } => name,
            PlanBase::Nothing => "nothing",
        };
        match &self.opening {
            None => base.to_owned(),
            Some(
                PlanOpening::Spell { name }
                | PlanOpening::Wand { name, .. }
                | PlanOpening::Thrown { name, .. },
            ) => format!("{name}, then {base}"),
        }
    }
}

/// What the Fight Outcome leaves out, whatever is in view.
const ALWAYS_NOT_MODELLED: [NotModelled; 10] = [
    NotModelled::Summons,
    NotModelled::Allies,
    NotModelled::Hexes,
    NotModelled::SelfBuffs,
    NotModelled::MixingWands,
    NotModelled::MovingBetweenAttacks,
    NotModelled::MonsterWounds,
    NotModelled::OwnAttackExtras,
    NotModelled::OwnFractions,
    NotModelled::PlayerSpeed,
];

/// The fight with the hostiles in view, `hostiles`, each with the gear the Threat Level plays it
/// with, by the player as `visible` shows them and the Advisor remembers them, defended as
/// `defender`: `None` with no hostile in view, and the Gaps that kept the Threat Level from being
/// judged.
pub(crate) fn judge<'a>(
    visible: &VisibleState,
    remembered: Option<&AccumulatedState>,
    bundle: Option<&'a Bundle>,
    defender: Result<Option<Defender>, Vec<Gap>>,
    hostiles: Vec<Attacker<'a>>,
) -> Fight {
    let defender = match defender {
        Ok(Some(defender)) => defender,
        Ok(None) => return Fight::NoFight,
        Err(gaps) => return Fight::Unjudged { gaps },
    };
    let foes: Vec<Attacker<'a>> = hostiles
        .into_iter()
        .filter(|attacker| !attacker.monster.firewood)
        .collect();
    let (Some(bundle), false) = (bundle, foes.is_empty()) else {
        return Fight::NoFight;
    };
    if foes.len() > MOST_HOSTILES {
        return Fight::TooMany {
            hostiles: u32::try_from(foes.len()).unwrap_or(u32::MAX),
        };
    }
    let player = match Player::read(visible, remembered) {
        Ok(player) => player,
        Err(gaps) => return Fight::Unjudged { gaps },
    };
    let mut not_modelled: BTreeSet<NotModelled> = ALWAYS_NOT_MODELLED.into_iter().collect();
    let mut gaps = skills_out_of_date(visible, remembered, bundle);
    let (base, base_plan) = player.base(bundle, &mut not_modelled);
    let fighter = Fighter {
        hp: defender.hp,
        mp: player.mp,
        strength: player.strength,
        dexterity: player.dexterity,
        fighting: player.skill(bundle, skill_types::FIGHTING),
        evocations: player.skill(bundle, skill_types::EVOCATIONS),
        base,
    };
    if foes.iter().any(|foe| {
        matches!(
            foe.monster.item_use,
            ItemUse::StartingEquipment | ItemUse::WeaponsArmour
        )
    }) {
        not_modelled.insert(NotModelled::MonsterArmour);
    }
    if foes_in_light(visible, &foes) {
        not_modelled.insert(NotModelled::MonsterLighting);
    }
    let mut not_played = Vec::new();
    let mut openings: Vec<(PlanOpening, Opening<'a>)> = player.spells(
        remembered,
        bundle,
        &mut gaps,
        &mut not_played,
        &mut not_modelled,
    );
    openings.extend(player.wands(bundle, &mut not_played));
    openings.extend(player.thrown(bundle));
    if openings
        .iter()
        .any(|(_, opening)| opening_blasts_or_pierces(opening))
    {
        not_modelled.insert(NotModelled::BlastsAndBolts);
    }
    let foes: Vec<Foe<'a>> = foes
        .into_iter()
        .map(|attacker| Foe {
            defence: Defence::of(bundle.formulas.player_beam, attacker.monster),
            attacker,
        })
        .collect();
    let formulas = bundle.formulas;
    let plan_outcome = |opening: Option<PlanOpening>, played: Option<&Opening<'_>>| {
        let outcome = fight::simulate(formulas, defender, &fighter, played, &foes);
        PlanOutcome {
            plan: AttackPlan {
                opening,
                base: base_plan.clone(),
            },
            win: Chance {
                times: outcome.wins,
                trials: outcome.trials,
            },
            death: Chance {
                times: outcome.deaths,
                trials: outcome.trials,
            },
            hp_lost: mean(outcome.hp_lost, outcome.trials),
        }
    };
    let mut plans = Vec::new();
    if base_plan != PlanBase::Nothing {
        plans.push(plan_outcome(None, None));
    }
    for (plan, opening) in &openings {
        plans.push(plan_outcome(Some(plan.clone()), Some(opening)));
    }
    plans.sort_by(|a, b| {
        b.win
            .times
            .cmp(&a.win.times)
            .then(a.hp_lost.cmp(&b.hp_lost))
    });
    Fight::Judged(Box::new(FightOutcome {
        hp: defender.hp,
        plans,
        not_played,
        gaps: gaps.into_iter().collect(),
        not_modelled: not_modelled.into_iter().collect(),
    }))
}

/// `total` over `trials`, rounded to the nearest whole number.
fn mean(total: u64, trials: u32) -> i32 {
    if trials == 0 {
        return 0;
    }
    let trials = u64::from(trials);
    i32::try_from((total + trials / 2) / trials).unwrap_or(i32::MAX)
}

/// Whether a hostile stands in a halo or an umbra, as the map shows it, which changes how well
/// the player aims at it.
fn foes_in_light(visible: &VisibleState, foes: &[Attacker<'_>]) -> bool {
    let (Some(map), Some(monsters), Some(player)) =
        (&visible.map, &visible.monsters, visible.position)
    else {
        return false;
    };
    foes.iter().any(|foe| {
        monsters
            .iter()
            .filter(|monster| monster.id.as_ref() == Some(&foe.monster.id))
            .filter(|monster| Offset::between(player, monster.position) == foe.offset)
            .any(|monster| {
                map.cell(monster.position)
                    .is_some_and(|cell| cell.light != SquareLight::Plain)
            })
    })
}

/// Whether `opening` fires an explosion or a beam that passes through what it hits, which may
/// strike more than its target.
fn opening_blasts_or_pierces(opening: &Opening<'_>) -> bool {
    match opening {
        Opening::Spell { spell, .. } => spell.blasts_or_pierces(),
        Opening::Wand { zap, .. } => zap.blasts_or_pierces(),
        Opening::Thrown { id, .. } => id.0 == missile_types::JAVELIN,
    }
}

/// What may keep the skills played from being the player's own: a skills screen read before
/// something that may have changed them, or a god who shifts them unsaid.
fn skills_out_of_date(
    visible: &VisibleState,
    remembered: Option<&AccumulatedState>,
    bundle: &Bundle,
) -> BTreeSet<Gap> {
    // The fight is played only once the skills are read: a Gap here is their being Stale.
    let mut gaps: BTreeSet<Gap> = remembered
        .and_then(|state| ScreenFact::Skills.gap(&state.skills))
        .into_iter()
        .collect();
    gaps.extend(gaps::skills_shifting(visible, bundle));
    gaps
}

/// The player as the Fight Outcome reads them.
struct Player<'v> {
    strength: i32,
    dexterity: i32,
    mp: i32,
    mp_max: i32,
    skills: &'v Skills,
    inventory: &'v [InventoryItem],
    wielding: Wielding,
}

impl<'v> Player<'v> {
    /// The player as `visible` shows them and the Advisor remembers them, or the Gaps that keep
    /// the Advisor from playing their attacks.
    fn read(
        visible: &'v VisibleState,
        remembered: Option<&'v AccumulatedState>,
    ) -> Result<Self, Vec<Gap>> {
        let mut gaps = BTreeSet::new();
        let stats = &visible.stats;
        if stats.strength.is_none()
            || stats.dexterity.is_none()
            || stats.mp.is_none()
            || stats.mp_max.is_none()
        {
            gaps.insert(Gap::Attributes);
        }
        let skills = remembered.and_then(|state| state.skills.read_value());
        if skills.is_none() {
            gaps.insert(Gap::Unread {
                fact: ScreenFact::Skills,
            });
        }
        if visible.inventory.is_none() {
            gaps.insert(Gap::Inventory);
        }
        if visible.wielded.is_none() {
            gaps.insert(Gap::Wielded);
        }
        match (
            (stats.strength, stats.dexterity, stats.mp, stats.mp_max),
            skills,
            visible.inventory.as_deref(),
            visible.wielded,
        ) {
            (
                (Some(strength), Some(dexterity), Some(mp), Some(mp_max)),
                Some(skills),
                Some(inventory),
                Some(wielding),
            ) if gaps.is_empty() => Ok(Self {
                strength,
                dexterity,
                mp,
                mp_max,
                skills,
                inventory,
                wielding,
            }),
            _ => Err(gaps.into_iter().collect()),
        }
    }

    /// The player's skill of `id`, a `skill_type` identifier, as the skills screen showed it: 0
    /// when it showed none, or the bundle doesn't name it.
    fn skill(&self, bundle: &Bundle, id: &str) -> Skill {
        let tenths = bundle
            .skills
            .get(&EnumId(id.to_owned()))
            .map_or(0, |name| self.skills.tenths(name));
        Skill::shown(tenths)
    }

    /// The item carried under `letter`.
    fn carried(&self, letter: char) -> Option<&'v InventoryItem> {
        self.inventory.iter().find(|item| item.letter == letter)
    }

    /// What the player attacks with when nothing else is left, as the engine plays it and as the
    /// plan names it, noting in `not_modelled` a weapon it can't play as the game named it.
    fn base<'b>(
        &self,
        bundle: &'b Bundle,
        not_modelled: &mut BTreeSet<NotModelled>,
    ) -> (Base<'b>, PlanBase) {
        let Wielding::Item(letter) = self.wielding else {
            let skill = self.skill(bundle, skill_types::UNARMED_COMBAT);
            return (Base::Unarmed { skill }, PlanBase::Melee);
        };
        let item = self.carried(letter);
        let kind = item.and_then(|item| item.kind.as_ref());
        let sub_type = kind
            .filter(|kind| kind.class.0 == object_classes::WEAPONS)
            .and_then(|kind| kind.sub_type.as_ref());
        let (Some(item), Some(sub_type)) = (item, sub_type) else {
            not_modelled.insert(NotModelled::OwnWeapon);
            return (Base::Nothing, PlanBase::Nothing);
        };
        let read = carried_gear::read_item(&item.name, bundle);
        let mut weapon_unread = read.plus.is_none();
        let heft = match &read.brand {
            ItemBrand::None => Heft::Plain,
            ItemBrand::Weapon(brand) => Heft::of(bundle.formulas.player_attack, brand)
                .unwrap_or_else(|| {
                    weapon_unread = true;
                    Heft::Plain
                }),
            ItemBrand::Unnamed(_) | ItemBrand::Unknown => {
                weapon_unread = true;
                Heft::Plain
            }
        };
        if weapon_unread {
            not_modelled.insert(NotModelled::OwnWeapon);
        }
        let plus = read.plus.unwrap_or(0);
        if let Some(weapon) = bundle.melee_weapons.get(sub_type) {
            let skill = self.skill(bundle, &weapon.skill.0);
            let base = Base::Weapon {
                weapon,
                plus,
                heft,
                skill,
            };
            return (base, PlanBase::Melee);
        }
        match bundle.missile_weapons.get_key_value(sub_type) {
            Some((id, weapon)) if weapon.kind == MissileKind::Launcher => {
                let skill = self.skill(bundle, skill_types::RANGED_WEAPONS);
                let base = Base::Launcher {
                    weapon,
                    id,
                    plus,
                    heft,
                    skill,
                };
                (
                    base,
                    PlanBase::Launcher {
                        name: weapon.name.clone(),
                    },
                )
            }
            _ => {
                not_modelled.insert(NotModelled::OwnWeapon);
                (Base::Nothing, PlanBase::Nothing)
            }
        }
    }

    /// An opening for each spell the player knows that the engine ports as an attack, as the
    /// spell list last showed it with its power and range, and the names of the spells it
    /// doesn't play. What [`spell_gaps`] finds is a Gap in `gaps` that may hide a better plan.
    fn spells<'b>(
        &self,
        remembered: Option<&AccumulatedState>,
        bundle: &'b Bundle,
        gaps: &mut BTreeSet<Gap>,
        not_played: &mut Vec<String>,
        not_modelled: &mut BTreeSet<NotModelled>,
    ) -> Vec<(PlanOpening, Opening<'b>)> {
        gaps.extend(spell_gaps(remembered, self.mp_max > 0, bundle));
        let Some(known) = remembered.and_then(|state| state.spells.read_value()) else {
            return Vec::new();
        };
        let reaches = remembered.and_then(|state| state.spell_reaches.read_value());
        let mut openings = Vec::new();
        for spell in known {
            let Some(attack) = attack_spell(spell, bundle) else {
                not_played.push(spell.name.clone());
                continue;
            };
            let Some(reach) =
                reaches.and_then(|reaches| reaches.iter().find(|reach| reach.name == spell.name))
            else {
                continue;
            };
            match spell_opening(spell, reach, attack, not_modelled) {
                Some(opening) => openings.push((
                    PlanOpening::Spell {
                        name: spell.name.clone(),
                    },
                    opening,
                )),
                None => not_played.push(spell.name.clone()),
            }
        }
        openings
    }

    /// An opening for each wand the player carries that zaps a spell the engine ports as an
    /// attack, known by its kind and with its charges shown, and the names of the other wands in
    /// `not_played`.
    fn wands<'b>(
        &self,
        bundle: &'b Bundle,
        not_played: &mut Vec<String>,
    ) -> Vec<(PlanOpening, Opening<'b>)> {
        let variant = bundle.formulas.player_beam;
        let wands = self.inventory.iter().filter(|item| {
            item.kind
                .as_ref()
                .is_some_and(|kind| kind.class.0 == object_classes::WANDS)
        });
        let mut openings = Vec::new();
        for item in wands {
            let opening = (|| {
                let wand = bundle.wands.get(item.kind.as_ref()?.sub_type.as_ref()?)?;
                let spell = bundle.spell_by_id(&wand.spell)?;
                let zap = PlayerZap::of(variant, spell)?;
                let range = spell.range?;
                let charges = charges(&item.name)?;
                Some((
                    PlanOpening::Wand {
                        name: format!("{}{}", carried_gear::WAND_OF, wand.name),
                        charges,
                    },
                    Opening::Wand {
                        zap,
                        range,
                        charges,
                    },
                ))
            })();
            match opening {
                Some(opening) => openings.push(opening),
                None => not_played.push(carried_gear::without_article(&item.name).to_owned()),
            }
        }
        openings
    }

    /// An opening for each stack of missiles the player carries that they throw and that deal
    /// damage of their own: not a large rock, which only a large species throws.
    fn thrown<'b>(&self, bundle: &'b Bundle) -> Vec<(PlanOpening, Opening<'b>)> {
        let skill = self.skill(bundle, skill_types::THROWING);
        self.inventory
            .iter()
            .filter_map(|item| {
                let kind = item.kind.as_ref()?;
                if kind.class.0 != object_classes::MISSILES {
                    return None;
                }
                let sub_type = kind.sub_type.as_ref()?;
                if sub_type.0 == missile_types::LARGE_ROCK {
                    return None;
                }
                let (id, weapon) = bundle.missile_weapons.get_key_value(sub_type)?;
                if weapon.kind != MissileKind::Thrown || weapon.damage <= 0 {
                    return None;
                }
                Some((
                    PlanOpening::Thrown {
                        name: carried_gear::without_article(&item.name).to_owned(),
                        count: item.quantity,
                    },
                    Opening::Thrown {
                        weapon,
                        id,
                        count: item.quantity,
                        skill,
                    },
                ))
            })
            .collect()
    }
}

/// The opening of a spell the player knows, `spell`, cast as `reach` shows it, as `attack`:
/// `None` for one they always fail, can't cast for its range, or whose power the list doesn't
/// show. A power shown as a percentage of the spell's cap is drawn from those it could be. Its
/// miscasts are noted in `not_modelled`.
fn spell_opening<'b>(
    spell: &KnownSpell,
    reach: &SpellReach,
    attack: AttackSpell<'b>,
    not_modelled: &mut BTreeSet<NotModelled>,
) -> Option<Opening<'b>> {
    let failure_percent = i32::try_from(spell.failure_percent).ok()?;
    if failure_percent >= 100 {
        return None;
    }
    let power = match reach.power? {
        SpellPower::Exact(power) => Power::Exact(power),
        SpellPower::PercentOfCap(percent) => {
            let cap = attack.power_cap()?;
            let percent = i32::try_from(percent).ok()?;
            // `spell_power_percent` (`spl-cast.cc:3299`) rounds `power * 100 / cap` down.
            let least = (percent * cap + 99) / 100;
            let most = (((percent + 1) * cap - 1) / 100).min(cap);
            Power::Between(least, most.max(least))
        }
    };
    not_modelled.insert(NotModelled::Miscasts);
    Some(Opening::Spell {
        spell: attack,
        power,
        cost: i32::try_from(spell.level).ok()?,
        failure_percent,
        range: attack.reach(reach.range?),
    })
}

/// What keeps the Fight Outcome from playing the player's attack spells, or may hide one,
/// whatever is in view: the spell list never read, for a player with magic to cast with
/// (`casts`), a spell memorised the game didn't name and the list out of date, as
/// [`gaps::spell_list`] finds them; its power and range out of date; and an attack spell the
/// engine ports whose power and range haven't been read.
pub(crate) fn spell_gaps(
    remembered: Option<&AccumulatedState>,
    casts: bool,
    bundle: &Bundle,
) -> BTreeSet<Gap> {
    let mut gaps = gaps::spell_list(remembered, casts);
    let (Some(state), Some(known)) = (
        remembered,
        remembered.and_then(|state| state.spells.read_value()),
    ) else {
        return gaps;
    };
    // Power out of date hides an attack only among spells known.
    if let (false, Remembered::Stale(read)) = (known.is_empty(), &state.spell_reaches) {
        gaps.insert(Gap::Stale {
            fact: ScreenFact::SpellPower,
            turn: read.turn,
        });
    }
    let reaches = state.spell_reaches.read_value();
    let unreached = |spell: &KnownSpell| {
        !reaches.is_some_and(|reaches| reaches.iter().any(|reach| reach.name == spell.name))
    };
    if known
        .iter()
        .any(|spell| attack_spell(spell, bundle).is_some() && unreached(spell))
    {
        gaps.insert(Gap::Unread {
            fact: ScreenFact::SpellPower,
        });
    }
    gaps
}

/// `spell`, which the player knows, as the engine plays it, when it ports it as an attack.
fn attack_spell<'b>(spell: &KnownSpell, bundle: &'b Bundle) -> Option<AttackSpell<'b>> {
    bundle
        .spells
        .values()
        .find(|known| known.name == spell.name)
        .and_then(|known| AttackSpell::of(bundle.formulas.player_beam, known))
}

/// A known wand's charges, as its name shows them after it, such as `a wand of flame (12)`
/// (`item-name.cc:1678`).
fn charges(name: &str) -> Option<u32> {
    let (_, charges) = name.strip_suffix(')')?.rsplit_once(" (")?;
    charges.parse().ok()
}

impl Fight {
    /// The focus card for this fight with `monsters` listed in view, for `reader`: `None` when
    /// there is no small fight to judge, or the Advisor can't judge the moment at all, which the
    /// now pill says.
    pub(crate) fn card(&self, monsters: Option<Vec<Listing>>, reader: Reader) -> Option<FocusCard> {
        let card = |line: String, others, not_modelled| FocusCard::Fight {
            monsters,
            line: self.marked(line, reader),
            others,
            not_modelled,
            gaps: self
                .gaps()
                .iter()
                .map(|gap| fight_gap(gap, reader))
                .collect(),
        };
        match self {
            Self::NoFight => None,
            Self::TooMany { hostiles } => Some(card(
                format!("Too many hostiles to judge a fight: {hostiles} in view."),
                Vec::new(),
                None,
            )),
            Self::Unjudged { gaps } if !gaps.iter().any(fight_input) => None,
            Self::Unjudged { .. } => Some(card(
                "The Advisor can't judge a fight yet.".to_owned(),
                Vec::new(),
                None,
            )),
            Self::Judged(outcome) => {
                let hp = outcome.hp;
                let mut plans = outcome.plans.iter();
                let line = plans.next().map_or_else(
                    || "No plan to fight with.".to_owned(),
                    |best| format!("Best plan ({}): {}", best.plan.name(), odds(best, hp)),
                );
                let others = plans
                    .map(|plan| self.marked(plan.summary(hp), reader))
                    .collect();
                let words: Vec<&str> = outcome
                    .not_modelled
                    .iter()
                    .filter_map(|&variant| fight_words(variant))
                    .collect();
                let mut not_modelled = format!("Not modelled: {}", words.join(", "));
                if !outcome.not_played.is_empty() {
                    not_modelled
                        .push_str(&format!(". Not played: {}", outcome.not_played.join(", ")));
                }
                Some(card(line, others, Some(not_modelled)))
            }
        }
    }
}

/// Whether `gap` is a fact only the Fight Outcome needs, rather than one that keeps the Threat
/// Level from being judged too.
fn fight_input(gap: &Gap) -> bool {
    matches!(
        gap,
        Gap::Unread {
            fact: ScreenFact::Skills | ScreenFact::SpellPower
        } | Gap::Inventory
            | Gap::Wielded
            | Gap::Attributes
    )
}

/// A plan's odds in words, such as `~80% win, ~45% HP lost`, against the `hp` it started from.
fn odds(plan: &PlanOutcome, hp: i32) -> String {
    format!(
        "~{:.0}% win, ~{}% HP lost",
        plan.win.percent(),
        plan.hp_lost_percent(hp)
    )
}

/// A few words for what a fight leaves out, as the card lists it: `None` for what only the
/// Threat Level or the Ways Out leave out, which the card doesn't list. Matched exhaustively, so
/// a new variant doesn't compile until it is sorted here.
fn fight_words(variant: NotModelled) -> Option<&'static str> {
    match variant {
        NotModelled::Summons => Some("summons"),
        NotModelled::Allies => Some("allies"),
        NotModelled::Hexes => Some("hexes"),
        NotModelled::SelfBuffs => Some("self-buffs"),
        NotModelled::MixingWands => Some("mixing wands"),
        NotModelled::MovingBetweenAttacks => Some("moving between attacks"),
        NotModelled::MonsterWounds => Some("monsters' wounds"),
        NotModelled::OwnAttackExtras => Some("brands, kicks and slaying"),
        NotModelled::OwnWeapon => Some("your weapon as named"),
        NotModelled::OwnFractions => Some("skill fractions"),
        NotModelled::PlayerSpeed => Some("your speed"),
        NotModelled::Miscasts => Some("miscasts"),
        NotModelled::BlastsAndBolts => Some("blast and bolt spread"),
        NotModelled::MonsterLighting => Some("halos and umbras"),
        NotModelled::MonsterArmour => Some("monsters' armour"),
        NotModelled::MonsterEquipment
        | NotModelled::AttackFlavours
        | NotModelled::Statuses
        | NotModelled::Paths
        | NotModelled::Awareness
        | NotModelled::OtherSpells
        | NotModelled::SpellsPerMonster
        | NotModelled::DraconianBreath
        | NotModelled::MutantBeastFacets
        | NotModelled::PlayerResistances
        | NotModelled::CasterSafety
        | NotModelled::CarriedGear
        | NotModelled::OtherWands
        | NotModelled::WornShield
        | NotModelled::OwnHalo
        | NotModelled::DefenceFractions
        | NotModelled::SwimmingAndFlying
        | NotModelled::UnexploredSquares
        | NotModelled::HitsOnTheWay
        | NotModelled::MonstersOutOfView
        | NotModelled::OwnConditions
        | NotModelled::MovesWithParent
        | NotModelled::OwnActions
        | NotModelled::SwoopOrFlank
        | NotModelled::SpecialAbilities
        | NotModelled::MovesThePlayer
        | NotModelled::BlinksAway
        | NotModelled::BlinksWithThePlayer
        | NotModelled::BoundToTerrain => None,
    }
}

/// What a Gap keeps the Advisor from judging of a fight, for `reader`: for the player, with how
/// to fill it where they can.
fn fight_gap(gap: &Gap, reader: Reader) -> String {
    if reader == Reader::Spectator {
        return gap.sentence(reader);
    }
    match gap {
        Gap::Unread {
            fact: fact @ ScreenFact::Skills,
        } => gaps::open_to(*fact, "to judge a fight."),
        Gap::Unread {
            fact: fact @ (ScreenFact::SpellList | ScreenFact::SpellPower),
        } => gaps::open_to(*fact, "to count your attack spells."),
        Gap::Map | Gap::MonstersInView | Gap::Position => {
            "The game hasn't sent its map yet.".to_owned()
        }
        Gap::MonsterKnowledge { monster } => format!(
            "{} isn't in the Game Knowledge, so the fight can't be judged.",
            capitalised(monster)
        ),
        Gap::SpellMemorised { turn } => format!(
            "{}: {} to count it.",
            memorised_unnamed(*turn),
            ScreenFact::SpellList.how_to_open(false)
        ),
        Gap::HitPoints
        | Gap::Defences
        | Gap::GameKnowledge
        | Gap::Unread {
            fact:
                ScreenFact::AbilityMenu
                | ScreenFact::Mutations
                | ScreenFact::Resistances
                | ScreenFact::Equipped,
        }
        | Gap::Inventory
        | Gap::Wielded
        | Gap::Attributes
        | Gap::Stale { .. }
        | Gap::SkillsShifting { .. } => gap.sentence(reader),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use copilot_gk::{ItemKind, Library};

    use super::*;
    use crate::accumulated_state::{AsOf, Remembered};
    use crate::fact_sheet::{FactSheet, Threat, ThreatLevel};
    use crate::screen::Screen;
    use crate::view_model::{Colour, Glyph};
    use crate::visible_state::{Attitude, Position, Stats, VisibleMonster};

    /// The shipped 0.34.1 bundle.
    fn bundle() -> &'static Bundle {
        static LIBRARY: std::sync::OnceLock<Library> = std::sync::OnceLock::new();
        LIBRARY
            .get_or_init(|| Library::shipped().unwrap())
            .for_version("0.34.1")
            .unwrap()
    }

    /// A player on `hp`, with Strength and Dexterity at `attributes`, no defences and `mp` magic,
    /// carrying `inventory`, holding `wielded`, facing each `(name, distance)` hostile east of
    /// them.
    struct Player {
        hp: i32,
        mp: i32,
        attributes: i32,
        inventory: Vec<InventoryItem>,
        wielded: Wielding,
        skills: Option<Vec<(&'static str, u32)>>,
        spells: Option<Vec<(&'static str, u32, u32)>>,
        powers: Option<Vec<(&'static str, i32, i32)>>,
    }

    impl Player {
        /// Unarmed, with no skills at all read yet.
        fn new(hp: i32) -> Self {
            Self {
                hp,
                mp: 0,
                attributes: 10,
                inventory: Vec::new(),
                wielded: Wielding::Nothing,
                skills: Some(Vec::new()),
                spells: Some(Vec::new()),
                powers: Some(Vec::new()),
            }
        }

        fn skilled(mut self, skills: &[(&'static str, u32)]) -> Self {
            self.skills = Some(skills.to_vec());
            self
        }

        fn carrying(
            mut self,
            letter: char,
            name: &str,
            quantity: u32,
            class: &str,
            sub: &str,
        ) -> Self {
            self.inventory.push(InventoryItem {
                letter,
                name: name.to_owned(),
                quantity,
                kind: Some(ItemKind {
                    class: EnumId(class.to_owned()),
                    sub_type: Some(EnumId(sub.to_owned())),
                }),
                useless: false,
            });
            self
        }

        fn wielding(mut self, letter: char) -> Self {
            self.wielded = Wielding::Item(letter);
            self
        }

        /// Knowing each `(name, failure percent, level)` spell, each cast at `(name, power,
        /// range)`.
        fn casting(
            mut self,
            mp: i32,
            spells: &[(&'static str, u32, u32)],
            powers: &[(&'static str, i32, i32)],
        ) -> Self {
            self.mp = mp;
            self.spells = Some(spells.to_vec());
            self.powers = Some(powers.to_vec());
            self
        }

        /// What the Advisor sees and remembers of them facing `hostiles`.
        fn facing(&self, hostiles: &[(&str, i32)]) -> (VisibleState, AccumulatedState) {
            let player = Position { x: 10, y: 10 };
            let monsters = hostiles
                .iter()
                .enumerate()
                .map(|(i, &(name, distance))| {
                    let kind = bundle().monsters.values().find(|m| m.name == name).unwrap();
                    VisibleMonster {
                        name: name.to_owned(),
                        glyph: Glyph {
                            text: kind.glyph.clone(),
                            colour: Colour::White,
                        },
                        id: Some(kind.id.clone()),
                        position: Position {
                            x: player.x + distance,
                            y: player.y + i32::try_from(i).unwrap(),
                        },
                        attitude: Some(Attitude::Hostile),
                        crawl_threat: None,
                        sighting: None,
                    }
                })
                .collect();
            let visible = VisibleState {
                stats: Stats {
                    hp: Some(self.hp),
                    hp_max: Some(self.hp),
                    mp: Some(self.mp),
                    mp_max: Some(self.mp),
                    ac: Some(0),
                    ev: Some(0),
                    sh: Some(0),
                    strength: Some(self.attributes),
                    dexterity: Some(self.attributes),
                    ..Stats::default()
                },
                position: Some(player),
                monsters: Some(monsters),
                inventory: Some(self.inventory.clone()),
                wielded: Some(self.wielded),
                ..VisibleState::default()
            };
            let mut remembered = AccumulatedState::default();
            if let Some(skills) = &self.skills {
                let tenths: BTreeMap<String, u32> = skills
                    .iter()
                    .map(|&(name, tenths)| (name.to_owned(), tenths))
                    .collect();
                remembered.skills = known(Skills { tenths }, Screen::Skills);
            }
            if let Some(spells) = &self.spells {
                let spells = spells
                    .iter()
                    .map(|&(name, failure_percent, level)| KnownSpell {
                        name: name.to_owned(),
                        failure_percent,
                        level,
                    })
                    .collect();
                remembered.spells = known(spells, Screen::Spells);
            }
            if let Some(powers) = &self.powers {
                let reaches = powers
                    .iter()
                    .map(|&(name, power, range)| SpellReach {
                        name: name.to_owned(),
                        power: Some(SpellPower::Exact(power)),
                        range: Some(range),
                    })
                    .collect();
                remembered.spell_reaches = known(reaches, Screen::Spells);
            }
            (visible, remembered)
        }

        /// The Fact Sheet of them facing `hostiles`.
        fn against(&self, hostiles: &[(&str, i32)]) -> FactSheet {
            let (visible, remembered) = self.facing(hostiles);
            FactSheet::compute(&visible, Some(&remembered), Some("Human"), Some(bundle()))
        }
    }

    /// `value`, Known as read off `screen` on turn 1.
    fn known<T>(value: T, screen: Screen) -> Remembered<T> {
        Remembered::Known(AsOf {
            value,
            turn: Some(1),
            source: screen.into(),
        })
    }

    /// The judged fight on `sheet`.
    fn judged(sheet: &FactSheet) -> &FightOutcome {
        match &sheet.fight {
            Fight::Judged(outcome) => outcome,
            other => panic!("not judged: {other:?}"),
        }
    }

    /// The outcome of the plan named `name` on `sheet`.
    fn plan<'s>(sheet: &'s FactSheet, name: &str) -> &'s PlanOutcome {
        judged(sheet)
            .plans
            .iter()
            .find(|plan| plan.plan.name() == name)
            .unwrap_or_else(|| panic!("no plan {name}: {:?}", judged(sheet).plans))
    }

    fn fighter() -> Player {
        Player::new(60)
            .skilled(&[("Fighting", 100), ("Axes", 100)])
            .carrying('a', "a +0 war axe", 1, "OBJ_WEAPONS", "WPN_WAR_AXE")
            .wielding('a')
    }

    #[test]
    fn a_trained_fighter_beats_a_jackal_in_melee_almost_every_time() {
        let sheet = fighter().against(&[("jackal", 2)]);
        let best = &judged(&sheet).plans[0];
        assert_eq!(best.plan.name(), "melee");
        assert!(best.win.times >= 1_990, "{best:?}");
    }

    #[test]
    fn a_fight_the_best_plan_likely_loses_is_dangerous_before_anything_can_reach() {
        let sheet = Player::new(10).against(&[("ogre", 6)]);
        let Threat::Rated(assessment) = &sheet.threat else {
            panic!("unrated");
        };
        assert!(assessment.acting.is_empty(), "{assessment:?}");
        assert!(assessment.fight_doubtful(), "{assessment:?}");
        assert_eq!(assessment.level, ThreatLevel::Dangerous);
    }

    #[test]
    fn a_fight_the_best_plan_wins_leaves_a_monster_out_of_reach_calm() {
        let sheet = fighter().against(&[("jackal", 6)]);
        let Threat::Rated(assessment) = &sheet.threat else {
            panic!("unrated");
        };
        assert_eq!(assessment.level, ThreatLevel::Calm);
    }

    /// A novice who knows Magic Dart, with `mp` magic, against a gnoll.
    fn dart_thrower(mp: i32) -> FactSheet {
        Player::new(25)
            .skilled(&[("Fighting", 10)])
            .casting(mp, &[("Magic Dart", 0, 1)], &[("Magic Dart", 25, 4)])
            .against(&[("gnoll", 3)])
    }

    #[test]
    fn a_spell_plan_falls_back_on_melee_once_the_magic_runs_out() {
        let melee = plan(&dart_thrower(0), "melee").win.times;
        let no_magic = plan(&dart_thrower(0), "Magic Dart, then melee").win.times;
        let magic = plan(&dart_thrower(20), "Magic Dart, then melee").win.times;
        // Without magic it is melee from the first action, rolled on other dice.
        assert!(no_magic.abs_diff(melee) < 80, "{no_magic} against {melee}");
        assert!(magic > no_magic + 200, "{magic} against {no_magic}");
    }

    /// A novice who knows Freeze, cast at power 20 with the range the spell list shows, `range`,
    /// against a hobgoblin `distance` squares away.
    fn freezer(range: i32, distance: i32) -> FactSheet {
        Player::new(15)
            .skilled(&[("Fighting", 10)])
            .casting(10, &[("Freeze", 5, 1)], &[("Freeze", 20, range)])
            .against(&[("hobgoblin", distance)])
    }

    #[test]
    fn freeze_opens_a_plan_that_beats_melee_once_the_monster_walks_up() {
        let sheet = freezer(1, 3);
        let freeze = plan(&sheet, "Freeze, then melee");
        assert!(
            freeze.win.times > plan(&sheet, "melee").win.times + 200,
            "{freeze:?}"
        );
        assert_eq!(judged(&sheet).plans[0].plan.name(), "Freeze, then melee");
    }

    #[test]
    fn freeze_reaches_only_a_monster_beside_the_player_whatever_range_the_list_shows() {
        // The same rolls, so a Freeze that reached farther would win sooner or cost less.
        let shown_far = freezer(4, 3);
        assert_eq!(
            plan(&shown_far, "Freeze, then melee"),
            plan(&freezer(1, 3), "Freeze, then melee")
        );
    }

    #[test]
    fn kiss_of_death_opens_a_plan() {
        let sheet = Player::new(15)
            .casting(10, &[("Kiss of Death", 5, 1)], &[("Kiss of Death", 20, 1)])
            .against(&[("hobgoblin", 3)]);
        let kiss = plan(&sheet, "Kiss of Death, then melee");
        assert!(
            kiss.win.times > plan(&sheet, "melee").win.times + 200,
            "{kiss:?}"
        );
    }

    /// A trained archer with a shortbow, against a monster `distance` squares away.
    fn archer(distance: i32) -> FactSheet {
        Player::new(30)
            .skilled(&[("Fighting", 50), ("Ranged Weapons", 80)])
            .carrying('a', "a +0 shortbow", 1, "OBJ_WEAPONS", "WPN_SHORTBOW")
            .wielding('a')
            .against(&[("orc warrior", distance)])
    }

    #[test]
    fn a_launcher_shoots_freely_while_a_monster_closes_in() {
        let (near, far) = (archer(1), archer(6));
        let (near, far) = (plan(&near, "shortbow"), plan(&far, "shortbow"));
        assert!(far.win.times > near.win.times, "{far:?} against {near:?}");
        assert!(far.hp_lost < near.hp_lost, "{far:?} against {near:?}");
    }

    /// An untrained player with `count` javelins, against a hobgoblin.
    fn javelineer(count: u32) -> FactSheet {
        Player::new(20)
            .skilled(&[("Throwing", 50)])
            .carrying('b', "javelins", count, "OBJ_MISSILES", "MI_JAVELIN")
            .against(&[("hobgoblin", 4)])
    }

    #[test]
    fn thrown_missiles_run_out() {
        let one = plan(&javelineer(1), "javelins, then melee").win.times;
        let many = plan(&javelineer(20), "javelins, then melee").win.times;
        assert!(many > one + 200, "{many} against {one}");
    }

    #[test]
    fn a_known_wand_with_charges_is_a_plan_of_its_own() {
        let sheet = Player::new(20)
            .carrying('c', "a wand of flame (12)", 1, "OBJ_WANDS", "WAND_FLAME")
            .against(&[("hobgoblin", 3)]);
        let wand = plan(&sheet, "wand of flame, then melee");
        assert!(wand.win.times > plan(&sheet, "melee").win.times, "{wand:?}");
    }

    #[test]
    fn more_than_three_hostiles_are_too_many_to_judge() {
        let sheet =
            fighter().against(&[("jackal", 2), ("jackal", 3), ("jackal", 4), ("jackal", 5)]);
        assert_eq!(sheet.fight, Fight::TooMany { hostiles: 4 });
    }

    #[test]
    fn without_the_skills_screen_a_fight_is_unjudged() {
        let mut player = fighter();
        player.skills = None;
        let sheet = player.against(&[("jackal", 2)]);
        assert_eq!(
            sheet.fight,
            Fight::Unjudged {
                gaps: vec![Gap::Unread {
                    fact: ScreenFact::Skills
                }]
            }
        );
    }

    #[test]
    fn an_unread_spell_list_or_its_power_may_hide_a_better_plan() {
        let mut unread = fighter();
        unread.mp = 5;
        unread.spells = None;
        assert_eq!(
            judged(&unread.against(&[("jackal", 2)])).gaps,
            [Gap::Unread {
                fact: ScreenFact::SpellList
            }]
        );
        let mut powerless = fighter().casting(10, &[("Magic Dart", 0, 1)], &[]);
        powerless.powers = None;
        let sheet = powerless.against(&[("jackal", 2)]);
        assert_eq!(
            judged(&sheet).gaps,
            [Gap::Unread {
                fact: ScreenFact::SpellPower
            }]
        );
        assert!(
            judged(&sheet)
                .plans
                .iter()
                .all(|plan| plan.plan.opening.is_none())
        );
    }

    #[test]
    fn spells_and_wands_no_plan_uses_are_named_as_not_played() {
        let sheet = fighter()
            .casting(10, &[("Blink", 5, 2)], &[("Blink", 30, 0)])
            .carrying('c', "a wand of flame", 1, "OBJ_WANDS", "WAND_FLAME")
            .against(&[("jackal", 2)]);
        assert_eq!(judged(&sheet).not_played, ["Blink", "wand of flame"]);
        let Some(FocusCard::Fight { not_modelled, .. }) = sheet.fight.card(None, Reader::Player)
        else {
            panic!("no fight card");
        };
        assert!(
            not_modelled
                .unwrap()
                .ends_with("Not played: Blink, wand of flame"),
        );
    }

    #[test]
    fn a_player_without_magic_isnt_asked_for_the_spell_list() {
        let mut fighter = fighter();
        fighter.spells = None;
        assert!(judged(&fighter.against(&[("jackal", 2)])).gaps.is_empty());
    }

    #[test]
    fn a_spectator_is_told_what_keeps_a_fight_unjudged_but_not_asked_to_open_a_screen() {
        let mut player = fighter();
        player.skills = None;
        let sheet = player.against(&[("jackal", 2)]);
        let Some(FocusCard::Fight { line, gaps, .. }) = sheet.fight.card(None, Reader::Spectator)
        else {
            panic!("no fight card");
        };
        let said = "Skills: no fight can be judged without them.";
        assert_eq!(line.gap.as_deref(), Some(said));
        assert_eq!(gaps, [said]);
    }

    #[test]
    fn too_many_hostiles_are_said_on_the_card() {
        let sheet =
            fighter().against(&[("jackal", 2), ("jackal", 3), ("jackal", 4), ("jackal", 5)]);
        let Some(FocusCard::Fight { line, .. }) = sheet.fight.card(None, Reader::Player) else {
            panic!("no fight card");
        };
        assert_eq!(
            line,
            MarkedValue::known("Too many hostiles to judge a fight: 4 in view.".to_owned())
        );
    }

    #[test]
    fn the_card_names_the_best_plan_with_its_odds_and_what_isnt_modelled() {
        let sheet = dart_thrower(20);
        let Some(FocusCard::Fight {
            line, not_modelled, ..
        }) = sheet.fight.card(None, Reader::Player)
        else {
            panic!("no fight card");
        };
        let line = line.text;
        assert!(
            line.starts_with("Best plan (Magic Dart, then melee): ~"),
            "{line}"
        );
        assert!(
            line.contains("% win, ~") && line.ends_with("% HP lost"),
            "{line}"
        );
        let not_modelled = not_modelled.unwrap();
        for words in ["summons", "allies", "hexes", "self-buffs", "mixing wands"] {
            assert!(not_modelled.contains(words), "{not_modelled}");
        }
        assert!(
            not_modelled.contains("moving between attacks"),
            "{not_modelled}"
        );
    }
}

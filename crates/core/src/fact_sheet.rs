//! The Fact Sheet: everything the Advisor has computed about one moment, and the Gaps that kept
//! it from computing more.

use std::collections::BTreeSet;

use copilot_gk::{Bundle, LightingSigns, MonsterSpells};
use serde::Serialize;
use specta::Type;

use crate::accumulated_state::{AccumulatedState, GearSource, Remembered};
use crate::carried_gear::Unlisted;
use crate::engine::lighting::Lighting;
use crate::engine::melee::DisplayedDefences;
use crate::engine::spells::{DamagingSpell, Reach};
use crate::engine::threat::{self, Attacker, Defender, Outcome};
use crate::visible_state::{
    Attitude, Position, Religion, SquareLight, VisibleMonster, VisibleState,
};
use crate::ways_out::WaysOut;

/// LETHAL from this chance of death before the player's next action, per mille: 5%. A starting
/// threshold from the spec (#24, "Engine (ADR 0003)"), to be tuned on the corpus.
const LETHAL_DEATHS_PER_MILLE: u32 = 50;

/// DANGEROUS from this chance of death, per mille: 0.5%. A starting threshold from the spec (#24,
/// "Engine (ADR 0003)"), to be tuned on the corpus.
const DANGEROUS_DEATHS_PER_MILLE: u32 = 5;

/// DANGEROUS when the worst-case hit is at least this percentage of current HP. A starting
/// threshold from the spec (#24, "Engine (ADR 0003)"), to be tuned on the corpus.
const DANGEROUS_WORST_HIT_PERCENT_OF_HP: i32 = 40;

/// The genus crawl gives a breath outside its book, by colour: `drac_breath` in `mon-util.cc`.
const DRACONIAN_GENUS: &str = "MONS_DRACONIAN";

/// Everything the Advisor has computed about one moment of the game it follows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FactSheet {
    /// How dangerous this moment is.
    pub threat: Threat,
    /// The player's escapes and emergency options, and the ground they stand on.
    pub ways_out: WaysOut,
}

/// How dangerous this moment is, or why the Advisor can't tell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Threat {
    /// The Advisor judged the moment.
    Rated(Box<ThreatAssessment>),
    /// The Advisor lacks facts it needs to judge the moment, so it judges nothing.
    Unrated {
        /// What it lacks.
        gaps: Vec<Gap>,
    },
}

/// The Threat Level and the numbers it was judged from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreatAssessment {
    /// How dangerous the moment is.
    pub level: ThreatLevel,
    /// The player's hit points it was judged against.
    pub hp: i32,
    /// The chance the player dies before their next standard-speed action. Nothing is played
    /// out when nothing can attack.
    pub death: Chance,
    /// The total damage the player takes before their next standard-speed action at the 99th
    /// percentile: as bad as it gets, barring one time in a hundred.
    pub worst_hit: i32,
    /// How many monsters in view would attack the player, whether or not they can yet.
    pub hostiles_in_view: u32,
    /// The monsters that can attack before the player's next standard-speed action, in map
    /// order.
    pub acting: Vec<ActingMonster>,
    /// The gear the simulation played out for each hostile in view, acting or not, in map
    /// order: its Carried Gear, or its kind's Possible Gear when that isn't read.
    pub gear: Vec<GearPlayed>,
    /// How the light falls on the player, which changes how well monsters aim at them. `None`
    /// when where they stand isn't known.
    pub lighting: Option<Lighting>,
    /// What the judgement leaves out, so the numbers are read for what they are.
    pub not_modelled: Vec<NotModelled>,
}

/// How dangerous a moment is: the Threat Level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ThreatLevel {
    /// Neither likely to die nor to be hit hard before the next action.
    Calm,
    /// A real chance of death, or a hard hit, before the next action.
    Dangerous,
    /// Death before the next action is likely enough, or a hit could take every hit point.
    Lethal,
}

/// How often something happened when the engine played a moment out many times: exact, as
/// counted, rather than rounded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Chance {
    /// Trials in which it happened.
    pub times: u32,
    /// Trials played out.
    pub trials: u32,
}

impl Chance {
    /// No chance at all: nothing could happen, so nothing was played out.
    const NONE: Self = Self {
        times: 0,
        trials: 0,
    };

    /// Whether the chance is at least `per_mille` in a thousand, for a `per_mille` above 0.
    fn at_least_per_mille(self, per_mille: u32) -> bool {
        self.times > 0
            && u64::from(self.times) * 1000 >= u64::from(per_mille) * u64::from(self.trials)
    }

    /// The chance as a percentage.
    #[must_use]
    pub fn percent(self) -> f64 {
        if self.trials == 0 {
            return 0.0;
        }
        f64::from(self.times) * 100.0 / f64::from(self.trials)
    }
}

/// A monster that can attack the player before their next standard-speed action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActingMonster {
    /// Its name, as the game names it.
    pub name: String,
    /// The square it is on.
    pub position: Position,
    /// Squares between it and the player.
    pub distance: i32,
    /// The spells it may cast at the player that the simulation plays out, by name.
    pub casts: Vec<String>,
    /// The other spells of its book, by name: the simulation assumes it doesn't cast them.
    pub spells_not_modelled: Vec<String>,
    /// The launchers, throwing missiles and wands the simulation plays out for it, by name: those
    /// of its Carried Gear as the game named them, such as `+3 shortbow of flaming`, or those
    /// crawl may make its kind with, such as `shortbow` or `wand of flame`.
    pub shoots: Vec<String>,
}

/// The gear the simulation played out for one hostile in view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GearPlayed {
    /// Its name, as the game names it.
    pub name: String,
    /// The square it is on.
    pub position: Position,
    /// Squares between it and the player.
    pub distance: i32,
    /// Whose gear was played: what it was seen with, or what its kind is made with.
    pub basis: GearBasis,
    /// What of its Carried Gear isn't played out as the game named it.
    pub unplayed: Vec<Unplayed>,
}

/// Whose gear the simulation played out for a monster.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "basis",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum GearBasis {
    /// Its Carried Gear, as the game last listed it.
    Carried {
        /// Whether the game has shown anything since that may have changed it.
        state: Freshness,
        /// The turn it was read on.
        turn: Option<u32>,
        /// What it was read off.
        source: GearSource,
        /// What that listing leaves out, which is played from its kind's Possible Gear, given
        /// that it is nothing the listing would have named.
        unlisted: Unlisted,
    },
    /// Its kind's Possible Gear, by the odds crawl makes its kind with, as its Carried Gear
    /// hasn't been read.
    Possible {
        /// Whether its kind may be made with a launcher, throwing missiles or a wand.
        may_shoot: bool,
    },
}

/// Whether a remembered fact is Known or Stale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Freshness {
    /// Nothing the game has shown since may have changed it.
    Known,
    /// Something the game has shown since may have changed it.
    Stale,
}

/// Something about a monster's Carried Gear the simulation doesn't play out as the game named it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "unplayed",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Unplayed {
    /// A launcher whose name doesn't show its enchantment: played as +0.
    UnknownPlus {
        /// Its name, as the game wrote it.
        item: String,
    },
    /// A launcher whose name doesn't show its brand: played as unbranded.
    UnknownBrand {
        /// Its name, as the game wrote it.
        item: String,
    },
    /// A launcher or missile whose brand isn't ported, such as venom or poison: played as
    /// unbranded, its effect left with the attack flavours.
    BrandNotPorted {
        /// Its name, as the game wrote it.
        item: String,
        /// The brand, as its name gives it.
        brand: String,
    },
    /// A wand the Advisor can't name, carried by a monster whose kind crawl never makes with a
    /// wand, so its odds can't say which: not played.
    UnnamedWand {
        /// Its name, as the game wrote it, such as `bone wand`.
        item: String,
    },
}

/// Something the engine doesn't model, in the Threat Level's simulation or in judging the Ways
/// Out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NotModelled {
    /// Monsters fight in melee as if they wield nothing, with no weapon damage or brands, and
    /// wear no armour.
    MonsterEquipment,
    /// Attacks, spells and missiles count only their damage as they land: no poison, burning,
    /// draining, nets, clouds left behind and the like.
    AttackFlavours,
    /// Statuses on either side, such as haste, might or confusion, beyond those that backlight
    /// the player.
    Statuses,
    /// Monsters walk straight at the player, through walls and each other.
    Paths,
    /// Monsters are assumed awake and hunting the player.
    Awareness,
    /// A monster in reach has spells the simulation doesn't play out, such as summons, hexes,
    /// self-buffs and spells that aren't a single bolt or explosion: it is assumed never to cast
    /// them, and to fight instead.
    OtherSpells,
    /// A monster in view is given its own spells when it is made, such as a player ghost, so
    /// which it has isn't known, and none are played out.
    SpellsPerMonster,
    /// A draconian in view breathes by its colour, which isn't played out.
    DraconianBreath,
    /// Spells, and what a launcher's brand adds, land as if the player resists nothing and is a
    /// living mortal who worships no good god, because the engine doesn't apply the player's
    /// resistances yet.
    PlayerResistances,
    /// Monsters are assumed to cast explosions and clouds even when they'd catch themselves;
    /// crawl's casters weigh that, and often hold off up close.
    CasterSafety,
    /// A monster in view whose kind may carry a launcher, missiles or a wand is judged by its
    /// kind's Possible Gear, the odds crawl makes its kind with, +0 and unbranded, as the Advisor
    /// hasn't read its Carried Gear: it came back into view, or came with others whose message
    /// didn't list it. Its look text (`x`) tells it.
    CarriedGear,
    /// A monster in reach may carry a wand the simulation doesn't play out, such as paralysis or
    /// polymorph: it is assumed never to zap it, and to shoot or fight instead.
    OtherWands,
    /// The player's shield is assumed to block at most one attack before their next action, as
    /// without a shield, because the Advisor doesn't know which shield is worn. The character
    /// overview (`%`) tells it.
    WornShield,
    /// The player stands in a halo and may have one of their own, which keeps another's from
    /// backlighting them: they carry gear that gives one, such as the morningstar "Eos" or
    /// armour of light, and the Advisor doesn't know whether they wear it. They are assumed to
    /// have none, and to be backlit. The character overview (`%`) tells it.
    OwnHalo,
    /// The player is assumed to walk at standard speed, a square an action, whatever their
    /// species, statuses or gear.
    PlayerSpeed,
    /// Everyone is assumed to walk: deep water and lava stop them, even those who swim or fly.
    SwimmingAndFlying,
    /// Monsters are assumed able to cross every unexplored square, and to stand on one to
    /// attack, and the player to walk only over explored ones.
    UnexploredSquares,
    /// A staircase is cut off only by monsters that can stand beside it as the player takes it:
    /// the blows, shots and spells they could land on the way there aren't counted.
    HitsOnTheWay,
    /// Abilities and spells are judged only by their cost in magic and the statuses that stop
    /// them: what each needs of its own, such as piety, a cooldown, an exhausted berserker or a
    /// free square to blink to, isn't checked.
    OwnConditions,
}

/// What the simulation leaves out, whatever is in view.
const ALWAYS_NOT_MODELLED: [NotModelled; 5] = [
    NotModelled::MonsterEquipment,
    NotModelled::AttackFlavours,
    NotModelled::Statuses,
    NotModelled::Paths,
    NotModelled::Awareness,
];

/// A fact the Advisor needs about this moment but doesn't have.
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
}

impl FactSheet {
    /// What the engine makes of `visible` and what the Advisor remembers of the game, with
    /// `knowledge` for the game's Crawl Version if the Advisor has it.
    #[must_use]
    pub fn compute(
        visible: &VisibleState,
        remembered: Option<&AccumulatedState>,
        knowledge: Option<&Bundle>,
    ) -> Self {
        Self {
            threat: threat(visible, remembered, knowledge),
            ways_out: WaysOut::compute(visible, remembered, knowledge),
        }
    }
}

/// Monsters on these sides attack the player: crawl's `mons_att_wont_attack` (`mon-util.cc`)
/// spares only friends, good neutrals and marionettes, and strict neutrals don't hunt the player.
pub(crate) fn attacks_player(attitude: Attitude) -> bool {
    matches!(attitude, Attitude::Hostile | Attitude::Neutral)
}

fn threat(
    visible: &VisibleState,
    remembered: Option<&AccumulatedState>,
    knowledge: Option<&Bundle>,
) -> Threat {
    let mut gaps = BTreeSet::new();
    let hp = visible.stats.hp;
    if hp.is_none() {
        gaps.insert(Gap::HitPoints);
    }
    let Some(monsters) = &visible.monsters else {
        gaps.insert(Gap::MonstersInView);
        return Threat::Unrated {
            gaps: gaps.into_iter().collect(),
        };
    };

    // The monsters that may attack, with their Game Knowledge.
    let mut hostile: Vec<(&VisibleMonster, &copilot_gk::Monster)> = Vec::new();
    for monster in monsters {
        let Some(attitude) = monster.attitude else {
            gaps.insert(Gap::GameKnowledge);
            continue;
        };
        if !attacks_player(attitude) {
            continue;
        }
        let Some(bundle) = knowledge else {
            gaps.insert(Gap::GameKnowledge);
            continue;
        };
        match monster.id.as_ref().and_then(|id| bundle.monster_by_id(id)) {
            Some(known) => hostile.push((monster, known)),
            None => {
                gaps.insert(Gap::MonsterKnowledge {
                    monster: monster.name.clone(),
                });
            }
        }
    }

    let stats = &visible.stats;
    let defences = match (stats.ac, stats.ev, stats.sh) {
        (Some(ac), Some(ev), Some(sh)) => Some((ac, ev, sh)),
        _ => None,
    };
    if !hostile.is_empty() {
        if defences.is_none() {
            gaps.insert(Gap::Defences);
        }
        if visible.position.is_none() {
            gaps.insert(Gap::Position);
        }
    }
    let (Some(hp), true) = (hp, gaps.is_empty()) else {
        return Threat::Unrated {
            gaps: gaps.into_iter().collect(),
        };
    };
    let (false, Some(bundle), Some(player), Some((ac, ev, sh))) =
        (hostile.is_empty(), knowledge, visible.position, defences)
    else {
        // Nothing in view attacks the player: a hostile would have needed the rest, or been a Gap.
        return Threat::Rated(Box::new(ThreatAssessment {
            level: ThreatLevel::Calm,
            hp,
            death: Chance::NONE,
            worst_hit: 0,
            hostiles_in_view: 0,
            acting: Vec::new(),
            gear: Vec::new(),
            lighting: knowledge
                .and_then(|bundle| lighting(visible, remembered, &bundle.lighting))
                .map(|(lighting, _)| lighting),
            not_modelled: Vec::new(),
        }));
    };

    let formulas = bundle.formulas;
    let worn_shield = remembered.and_then(|state| state.worn_shield.known().copied());
    let mut not_modelled = ALWAYS_NOT_MODELLED.to_vec();
    if sh > 0 && worn_shield.is_none() {
        not_modelled.push(NotModelled::WornShield);
    }
    if hostile
        .iter()
        .any(|(_, monster)| monster.spells == MonsterSpells::PerMonster)
    {
        not_modelled.push(NotModelled::SpellsPerMonster);
    }
    if hostile
        .iter()
        .any(|(_, monster)| monster.genus.0 == DRACONIAN_GENUS)
    {
        not_modelled.push(NotModelled::DraconianBreath);
    }
    let (lighting, own_halo) =
        lighting(visible, remembered, &bundle.lighting).unwrap_or((Lighting::Plain, None));
    not_modelled.extend(own_halo);
    let defender = Defender {
        hp,
        defences: DisplayedDefences { ac, ev, sh },
        lighting,
        shield_blocks: threat::shield_blocks_per_action(formulas, worn_shield.flatten()),
    };
    let hostiles_in_view = u32::try_from(hostile.len()).unwrap_or(u32::MAX);
    // Each monster with the gear it is played with, and the monsters that can attack, each with
    // the spells of its book not played out.
    let mut gear: Vec<GearPlayed> = Vec::new();
    let mut acting: Vec<(&VisibleMonster, Vec<String>)> = Vec::new();
    let mut attackers: Vec<Attacker<'_>> = Vec::new();
    for (seen, monster) in hostile {
        let distance = distance(player, seen.position);
        let attacker = Attacker::new(bundle, monster, distance);
        let carried = remembered
            .and_then(|state| state.gear_of(seen, bundle))
            .and_then(|read| Some((read, read.read_as_of()?)));
        let (attacker, basis, unplayed) = match carried {
            Some((read, as_of)) => {
                let (attacker, unplayed) = attacker.carrying(bundle, &as_of.value);
                let basis = GearBasis::Carried {
                    state: match read {
                        Remembered::Stale(_) => Freshness::Stale,
                        Remembered::Known(_) | Remembered::Missing => Freshness::Known,
                    },
                    turn: as_of.turn,
                    source: as_of.source,
                    unlisted: as_of.value.unlisted,
                };
                (attacker, basis, unplayed)
            }
            None => {
                let basis = GearBasis::Possible {
                    may_shoot: monster.gear.may_shoot_or_zap(),
                };
                (attacker, basis, Vec::new())
            }
        };
        gear.push(GearPlayed {
            name: seen.name.clone(),
            position: seen.position,
            distance,
            basis,
            unplayed,
        });
        if threat::can_attack(formulas, &attacker) {
            acting.push((seen, spells_not_modelled(bundle, &attacker)));
            attackers.push(attacker);
        }
    }
    if gear
        .iter()
        .any(|played| played.basis == GearBasis::Possible { may_shoot: true })
    {
        not_modelled.push(NotModelled::CarriedGear);
    }
    if acting.iter().any(|(_, others)| !others.is_empty()) {
        not_modelled.push(NotModelled::OtherSpells);
    }
    if attackers.iter().any(|attacker| attacker.unported_wand) {
        not_modelled.push(NotModelled::OtherWands);
    }
    if attackers.iter().any(|attacker| {
        !attacker.spells.is_empty()
            || !attacker.wands.is_empty()
            || attacker.shoots_resisted_brand()
    }) {
        not_modelled.push(NotModelled::PlayerResistances);
    }
    if attackers.iter().any(|attacker| {
        let unsafe_reach = |reach: Reach, spell: &DamagingSpell<'_>| {
            reach.casts > reach.strikes || spell.zap.explodes
        };
        attacker
            .spells
            .iter()
            .any(|cast| unsafe_reach(cast.reach, &cast.spell))
            || attacker
                .wands
                .items()
                .any(|zap| unsafe_reach(zap.reach, &zap.spell))
    }) {
        not_modelled.push(NotModelled::CasterSafety);
    }
    let outcome = if attackers.is_empty() {
        Outcome {
            deaths: 0,
            trials: 0,
            worst_hit: 0,
        }
    } else {
        threat::simulate(formulas, defender, &attackers)
    };
    let death = Chance {
        times: outcome.deaths,
        trials: outcome.trials,
    };
    Threat::Rated(Box::new(ThreatAssessment {
        level: level(hp, death, outcome.worst_hit),
        hp,
        death,
        worst_hit: outcome.worst_hit,
        hostiles_in_view,
        acting: acting
            .into_iter()
            .zip(&attackers)
            .map(|((seen, others), attacker)| ActingMonster {
                name: seen.name.clone(),
                position: seen.position,
                distance: attacker.distance,
                casts: attacker
                    .spells
                    .iter()
                    .map(|cast| cast.spell.spell.name.clone())
                    .collect(),
                spells_not_modelled: others,
                shoots: attacker.shoots.clone(),
            })
            .collect(),
        gear,
        lighting: Some(lighting),
        not_modelled,
    }))
}

/// How the light falls on the player, as crawl's `player::backlit(false)` and `player::umbra()`
/// (`player.cc:7850`, `player.cc:7864`) decide it from what the game shows, read through `signs`: a status that backlights them, or
/// the halo or umbra on their square. A halo backlights them only if they have none of their
/// own; with [`NotModelled::OwnHalo`] beside it when the Advisor can't tell. `None` when where
/// they stand isn't known.
fn lighting(
    visible: &VisibleState,
    remembered: Option<&AccumulatedState>,
    signs: &LightingSigns,
) -> Option<(Lighting, Option<NotModelled>)> {
    let square = visible.light_at_player()?;
    let backlit_by_status = visible.statuses.iter().flatten().any(|status| {
        status
            .light
            .as_ref()
            .is_some_and(|light| signs.backlighting_lights.contains(light))
    });
    if backlit_by_status {
        return Some((Lighting::Backlit, None));
    }
    Some(match square {
        SquareLight::Plain => (Lighting::Plain, None),
        SquareLight::Umbra => (Lighting::Umbra, None),
        SquareLight::Halo => match own_halo(visible, remembered, signs) {
            Some(true) => (Lighting::Plain, None),
            Some(false) => (Lighting::Backlit, None),
            None => (Lighting::Backlit, Some(NotModelled::OwnHalo)),
        },
    })
}

/// Whether the player has a halo of their own (`player::halo_radius`, `areas.cc:564`, of 0 or more): from their
/// god or a status, or from gear they carry that the character overview (`%`) last showed them
/// wearing or wielding. `None` when they carry such gear and the overview isn't Known, or the
/// inventory hasn't been sent.
fn own_halo(
    visible: &VisibleState,
    remembered: Option<&AccumulatedState>,
    signs: &LightingSigns,
) -> Option<bool> {
    let blessed = matches!(
        &visible.religion,
        Some(Religion::God { name, penance: false, .. }) if signs.halo_gods.contains(name)
    );
    let storm = visible.statuses.iter().flatten().any(|status| {
        status
            .text
            .as_ref()
            .is_some_and(|text| signs.halo_statuses.contains(text))
    });
    if blessed || storm {
        return Some(true);
    }
    let halo_gear: Vec<char> = visible
        .inventory
        .as_ref()?
        .iter()
        .filter(|item| {
            let name = item.name.to_lowercase();
            signs.halo_gear.iter().any(|gear| name.contains(gear))
        })
        .map(|item| item.letter)
        .collect();
    if halo_gear.is_empty() {
        return Some(false);
    }
    let equipped = remembered?.equipped.known()?;
    Some(halo_gear.iter().any(|letter| equipped.contains(letter)))
}

/// The spells of `attacker`'s book the simulation doesn't play out, by name: its Game
/// Knowledge's name, or crawl's identifier for a spell the bundle lacks.
fn spells_not_modelled(bundle: &Bundle, attacker: &Attacker<'_>) -> Vec<String> {
    let MonsterSpells::Book { spells } = &attacker.monster.spells else {
        return Vec::new();
    };
    spells
        .iter()
        .filter(|slot| {
            !attacker
                .spells
                .iter()
                .any(|cast| cast.spell.spell.id == slot.spell)
        })
        .map(|slot| {
            bundle
                .spell_by_id(&slot.spell)
                .map_or_else(|| slot.spell.0.clone(), |spell| spell.name.clone())
        })
        .collect()
}

/// The Threat Level for a chance of death and a worst-case hit against `hp`.
fn level(hp: i32, death: Chance, worst_hit: i32) -> ThreatLevel {
    if death.at_least_per_mille(LETHAL_DEATHS_PER_MILLE) || worst_hit >= hp {
        ThreatLevel::Lethal
    } else if death.at_least_per_mille(DANGEROUS_DEATHS_PER_MILLE)
        || worst_hit * 100 >= hp * DANGEROUS_WORST_HIT_PERCENT_OF_HP
    {
        ThreatLevel::Dangerous
    } else {
        ThreatLevel::Calm
    }
}

/// Squares between two positions, counting a diagonal step as one: crawl's `grid_distance`.
fn distance(a: Position, b: Position) -> i32 {
    (a.x - b.x).abs().max((a.y - b.y).abs())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chance(times: u32) -> Chance {
        Chance {
            times,
            trials: 10_000,
        }
    }

    #[test]
    fn a_five_percent_chance_of_death_is_lethal_and_just_under_it_dangerous() {
        assert_eq!(level(100, chance(500), 0), ThreatLevel::Lethal);
        assert_eq!(level(100, chance(499), 0), ThreatLevel::Dangerous);
    }

    #[test]
    fn a_half_percent_chance_of_death_is_dangerous_and_just_under_it_calm() {
        assert_eq!(level(100, chance(50), 0), ThreatLevel::Dangerous);
        assert_eq!(level(100, chance(49), 0), ThreatLevel::Calm);
    }

    #[test]
    fn nothing_able_to_attack_is_calm() {
        assert_eq!(level(1, Chance::NONE, 0), ThreatLevel::Calm);
    }

    #[test]
    fn a_worst_hit_of_all_hp_is_lethal_and_of_forty_percent_dangerous() {
        assert_eq!(level(50, chance(0), 50), ThreatLevel::Lethal);
        assert_eq!(level(50, chance(0), 49), ThreatLevel::Dangerous);
        assert_eq!(level(50, chance(0), 20), ThreatLevel::Dangerous);
        assert_eq!(level(50, chance(0), 19), ThreatLevel::Calm);
    }

    /// The Not Modelled ledger: one entry for each thing the engine leaves out.
    const LEDGER: &str = include_str!("../../../docs/not-modelled.md");

    /// The field of a ledger entry that says how the Fact Sheet surfaces it.
    const SURFACED_AS: &str = "- **Surfaced as:** ";

    /// The variants named, checked by an exhaustive match so that a new variant doesn't compile
    /// until it is listed here too.
    macro_rules! every_not_modelled {
        ($($variant:ident),+ $(,)?) => {{
            fn _exhaustive(not_modelled: NotModelled) {
                match not_modelled {
                    $(NotModelled::$variant)|+ => {}
                }
            }
            [$(NotModelled::$variant),+]
        }};
    }

    fn every_variant_name() -> BTreeSet<String> {
        every_not_modelled![
            MonsterEquipment,
            AttackFlavours,
            Statuses,
            Paths,
            Awareness,
            OtherSpells,
            SpellsPerMonster,
            DraconianBreath,
            PlayerResistances,
            CasterSafety,
            CarriedGear,
            OtherWands,
            WornShield,
            OwnHalo,
            PlayerSpeed,
            SwimmingAndFlying,
            UnexploredSquares,
            HitsOnTheWay,
            OwnConditions,
        ]
        .into_iter()
        .map(|variant| {
            serde_json::to_value(variant)
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect()
    }

    /// The variant each ledger entry names, by its serialized name, from the start of its
    /// "Surfaced as" field. Fails unless every entry has that field, and an entry without a
    /// variant says it is an accepted simplification or harness only.
    fn ledger_variant_names() -> Vec<&'static str> {
        let fields: Vec<&str> = LEDGER
            .lines()
            .filter_map(|line| line.strip_prefix(SURFACED_AS))
            .collect();
        let entries = LEDGER
            .lines()
            .filter(|line| line.starts_with("### "))
            .count();
        assert_eq!(
            fields.len(),
            entries,
            "each entry has one Surfaced as field"
        );
        fields
            .into_iter()
            .filter_map(|field| match field.strip_prefix('`') {
                Some(named) => named.split('`').next(),
                None => {
                    assert!(
                        field.starts_with("accepted simplification")
                            || field.starts_with("harness only"),
                        "an entry without a variant says why: {field}"
                    );
                    None
                }
            })
            .collect()
    }

    #[test]
    fn every_not_modelled_variant_has_a_ledger_entry() {
        let in_ledger: BTreeSet<&str> = ledger_variant_names().into_iter().collect();
        for name in every_variant_name() {
            assert!(
                in_ledger.contains(name.as_str()),
                "docs/not-modelled.md has no entry for `{name}`"
            );
        }
    }

    #[test]
    fn every_variant_the_ledger_names_exists_once() {
        let variants = every_variant_name();
        let mut seen = BTreeSet::new();
        for name in ledger_variant_names() {
            assert!(
                variants.contains(name),
                "docs/not-modelled.md names `{name}`, which isn't a NotModelled variant"
            );
            assert!(
                seen.insert(name),
                "docs/not-modelled.md names `{name}` twice"
            );
        }
    }
}

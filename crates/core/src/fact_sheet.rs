//! The Fact Sheet: everything the Advisor has computed about one moment, and the Gaps that kept
//! it from computing more.

use std::collections::BTreeSet;

use copilot_gk::{Bundle, MonsterSpells};
use serde::Serialize;
use specta::Type;

use crate::accumulated_state::AccumulatedState;
use crate::engine::lighting::Lighting;
use crate::engine::melee::DisplayedDefences;
use crate::engine::spells::{DamagingSpell, Reach};
use crate::engine::threat::{self, Attacker, Defender, Outcome};
use crate::visible_state::{
    Attitude, Position, Religion, SquareLight, VisibleMonster, VisibleState,
};

/// LETHAL from this chance of death before the player's next action, per mille: 5%. A starting
/// threshold from the spec (#24, "Engine (ADR 0003)"), to be tuned on the corpus.
const LETHAL_DEATHS_PER_MILLE: u32 = 50;

/// DANGEROUS from this chance of death, per mille: 0.5%. A starting threshold from the spec (#24,
/// "Engine (ADR 0003)"), to be tuned on the corpus.
const DANGEROUS_DEATHS_PER_MILLE: u32 = 5;

/// DANGEROUS when the worst-case hit is at least this percentage of current HP. A starting
/// threshold from the spec (#24, "Engine (ADR 0003)"), to be tuned on the corpus.
const DANGEROUS_WORST_HIT_PERCENT_OF_HP: i32 = 40;

/// The status lights of what backlights the player, whatever halo or umbra they stand in: a
/// corona, liquid fire, quad damage and harmful contamination, as `player::backlit` reads them
/// (`player.cc`). Crawl lights `Contam` only once contamination is harmful, and liquid fire as
/// `Fire`, `Fire+` or `Fire++` by how hot it burns (`status.cc`).
const BACKLIGHTING_LIGHTS: [&str; 4] = ["Corona", "Fire", "Quad", "Contam"];

/// The god whose worshippers carry a halo of their own while not under penance
/// (`passive_t::halo`, from the moment they join), as the game names it.
const HALO_GOD: &str = "the Shining One";

/// The status a halo of the player's own shows as: Wu Jian's heavenly storm.
const HALO_STATUS: &str = "in a heavenly storm";

/// What the names of items that give their wearer or wielder a halo hold, in lower case: the
/// morningstar "Eos", the crown of Vainglory and armour of light (`player::halo_radius`).
const HALO_GEAR: [&str; 3] = ["\"eos\"", "vainglory", "of light"];

/// The genus crawl gives a breath outside its book, by colour: `drac_breath` in `mon-util.cc`.
const DRACONIAN_GENUS: &str = "MONS_DRACONIAN";

/// Everything the Advisor has computed about one moment of the game it follows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FactSheet {
    /// How dangerous this moment is.
    pub threat: Threat,
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
    /// The hostiles in view, acting or not, whose kind crawl may make with a launcher, throwing
    /// missiles or a wand. The simulation plays out what crawl may make their kind with, by its
    /// odds, rather than what each carries, which the Advisor can't see (#83).
    pub may_shoot: Vec<MayShoot>,
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
    /// The launchers, throwing missiles and wands crawl may make its kind with that the
    /// simulation plays out, by name, such as `shortbow` or `wand of flame`.
    pub shoots: Vec<String>,
}

/// A hostile in view whose kind crawl may make with something to shoot, throw or zap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MayShoot {
    /// Its name, as the game names it.
    pub name: String,
    /// The square it is on.
    pub position: Position,
    /// Squares between it and the player.
    pub distance: i32,
}

/// Something the Threat Level's simulation doesn't model.
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
    /// Spells land as if the player resists nothing, because the Advisor doesn't read the
    /// player's resistances yet.
    PlayerResistances,
    /// Monsters are assumed to cast explosions and clouds even when they'd catch themselves;
    /// crawl's casters weigh that, and often hold off up close.
    CasterSafety,
    /// A monster in view whose kind may carry a launcher, missiles or a wand is judged by the odds
    /// crawl makes its kind with, its launcher or missiles +0 and unbranded, as the Advisor can't
    /// see what it carries (#83).
    CarriedGear,
    /// A monster in reach may carry a wand the simulation doesn't play out, such as paralysis or
    /// polymorph: it is assumed never to zap it, and to shoot or fight instead.
    OtherWands,
    /// The player's shield is assumed to block at most one attack before their next action, as
    /// without a shield, because the Advisor doesn't know which shield is worn. The character
    /// overview (`%`) tells it.
    WornShield,
    /// The player stands in a halo and may have one of their own, which keeps another's from
    /// backlighting them, from gear the Advisor can't tell they wear, such as the morningstar
    /// "Eos" or armour of light. They are assumed to have none, and to be backlit.
    OwnHalo,
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
    /// Where the player stands hasn't been reported.
    Position,
    /// There is no Game Knowledge for the game's Crawl Version, so monsters can't be judged.
    GameKnowledge,
    /// A monster in view isn't in the Game Knowledge for the game's Crawl Version.
    MonsterKnowledge {
        /// The monster, as the game names it.
        monster: String,
    },
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
        }
    }
}

/// Monsters on these sides attack the player: crawl's `mons_att_wont_attack` (`mon-util.cc`)
/// spares only friends, good neutrals and marionettes, and strict neutrals don't hunt the player.
fn attacks_player(attitude: Attitude) -> bool {
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
            may_shoot: Vec::new(),
            lighting: visible.position.map(|player| lighting(visible, player).0),
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
    if hostile
        .iter()
        .any(|(_, monster)| monster.gear.may_shoot_or_zap())
    {
        not_modelled.push(NotModelled::CarriedGear);
    }
    let (lighting, own_halo) = lighting(visible, player);
    not_modelled.extend(own_halo);
    let defender = Defender {
        hp,
        defences: DisplayedDefences { ac, ev, sh },
        lighting,
        shield_blocks: threat::shield_blocks_per_action(formulas, worn_shield.flatten()),
    };
    let hostiles_in_view = u32::try_from(hostile.len()).unwrap_or(u32::MAX);
    let may_shoot: Vec<MayShoot> = hostile
        .iter()
        .filter(|(_, monster)| monster.gear.may_shoot_or_zap())
        .map(|(seen, _)| MayShoot {
            name: seen.name.clone(),
            position: seen.position,
            distance: distance(player, seen.position),
        })
        .collect();
    // The monsters that can attack, each with the spells of its book not played out.
    let mut acting: Vec<(&VisibleMonster, Vec<String>)> = Vec::new();
    let mut attackers: Vec<Attacker<'_>> = Vec::new();
    for (seen, monster) in hostile {
        let attacker = Attacker::new(bundle, monster, distance(player, seen.position));
        if threat::can_attack(formulas, &attacker) {
            acting.push((seen, spells_not_modelled(bundle, &attacker)));
            attackers.push(attacker);
        }
    }
    if acting.iter().any(|(_, others)| !others.is_empty()) {
        not_modelled.push(NotModelled::OtherSpells);
    }
    if attackers
        .iter()
        .any(|attacker| wands_not_modelled(bundle, attacker))
    {
        not_modelled.push(NotModelled::OtherWands);
    }
    if attackers
        .iter()
        .any(|attacker| !attacker.spells.is_empty() || !attacker.wands.is_empty())
    {
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
                shoots: shoots(attacker),
            })
            .collect(),
        may_shoot,
        lighting: Some(lighting),
        not_modelled,
    }))
}

/// How the light falls on the player standing at `player`, as crawl's `player::backlit(false)`
/// and `player::umbra()` decide it from what the game shows: a status that backlights them, or
/// the halo or umbra on their square. A halo backlights them only if they have none of their
/// own; with [`NotModelled::OwnHalo`] beside it when the Advisor can't tell.
fn lighting(visible: &VisibleState, player: Position) -> (Lighting, Option<NotModelled>) {
    let lights = |names: &[&str]| {
        visible.statuses.iter().flatten().any(|status| {
            status
                .light
                .as_deref()
                .is_some_and(|light| names.contains(&light.trim_end_matches('+')))
        })
    };
    if lights(&BACKLIGHTING_LIGHTS) {
        return (Lighting::Backlit, None);
    }
    let square = visible
        .map
        .as_ref()
        .and_then(|map| map.cell(player))
        .map_or(SquareLight::Plain, |cell| cell.light);
    match square {
        SquareLight::Plain => (Lighting::Plain, None),
        SquareLight::Umbra => (Lighting::Umbra, None),
        SquareLight::Halo => match own_halo(visible) {
            Some(true) => (Lighting::Plain, None),
            Some(false) => (Lighting::Backlit, None),
            None => (Lighting::Backlit, Some(NotModelled::OwnHalo)),
        },
    }
}

/// Whether the player has a halo of their own (`player::halo_radius` of 0 or more), `None` when
/// they may from gear the Advisor can't tell they wear, or carry.
fn own_halo(visible: &VisibleState) -> Option<bool> {
    let shining = matches!(
        &visible.religion,
        Some(Religion::God { name, penance: false, .. }) if name.eq_ignore_ascii_case(HALO_GOD)
    );
    let storm = visible
        .statuses
        .iter()
        .flatten()
        .any(|status| status.text.as_deref() == Some(HALO_STATUS));
    if shining || storm {
        return Some(true);
    }
    let inventory = visible.inventory.as_ref()?;
    let halo_gear = inventory.iter().any(|item| {
        let name = item.name.to_lowercase();
        HALO_GEAR.iter().any(|gear| name.contains(gear))
    });
    (!halo_gear).then_some(false)
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

/// The launchers, missiles and wands of `attacker`'s kind that the simulation plays out, by
/// name.
fn shoots(attacker: &Attacker<'_>) -> Vec<String> {
    attacker
        .missiles
        .items()
        .map(|missile| missile.weapon.name.clone())
        .chain(
            attacker
                .wands
                .items()
                .map(|zap| format!("wand of {}", zap.wand.name)),
        )
        .collect()
}

/// Whether `attacker`'s kind may carry a wand whose spell the simulation doesn't play out.
fn wands_not_modelled(bundle: &Bundle, attacker: &Attacker<'_>) -> bool {
    attacker.monster.gear.wand.items.iter().any(|weighted| {
        weighted.weight > 0
            && bundle
                .wands
                .get(&weighted.item)
                .is_none_or(|wand| !attacker.wands.items().any(|zap| zap.wand == wand))
    })
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
    use crate::view_model::{Colour, Glyph};
    use crate::visible_state::{InventoryItem, LevelMap, MapCell, Status};

    const PLAYER: Position = Position { x: 5, y: 5 };

    /// A player at [`PLAYER`], whose square is lit by `light`, showing `lights` among their
    /// statuses and carrying nothing.
    fn standing_in(light: SquareLight, lights: &[&str]) -> VisibleState {
        let square = MapCell {
            glyph: Glyph {
                text: "@".to_owned(),
                colour: Colour::White,
            },
            feature: None,
            in_view: true,
            light,
        };
        VisibleState {
            position: Some(PLAYER),
            map: Some(LevelMap::new([(PLAYER, square)])),
            statuses: Some(
                lights
                    .iter()
                    .map(|&light| Status {
                        light: Some(light.to_owned()),
                        text: None,
                        colour: None,
                    })
                    .collect(),
            ),
            inventory: Some(Vec::new()),
            ..VisibleState::default()
        }
    }

    fn worshipping(god: &str, penance: bool) -> Option<Religion> {
        Some(Religion::God {
            name: god.to_owned(),
            piety_rank: Some(3),
            penance,
        })
    }

    #[test]
    fn a_corona_liquid_fire_quad_damage_or_harmful_contamination_backlights_the_player() {
        for light in BACKLIGHTING_LIGHTS {
            for square in [SquareLight::Plain, SquareLight::Umbra] {
                let visible = standing_in(square, &[light]);
                assert_eq!(
                    lighting(&visible, PLAYER),
                    (Lighting::Backlit, None),
                    "{light} on a {square:?} square"
                );
            }
        }
        for light in ["Fire+", "Fire++"] {
            let visible = standing_in(SquareLight::Plain, &[light]);
            assert_eq!(
                lighting(&visible, PLAYER),
                (Lighting::Backlit, None),
                "{light}"
            );
        }
        let visible = standing_in(SquareLight::Plain, &["Pois", "Slow"]);
        assert_eq!(lighting(&visible, PLAYER), (Lighting::Plain, None));
    }

    #[test]
    fn a_halo_backlights_a_player_without_one_of_their_own() {
        let visible = standing_in(SquareLight::Halo, &[]);
        assert_eq!(lighting(&visible, PLAYER), (Lighting::Backlit, None));
    }

    #[test]
    fn a_halo_doesnt_backlight_a_follower_of_the_shining_one_unless_under_penance() {
        let visible = VisibleState {
            religion: worshipping("the Shining One", false),
            ..standing_in(SquareLight::Halo, &[])
        };
        assert_eq!(lighting(&visible, PLAYER), (Lighting::Plain, None));
        let visible = VisibleState {
            religion: worshipping("the Shining One", true),
            ..standing_in(SquareLight::Halo, &[])
        };
        assert_eq!(lighting(&visible, PLAYER), (Lighting::Backlit, None));
        let visible = VisibleState {
            religion: worshipping("Zin", false),
            ..standing_in(SquareLight::Halo, &[])
        };
        assert_eq!(lighting(&visible, PLAYER), (Lighting::Backlit, None));
    }

    #[test]
    fn a_halo_doesnt_backlight_a_player_in_a_heavenly_storm() {
        let mut visible = standing_in(SquareLight::Halo, &[]);
        visible.statuses = Some(vec![Status {
            light: None,
            text: Some("in a heavenly storm".to_owned()),
            colour: None,
        }]);
        assert_eq!(lighting(&visible, PLAYER), (Lighting::Plain, None));
    }

    #[test]
    fn a_player_who_may_wear_a_halo_or_whose_items_arent_known_is_assumed_backlit_and_told() {
        let carrying = |name: &str| VisibleState {
            inventory: Some(vec![InventoryItem {
                letter: 'a',
                name: name.to_owned(),
                quantity: 1,
                kind: None,
            }]),
            ..standing_in(SquareLight::Halo, &[])
        };
        for name in [
            "the +2 morningstar \"Eos\" (weapon)",
            "the +2 crown of Vainglory",
            "a +1 leather armour of light",
        ] {
            assert_eq!(
                lighting(&carrying(name), PLAYER),
                (Lighting::Backlit, Some(NotModelled::OwnHalo)),
                "{name}"
            );
        }
        assert_eq!(
            lighting(&carrying("a +0 leather armour (worn)"), PLAYER),
            (Lighting::Backlit, None)
        );
        let unknown = VisibleState {
            inventory: None,
            ..standing_in(SquareLight::Halo, &[])
        };
        assert_eq!(
            lighting(&unknown, PLAYER),
            (Lighting::Backlit, Some(NotModelled::OwnHalo))
        );
    }

    #[test]
    fn an_umbra_hides_the_player_and_a_plain_square_neither_lights_nor_hides_them() {
        let visible = standing_in(SquareLight::Umbra, &[]);
        assert_eq!(lighting(&visible, PLAYER), (Lighting::Umbra, None));
        let visible = standing_in(SquareLight::Plain, &[]);
        assert_eq!(lighting(&visible, PLAYER), (Lighting::Plain, None));
    }

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
}

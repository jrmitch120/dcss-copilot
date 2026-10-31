//! The fight outcome's Monte Carlo: the player fighting the hostiles in view by one Attack Plan,
//! action by action, and the hostiles fighting back as the Threat Level plays them, through
//! crawl's own dice, until one side falls, many times over.
//!
//! The player stands where they are: they attack the nearest hostile, wait for one that walks up
//! to come into reach, and step toward one that never moves or keeps its range. Each of their
//! actions takes as long as crawl makes it take, and every hostile then gains the energy that
//! time gives it and acts.

use copilot_gk::{EnumId, Formulas, MeleeWeapon, MissileWeapon, SpellRange};

use super::melee::BASELINE_DELAY;
use super::missiles::MISSILE_RANGE;
use super::player_attacks::{
    self, AttackSpell, Defence, DrawnSkill, Hand, Heft, PlayerZap, Shot, Skill, Striker,
};
use super::rng::Rng;
use super::threat::{Attacker, Defender, Engaged};
use super::{spells, step};

/// How many fights each Attack Plan plays out. At 2,000 a win chance near 90% is within about 0.7
/// percentage points of its true value, enough to tell most plans from the DANGEROUS line, at a
/// cost the Fact Sheet can pay for several plans on every update.
pub(crate) const TRIALS: u32 = 2_000;

/// The longest fight played out, in the player's actions: one that neither side has won by then
/// counts as not won.
pub(crate) const MOST_ACTIONS: u32 = 100;

/// The seed every plan's fights start from: the same for each plan, so plans are compared on the
/// same rolls of the dice where they can be, and the same Game Situation always gives the same
/// Fact Sheet.
const SEED: (u64, u64) = (0x00dc_55c0_9170_7036, 0x0000_0000_0000_0036);

/// A hostile in view as the fight plays it: how it acts, as the Threat Level plays it, and how it
/// meets the player's attacks.
#[derive(Debug, Clone)]
pub(crate) struct Foe<'a> {
    /// It acting, with the gear it is played with.
    pub(crate) attacker: Attacker<'a>,
    /// Its armour, evasion and resistances.
    pub(crate) defence: Defence,
}

/// The player as the fight plays them, before any plan: their hit points and magic, Strength and
/// Dexterity, the skills they fight with, and what they attack with when nothing else is left.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Fighter<'a> {
    /// Hit points.
    pub(crate) hp: i32,
    /// Magic points.
    pub(crate) mp: i32,
    /// Strength.
    pub(crate) strength: i32,
    /// Dexterity.
    pub(crate) dexterity: i32,
    /// Fighting.
    pub(crate) fighting: Skill,
    /// Evocations, which a wand's power rises with.
    pub(crate) evocations: Skill,
    /// What they attack with once a plan's opening is spent, or from the start.
    pub(crate) base: Base<'a>,
}

/// What the player attacks with when no opening is left: what they hold.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Base<'a> {
    /// Their fists, trained by Unarmed Combat.
    Unarmed {
        /// Unarmed Combat.
        skill: Skill,
    },
    /// A melee weapon.
    Weapon {
        /// Its kind.
        weapon: &'a MeleeWeapon,
        /// Its enchantment.
        plus: i32,
        /// Its brand, as far as damage and delay go.
        heft: Heft,
        /// The skill it trains.
        skill: Skill,
    },
    /// A launcher, which needs no ammunition.
    Launcher {
        /// Its kind.
        weapon: &'a MissileWeapon,
        /// Its kind's identifier.
        id: &'a EnumId,
        /// Its enchantment.
        plus: i32,
        /// Its brand, as far as damage and delay go.
        heft: Heft,
        /// Ranged Weapons.
        skill: Skill,
    },
    /// Something the engine can't attack with: the player only waits.
    Nothing,
}

/// What an Attack Plan opens with, until it runs out.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Opening<'a> {
    /// Casting a spell, while there is the magic for it.
    Spell {
        /// The spell.
        spell: AttackSpell<'a>,
        /// The power it is cast with.
        power: Power,
        /// The magic points each cast costs.
        cost: i32,
        /// The chance in a hundred of failing to cast it, which spends the magic all the same.
        failure_percent: i32,
        /// How many squares it reaches.
        range: i32,
    },
    /// Zapping a wand, while it has charges.
    Wand {
        /// The spell it zaps.
        zap: PlayerZap<'a>,
        /// How far the spell reaches, and the power past which it grows no stronger.
        range: SpellRange,
        /// Its charges.
        charges: u32,
    },
    /// Throwing missiles, while there are any.
    Thrown {
        /// Their kind.
        weapon: &'a MissileWeapon,
        /// Their kind's identifier.
        id: &'a EnumId,
        /// How many there are.
        count: u32,
        /// Throwing.
        skill: Skill,
    },
}

/// The power a spell is cast with, as the spell list shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Power {
    /// Exactly this.
    Exact(i32),
    /// Somewhere from the first to the second, any as likely as another: the powers that show as
    /// the same percentage of the spell's cap.
    Between(i32, i32),
}

impl Power {
    /// The power one fight plays it at.
    fn drawn(self, rng: &mut Rng) -> i32 {
        match self {
            Self::Exact(power) => power,
            Self::Between(least, most) => least + rng.random2(most - least + 1),
        }
    }
}

/// What playing out one Attack Plan many times showed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Outcome {
    /// Fights in which every hostile fell before the player did.
    pub(crate) wins: u32,
    /// Fights in which the player died.
    pub(crate) deaths: u32,
    /// Fights played out.
    pub(crate) trials: u32,
    /// The hit points the player lost, over every fight: all they had in a fight they died in.
    pub(crate) hp_lost: u64,
}

/// Plays out [`TRIALS`] fights of `fighter`, defended as `defender`, against `foes` by the plan
/// that opens with `opening`, if any, and falls back on the fighter's base attack.
pub(crate) fn simulate(
    formulas: Formulas,
    defender: Defender,
    fighter: &Fighter<'_>,
    opening: Option<&Opening<'_>>,
    foes: &[Foe<'_>],
) -> Outcome {
    // A monster that torments plays first, as in the Threat Level's trial.
    let mut order: Vec<&Foe<'_>> = foes.iter().collect();
    order.sort_by_key(|foe| !foe.attacker.monster.torments_as_it_dies);
    let mut rng = Rng::new(SEED.0, SEED.1);
    let mut outcome = Outcome {
        wins: 0,
        deaths: 0,
        trials: TRIALS,
        hp_lost: 0,
    };
    for _ in 0..TRIALS {
        let fight = one_fight(formulas, &mut rng, defender, fighter, opening, &order);
        outcome.wins += u32::from(fight.won);
        outcome.deaths += u32::from(fight.hp_left <= 0);
        let lost = fighter.hp - fight.hp_left.max(0);
        outcome.hp_lost += u64::try_from(lost.max(0)).unwrap_or(0);
    }
    outcome
}

/// How one fight ended.
struct Ending {
    /// Every hostile fell.
    won: bool,
    /// The player's hit points as it ended: 0 or less if they died.
    hp_left: i32,
}

/// The player in one fight: what of the plan's opening is left, and the skills and power drawn
/// for this fight.
struct Side<'f, 'a> {
    fighter: &'f Fighter<'a>,
    striker: Striker,
    hp: i32,
    mp: i32,
    opening: Option<Opening<'a>>,
    /// The opening's power this fight: a spell's, or a wand's from Evocations.
    power: i32,
    /// The opening's skill this fight: Throwing for thrown missiles.
    opening_skill: DrawnSkill,
    /// The base attack's skill this fight.
    base_skill: DrawnSkill,
}

/// A hostile in one fight.
struct Hostile<'e, 'a> {
    engaged: Engaged<'e, 'a>,
    defence: Defence,
    hp: i32,
}

impl Hostile<'_, '_> {
    fn alive(&self) -> bool {
        self.hp > 0 && !self.engaged.gone
    }
}

/// Plays out one fight.
fn one_fight<'a>(
    formulas: Formulas,
    rng: &mut Rng,
    defender: Defender,
    fighter: &Fighter<'a>,
    opening: Option<&Opening<'a>>,
    foes: &[&Foe<'a>],
) -> Ending {
    let target = defender.target(rng);
    let mut side = Side::drawn(formulas, rng, fighter, opening.copied());
    let mut hostiles: Vec<Hostile<'_, 'a>> = foes
        .iter()
        .map(|foe| Hostile {
            // A monster starts with the energy it has left, and gains more as the player acts.
            engaged: Engaged::new(formulas, rng, &foe.attacker, 0),
            defence: foe.defence,
            hp: player_attacks::monster_hit_points(
                formulas.monster_hit_points,
                rng,
                foe.attacker.monster.hp_10x,
            ),
        })
        .collect();
    for _ in 0..MOST_ACTIONS {
        if !hostiles.iter().any(Hostile::alive) {
            return Ending {
                won: true,
                hp_left: side.hp,
            };
        }
        let time = side.act(formulas, rng, &mut hostiles);
        let mut blocks_left = defender.shield_blocks;
        for hostile in hostiles.iter_mut().filter(|hostile| hostile.alive()) {
            hostile.engaged.gain(formulas, rng, time);
            let turn = hostile
                .engaged
                .act(formulas, rng, target, side.hp, &mut blocks_left);
            side.hp -= turn.damage;
            if side.hp <= 0 {
                return Ending {
                    won: false,
                    hp_left: side.hp,
                };
            }
        }
    }
    Ending {
        won: !hostiles.iter().any(Hostile::alive),
        hp_left: side.hp,
    }
}

impl<'f, 'a> Side<'f, 'a> {
    /// `fighter` as one fight plays them, with what of their skills and their spell's power the
    /// game doesn't show drawn afresh.
    fn drawn(
        formulas: Formulas,
        rng: &mut Rng,
        fighter: &'f Fighter<'a>,
        opening: Option<Opening<'a>>,
    ) -> Self {
        let striker = Striker {
            strength: fighter.strength,
            dexterity: fighter.dexterity,
            fighting: fighter.fighting.drawn(rng),
        };
        let base_skill = match fighter.base {
            Base::Unarmed { skill } | Base::Weapon { skill, .. } | Base::Launcher { skill, .. } => {
                skill.drawn(rng)
            }
            Base::Nothing => DrawnSkill::default(),
        };
        let (power, opening_skill) = match opening {
            Some(Opening::Spell { power, .. }) => (power.drawn(rng), DrawnSkill::default()),
            Some(Opening::Wand { range, .. }) => {
                let evocations = fighter.evocations.drawn(rng);
                let cap = range.power_cap;
                let power = player_attacks::wand_power(formulas.wand_power, evocations, cap);
                (power, DrawnSkill::default())
            }
            Some(Opening::Thrown { skill, .. }) => (0, skill.drawn(rng)),
            None => (0, DrawnSkill::default()),
        };
        Self {
            fighter,
            striker,
            hp: fighter.hp,
            mp: fighter.mp,
            opening,
            power,
            opening_skill,
            base_skill,
        }
    }

    /// Takes the player's next action against the nearest hostile, and returns how long it took.
    fn act(&mut self, formulas: Formulas, rng: &mut Rng, hostiles: &mut [Hostile<'_, '_>]) -> i32 {
        let Some(hostile) = hostiles
            .iter_mut()
            .filter(|hostile| hostile.alive())
            .min_by_key(|hostile| hostile.engaged.distance())
        else {
            return BASELINE_DELAY;
        };
        if let Some(time) = self.open(formulas, rng, hostile) {
            return time;
        }
        if let Some(time) = self.strike(formulas, rng, hostile) {
            return time;
        }
        // Nothing reaches it: wait for it to walk up, or step toward it if it never will. One
        // that keeps its range comes no nearer than that, out of reach.
        let monster = hostile.engaged.attacker().monster;
        let distance = hostile.engaged.distance();
        let comes = !monster.stationary
            && !monster.keeps_range
            && step::may_step(formulas.monster_step, monster.watched_step, distance);
        if !comes && distance > 1 {
            hostile.engaged.approached();
        }
        BASELINE_DELAY
    }

    /// The plan's opening at `hostile`, if any of it is left and it reaches: how long it took.
    fn open(
        &mut self,
        formulas: Formulas,
        rng: &mut Rng,
        hostile: &mut Hostile<'_, '_>,
    ) -> Option<i32> {
        let distance = hostile.engaged.distance();
        let variant = formulas.player_beam;
        match self.opening.as_mut()? {
            Opening::Spell {
                spell,
                cost,
                failure_percent,
                range,
                ..
            } => {
                if self.mp < *cost || distance > *range {
                    return None;
                }
                self.mp -= *cost;
                // A failed cast spends the magic and the time all the same.
                if !rng.x_chance_in_y(*failure_percent, 100) {
                    let blow =
                        player_attacks::cast(variant, rng, *spell, self.power, hostile.defence);
                    hostile.hp -= blow.damage;
                }
                Some(BASELINE_DELAY)
            }
            Opening::Wand {
                zap,
                range,
                charges,
            } => {
                // Crawl's `calc_spell_range` serves the player's wands as it does monsters'
                // spells.
                let reach = spells::range(formulas.monster_spell_power, *range, self.power);
                if *charges == 0 || distance > reach {
                    return None;
                }
                *charges -= 1;
                let blow = player_attacks::zapped(variant, rng, *zap, self.power, hostile.defence);
                hostile.hp -= blow.damage;
                Some(BASELINE_DELAY)
            }
            Opening::Thrown {
                weapon, id, count, ..
            } => {
                if *count == 0 || distance > MISSILE_RANGE {
                    return None;
                }
                let shot = Shot {
                    weapon,
                    id,
                    plus: 0,
                    heft: Heft::Plain,
                    skill: self.opening_skill,
                };
                let attack = formulas.player_attack;
                let blow = player_attacks::shoot(attack, rng, self.striker, shot, hostile.defence);
                // A boomerang comes back unless it breaks; anything else lies where it landed.
                if blow.broke || !player_attacks::returns(attack, id) {
                    *count -= 1;
                }
                hostile.hp -= blow.damage;
                Some(player_attacks::shot_delay(attack, rng, shot))
            }
        }
    }

    /// The player's base attack at `hostile`, if it reaches: how long it took.
    fn strike(
        &mut self,
        formulas: Formulas,
        rng: &mut Rng,
        hostile: &mut Hostile<'_, '_>,
    ) -> Option<i32> {
        let distance = hostile.engaged.distance();
        let attack = formulas.player_attack;
        let skill = self.base_skill;
        let (blow, time) = match self.fighter.base {
            Base::Unarmed { .. } | Base::Weapon { .. } => {
                let (hand, reach) = match self.fighter.base {
                    Base::Weapon {
                        weapon, plus, heft, ..
                    } => (
                        Hand::Weapon {
                            weapon,
                            plus,
                            heft,
                            skill,
                        },
                        weapon.reach,
                    ),
                    _ => (Hand::Unarmed { skill }, 1),
                };
                if distance > reach {
                    return None;
                }
                let blow = player_attacks::swing(attack, rng, self.striker, hand, hostile.defence);
                (blow, player_attacks::melee_delay(attack, rng, hand))
            }
            Base::Launcher {
                weapon,
                id,
                plus,
                heft,
                ..
            } => {
                if distance > MISSILE_RANGE {
                    return None;
                }
                let shot = Shot {
                    weapon,
                    id,
                    plus,
                    heft,
                    skill,
                };
                let blow = player_attacks::shoot(attack, rng, self.striker, shot, hostile.defence);
                (blow, player_attacks::shot_delay(attack, rng, shot))
            }
            Base::Nothing => return None,
        };
        hostile.hp -= blow.damage;
        Some(time)
    }
}

#[cfg(test)]
mod tests {
    use copilot_gk::{EnumId, Library};

    use super::*;
    use crate::engine::lighting::Lighting;
    use crate::engine::melee::DisplayedDefences;
    use crate::engine::resists::Resists;
    use crate::engine::step::Offset;

    #[test]
    fn the_player_steps_toward_a_monster_that_keeps_its_range_out_of_their_reach() {
        // An orb spider holds three squares from the player; slowed to speed 5, it backs off
        // too seldom to keep a fighter who steps toward it from reaching it.
        let library = Library::shipped().unwrap();
        let bundle = library.for_version("0.34.1").unwrap();
        let formulas = bundle.formulas;
        let mut spider = bundle
            .monsters
            .values()
            .find(|m| m.name == "orb spider")
            .unwrap()
            .clone();
        spider.speed = 5;
        let attacker = Attacker::new(bundle, &spider, Offset::east(3)).without_gear();
        assert_eq!(attacker.firing_range(formulas), Some(3));
        let foe = Foe {
            defence: player_attacks::Defence::of(formulas.player_beam, &spider),
            attacker,
        };
        let defender = Defender {
            hp: 100,
            defences: DisplayedDefences {
                ac: 10,
                ev: 10,
                sh: 0,
            },
            lighting: Lighting::Plain,
            shield_blocks: 1,
            resists: Resists::NONE,
        };
        let fighter = Fighter {
            hp: 100,
            mp: 0,
            strength: 20,
            dexterity: 12,
            fighting: Skill::shown(150),
            evocations: Skill::shown(0),
            base: Base::Weapon {
                weapon: &bundle.melee_weapons[&EnumId("WPN_WAR_AXE".to_owned())],
                plus: 0,
                heft: Heft::Plain,
                skill: Skill::shown(150),
            },
        };
        let outcome = simulate(formulas, defender, &fighter, None, &[foe]);
        assert!(outcome.wins > outcome.trials / 2, "{outcome:?}");
        assert_eq!(outcome.hp_lost, 0, "{outcome:?}");
    }
}

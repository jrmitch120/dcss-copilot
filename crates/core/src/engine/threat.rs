//! The Threat Level's Monte Carlo: every monster that can reach the player before their next
//! standard-speed action, casting its spells, zapping its wand, shooting and fighting in melee
//! through crawl's own dice, many times over.

use copilot_gk::{Bundle, EnumId, Formulas, Monster, MonsterSpells, Odds, Wand};

use crate::accumulated_state::Shield;

use super::melee::{
    self, Attack, Defences, ENERGY_THRESHOLD, attack_reach, energy_gained, starting_energy,
};
use super::missiles::{self, MISSILE_RANGE, Missile, Shooter};
use super::rng::Rng;
use super::spells::{self, DamagingSpell, Prospect, Reach, Slot};
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
    pub(crate) defences: Defences,
    /// The most attacks the shield blocks before the player's next action.
    pub(crate) shield_blocks: u32,
}

/// The most attacks the player's shield blocks before their next action, wearing `shield`: the
/// fewest, as without one, when it isn't known.
pub(crate) fn shield_blocks_per_action(formulas: Formulas, shield: Option<Shield>) -> u32 {
    melee::shield_block_limit(formulas.shield_block, shield)
}

/// A monster that may attack the player, how far away it is, and the spells, wands and missiles
/// it may use that the engine ports.
#[derive(Debug, Clone)]
pub(crate) struct Attacker<'a> {
    /// Its Game Knowledge.
    pub(crate) monster: &'a Monster,
    /// Squares between it and the player, counting diagonals as one, as crawl's `grid_distance`.
    pub(crate) distance: i32,
    /// The damaging spells of its book that the engine ports, in book order, as it casts them.
    pub(crate) spells: Vec<Cast<'a>>,
    /// The launcher or throwing missile it may carry, by the odds crawl makes its kind with.
    pub(crate) missiles: Draw<Missile<'a>>,
    /// The wands it may carry whose spells the engine ports, by the odds crawl makes its kind
    /// with. A wand whose spell isn't ported counts as none: it is assumed never to zap it.
    pub(crate) wands: Draw<WandZap<'a>>,
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

/// Which of some items a monster carries, drawn afresh for every trial by the odds crawl makes its
/// kind with: the Advisor can't see what it carries (#83).
#[derive(Debug, Clone)]
pub(crate) struct Draw<T> {
    out_of: u32,
    items: Vec<(T, u32)>,
}

impl<T: Copy> Draw<T> {
    /// The items of `odds`, each as `item` makes it: one it makes nothing of counts as none.
    pub(crate) fn new<'o>(odds: &'o Odds, mut item: impl FnMut(&'o EnumId) -> Option<T>) -> Self {
        Self {
            out_of: odds.out_of,
            items: odds
                .items
                .iter()
                .filter_map(|weighted| Some((item(&weighted.item)?, weighted.weight)))
                .collect(),
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
    /// of its kind the engine ports, looked up in `bundle`. A spell, missile or wand the bundle
    /// lacks is left out.
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
        let missiles = Draw::new(&monster.gear.missile, |id| {
            Some(Missile::of(
                formulas.missile_attack,
                id,
                bundle.missile_weapons.get(id)?,
            ))
        });
        let wands = Draw::new(&monster.gear.wand, |id| {
            let wand = bundle.wands.get(id)?;
            let spell = bundle.spell_by_id(&wand.spell)?;
            let damaging = spells::damaging(formulas.monster_spell_beam, spell)?;
            let power = wands::power(
                formulas.monster_wand,
                formulas.monster_spell_power,
                &spell.id,
                hit_dice,
            );
            let range = spells::range(formulas.monster_spell_power, spell.range?, power);
            Some(WandZap {
                wand,
                spell: damaging,
                power,
                reach: spells::reach(formulas.monster_spell_beam, &damaging, range),
            })
        });
        Self {
            monster,
            distance,
            spells,
            missiles,
            wands,
        }
    }

    /// The same monster, carrying nothing to shoot, throw or zap.
    pub(crate) fn without_gear(self) -> Self {
        Self {
            missiles: Draw::none(),
            wands: Draw::none(),
            ..self
        }
    }

    /// Whether it can strike the player from `distance` squares in melee, with a spell, a wand
    /// or a missile, if its kind may carry them.
    fn strikes_from(&self, formulas: Formulas, distance: i32) -> bool {
        let monster = self.monster;
        let shoots = !self.missiles.is_empty()
            && distance <= MISSILE_RANGE
            && (distance > 1 || monster.prefers_ranged);
        distance >= 1
            && (distance <= melee_reach(formulas, monster)
                || self
                    .spells
                    .iter()
                    .any(|cast| cast.reach.strikes >= distance)
                || self.wands.items().any(|zap| zap.reach.strikes >= distance)
                || shoots)
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
/// action left to strike the player in melee, with a spell or wand, or with a missile.
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

/// Plays out the player's next standard-speed action [`TRIALS`] times against `attackers`.
pub(crate) fn simulate(
    formulas: Formulas,
    defender: Defender,
    attackers: &[Attacker<'_>],
) -> Outcome {
    let mut rng = Rng::new(SEED.0, SEED.1);
    let mut totals: Vec<i32> = (0..TRIALS)
        .map(|_| one_action(formulas, &mut rng, defender, attackers))
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

/// The damage the player takes in one trial of their next action.
fn one_action(
    formulas: Formulas,
    rng: &mut Rng,
    defender: Defender,
    attackers: &[Attacker<'_>],
) -> i32 {
    let mut blocks_left = defender.shield_blocks;
    attackers
        .iter()
        .map(|attacker| turn(formulas, rng, attacker, defender.defences, &mut blocks_left).damage)
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

/// Plays out `attacker`'s actions before the player's next standard-speed action against a
/// player with `defences`, whose shield has `blocks_left` blocks left this action.
pub(crate) fn turn(
    formulas: Formulas,
    rng: &mut Rng,
    attacker: &Attacker<'_>,
    defences: Defences,
    blocks_left: &mut u32,
) -> Turn {
    let monster = attacker.monster;
    let reach = melee_reach(formulas, monster);
    let hit_dice = hit_dice(monster);
    let mut distance = attacker.distance;
    let mut energy = starting_energy(formulas.monster_energy, rng)
        + energy_gained(formulas.monster_energy, rng, speed(monster));
    let missile = attacker.missiles.draw(rng);
    let wand = attacker.wands.draw(rng);
    let shooter = Shooter::of(monster, hit_dice);
    let mut turn = Turn::default();
    // The actions left before it can breathe again.
    let mut breath_timeout: u32 = 0;
    let mut first_action = true;
    while energy >= ENERGY_THRESHOLD
        && (reach > 0 || !attacker.spells.is_empty() || missile.is_some() || wand.is_some())
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
                turn.damage += fire_beam(
                    formulas,
                    rng,
                    &cast.spell,
                    cast.power,
                    defences,
                    blocks_left,
                );
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
                turn.damage +=
                    fire_beam(formulas, rng, &zap.spell, zap.power, defences, blocks_left);
            }
            energy -= wands::energy(formulas.monster_wand);
            continue;
        }
        if let Some(missile) = missile
            && missiles::fires(
                formulas.monster_fire,
                rng,
                missile,
                monster.prefers_ranged,
                distance,
            )
        {
            let shot = missiles::shoot(formulas, rng, shooter, missile, defences, *blocks_left > 0);
            if shot.blocked {
                *blocks_left -= 1;
            }
            turn.damage += shot.damage;
            energy -=
                missiles::fire_energy(formulas.monster_fire, rng, cost(monster.energy.missile));
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
        // Every attack it has, as one melee action makes them
        // (`melee_attack::run_monster_attack_set`, `melee-attack.cc:1569`).
        // An attack only made with a weapon (`AT_WEAP_ONLY`) is counted, as the Advisor can't
        // see what monsters wield.
        for (_, attack) in attacks_from(formulas, monster, distance) {
            let swing = melee::swing(
                formulas,
                rng,
                hit_dice,
                monster.fighter,
                attack,
                defences,
                *blocks_left > 0,
            );
            if swing.blocked {
                *blocks_left -= 1;
            }
            turn.damage += swing.damage;
        }
        energy -= cost(monster.energy.attack);
    }
    turn
}

/// The damage a beam of `spell` at `power` deals the player, using up a block of their shield
/// when it blocks it.
fn fire_beam(
    formulas: Formulas,
    rng: &mut Rng,
    spell: &DamagingSpell<'_>,
    power: i32,
    defences: Defences,
    blocks_left: &mut u32,
) -> i32 {
    let bolt = spells::bolt(formulas.beam_damage, rng, spell, power);
    let shot = spells::shoot(
        formulas.beam_hit,
        formulas.beam_damage,
        rng,
        bolt,
        defences,
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

/// How far the monster's melee reaches: its farthest-reaching attack that deals damage, as
/// `monster::reach_range` (`monster.cc:6224`). 0 when it has no melee attack.
pub(crate) fn melee_reach(formulas: Formulas, monster: &Monster) -> i32 {
    attacks(monster)
        .filter(|(_, attack)| attack.damage > 0)
        .map(|(_, attack)| attack_reach(formulas.monster_reach, attack))
        .max()
        .unwrap_or(0)
}

/// The attacks the monster makes in one melee action from `distance` squares, in crawl's order,
/// each with its slot among the monster's attacks: none from beyond its reach.
pub(crate) fn attacks_from(
    formulas: Formulas,
    monster: &Monster,
    distance: i32,
) -> impl Iterator<Item = (usize, Attack)> + '_ {
    let in_reach = distance <= melee_reach(formulas, monster);
    attacks(monster).filter(move |&(_, attack)| {
        in_reach && melee::reaches_from(formulas.monster_reach, attack, distance)
    })
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

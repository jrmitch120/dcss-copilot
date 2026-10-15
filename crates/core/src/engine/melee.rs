//! Crawl's melee formulas for a monster attacking the player, ported closely (ADR 0002) behind
//! the formula variants a Game Knowledge bundle names.
//!
//! Each function cites the crawl source it ports. Line numbers are crawl 0.34.1's
//! (`crawl-ref/source/` at tag 0.34.1); trunk at 28a137de has the same code for all of them except
//! reach, where an attack carries its own reach (`mon_attack_def::reach`), which a later variant
//! will read. Only the paths a monster's plain melee attack on the player takes are ported: the
//! attacker wields nothing and has no statuses, and the player can see it. The Fact Sheet lists
//! what that leaves out.

use copilot_gk::{
    Armour, EvasionRoll, MonsterAttack, MonsterDamage, MonsterEnergy, MonsterReach, MonsterToHit,
    ShieldBlock,
};

use super::rng::Rng;

/// `AUTOMATIC_HIT` (`defines.h:141`): a to-hit this high never misses.
const AUTOMATIC_HIT: i32 = 1500;

/// `MIN_HIT_MISS_PERCENTAGE` (`defines.h:147`): the chance in 100 that an attack ignores
/// evasion, hitting or missing outright with equal odds.
const MIN_HIT_MISS_PERCENTAGE: i32 = 5;

/// `ENERGY_THRESHOLD` (`monster.cc:5353`): a monster acts while it has this much energy.
pub(crate) const ENERGY_THRESHOLD: i32 = 80;

/// `BASELINE_DELAY` (`defines.h:149`): the time a standard-speed player action takes.
const BASELINE_DELAY: i32 = 10;

/// A monster's accuracy for one attack: the `to_hit` that `attack::calc_to_hit` (`attack.cc:288`)
/// rolls for a monster.
///
/// The pre-roll value is `mon_to_hit_base` (`fight.cc:232`), via
/// `melee_attack::calc_mon_to_hit_base` (`melee-attack.cc:5087`) and the monster branch of
/// `attack::calc_pre_roll_to_hit` (`attack.cc:226`), with no weapon and no slaying. The
/// post-roll modifiers (`attack.cc:244`) are all zero for a visible, unconfused monster without
/// inaccuracy, so the monster's roll is `random2(mhit + 1)` (`attack.cc:300`).
pub(crate) fn monster_to_hit(
    variant: MonsterToHit,
    rng: &mut Rng,
    hit_dice: i32,
    fighter: bool,
) -> i32 {
    match variant {
        MonsterToHit::Crawl034 => {
            let hd_mult = if fighter { 5 } else { 3 };
            let mhit = 18 + hit_dice * hd_mult / 2;
            rng.random2(mhit + 1)
        }
    }
}

/// Whether an attack with this `to_land` gets past the player's evasion `ev`: the margin
/// `attack::test_hit` (`attack.cc:984`) returns, which hits when it is at least 0. A monster's
/// attack randomises the player's evasion (`melee-attack.cc:1898` passes `randomise_ev`).
pub(crate) fn evasion_margin(variant: EvasionRoll, rng: &mut Rng, to_land: i32, ev: i32) -> i32 {
    match variant {
        EvasionRoll::Crawl034 => {
            let ev = rng.random2avg(2 * ev, 2);
            if to_land >= AUTOMATIC_HIT {
                return AUTOMATIC_HIT;
            }
            if rng.x_chance_in_y(MIN_HIT_MISS_PERCENTAGE, 100) {
                if rng.random2(2) == 1 {
                    AUTOMATIC_HIT
                } else {
                    -AUTOMATIC_HIT
                }
            } else {
                to_land - ev
            }
        }
    }
}

/// Whether the player's shield blocks an attack: `attack::attack_shield_blocked`
/// (`attack.cc:1040`) for a monster attacking a player who can see it and isn't incapacitated.
///
/// The monster's bypass is `random2(mon_shield_bypass(hd))` (`fight.cc:237`, via
/// `monster::shield_bypass_ability`, `monster.cc:3096`); the player's block is
/// `player::shield_bonus` (`player.cc:6106`), from `player_shield_class`, which is twice the SH the
/// game displays (`player_displayed_shield_class`, `player.cc:2337`).
pub(crate) fn shield_blocks(
    variant: ShieldBlock,
    rng: &mut Rng,
    to_hit: i32,
    hit_dice: i32,
    displayed_sh: i32,
) -> bool {
    match variant {
        ShieldBlock::Crawl034 => {
            if to_hit >= AUTOMATIC_HIT {
                return false;
            }
            let con_block = rng.random2(15 + hit_dice * 2 / 3);
            let shield_class = displayed_sh * 2;
            let pro_block = if shield_class <= 0 {
                -100
            } else {
                rng.random2avg(shield_class * 2, 2) / 3 - 1
            };
            pro_block >= con_block
        }
    }
}

/// A monster attack's damage before armour, and the most it could have dealt, which sets the
/// player's guaranteed damage reduction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RolledDamage {
    /// The damage rolled.
    pub(crate) damage: i32,
    /// `damage_max`: the most this attack can deal.
    pub(crate) max: i32,
}

/// The damage of one monster attack that hit, before armour: the monster branch of
/// `attack::calc_damage` (`attack.cc:918`) with no weapon, `1 + random2(attk_damage)`.
///
/// `melee_attack::apply_damage_modifiers` (`melee-attack.cc:5098`) changes nothing for a monster
/// without statuses attacking an awake player. A crushing attack only grabs, and deals nothing
/// itself (`melee_attack::calc_damage`, `melee-attack.cc:5156`).
pub(crate) fn monster_damage(
    variant: MonsterDamage,
    rng: &mut Rng,
    attack: &MonsterAttack,
) -> RolledDamage {
    match variant {
        MonsterDamage::Crawl034 => {
            if attack.flavour.0 == "AF_CRUSH" {
                return RolledDamage { damage: 0, max: 0 };
            }
            let attk_damage = i32::try_from(attack.damage).unwrap_or(i32::MAX);
            RolledDamage {
                damage: 1 + rng.random2(attk_damage),
                max: attk_damage,
            }
        }
    }
}

/// What the player takes from `damage` after armour class `ac`: `actor::apply_ac`
/// (`actor.cc:343`) under the normal rule, with guaranteed damage reduction from
/// `player::gdr_perc` (`player.cc:6620`).
pub(crate) fn apply_ac(variant: Armour, rng: &mut Rng, damage: RolledDamage, ac: i32) -> i32 {
    match variant {
        Armour::Crawl034 => {
            let ac = ac.max(0);
            let gdr = gdr_percent(ac);
            let saved = rng.random2(1 + ac);
            let saved = saved.max((gdr * damage.max / 100).min(rng.div_rand_round(ac, 2)));
            (damage.damage - saved).max(0)
        }
    }
}

/// `player::gdr_perc` (`player.cc:6620`): `16 * AC^(1/4)`, truncated, as a percentage.
fn gdr_percent(ac: i32) -> i32 {
    // Truncation is crawl's `(int)` cast.
    #[allow(clippy::cast_possible_truncation)]
    let gdr = (16.0 * f64::from(ac.max(0)).sqrt().sqrt()) as i32;
    gdr
}

/// How many squares away this attack reaches: `melee_attack::can_reach` (`melee-attack.cc:97`)
/// for an unarmed monster, where an attack reaches past adjacent squares only when its flavour
/// does (`flavour_has_reach`, `mon-util.cc:2209`), and a rift reaches three
/// (`monster::reach_range`, `monster.cc:6224`).
pub(crate) fn attack_reach(variant: MonsterReach, attack: &MonsterAttack) -> i32 {
    match variant {
        MonsterReach::Crawl034 => match attack.flavour.0.as_str() {
            "AF_RIFT" => 3,
            "AF_REACH" | "AF_REACH_STING" | "AF_REACH_TONGUE" | "AF_REACH_CLEAVE_UGLY" => 2,
            _ => 1,
        },
    }
}

/// A monster's energy as the player starts an action: just below the threshold, as crawl leaves
/// a monster that has used its action (`monster::drain_action_energy`, `monster.cc:5361`).
pub(crate) fn starting_energy(variant: MonsterEnergy, rng: &mut Rng) -> i32 {
    match variant {
        MonsterEnergy::Crawl034 => ENERGY_THRESHOLD - rng.roll_dice(1, 10),
    }
}

/// The energy a monster of this speed gains while the player takes a standard-speed action:
/// `_monster_add_energy` (`mon-act.cc:1653`) with `you.time_taken` at `BASELINE_DELAY`.
pub(crate) fn energy_gained(variant: MonsterEnergy, rng: &mut Rng, speed: i32) -> i32 {
    match variant {
        MonsterEnergy::Crawl034 => {
            if speed > 0 {
                rng.div_rand_round(speed * BASELINE_DELAY, 10)
            } else {
                0
            }
        }
    }
}

/// The most energy a monster can have as the player starts an action.
pub(crate) fn most_starting_energy(variant: MonsterEnergy) -> i32 {
    match variant {
        MonsterEnergy::Crawl034 => ENERGY_THRESHOLD - 1,
    }
}

#[cfg(test)]
mod tests {
    use copilot_gk::EnumId;

    use super::*;

    const TRIALS: u32 = 200_000;

    fn rng() -> Rng {
        Rng::new(7, 11)
    }

    /// The share of `TRIALS` runs of `hits` that came out true.
    fn chance(mut hits: impl FnMut(&mut Rng) -> bool) -> f64 {
        let mut rng = rng();
        let count = (0..TRIALS).filter(|_| hits(&mut rng)).count();
        f64::from(u32::try_from(count).unwrap()) / f64::from(TRIALS)
    }

    fn attack(flavour: &str, damage: u32) -> MonsterAttack {
        MonsterAttack {
            kind: EnumId("AT_HIT".to_owned()),
            flavour: EnumId(flavour.to_owned()),
            damage,
        }
    }

    /// `mon_to_hit_pct(to_land, scaled_ev)` (`fight.cc:251`): crawl's own estimate of a monster's
    /// chance to hit the player, which its monster descriptions show. Ported here as an
    /// independent check on the rolls: it sums the evasion roll as two uniform halves, where the
    /// real roll is `random2avg`, so the two agree only to within a point or two.
    fn crawl_mon_to_hit_pct(to_land: i32, ev: i32) -> f64 {
        if ev <= 0 {
            return f64::from(100 - MIN_HIT_MISS_PERCENTAGE / 2);
        }
        let to_land = to_land + 1;
        let mut hits = 0;
        for ev1 in 0..ev {
            for ev2 in 0..ev {
                hits += (to_land - (ev1 + ev2)).max(0);
            }
        }
        let hit_chance = f64::from(hits) / f64::from(to_land * ev * ev);
        let auto = f64::from(MIN_HIT_MISS_PERCENTAGE) / 200.0;
        (hit_chance * (1.0 - auto) + (1.0 - hit_chance) * auto) * 100.0
    }

    #[test]
    fn a_monsters_chance_to_hit_agrees_with_crawls_own_estimate() {
        for (hd, fighter, ev) in [
            (1, false, 10),
            (5, true, 10),
            (10, false, 25),
            (3, false, 0),
        ] {
            let pre_roll = 18 + hd * if fighter { 5 } else { 3 } / 2;
            let ported = chance(|rng| {
                let to_hit = monster_to_hit(MonsterToHit::Crawl034, rng, hd, fighter);
                evasion_margin(EvasionRoll::Crawl034, rng, to_hit, ev) >= 0
            }) * 100.0;
            let crawl = crawl_mon_to_hit_pct(pre_roll, ev);
            assert!(
                (ported - crawl).abs() < 2.5,
                "HD {hd}, EV {ev}: ported {ported:.1}% vs crawl's estimate {crawl:.1}%"
            );
        }
    }

    #[test]
    fn one_attack_in_forty_hits_or_misses_regardless_of_evasion() {
        // Against EV 1000 only the automatic hit lands: 5% of attacks ignore evasion, half hit.
        let ported = chance(|rng| {
            let to_hit = monster_to_hit(MonsterToHit::Crawl034, rng, 1, false);
            evasion_margin(EvasionRoll::Crawl034, rng, to_hit, 1000) >= 0
        });
        assert!((ported - 0.025).abs() < 0.002, "{ported}");
    }

    #[test]
    fn without_a_shield_nothing_is_blocked() {
        assert!((0..1000).all(|_| !shield_blocks(ShieldBlock::Crawl034, &mut rng(), 10, 5, 0)));
    }

    /// `mon_beat_sh_pct(bypass, scaled_sh)` (`fight.cc:286`) for a whole shield class: crawl's
    /// own estimate of the chance in 100 that a monster's attack gets past the player's shield,
    /// shown in monster descriptions. Ported here as an independent check on the rolls. It counts
    /// `bypass - adj_sh` of the `bypass` bypass rolls as beating the shield, where the attack
    /// code beats it on only `bypass - adj_sh - 1` of them (a tie blocks, `attack.cc:1060`), so
    /// it overstates the chance by `100 / bypass` points.
    fn crawl_mon_beat_sh_pct(bypass: i32, shield_class: i32) -> f64 {
        let sh = shield_class * 2;
        let mut hits = 0;
        for sh1 in 0..sh {
            for sh2 in 0..sh {
                let adj_sh = (sh1 + sh2) / (3 * 2) - 1;
                hits += (bypass - adj_sh).max(0);
            }
        }
        f64::from(hits * 100) / f64::from(sh * sh * bypass)
    }

    #[test]
    fn a_shield_blocks_as_often_as_crawls_own_estimate_says() {
        for (hd, displayed_sh) in [(1, 3), (6, 5), (12, 15)] {
            let bypass = 15 + hd * 2 / 3;
            let estimate =
                100.0 - crawl_mon_beat_sh_pct(bypass, displayed_sh * 2) + 100.0 / f64::from(bypass);
            let ported =
                chance(|rng| shield_blocks(ShieldBlock::Crawl034, rng, 10, hd, displayed_sh))
                    * 100.0;
            assert!(
                (ported - estimate).abs() < 2.5,
                "HD {hd}, SH {displayed_sh}: ported {ported:.1}% vs crawl's estimate {estimate:.1}%"
            );
        }
    }

    #[test]
    fn a_monster_attack_deals_one_to_its_damage_before_armour() {
        let mut rng = rng();
        let rolls: Vec<_> = (0..10_000)
            .map(|_| monster_damage(MonsterDamage::Crawl034, &mut rng, &attack("AF_PLAIN", 5)))
            .collect();
        assert!(rolls.iter().all(|roll| roll.max == 5));
        assert_eq!(rolls.iter().map(|roll| roll.damage).min(), Some(1));
        assert_eq!(rolls.iter().map(|roll| roll.damage).max(), Some(5));
    }

    #[test]
    fn a_crushing_attack_deals_nothing_as_it_grabs() {
        let roll = monster_damage(MonsterDamage::Crawl034, &mut rng(), &attack("AF_CRUSH", 20));
        assert_eq!(roll, RolledDamage { damage: 0, max: 0 });
    }

    #[test]
    fn gdr_is_sixteen_times_the_fourth_root_of_ac() {
        // Worked by hand: 16 * 0^(1/4) = 0, 16 * 1 = 16, 16 * 16^(1/4) = 32, 16 * 81^(1/4) = 48,
        // 16 * 10^(1/4) = 28.45.
        assert_eq!([0, 1, 10, 16, 81].map(gdr_percent), [0, 16, 28, 32, 48]);
    }

    #[test]
    fn armour_saves_up_to_its_ac_and_at_least_its_guaranteed_reduction() {
        // AC 16: GDR 32%, so a 20-damage hit (max 20) always saves at least min(6, AC/2 = 8) = 6,
        // and at most 16.
        let mut rng = rng();
        let taken: Vec<_> = (0..20_000)
            .map(|_| {
                apply_ac(
                    Armour::Crawl034,
                    &mut rng,
                    RolledDamage {
                        damage: 20,
                        max: 20,
                    },
                    16,
                )
            })
            .collect();
        assert_eq!(taken.iter().max(), Some(&14));
        assert_eq!(taken.iter().min(), Some(&4));
    }

    #[test]
    fn reaching_flavours_reach_two_squares_and_a_rift_three() {
        let reach = |flavour| attack_reach(MonsterReach::Crawl034, &attack(flavour, 5));
        assert_eq!(reach("AF_PLAIN"), 1);
        assert_eq!(reach("AF_REACH"), 2);
        assert_eq!(reach("AF_REACH_STING"), 2);
        assert_eq!(reach("AF_RIFT"), 3);
    }

    #[test]
    fn a_monster_of_speed_fourteen_acts_twice_in_two_turns_out_of_five() {
        // Starting at 70..=79 and gaining 14, it acts once at 84..=93, and again only from 90.
        let twice = chance(|rng| {
            let energy = starting_energy(MonsterEnergy::Crawl034, rng)
                + energy_gained(MonsterEnergy::Crawl034, rng, 14);
            energy - 10 >= ENERGY_THRESHOLD
        });
        assert!((twice - 0.4).abs() < 0.005, "{twice}");
    }
}

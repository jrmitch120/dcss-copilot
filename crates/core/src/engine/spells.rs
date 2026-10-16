//! Crawl's formulas for a monster casting a spell at the player, ported closely (ADR 0002)
//! behind the formula variants a Game Knowledge bundle names: which spell it picks, the power
//! and range it casts with, and the beam that flies at the player.
//!
//! Line numbers are crawl 0.34.1's (`crawl-ref/source/` at tag 0.34.1). Only spells a monster
//! fires as one plain zap are ported, with the paths a cast at a player in plain view takes: the
//! caster has no statuses, is at full health and sees the player, and the player has no
//! resistances. The Fact Sheet lists what that leaves out.

use copilot_gk::{
    BeamDamage, BeamHit, EnumId, MonsterSpell, MonsterSpellBeam, MonsterSpellChoice,
    MonsterSpellPower, Spell, SpellRange, Zap, ZapDamage, ZapToHit,
};

use super::melee::{AUTOMATIC_HIT, Defences};
use super::rng::Rng;

/// `LOS_DEFAULT_RANGE` (`defines.h:115`): how far a player normally sees, which caps a spell's
/// range (`calc_spell_range`, `spl-util.cc:1068`).
const LOS_DEFAULT_RANGE: i32 = 7;

/// The spells crawl 0.34's `mons_spell_beam` (`mon-cast.cc:2249`) sets up as a plain zap with
/// `zappy`, that `mons_cast` (`mon-cast.cc:7540`) then fires once as a simple beam: no special
/// case in its switch and no spell logic of their own.
const PLAIN_ZAPS_0_34: &[&str] = &[
    "SPELL_ORB_OF_ELECTRICITY",
    "SPELL_MAGIC_DART",
    "SPELL_THROW_FLAME",
    "SPELL_THROW_FROST",
    "SPELL_VENOM_BOLT",
    "SPELL_POISON_ARROW",
    "SPELL_BOLT_OF_MAGMA",
    "SPELL_BOLT_OF_FIRE",
    "SPELL_BOLT_OF_COLD",
    "SPELL_THROW_ICICLE",
    "SPELL_SHOCK",
    "SPELL_LIGHTNING_BOLT",
    "SPELL_FIREBALL",
    "SPELL_ICEBLAST",
    "SPELL_LEHUDIBS_CRYSTAL_SPEAR",
    "SPELL_BOLT_OF_DRAINING",
    "SPELL_STICKY_FLAME",
    "SPELL_STING",
    "SPELL_IRON_SHOT",
    "SPELL_STONE_ARROW",
    "SPELL_FORCE_LANCE",
    "SPELL_CORROSIVE_BOLT",
    "SPELL_BOLT_OF_LIGHT",
    "SPELL_QUICKSILVER_BOLT",
    "SPELL_BLINKBOLT",
    "SPELL_STEAM_BALL",
    "SPELL_THROW_BOLAS",
    "SPELL_SOJOURNING_BOLT",
    "SPELL_NOXIOUS_CLOUD",
    "SPELL_THORN_VOLLEY",
    "SPELL_HURL_DAMNATION",
    "SPELL_SPIT_POISON",
    "SPELL_MIASMA_BREATH",
    "SPELL_GHOSTLY_FIREBALL",
    "SPELL_FLASH_FREEZE",
    "SPELL_REBOUNDING_BLAZE",
    "SPELL_REBOUNDING_CHILL",
    "SPELL_SPIT_LAVA",
    "SPELL_HURL_SLUDGE",
    "SPELL_HOLY_BREATH",
    "SPELL_SPIT_ACID",
    "SPELL_CAUSTIC_BREATH",
    "SPELL_ELECTRICAL_BOLT",
    "SPELL_STUNNING_BURST",
    "SPELL_BOLT_OF_DEVASTATION",
    "SPELL_CRYSTALLISING_SHOT",
    "SPELL_MAGMA_BARRAGE",
    "SPELL_DOOM_BOLT",
    "SPELL_ACID_BALL",
    "SPELL_FLAMING_CLOUD",
    "SPELL_CHAOS_BREATH",
    "SPELL_FREEZING_GUST",
];

/// One of a monster's spells that the engine ports: a zap that deals damage, fired once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DamagingSpell<'a> {
    /// The spell.
    pub(crate) spell: &'a Spell,
    /// The zap it fires.
    pub(crate) zap: &'a Zap,
    /// How often the monster picks it, of 200.
    pub(crate) frequency: u32,
    /// Its slot flags that decide when it can be picked.
    pub(crate) when: SlotRange,
    /// It is a breath (`MON_SPELL_BREATH`): once breathed, the monster can't breathe again until
    /// at least a turn has passed (`setup_breath_timeout`, `mon-cast.cc:5943`).
    pub(crate) breath: bool,
    /// It takes no time (`MON_SPELL_INSTANT`), so the monster acts again straight after.
    pub(crate) instant: bool,
    damage: ZapDamage,
    to_hit: ZapToHit,
}

/// At what distance from the player a spell slot can be picked: `_spell_flags_invalid_for_situation`
/// (`mon-cast.cc:4896`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SlotRange {
    /// At any distance.
    Any,
    /// `MON_SPELL_SHORT_RANGE`: only closer than half the player's sight.
    Short,
    /// `MON_SPELL_LONG_RANGE`: only farther than half the player's sight.
    Long,
    /// `MON_SPELL_EMERGENCY`: only when badly hurt, which a monster in full health never is.
    Emergency,
}

impl SlotRange {
    fn of(flags: &[EnumId]) -> Self {
        let has = |flag| has_flag(flags, flag);
        if has("MON_SPELL_EMERGENCY") {
            Self::Emergency
        } else if has("MON_SPELL_SHORT_RANGE") {
            Self::Short
        } else if has("MON_SPELL_LONG_RANGE") {
            Self::Long
        } else {
            Self::Any
        }
    }

    /// Whether a slot can be picked at `distance` from the player.
    fn allows(self, distance: i32) -> bool {
        let half_sight = LOS_DEFAULT_RANGE / 2;
        match self {
            Self::Any => true,
            Self::Short => distance < half_sight,
            Self::Long => distance > half_sight,
            Self::Emergency => false,
        }
    }
}

/// Whether the engine ports `spell` when a monster casts it: in `variant`'s plain zaps, firing a
/// zap that deals damage and has an accuracy.
pub(crate) fn is_ported(variant: MonsterSpellBeam, spell: &Spell) -> bool {
    let plain = match variant {
        MonsterSpellBeam::Crawl034 => PLAIN_ZAPS_0_34,
    };
    plain.contains(&spell.id.0.as_str())
        && spell.zap.as_ref().is_some_and(|zap| {
            !zap.enchantment && zap.monster_damage.is_some() && zap.monster_to_hit.is_some()
        })
}

/// Whether a spell slot's `flags` include `flag`, a `mon_spell_slot_flag` such as
/// `MON_SPELL_BREATH`.
fn has_flag(flags: &[EnumId], flag: &str) -> bool {
    flags.iter().any(|f| f.0 == flag)
}

/// `slot` as a spell the engine ports, if it is one ([`is_ported`]).
pub(crate) fn damaging<'a>(
    variant: MonsterSpellBeam,
    slot: &MonsterSpell,
    spell: &'a Spell,
) -> Option<DamagingSpell<'a>> {
    if !is_ported(variant, spell) {
        return None;
    }
    let zap = spell.zap.as_ref()?;
    let has = |flag| has_flag(&slot.flags, flag);
    Some(DamagingSpell {
        spell,
        zap,
        frequency: slot.frequency,
        when: SlotRange::of(&slot.flags),
        breath: has("MON_SPELL_BREATH"),
        instant: has("MON_SPELL_INSTANT"),
        damage: zap.monster_damage?,
        to_hit: zap.monster_to_hit?,
    })
}

/// The power a monster with `hit_dice` casts `spell` with: `mons_power_for_hd`
/// (`mon-cast.cc:2158`), its hit dice times the spell's factor (`_mons_power_hd_factor`,
/// `mon-cast.cc:2064`), as `mons_spellpower` gives it for a monster with no statuses.
pub(crate) fn power(variant: MonsterSpellPower, spell: &EnumId, hit_dice: i32) -> i32 {
    match variant {
        MonsterSpellPower::Crawl034 => {
            // The factors of `_mons_power_hd_factor` for the ported spells; every other is 12.
            let factor = match spell.0.as_str() {
                "SPELL_STICKY_FLAME" => 8,
                _ => 12,
            };
            hit_dice * factor
        }
    }
}

/// How many squares a spell cast with `power` reaches: `calc_spell_range` (`spl-util.cc:1068`),
/// capped by how far the player sees.
pub(crate) fn range(variant: MonsterSpellPower, range: SpellRange, power: i32) -> i32 {
    match variant {
        MonsterSpellPower::Crawl034 => {
            let reach = if range.min == range.max || range.power_cap <= power {
                range.max
            } else {
                (power * (range.max - range.min) + range.power_cap / 2) / range.power_cap
                    + range.min
            };
            reach.min(LOS_DEFAULT_RANGE)
        }
    }
}

/// How far from the player a monster casts a spell from, and how far its beam strikes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Reach {
    /// The farthest a monster casts it from.
    pub(crate) casts: i32,
    /// The farthest its beam strikes the player from.
    pub(crate) strikes: i32,
}

/// How far `spell`, with a range of `range` squares, reaches: as far as the tracer
/// `_should_cast_spell` fires (`mon-cast.cc:4980`) finds the player.
///
/// A bolt reaches its range. An explosion bursts where it stops, at the player or at the end of
/// its range, so it strikes its blast radius further (`bolt::refine_for_explosion`,
/// `beam.cc:6859`). A spell leaving a cloud of at least nine squares is cast at a player next to
/// where the cloud lands (`bolt::affect_endpoint`, `beam.cc:2583`, and `targeter_cloud`), though
/// its beam stops at its range.
///
/// The tracer also weighs harm to the caster against harm to the player (`mons_should_fire`),
/// which isn't ported: a monster is assumed to set off a blast or cloud even with itself inside.
pub(crate) fn reach(variant: MonsterSpellBeam, spell: &DamagingSpell<'_>, range: i32) -> Reach {
    match variant {
        MonsterSpellBeam::Crawl034 => {
            let id = spell.spell.id.0.as_str();
            if spell.zap.explodes {
                // `bolt::refine_for_explosion`: a radius of 1 unless the spell sets another.
                let radius = if id == "SPELL_ORB_OF_ELECTRICITY" {
                    2
                } else {
                    1
                };
                return Reach {
                    casts: range + radius,
                    strikes: range + radius,
                };
            }
            // The ported spells `bolt::get_cloud_size` (`beam.cc:2490`) gives at least ten squares.
            let wide_cloud = matches!(id, "SPELL_MIASMA_BREATH" | "SPELL_FREEZING_GUST");
            Reach {
                casts: range + i32::from(wide_cloud),
                strikes: range,
            }
        }
    }
}

/// A spell the monster could cast now, and whether its target is in reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Prospect {
    /// How often it is picked, of 200.
    pub(crate) frequency: u32,
    /// At what distances it can be picked.
    pub(crate) when: SlotRange,
    /// The player is within its range.
    pub(crate) in_range: bool,
}

/// Which of `prospects`, if any, a monster `distance` squares from the player casts this action:
/// `_choose_spell_to_cast` (`mon-cast.cc:5129`) for a hostile monster in full health, of the
/// spells `_find_usable_spells` keeps.
///
/// A spell rolled whose target is out of its range is given up, and the monster tries once more,
/// but on that try half the time looks only for a self-enchantment, which no damaging spell is
/// (`_find_spell_prospect`, `mon-cast.cc:4916`).
pub(crate) fn choose(
    variant: MonsterSpellChoice,
    rng: &mut Rng,
    prospects: &[Prospect],
    distance: i32,
) -> Option<usize> {
    match variant {
        MonsterSpellChoice::Crawl034 => {
            for attempt in 0..2 {
                if attempt > 0 && rng.random2(2) == 1 {
                    return None;
                }
                let mut what = u32::try_from(rng.random2(200)).unwrap_or(0);
                let mut chosen = None;
                for (i, prospect) in prospects.iter().enumerate() {
                    if !prospect.when.allows(distance) {
                        continue;
                    }
                    if prospect.frequency >= what {
                        chosen = Some(i);
                        break;
                    }
                    what -= prospect.frequency;
                }
                let i = chosen?;
                if prospects.get(i).is_some_and(|prospect| prospect.in_range) {
                    return Some(i);
                }
            }
            None
        }
    }
}

/// How a beam's armour applies: `ac_type`, chosen by `bolt::apply_AC` (`beam.cc:4588`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArmourRule {
    Normal,
    Half,
    Triple,
    None,
}

/// A beam a monster fires, set up for one cast: `zappy` (`beam.cc:470`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Bolt {
    /// Its accuracy, `None` when it never misses.
    pub(crate) to_hit: Option<i32>,
    /// How many damage dice it rolls.
    pub(crate) dice: i32,
    /// How many sides each has.
    pub(crate) sides: i32,
    explodes: bool,
    blockable: bool,
    armour: ArmourRule,
}

/// The beam `spell` fires at `power`: its dice and accuracy from the zap's monster calculators
/// (`zap_damage` and `zap_to_hit`, `beam.cc:385`), rolling `calc_dice`'s rounding for a zap whose
/// dice share a total (`random.cc:289`).
pub(crate) fn bolt(
    variant: BeamDamage,
    rng: &mut Rng,
    spell: &DamagingSpell<'_>,
    power: i32,
) -> Bolt {
    match variant {
        BeamDamage::Crawl034 => {
            let damage = spell.damage;
            let total = damage.adder + power * damage.per_power / damage.per.max(1);
            let (dice, sides) = if !damage.spread {
                (damage.dice, total)
            } else if damage.dice <= 1 {
                (1, total)
            } else if total <= damage.dice {
                (total, 1)
            } else {
                (damage.dice, rng.div_rand_round(total, damage.dice))
            };
            let to_hit = match spell.to_hit {
                ZapToHit::Automatic => None,
                ZapToHit::ByPower {
                    adder,
                    per_power,
                    per,
                } => {
                    Some(adder + power * per_power / per.max(1)).filter(|&hit| hit != AUTOMATIC_HIT)
                }
            };
            let zap = spell.zap;
            let flavour = zap.flavour.0.as_str();
            // `bolt::is_blockable` (`beam.cc:107`).
            let blockable =
                !zap.pierces && !zap.explodes && flavour != "BEAM_ELECTRICITY" && to_hit.is_some();
            // `bolt::apply_AC`, with `get_beam_resist_type` (`fight.cc:759`) for electricity.
            let armour = match flavour {
                "BEAM_DAMNATION" => ArmourRule::None,
                "BEAM_ELECTRICITY" | "BEAM_THUNDER" | "BEAM_STUN_BOLT" => ArmourRule::Half,
                "BEAM_FRAG" => ArmourRule::Triple,
                _ => ArmourRule::Normal,
            };
            Bolt {
                to_hit,
                dice,
                sides,
                explodes: zap.explodes,
                blockable,
                armour,
            }
        }
    }
}

/// How a beam fired at the player went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Shot {
    /// The player's shield blocked it.
    pub(crate) blocked: bool,
    /// It got past the shield and the player's evasion.
    pub(crate) hits: bool,
    /// What the player took: 0 unless it hit.
    pub(crate) damage: i32,
}

/// A beam fired at the player, rolled as `bolt::affect_player` (`beam.cc:4233`) rolls it: the
/// shield and evasion in `bolt::misses_player` (`beam.cc:3506`), then its dice and armour.
/// `can_block` is whether the shield has a block left before the player's next action.
pub(crate) fn shoot(
    hit: BeamHit,
    damage: BeamDamage,
    rng: &mut Rng,
    bolt: Bolt,
    defences: Defences,
    can_block: bool,
) -> Shot {
    let mut shot = Shot {
        blocked: false,
        hits: false,
        damage: 0,
    };
    match hit {
        BeamHit::Crawl034 => {
            // An explosion engulfs the player: nothing blocks or dodges it.
            if !bolt.explodes {
                if let (true, true, true, Some(to_hit)) =
                    (bolt.blockable, can_block, defences.sh > 0, bolt.to_hit)
                {
                    let test_hit = rng.random2(to_hit * 130 / 100);
                    // `player::shield_bonus` (`player.cc:6106`), from twice the displayed SH.
                    let block = rng.random2avg(defences.sh * 2 * 2, 2) / 3 - 1;
                    if test_hit < block {
                        shot.blocked = true;
                        return shot;
                    }
                }
                // `_test_beam_hit` (`beam.cc:3228`).
                if let Some(to_hit) = bolt.to_hit {
                    let attack = rng.random2(to_hit);
                    let defence = rng.random2avg(defences.ev, 2);
                    if attack - defence < 0 {
                        return shot;
                    }
                }
            }
        }
    }
    shot.hits = true;
    match damage {
        BeamDamage::Crawl034 => {
            let rolled = rng.roll_dice(bolt.dice, bolt.sides);
            // `actor::apply_ac` (`actor.cc:343`) without guaranteed damage reduction, which beams
            // don't get.
            let ac = defences.ac.max(0);
            let saved = match bolt.armour {
                ArmourRule::None => 0,
                ArmourRule::Normal => rng.random2(1 + ac),
                ArmourRule::Half => rng.random2(1 + ac) / 2,
                ArmourRule::Triple => {
                    rng.random2(1 + ac) + rng.random2(1 + ac) + rng.random2(1 + ac)
                }
            };
            shot.damage = (rolled - saved).max(0);
        }
    }
    shot
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spell_range(min: i32, max: i32, power_cap: i32) -> SpellRange {
        SpellRange {
            min,
            max,
            power_cap,
        }
    }

    #[test]
    fn a_spells_range_grows_with_power_up_to_its_cap_and_never_past_sight() {
        // Worked by hand from `calc_spell_range`: (50 * (5 - 3) + 50) / 100 + 3 = 4.
        let p = MonsterSpellPower::Crawl034;
        assert_eq!(range(p, spell_range(3, 5, 100), 0), 3);
        assert_eq!(range(p, spell_range(3, 5, 100), 50), 4);
        assert_eq!(range(p, spell_range(3, 5, 100), 100), 5);
        assert_eq!(range(p, spell_range(8, 8, 25), 0), 7);
    }

    fn prospect(frequency: u32, when: SlotRange, in_range: bool) -> Prospect {
        Prospect {
            frequency,
            when,
            in_range,
        }
    }

    /// How often each prospect, or none, was chosen in 200,000 tries.
    fn shares(prospects: &[Prospect], distance: i32) -> Vec<f64> {
        let mut rng = Rng::new(3, 5);
        let mut counts = vec![0_u32; prospects.len() + 1];
        let trials = 200_000;
        for _ in 0..trials {
            let i = choose(MonsterSpellChoice::Crawl034, &mut rng, prospects, distance)
                .unwrap_or(prospects.len());
            counts[i] += 1;
        }
        counts
            .into_iter()
            .map(|count| f64::from(count) / f64::from(trials))
            .collect()
    }

    #[test]
    fn a_spell_in_range_is_cast_about_as_often_as_its_frequency_in_two_hundred() {
        // Worked by hand: the first slot takes rolls 0..=20 (21 of 200), the second the next 40.
        let got = shares(
            &[
                prospect(20, SlotRange::Any, true),
                prospect(40, SlotRange::Any, true),
            ],
            2,
        );
        for (got, expected) in got.iter().zip([0.105, 0.2, 0.695]) {
            assert!((got - expected).abs() < 0.004, "{got} vs {expected}");
        }
    }

    #[test]
    fn a_spell_out_of_range_is_retried_once_on_half_the_casts() {
        // Out of range: a roll for the first slot (21/200) is retried half the time.
        let got = shares(
            &[
                prospect(20, SlotRange::Any, false),
                prospect(40, SlotRange::Any, true),
            ],
            2,
        );
        let second = 0.2 + 0.105 * 0.5 * 0.2;
        assert!((got[1] - second).abs() < 0.004, "{got:?}");
        assert!(got[0] == 0.0, "{got:?}");
    }

    #[test]
    fn a_long_range_slot_is_skipped_up_close_without_using_up_the_roll() {
        // Up close, the long-range slot is skipped and the next takes rolls 0..=40.
        let got = shares(
            &[
                prospect(20, SlotRange::Long, true),
                prospect(40, SlotRange::Any, true),
            ],
            1,
        );
        assert!((got[1] - 0.205).abs() < 0.004, "{got:?}");
    }
}

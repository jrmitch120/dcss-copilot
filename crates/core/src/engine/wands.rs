//! Crawl's formulas for a monster zapping a wand at the player, ported closely (ADR 0002) behind
//! the formula variant a Game Knowledge bundle names: whether it zaps this action, the power it
//! zaps with and the energy that takes. The beam is the wand's spell's, as a monster casts it
//! (`spells.rs`).
//!
//! Line numbers are crawl 0.34.1's (`crawl-ref/source/` at tag 0.34.1). Only the path a monster
//! in plain view takes, with no statuses and a wand it likes, is ported.

use copilot_gk::{EnumId, MonsterSpellPower, MonsterWand};

use super::rng::Rng;
use super::spells;

/// Whether a monster that carries a wand zaps it this action, before its tracer checks the wand
/// reaches the player: `_handle_wand` (`mon-act.cc:1430`) gives up three times in four.
pub(crate) fn zaps(variant: MonsterWand, rng: &mut Rng) -> bool {
    match variant {
        MonsterWand::Crawl034 => !rng.x_chance_in_y(3, 4),
    }
}

/// The power a monster with `hit_dice` zaps a wand of `spell` with: `mons_wand_power`
/// (`mon-cast.cc:2195`), `30 + hit_dice` scaled by the spell's factor as `spell_power` gives it
/// to a monster of one hit die.
pub(crate) fn power(
    variant: MonsterWand,
    spell_power: MonsterSpellPower,
    spell: &EnumId,
    hit_dice: i32,
) -> i32 {
    match variant {
        MonsterWand::Crawl034 => (30 + hit_dice) * spells::power(spell_power, spell, 1) / 12,
    }
}

/// The energy a zap takes: `_mons_fire_wand` (`mon-act.cc:1419`) spends an item action, which
/// costs every monster 10 (`monster::action_energy`, `monster.cc:5913`).
pub(crate) fn energy(variant: MonsterWand) -> i32 {
    match variant {
        MonsterWand::Crawl034 => 10,
    }
}

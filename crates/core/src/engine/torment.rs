//! Crawl's torment of the player, ported closely (ADR 0002) behind the formula variant a Game
//! Knowledge bundle names: `torment_player` (`spl-goditem.cc:1089`), with `player::res_torment`
//! (`player.cc:7035`) for what the player is. A monster that blows itself up into torment, the
//! lurking horror, sets it off as it attacks.
//!
//! Line numbers are crawl 0.34.1's (`crawl-ref/source/` at tag 0.34.1).

use copilot_gk::{Form, Torment};

use super::resists::Resists;

/// How torment hurts the player for what they are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Shield {
    /// It takes its full share.
    None,
    /// It takes half, in a form that leaves them only partly petrified or undead.
    Halved,
    /// It takes nothing.
    Immune,
}

/// How torment hurts a player in `form`, if they are in one, petrified or not, a plant or not,
/// as `variant` judges them: `player::res_torment` and the halving of `torment_player`, but for
/// the Torment Resistance mutation, the unrandart "Eternal Torment" and Kikubaaqudgha's
/// shielding, which the Advisor doesn't read.
pub(crate) fn shield(
    variant: Torment,
    form: Option<&Form>,
    petrified: bool,
    plant: bool,
) -> Shield {
    match variant {
        Torment::Crawl034 => {
            if petrified || plant || form.is_some_and(|form| form.negative >= 3) {
                Shield::Immune
            } else if form.is_some_and(|form| form.halves_torment) {
                Shield::Halved
            } else {
                Shield::None
            }
        }
    }
}

/// The hit points torment takes from a player on `hp` who resists as `resists` says: about half,
/// less for each level of `rNeg`, and never all of them.
pub(crate) fn damage(variant: Torment, hp: i32, resists: Resists) -> i32 {
    match variant {
        Torment::Crawl034 => {
            let full = (hp * (50 - resists.negative * 5) / 100 - 1).max(0);
            match resists.holiness.torment {
                Shield::None => full,
                Shield::Halved => full / 2,
                Shield::Immune => 0,
            }
        }
    }
}

//! Crawl's torment of the player, ported closely (ADR 0002) behind the formula variant a Game
//! Knowledge bundle names: `torment_player` (`spl-goditem.cc:1089`), with `player::res_torment`
//! (`player.cc:7035`) for what the player is. A monster that blows itself up into torment, the
//! lurking horror, sets it off as it attacks.
//!
//! Line numbers are crawl 0.34.1's (`crawl-ref/source/` at tag 0.34.1).

use copilot_gk::{Form, Torment};

use super::resists::Resists;

/// The share of its hit points torment takes from the player for what they are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Share {
    /// Its full share.
    Full,
    /// It takes half, in a form that leaves them only partly petrified or undead.
    Halved,
    /// It takes nothing.
    Immune,
}

/// How torment hurts a player in `form`, if they are in one, petrified or not, a plant or not,
/// as `variant` judges them: `player::res_torment` and the halving of `torment_player`, but for
/// the Torment Resistance mutation, the unrandart "Eternal Torment" and Kikubaaqudgha's
/// shielding, which the Advisor doesn't read.
pub(crate) fn share(variant: Torment, form: Option<&Form>, petrified: bool, plant: bool) -> Share {
    match variant {
        Torment::Crawl034 => {
            if petrified || plant || form.is_some_and(|form| form.negative >= 3) {
                Share::Immune
            } else if form.is_some_and(|form| form.halves_torment) {
                Share::Halved
            } else {
                Share::Full
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
                Share::Full => full,
                Share::Halved => full / 2,
                Share::Immune => 0,
            }
        }
    }
}

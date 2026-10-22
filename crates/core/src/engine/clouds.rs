//! Crawl's rules for what the clouds a monster's own spells leave change before the player's
//! next action, ported closely (ADR 0002) behind the formula variant a Game Knowledge bundle
//! names.
//!
//! A cloud spell's beam leaves its cloud round where it ends (`bolt::affect_endpoint`,
//! `beam.cc:2617`): the player's square when it reaches them, and one beside them when it stops
//! short and its cloud spreads to them. Some leave it on every square they pass too
//! (`bolt::affect_place_clouds`, `beam.cc:2990`). Every cloud lasts longer than one player action,
//! so none clears before the player's next.
//!
//! Two squares of an opaque cloud between a monster and the player block its sight
//! (`opacity_default`, `losparam.cc:30`, and `_losight_quadrant`, `los.cc:735`), and a monster
//! that can't see the player takes no special action: it casts nothing (`monster::near_foe` in
//! `_mons_take_special_action`, `mon-act.cc:1856`), zaps nothing (`_handle_wand`,
//! `mon-act.cc:1439`) and shoots nothing. A pearl dragon that breathes blessed fire at the player
//! from three squares or more walks on until it is two squares away, with one square of its fire
//! between them, before it breathes again.
//!
//! A cloud spell's tracer counts the player only if its cloud reaches them (`bolt::affect_endpoint`,
//! `beam.cc:2583`), and a cloud spreads only over squares no cloud holds yet (`_cloudable` in
//! `targeter_cloud::set_aim`, `target.cc:1103`). A beam that reaches the player ends on their
//! square, which is always counted; one that stops short spreads to them only while their square
//! holds no cloud, and once it does the tracer finds no foe, so the monster doesn't cast it
//! (`targeting_tracer::good_to_fire`, `beam.cc:7684`). A death drake that breathes miasma from a
//! square past its beam's range doesn't breathe from there again: it walks a square nearer first.
//!
//! The engine has no map (`NotModelled::Paths`), so it follows a beam's path only along a row, a
//! column or a diagonal through the player, where the path and every line of sight run through
//! the same squares. A monster that casts from anywhere else leaves no opaque cloud the engine
//! follows, and one that stands anywhere else sees the player: both err high. So does leaving out
//! the cloud round where the beam ends, which spreads at random beside the player. Each player
//! action starts with none: the engine doesn't follow a cloud's decay or the player's steps.

use copilot_gk::MonsterClouds;

use super::spells::{DamagingSpell, Reach};
use super::step::Offset;

/// The clouds a monster's own spells have left since the player's last action.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CloudsLeft {
    /// The squares an opaque cloud covers that the engine follows, from the player.
    opaque: Vec<Offset>,
    /// The player's square holds a cloud.
    on_player: bool,
}

impl CloudsLeft {
    /// Notes the clouds `spell` leaves, cast at the player from `from` with `reach`.
    pub(crate) fn cast(
        &mut self,
        variant: MonsterClouds,
        spell: &DamagingSpell<'_>,
        reach: Reach,
        from: Offset,
    ) {
        match variant {
            MonsterClouds::Crawl034 => {
                let Some(cloud) = &spell.spell.cloud else {
                    return;
                };
                self.on_player = true;
                if cloud.opaque && cloud.trail && straight(from) {
                    // Every square its beam passes, to the player or as far as it reaches.
                    let passed = from.distance().min(reach.strikes);
                    let mut square = from;
                    for _ in 0..passed {
                        square = square.nearer();
                        if !self.opaque.contains(&square) {
                            self.opaque.push(square);
                        }
                    }
                }
            }
        }
    }

    /// Whether a monster at `at` sees the player past the opaque clouds: unless two of the
    /// squares between them hold one.
    pub(crate) fn sees_player(&self, variant: MonsterClouds, at: Offset) -> bool {
        match variant {
            MonsterClouds::Crawl034 => {
                if !straight(at) {
                    return true;
                }
                let between =
                    std::iter::successors(Some(at.nearer()), |square| Some(square.nearer()))
                        .take_while(|&square| square != Offset::PLAYER);
                between
                    .filter(|square| self.opaque.contains(square))
                    .count()
                    < 2
            }
        }
    }

    /// Whether the tracer of `spell`, cast with `reach` from `distance` squares, finds the
    /// player: always but for a cloud spell whose beam stops short of them, which finds them
    /// only while their square holds no cloud.
    pub(crate) fn tracer_finds_player(
        &self,
        variant: MonsterClouds,
        spell: &DamagingSpell<'_>,
        reach: Reach,
        distance: i32,
    ) -> bool {
        match variant {
            MonsterClouds::Crawl034 => {
                spell.spell.cloud.is_none() || distance <= reach.strikes || !self.on_player
            }
        }
    }
}

/// Whether `at` lies on a row, a column or a diagonal through the player.
fn straight(at: Offset) -> bool {
    at.x == 0 || at.y == 0 || at.x.abs() == at.y.abs()
}

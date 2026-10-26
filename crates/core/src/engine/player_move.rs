//! The time a move takes the player, ported closely (ADR 0002) behind the formula variant a Game
//! Knowledge bundle names: `player_speed` (`player.cc:1921`), the time any action of theirs
//! takes, `player_movement_speed` (`player.cc:1843`), the delay a move adds to it, and
//! `move_player_action` (`movement.cc:1069`), which takes `div_rand_round(player_speed() *
//! delay, BASELINE_DELAY)` for a step onto a square, the delay that of the square stepped onto.
//!
//! Where crawl rounds at random (`div_rand_round`), the caller says how: as crawl does, for the
//! crawl oracle, or always up, which is the slowest the player can be ([`Slowest`]). A player who
//! may be slowest is how the stair race plays them, as it plays the monster racing them with the
//! most energy it can have.
//!
//! Line numbers are crawl 0.34.1's (`crawl-ref/source/` at tag 0.34.1).

use std::collections::BTreeMap;

use copilot_gk::{
    BodySize, Bundle, Form, FormBody, PlayerMove, Species, SpeedMutation, SpeedSigns, SpeedStatus,
};

use super::melee::BASELINE_DELAY;
use super::rng::Rng;
use crate::accumulated_state::MutationsRead;
use crate::visible_state::Religion;

copilot_gk::crawl_identifiers! {
    /// The forms, mutations and banes `player_speed` and `player_movement_speed` name in code.
    named {
        /// `transformation::statue`, which slows every action.
        STATUE = "statue";
        /// `transformation::fortress_crab`, which slows moves.
        FORTRESS_CRAB = "fortress_crab";
        /// Moves faster, by a level and one more.
        FAST = "MUT_FAST";
        /// Moves slower, a fifth more for each level.
        SLOW = "MUT_SLOW";
        /// Moves slower, a fifth more, at any level.
        FROG_LEGS = "MUT_FROG_LEGS";
        /// Moves slower, two fifths more, at any level.
        CONSTRICTING_TAIL = "MUT_CONSTRICTING_TAIL";
        /// Moves faster in water from its second level.
        NIMBLE_SWIMMER = "MUT_NIMBLE_SWIMMER";
        /// Moves slower, three tenths more.
        LETHARGY = "BANE_LETHARGY";
    }
}

/// The fastest a move takes the player, whatever speeds it up: `FASTEST_PLAYER_MOVE_SPEED`
/// (`player.h:96`).
const FASTEST_MOVE: i32 = 6;

/// The piety each rank of stars the status pane shows starts at, from one star: `piety_breakpoint`
/// (`religion.cc:4522`).
const PIETY_BREAKPOINTS: [i32; 6] = [30, 50, 75, 100, 120, 160];

/// The most piety a follower can have: `MAX_PIETY` (`religion.h`).
const MAX_PIETY: i32 = 200;

/// How a port rounds where crawl rounds at random.
pub(crate) trait Rounding {
    /// `div_rand_round(num, den)` (`random.cc:313`), for `num` of 0 or more.
    fn div_rand_round(&mut self, num: i32, den: i32) -> i32;
}

impl Rounding for Rng {
    fn div_rand_round(&mut self, num: i32, den: i32) -> i32 {
        Rng::div_rand_round(self, num, den)
    }
}

/// Rounding always up: the slowest crawl can roll a player's moves, each of its rounds slowing
/// them as it rounds up.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Slowest;

impl Rounding for Slowest {
    fn div_rand_round(&mut self, num: i32, den: i32) -> i32 {
        num / den + i32::from(num % den != 0)
    }
}

/// The player, as the time their moves take goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Mover<'a> {
    /// What tells the player's speed, for this Crawl Version.
    pub(crate) signs: &'a SpeedSigns,
    /// Their species.
    pub(crate) species: &'a Species,
    /// The form they are in, `None` for none.
    pub(crate) form: Option<&'a Form>,
    /// The statuses they have that change their speed.
    pub(crate) statuses: Vec<SpeedStatus>,
    /// The levels of each mutation that changes their speed, innate and acquired, by
    /// `mutation_type` identifier: crawl's `get_base_mutation_level`, before a form suppresses
    /// any.
    pub(crate) mutations: BTreeMap<&'a str, u32>,
    /// The banes they have that change their speed, by `bane_type` identifier.
    pub(crate) banes: Vec<&'a str>,
    /// Their piety, if they follow a god who slows them (`passive_t::slowed`).
    pub(crate) slowing_god: Option<i32>,
    /// They follow a god who lets them walk on water (`passive_t::water_walk`).
    pub(crate) water_walking: bool,
    /// The pieces of armour of ponderousness they wear (`SPARM_PONDEROUSNESS`).
    pub(crate) ponderous: i32,
    /// They wear the lightning scales (`UNRAND_LIGHTNING_SCALES`).
    pub(crate) quickened: bool,
}

impl Mover<'_> {
    fn has(&self, status: SpeedStatus) -> bool {
        self.statuses.contains(&status)
    }

    /// The body they have: their form's, or the one in no form.
    fn body(&self) -> &FormBody {
        self.form.map_or(&self.signs.unformed, |form| &form.body)
    }

    fn in_form(&self, id: &str) -> bool {
        self.form.is_some_and(|form| form.id == id)
    }

    /// Their level of the mutation `id` in effect: `player::get_mutation_level`
    /// (`mutation.cc:473`), none when it is of their anatomy and their form changes it
    /// (`mut_is_compatible`, `mutation.cc:1603`).
    pub(crate) fn mutation(&self, id: &str) -> u32 {
        let level = self.mutations.get(id).copied().unwrap_or(0);
        let anatomy = self
            .signs
            .mutations
            .iter()
            .any(|mutation| mutation.id.0 == id && mutation.anatomy);
        if anatomy && self.body().changes_anatomy {
            0
        } else {
            level
        }
    }

    /// The size of their body: `player::body_size(PSIZE_BODY)` (`player-act.cc:173`).
    fn size(&self) -> BodySize {
        self.body().size.unwrap_or(self.species.size)
    }

    /// Whether they swim: `Form::player_can_swim` (`transform.cc:521`).
    fn swims(&self) -> bool {
        let body = self.body();
        body.swims == Some(true)
            || (self.species.swims && body.swims != Some(false))
            || self.size() >= BodySize::Giant
    }
}

/// The time an action takes the player as `variant` reckons it, where 10 is a standard action:
/// `player_speed` (`player.cc:1921`).
pub(crate) fn action_time(
    variant: PlayerMove,
    mover: &Mover<'_>,
    round: &mut impl Rounding,
) -> i32 {
    match variant {
        PlayerMove::Crawl034 => {
            let mut time = BASELINE_DELAY;
            if mover.has(SpeedStatus::Helpless) {
                return time;
            }
            let slowed = mover.has(SpeedStatus::Slowed) || mover.has(SpeedStatus::HastedAndSlowed);
            let hasted = mover.has(SpeedStatus::Hasted) || mover.has(SpeedStatus::HastedAndSlowed);
            if slowed {
                // `haste_mul` (`defines.h:196`).
                time = round.div_rand_round(time * 3, 2);
            }
            if mover.has(SpeedStatus::Berserk) && mover.slowing_god.is_none() {
                // `berserk_div`, as the no_haste passive of a god who slows them forbids.
                time = round.div_rand_round(time * 2, 3);
            } else if hasted {
                // `haste_div`.
                time = round.div_rand_round(time * 2, 3);
            }
            if mover.in_form(named::STATUE) || mover.has(SpeedStatus::Petrifying) {
                time = time * 15 / 10;
            }
            time
        }
    }
}

/// The delay a move onto a square adds to it, as `variant` reckons it, where 10 is a standard
/// move, onto water or not: `player_movement_speed` (`player.cc:1843`).
pub(crate) fn move_delay(
    variant: PlayerMove,
    mover: &Mover<'_>,
    water: bool,
    round: &mut impl Rounding,
) -> i32 {
    match variant {
        PlayerMove::Crawl034 => {
            let mut delay = mover.body().move_speed;
            if water {
                // `player::in_water` and `player::slow_in_water`.
                let in_water = !mover.has(SpeedStatus::Flying) && !mover.water_walking;
                let slow_in_water = !mover.swims() && mover.size() <= BodySize::Medium;
                if mover.mutation(named::NIMBLE_SWIMMER) >= 2 {
                    delay -= 4;
                } else if in_water && slow_in_water {
                    delay += 6;
                }
            }
            if mover.has(SpeedStatus::SlowedMovement) {
                delay += 3;
            }
            if mover.quickened {
                delay -= 1;
            }
            delay += mover.ponderous;
            if let Some(piety) = mover.slowing_god {
                delay += 2 + round.div_rand_round(piety, 20).min(8);
            }
            let fast = i32::try_from(mover.mutation(named::FAST)).unwrap_or(0);
            if fast > 0 {
                delay -= fast + 1;
            }
            let slow = i32::try_from(
                mover.mutation(named::SLOW)
                    + u32::from(mover.mutation(named::FROG_LEGS) > 0)
                    + u32::from(mover.mutation(named::CONSTRICTING_TAIL) > 0) * 2,
            )
            .unwrap_or(0);
            if slow > 0 {
                delay = delay * (10 + slow * 2) / 10;
            }
            if mover.banes.contains(&named::LETHARGY) {
                delay = delay * 13 / 10;
            }
            if mover.in_form(named::FORTRESS_CRAB) {
                delay = delay * 13 / 10;
            }
            if mover.has(SpeedStatus::Frozen) {
                delay = round.div_rand_round(delay * 3, 2);
            }
            if mover.has(SpeedStatus::Swift) {
                delay = round.div_rand_round(3 * delay, 4);
            } else if mover.has(SpeedStatus::Unswift) {
                if delay >= 8 {
                    delay = round.div_rand_round(3 * delay, 2);
                } else if delay == 7 {
                    // Balanced for the floor, as crawl has it.
                    delay = round.div_rand_round(7 * 6, 5);
                }
            }
            delay.max(FASTEST_MOVE)
        }
    }
}

/// The time a step onto a square takes the player, onto water or not, as `variant` reckons it,
/// where 10 is a standard move: `move_player_action` (`movement.cc:1069`).
pub(crate) fn move_time(
    variant: PlayerMove,
    mover: &Mover<'_>,
    water: bool,
    round: &mut impl Rounding,
) -> i32 {
    match variant {
        PlayerMove::Crawl034 => {
            let delay = move_delay(variant, mover, water, round);
            let action = action_time(variant, mover, round);
            round.div_rand_round(action * delay, BASELINE_DELAY)
        }
    }
}

/// What the game shows of the player that tells how fast they move.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Seen<'a> {
    /// The lights of the statuses the status pane lists, `None` until it has.
    pub(crate) lights: Option<&'a [&'a str]>,
    /// Their species, as the game names it.
    pub(crate) species: Option<&'a str>,
    /// Their experience level.
    pub(crate) xl: Option<u32>,
    /// Their god, if the game has said.
    pub(crate) religion: Option<&'a Religion>,
    /// What the mutations screen last showed, Known or Stale, if it has been read.
    pub(crate) mutations: Option<&'a MutationsRead>,
    /// The names of the items they carry, if the game has sent the inventory, each with whether
    /// they wear it: `None` for one the Advisor hasn't seen them equip or not.
    pub(crate) carried: Option<&'a [(&'a str, Option<bool>)]>,
}

/// The player as the Advisor reads them, and what of them it couldn't read that may change how
/// fast they move.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Read<'a> {
    /// The player.
    pub(crate) mover: Mover<'a>,
    /// What it couldn't read.
    pub(crate) unread: Vec<Unread>,
}

/// What of the player the Advisor couldn't read that may change how fast they move.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Unread {
    /// The status pane hasn't been reported.
    Statuses,
    /// A status light the Game Knowledge doesn't classify, neither a form nor a status it knows.
    Light(String),
    /// The mutations screen hasn't been read, so only the species' own mutations are known.
    Mutations,
    /// The mutations were read off what doesn't list banes, the character dump, so any bane
    /// that slows the player is unknown.
    Banes,
    /// They carry armour that changes their speed, and the Advisor doesn't know whether they
    /// wear it: taken as not worn.
    Gear,
}

/// The player as `seen`, read through `bundle`: `None` when the bundle doesn't know their
/// species. A follower of a god who slows them is taken at the most piety the rank of stars the
/// status pane shows holds, which slows them most; one whose piety isn't shown, at the most a
/// follower can have.
pub(crate) fn read<'a>(bundle: &'a Bundle, seen: &Seen<'_>) -> Option<Read<'a>> {
    let signs = &bundle.speed;
    let species = bundle.species_named(seen.species?)?;
    let mut unread = Vec::new();
    let mut statuses = Vec::new();
    let mut form = None;
    match seen.lights {
        None => unread.push(Unread::Statuses),
        Some(lights) => {
            for &light in lights {
                if let Some(lit) = bundle.holiness.forms.iter().find(|f| f.light == light) {
                    form = Some(lit);
                } else {
                    match classify(signs, light) {
                        Lit::Status(status) => statuses.push(status),
                        Lit::Nothing => {}
                        Lit::Unclassified => unread.push(Unread::Light(light.to_owned())),
                    }
                }
            }
        }
    }
    let mut mutations: BTreeMap<&str, u32> = BTreeMap::new();
    let innate = |xl: Option<u32>| -> BTreeMap<&'a str, u32> {
        let mut levels = BTreeMap::new();
        for gained in &species.mutations {
            if xl.is_some_and(|xl| gained.xl <= xl)
                && let Some(mutation) = signs.mutations.iter().find(|m| m.id == gained.mutation)
            {
                *levels.entry(mutation.id.0.as_str()).or_default() += gained.levels;
            }
        }
        levels
    };
    // A species has its first level's mutations from the start, whatever level is reported.
    let now = innate(Some(seen.xl.unwrap_or(1)));
    for (&id, &level) in &now {
        mutations.insert(id, level);
    }
    let mut banes = Vec::new();
    match seen.mutations {
        None => unread.push(Unread::Mutations),
        Some(read) => {
            let then = innate(Some(read.xl.unwrap_or(1)));
            for shown in &read.shown.mutations {
                if let Some((mutation, level)) = shown_level(signs, shown) {
                    let id = mutation.id.0.as_str();
                    let acquired = level.saturating_sub(then.get(id).copied().unwrap_or(0));
                    *mutations.entry(id).or_default() += acquired;
                }
            }
            if read.shown.banes.is_none() {
                unread.push(Unread::Banes);
            }
            for shown in read.shown.banes.iter().flatten() {
                banes.extend(
                    signs
                        .banes
                        .iter()
                        .filter(|bane| bane.shown == *shown)
                        .map(|bane| bane.id.0.as_str()),
                );
            }
        }
    }
    let (slowing_god, water_walking) = match seen.religion {
        Some(Religion::God {
            name,
            piety_rank,
            penance,
        }) => {
            let slowing = signs.slowing_gods.contains(name);
            let walks = signs.water_walking_gods.iter().any(|god| {
                god.god == *name
                    && !penance
                    && piety_rank.is_some_and(|rank| rank >= god.piety_rank)
            });
            (slowing.then(|| most_piety(*piety_rank)), walks)
        }
        Some(Religion::NoGod) | None => (None, false),
    };
    let mut ponderous = 0;
    let mut quickened = false;
    for &(name, worn) in seen.carried.unwrap_or_default() {
        let name = name.to_lowercase();
        let slows = signs.ponderous_gear.iter().any(|gear| name.contains(gear));
        let quickens = signs.quickening_gear.iter().any(|gear| name.contains(gear));
        if !(slows || quickens) {
            continue;
        }
        match worn {
            Some(true) => {
                ponderous += i32::from(slows);
                quickened |= quickens;
            }
            Some(false) => {}
            None => unread.push(Unread::Gear),
        }
    }
    unread.sort();
    unread.dedup();
    Some(Read {
        mover: Mover {
            signs,
            species,
            form,
            statuses,
            mutations,
            banes,
            slowing_god,
            water_walking,
            ponderous,
            quickened,
        },
        unread,
    })
}

/// What a status light tells of the player's speed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Lit {
    /// It lights a status that changes their speed.
    Status(SpeedStatus),
    /// It lights a status that doesn't.
    Nothing,
    /// The Game Knowledge doesn't classify it.
    Unclassified,
}

/// What `light` tells of the player's speed, as `signs` classify the lights the status pane may
/// show. A light crawl builds as it draws it, such as `Torch (3)`, is classified by the longest
/// light of `signs` it starts with, that ends where a word does.
pub(crate) fn classify(signs: &SpeedSigns, light: &str) -> Lit {
    signs
        .lights
        .iter()
        .filter(|known| {
            light
                .strip_prefix(known.light.as_str())
                .is_some_and(|rest| {
                    rest.chars()
                        .next()
                        .is_none_or(|next| !next.is_alphanumeric())
                })
        })
        .max_by_key(|known| known.light.len())
        .map_or(Lit::Unclassified, |known| {
            known.status.map_or(Lit::Nothing, Lit::Status)
        })
}

/// The mutation of `signs` a row of the mutations screen shows, and its level, if it is one.
fn shown_level<'a>(signs: &'a SpeedSigns, shown: &str) -> Option<(&'a SpeedMutation, u32)> {
    signs.mutations.iter().find_map(|mutation| {
        let at = mutation.shown.iter().position(|text| text == shown)?;
        Some((mutation, u32::try_from(at + 1).ok()?))
    })
}

/// The most piety a follower whose status pane shows `rank` stars can have, the most a follower
/// can for a rank not shown.
fn most_piety(rank: Option<u32>) -> i32 {
    rank.and_then(|rank| PIETY_BREAKPOINTS.get(usize::try_from(rank).ok()?))
        .map_or(MAX_PIETY, |next| next - 1)
}

#[cfg(test)]
mod tests {
    use copilot_gk::Library;

    use super::*;

    #[test]
    fn every_name_the_port_reads_is_in_every_shipped_bundle_of_its_variant() {
        let library = Library::shipped().unwrap();
        for bundle in library.bundles() {
            let signs = &bundle.speed;
            let forms: Vec<&str> = bundle
                .holiness
                .forms
                .iter()
                .map(|f| f.id.as_str())
                .collect();
            for id in named::ALL {
                let known = forms.contains(id)
                    || signs.mutations.iter().any(|m| m.id.0 == *id)
                    || signs.banes.iter().any(|b| b.id.0 == *id);
                assert!(known, "{} lacks {id}", bundle.crawl.tag);
            }
        }
    }

    #[test]
    fn rounding_up_is_the_slowest_crawl_can_roll() {
        assert_eq!(Slowest.div_rand_round(20, 3), 7);
        assert_eq!(Slowest.div_rand_round(30, 2), 15);
        assert_eq!(Slowest.div_rand_round(0, 20), 0);
    }

    #[test]
    fn the_most_piety_of_a_rank_is_just_short_of_the_next() {
        assert_eq!(most_piety(Some(0)), 29);
        assert_eq!(most_piety(Some(5)), 159);
        assert_eq!(most_piety(Some(6)), MAX_PIETY);
        assert_eq!(most_piety(None), MAX_PIETY);
    }
}

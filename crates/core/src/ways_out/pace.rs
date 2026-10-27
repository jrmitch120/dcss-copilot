//! The player's Pace in the stair race: the time each of their moves to a staircase takes, at the
//! speed crawl gives them, the slowest it may roll it, and why it isn't standard.

use copilot_gk::{Bundle, PlayerMove};

use super::{PaceReason, PlayerPace, ReadOn, STANDARD_TIME, current};
use crate::accumulated_state::{AccumulatedState, Remembered};
use crate::engine::player_move::{self, Read, Seen, Slowest, Unread};
use crate::gaps::{Gap, ScreenFact};
use crate::visible_state::{Religion, VisibleState};

/// The player's Pace, as the Advisor reads it.
pub(super) struct Pace<'a> {
    variant: PlayerMove,
    /// The player, `None` when the Game Knowledge doesn't know their species: they are taken to
    /// move at standard speed.
    read: Option<Read<'a>>,
    /// When the mutations screen was read, while it has been.
    mutations: Option<ReadOn>,
    /// Their god and its stars, if the god slows them.
    slowing_god: Option<(String, Option<u32>)>,
}

impl<'a> Pace<'a> {
    /// The Pace of the player `visible` shows and the Advisor remembers, of `species`, read
    /// through `bundle`.
    pub(super) fn of(
        visible: &VisibleState,
        remembered: Option<&AccumulatedState>,
        species: Option<&str>,
        bundle: &'a Bundle,
    ) -> Self {
        let lights: Option<Vec<&str>> = visible.statuses.as_ref().map(|statuses| {
            statuses
                .iter()
                .filter_map(|status| status.light.as_deref())
                .collect()
        });
        let mutations = remembered.map(|remembered| &remembered.mutations);
        let equipped = remembered.and_then(|remembered| remembered.equipped.read_value());
        let carried: Option<Vec<(&str, Option<bool>)>> = visible.inventory.as_ref().map(|items| {
            items
                .iter()
                .map(|item| {
                    let worn = equipped.map(|letters| letters.contains(&item.letter));
                    (item.name.as_str(), worn)
                })
                .collect()
        });
        let seen = Seen {
            lights: lights.as_deref(),
            species,
            xl: visible.stats.xl,
            religion: visible.religion.as_ref(),
            mutations: mutations.and_then(Remembered::read_value),
            carried: carried.as_deref(),
        };
        let read = player_move::read(bundle, &seen);
        let mutations = mutations.and_then(current).map(|(_, read)| read);
        let slowing_god = match &visible.religion {
            Some(Religion::God {
                name, piety_rank, ..
            }) if bundle.speed.slowing_gods.contains(name) => Some((name.clone(), *piety_rank)),
            _ => None,
        };
        Self {
            variant: bundle.formulas.player_move,
            read,
            mutations,
            slowing_god,
        }
    }

    /// The time a step onto a square takes them, onto water or not, the slowest crawl may roll
    /// it.
    pub(super) fn step(&self, water: bool) -> u32 {
        self.read.as_ref().map_or(STANDARD_TIME, |read| {
            let time = player_move::move_time(self.variant, &read.mover, water, &mut Slowest);
            u32::try_from(time).unwrap_or(STANDARD_TIME)
        })
    }

    /// The time an action other than a move takes them, such as opening a door, the slowest
    /// crawl may roll it.
    pub(super) fn action(&self) -> u32 {
        self.read.as_ref().map_or(STANDARD_TIME, |read| {
            let time = player_move::action_time(self.variant, &read.mover, &mut Slowest);
            u32::try_from(time).unwrap_or(STANDARD_TIME)
        })
    }

    /// Whether something the Advisor can't see could change their Pace
    /// ([`crate::fact_sheet::NotModelled::PlayerSpeed`]): it doesn't know their species, hasn't
    /// read the mutations screen or their status pane, can't classify a status or form they
    /// have, or doesn't know whether they wear armour they carry that changes their speed.
    pub(super) fn unseen(&self) -> bool {
        self.read
            .as_ref()
            .is_none_or(|read| !read.unread.is_empty())
    }

    /// The Gaps their Pace was judged around in what is `remembered` of the game: their
    /// mutations unread, counting only their species' own, or out of date; and what they wear,
    /// unread or out of date, while they carry armour that changes their speed.
    pub(super) fn gaps(&self, remembered: Option<&AccumulatedState>) -> Vec<Gap> {
        let Some(read) = &self.read else {
            return Vec::new();
        };
        let mut gaps: Vec<Gap> = ScreenFact::Mutations
            .gap_in(remembered, |state| &state.mutations)
            .into_iter()
            .collect();
        if read.unread.contains(&Unread::Gear) {
            gaps.extend(ScreenFact::Equipped.gap_in(remembered, |state| &state.equipped));
        }
        gaps
    }

    /// Their Pace as the Fact Sheet records it.
    pub(super) fn record(&self) -> PlayerPace {
        let move_time = self.step(false);
        let water = self.step(true);
        PlayerPace {
            move_time,
            water_move_time: (water != move_time).then_some(water),
            action_time: self.action(),
            because: self.reasons(),
            mutations_read: self.mutations,
        }
    }

    /// What makes their Pace other than standard, as the Advisor reads them.
    fn reasons(&self) -> Vec<PaceReason> {
        let Some(read) = &self.read else {
            return Vec::new();
        };
        let mover = &read.mover;
        let mut reasons: Vec<PaceReason> = mover
            .statuses
            .iter()
            .map(|&status| PaceReason::Status { status })
            .collect();
        if let Some(form) = mover.form {
            reasons.push(PaceReason::Form {
                light: form.light.clone(),
                move_speed: form.body.move_speed,
            });
        }
        for (&id, &level) in &mover.mutations {
            if level == 0 {
                continue;
            }
            let innate: u32 = mover
                .species
                .mutations
                .iter()
                .filter(|innate| innate.mutation.0 == id)
                .map(|innate| innate.levels)
                .sum();
            reasons.push(PaceReason::Mutation {
                mutation: id.to_owned(),
                level,
                from_species: innate > 0,
                suppressed: mover.mutation(id) == 0,
            });
        }
        reasons.extend(mover.banes.iter().map(|&bane| PaceReason::Bane {
            bane: bane.to_owned(),
        }));
        if let Some((god, piety_rank)) = &self.slowing_god {
            reasons.push(PaceReason::God {
                god: god.clone(),
                piety_rank: *piety_rank,
            });
        }
        if mover.ponderous > 0 {
            reasons.push(PaceReason::Ponderous {
                pieces: u32::try_from(mover.ponderous).unwrap_or(0),
            });
        }
        if mover.quickened {
            reasons.push(PaceReason::Quickened);
        }
        if mover.water_walking {
            reasons.push(PaceReason::WaterWalking);
        }
        reasons
    }
}

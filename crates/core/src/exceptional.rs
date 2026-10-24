//! Exceptional monsters: the hostiles in view the player should recognise as out of the
//! ordinary, uniques and monsters out of depth, with the Threat Level each makes on its own.

use copilot_gk::Bundle;
use serde::Serialize;

use crate::fact_sheet::{ThreatLevel, attacks_player};
use crate::visible_state::{Position, VisibleMonster, VisibleState};

/// A monster is Out of Depth when the shallowest level of the branch crawl makes its kind on at
/// random is at least this many levels deeper than the player is. From the spec (#24,
/// "Detectors and Suggested Action", trigger 3).
const OUT_OF_DEPTH_LEVELS: u32 = 3;

/// A hostile in view that is a unique or Out of Depth.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExceptionalMonster {
    /// Its name, as the game names it.
    pub name: String,
    /// The square it is on.
    pub position: Position,
    /// What makes it exceptional.
    pub why: Exceptional,
    /// The Threat Level it makes on its own, judged as if no other hostile were in view: whether
    /// it alone makes the moment DANGEROUS. `None` when the Advisor can't judge one.
    pub alone: Option<ThreatLevel>,
}

/// What makes a monster exceptional.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "is", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Exceptional {
    /// There is only one of it, such as Sigmund.
    Unique,
    /// It is Out of Depth: crawl makes its kind at random only deeper in this branch.
    OutOfDepth {
        /// The shallowest level of the branch crawl makes its kind on at random.
        from_depth: u32,
        /// How many levels deeper than the player that is.
        levels: u32,
    },
}

/// The exceptional hostiles in `visible`, in map order, with the Threat Level each makes on its
/// own: `overall`, the level of `visible`, for the only hostile in view, and as `alone` judges a
/// Visible State with it the only hostile otherwise. Firewood stays in view, as scenery, and so
/// do monsters that don't attack the player. Without `knowledge` none is known to be exceptional.
pub(crate) fn in_view(
    visible: &VisibleState,
    knowledge: Option<&Bundle>,
    overall: Option<ThreatLevel>,
    alone: impl Fn(&VisibleState) -> Option<ThreatLevel>,
) -> Vec<ExceptionalMonster> {
    let (Some(bundle), Some(monsters)) = (knowledge, &visible.monsters) else {
        return Vec::new();
    };
    let fights = |monster: &VisibleMonster| {
        monster.attitude.is_some_and(attacks_player)
            && monster
                .id
                .as_ref()
                .and_then(|id| bundle.monster_by_id(id))
                .is_none_or(|kind| !kind.firewood)
    };
    let mut exceptional = Vec::new();
    for (at, monster) in monsters.iter().enumerate() {
        let Some(why) = fights(monster)
            .then(|| why_exceptional(visible, monster, bundle))
            .flatten()
        else {
            continue;
        };
        let others = monsters
            .iter()
            .enumerate()
            .any(|(other, monster)| other != at && fights(monster));
        let level = if others {
            let mut on_its_own = visible.clone();
            on_its_own.monsters = Some(
                monsters
                    .iter()
                    .enumerate()
                    .filter(|&(other, monster)| other == at || !fights(monster))
                    .map(|(_, monster)| monster.clone())
                    .collect(),
            );
            alone(&on_its_own)
        } else {
            overall
        };
        exceptional.push(ExceptionalMonster {
            name: monster.name.clone(),
            position: monster.position,
            why,
            alone: level,
        });
    }
    exceptional
}

/// What makes `monster` exceptional where the player is, if anything does: a unique, or a kind
/// crawl makes at random in this branch only [`OUT_OF_DEPTH_LEVELS`] or more levels deeper. A
/// kind crawl doesn't make at random in this branch at all isn't judged Out of Depth.
fn why_exceptional(
    visible: &VisibleState,
    monster: &VisibleMonster,
    bundle: &Bundle,
) -> Option<Exceptional> {
    let kind = bundle.monster_by_id(monster.id.as_ref()?)?;
    if kind.unique {
        return Some(Exceptional::Unique);
    }
    let place = visible.place.as_ref()?;
    // A branch one level deep reports no depth: its one level is its first.
    let depth = place.depth.unwrap_or(1);
    let from_depth = *bundle
        .branch_named(&place.branch)?
        .shallowest
        .get(&kind.id)?;
    let levels = from_depth.checked_sub(depth)?;
    (levels >= OUT_OF_DEPTH_LEVELS).then_some(Exceptional::OutOfDepth { from_depth, levels })
}

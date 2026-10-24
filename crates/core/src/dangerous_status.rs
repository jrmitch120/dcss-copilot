//! Dangerous Statuses: what the status pane shows on the player that the Advisor raises an Alert
//! for as it lands, whatever is in view.

use copilot_gk::{Bundle, Terrain};
use serde::Serialize;

use crate::visible_state::{Position, VisibleState};

/// A Dangerous Status: one on the player that puts them in danger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DangerousStatus {
    /// Poison that will kill them as it runs its course, unless they cure it.
    LethalPoison,
    /// Turning to stone, or stone already, which leaves them unable to act.
    Petrification,
    /// Burning in liquid fire, the sticky flame, which burns them each turn until put out.
    LiquidFire,
    /// Confusion beside deep water or lava, as the spec names it (#24). Crawl 0.34 stops a
    /// confused step into either and wastes it (`_adjust_confused_movement`), and warns before
    /// one only while flight or a form that keeps the player safe may run out
    /// (`cancel_confused_move`); whether they fly or swim isn't weighed.
    ConfusionBesideDeepWaterOrLava,
}

/// The dangerous statuses `visible` shows on the player, by the signs in `knowledge`, in the
/// order [`DangerousStatus`] lists them. Without `knowledge`, or before the status pane is
/// reported, none is known.
pub(crate) fn on_the_player(
    visible: &VisibleState,
    knowledge: Option<&Bundle>,
) -> Vec<DangerousStatus> {
    let (Some(bundle), Some(statuses)) = (knowledge, &visible.statuses) else {
        return Vec::new();
    };
    let signs = &bundle.danger;
    let lit = |lights: &[String]| {
        statuses.iter().any(|status| {
            status
                .light
                .as_ref()
                .is_some_and(|light| lights.contains(light))
        })
    };
    let mut dangerous = Vec::new();
    if statuses.iter().any(|status| {
        status
            .text
            .as_ref()
            .is_some_and(|text| text.starts_with(&signs.lethal_poison))
    }) {
        dangerous.push(DangerousStatus::LethalPoison);
    }
    if lit(&signs.petrification_lights) {
        dangerous.push(DangerousStatus::Petrification);
    }
    if lit(&signs.liquid_fire_lights) {
        dangerous.push(DangerousStatus::LiquidFire);
    }
    if lit(&signs.confusion_lights) && beside_deep_water_or_lava(visible, bundle) {
        dangerous.push(DangerousStatus::ConfusionBesideDeepWaterOrLava);
    }
    dangerous
}

/// Whether a square beside the player is deep water or lava, as the map shows it.
fn beside_deep_water_or_lava(visible: &VisibleState, bundle: &Bundle) -> bool {
    let (Some(player), Some(map)) = (visible.position, &visible.map) else {
        return false;
    };
    (-1..=1)
        .flat_map(|dy| (-1..=1).map(move |dx| (dx, dy)))
        .filter(|&step| step != (0, 0))
        .filter_map(|(dx, dy)| {
            map.cell(Position {
                x: player.x + dx,
                y: player.y + dy,
            })
        })
        .filter_map(|cell| bundle.feature_by_id(cell.feature.as_ref()?))
        .any(|feature| matches!(feature.terrain, Terrain::DeepWater | Terrain::Lava))
}

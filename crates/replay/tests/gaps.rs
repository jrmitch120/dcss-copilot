//! Seam 1 for Gaps: Recordings and Scenarios go in; the Gaps the Fact Sheet lists for a moment,
//! the values the panel marks in place, the Gaps chip and the prompt card come out.

// Helpers outside #[test] functions panic on bad fixtures too.
#![allow(clippy::unwrap_used)]

use copilot_core::Gap;
use copilot_replay::{Recording, Replayer};

/// A Minotaur Berserker, who has read none of their screens, shot at with a longbow of flaming.
const CENTAUR: &[u8] = include_bytes!("scenarios/centaur-shoots-a-flaming-longbow.dcssrec");

fn replayed(recording: &[u8]) -> Replayer {
    let mut replayer = Replayer::new(Recording::read(recording).unwrap()).unwrap();
    replayer.seek(usize::MAX);
    replayer
}

#[test]
fn the_fact_sheet_lists_the_gaps_that_affected_the_moment() {
    // The fire the bow adds is played as if the character resists none, as the character
    // overview is unread; the fight waits for the skills screen; the Ways Out for the ability
    // menu and the spell list.
    let replayer = replayed(CENTAUR);

    assert_eq!(
        replayer.advisor().fact_sheet().gaps,
        [
            Gap::AbilityMenu,
            Gap::SpellList,
            Gap::Skills,
            Gap::Resistances
        ]
    );
}

//! The Ways Out as the panel lists them: a ✓ or ✕ for each, with why not, in words.

use copilot_gk::{Relief, StairDirection};

use super::{Blocker, Escape, Ground, ReadOn, Usable, WayOut, WaysOut};
use crate::fact_sheet::Gap;
use crate::view_model::{WayOutRow, WaysOutList, capitalised};

impl WaysOut {
    /// The panel's list: the nearest usable staircase each way, or else the nearest, then each
    /// escape carried, ability and spell.
    pub(crate) fn list(&self) -> WaysOutList {
        let stairs = [StairDirection::Down, StairDirection::Up]
            .into_iter()
            .filter_map(|direction| {
                let each_way = || {
                    self.options.iter().filter(move |way_out| {
                        matches!(way_out.option, Escape::Stairs { direction: way, .. } if way == direction)
                    })
                };
                each_way()
                    .find(|way_out| way_out.usable == Usable::Yes)
                    .or_else(|| each_way().next())
            });
        let carried = self
            .options
            .iter()
            .filter(|way_out| !matches!(way_out.option, Escape::Stairs { .. }));
        WaysOutList {
            rows: stairs.chain(carried).map(row).collect(),
            ground: self.ground.as_ref().map(ground),
            gaps: self.gaps.iter().map(gap).collect(),
        }
    }
}

/// A Way Out's row.
fn row(way_out: &WayOut) -> WayOutRow {
    let (name, detail) = match &way_out.option {
        Escape::Stairs { name, steps, .. } => (capitalised(name), Some(steps_away(*steps))),
        Escape::Item {
            kind_name,
            quantity,
            effect,
            ..
        } => {
            let name = capitalised(kind_name);
            let name = if *quantity > 1 {
                format!("{name} ×{quantity}")
            } else {
                name
            };
            (name, Some(relief(*effect).to_owned()))
        }
        Escape::Ability {
            name,
            failure_percent,
            read,
            ..
        } => (
            name.clone(),
            Some(failing("ability", *failure_percent, *read)),
        ),
        Escape::Spell {
            name,
            failure_percent,
            read,
            ..
        } => (
            name.clone(),
            Some(failing("spell", *failure_percent, *read)),
        ),
    };
    let (usable, why_not) = match &way_out.usable {
        Usable::Yes => (true, None),
        Usable::No { because } => (
            false,
            Some(because.iter().map(blocker).collect::<Vec<_>>().join("; ")),
        ),
    };
    WayOutRow {
        usable,
        name,
        detail,
        why_not,
    }
}

/// How far a staircase is, such as `7 steps`.
fn steps_away(steps: u32) -> String {
    match steps {
        0 => "underfoot".to_owned(),
        1 => "1 step".to_owned(),
        steps => format!("{steps} steps"),
    }
}

/// What a Way Out does, in a few words.
fn relief(effect: Relief) -> &'static str {
    match effect {
        Relief::Heals => "heals",
        Relief::Escapes => "escapes",
        Relief::EscapesLater => "escapes after a delay",
        Relief::Hinders => "hinders monsters",
        Relief::Hastens => "hastens you",
        Relief::Strengthens => "strengthens you",
    }
}

/// What an ability or spell is and how often it fails, `~` before a Stale rate with the turn
/// it was read on.
fn failing(what: &str, percent: u32, read: ReadOn) -> String {
    match (read.stale, read.turn) {
        (true, Some(turn)) => format!("{what} · ~{percent}% fail as of turn {turn}"),
        (true, None) => format!("{what} · ~{percent}% fail"),
        (false, _) => format!("{what} · {percent}% fail"),
    }
}

/// Why a Way Out can't be used, in words.
fn blocker(blocker: &Blocker) -> String {
    match blocker {
        Blocker::InTheWay { by } => {
            first_and_more(by, |monster| &monster.name, |_| "is in the way")
        }
        Blocker::CutOff { by } => first_and_more(
            by,
            |monster| &monster.name,
            |first| {
                if first.moves == 0 {
                    "is beside it"
                } else if first.faster {
                    "is faster"
                } else {
                    "can get there first"
                }
            },
        ),
        Blocker::Followed { by } => {
            first_and_more(by, |monster| &monster.name, |_| "would follow you")
        }
        Blocker::Status { status } => format!("you're {status}"),
        Blocker::Magic { needs, has } => format!("needs {needs} MP, you have {has}"),
        Blocker::Useless => "the game greys it out".to_owned(),
    }
}

/// What the first of `monsters` does, by `name` and `how`, and how many more do likewise, such
/// as `Jackal is faster, and 1 more`.
fn first_and_more<T>(
    monsters: &[T],
    name: impl Fn(&T) -> &String,
    how: impl Fn(&T) -> &'static str,
) -> String {
    let Some(first) = monsters.first() else {
        return String::new();
    };
    let more = match monsters.len() - 1 {
        0 => String::new(),
        more => format!(", and {more} more"),
    };
    format!("{} {}{more}", capitalised(name(first)), how(first))
}

/// The ground the player stands on, in words.
fn ground(ground: &Ground) -> String {
    let sides = |open: u32| match open {
        1 => "1 side open".to_owned(),
        open => format!("{open} sides open"),
    };
    match ground {
        Ground::Corridor { open_sides } => format!("In a corridor: {}", sides(*open_sides)),
        Ground::OpenGround {
            open_sides,
            nearest_corridor,
        } => {
            let corridor = nearest_corridor.map_or_else(String::new, |square| {
                format!(" · a corridor {} away", steps_away(square.steps))
            });
            format!("On open ground: {}{corridor}", sides(*open_sides))
        }
    }
}

/// What a Gap keeps the Advisor from judging, and how to fill it where the player can.
fn gap(gap: &Gap) -> String {
    match gap {
        Gap::AbilityMenu => "Open the ability menu (a) to judge your abilities.".to_owned(),
        Gap::SpellList => "Open the spell list (I) to judge your spells.".to_owned(),
        Gap::Map | Gap::MonstersInView => {
            "The stairs can't be judged until the game sends its map.".to_owned()
        }
        Gap::Position => {
            "The stairs can't be judged until the game says where you stand.".to_owned()
        }
        Gap::GameKnowledge => {
            "Nothing can be judged without Game Knowledge for this version.".to_owned()
        }
        Gap::MonsterKnowledge { monster } => format!(
            "{} isn't in the Game Knowledge, so it isn't counted.",
            capitalised(monster)
        ),
        Gap::HitPoints | Gap::Defences => "The game hasn't sent your stats yet.".to_owned(),
    }
}

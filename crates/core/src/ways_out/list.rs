//! The Ways Out as the panel lists them: a ✓ or ✕ for each, with why not, in words.

use copilot_gk::{Relief, StairDirection};

use super::{Blocker, Escape, Ground, ReadOn, Usable, WayOut, WaysOut};
use crate::gaps::{Gap, ScreenFact};
use crate::view_model::{MarkedValue, WayOutRow, WaysOutList, capitalised};

impl WaysOut {
    /// The panel's list: the nearest usable staircase each way, or else the nearest, the nearer
    /// of the two first, then each escape carried, ability and spell.
    pub(crate) fn list(&self) -> WaysOutList {
        let shown: Vec<&WayOut> = [StairDirection::Down, StairDirection::Up]
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
            })
            .collect();
        // The staircases come nearest first, so keeping their order puts the nearer one first.
        let stairs = self
            .options
            .iter()
            .filter(|way_out| shown.iter().any(|shown| std::ptr::eq(*shown, *way_out)));
        let carried = self
            .options
            .iter()
            .filter(|way_out| !matches!(way_out.option, Escape::Stairs { .. }));
        // A staircase's verdict leans on the player's Pace, and so on what it was judged around.
        let pace: Vec<String> = self
            .gaps
            .iter()
            .filter(|gap| {
                matches!(
                    gap.screen_fact(),
                    Some(ScreenFact::Mutations | ScreenFact::Equipped)
                )
            })
            .map(Gap::sentence)
            .collect();
        let (mut hidden, said): (Vec<&Gap>, Vec<&Gap>) =
            self.gaps.iter().partition(|gap| hidden_row(gap).is_some());
        // Abilities before spells, as the rows of those read are.
        hidden.sort_by_key(|gap| {
            !matches!(
                gap,
                Gap::Unread {
                    fact: ScreenFact::AbilityMenu
                }
            )
        });
        WaysOutList {
            rows: stairs
                .chain(carried)
                .map(|way_out| row(way_out, &pace))
                .chain(hidden.into_iter().filter_map(hidden_row))
                .collect(),
            ground: self.ground.as_ref().map(ground),
            gaps: said.into_iter().map(ways_out_gap).collect(),
        }
    }
}

impl Escape {
    /// The Way Out as one line, such as `Stone staircase leading down · 7 steps`: its name, then
    /// the fact its row shows beside it.
    pub(crate) fn summary(&self) -> String {
        match self.named() {
            (name, Some(detail)) => format!("{name} · {detail}"),
            (name, None) => name,
        }
    }

    /// The Way Out as one line, as [`Escape::summary`] says it, marked when the menu or list it
    /// was read off is out of date.
    pub(crate) fn value(&self) -> MarkedValue {
        MarkedValue::marked(self.summary(), self.gap().as_ref().map(Gap::sentence))
    }

    /// The Gap it was judged around: the ability menu or spell list it was read off out of
    /// date.
    fn gap(&self) -> Option<Gap> {
        match self {
            Escape::Ability { read, .. } if read.stale => Some(Gap::Stale {
                fact: ScreenFact::AbilityMenu,
                turn: read.turn,
            }),
            Escape::Spell { read, .. } if read.stale => Some(Gap::Stale {
                fact: ScreenFact::SpellList,
                turn: read.turn,
            }),
            Escape::Stairs { .. }
            | Escape::Item { .. }
            | Escape::Ability { .. }
            | Escape::Spell { .. } => None,
        }
    }

    /// Its name as the panel lists it, and a short fact beside it.
    fn named(&self) -> (String, Option<String>) {
        match self {
            Escape::Stairs {
                name,
                steps,
                over_remembered_squares,
                ..
            } => (
                capitalised(name),
                Some(stairs_away(*steps, *over_remembered_squares)),
            ),
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
        }
    }
}

/// The row for the Ways Out `gap` hides, if it hides any: the abilities of an ability menu, or
/// the spells of a spell list, never read, and a spell memorised the game didn't name. Whether
/// they can be used is unknown, and the row says so, marked with what fills it.
fn hidden_row(gap: &Gap) -> Option<WayOutRow> {
    let name = match gap {
        Gap::Unread {
            fact: ScreenFact::AbilityMenu,
        } => "Abilities".to_owned(),
        Gap::Unread {
            fact: ScreenFact::SpellList,
        } => "Spells".to_owned(),
        Gap::SpellMemorised { turn: Some(turn) } => format!("A spell memorised on turn {turn}"),
        Gap::SpellMemorised { turn: None } => "A spell memorised".to_owned(),
        Gap::HitPoints
        | Gap::Defences
        | Gap::MonstersInView
        | Gap::Map
        | Gap::Position
        | Gap::GameKnowledge
        | Gap::MonsterKnowledge { .. }
        | Gap::Inventory
        | Gap::Wielded
        | Gap::Attributes
        | Gap::Unread {
            fact:
                ScreenFact::Skills
                | ScreenFact::SpellPower
                | ScreenFact::Mutations
                | ScreenFact::Resistances
                | ScreenFact::Equipped,
        }
        | Gap::Stale { .. }
        | Gap::SkillsShifting { .. } => return None,
    };
    Some(WayOutRow {
        usable: None,
        name,
        detail: Some(MarkedValue::marked(
            "unknown".to_owned(),
            [ways_out_gap(gap)],
        )),
        why_not: None,
    })
}

/// A Way Out's row, a staircase's marked with `pace`, what its race was judged around.
fn row(way_out: &WayOut, pace: &[String]) -> WayOutRow {
    let (name, detail) = way_out.option.named();
    let menu_gap = way_out.option.gap();
    let marks: Vec<String> = match way_out.option {
        Escape::Stairs { .. } => pace.to_vec(),
        Escape::Item { .. } | Escape::Ability { .. } | Escape::Spell { .. } => {
            menu_gap.iter().map(Gap::sentence).collect()
        }
    };
    let (usable, why_not) = match &way_out.usable {
        Usable::Yes => (true, None),
        Usable::No { because } => {
            let why_not = because.iter().map(blocker).collect::<Vec<_>>().join("; ");
            // Only the menu's greying out rests on what was read off it.
            let greyed_out = because
                .iter()
                .any(|blocker| matches!(blocker, Blocker::GreyedOut { .. }));
            let gap = menu_gap.as_ref().filter(|_| greyed_out).map(Gap::sentence);
            (false, Some(MarkedValue::marked(why_not, gap)))
        }
    };
    WayOutRow {
        usable: Some(usable),
        name,
        detail: detail.map(|detail| MarkedValue::marked(detail, marks)),
        why_not,
    }
}

/// How far a staircase is, `~` before it and saying so when the way there leaves view.
fn stairs_away(steps: u32, over_remembered_squares: bool) -> String {
    let away = steps_away(steps);
    if over_remembered_squares {
        format!("~{away} · partly out of view")
    } else {
        away
    }
}

/// How far a square is, such as `7 steps`.
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
        Blocker::FullHealth => "you're at full health".to_owned(),
        Blocker::Useless => "no use right now".to_owned(),
        Blocker::GreyedOut { read } => {
            let stale = if read.stale { "~" } else { "" };
            match read.turn {
                Some(turn) => format!("{stale}greyed out in the ability menu as of turn {turn}"),
                None => format!("{stale}greyed out in the ability menu"),
            }
        }
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

/// What a Gap keeps the Advisor from judging of the Ways Out, and how to fill it where the
/// player can.
fn ways_out_gap(gap: &Gap) -> String {
    match gap {
        Gap::Map | Gap::MonstersInView => {
            "The stairs can't be judged until the game sends its map.".to_owned()
        }
        Gap::Position => {
            "The stairs can't be judged until the game says where you stand.".to_owned()
        }
        Gap::HitPoints
        | Gap::Defences
        | Gap::GameKnowledge
        | Gap::MonsterKnowledge { .. }
        | Gap::Inventory
        | Gap::Wielded
        | Gap::Attributes
        | Gap::Unread { .. }
        | Gap::Stale { .. }
        | Gap::SpellMemorised { .. }
        | Gap::SkillsShifting { .. } => gap.sentence(),
    }
}

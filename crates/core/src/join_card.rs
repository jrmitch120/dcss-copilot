//! The join card: what the panel asks of a player whose game the Advisor joined part way through,
//! and what it tells them once it has read the screen they opened.

use crate::accumulated_state::{AccumulatedState, Backfill, Remembered};
use crate::screen::{DungeonOverview, Screen, ScreenRead};
use crate::view_model::{ChecklistRow, JoinCard};
use crate::visible_state::VisibleState;

/// How far the join card has got for one game, kept only while the app runs: after a restart, it
/// asks again for a game whose Backfill still awaits the dump.
#[derive(Debug, Clone, Default)]
pub(crate) enum Joining {
    /// It asks for the character dump, while the game's Backfill awaits it.
    #[default]
    Asking,
    /// The player put the card away without opening anything.
    NotNow,
    /// The Advisor read the character dump the player opened, while the game's Backfill awaited
    /// it, and this is what it read.
    ReadDump { checklist: Vec<ChecklistRow> },
    /// The Advisor read the dungeon overview the player opened while it asked, and this is what
    /// it read of the levels.
    ReadOverview { levels: String },
    /// The player put the card away after it said what it read.
    Done,
}

impl Joining {
    /// Takes in what the player opened, which `accumulated` has just remembered, when the game's
    /// Backfill was `before` it.
    pub(crate) fn read(
        &mut self,
        read: &ScreenRead,
        before: Backfill,
        accumulated: &AccumulatedState,
    ) {
        if !matches!(before, Backfill::Awaiting { .. }) {
            return;
        }
        match read.screen {
            Screen::CharacterDump => {
                *self = Self::ReadDump {
                    checklist: checklist(accumulated),
                };
            }
            Screen::DungeonOverview if matches!(self, Self::Asking) => {
                if let Some(overview) = accumulated.dungeon.known() {
                    *self = Self::ReadOverview {
                        levels: levels(overview),
                    };
                }
            }
            _ => {}
        }
    }

    /// The player put the card away: "Not now" while it asks, "OK" once it says what it read.
    pub(crate) fn close(&mut self) {
        *self = match self {
            Self::Asking | Self::NotNow => Self::NotNow,
            Self::ReadDump { .. } | Self::ReadOverview { .. } | Self::Done => Self::Done,
        };
    }

    /// What the card shows now, as `visible` shows the game and its Backfill is `backfill`, if it
    /// shows at all.
    pub(crate) fn card(&self, backfill: Backfill, visible: &VisibleState) -> Option<JoinCard> {
        match self {
            Self::Asking => match backfill {
                Backfill::Awaiting { joined_on } => Some(JoinCard::Asking { turn: joined_on }),
                Backfill::Complete => None,
            },
            Self::ReadDump { checklist } => {
                let dump_open = visible
                    .screen
                    .as_ref()
                    .is_some_and(|read| read.screen == Screen::CharacterDump);
                let checklist = checklist.clone();
                Some(if dump_open {
                    JoinCard::Reading { checklist }
                } else {
                    JoinCard::CaughtUpFromDump { checklist }
                })
            }
            Self::ReadOverview { levels } => Some(JoinCard::CaughtUpFromOverview {
                levels: levels.clone(),
            }),
            Self::NotNow | Self::Done => None,
        }
    }
}

/// What `accumulated` holds that it read off the character dump, one row for each part the
/// Advisor reads.
fn checklist(accumulated: &AccumulatedState) -> Vec<ChecklistRow> {
    let row = |line: Option<String>, missing: &str| match line {
        Some(line) => ChecklistRow { read: true, line },
        None => ChecklistRow {
            read: false,
            line: missing.to_owned(),
        },
    };
    vec![
        row(
            off_dump(&accumulated.skills)
                .map(|skills| format!("Skills ({}, with decimals)", skills.tenths.len())),
            "Skills: not in your dump",
        ),
        row(
            off_dump(&accumulated.spells).map(|spells| match spells.len() {
                0 => "Spells: none memorised".to_owned(),
                known => format!("Spells ({known}, with failure rates)"),
            }),
            "Spells: not in your dump",
        ),
        row(
            off_dump(&accumulated.mutations).map(|mutations| {
                let shown = &mutations.shown;
                match shown.banes.len() {
                    0 => format!("Mutations ({})", shown.mutations.len()),
                    banes => format!("Mutations ({}) and banes ({banes})", shown.mutations.len()),
                }
            }),
            "Mutations: none listed in your dump",
        ),
        row(
            off_dump(&accumulated.dungeon)
                .map(|overview| format!("Dungeon overview: {}", levels(overview))),
            "Dungeon overview: not in your dump",
        ),
    ]
}

/// The fact, when it is Known as read off the character dump.
fn off_dump<T>(fact: &Remembered<T>) -> Option<&T> {
    match fact {
        Remembered::Known(read) if read.source == Screen::CharacterDump => Some(&read.value),
        _ => None,
    }
}

/// What `overview` shows of the levels, in a few words, such as `3 branches, 1 altar, 2 shops`:
/// the altars only when it tells which were found.
fn levels(overview: &DungeonOverview) -> String {
    let shops: u32 = overview.shops.iter().map(|shops| shops.count).sum();
    let mut parts = vec![counted(overview.branches.len(), "branch", "branches")];
    if let Some(altars) = &overview.altars {
        parts.push(counted(altars.len(), "altar", "altars"));
    }
    parts.push(counted(
        usize::try_from(shops).unwrap_or(usize::MAX),
        "shop",
        "shops",
    ));
    parts.join(", ")
}

/// `count` things, as `no shops`, `1 shop` or `3 shops`.
fn counted(count: usize, one: &str, many: &str) -> String {
    match count {
        0 => format!("no {many}"),
        1 => format!("1 {one}"),
        count => format!("{count} {many}"),
    }
}

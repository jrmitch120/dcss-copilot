//! The join card: what the panel asks of a player whose game the Advisor joined part way through,
//! and what it tells them once it has read the screen they opened.

use crate::accumulated_state::{AccumulatedState, Backfill, Remembered, Source};
use crate::screen::{DungeonOverview, Screen, ScreenRead};
use crate::view_model::{ChecklistRow, JoinCard, UnreadPart};
use crate::visible_state::VisibleState;

/// How far the join card has got for one game, kept only while the app runs: after a restart, it
/// asks again for a game whose Backfill still awaits the dump.
#[derive(Debug, Clone, Default)]
pub(crate) enum Joining {
    /// It asks for the character dump, while the game's Backfill awaits it.
    #[default]
    Asking,
    /// The Advisor read the character dump the player opened, while the game's Backfill awaited
    /// it, and this is what it read of each part, if it showed it.
    ReadDump { parts: Vec<PartRead> },
    /// The Advisor read the dungeon overview the player opened while it asked, and this is what
    /// it read of the levels.
    ReadOverview { levels: String },
    /// The player put the card away: with Not now, or with OK once it said what it read.
    PutAway,
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
                    parts: read_off_dump(accumulated),
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
        *self = Self::PutAway;
    }

    /// What the card shows now, as `visible` shows the game and its Backfill is `backfill`, if it
    /// shows at all.
    pub(crate) fn card(&self, backfill: Backfill, visible: &VisibleState) -> Option<JoinCard> {
        match self {
            Self::Asking => match backfill {
                Backfill::Awaiting { joined_on } => Some(JoinCard::Asking { turn: joined_on }),
                Backfill::Complete => None,
            },
            Self::ReadDump { parts } => {
                let dump_open = visible
                    .screen
                    .as_ref()
                    .is_some_and(|read| read.screen == Screen::CharacterDump);
                Some(if dump_open {
                    JoinCard::Reading {
                        checklist: parts.iter().map(PartRead::row).collect(),
                    }
                } else {
                    JoinCard::CaughtUpFromDump {
                        current: parts
                            .iter()
                            .filter(|read| read.off_dump.is_some())
                            .map(|read| read.part.phrase().to_owned())
                            .collect(),
                        not_in_dump: parts.iter().filter_map(PartRead::unread).collect(),
                    }
                })
            }
            Self::ReadOverview { levels } => Some(JoinCard::CaughtUpFromOverview {
                levels: levels.clone(),
            }),
            Self::PutAway => None,
        }
    }
}

/// A part of the character dump the Advisor reads, as the join card lists it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DumpPart {
    Skills,
    Spells,
    Mutations,
    DungeonOverview,
}

/// What the Advisor read of one part of the character dump.
#[derive(Debug, Clone)]
pub(crate) struct PartRead {
    part: DumpPart,
    /// What the dump showed of it, as the card's row says it, such as `Skills (12, with
    /// decimals)`: `None` when the dump didn't show it.
    off_dump: Option<String>,
    /// The Advisor knows it all the same, as read now off the dump or before off its own screen.
    known: bool,
}

impl DumpPart {
    /// Every part, in the order the card lists them.
    const ALL: [Self; 4] = [
        Self::Skills,
        Self::Spells,
        Self::Mutations,
        Self::DungeonOverview,
    ];

    /// Its name, as a row of the checklist starts.
    fn name(self) -> &'static str {
        match self {
            Self::Skills => "Skills",
            Self::Spells => "Spells",
            Self::Mutations => "Mutations",
            Self::DungeonOverview => "Dungeon overview",
        }
    }

    /// It, as a sentence names it, such as `the dungeon overview`.
    fn phrase(self) -> &'static str {
        match self {
            Self::Skills => "skills",
            Self::Spells => "spells",
            Self::Mutations => "mutations",
            Self::DungeonOverview => "the dungeon overview",
        }
    }

    /// The keys that open the screen showing it on its own.
    fn keys(self) -> &'static [&'static str] {
        match self {
            Self::Skills => &["m"],
            Self::Spells => &["I"],
            Self::Mutations => &["A"],
            Self::DungeonOverview => &["Ctrl", "O"],
        }
    }

    /// What `accumulated` holds of it.
    fn read(self, accumulated: &AccumulatedState) -> PartRead {
        let (off_dump, known) = match self {
            Self::Skills => (
                off_dump(&accumulated.skills)
                    .map(|skills| format!("Skills ({}, with decimals)", skills.tenths.len())),
                accumulated.skills.known().is_some(),
            ),
            Self::Spells => (
                off_dump(&accumulated.spells).map(|spells| match spells.len() {
                    0 => "Spells: none memorised".to_owned(),
                    known => format!("Spells ({known}, with failure rates)"),
                }),
                accumulated.spells.known().is_some(),
            ),
            Self::Mutations => (
                off_dump(&accumulated.mutations).map(|read| {
                    let shown = &read.shown;
                    let banes = shown.banes.as_ref().map_or(0, Vec::len);
                    match (shown.mutations.len(), banes) {
                        (0, 0) => "Mutations: none".to_owned(),
                        (mutations, 0) => format!("Mutations ({mutations})"),
                        (mutations, banes) => {
                            format!("Mutations ({mutations}) and banes ({banes})")
                        }
                    }
                }),
                accumulated.mutations.known().is_some(),
            ),
            Self::DungeonOverview => (
                off_dump(&accumulated.dungeon)
                    .map(|overview| format!("Dungeon overview: {}", levels(overview))),
                accumulated.dungeon.known().is_some(),
            ),
        };
        PartRead {
            part: self,
            off_dump,
            known,
        }
    }
}

impl PartRead {
    /// Its row on the checklist.
    fn row(&self) -> ChecklistRow {
        match &self.off_dump {
            Some(line) => ChecklistRow {
                read: true,
                line: line.clone(),
            },
            None => ChecklistRow {
                read: false,
                line: format!("{}: not in your dump", self.part.name()),
            },
        }
    }

    /// It, as the card names a part the dump didn't show and the Advisor doesn't know: `None`
    /// when it isn't one.
    fn unread(&self) -> Option<UnreadPart> {
        (self.off_dump.is_none() && !self.known).then(|| UnreadPart {
            name: self.part.phrase().to_owned(),
            keys: self.part.keys().iter().map(|&key| key.to_owned()).collect(),
        })
    }
}

/// What `accumulated` holds of each part of the character dump the Advisor reads.
fn read_off_dump(accumulated: &AccumulatedState) -> Vec<PartRead> {
    DumpPart::ALL
        .into_iter()
        .map(|part| part.read(accumulated))
        .collect()
}

/// The fact, when it is Known as read off the character dump.
fn off_dump<T>(fact: &Remembered<T>) -> Option<&T> {
    match fact {
        Remembered::Known(read) if read.source == Source::Screen(Screen::CharacterDump) => {
            Some(&read.value)
        }
        _ => None,
    }
}

/// What `overview` shows of the levels, in a few words, such as `3 branches, 1 altar, 2 shops`:
/// the altars only when it tells which were found.
fn levels(overview: &DungeonOverview) -> String {
    let shops = overview
        .shops
        .iter()
        .map(|shops| usize::try_from(shops.count).unwrap_or(usize::MAX))
        .fold(0, usize::saturating_add);
    let mut parts = vec![counted(overview.branches.len(), "branch", "branches")];
    if let Some(altars) = &overview.altars {
        parts.push(counted(altars.len(), "altar", "altars"));
    }
    parts.push(counted(shops, "shop", "shops"));
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    /// The Scenario's dump shows every part; a player's options can leave some out.
    #[test]
    fn caught_up_from_a_dump_names_each_part_it_didnt_show_with_the_keys_that_show_it() {
        let dump = ScreenRead {
            screen: Screen::CharacterDump,
            title: None,
            lines: [
                "A: no striking features",
                "",
                "   Skills:",
                " + Level 5.3 Fighting",
            ]
            .map(str::to_owned)
            .to_vec(),
            colours: BTreeMap::new(),
        };
        let mut accumulated = AccumulatedState::default();
        accumulated.read(&dump, &VisibleState::default(), None);
        let mut joining = Joining::Asking;
        joining.read(
            &dump,
            Backfill::Awaiting { joined_on: Some(9) },
            &accumulated,
        );

        let reading = VisibleState {
            screen: Some(dump),
            ..VisibleState::default()
        };
        let Some(JoinCard::Reading { checklist }) = joining.card(Backfill::Complete, &reading)
        else {
            panic!("not reading");
        };
        let rows: Vec<_> = checklist
            .iter()
            .map(|row| (row.read, row.line.as_str()))
            .collect();
        assert_eq!(
            rows,
            [
                (true, "Skills (1, with decimals)"),
                (false, "Spells: not in your dump"),
                (true, "Mutations: none"),
                (false, "Dungeon overview: not in your dump"),
            ]
        );
        let closed = VisibleState::default();
        let Some(JoinCard::CaughtUpFromDump {
            current,
            not_in_dump,
        }) = joining.card(Backfill::Complete, &closed)
        else {
            panic!("not caught up");
        };
        assert_eq!(current, ["skills", "mutations"]);
        let unread: Vec<_> = not_in_dump
            .iter()
            .map(|part| (part.name.as_str(), part.keys.join(" ")))
            .collect();
        assert_eq!(
            unread,
            [
                ("spells", "I".to_owned()),
                ("the dungeon overview", "Ctrl O".to_owned())
            ]
        );
    }

    #[test]
    fn caught_up_from_a_dump_doesnt_ask_for_a_part_already_read_off_its_own_screen() {
        let screen = |screen: Screen, lines: &[&str]| ScreenRead {
            screen,
            title: None,
            lines: lines.iter().map(|&line| line.to_owned()).collect(),
            colours: BTreeMap::new(),
        };
        let spell_list = screen(
            Screen::Spells,
            &[" a - Magic Dart                      Conjuration               4%       1"],
        );
        let dump = screen(
            Screen::CharacterDump,
            &["   Skills:", " + Level 5.3 Fighting"],
        );
        let mut accumulated = AccumulatedState::default();
        accumulated.read(&spell_list, &VisibleState::default(), None);
        accumulated.read(&dump, &VisibleState::default(), None);
        let mut joining = Joining::Asking;
        joining.read(&dump, Backfill::Awaiting { joined_on: None }, &accumulated);

        let Some(JoinCard::CaughtUpFromDump { not_in_dump, .. }) =
            joining.card(Backfill::Complete, &VisibleState::default())
        else {
            panic!("not caught up");
        };
        let names: Vec<_> = not_in_dump.iter().map(|part| part.name.as_str()).collect();
        assert_eq!(names, ["mutations", "the dungeon overview"]);
    }
}

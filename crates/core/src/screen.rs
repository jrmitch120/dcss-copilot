//! Screen reads: the screens the player opens, read as crawl wrote them, and what the core reads
//! off them.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::view_model::Colour;

/// A screen the player has open, read as plain text: crawl's own words, laid out as crawl lays
/// them out, so the core reads it the same whichever Game Source it came from, with the colour
/// crawl gives a menu's entries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenRead {
    /// Which screen it is.
    pub screen: Screen,
    /// Its title, if it has one.
    pub title: Option<String>,
    /// Its text, line by line, without colours: the rows of a text screen, or one line per
    /// entry of a menu.
    pub lines: Vec<String>,
    /// The colour of each entry of a menu crawl draws in a colour of its own, rather than the
    /// menu's, by its index in `lines`: such as an ability crawl greys out as one that can't be
    /// used now. Empty for a text screen.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub colours: BTreeMap<usize, Colour>,
}

/// Which screen the player has open.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "name", rename_all = "camelCase")]
pub enum Screen {
    /// The character overview (`%`): stats, resistances and equipment.
    CharacterOverview,
    /// The dungeon overview (`Ctrl-O`): branches, altars, shops, portals and annotations.
    DungeonOverview,
    /// The skills screen (`m`).
    Skills,
    /// The spell list (`I`, or choosing a spell to cast).
    Spells,
    /// The ability menu (`a`).
    Abilities,
    /// Innate abilities, weirdness and mutations (`A`).
    Mutations,
    /// The character dump (`#`, or from the help menu).
    CharacterDump,
    /// Any other menu or popup, by crawl's name for it, such as `describe-monster`.
    Other(String),
}

/// The character's resistances, as the character overview and the character dump show them. Each is `None` when the
/// screen doesn't show it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resistances {
    /// `rFire`.
    pub fire: Option<Resistance>,
    /// `rCold`.
    pub cold: Option<Resistance>,
    /// `rNeg`.
    pub negative: Option<Resistance>,
    /// `rPois`.
    pub poison: Option<Resistance>,
    /// `rElec`.
    pub electricity: Option<Resistance>,
    /// `rCorr`.
    pub corrosion: Option<Resistance>,
    /// `SInv`: seeing invisible.
    pub see_invisible: Option<Resistance>,
    /// `Will`: willpower, in pips.
    pub willpower: Option<Resistance>,
}

/// How resistant the character is to one thing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Resistance {
    /// Pips of resistance: positive resists, negative is vulnerable, 0 is neither.
    Pips(i8),
    /// Immune.
    Immune,
}

/// The width of the resistance names in the character overview's first column
/// (`_get_overview_resistances` in crawl's `output.cc`): the pips start after it.
const RESIST_NAME_WIDTH: usize = 8;

/// A spell the character knows, as the spell list shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownSpell {
    /// Its name, such as `Blink`.
    pub name: String,
    /// The chance of failing to cast it, in percent.
    pub failure_percent: u32,
    /// Its level, which is also the magic points it costs.
    pub level: u32,
}

/// A spell's power and range as the spell list shows them once the player switches it to its
/// other columns (`!`): `_spell_extra_description` in crawl's `spl-cast.cc`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpellReach {
    /// Its name, such as `Magic Dart`.
    pub name: String,
    /// The power the character casts it with, `None` for a spell the list gives none.
    pub power: Option<SpellPower>,
    /// How many squares it reaches, `None` for a spell the list gives no range.
    pub range: Option<i32>,
}

/// The power a spell is cast with, as the spell list shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "shown", content = "value", rename_all = "camelCase")]
pub enum SpellPower {
    /// As a percentage of the most power the spell takes, rounded down
    /// (`spell_power_percent`): a range of powers.
    PercentOfCap(u32),
    /// Exactly, as wizard mode shows it beside the most the spell takes
    /// (`_wizard_spell_power_numeric_string`).
    Exact(i32),
}

/// The character's skills, as the skills screen (`m`) or the character dump shows them: each
/// skill's level in tenths, by the skill's name, such as `Maces & Flails`. A skill they haven't
/// trained isn't shown, and is at level 0.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Skills {
    /// Tenths of a level, by name: `you.skill(sk, 10)` as crawl shows it.
    pub tenths: BTreeMap<String, u32>,
}

impl Skills {
    /// The level of the skill named `name` in tenths: 0 when it isn't shown.
    #[must_use]
    pub fn tenths(&self, name: &str) -> u32 {
        self.tenths.get(name).copied().unwrap_or(0)
    }
}

/// An ability the character has, as the ability menu shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownAbility {
    /// Its name, such as `Berserk`.
    pub name: String,
    /// What it costs, in crawl's words, such as `3 MP, Piety` or `None`.
    pub cost: String,
    /// The magic points it costs, none when its cost names none.
    pub magic_cost: u32,
    /// The chance of failing to use it, in percent.
    pub failure_percent: u32,
    /// Crawl greyed its row out as one that can't be used now (`check_ability_possible`). What
    /// the Advisor remembers of the menu keeps this only for a Way Out greyed out for a reason it
    /// couldn't see as the menu was read, such as an empty deck, a sanctuary already in place or
    /// stasis: not one a status it showed then, or too little magic, explains.
    pub greyed_out: bool,
}

impl ScreenRead {
    /// The spells, when this is the spell list showing failure rates rather than its other
    /// columns, or the character dump.
    ///
    /// The spell list draws one per row as `a - Blink    Translocation    4%    2`, the name, the
    /// schools, the failure rate and the level (`_spell_base_description` in crawl's
    /// `spl-cast.cc`); `None` when a row reads otherwise, as the list's other columns do.
    ///
    /// The dump lists them under `You know the following spells:` in columns of fixed width,
    /// with the name cut to fit, so a long one is shown only as far as its first 20 letters
    /// (`_sdump_spells` in `chardump.cc`); it says `You don't know any spells.` for none. `None`
    /// when the dump holds no spells section.
    #[must_use]
    pub fn spells(&self) -> Option<Vec<KnownSpell>> {
        match self.screen {
            Screen::Spells => self
                .lines
                .iter()
                .filter_map(|line| menu_entry(line))
                .map(|row| {
                    let columns = columns(row);
                    let [name, _schools, failure, .., level] = columns.as_slice() else {
                        return None;
                    };
                    // The failure rate may be followed by another, as a revenant's enkindled one.
                    let failure = failure.split_whitespace().next()?;
                    Some(KnownSpell {
                        name: (*name).to_owned(),
                        failure_percent: percent(failure)?,
                        level: level.parse().ok()?,
                    })
                })
                .collect(),
            Screen::CharacterDump => dump_spells(&self.lines),
            _ => None,
        }
    }

    /// The power and range of each spell, when this is the spell list showing them rather than
    /// failure rates: one per row as `a - Magic Dart    100%    1d7    4    Quiet`, the name,
    /// the power, the damage, the range and the noise (`_spell_extra_description` in crawl's
    /// `spl-cast.cc`), or with the power as `25 (50)` in wizard mode. `None` when a row reads
    /// otherwise, as the list's failure rates do.
    #[must_use]
    pub fn spell_reaches(&self) -> Option<Vec<SpellReach>> {
        if self.screen != Screen::Spells {
            return None;
        }
        self.lines
            .iter()
            .filter_map(|line| menu_entry(line))
            .map(|row| {
                let columns = columns(row);
                let [name, power, _damage, range, _noise] = columns.as_slice() else {
                    return None;
                };
                Some(SpellReach {
                    name: (*name).to_owned(),
                    power: spell_power(power)?,
                    range: spell_range(range)?,
                })
            })
            .collect()
    }

    /// The skills, when this is the skills screen or the character dump.
    ///
    /// The skills screen draws each as ` a + Fighting        5.3`, a hotkey, whether it is
    /// trained, its name padded to 15 and its level to one decimal, or whole once mastered,
    /// and two to a row (`SkillMenuEntry::set_name` and `set_level` in crawl's `skill-menu.cc`);
    /// other columns, such as the cost, follow the level. The dump writes one a line as
    /// ` + Level 5.3 Fighting`, with the level in effect in brackets after the trained one when
    /// they differ, such as `5.3(9.3)` (`dump_skills` in `skills.cc`).
    ///
    /// `None` when it shows no skill at all: every character has trained some, so the screen
    /// isn't drawn yet, or the dump holds no skills section.
    #[must_use]
    pub fn skills(&self) -> Option<Skills> {
        let tenths: BTreeMap<String, u32> = match self.screen {
            Screen::Skills => self
                .lines
                .iter()
                .flat_map(|line| menu_skills(line))
                .collect(),
            Screen::CharacterDump => self
                .lines
                .iter()
                .filter_map(|line| dump_skill(line))
                .collect(),
            _ => return None,
        };
        (!tenths.is_empty()).then_some(Skills { tenths })
    }

    /// The abilities, when this is the ability menu: one per row as `a - Berserk    None    0%`,
    /// the name, the cost and the failure rate (`describe_talent` in crawl's `ability.cc`), each
    /// greyed out when its row is in the colour `greyed_out`, if the Advisor knows it.
    #[must_use]
    pub fn abilities(&self, greyed_out: Option<Colour>) -> Option<Vec<KnownAbility>> {
        if self.screen != Screen::Abilities {
            return None;
        }
        self.lines
            .iter()
            .enumerate()
            .filter_map(|(index, line)| Some((index, menu_entry(line)?)))
            .map(|(index, row)| {
                let columns = columns(row);
                let [name, cost, failure] = columns.as_slice() else {
                    return None;
                };
                Some(KnownAbility {
                    name: (*name).to_owned(),
                    cost: (*cost).to_owned(),
                    magic_cost: magic_cost(cost),
                    failure_percent: percent(failure)?,
                    greyed_out: greyed_out.is_some()
                        && self.colours.get(&index) == greyed_out.as_ref(),
                })
            })
            .collect()
    }
}

/// The text of a menu row after its hotkey, as `a - Blink ...` is drawn: `None` for a row
/// without one, such as the ability menu's `Invocations -` subtitle.
fn menu_entry(line: &str) -> Option<&str> {
    let line = line.trim_start();
    let mut chars = line.chars();
    let hotkey = chars.next()?;
    let rest = chars.as_str().strip_prefix(" - ")?;
    hotkey.is_ascii_alphabetic().then_some(rest)
}

/// The columns of a menu row, which crawl pads apart with spaces (`chop_string`): split where
/// two or more spaces run, as none of their texts holds two together.
fn columns(row: &str) -> Vec<&str> {
    row.split("  ")
        .map(str::trim)
        .filter(|column| !column.is_empty())
        .collect()
}

/// A spell's power as the spell list shows it: `85%` of the most it takes, or `25 (50)` in
/// wizard mode; `Some(None)` for `N/A`, and `None` for anything else.
fn spell_power(column: &str) -> Option<Option<SpellPower>> {
    if column == "N/A" {
        return Some(None);
    }
    if let Some(percent) = percent(column) {
        return Some(Some(SpellPower::PercentOfCap(percent)));
    }
    let (power, cap) = column.split_once(' ')?;
    cap.strip_prefix('(')?
        .strip_suffix(')')?
        .parse::<i32>()
        .ok()?;
    Some(Some(SpellPower::Exact(power.parse().ok()?)))
}

/// A spell's range as the spell list shows it (`range_string` in crawl's `spl-cast.cc`): `5`,
/// `4/7` with the range at the most power after it, or `2-5` with the least range it is cast
/// at before it, the range now in each case; `Some(None)` for `N/A`, and `None` for anything
/// else.
fn spell_range(column: &str) -> Option<Option<i32>> {
    if column == "N/A" {
        return Some(None);
    }
    let range = column.rsplit_once('-').map_or(column, |(_, range)| range);
    let range = range.split_once('/').map_or(range, |(now, _)| now);
    Some(Some(range.parse().ok()?))
}

/// What the character dump says before the spells the character knows (`_sdump_spells`).
const DUMP_KNOWN_SPELLS: &str = "You know the following spells:";

/// What the character dump says when the character knows no spells.
const DUMP_NO_SPELLS: &str = "You don't know any spells.";

/// Where each column of the character dump's spells starts, as `chop_string` pads them in
/// `_sdump_spells`: the hotkey and name, then the schools, power, damage, failure rate and level.
const DUMP_SPELL_FAILURE_AT: usize = 62;
const DUMP_SPELL_LEVEL_AT: usize = 74;
const DUMP_SPELL_NAME_ENDS: usize = "a - ".len() + DUMP_SPELL_NAME_LETTERS;

/// The most letters of a spell's name the character dump shows, after its hotkey (`a - `): a
/// longer name is cut short.
pub(crate) const DUMP_SPELL_NAME_LETTERS: usize = 20;

/// The spells the character dump lists as known, `None` when it says nothing of them.
fn dump_spells(lines: &[String]) -> Option<Vec<KnownSpell>> {
    let mut lines = lines.iter().map(|line| line.trim_end());
    let said = lines
        .by_ref()
        .find(|line| *line == DUMP_KNOWN_SPELLS || *line == DUMP_NO_SPELLS)?;
    if said == DUMP_NO_SPELLS {
        return Some(Vec::new());
    }
    // A blank line, then the table's header, then a row for each spell.
    let rows = lines
        .skip_while(|line| line.is_empty())
        .skip(1)
        .take_while(|line| !line.is_empty());
    let mut spells = Vec::new();
    for row in rows {
        let column = |from: usize, to: usize| -> String {
            row.chars()
                .take(to)
                .skip(from)
                .collect::<String>()
                .trim()
                .to_owned()
        };
        let name = menu_entry(&column(0, DUMP_SPELL_NAME_ENDS))?
            .trim()
            .to_owned();
        spells.push(KnownSpell {
            name,
            failure_percent: percent(&column(DUMP_SPELL_FAILURE_AT, DUMP_SPELL_LEVEL_AT))?,
            level: column(DUMP_SPELL_LEVEL_AT, usize::MAX).parse().ok()?,
        });
    }
    Some(spells)
}

/// The skills a row of the skills screen shows, by name, in tenths of a level: each a name of
/// words and `&` after a hotkey and a training sign, then its level. A number after another
/// column, such as a cost, names no skill.
fn menu_skills(line: &str) -> Vec<(String, u32)> {
    let words: Vec<&str> = line.split_whitespace().collect();
    let mut skills = Vec::new();
    let mut name: Vec<&str> = Vec::new();
    for word in words {
        if let Some(tenths) = level_tenths(word) {
            if !name.is_empty() {
                skills.push((name.join(" "), tenths));
            }
            name.clear();
        } else if is_hotkey(word) || !word.chars().all(|c| c.is_alphabetic() || c == '&') {
            // A hotkey, a training sign or another column, such as an aptitude, starts afresh.
            name.clear();
        } else {
            name.push(word);
        }
    }
    skills
}

/// Whether `word` is a hotkey the skills screen draws before a skill: one letter.
fn is_hotkey(word: &str) -> bool {
    let mut chars = word.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic()) && chars.next().is_none()
}

/// The skill a line of the character dump's skills names, in tenths of a level: the level in
/// effect, in brackets, when it is written.
fn dump_skill(line: &str) -> Option<(String, u32)> {
    let line = line.trim_start();
    let rest = match line.split_once(' ') {
        Some((sign, rest)) if sign.len() == 1 && "+-*O".contains(sign) => rest,
        _ => line,
    };
    let (level, name) = rest.strip_prefix("Level ")?.split_once(' ')?;
    let level = match level.split_once('(') {
        Some((_, effective)) => effective.strip_suffix(')')?,
        None => level,
    };
    Some((name.trim().to_owned(), level_tenths(level)?))
}

/// A skill level as crawl writes one, `5.3` or a mastered `27`, in tenths.
fn level_tenths(word: &str) -> Option<u32> {
    let (whole, tenth) = word.split_once('.').unwrap_or((word, "0"));
    if whole.is_empty() || tenth.len() != 1 || !whole.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let whole: u32 = whole.parse().ok()?;
    let tenth: u32 = tenth.parse().ok()?;
    Some(whole * 10 + tenth)
}

/// A failure rate as crawl writes one, such as `4%`.
fn percent(word: &str) -> Option<u32> {
    word.strip_suffix('%')?.parse().ok()
}

/// The magic points a cost names, as `3 MP, Piety` names 3: none when it names none.
fn magic_cost(cost: &str) -> u32 {
    cost.split(',')
        .find_map(|part| part.trim().strip_suffix(" MP")?.trim().parse().ok())
        .unwrap_or(0)
}

/// What the mutations screen (`A`) or the character dump shows: the mutations and banes the
/// character has, as crawl describes them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShownMutations {
    /// Crawl's description of each mutation it lists at the level the character has, such as
    /// `You cover ground slowly.`, innate and acquired alike: not those it lists as gained at a
    /// future experience level. One that is transient, shown in brackets, or that their form
    /// suppresses, in parentheses (`mutation_desc`), is described without them.
    pub mutations: Vec<String>,
    /// Each bane, by its description, such as `You cover ground slowly.`: `None` when what it
    /// was read off doesn't list banes, as the character dump doesn't.
    pub banes: Option<Vec<String>>,
}

impl ScreenRead {
    /// The mutations and banes, when this is the mutations screen or the character dump.
    ///
    /// The screen lists one per row, after a hotkey or an indent, as `_get_ordered_mutations`
    /// and `MutationMenu` in crawl's `mutation.cc` list them. A mutation it will gain at a later
    /// level is `[description] XL 13`, which isn't one the character has, and a bane is
    /// `description [3.5]`, with the experience levels until it lifts.
    ///
    /// The dump lists the mutations one per line the same way, without hotkeys, under the
    /// screen's title, but never the banes (`_sdump_mutations` in `chardump.cc`, which lists
    /// `_get_mutations_descs`), so they are unread. It leaves the section out for a character
    /// with no mutations, whose overview says `A: no striking features` (`terse_mutation_list` in
    /// `mutation.cc`): none then. `None` for a dump that holds neither.
    #[must_use]
    pub fn mutations(&self) -> Option<ShownMutations> {
        let lists_banes = self.screen == Screen::Mutations;
        let entries: Vec<&str> = match self.screen {
            Screen::Mutations => self
                .lines
                .iter()
                .map(|line| menu_entry(line).unwrap_or(line))
                .collect(),
            Screen::CharacterDump => match dump_section(&self.lines, MUTATIONS_TITLE) {
                Some(section) => section.collect(),
                None if self
                    .lines
                    .iter()
                    .any(|line| line.trim() == DUMP_NO_MUTATIONS) =>
                {
                    Vec::new()
                }
                None => return None,
            },
            _ => return None,
        };
        let mut mutations = Vec::new();
        let mut banes = Vec::new();
        for entry in entries {
            let entry = entry.trim();
            if entry.is_empty() || future_level(entry) {
                continue;
            }
            if let Some(bane) = bane(entry) {
                banes.push(bane.to_owned());
                continue;
            }
            let transient = entry.strip_prefix('[').and_then(|e| e.strip_suffix(']'));
            let entry = transient.unwrap_or(entry);
            let suppressed = entry.strip_prefix('(').and_then(|e| e.strip_suffix(')'));
            mutations.push(suppressed.unwrap_or(entry).to_owned());
        }
        Some(ShownMutations {
            mutations,
            banes: lists_banes.then_some(banes),
        })
    }
}

/// What the character dump's overview says of a character without mutations.
const DUMP_NO_MUTATIONS: &str = "A: no striking features";

/// The title of the mutations screen, which the character dump also heads its mutations with.
const MUTATIONS_TITLE: &str = "Innate Abilities, Weirdness & Mutations";

/// The lines of the character dump's section headed `title`: those after the heading and the
/// blank lines under it, up to the next blank line. `None` when the dump has no such section.
fn dump_section<'a>(lines: &'a [String], title: &str) -> Option<impl Iterator<Item = &'a str>> {
    let heading = lines.iter().position(|line| line.trim() == title)?;
    Some(
        lines
            .get(heading + 1..)
            .unwrap_or_default()
            .iter()
            .map(String::as_str)
            .skip_while(|line| line.trim().is_empty())
            .take_while(|line| !line.trim().is_empty()),
    )
}

/// Whether a row of the mutations screen is a mutation gained at a future level, as
/// `[description] XL 13`.
fn future_level(entry: &str) -> bool {
    entry.starts_with('[')
        && entry
            .rsplit_once("] XL ")
            .is_some_and(|(_, xl)| !xl.is_empty() && xl.bytes().all(|b| b.is_ascii_digit()))
}

/// The description of the bane a row of the mutations screen shows, as `description [3.5]`.
fn bane(entry: &str) -> Option<&str> {
    let (description, levels) = entry.rsplit_once(" [")?;
    let (whole, tenth) = levels.strip_suffix(']')?.split_once('.')?;
    let digits = |part: &str| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit());
    (digits(whole) && digits(tenth)).then_some(description)
}

/// What the dungeon overview (`Ctrl-O`) or the character dump shows of the levels the character
/// has found: the branches, the altars and the shops (`overview_description_string` in crawl's
/// `dgn-overview.cc`). Portals and the player's annotations aren't read.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DungeonOverview {
    /// Each branch the character has found the way into, in crawl's order: not a portal branch
    /// visited and left.
    pub branches: Vec<BranchFound>,
    /// The gods whose altars the character has found, as crawl names them, such as `Trog`.
    /// `None` when the screen doesn't say: the dungeon overview lists every god the character
    /// could worship, telling those whose altars they found only by colour, which isn't read.
    /// The dump lists only those.
    pub altars: Option<Vec<String>>,
    /// The levels the character has found shops on, in crawl's order.
    pub shops: Vec<ShopsFound>,
}

/// A branch the character has found the way into, as the dungeon overview shows it, such as
/// `Lair (2/5) D:9`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchFound {
    /// Crawl's short name for it, such as `Dungeon` or `Lair`.
    pub name: String,
    /// How deep the character has explored it: 0 until they enter it.
    pub explored: u32,
    /// How many levels it has.
    pub levels: u32,
    /// The levels its entrances are on, such as `D:9`: none for the Dungeon.
    pub entrances: Vec<String>,
}

/// The shops the character has found on one level.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShopsFound {
    /// The level, as crawl names it, such as `D:2`.
    pub level: String,
    /// How many shops.
    pub count: u32,
}

/// The heading crawl starts the dungeon overview with, on the screen and in the character dump.
const DUNGEON_OVERVIEW_HEADING: &str = "Dungeon Overview and Level Annotations";

/// What the dungeon overview says under a heading of its own, after a blank line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OverviewPart {
    Branches,
    Altars,
    Shops,
    /// Anything else it says, which isn't read: the Realm of Zot's guardians, portals and
    /// annotations.
    Unread,
}

impl OverviewPart {
    /// The part `line` heads, if it is a heading.
    fn headed_by(line: &str) -> Option<Self> {
        if line.starts_with("The Realm of Zot is guarded by") {
            return Some(Self::Unread);
        }
        match line.split_once(':')?.0 {
            "Branches" => Some(Self::Branches),
            "Altars" => Some(Self::Altars),
            "Shops" => Some(Self::Shops),
            "Portals" | "Annotations" => Some(Self::Unread),
            _ => None,
        }
    }
}

impl ScreenRead {
    /// The levels the character has found, when this is the dungeon overview or the character
    /// dump, which writes it as a section of its own (`_sdump_overview` in `chardump.cc`).
    ///
    /// Each part follows a heading after a blank line, so the section ends where a blank line is
    /// followed by anything else. A found branch is drawn as `Lair (2/5) D:9`, its name, how deep
    /// it is explored of how deep it goes, and where its entrances are; a branch not yet found as
    /// `Lair: D:8-11`, which isn't read. Shops are listed by level, as `D:2 (+   D:4 *`, one
    /// sign for each shop.
    #[must_use]
    pub fn dungeon_overview(&self) -> Option<DungeonOverview> {
        let altars_listed = match self.screen {
            Screen::DungeonOverview => false,
            Screen::CharacterDump => true,
            _ => return None,
        };
        let heading = self
            .lines
            .iter()
            .position(|line| line.trim() == DUNGEON_OVERVIEW_HEADING)?;
        let mut overview = DungeonOverview {
            altars: altars_listed.then(Vec::new),
            ..DungeonOverview::default()
        };
        let mut part = OverviewPart::Unread;
        let mut after_blank = false;
        for line in self.lines.get(heading + 1..).unwrap_or_default() {
            let line = line.trim();
            if line.is_empty() {
                after_blank = true;
                continue;
            }
            if let Some(headed) = OverviewPart::headed_by(line) {
                part = headed;
            } else if after_blank {
                break;
            } else {
                match part {
                    OverviewPart::Branches => overview.branches.extend(branches_found(line)),
                    OverviewPart::Altars => {
                        if let Some(altars) = &mut overview.altars {
                            altars.push(line.to_owned());
                        }
                    }
                    OverviewPart::Shops => overview.shops.extend(shops_found(line)),
                    OverviewPart::Unread => {}
                }
            }
            after_blank = false;
        }
        Some(overview)
    }
}

/// The branches found that a line of the dungeon overview's branches shows: each a name, then
/// how deep it is explored as `(2/5)`, then the levels of its entrances, as `D:9`.
fn branches_found(line: &str) -> Vec<BranchFound> {
    let words: Vec<&str> = line.split_whitespace().collect();
    let mut found = Vec::new();
    let mut at = 0;
    while let Some(&name) = words.get(at) {
        let Some((explored, levels)) = words.get(at + 1).and_then(|word| explored(word)) else {
            at += 1;
            continue;
        };
        let entrances: Vec<String> = words
            .iter()
            .skip(at + 2)
            .take_while(|word| names_a_level(word))
            .map(|&word| word.to_owned())
            .collect();
        at += 2 + entrances.len();
        found.push(BranchFound {
            name: name.to_owned(),
            explored,
            levels,
            entrances,
        });
    }
    found
}

/// How deep a branch is explored of how deep it goes, as `(2/5)`.
fn explored(word: &str) -> Option<(u32, u32)> {
    let (explored, levels) = word.strip_prefix('(')?.strip_suffix(')')?.split_once('/')?;
    Some((explored.parse().ok()?, levels.parse().ok()?))
}

/// Whether `word` names a level as crawl writes one with its depth, such as `D:9`: not a heading
/// such as `Zot:`, which the Zot clock writes before its turns.
fn names_a_level(word: &str) -> bool {
    word.split_once(':')
        .is_some_and(|(branch, depth)| !branch.is_empty() && depth.parse::<u32>().is_ok())
}

/// The shops a line of the dungeon overview's shops shows: each level, then one sign for each
/// shop on it.
fn shops_found(line: &str) -> Vec<ShopsFound> {
    let words: Vec<&str> = line.split_whitespace().collect();
    words
        .chunks(2)
        .filter_map(|pair| match pair {
            [level, shops] => Some(ShopsFound {
                level: (*level).to_owned(),
                count: u32::try_from(shops.chars().count()).ok()?,
            }),
            _ => None,
        })
        .collect()
}

/// The tag crawl gives the full-view list, which the ordinary pickup menu shares
/// (`_full_describe_menu`, `directn.cc:557`).
const FULL_VIEW_TAG: &str = "pickup";

/// What the full-view list's title starts with when it lists monsters: `Visible`, then what it
/// lists, as `Monsters/Items/Features` (`_full_describe_menu`, `directn.cc:542`).
const FULL_VIEW_TITLE: &str = "Visible Monsters";

/// The subtitle the full-view list puts above its monsters; its items and features follow under
/// subtitles of their own (`_full_describe_menu`, `directn.cc:578`).
const MONSTERS_SUBTITLE: &str = "Monsters";

/// How the full-view list starts each further line of an entry it wraps (`_full_describe_menu`,
/// `directn.cc:621`).
const WRAPPED: &str = "         ";

impl ScreenRead {
    /// Crawl's description of each monster, when this is the full-view list (`Ctrl-X`,
    /// `full_describe_view`, `directn.cc:880`), as `an orc (asleep), wielding a +0 club,
    /// lightly wounded`: an entry wrapped onto further lines joined back into one, and without
    /// its hotkey, its glyph and the coordinates the `monster_item_view_coordinates` option
    /// adds. Its items and features aren't monsters'.
    #[must_use]
    pub(crate) fn full_view_monsters(&self) -> Option<Vec<String>> {
        let full_view = self.screen == Screen::Other(FULL_VIEW_TAG.to_owned())
            && self
                .title
                .as_deref()
                .is_some_and(|title| title.starts_with(FULL_VIEW_TITLE));
        if !full_view {
            return None;
        }
        let mut monsters: Vec<String> = Vec::new();
        let entries = self
            .lines
            .iter()
            .skip_while(|line| line.trim() != MONSTERS_SUBTITLE)
            .skip(1);
        for line in entries {
            if let Some(wrapped) = line.strip_prefix(WRAPPED) {
                if let Some(monster) = monsters.last_mut() {
                    monster.push(' ');
                    monster.push_str(wrapped.trim());
                }
                continue;
            }
            // The next subtitle ends the monsters.
            let Some(entry) = menu_entry(line) else { break };
            monsters.push(without_coordinates(without_glyph(entry)).to_owned());
        }
        Some(monsters)
    }
}

/// A full-view entry without the glyph in brackets crawl starts it with outside local tiles, as
/// `(o) an orc`.
fn without_glyph(entry: &str) -> &str {
    let mut chars = entry.chars();
    if chars.next() == Some('(') && chars.next().is_some() {
        chars.as_str().strip_prefix(") ").unwrap_or(entry)
    } else {
        entry
    }
}

/// A full-view entry without the coordinates relative to the player that the
/// `monster_item_view_coordinates` option adds, as `(3, -1) an orc`.
fn without_coordinates(entry: &str) -> &str {
    let Some((coordinates, rest)) = entry
        .strip_prefix('(')
        .and_then(|entry| entry.split_once(") "))
    else {
        return entry;
    };
    let number = |text: &str| text.trim().parse::<i32>().is_ok();
    match coordinates.split_once(", ") {
        Some((x, y)) if number(x) && number(y) => rest,
        _ => entry,
    }
}

impl ScreenRead {
    /// The resistances, when this is the character overview or the character dump, which draws
    /// the same lines (`dump_overview_screen` in crawl's `output.cc`).
    #[must_use]
    pub fn resistances(&self) -> Option<Resistances> {
        if !matches!(
            self.screen,
            Screen::CharacterOverview | Screen::CharacterDump
        ) {
            return None;
        }
        let mut resistances = Resistances::default();
        for line in &self.lines {
            let Some(name) = line.get(..RESIST_NAME_WIDTH).map(str::trim_end) else {
                continue;
            };
            let field = match name {
                "rFire" => &mut resistances.fire,
                "rCold" => &mut resistances.cold,
                "rNeg" => &mut resistances.negative,
                "rPois" => &mut resistances.poison,
                "rElec" => &mut resistances.electricity,
                "rCorr" => &mut resistances.corrosion,
                "SInv" => &mut resistances.see_invisible,
                "Will" => &mut resistances.willpower,
                _ => continue,
            };
            // The overview comes first in a dump; anything later named the same isn't it.
            if field.is_some() {
                continue;
            }
            *field = line.get(RESIST_NAME_WIDTH..).and_then(resistance);
        }
        Some(resistances)
    }
}

impl ScreenRead {
    /// The inventory letters of the items the character has equipped and not melded, when this
    /// is the character overview or the character dump.
    ///
    /// The overview draws them beside the resistances, one per line as `a - +0 mace`
    /// (`_print_overview_screen_equip` in crawl's `output.cc`), and its block ends at the first
    /// empty line; in a dump, the inventory listed later is not this.
    #[must_use]
    pub fn equipped(&self) -> Option<Vec<char>> {
        if !matches!(
            self.screen,
            Screen::CharacterOverview | Screen::CharacterDump
        ) {
            return None;
        }
        let overview = self
            .lines
            .iter()
            .skip_while(|line| !line.starts_with("rFire"))
            .take_while(|line| !line.trim().is_empty());
        Some(overview.filter_map(|line| equipped_letter(line)).collect())
    }
}

/// The inventory letter of the equipped item a line of the overview names, as `a - +0 mace`,
/// unless it is melded.
fn equipped_letter(line: &str) -> Option<char> {
    let bytes = line.as_bytes();
    (0..bytes.len()).find_map(|at| {
        let letter = char::from(*bytes.get(at)?);
        let starts_word = at == 0 || bytes.get(at - 1) == Some(&b' ');
        let rest = line.get(at + 1..)?.strip_prefix(" - ")?;
        (letter.is_ascii_alphabetic() && starts_word && !rest.starts_with("melded "))
            .then_some(letter)
    })
}

/// The resistance `desc_resist` draws at the start of `text`: pips `+`, `.` and `x`, single
/// spaced when there are few, or `∞` (`inf` in ASCII) for immunity.
fn resistance(text: &str) -> Option<Resistance> {
    if text.starts_with('∞') || text.starts_with("inf") {
        return Some(Resistance::Immune);
    }
    let mut pips = 0_i8;
    let mut any = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '+' => pips += 1,
            'x' => pips -= 1,
            '.' => {}
            // One space separates pips; more is the padding after them.
            ' ' if chars.peek().is_some_and(|next| "+.x".contains(*next)) && any => continue,
            _ => break,
        }
        any = true;
    }
    any.then_some(Resistance::Pips(pips))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(screen: Screen, lines: &[&str]) -> ScreenRead {
        ScreenRead {
            screen,
            title: None,
            lines: lines.iter().map(|&line| line.to_owned()).collect(),
            colours: BTreeMap::new(),
        }
    }

    fn menu(title: &str, lines: &[&str]) -> ScreenRead {
        ScreenRead {
            title: Some(title.to_owned()),
            ..read(Screen::Other(FULL_VIEW_TAG.to_owned()), lines)
        }
    }

    /// The ordinary pickup menu shares the full-view list's tag: only the title tells them apart.
    #[test]
    fn only_the_full_view_list_of_the_menus_tagged_pickup_lists_monsters() {
        let pickup = menu(
            "Pick up what? 1/52 slots (_ for help)",
            &["Hand Weapons", " a - a +0 club"],
        );

        assert_eq!(pickup.full_view_monsters(), None);
    }

    /// With the `monster_item_view_coordinates` option, which no Scenario sets, crawl puts
    /// where each monster stands before its name.
    #[test]
    fn the_full_view_list_reads_each_monster_without_the_coordinates_an_option_adds() {
        let list = menu(
            "Visible Monsters/Items (select to target/travel, ! to examine)",
            &[
                "Monsters",
                " a - (o) (3, 0) an orc, wielding a +0 club",
                " b - (N) (-1, -2) a naga, wielding a +0 trident and carrying a wand of",
                "         flame",
                "Items",
                " c - ()) (1, 0) a +0 club",
            ],
        );

        assert_eq!(
            list.full_view_monsters().unwrap(),
            [
                "an orc, wielding a +0 club",
                "a naga, wielding a +0 trident and carrying a wand of flame",
            ]
        );
    }

    /// Scenarios are played in wizard mode, which shows exact power, so the percentages a
    /// played game shows, and ranges with a least or a most, are read here.
    #[test]
    fn the_spell_list_shows_power_as_a_percentage_of_the_cap_outside_wizard_mode() {
        let list = read(
            Screen::Spells,
            &[
                " a - Magic Dart                      100%      1d7       4       Quiet",
                " b - Stone Arrow                     34%       3d10      4/5     Quiet",
                " c - Hailstorm                       12%       3d16      2-4/5   Loud",
                " d - Gloom                           N/A       N/A       N/A     Silent",
            ],
        );
        let reaches: Vec<_> = list
            .spell_reaches()
            .unwrap()
            .into_iter()
            .map(|reach| (reach.name, reach.power, reach.range))
            .collect();
        let percent = |p| Some(SpellPower::PercentOfCap(p));
        assert_eq!(
            reaches,
            [
                ("Magic Dart".to_owned(), percent(100), Some(4)),
                ("Stone Arrow".to_owned(), percent(34), Some(4)),
                ("Hailstorm".to_owned(), percent(12), Some(4)),
                ("Gloom".to_owned(), None, None),
            ]
        );
    }

    #[test]
    fn the_spell_lists_failure_rates_are_not_read_as_power() {
        let list = read(
            Screen::Spells,
            &[" a - Magic Dart                      Conjuration               4%       1"],
        );
        assert_eq!(list.spell_reaches(), None);
    }

    #[test]
    fn the_mutations_screen_lists_each_mutation_and_bane_but_not_those_of_a_future_level() {
        let screen = read(
            Screen::Mutations,
            &[
                "     Your serpentine skin is tough. (AC +0)",
                " a - You have supernaturally acute vision. (SInv)",
                " b - Your snake-like lower body moves slowly.",
                " c - (You can hop short distances, but move slowly.)",
                " d - [You cover ground very slowly.]",
                " e - [Your tail will grow strong enough to constrict your foes.] XL 13",
                " f - You cover ground slowly. [3.5]",
            ],
        );
        let shown = screen.mutations().unwrap();
        assert_eq!(
            shown.mutations,
            [
                "Your serpentine skin is tough. (AC +0)",
                "You have supernaturally acute vision. (SInv)",
                "Your snake-like lower body moves slowly.",
                "You can hop short distances, but move slowly.",
                "You cover ground very slowly.",
            ]
        );
        assert_eq!(
            shown.banes,
            Some(vec!["You cover ground slowly.".to_owned()])
        );
        assert_eq!(read(Screen::Abilities, &[]).mutations(), None);
    }

    /// The dump is a Screen Read like any other, but no Scenario opens it.
    #[test]
    fn the_character_dump_gives_each_skills_level_in_effect() {
        let dump = read(
            Screen::CharacterDump,
            &[
                "Skills:",
                " + Level 5.3 Fighting",
                " * Level 4.0(9.0) Maces & Flails",
                " O Level 27 Dodging",
                "   Level 1.2 Stealth",
            ],
        );
        let skills = dump.skills().unwrap();
        assert_eq!(skills.tenths("Fighting"), 53);
        assert_eq!(skills.tenths("Maces & Flails"), 90);
        assert_eq!(skills.tenths("Dodging"), 270);
        assert_eq!(skills.tenths("Stealth"), 12);
        assert_eq!(skills.tenths.len(), 4);
    }

    /// The Scenarios reach only the Dungeon, so a dungeon overview further on is read here, as
    /// `_get_seen_branches` and `_get_unseen_branches` lay it out with the Zot clock showing.
    #[test]
    fn the_dungeon_overview_reads_each_branch_found_with_its_entrances_but_not_those_unfound() {
        let overview = read(
            Screen::DungeonOverview,
            &[
                "                    Dungeon Overview and Level Annotations",
                "",
                "Branches: (press G to reach them and ?/b for more information)",
                "Dungeon (9/15)            Zot: 4800 Temple (1/1) D:4        Zot: 5000",
                "   Orc (0/2) D:9          Zot: 5000    Lair (2/5) D:10       Zot: 5000",
                "    Lair: D:8-11                          Snake: Lair:2-4",
                "",
                "Altars: (press _ to reach them and ?/g for information about gods)",
                "Ashenzari          Cheibriados            Trog",
                "",
                "Shops: (press $ to reach them - yellow denotes antique shop)",
                "D:2 (+   D:7 *",
                "",
                "Portals:",
                "D:8 Sewer (12 turns)",
            ],
        );
        let overview = overview.dungeon_overview().unwrap();
        let branches: Vec<_> = overview
            .branches
            .iter()
            .map(|branch| {
                (
                    branch.name.as_str(),
                    branch.explored,
                    branch.levels,
                    branch.entrances.join(" "),
                )
            })
            .collect();
        assert_eq!(
            branches,
            [
                ("Dungeon", 9, 15, String::new()),
                ("Temple", 1, 1, "D:4".to_owned()),
                ("Orc", 0, 2, "D:9".to_owned()),
                ("Lair", 2, 5, "D:10".to_owned()),
            ]
        );
        assert_eq!(overview.altars, None);
        let shops: Vec<_> = overview
            .shops
            .iter()
            .map(|shops| (shops.level.as_str(), shops.count))
            .collect();
        assert_eq!(shops, [("D:2", 2), ("D:7", 1)]);
    }

    /// Without mutations, the next section of the dump follows the overview after a single blank
    /// line, and isn't read as part of it.
    #[test]
    fn the_character_dump_lists_altars_found_and_its_overview_ends_where_its_next_section_starts() {
        let dump = read(
            Screen::CharacterDump,
            &[
                "Dungeon Overview and Level Annotations",
                "",
                "Branches:",
                "Dungeon (3/15)            Temple (1/1) D:4",
                "",
                "Altars:",
                "Trog",
                "The Shining One",
                "",
                "Shops:",
                "D:2 (",
                "",
                "Message History",
                "",
                "You feel a little pissed off.",
            ],
        );
        let overview = dump.dungeon_overview().unwrap();
        assert_eq!(
            overview.altars,
            Some(vec!["Trog".to_owned(), "The Shining One".to_owned()])
        );
        assert_eq!(overview.shops.len(), 1);
        assert_eq!(dump.mutations(), None);
    }

    #[test]
    fn the_character_dump_of_a_character_who_knows_no_spells_says_so() {
        let dump = read(
            Screen::CharacterDump,
            &[
                "You have 3 spell levels left.",
                "You don't know any spells.",
                "Your spell library is empty.",
            ],
        );
        assert_eq!(dump.spells(), Some(Vec::new()));
        assert_eq!(read(Screen::CharacterDump, &["Skills:"]).spells(), None);
    }

    /// Crawl leaves the mutations out of the dump of a character with none, but says so in its
    /// overview; the Scenarios' characters all have some.
    #[test]
    fn the_character_dump_of_a_character_without_mutations_says_so_in_its_overview() {
        let dump = |lines: &[&str]| read(Screen::CharacterDump, lines).mutations();
        let none = dump(&[
            "%: no passive effects",
            "@: no status effects",
            "A: no striking features",
            "a: no special abilities",
            "",
            "Message History",
        ]);
        assert_eq!(
            none,
            Some(ShownMutations {
                mutations: Vec::new(),
                banes: None,
            })
        );
        // Some, but their section left out of the dump by the player's options.
        assert_eq!(dump(&["A: magic regeneration, clarity"]), None);
    }
}

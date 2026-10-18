//! Screen reads: the screens the player opens, read as crawl wrote them, and what the core reads
//! off them.

use serde::Serialize;

/// A screen the player has open, read as plain text: crawl's own words, laid out as crawl lays
/// them out, so the core reads it the same whichever Game Source it came from.
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
}

/// Which screen the player has open.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownSpell {
    /// Its name, such as `Blink`.
    pub name: String,
    /// The chance of failing to cast it, in percent.
    pub failure_percent: u32,
    /// Its level, which is also the magic points it costs.
    pub level: u32,
}

/// An ability the character has, as the ability menu shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
}

impl ScreenRead {
    /// The spells, when this is the spell list showing failure rates rather than its other
    /// columns: one per row as `a - Blink    Translocation    4%    2`, the name, the schools,
    /// the failure rate and the level (`_spell_base_description` in crawl's `spl-cast.cc`). `None`
    /// when a row reads otherwise, as the list's other columns do.
    #[must_use]
    pub fn spells(&self) -> Option<Vec<KnownSpell>> {
        if self.screen != Screen::Spells {
            return None;
        }
        self.lines
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
            .collect()
    }

    /// The abilities, when this is the ability menu: one per row as `a - Berserk    None    0%`,
    /// the name, the cost and the failure rate (`describe_talent` in crawl's `ability.cc`).
    #[must_use]
    pub fn abilities(&self) -> Option<Vec<KnownAbility>> {
        if self.screen != Screen::Abilities {
            return None;
        }
        self.lines
            .iter()
            .filter_map(|line| menu_entry(line))
            .map(|row| {
                let columns = columns(row);
                let [name, cost, failure] = columns.as_slice() else {
                    return None;
                };
                Some(KnownAbility {
                    name: (*name).to_owned(),
                    cost: (*cost).to_owned(),
                    magic_cost: magic_cost(cost),
                    failure_percent: percent(failure)?,
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

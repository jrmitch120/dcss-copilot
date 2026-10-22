//! Screen reads: the screens the player opens, read as crawl wrote them, and what the core reads
//! off them.

use std::collections::BTreeMap;

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

/// A spell's power and range as the spell list shows them once the player switches it to its
/// other columns (`!`): `_spell_extra_description` in crawl's `spl-cast.cc`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
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

/// What the mutations screen (`A`) shows: the mutations and banes the character has, as crawl
/// describes them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShownMutations {
    /// Each mutation it lists, innate and acquired alike, as it lists them: not those it lists
    /// as gained at a future experience level.
    pub mutations: Vec<ShownMutation>,
    /// Each bane, by its description, such as `You cover ground slowly.`
    pub banes: Vec<String>,
}

/// A mutation as the mutations screen describes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShownMutation {
    /// Crawl's description of it at the level the character has, such as `You cover ground
    /// slowly.`
    pub text: String,
    /// It doesn't work in the form the character is in, shown in parentheses (`mutation_desc`).
    pub suppressed: bool,
    /// It will wear off, shown in brackets.
    pub transient: bool,
}

impl ScreenRead {
    /// The mutations and banes, when this is the mutations screen: one per row, after a hotkey
    /// or an indent, as `_get_ordered_mutations` and `MutationMenu` in crawl's `mutation.cc` list
    /// them. A mutation it will gain at a later level is `[description] XL 13`, which isn't one
    /// the character has, and a bane is `description [3.5]`, with the experience levels until it
    /// lifts.
    #[must_use]
    pub fn mutations(&self) -> Option<ShownMutations> {
        if self.screen != Screen::Mutations {
            return None;
        }
        let mut shown = ShownMutations::default();
        for line in &self.lines {
            let entry = menu_entry(line).unwrap_or(line).trim();
            if entry.is_empty() || future_level(entry) {
                continue;
            }
            if let Some(bane) = bane(entry) {
                shown.banes.push(bane.to_owned());
                continue;
            }
            let (transient, entry) = match entry.strip_prefix('[').and_then(|e| e.strip_suffix(']'))
            {
                Some(inner) => (true, inner),
                None => (false, entry),
            };
            let (suppressed, entry) =
                match entry.strip_prefix('(').and_then(|e| e.strip_suffix(')')) {
                    Some(inner) => (true, inner),
                    None => (false, entry),
                };
            shown.mutations.push(ShownMutation {
                text: entry.to_owned(),
                suppressed,
                transient,
            });
        }
        Some(shown)
    }
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
        }
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
        let mutations: Vec<_> = shown
            .mutations
            .iter()
            .map(|mutation| {
                (
                    mutation.text.as_str(),
                    mutation.suppressed,
                    mutation.transient,
                )
            })
            .collect();
        assert_eq!(
            mutations,
            [
                ("Your serpentine skin is tough. (AC +0)", false, false),
                ("You have supernaturally acute vision. (SInv)", false, false),
                ("Your snake-like lower body moves slowly.", false, false),
                ("You can hop short distances, but move slowly.", true, false),
                ("You cover ground very slowly.", false, true),
            ]
        );
        assert_eq!(shown.banes, ["You cover ground slowly."]);
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
}

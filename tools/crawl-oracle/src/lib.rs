//! The crawl oracle: diffs what crawl's own melee and spell code rolled, as its harnesses
//! (`harness/`) report it, against the engine's ported formulas on the same inputs.
//!
//! A harness prints a header line naming the crawl version and seed, then one line per case, with
//! the inputs the ported formulas take and tallies of what crawl rolled:
//!
//! - a melee case: a monster attacking the player many times, run through
//!   [`copilot_core::oracle::melee`];
//! - a beam case (`"kind":"beam"`): a monster casting one spell at the player many times, run
//!   through [`copilot_core::oracle::spell`];
//! - a choice case (`"kind":"choice"`): a monster picking a spell many times, run through
//!   [`copilot_core::oracle::spell_choice`].
//!
//! Each case runs with the formula variants the crawl version's Game Knowledge bundle names.
//! Values the formulas fix, such as a spell's power, must be equal; each tally is compared with a
//! chi-squared test. Both sides are seeded, so the verdict is the same every run.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use copilot_core::oracle::{
    self, MeleeCase, MeleeTally, SpellCase, SpellCaseError, SpellChoiceCase, SpellTally,
};
use copilot_gk::{Bundle, BundleError, Library};
use serde::Deserialize;

/// How far, in standard deviations, a tally's chi-squared statistic may sit above what chance
/// gives before the two sides are said to differ. At 5, chance alone crosses it about once in
/// 3.5 million comparisons, while an off-by-one in a to-hit roll lands far beyond it.
const Z_LIMIT: f64 = 5.0;

/// Neighbouring values are pooled until both sides together counted at least this many, so
/// every cell of the chi-squared test expects about five.
const MIN_POOLED: u32 = 10;

/// Why a dump couldn't be diffed.
#[derive(Debug, thiserror::Error)]
pub enum DiffError {
    /// The dump has no header line.
    #[error("the dump is empty")]
    Empty,
    /// A line isn't what the harness prints.
    #[error("line {line} of the dump doesn't read: {source}")]
    Line {
        /// The line, counting from 1.
        line: usize,
        /// Why it doesn't read.
        source: serde_json::Error,
    },
    /// The shipped Game Knowledge doesn't read.
    #[error(transparent)]
    Bundle(#[from] BundleError),
    /// A case counts more hits or blocks than it ran trials.
    #[error("case {0} counts more hits or blocks than trials")]
    Overcounted(u64),
    /// A spell case names a monster or spell the engine can't run.
    #[error("case {case}: {source}")]
    Spell {
        /// The case's index in the dump.
        case: u64,
        /// Why it can't run.
        source: SpellCaseError,
    },
    /// No shipped bundle is for the crawl the dump came from.
    #[error(
        "no Game Knowledge bundle is shipped for crawl {0}, so its formula variants are unknown"
    )]
    NoBundle(String),
}

/// What the harness measures for each case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Measure {
    /// The monster's rolled to-hit.
    ToHit,
    /// How often it got past the player's evasion.
    Hits,
    /// How often the player's shield blocked it.
    Blocks,
    /// The damage taken, after armour, from an attack or beam that landed.
    Damage,
    /// The power a spell was cast with.
    Power,
    /// How far a spell reaches.
    Range,
    /// A beam's accuracy.
    BeamToHit,
    /// Which spell a monster chose.
    SpellChosen,
}

impl fmt::Display for Measure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ToHit => "to-hit",
            Self::Hits => "hit rate",
            Self::Blocks => "block rate",
            Self::Damage => "damage taken",
            Self::Power => "spell power",
            Self::Range => "spell range",
            Self::BeamToHit => "beam to-hit",
            Self::SpellChosen => "spell chosen",
        })
    }
}

/// One measure on which the engine and crawl differ.
#[derive(Debug, Clone, PartialEq)]
pub struct Mismatch {
    /// The case's index in the dump.
    pub case: u64,
    /// The attacking monster.
    pub monster: String,
    /// What differs.
    pub measure: Measure,
    /// How many standard deviations past chance the difference is.
    pub z: f64,
    /// The measure's mean in crawl.
    pub crawl_mean: f64,
    /// The measure's mean in the engine.
    pub engine_mean: f64,
}

/// The `spell_type` value of every spell the engine ports when a monster casts it, as the spell
/// harness reads them, for the shipped bundle of crawl `version`.
///
/// # Errors
/// If the shipped Game Knowledge doesn't read or has no bundle for `version`.
pub fn ported_spells(version: &str) -> Result<Vec<u16>, DiffError> {
    let library = Library::shipped()?;
    let bundle = library
        .for_version(version)
        .ok_or_else(|| DiffError::NoBundle(version.to_owned()))?;
    Ok(oracle::ported_spells(bundle))
}

/// What diffing a dump found.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    /// The crawl version the dump came from.
    pub crawl: String,
    /// Cases compared.
    pub cases: usize,
    /// Every measure that differs, by case.
    pub mismatches: Vec<Mismatch>,
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "crawl {}: {} cases, {} mismatches",
            self.crawl,
            self.cases,
            self.mismatches.len()
        )?;
        for m in &self.mismatches {
            writeln!(
                f,
                "  case {} ({}): {} differs, z = {:.1}; crawl's mean {:.3}, the engine's {:.3}",
                m.case, m.monster, m.measure, m.z, m.crawl_mean, m.engine_mean
            )?;
        }
        Ok(())
    }
}

#[derive(Deserialize)]
struct Header {
    crawl: String,
    seed: u64,
}

#[derive(Deserialize)]
struct Case {
    #[serde(rename = "case")]
    index: u64,
    monster: String,
    #[serde(rename = "hd")]
    hit_dice: i32,
    fighter: bool,
    attack: CaseAttack,
    player: CasePlayer,
    trials: u32,
    to_hit: BTreeMap<i32, u32>,
    hits: u32,
    blocks: u32,
    damage: BTreeMap<i32, u32>,
}

#[derive(Deserialize)]
struct CaseAttack {
    damage: i32,
    crush: bool,
}

#[derive(Deserialize)]
struct BeamLine {
    #[serde(rename = "case")]
    index: u64,
    monster: u16,
    name: String,
    #[serde(rename = "hd")]
    hit_dice: i32,
    spell: u16,
    spell_name: String,
    player: CasePlayer,
    power: i32,
    range: i32,
    to_hit: Option<i32>,
    trials: u32,
    hits: u32,
    blocks: u32,
    damage: BTreeMap<i32, u32>,
}

#[derive(Deserialize)]
struct ChoiceLine {
    #[serde(rename = "case")]
    index: u64,
    monster: u16,
    name: String,
    #[serde(rename = "hd")]
    hit_dice: i32,
    distance: i32,
    trials: u32,
    /// By `spell_type` value, -1 for none.
    chosen: BTreeMap<i32, u32>,
}

#[derive(Deserialize)]
struct CasePlayer {
    ac: i32,
    ev: i32,
    sh: i32,
}

/// Diffs a harness dump against the engine.
///
/// # Errors
/// If the dump doesn't read, or no shipped Game Knowledge bundle is for its crawl version.
pub fn diff(dump: &str) -> Result<Report, DiffError> {
    let mut lines = dump
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty());
    let (_, header) = lines.next().ok_or(DiffError::Empty)?;
    let header: Header =
        serde_json::from_str(header).map_err(|source| DiffError::Line { line: 1, source })?;
    let library = Library::shipped()?;
    let bundle = library
        .for_version(&header.crawl)
        .ok_or_else(|| DiffError::NoBundle(header.crawl.clone()))?;

    let mut cases = 0;
    let mut mismatches = Vec::new();
    for (index, line) in lines {
        let read = |source| DiffError::Line {
            line: index + 1,
            source,
        };
        let value: serde_json::Value = serde_json::from_str(line).map_err(read)?;
        let seed = header.seed;
        match value.get("kind").and_then(serde_json::Value::as_str) {
            Some("beam") => {
                let case: BeamLine = serde_json::from_value(value).map_err(read)?;
                mismatches.extend(compare_beam(bundle, seed, &case)?);
            }
            Some("choice") => {
                let case: ChoiceLine = serde_json::from_value(value).map_err(read)?;
                mismatches.extend(compare_choice(bundle, seed, &case)?);
            }
            _ => {
                let case: Case = serde_json::from_value(value).map_err(read)?;
                mismatches.extend(compare_melee(bundle, seed, &case)?);
            }
        }
        cases += 1;
    }
    Ok(Report {
        crawl: header.crawl,
        cases,
        mismatches,
    })
}

/// Every measure of a melee `case` on which crawl and the engine differ.
fn compare_melee(bundle: &Bundle, seed: u64, case: &Case) -> Result<Vec<Mismatch>, DiffError> {
    let input = MeleeCase {
        hit_dice: case.hit_dice,
        fighter: case.fighter,
        damage: case.attack.damage,
        crushes: case.attack.crush,
        ac: case.player.ac,
        ev: case.player.ev,
        sh: case.player.sh,
    };
    let engine: MeleeTally =
        oracle::melee(bundle.formulas, &input, (seed, case.index), case.trials);
    let measures = [
        (Measure::ToHit, case.to_hit.clone(), engine.to_hit),
        (
            Measure::Hits,
            rate(case.index, case.trials, case.hits)?,
            rate(case.index, case.trials, engine.hits)?,
        ),
        (
            Measure::Blocks,
            rate(case.index, case.trials, case.blocks)?,
            rate(case.index, case.trials, engine.blocks)?,
        ),
        (Measure::Damage, case.damage.clone(), engine.damage),
    ];
    Ok(differing(case.index, &case.monster, measures))
}

/// Every measure of a beam `case` on which crawl and the engine differ.
fn compare_beam(bundle: &Bundle, seed: u64, case: &BeamLine) -> Result<Vec<Mismatch>, DiffError> {
    let input = SpellCase {
        monster: case.monster,
        spell: case.spell,
        hit_dice: case.hit_dice,
        ac: case.player.ac,
        ev: case.player.ev,
        sh: case.player.sh,
    };
    let engine: SpellTally = oracle::spell(bundle, &input, (seed, case.index), case.trials)
        .map_err(|source| DiffError::Spell {
            case: case.index,
            source,
        })?;
    let who = format!("{} casting {}", case.name, case.spell_name);
    // What the formulas fix for the case must be equal: any difference is as far from chance as
    // it gets.
    let mut mismatches: Vec<Mismatch> = [
        (Measure::Power, case.power, engine.power),
        (Measure::Range, case.range, engine.range),
        (
            Measure::BeamToHit,
            case.to_hit.unwrap_or(AUTOMATIC_HIT),
            engine.to_hit.unwrap_or(AUTOMATIC_HIT),
        ),
    ]
    .into_iter()
    .filter(|(_, crawl, ours)| crawl != ours)
    .map(|(measure, crawl, ours)| Mismatch {
        case: case.index,
        monster: who.clone(),
        measure,
        z: f64::INFINITY,
        crawl_mean: f64::from(crawl),
        engine_mean: f64::from(ours),
    })
    .collect();
    let measures = [
        (
            Measure::Hits,
            rate(case.index, case.trials, case.hits)?,
            rate(case.index, case.trials, engine.hits)?,
        ),
        (
            Measure::Blocks,
            rate(case.index, case.trials, case.blocks)?,
            rate(case.index, case.trials, engine.blocks)?,
        ),
        (Measure::Damage, case.damage.clone(), engine.damage),
    ];
    mismatches.extend(differing(case.index, &who, measures));
    Ok(mismatches)
}

/// Every measure of a choice `case` on which crawl and the engine differ.
fn compare_choice(
    bundle: &Bundle,
    seed: u64,
    case: &ChoiceLine,
) -> Result<Vec<Mismatch>, DiffError> {
    let input = SpellChoiceCase {
        monster: case.monster,
        hit_dice: case.hit_dice,
        distance: case.distance,
    };
    let engine = oracle::spell_choice(bundle, &input, (seed, case.index), case.trials).map_err(
        |source| DiffError::Spell {
            case: case.index,
            source,
        },
    )?;
    let engine: BTreeMap<i32, u32> = engine
        .into_iter()
        .map(|(spell, count)| (spell.map_or(-1, i32::from), count))
        .collect();
    let who = format!("{} at {} squares", case.name, case.distance);
    Ok(differing(
        case.index,
        &who,
        [(Measure::SpellChosen, case.chosen.clone(), engine)],
    ))
}

/// The to-hit the harness prints for a beam that never misses: crawl's `AUTOMATIC_HIT`.
const AUTOMATIC_HIT: i32 = 1500;

/// A rate as a two-value tally: how many of the trials it happened in, and how many not.
fn rate(case: u64, trials: u32, count: u32) -> Result<BTreeMap<i32, u32>, DiffError> {
    let rest = trials
        .checked_sub(count)
        .ok_or(DiffError::Overcounted(case))?;
    Ok(BTreeMap::from([(1, count), (0, rest)]))
}

/// The `measures` whose crawl and engine tallies differ past chance.
fn differing(
    case: u64,
    who: &str,
    measures: impl IntoIterator<Item = (Measure, BTreeMap<i32, u32>, BTreeMap<i32, u32>)>,
) -> Vec<Mismatch> {
    measures
        .into_iter()
        .filter_map(|(measure, crawl, ours)| {
            let z = homogeneity_z(&crawl, &ours);
            (z > Z_LIMIT).then(|| Mismatch {
                case,
                monster: who.to_owned(),
                measure,
                z,
                crawl_mean: mean(&crawl),
                engine_mean: mean(&ours),
            })
        })
        .collect()
}

/// How many standard deviations past chance the two samples' difference is: Pearson's
/// chi-squared test that both came from one distribution, over pooled values, turned into a
/// standard normal score by the Wilson-Hilferty approximation. Infinite when only one side
/// counted anything.
fn homogeneity_z(a: &BTreeMap<i32, u32>, b: &BTreeMap<i32, u32>) -> f64 {
    let cells = pooled(a, b);
    let total_a: f64 = cells.iter().map(|&(x, _)| f64::from(x)).sum();
    let total_b: f64 = cells.iter().map(|&(_, y)| f64::from(y)).sum();
    // One side never saw what the other saw plenty of: as far from chance as it gets.
    if (total_a == 0.0) != (total_b == 0.0) && total_a + total_b >= f64::from(MIN_POOLED) {
        return f64::INFINITY;
    }
    if cells.len() < 2 || total_a == 0.0 || total_b == 0.0 {
        return 0.0;
    }
    let total = total_a + total_b;
    let chi2: f64 = cells
        .iter()
        .map(|&(x, y)| {
            let both = f64::from(x) + f64::from(y);
            let expect_a = both * total_a / total;
            let expect_b = both * total_b / total;
            (f64::from(x) - expect_a).powi(2) / expect_a
                + (f64::from(y) - expect_b).powi(2) / expect_b
        })
        .sum();
    let df = f64::from(u32::try_from(cells.len() - 1).unwrap_or(u32::MAX));
    let spread = 2.0 / (9.0 * df);
    ((chi2 / df).cbrt() - (1.0 - spread)) / spread.sqrt()
}

/// The two samples' counts side by side, value by value, with neighbouring values pooled until
/// each cell holds at least [`MIN_POOLED`]; a short last cell joins the one before it.
fn pooled(a: &BTreeMap<i32, u32>, b: &BTreeMap<i32, u32>) -> Vec<(u32, u32)> {
    let values: BTreeSet<i32> = a.keys().chain(b.keys()).copied().collect();
    let mut cells: Vec<(u32, u32)> = Vec::new();
    let mut open = (0, 0);
    for value in values {
        open.0 += a.get(&value).copied().unwrap_or(0);
        open.1 += b.get(&value).copied().unwrap_or(0);
        if open.0 + open.1 >= MIN_POOLED {
            cells.push(open);
            open = (0, 0);
        }
    }
    if open != (0, 0) {
        match cells.last_mut() {
            Some(last) => {
                last.0 += open.0;
                last.1 += open.1;
            }
            None => cells.push(open),
        }
    }
    cells
}

fn mean(counts: &BTreeMap<i32, u32>) -> f64 {
    let n: f64 = counts.values().map(|&count| f64::from(count)).sum();
    let sum: f64 = counts
        .iter()
        .map(|(&value, &count)| f64::from(value) * f64::from(count))
        .sum();
    if n == 0.0 { 0.0 } else { sum / n }
}

//! The crawl oracle: diffs what crawl's own melee code rolled, as its harness (`harness/`)
//! reports it, against the engine's ported formulas on the same inputs.
//!
//! The harness prints a header line naming the crawl version and seed, then one line per case: a
//! monster attacking the player many times, with the inputs the ported formulas take and tallies
//! of what crawl rolled. Each case is run as many times through [`copilot_core::oracle::melee`],
//! with the formula variants the crawl version's Game Knowledge bundle names, and each tally is
//! compared with a chi-squared test. Both sides are seeded, so the verdict is the same every run.

use std::collections::BTreeMap;
use std::fmt;

use copilot_core::oracle::{self, MeleeCase, MeleeTally};
use copilot_gk::{BundleError, Library};
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
    /// The damage taken, after armour, from an attack that landed.
    Damage,
}

impl fmt::Display for Measure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ToHit => "to-hit",
            Self::Hits => "hit rate",
            Self::Blocks => "block rate",
            Self::Damage => "damage taken",
        })
    }
}

/// One measure on which the engine and crawl differ.
#[derive(Debug, Clone, PartialEq)]
pub struct Mismatch {
    /// The case's index in the dump.
    pub case: usize,
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
    case: usize,
    monster: String,
    hd: i32,
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
        let case: Case = serde_json::from_str(line).map_err(|source| DiffError::Line {
            line: index + 1,
            source,
        })?;
        let input = MeleeCase {
            hit_dice: case.hd,
            fighter: case.fighter,
            damage: case.attack.damage,
            crushes: case.attack.crush,
            ac: case.player.ac,
            ev: case.player.ev,
            sh: case.player.sh,
        };
        let seed = (header.seed, u64::try_from(case.case).unwrap_or(u64::MAX));
        let engine = oracle::melee(bundle.formulas, &input, seed, case.trials);
        mismatches.extend(compare(&case, &engine));
        cases += 1;
    }
    Ok(Report {
        crawl: header.crawl,
        cases,
        mismatches,
    })
}

/// Every measure of `case` on which crawl and the `engine` differ.
fn compare(case: &Case, engine: &MeleeTally) -> Vec<Mismatch> {
    let rate = |count: u32| BTreeMap::from([(1, count), (0, case.trials - count)]);
    let engine_rate = |count: u32| BTreeMap::from([(1, count), (0, case.trials - count)]);
    [
        (Measure::ToHit, case.to_hit.clone(), engine.to_hit.clone()),
        (Measure::Hits, rate(case.hits), engine_rate(engine.hits)),
        (
            Measure::Blocks,
            rate(case.blocks),
            engine_rate(engine.blocks),
        ),
        (Measure::Damage, case.damage.clone(), engine.damage.clone()),
    ]
    .into_iter()
    .filter_map(|(measure, crawl, ours)| {
        let z = homogeneity_z(&crawl, &ours);
        (z > Z_LIMIT).then(|| Mismatch {
            case: case.case,
            monster: case.monster.clone(),
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
/// standard normal score by the Wilson-Hilferty approximation.
fn homogeneity_z(a: &BTreeMap<i32, u32>, b: &BTreeMap<i32, u32>) -> f64 {
    let cells = pooled(a, b);
    let total_a: f64 = cells.iter().map(|&(x, _)| f64::from(x)).sum();
    let total_b: f64 = cells.iter().map(|&(_, y)| f64::from(y)).sum();
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
    #[allow(clippy::cast_precision_loss)]
    let df = (cells.len() - 1) as f64;
    let spread = 2.0 / (9.0 * df);
    ((chi2 / df).cbrt() - (1.0 - spread)) / spread.sqrt()
}

/// The two samples' counts side by side, value by value, with neighbouring values pooled until
/// each cell holds at least [`MIN_POOLED`]; a short last cell joins the one before it.
fn pooled(a: &BTreeMap<i32, u32>, b: &BTreeMap<i32, u32>) -> Vec<(u32, u32)> {
    let values: std::collections::BTreeSet<i32> = a.keys().chain(b.keys()).copied().collect();
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

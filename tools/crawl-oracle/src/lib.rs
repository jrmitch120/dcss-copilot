//! The crawl oracle: diffs what crawl's own melee, spell, missile and wand code rolled, and the
//! gear it made monsters with, as its harnesses (`harness/`) report it, against the engine's
//! ported formulas and Game Knowledge on the same inputs.
//!
//! A harness prints a header line naming the crawl version, the seed and the shard of the cases it
//! drew, then one line per case, with the inputs the ported formulas take and tallies of what
//! crawl rolled:
//!
//! - a melee case: a monster attacking the player many times, run through
//!   [`copilot_core::oracle::melee`];
//! - a beam case (`"kind":"beam"`): a monster casting one spell at the player many times, run
//!   through [`copilot_core::oracle::spell`];
//! - a choice case (`"kind":"choice"`): a monster picking a spell many times, run through
//!   [`copilot_core::oracle::spell_choice`];
//! - a gear case (`"kind":"gear"`): a kind of monster made many times, against
//!   [`copilot_core::oracle::gear`];
//! - a missile case (`"kind":"missile"`): a monster shooting or throwing at the player many
//!   times, run through [`copilot_core::oracle::missile`];
//! - a fire case (`"kind":"fire"`): a monster deciding many times whether to shoot or throw,
//!   run through [`copilot_core::oracle::fire`];
//! - a wand case (`"kind":"wand"`): a monster zapping a wand at the player many times, run
//!   through [`copilot_core::oracle::wand`];
//! - a zap case (`"kind":"zap"`): a monster deciding many times whether to zap its wand, run
//!   through [`copilot_core::oracle::zap`];
//! - a block case (`"kind":"block"`): how many attacks a shield blocks before the player's next
//!   action, against [`copilot_core::oracle::shield_block_limit`];
//! - a reach case (`"kind":"reach"`): how far a kind of monster reaches in melee and with which
//!   attacks, against [`copilot_core::oracle::melee_reach`];
//! - a turn case (`"kind":"turn"`): a monster playing out the player's next standard-speed
//!   action many times, run through [`copilot_core::oracle::turn`].
//!
//! Each case runs with the formula variants the crawl version's Game Knowledge bundle names.
//! Values the formulas fix, such as a spell's power or a monster's reach, must be equal; each
//! tally is compared with a chi-squared test. Both sides are seeded, so the verdict is the same
//! every run.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use copilot_core::oracle::{
    self, AUTOMATIC_HIT, AttackTally, CaseError, FireCase, MeleeCase, MissileCase, SpellCase,
    SpellChoiceCase, SpellTally, TurnCase, WandCase, ZapCase,
};
use copilot_gk::{Bundle, BundleError, EnumId, Library};
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
    /// A case names a monster, spell or item the engine can't run.
    #[error("case {case}: {source}")]
    Case {
        /// The case's index in the dump.
        case: u64,
        /// Why it can't run.
        source: CaseError,
    },
    /// A case names an item the Game Knowledge doesn't have.
    #[error("case {case}: the Game Knowledge has no item {item}")]
    NoItem {
        /// The case's index in the dump.
        case: u64,
        /// The item, as `<object_class_type>:<sub_type>`.
        item: String,
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
    /// Which launcher or throwing missile a monster was made with.
    MissileMade,
    /// Which wand a monster was made with.
    WandMade,
    /// The energy a monster's action took, 0 when it didn't shoot or zap.
    Energy,
    /// How many attacks the player's shield blocks before their next action.
    BlockLimit,
    /// How far a monster reaches in melee.
    Reach,
    /// Which attacks a monster makes from a distance.
    AttacksFrom,
    /// The melee actions a monster took against the player before their next action.
    MeleeActions,
    /// The spells a monster cast before the player's next action.
    Casts,
    /// The squares a monster closed on the player before their next action.
    Closed,
    /// Whether the Threat Level counts a monster that crawl shows fighting the player, or that
    /// strikes them in its trial.
    CanAttack,
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
            Self::MissileMade => "missile weapon made with",
            Self::WandMade => "wand made with",
            Self::Energy => "energy spent",
            Self::BlockLimit => "shield block limit",
            Self::Reach => "melee reach",
            Self::AttacksFrom => "attacks made from a distance",
            Self::MeleeActions => "melee actions",
            Self::Casts => "spells cast",
            Self::Closed => "squares closed",
            Self::CanAttack => "whether it can attack",
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
    /// The shard of the run's cases the dump holds.
    pub shard: Shard,
    /// Cases compared.
    pub cases: usize,
    /// Every measure that differs, by case.
    pub mismatches: Vec<Mismatch>,
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "crawl {}, shard {}: {} cases, {} mismatches",
            self.crawl,
            self.shard,
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

/// Which of a run's cases a harness drew: those whose index leaves `index` when divided by
/// `count`, so `count` harnesses, one per shard, draw them all between them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct Shard {
    /// The shard, from 0.
    pub index: u32,
    /// How many shards the run's cases are split into.
    pub count: u32,
}

impl fmt::Display for Shard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.index, self.count)
    }
}

#[derive(Deserialize)]
struct Header {
    crawl: String,
    seed: u64,
    shard: Shard,
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
struct GearLine {
    #[serde(rename = "case")]
    index: u64,
    monster: u16,
    name: String,
    trials: u32,
    /// By item, as `<object_class_type>:<sub_type>`, `none` or `both`.
    missile: BTreeMap<String, u32>,
    wand: BTreeMap<String, u32>,
}

#[derive(Deserialize)]
struct MissileLine {
    #[serde(rename = "case")]
    index: u64,
    monster: u16,
    name: String,
    #[serde(rename = "hd")]
    hit_dice: i32,
    weapon: String,
    weapon_name: String,
    player: CasePlayer,
    trials: u32,
    to_hit: BTreeMap<i32, u32>,
    hits: u32,
    blocks: u32,
    damage: BTreeMap<i32, u32>,
}

#[derive(Deserialize)]
struct FireLine {
    #[serde(rename = "case")]
    index: u64,
    monster: u16,
    name: String,
    weapon: String,
    weapon_name: String,
    distance: i32,
    trials: u32,
    energy: BTreeMap<i32, u32>,
}

#[derive(Deserialize)]
struct ZapLine {
    #[serde(rename = "case")]
    index: u64,
    name: String,
    #[serde(rename = "hd")]
    hit_dice: i32,
    spell: u16,
    spell_name: String,
    distance: i32,
    trials: u32,
    energy: BTreeMap<i32, u32>,
}

#[derive(Deserialize)]
struct BlockLine {
    #[serde(rename = "case")]
    index: u64,
    /// The shield, as `<object_class_type>:<sub_type>`, or `none`.
    shield: String,
    limit: u32,
}

#[derive(Deserialize)]
struct ReachLine {
    #[serde(rename = "case")]
    index: u64,
    monster: u16,
    name: String,
    reach: i32,
    /// From each distance up to its reach, the slots of the attacks it makes.
    attacks_from: BTreeMap<i32, Vec<usize>>,
}

#[derive(Deserialize)]
struct TurnLine {
    #[serde(rename = "case")]
    index: u64,
    monster: u16,
    name: String,
    #[serde(rename = "hd")]
    hit_dice: u32,
    speed: u32,
    distance: i32,
    trials: u32,
    melee: BTreeMap<i32, u32>,
    casts: BTreeMap<i32, u32>,
    closed: BTreeMap<i32, u32>,
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
            Some("gear") => {
                let case: GearLine = serde_json::from_value(value).map_err(read)?;
                mismatches.extend(compare_gear(bundle, seed, &case)?);
            }
            Some("missile") => {
                let case: MissileLine = serde_json::from_value(value).map_err(read)?;
                mismatches.extend(compare_missile(bundle, seed, &case)?);
            }
            Some("fire") => {
                let case: FireLine = serde_json::from_value(value).map_err(read)?;
                mismatches.extend(compare_fire(bundle, seed, &case)?);
            }
            Some("wand") => {
                let case: BeamLine = serde_json::from_value(value).map_err(read)?;
                mismatches.extend(compare_wand(bundle, seed, &case)?);
            }
            Some("zap") => {
                let case: ZapLine = serde_json::from_value(value).map_err(read)?;
                mismatches.extend(compare_zap(bundle, seed, &case)?);
            }
            Some("block") => {
                let case: BlockLine = serde_json::from_value(value).map_err(read)?;
                mismatches.extend(compare_block(bundle, &case)?);
            }
            Some("reach") => {
                let case: ReachLine = serde_json::from_value(value).map_err(read)?;
                mismatches.extend(compare_reach(bundle, &case)?);
            }
            Some("turn") => {
                let case: TurnLine = serde_json::from_value(value).map_err(read)?;
                mismatches.extend(compare_turn(bundle, seed, &case)?);
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
        shard: header.shard,
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
    let engine: AttackTally =
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
        .map_err(|source| DiffError::Case {
            case: case.index,
            source,
        })?;
    let who = format!("{} casting {}", case.name, case.spell_name);
    beam_mismatches(case, &engine, &who)
}

/// Every measure of a wand `case` on which crawl and the engine differ.
fn compare_wand(bundle: &Bundle, seed: u64, case: &BeamLine) -> Result<Vec<Mismatch>, DiffError> {
    let input = WandCase {
        spell: case.spell,
        hit_dice: case.hit_dice,
        ac: case.player.ac,
        ev: case.player.ev,
        sh: case.player.sh,
    };
    let engine =
        oracle::wand(bundle, &input, (seed, case.index), case.trials).map_err(|source| {
            DiffError::Case {
                case: case.index,
                source,
            }
        })?;
    let who = format!("{} zapping a wand of {}", case.name, case.spell_name);
    beam_mismatches(case, &engine, &who)
}

/// Every measure on which crawl's beams in `case` and the engine's `engine` differ.
fn beam_mismatches(
    case: &BeamLine,
    engine: &SpellTally,
    who: &str,
) -> Result<Vec<Mismatch>, DiffError> {
    // What the formulas fix for the case must be equal.
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
    .filter_map(|(measure, crawl, ours)| unequal(case.index, who, measure, crawl, ours))
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
        (Measure::Damage, case.damage.clone(), engine.damage.clone()),
    ];
    mismatches.extend(differing(case.index, who, measures));
    Ok(mismatches)
}

/// Every measure of a gear `case` on which crawl and the engine differ.
fn compare_gear(bundle: &Bundle, seed: u64, case: &GearLine) -> Result<Vec<Mismatch>, DiffError> {
    let engine =
        oracle::gear(bundle, case.monster, (seed, case.index), case.trials).map_err(|source| {
            DiffError::Case {
                case: case.index,
                source,
            }
        })?;
    // Each item numbered as crawl names it, `none` as -1 and `both` as -2.
    let crawl = |tally: &BTreeMap<String, u32>| -> BTreeMap<i32, u32> {
        tally
            .iter()
            .map(|(item, &count)| (item_code(item), count))
            .collect()
    };
    let ours = |tally: BTreeMap<Option<EnumId>, u32>| -> BTreeMap<i32, u32> {
        tally
            .into_iter()
            .map(|(item, count)| {
                let code = item
                    .and_then(|id| item_key(bundle, &id))
                    .map_or(-1, |key| item_code(&key));
                (code, count)
            })
            .collect()
    };
    Ok(differing(
        case.index,
        &case.name,
        [
            (
                Measure::MissileMade,
                crawl(&case.missile),
                ours(engine.missile),
            ),
            (Measure::WandMade, crawl(&case.wand), ours(engine.wand)),
        ],
    ))
}

/// Every measure of a missile `case` on which crawl and the engine differ.
fn compare_missile(
    bundle: &Bundle,
    seed: u64,
    case: &MissileLine,
) -> Result<Vec<Mismatch>, DiffError> {
    let input = MissileCase {
        monster: case.monster,
        weapon: item_id(bundle, case.index, &case.weapon)?,
        hit_dice: case.hit_dice,
        ac: case.player.ac,
        ev: case.player.ev,
        sh: case.player.sh,
    };
    let engine: AttackTally = oracle::missile(bundle, &input, (seed, case.index), case.trials)
        .map_err(|source| DiffError::Case {
            case: case.index,
            source,
        })?;
    let who = format!("{} firing a {}", case.name, case.weapon_name);
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
    Ok(differing(case.index, &who, measures))
}

/// Every measure of a fire `case` on which crawl and the engine differ.
fn compare_fire(bundle: &Bundle, seed: u64, case: &FireLine) -> Result<Vec<Mismatch>, DiffError> {
    let input = FireCase {
        monster: case.monster,
        weapon: item_id(bundle, case.index, &case.weapon)?,
        distance: case.distance,
    };
    let engine =
        oracle::fire(bundle, &input, (seed, case.index), case.trials).map_err(|source| {
            DiffError::Case {
                case: case.index,
                source,
            }
        })?;
    let who = format!(
        "{} with a {} at {} squares",
        case.name, case.weapon_name, case.distance
    );
    Ok(differing(
        case.index,
        &who,
        [(Measure::Energy, case.energy.clone(), engine)],
    ))
}

/// Every measure of a zap `case` on which crawl and the engine differ.
fn compare_zap(bundle: &Bundle, seed: u64, case: &ZapLine) -> Result<Vec<Mismatch>, DiffError> {
    let input = ZapCase {
        spell: case.spell,
        hit_dice: case.hit_dice,
        distance: case.distance,
    };
    let engine =
        oracle::zap(bundle, &input, (seed, case.index), case.trials).map_err(|source| {
            DiffError::Case {
                case: case.index,
                source,
            }
        })?;
    let who = format!(
        "{} with a wand of {} at {} squares",
        case.name, case.spell_name, case.distance
    );
    Ok(differing(
        case.index,
        &who,
        [(Measure::Energy, case.energy.clone(), engine)],
    ))
}

/// A block `case` on which crawl and the engine differ.
fn compare_block(bundle: &Bundle, case: &BlockLine) -> Result<Option<Mismatch>, DiffError> {
    let shield = match case.shield.as_str() {
        "none" => None,
        key => Some(item_id(bundle, case.index, key)?),
    };
    let ours =
        oracle::shield_block_limit(bundle, shield.as_ref()).map_err(|source| DiffError::Case {
            case: case.index,
            source,
        })?;
    let who = shield.map_or_else(|| "no shield".to_owned(), |id| id.0);
    Ok(unequal(
        case.index,
        &who,
        Measure::BlockLimit,
        case.limit,
        ours,
    ))
}

/// Every measure of a reach `case` on which crawl and the engine differ.
fn compare_reach(bundle: &Bundle, case: &ReachLine) -> Result<Vec<Mismatch>, DiffError> {
    let ours = oracle::melee_reach(bundle, case.monster).map_err(|source| DiffError::Case {
        case: case.index,
        source,
    })?;
    let mut mismatches: Vec<Mismatch> = unequal(
        case.index,
        &case.name,
        Measure::Reach,
        case.reach,
        ours.reach,
    )
    .into_iter()
    .collect();
    // Counted as the number of distances whose attacks differ.
    let differing = case
        .attacks_from
        .keys()
        .chain(ours.attacks_from.keys())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|distance| case.attacks_from.get(distance) != ours.attacks_from.get(distance))
        .count();
    mismatches.extend(unequal(
        case.index,
        &case.name,
        Measure::AttacksFrom,
        0,
        u32::try_from(differing).unwrap_or(u32::MAX),
    ));
    Ok(mismatches)
}

/// Every measure of a turn `case` on which crawl and the engine differ.
fn compare_turn(bundle: &Bundle, seed: u64, case: &TurnLine) -> Result<Vec<Mismatch>, DiffError> {
    let input = TurnCase {
        monster: case.monster,
        hit_dice: case.hit_dice,
        speed: case.speed,
        distance: case.distance,
    };
    let engine =
        oracle::turn(bundle, &input, (seed, case.index), case.trials).map_err(|source| {
            DiffError::Case {
                case: case.index,
                source,
            }
        })?;
    let who = format!(
        "{} at speed {}, {} squares away",
        case.name, case.speed, case.distance
    );
    // The Threat Level leaves out a monster it says can't attack: crawl mustn't show it
    // fighting the player, nor the trial striking them. A spell crawl casts may not reach them,
    // such as a cloud landing beside them, and the harness can't tell whether one did.
    let fights = case.melee.iter().any(|(&n, &count)| n > 0 && count > 0);
    let attacks = fights || engine.struck;
    let mut mismatches: Vec<Mismatch> = unequal(
        case.index,
        &who,
        Measure::CanAttack,
        u32::from(attacks),
        u32::from(attacks && engine.can_attack),
    )
    .into_iter()
    .collect();
    mismatches.extend(differing(
        case.index,
        &who,
        [
            (Measure::MeleeActions, case.melee.clone(), engine.melee),
            (Measure::Casts, case.casts.clone(), engine.casts),
            (Measure::Closed, case.closed.clone(), engine.closed),
        ],
    ));
    Ok(mismatches)
}

/// A mismatch when a value the formulas fix differs: as far from chance as it gets.
fn unequal<T: PartialEq + Into<f64>>(
    case: u64,
    who: &str,
    measure: Measure,
    crawl: T,
    ours: T,
) -> Option<Mismatch> {
    (crawl != ours).then(|| Mismatch {
        case,
        monster: who.to_owned(),
        measure,
        z: f64::INFINITY,
        crawl_mean: crawl.into(),
        engine_mean: ours.into(),
    })
}

/// The item `id` as a harness names it, `<object_class_type>:<sub_type>`, if the bundle numbers
/// it.
fn item_key(bundle: &Bundle, id: &EnumId) -> Option<String> {
    bundle.item_classes.iter().find_map(|(class, known)| {
        known
            .sub_types
            .iter()
            .find(|(_, sub_type)| sub_type.id == *id)
            .map(|(sub_type, _)| format!("{class}:{sub_type}"))
    })
}

/// The sub-type identifier of the item a harness names `key`.
fn item_id(bundle: &Bundle, case: u64, key: &str) -> Result<EnumId, DiffError> {
    let no_item = || DiffError::NoItem {
        case,
        item: key.to_owned(),
    };
    let (class, sub_type) = key.split_once(':').ok_or_else(no_item)?;
    let (Ok(class), Ok(sub_type)) = (class.parse(), sub_type.parse()) else {
        return Err(no_item());
    };
    bundle
        .item(class, Some(sub_type))
        .and_then(|item| item.sub_type)
        .ok_or_else(no_item)
}

/// An item as a harness names it, numbered for a tally: `class * 1000 + sub_type`, -1 for
/// `none`, and -2 for anything else, such as `both`.
fn item_code(key: &str) -> i32 {
    if key == "none" {
        return -1;
    }
    key.split_once(':')
        .and_then(|(class, sub_type)| {
            Some(class.parse::<i32>().ok()? * 1000 + sub_type.parse::<i32>().ok()?)
        })
        .unwrap_or(-2)
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
        |source| DiffError::Case {
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

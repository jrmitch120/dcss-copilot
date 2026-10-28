//! The branches of the dungeon, from `branches` in `branch-data.h`, each with what the game names
//! the place a level of it is and the shallowest of its levels crawl makes each monster on at
//! random, from `population` in `mon-pick-data.h`.

use std::collections::BTreeMap;

use copilot_gk::{Branch, EnumId, Landmark};

use crate::ExtractError;
use crate::cpp::{Source, Token};

/// The enumerator that ends `branch_type`'s branches.
const BRANCHES_END: &str = "NUM_BRANCHES";

/// Where a branch's fields are in its entry in `branches` (`struct Branch` in `branch.h`): its
/// identifier, its parent, the depths it may be entered from, how many levels it has, its
/// absolute depth, its flags, its three features, then its short name.
const LEVELS_FIELD: usize = 4;
const FLAGS_FIELD: usize = 6;
const ENTRY_FIELD: usize = 7;
const SHORT_NAME_FIELD: usize = 10;

/// How webtiles names the place a level of a branch one level deep is (`tileweb.cc`), as crawl's
/// code does it rather than its data: its short name, with a definite article for some branches,
/// bare for others, and otherwise with an indefinite one (`article_a`) if the dungeon doesn't lead
/// to it by stairs (`is_connected_branch`).
pub struct PlaceNaming {
    /// The branches given a definite article, by `branch_type` identifier, with it, such as
    /// `The `.
    pub definite: &'static [(&'static str, &'static str)],
    /// The branches named bare, by `branch_type` identifier.
    pub bare: &'static [&'static str],
    /// The flag of a branch the dungeon doesn't lead to by stairs.
    pub unconnected_flag: &'static str,
    /// The first letters `article_a` writes `an` before rather than `a`.
    pub vowels: &'static str,
}

/// Every branch, in `branch_type` order, from `branch_types` (`branch-type.h`), `branch_data`
/// (`branch-data.h`) and `populations` (`mon-pick-data.h`), each place named by `naming`. Each
/// table must hold one entry per branch, in the enum's order, as crawl indexes both by
/// `branch_type`; every monster a population names must be in `monster_type`, and every branch
/// `naming` names in `branch_type`.
pub fn branches(
    branch_types: &Source,
    branch_data: &Source,
    populations: &Source,
    monster_type: &[(String, i64)],
    naming: &PlaceNaming,
) -> Result<Vec<Branch>, ExtractError> {
    let mut ids = branch_types.enumeration("branch_type")?;
    let end = ids
        .iter()
        .position(|(id, _)| id == BRANCHES_END)
        .ok_or_else(|| missing("branch-type.h", BRANCHES_END))?;
    ids.truncate(end);
    // An alias, such as `BRANCH_FIRST_HELL = BRANCH_DIS`, names a branch already listed.
    let mut seen = Vec::new();
    ids.retain(|(_, value)| {
        let first = !seen.contains(value);
        seen.push(*value);
        first
    });
    let named = naming.definite.iter().map(|(id, _)| id).chain(naming.bare);
    for id in named {
        if !ids.iter().any(|(known, _)| known == id) {
            return Err(missing("branch-type.h", id));
        }
    }
    let data = branch_data.entries("branches")?;
    let population = populations.entries("population")?;
    if data.len() != ids.len() {
        return Err(unreadable(
            "branch-data.h",
            &format!(
                "branches: {} entries for {} branches",
                data.len(),
                ids.len()
            ),
        ));
    }
    if population.len() != ids.len() {
        return Err(unreadable(
            "mon-pick-data.h",
            &format!(
                "population: {} entries for {} branches",
                population.len(),
                ids.len()
            ),
        ));
    }
    ids.into_iter()
        .zip(data)
        .zip(population)
        .map(|(((id, _), fields), population)| {
            match fields.first().map(Vec::as_slice) {
                Some([Token::Ident(entry)]) if *entry == id => {}
                _ => {
                    return Err(unreadable(
                        "branch-data.h",
                        &format!("the entry of branches for {id}, out of branch_type's order"),
                    ));
                }
            }
            let (Some([Token::Str(name)]), Some(levels), Some(flags)) = (
                fields.get(SHORT_NAME_FIELD).map(Vec::as_slice),
                fields.get(LEVELS_FIELD).and_then(|field| integer(field)),
                fields.get(FLAGS_FIELD),
            ) else {
                return Err(unreadable(
                    "branch-data.h",
                    &format!("the entry of branches for {id}"),
                ));
            };
            let connected = !flags.contains(&Token::Ident(naming.unconnected_flag.to_owned()));
            Ok(Branch {
                shallowest: shallowest(&id, &population, monster_type)?,
                place: place(&id, name, levels == 1, connected, naming),
                id: EnumId(id),
            })
        })
        .collect()
}

/// The feature that leads into each branch, from its `entry_stairs` in `branch_data`
/// (`branch-data.h`), as a Landmark: a branch entrance where the dungeon leads to the branch by
/// stairs (`is_connected_branch`), and a portal where it doesn't, as its flags hold
/// `unconnected_flag`. The Dungeon, which nothing leads into, names `NUM_FEATURES`.
pub fn entrances(
    branch_data: &Source,
    unconnected_flag: &str,
) -> Result<Vec<(String, Landmark)>, ExtractError> {
    branch_data
        .entries("branches")?
        .iter()
        .map(|fields| {
            let (Some([Token::Ident(entry)]), Some(flags)) = (
                fields.get(ENTRY_FIELD).map(Vec::as_slice),
                fields.get(FLAGS_FIELD),
            ) else {
                return Err(unreadable("branch-data.h", "an entry of branches"));
            };
            let landmark = if flags.contains(&Token::Ident(unconnected_flag.to_owned())) {
                Landmark::Portal
            } else {
                Landmark::BranchEntrance
            };
            Ok((entry.clone(), landmark))
        })
        .collect()
}

/// What webtiles names the place a level of the branch `id`, short-named `name`, is: `one_level`
/// when it has one level, and `connected` when the dungeon leads to it by stairs.
fn place(id: &str, name: &str, one_level: bool, connected: bool, naming: &PlaceNaming) -> String {
    if !one_level {
        return name.to_owned();
    }
    if let Some((_, article)) = naming.definite.iter().find(|(named, _)| *named == id) {
        return format!("{article}{name}");
    }
    if connected || naming.bare.contains(&id) {
        return name.to_owned();
    }
    let article = if name.starts_with(|first| naming.vowels.contains(first)) {
        "an"
    } else {
        "a"
    };
    format!("{article} {name}")
}

/// The shallowest level of the branch `id` each monster of its `population` is made on, from 1
/// for its first: crawl's entries `{ minr, maxr, rarity, distribution, monster }` may start
/// before the branch does, as Depths' from `POP_DEPTHS` do.
fn shallowest(
    id: &str,
    population: &[Vec<Token>],
    monster_type: &[(String, i64)],
) -> Result<BTreeMap<EnumId, u32>, ExtractError> {
    let mut shallowest: BTreeMap<EnumId, u32> = BTreeMap::new();
    for entry in population {
        let fields = match entry.as_slice() {
            [Token::Punct('{'), inner @ .., Token::Punct('}')] => {
                crate::cpp::split_top_level(inner)
            }
            _ => Vec::new(),
        };
        let (Some(from), Some([Token::Ident(monster)])) = (
            fields.first().and_then(|field| integer(field)),
            fields.get(4).map(Vec::as_slice),
        ) else {
            return Err(unreadable(
                "mon-pick-data.h",
                &format!("an entry of the population of {id}"),
            ));
        };
        if !monster_type.iter().any(|(known, _)| known == monster) {
            return Err(missing("monster-type.h", monster));
        }
        let level = crate::in_range(monster, from.max(1))?;
        shallowest
            .entry(EnumId(monster.clone()))
            .and_modify(|known| *known = (*known).min(level))
            .or_insert(level);
    }
    Ok(shallowest)
}

/// The integer a field holds, such as `3` or `-3`.
fn integer(field: &[Token]) -> Option<i64> {
    match field {
        [Token::Number(digits)] => crate::cpp::number(digits),
        [Token::Punct('-'), Token::Number(digits)] => crate::cpp::number(digits).map(|n| -n),
        _ => None,
    }
}

fn missing(file: &str, what: &str) -> ExtractError {
    ExtractError::Missing {
        file: file.to_owned(),
        what: what.to_owned(),
    }
}

fn unreadable(file: &str, what: &str) -> ExtractError {
    ExtractError::Unreadable {
        file: file.to_owned(),
        what: what.to_owned(),
    }
}

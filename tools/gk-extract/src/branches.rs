//! The branches of the dungeon, from `branches` in `branch-data.h`, each with the shallowest of
//! its levels crawl makes each monster on at random, from `population` in `mon-pick-data.h`.

use std::collections::BTreeMap;

use copilot_gk::{Branch, EnumId};

use crate::ExtractError;
use crate::cpp::{Source, Token};

/// The enumerator that ends `branch_type`'s branches.
const BRANCHES_END: &str = "NUM_BRANCHES";

/// Where a branch's short name is among the fields of its entry in `branches`: after its
/// identifier, its parent, its depths, its flags and its three features (`struct Branch` in
/// `branch.h`).
const SHORT_NAME_FIELD: usize = 10;

/// Every branch, in `branch_type` order, from `branch_types` (`branch-type.h`), `branch_data`
/// (`branch-data.h`) and `populations` (`mon-pick-data.h`). Each table must hold one entry per
/// branch, in the enum's order, as crawl indexes both by `branch_type`, and every monster a
/// population names must be in `monster_type`.
pub fn branches(
    branch_types: &Source,
    branch_data: &Source,
    populations: &Source,
    monster_type: &[(String, i64)],
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
            let name = match fields.get(SHORT_NAME_FIELD).map(Vec::as_slice) {
                Some([Token::Str(name)]) => name.clone(),
                _ => {
                    return Err(unreadable(
                        "branch-data.h",
                        &format!("the short name of {id}"),
                    ));
                }
            };
            Ok(Branch {
                shallowest: shallowest(&id, &population, monster_type)?,
                id: EnumId(id),
                name,
            })
        })
        .collect()
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

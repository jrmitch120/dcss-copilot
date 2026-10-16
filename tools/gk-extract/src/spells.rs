//! Spells, the zaps they fire and monster spellbooks, from crawl's C++ tables: `spelldata` in
//! `spl-data.h`, `_spl_zaps` in `spl-zap.cc`, `zap_data` in `zap-data.h` and `mspell_list` in
//! `mon-spell.h`.

use std::collections::{BTreeMap, HashMap};

use copilot_gk::{EnumId, MonsterSpell, SpellRange, Zap, ZapDamage, ZapToHit};

use crate::ExtractError;
use crate::cpp::{Source, Token, number};

/// A spell's fields from `spelldata`, before its zap is joined to it.
pub struct SpellEntry {
    /// Its `spell_type` identifier.
    pub id: String,
    /// Its name.
    pub name: String,
    /// How far it reaches, when it has a range.
    pub range: Option<SpellRange>,
}

/// Every entry of `spelldata`, with names such as `LOS_RADIUS` read through `constants`.
///
/// An entry's fields are, in order: identifier, name, schools, flags, level, power cap, minimum
/// and maximum range, noise and tile (`struct spell_desc` in `spl-util.cc`).
pub fn spell_entries(
    source: &Source,
    file: &str,
    constants: &HashMap<String, i64>,
) -> Result<Vec<SpellEntry>, ExtractError> {
    let unreadable = |what: &str| ExtractError::Unreadable {
        file: file.to_owned(),
        what: what.to_owned(),
    };
    let mut spells = Vec::new();
    for fields in source.entries("spelldata")? {
        let [id, name, _, _, _, power_cap, min, max, ..] = fields.as_slice() else {
            return Err(unreadable("an entry of spelldata"));
        };
        let (Some(id), [Token::Str(name)]) = (identifier(id), name.as_slice()) else {
            return Err(unreadable("an entry of spelldata"));
        };
        let what = |field: &str| format!("the {field} of {id}");
        let integer = |tokens: &[Token], field: &str| {
            integer(tokens, constants).ok_or_else(|| unreadable(&what(field)))
        };
        let (min, max) = (
            integer(min, "minimum range")?,
            integer(max, "maximum range")?,
        );
        let range = (max >= 0).then_some(SpellRange {
            min: i32::try_from(min).map_err(|_| unreadable(&what("minimum range")))?,
            max: i32::try_from(max).map_err(|_| unreadable(&what("maximum range")))?,
            power_cap: i32::try_from(integer(power_cap, "power cap")?)
                .map_err(|_| unreadable(&what("power cap")))?,
        });
        spells.push(SpellEntry {
            id: id.to_owned(),
            name: name.clone(),
            range,
        });
    }
    Ok(spells)
}

/// The zap each spell fires, from `_spl_zaps`. A spell listed twice fires its first zap, as
/// `spell_to_zap` finds it.
pub fn spell_zaps(source: &Source, file: &str) -> Result<HashMap<String, String>, ExtractError> {
    let mut zaps = HashMap::new();
    for fields in source.entries("_spl_zaps")? {
        let [spell, zap] = fields.as_slice() else {
            return Err(unreadable(file, "an entry of _spl_zaps"));
        };
        let (Some(spell), Some(zap)) = (identifier(spell), identifier(zap)) else {
            return Err(unreadable(file, "an entry of _spl_zaps"));
        };
        zaps.entry(spell.to_owned())
            .or_insert_with(|| zap.to_owned());
    }
    Ok(zaps)
}

/// The helper `zap-data.h` builds a monster hex's zap with: an enchantment, accuracy as power.
const HEX_ZAP: &str = "_mon_hex_zap";

/// Every zap in `zap_data`, by its `zap_type` identifier.
///
/// A braced entry's fields are, in order: identifier, name, the player's damage and accuracy,
/// a monster's damage and accuracy, colour, whether it is an enchantment, flavour, glyph, whether
/// it pierces and whether it explodes (`struct zap_info` in `beam.cc`), then optional tiles.
pub fn zaps(
    source: &Source,
    file: &str,
    constants: &HashMap<String, i64>,
) -> Result<HashMap<String, Zap>, ExtractError> {
    let mut zaps = HashMap::new();
    for fields in source.entries("zap_data")? {
        let zap = match fields.as_slice() {
            [helper, id, flavour, ..] if identifier(helper) == Some(HEX_ZAP) => {
                let (Some(id), Some(flavour)) = (identifier(id), identifier(flavour)) else {
                    return Err(unreadable(file, "a monster hex in zap_data"));
                };
                Zap {
                    id: EnumId(id.to_owned()),
                    flavour: EnumId(flavour.to_owned()),
                    enchantment: true,
                    pierces: false,
                    explodes: false,
                    monster_damage: None,
                    monster_to_hit: Some(ZapToHit::ByPower {
                        adder: 0,
                        per_power: 1,
                        per: 1,
                    }),
                }
            }
            [
                id,
                _,
                _,
                _,
                damage,
                to_hit,
                _,
                enchantment,
                flavour,
                _,
                pierces,
                explodes,
                ..,
            ] => {
                let (Some(id), Some(flavour)) = (identifier(id), identifier(flavour)) else {
                    return Err(unreadable(file, "an entry of zap_data"));
                };
                let field = |what: &str| unreadable(file, &format!("the {what} of {id}"));
                Zap {
                    id: EnumId(id.to_owned()),
                    flavour: EnumId(flavour.to_owned()),
                    enchantment: boolean(enchantment).ok_or_else(|| field("enchantment flag"))?,
                    pierces: boolean(pierces).ok_or_else(|| field("can_beam flag"))?,
                    explodes: boolean(explodes).ok_or_else(|| field("is_explosion flag"))?,
                    monster_damage: damage_calculator(damage, constants)
                        .ok_or_else(|| field("monster damage"))?,
                    monster_to_hit: to_hit_calculator(to_hit, constants)
                        .ok_or_else(|| field("monster accuracy"))?,
                }
            }
            _ => return Err(unreadable(file, "an entry of zap_data")),
        };
        zaps.insert(zap.id.0.clone(), zap);
    }
    Ok(zaps)
}

/// Every monster spellbook in `mspell_list`, by its `mon_spellbook_type` identifier.
pub fn spellbooks(
    source: &Source,
    file: &str,
) -> Result<BTreeMap<String, Vec<MonsterSpell>>, ExtractError> {
    let mut books = BTreeMap::new();
    for fields in source.entries("mspell_list")? {
        let [book, slots] = fields.as_slice() else {
            return Err(unreadable(file, "an entry of mspell_list"));
        };
        let (Some(book), [Token::Punct('{'), slots @ .., Token::Punct('}')]) =
            (identifier(book), slots.as_slice())
        else {
            return Err(unreadable(file, "an entry of mspell_list"));
        };
        let what = format!("a spell of {book}");
        let mut slots = crate::cpp::split_top_level(slots);
        // A book may wrap its slots in a second pair of braces, as C++ allows.
        while let [only] = slots.as_slice() {
            match only.as_slice() {
                [Token::Punct('{'), inner @ .., Token::Punct('}')]
                    if inner.first() == Some(&Token::Punct('{')) =>
                {
                    slots = crate::cpp::split_top_level(inner);
                }
                _ => break,
            }
        }
        let spells = slots
            .iter()
            .map(|slot| {
                let [Token::Punct('{'), inner @ .., Token::Punct('}')] = slot.as_slice() else {
                    return Err(unreadable(file, &what));
                };
                match crate::cpp::split_top_level(inner).as_slice() {
                    [spell, frequency, flags] => {
                        let spell = identifier(spell).ok_or_else(|| unreadable(file, &what))?;
                        let frequency = match frequency.as_slice() {
                            [Token::Number(n)] => number(n).and_then(|n| u32::try_from(n).ok()),
                            _ => None,
                        }
                        .ok_or_else(|| unreadable(file, &what))?;
                        Ok(MonsterSpell {
                            spell: EnumId(spell.to_owned()),
                            frequency,
                            flags: slot_flags(flags).ok_or_else(|| unreadable(file, &what))?,
                        })
                    }
                    _ => Err(unreadable(file, &what)),
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        books.insert(book.to_owned(), spells);
    }
    Ok(books)
}

/// `MON_SPELL_A | MON_SPELL_B` as its identifiers.
fn slot_flags(tokens: &[Token]) -> Option<Vec<EnumId>> {
    tokens
        .split(|token| *token == Token::Punct('|'))
        .map(|flag| identifier(flag).map(|flag| EnumId(flag.to_owned())))
        .collect()
}

/// `new dicedef_calculator<n, a, b, c>` or `new calcdice_calculator<n, a, b, c>` as dice, or
/// `nullptr` as none.
fn damage_calculator(
    tokens: &[Token],
    constants: &HashMap<String, i64>,
) -> Option<Option<ZapDamage>> {
    if identifier(tokens) == Some("nullptr") {
        return Some(None);
    }
    let (calculator, args) = calculator(tokens, constants)?;
    let spread = match calculator {
        "dicedef_calculator" => false,
        "calcdice_calculator" => true,
        _ => return None,
    };
    let &[dice, adder, per_power, per] = args.as_slice() else {
        return None;
    };
    Some(Some(ZapDamage {
        dice,
        adder,
        per_power,
        per,
        spread,
    }))
}

/// `new tohit_calculator<a, b, c>` as accuracy, `new tohit_calculator<AUTOMATIC_HIT>` as never
/// missing, or `nullptr` as none.
fn to_hit_calculator(
    tokens: &[Token],
    constants: &HashMap<String, i64>,
) -> Option<Option<ZapToHit>> {
    if identifier(tokens) == Some("nullptr") {
        return Some(None);
    }
    let automatic = i32::try_from(*constants.get("AUTOMATIC_HIT")?).ok()?;
    let ("tohit_calculator", args) = calculator(tokens, constants)? else {
        return None;
    };
    // The template's defaults: `tohit_calculator<int adder, int mult_num = 0, int mult_denom = 1>`.
    let to_hit = match *args.as_slice() {
        [adder] if adder == automatic => ZapToHit::Automatic,
        [adder] => ZapToHit::ByPower {
            adder,
            per_power: 0,
            per: 1,
        },
        [adder, per_power] => ZapToHit::ByPower {
            adder,
            per_power,
            per: 1,
        },
        [adder, per_power, per] => ZapToHit::ByPower {
            adder,
            per_power,
            per,
        },
        _ => return None,
    };
    Some(Some(to_hit))
}

/// `new <name><args>`: the calculator's name and its integer template arguments.
fn calculator<'a>(
    tokens: &'a [Token],
    constants: &HashMap<String, i64>,
) -> Option<(&'a str, Vec<i32>)> {
    let [
        Token::Ident(new),
        Token::Ident(name),
        Token::Punct('<'),
        args @ ..,
        Token::Punct('>'),
    ] = tokens
    else {
        return None;
    };
    if new != "new" {
        return None;
    }
    let args = crate::cpp::split_top_level(args)
        .iter()
        .map(|arg| integer(arg, constants).and_then(|n| i32::try_from(n).ok()))
        .collect::<Option<Vec<_>>>()?;
    Some((name, args))
}

/// A literal integer, a negative one, or a name in `constants`.
fn integer(tokens: &[Token], constants: &HashMap<String, i64>) -> Option<i64> {
    match tokens {
        [Token::Number(n)] => number(n),
        [Token::Punct('-'), Token::Number(n)] => number(n).map(|n| -n),
        [Token::Ident(name)] => constants.get(name).copied(),
        _ => None,
    }
}

fn boolean(tokens: &[Token]) -> Option<bool> {
    match identifier(tokens)? {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// The one identifier `tokens` hold.
fn identifier(tokens: &[Token]) -> Option<&str> {
    match tokens {
        [Token::Ident(ident)] => Some(ident),
        _ => None,
    }
}

fn unreadable(file: &str, what: &str) -> ExtractError {
    ExtractError::Unreadable {
        file: file.to_owned(),
        what: what.to_owned(),
    }
}

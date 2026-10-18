//! Carried Gear: what one monster has been seen to wield, quiver or carry, read off what the game
//! says of it in its messages.
//!
//! Crawl lists a monster's gear in the same words in two places (`get_monster_equipment_desc`,
//! `directn.cc:3442`): the sighting message as monsters come into view, and the look text when the
//! look cursor rests on one. Line numbers are crawl 0.34.1's.

use copilot_gk::{Bundle, EnumId, ItemKind, MissileKind};
use serde::Serialize;

/// What one monster has been seen to wield, quiver or carry, as the game listed it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CarriedGear {
    /// What the listing named, in its order.
    pub items: Vec<CarriedItem>,
    /// What the listing leaves out even when the monster carries it.
    pub unlisted: Unlisted,
}

/// What a listing of a monster's gear leaves out even when the monster carries it, so that a
/// slot it shows empty may hold it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Unlisted {
    /// Nothing: the look text lists everything (`DESC_FULL`).
    Nothing,
    /// Unbranded large rocks, which crawl doesn't think worth listing (`item_is_worth_listing`,
    /// `items.cc:800`): a lone monster's sighting message lists every weapon, missile and wand
    /// but those (`DESC_NOTEWORTHY_AND_WEAPON`).
    PlainLargeRocks,
    /// Unbranded large rocks and an ordinary weapon in the monster's main hand, such as a +0
    /// shortbow: a group's sighting message lists only what is worth listing, and a launcher in
    /// the off hand (`DESC_NOTEWORTHY`).
    PlainWeaponsAndLargeRocks,
}

/// One item a monster was seen with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CarriedItem {
    /// How the monster has it.
    pub slot: GearSlot,
    /// Its name, as the game wrote it without its article, such as `+3 shortbow of flaming` or
    /// `arrows`.
    pub name: String,
    /// What it is, when the Game Knowledge names it: its class, such as `OBJ_WEAPONS`, and its
    /// sub-type, such as `WPN_SHORTBOW`, when the name shows it. An unidentified wand has the
    /// class alone.
    pub kind: Option<ItemKind>,
    /// Its enchantment, when its name shows one.
    pub plus: Option<i32>,
    /// Its brand, as its name shows it.
    pub brand: ItemBrand,
}

/// How a monster has an item, by the word crawl's listing gives it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GearSlot {
    /// `wielding`: in its hands.
    Wielding,
    /// `wearing`: armour, a shield or jewellery.
    Wearing,
    /// `quivering`: missiles to throw.
    Quivering,
    /// `carrying`: a launcher in its off hand, or a wand.
    Carrying,
}

/// An item's brand, as its name shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "brand", content = "name", rename_all = "camelCase")]
pub enum ItemBrand {
    /// Its name shows it has none.
    None,
    /// A weapon brand, by its `brand_type` identifier, such as `SPWPN_FLAMING`.
    Weapon(EnumId),
    /// Words its name holds that the Advisor doesn't read as a weapon brand: a missile's brand,
    /// such as `poisoned`, an armour's ego, or an artefact's name, which doesn't show its brand.
    Unnamed(String),
    /// Its name doesn't say: it isn't identified.
    Unknown,
}

/// What a message of the game says of monsters' gear.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Said {
    /// Monsters came into view (`_handle_encounter_messages`, `player-notices.cc:390`, and
    /// `_monster_headsup`, `player-notices.cc:217`).
    Sighting(Sighting),
    /// The look text of the monster under the look cursor, by name
    /// (`direction_chooser::print_target_monster_description`, `directn.cc:1792`).
    Look { name: String, gear: CarriedGear },
    /// A monster picked up, dropped, wielded, unwielded, put on or took off something, by name
    /// (`monster::pickup_message`, `monster.cc:991`, and its neighbours).
    Changed { name: String },
}

/// One sighting message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Sighting {
    /// The one monster it names, when only one came into view: its gear was listed in full, or
    /// it has none worth naming.
    pub(crate) lone: Option<LoneSighting>,
    /// When several came into view, the gear of those it lists, each by name. A monster it doesn't
    /// list may still carry what isn't worth listing.
    pub(crate) listed: Vec<(String, CarriedGear)>,
}

/// The one monster a sighting message names, and its gear.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LoneSighting {
    /// Its name, as the message gives it without its article.
    pub(crate) name: String,
    /// What it was listed with: nothing, when nothing was listed.
    pub(crate) gear: CarriedGear,
}

/// What starts the sighting message (`_handle_encounter_messages`, `player-notices.cc:442`).
const ENCOUNTER: &str = "You encounter ";

/// The prefixes crawl gives the look text: looking around, describing a target and aiming
/// (`print_target_monster_description`, `directn.cc:1798`, and `_look_around_target`,
/// `directn.cc:932`).
const LOOK_PREFIXES: [&str; 3] = ["Here: ", "Look: ", "Aim: "];

/// The words that start each part of a monster's gear as crawl lists it.
const SLOT_WORDS: [(&str, GearSlot); 4] = [
    ("wielding ", GearSlot::Wielding),
    ("wearing ", GearSlot::Wearing),
    ("quivering ", GearSlot::Quivering),
    ("carrying ", GearSlot::Carrying),
];

/// The subject a lone monster's gear sentence starts with, as its pronoun and `are`
/// conjugated (`_monster_headsup`, `player-notices.cc:244`).
const LONE_SUBJECTS: [&str; 4] = ["It is ", "He is ", "She is ", "They are "];

/// What a message says a monster did that may change what it carries, after its name
/// (`monster::pickup_message`, `monster::drop_item`, `monster::equip_weapon_message`,
/// `monster::equip_armour_message`, `monster::unequip_weapon` and `monster::unequip_armour`,
/// `monster.cc`).
const CHANGES: [&str; 6] = [
    " picks up ",
    " drops ",
    " wields ",
    " unwields ",
    " puts on ",
    " takes off ",
];

/// The articles crawl names a monster or an item with.
const ARTICLES: [&str; 3] = ["a ", "an ", "the "];

/// What a wand's name starts with once identified, and ends with before (`item_def::name_aux`,
/// `item-name.cc:1659`).
const WAND_OF: &str = "wand of ";
const WAND: &str = " wand";

/// What the game says of monsters' gear in the message `text`, with items named through `bundle`.
pub(crate) fn said(text: &str, bundle: &Bundle) -> Vec<Said> {
    let mut said = Vec::new();
    if let Some(at) = text.find(ENCOUNTER) {
        said.extend(sighting(&text[at + ENCOUNTER.len()..], bundle).map(Said::Sighting));
    }
    for line in text.lines() {
        if let Some(look) = look(line, bundle) {
            said.push(look);
        }
        for sentence in sentences(line) {
            if let Some(name) = CHANGES
                .iter()
                .find_map(|change| sentence.split_once(change).map(|(name, _)| name))
            {
                said.push(Said::Changed {
                    name: without_article(name).to_owned(),
                });
            }
        }
    }
    said
}

/// The sighting message after `You encounter `: the monsters it names, then, when one came into
/// view, a sentence listing its gear, and when several did, a sentence for each worth listing,
/// on the lines after.
fn sighting(text: &str, bundle: &Bundle) -> Option<Sighting> {
    let mut lines = text.lines();
    let mut said_first = sentences(lines.next()?);
    let named = said_first.next()?;
    let next = said_first.next();
    let lone_listing = next.and_then(|sentence| {
        LONE_SUBJECTS
            .iter()
            .find_map(|subject| sentence.strip_prefix(subject))
    });
    // `_describe_monsters_from_species` names several as `2 orcs` or `an orc and a goblin`; a
    // unique's title, as `Boris, Master of Life and Death`, may read as several, unless its gear
    // follows.
    let plainly_one = !named.contains(" and ")
        && !named.contains(", ")
        && !named.starts_with(|c: char| c.is_ascii_digit());
    if lone_listing.is_some() || plainly_one {
        let none = || CarriedGear {
            items: Vec::new(),
            unlisted: Unlisted::PlainLargeRocks,
        };
        let gear = lone_listing.map_or_else(none, |listing| {
            gear(listing, Unlisted::PlainLargeRocks, bundle).unwrap_or_else(none)
        });
        return Some(Sighting {
            lone: Some(LoneSighting {
                name: without_article(named).to_owned(),
                gear,
            }),
            listed: Vec::new(),
        });
    }
    let mut listed = Vec::new();
    for sentence in next
        .into_iter()
        .chain(said_first)
        .chain(lines.flat_map(sentences))
    {
        let Some((name, listing)) = sentence.split_once(" is ") else {
            continue;
        };
        if let Some(gear) = gear(listing, Unlisted::PlainWeaponsAndLargeRocks, bundle) {
            listed.push((without_article(name).to_owned(), gear));
        }
    }
    Some(Sighting { lone: None, listed })
}

/// The look text in `line`: `Here: an orc, wielding a +0 shortbow (asleep)`, the monster's name
/// with its article, its gear after a comma, and what else is said of it in brackets.
fn look(line: &str, bundle: &Bundle) -> Option<Said> {
    let text = LOOK_PREFIXES
        .iter()
        .find_map(|prefix| line.find(prefix).map(|at| &line[at + prefix.len()..]))?;
    // The suffixes in brackets end it (`cell_monster_description`, `directn.cc:1872`).
    let text = match text.strip_suffix(')').and_then(|text| text.rfind(" (")) {
        Some(at) if !text[at..].contains('"') => &text[..at],
        _ => text,
    };
    let (name, gear) = match text.split_once(", ") {
        Some((name, listing)) => (name, self::gear(listing, Unlisted::Nothing, bundle)?),
        None => (
            text,
            CarriedGear {
                items: Vec::new(),
                unlisted: Unlisted::Nothing,
            },
        ),
    };
    // Something without an article, such as `The floor`, isn't a monster.
    if !ARTICLES
        .iter()
        .any(|article| name.starts_with(article) || capitalised_article(name, article))
        && !name.starts_with(char::is_uppercase)
    {
        return None;
    }
    Some(Said::Look {
        name: without_article(name).to_owned(),
        gear,
    })
}

/// Whether `name` starts with `article` capitalised, as `The orc` does.
fn capitalised_article(name: &str, article: &str) -> bool {
    let mut chars = article.chars();
    chars.next().is_some_and(|first| {
        name.starts_with(first.to_ascii_uppercase()) && name[1..].starts_with(chars.as_str())
    })
}

/// A monster's gear as crawl lists it, `wielding a +0 shortbow, quivering arrows and carrying a
/// wand of flame`, with what such a listing leaves out: `None` unless it starts with the word of
/// a slot.
fn gear(listing: &str, unlisted: Unlisted, bundle: &Bundle) -> Option<CarriedGear> {
    let listing = listing.strip_suffix('.').unwrap_or(listing);
    // Where each part starts: at the start, or after `, ` or ` and `, with a slot's word.
    let mut starts: Vec<(usize, GearSlot, usize)> = Vec::new();
    for (word, slot) in SLOT_WORDS {
        for (at, _) in listing.match_indices(word) {
            let before = &listing[..at];
            if at == 0 || before.ends_with(", ") || before.ends_with(" and ") {
                starts.push((at, slot, at + word.len()));
            }
        }
    }
    starts.sort_unstable_by_key(|&(at, ..)| at);
    if starts.first().map(|&(at, ..)| at) != Some(0) {
        return None;
    }
    let mut items = Vec::new();
    for (i, &(_, slot, from)) in starts.iter().enumerate() {
        let to = starts.get(i + 1).map_or(listing.len(), |&(at, ..)| at);
        let part = listing[from..to]
            .trim_end_matches(", ")
            .trim_end_matches(" and ")
            .trim_end_matches(',');
        for name in item_names(part) {
            items.push(item(slot, name, bundle));
        }
    }
    Some(CarriedGear { items, unlisted })
}

/// The names of the items in one part of a listing, which joins two with ` and `, as `a +0
/// orcbow and a wand of flame`: each starts with its article, but missiles quivered have none.
fn item_names(part: &str) -> Vec<&str> {
    let mut names = Vec::new();
    let mut rest = part;
    'split: loop {
        for (at, _) in rest.match_indices(" and ") {
            let after = &rest[at + " and ".len()..];
            if ARTICLES.iter().any(|article| after.starts_with(article)) {
                names.push(&rest[..at]);
                rest = after;
                continue 'split;
            }
        }
        names.push(rest);
        return names;
    }
}

/// The item crawl names `name` (with its article, if any) in `slot`, read through `bundle`.
fn item(slot: GearSlot, name: &str, bundle: &Bundle) -> CarriedItem {
    let artefact = name.starts_with("the ") || name.contains('"');
    let name = without_article(name);
    // A known wand's charges (`item-name.cc:1678`), and what crawl adds of a weapon from an
    // undying armoury, follow its name.
    let base = match name.strip_suffix(')').and_then(|text| text.rfind(" (")) {
        Some(at) => &name[..at],
        None => name,
    };
    let base = base.strip_prefix("cursed ").unwrap_or(base);
    let (plus, body) = match base.split_once(' ') {
        Some((plus, body)) if plus.starts_with(['+', '-']) => (plus.parse::<i32>().ok(), body),
        _ => (None, base),
    };
    let (kind, brand) = if let Some(wand) = body.strip_prefix(WAND_OF) {
        let sub_type = bundle
            .wands
            .iter()
            .find(|(_, known)| known.name == wand)
            .map(|(id, _)| id.clone());
        (kind_of(bundle, "OBJ_WANDS", sub_type), ItemBrand::None)
    } else if body.ends_with(WAND) {
        (kind_of(bundle, "OBJ_WANDS", None), ItemBrand::None)
    } else {
        named_kind(body, plus.is_some(), artefact, bundle)
    };
    CarriedItem {
        slot,
        name: name.to_owned(),
        kind,
        plus,
        brand,
    }
}

/// The kind of item `body` names, and its brand, by the longest sub-type name of a weapon, a
/// missile or armour it holds, and the words around it: a weapon brand named before it, as
/// `heavy shortbow`, or after `of`, as `shortbow of flaming`. Missiles are quivered in the plural.
/// `identified` is whether its name showed its enchantment, without which it shows no brand.
fn named_kind(
    body: &str,
    identified: bool,
    artefact: bool,
    bundle: &Bundle,
) -> (Option<ItemKind>, ItemBrand) {
    let mut found: Option<(&EnumId, &EnumId, usize, usize)> = None;
    for class in bundle.item_classes.values() {
        if !matches!(
            class.id.0.as_str(),
            "OBJ_WEAPONS" | "OBJ_MISSILES" | "OBJ_ARMOUR"
        ) {
            continue;
        }
        for sub_type in class.sub_types.values() {
            for form in [format!("{}s", sub_type.name), sub_type.name.clone()] {
                let Some(at) = word_at(body, &form) else {
                    continue;
                };
                if found.is_none_or(|(.., len)| form.len() > len) {
                    found = Some((&class.id, &sub_type.id, at, form.len()));
                }
            }
        }
    }
    let Some((class, sub_type, at, len)) = found else {
        return (None, ItemBrand::Unknown);
    };
    let kind = Some(ItemKind {
        class: class.clone(),
        sub_type: Some(sub_type.clone()),
    });
    let before = body[..at].trim();
    let after = body[at + len..].trim();
    let after = after.strip_prefix("of ").unwrap_or(after);
    let thrown = bundle
        .missile_weapons
        .get(sub_type)
        .is_some_and(|weapon| weapon.kind == MissileKind::Thrown);
    let brand = if thrown {
        // Crawl names a missile's brand before it, as `poisoned darts`, or after, as `darts of
        // dispersal`; none are weapon brands.
        match [before, after].join(" ").trim() {
            "" => ItemBrand::None,
            words => ItemBrand::Unnamed(words.to_owned()),
        }
    } else if artefact || !identified {
        ItemBrand::Unknown
    } else {
        weapon_brand(before, after, bundle)
    };
    (kind, brand)
}

/// The weapon brand named by the words `before` and `after` a weapon's name, as
/// `weapon_brand_desc` (`item-name.cc:1415`) places them.
fn weapon_brand(before: &str, after: &str, bundle: &Bundle) -> ItemBrand {
    let named = |words: &str, placed_before: bool| {
        bundle
            .weapon_brands
            .iter()
            .find(|(_, brand)| brand.before == placed_before && brand.name == words)
            .map(|(id, _)| ItemBrand::Weapon(id.clone()))
    };
    match (before, after) {
        ("", "") => ItemBrand::None,
        (words, "") => named(words, true).unwrap_or_else(|| ItemBrand::Unnamed(words.to_owned())),
        ("", words) => named(words, false).unwrap_or_else(|| ItemBrand::Unnamed(words.to_owned())),
        (before, after) => ItemBrand::Unnamed(format!("{before} {after}")),
    }
}

/// Where `word` stands in `text` as whole words, if it does.
fn word_at(text: &str, word: &str) -> Option<usize> {
    text.match_indices(word).map(|(at, _)| at).find(|&at| {
        let starts = at == 0 || text[..at].ends_with(' ');
        let rest = &text[at + word.len()..];
        starts && (rest.is_empty() || rest.starts_with(' '))
    })
}

/// The kind of item of `class` and `sub_type`, when the bundle has the class.
fn kind_of(bundle: &Bundle, class: &str, sub_type: Option<EnumId>) -> Option<ItemKind> {
    let class = bundle
        .item_classes
        .values()
        .find(|known| known.id.0 == class)?;
    Some(ItemKind {
        class: class.id.clone(),
        sub_type,
    })
}

/// The sentences of `line`, each without its full stop: crawl ends each with `.` and joins the
/// messages of one moment with a space.
fn sentences(line: &str) -> impl Iterator<Item = &str> {
    line.split(". ")
        .map(|sentence| sentence.trim().trim_end_matches('.'))
        .filter(|sentence| !sentence.is_empty())
}

/// `name` without the article crawl gave it, whether or not it starts a sentence.
pub(crate) fn without_article(name: &str) -> &str {
    let name = name.trim();
    ARTICLES
        .iter()
        .find_map(|article| {
            name.strip_prefix(article)
                .or_else(|| capitalised_article(name, article).then(|| &name[article.len()..]))
        })
        .unwrap_or(name)
}

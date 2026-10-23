//! The player's inventory, merged from the `inv` deltas of webtiles `player` messages.

use std::collections::{BTreeMap, BTreeSet};

use copilot_core::InventoryItem;
use copilot_gk::{Bundle, ItemClass, ItemKind};
use serde_json::Value;

/// What an item is called when the game hasn't named it, which only a stream that missed the
/// item's first appearance could cause.
const UNNAMED: &str = "unknown item";

/// `ISFLAG_IDENTIFIED` (`item-status-flag-type.h`): the item is fully identified.
const IDENTIFIED: i64 = 0x1;

/// `ISFLAG_ARTEFACT_MASK` (`item-status-flag-type.h`): the item is a randart or an unrandart,
/// whose kind crawl never knows from its kind's identification, nor teaches it.
const ARTEFACT: i64 = 0x3000;

/// A kind of item as the game numbers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct KindId {
    /// Its `object_class_type` value.
    base_type: u8,
    /// Its value in its class's sub-type enum.
    sub_type: u16,
}

/// The inventory slots the game has described, by slot number, and the kinds of item the
/// player has identified in this game as far as the game has shown.
///
/// Crawl can identify a kind without resending a carried item of it: webtiles sends an item's
/// name only when some other field of it changed, and compares the names under what the player
/// knows now, so they always match. A kind identified once is remembered for the game, and
/// decodes every item of it.
#[derive(Debug, Default)]
pub struct Inventory {
    slots: BTreeMap<u16, Slot>,
    identified: BTreeSet<KindId>,
}

/// One slot's fields, merged across deltas. A field is sent only when it changed.
#[derive(Debug, Default)]
struct Slot {
    /// The item's `object_class_type` value.
    base_type: Option<i64>,
    /// Its value in its class's sub-type enum, whether or not the player knows it.
    sub_type: Option<i64>,
    quantity: Option<u32>,
    name: Option<String>,
    /// Its `iflags_t`, such as `ISFLAG_IDENTIFIED`.
    flags: Option<i64>,
    /// Its inventory letter, as a character code.
    letter: Option<u32>,
    /// Crawl's `is_useless_item` with temporary conditions, sent as 0 or 1.
    useless: Option<bool>,
}

impl Inventory {
    /// Merges a `player` message's `inv` object, keyed by slot number, learning through
    /// `knowledge` the kinds each item it sends shows identified.
    pub fn merge(&mut self, inv: &Value, knowledge: Option<&Bundle>) {
        let Some(slots) = inv.as_object() else { return };
        for (slot, delta) in slots {
            let Ok(slot) = slot.parse() else {
                continue;
            };
            let fields = self.slots.entry(slot).or_default();
            let int = |key: &str| delta.get(key).and_then(Value::as_i64);
            if let Some(base_type) = int("base_type") {
                fields.base_type = Some(base_type);
            }
            if let Some(sub_type) = int("sub_type") {
                fields.sub_type = Some(sub_type);
            }
            if let Some(quantity) = delta.get("quantity").and_then(Value::as_u64) {
                fields.quantity = u32::try_from(quantity).ok();
            }
            if let Some(name) = delta.get("name").and_then(Value::as_str) {
                fields.name = Some(name.to_owned());
            }
            if let Some(flags) = int("flags") {
                fields.flags = Some(flags);
            }
            if let Some(letter) = int("letter") {
                fields.letter = u32::try_from(letter).ok();
            }
            if let Some(useless) = int("useless") {
                fields.useless = Some(useless != 0);
            }
            if let Some((kind, class)) = knowledge.and_then(|bundle| fields.kind_id(bundle))
                && class.kinds_hidden
                && !fields.artefact()
                && fields.shows_identified(class, kind)
            {
                self.identified.insert(kind);
            }
        }
    }

    /// Learns from a game message, through `bundle`, the kind it says the player now knows of
    /// an item they carry, as crawl says when they see an identified item of that kind
    /// (`seen_item`). It is learned only when exactly one kind the player carries unidentified
    /// fits the name the message gives.
    pub fn learn_from_message(&mut self, message: &str, bundle: &Bundle) {
        let Some(name) = bundle.identification.learned_name(message) else {
            return;
        };
        let fitting: BTreeSet<KindId> = self
            .slots
            .values()
            .filter_map(|slot| {
                let (kind, class) = slot.kind_id(bundle)?;
                let unknown = class.kinds_hidden && !self.identified.contains(&kind);
                (unknown && named_sub_type(class, name) == Some(kind.sub_type)).then_some(kind)
            })
            .collect();
        if fitting.len() == 1 {
            self.identified.extend(fitting);
        }
    }

    /// The items carried, by letter, identified through `knowledge` when there is a bundle for
    /// the game's Crawl Version, as far as the player knows them. An empty slot has a quantity
    /// of zero.
    pub fn items(&self, knowledge: Option<&Bundle>) -> Vec<InventoryItem> {
        let mut items: Vec<InventoryItem> = self
            .slots
            .iter()
            .filter_map(|(&slot, fields)| {
                let quantity = fields.quantity.filter(|&quantity| quantity > 0)?;
                let name = fields.name.clone().unwrap_or_else(|| UNNAMED.to_owned());
                let kind = knowledge.and_then(|bundle| self.kind(fields, bundle));
                Some(InventoryItem {
                    letter: fields
                        .letter
                        .and_then(char::from_u32)
                        .or_else(|| letter(slot))?,
                    name,
                    quantity,
                    kind,
                    useless: fields.useless.unwrap_or(false),
                })
            })
            .collect();
        items.sort_by_key(|item| letter_order(item.letter));
        items
    }
}

impl Inventory {
    /// What the item in `slot` is, through `bundle`: its sub-type only when the player knows
    /// it (`item_type_known` in crawl's `item-name.cc`), as they know a scroll's only once it or
    /// another of its kind is identified, and an artefact's only once it is.
    fn kind(&self, slot: &Slot, bundle: &Bundle) -> Option<ItemKind> {
        let base_type = u8::try_from(slot.base_type?).ok()?;
        let sub_type = slot.sub_type.and_then(|sub| u16::try_from(sub).ok());
        let known = slot.kind_id(bundle).is_some_and(|(kind, class)| {
            !class.kinds_hidden
                || slot.shows_identified(class, kind)
                || (!slot.artefact() && self.identified.contains(&kind))
        });
        bundle.item(base_type, sub_type.filter(|_| known))
    }

    /// The inventory letter of the item in `slot`, if it holds one.
    pub fn letter_of(&self, slot: u16) -> Option<char> {
        let fields = self.slots.get(&slot)?;
        fields.quantity.filter(|&quantity| quantity > 0)?;
        fields
            .letter
            .and_then(char::from_u32)
            .or_else(|| letter(slot))
    }
}

impl Slot {
    /// The item's kind as the game numbers it, whether or not the player knows it, with its
    /// class in `bundle`.
    fn kind_id<'b>(&self, bundle: &'b Bundle) -> Option<(KindId, &'b ItemClass)> {
        let base_type = u8::try_from(self.base_type?).ok()?;
        let sub_type = u16::try_from(self.sub_type?).ok()?;
        let class = bundle.item_classes.get(&base_type)?;
        Some((
            KindId {
                base_type,
                sub_type,
            },
            class,
        ))
    }

    /// The item is an artefact, as its flags say.
    fn artefact(&self) -> bool {
        self.flags.is_some_and(|flags| flags & ARTEFACT != 0)
    }

    /// The slot itself shows the player has identified its item's `kind`, of `class`: the item
    /// is flagged identified, or its name names its sub-type.
    fn shows_identified(&self, class: &ItemClass, kind: KindId) -> bool {
        self.flags.is_some_and(|flags| flags & IDENTIFIED != 0)
            || self
                .name
                .as_deref()
                .is_some_and(|name| named_sub_type(class, name) == Some(kind.sub_type))
    }
}

/// The sub-type of `class` that `name` names, as `of <sub-type>`, such as `teleportation` in
/// `a scroll of teleportation`: the longest when several fit, so `protection from fire` rather
/// than `protection`.
fn named_sub_type(class: &ItemClass, name: &str) -> Option<u16> {
    class
        .sub_types
        .iter()
        .filter(|(_, sub)| name.contains(&format!("of {}", sub.name)))
        .max_by_key(|(_, sub)| sub.name.len())
        .map(|(&value, _)| value)
}

/// The letter crawl gives a slot it numbers 0 to 51 when the game hasn't sent the slot's own
/// letter: `a` to `z`, then `A` to `Z`.
fn letter(slot: u16) -> Option<char> {
    let slot = u8::try_from(slot).ok()?;
    match slot {
        0..26 => Some(char::from(b'a' + slot)),
        26..52 => Some(char::from(b'A' + slot - 26)),
        _ => None,
    }
}

/// Where a letter comes in the inventory: `a` to `z`, then `A` to `Z`, then anything else.
fn letter_order(letter: char) -> (u8, char) {
    match letter {
        'a'..='z' => (0, letter),
        'A'..='Z' => (1, letter),
        _ => (2, letter),
    }
}

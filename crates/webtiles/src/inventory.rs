//! The player's inventory, merged from the `inv` deltas of webtiles `player` messages.

use std::collections::BTreeMap;

use copilot_core::InventoryItem;
use copilot_gk::{Bundle, ItemKind};
use serde_json::Value;

/// What an item is called when the game hasn't named it, which only a stream that missed the
/// item's first appearance could cause.
const UNNAMED: &str = "unknown item";

/// `ISFLAG_IDENTIFIED` (`item-status-flag-type.h`): the item is fully identified.
const IDENTIFIED: i64 = 0x1;

/// The inventory slots the game has described, by slot number.
#[derive(Debug, Default)]
pub struct Inventory {
    slots: BTreeMap<u16, Slot>,
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
    /// Merges a `player` message's `inv` object, keyed by slot number.
    pub fn merge(&mut self, inv: &Value) {
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
                let kind = knowledge.and_then(|bundle| fields.kind(bundle, &name));
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
    /// What the item is, through `bundle`: its sub-type only when the player knows it, as they
    /// know a scroll's only once it is identified, by the item or by its kind, which its `name`
    /// then says (`item_type_known` in crawl's `item-name.cc`).
    fn kind(&self, bundle: &Bundle, name: &str) -> Option<ItemKind> {
        let base_type = u8::try_from(self.base_type?).ok()?;
        let sub_type = self.sub_type.and_then(|sub| u16::try_from(sub).ok());
        let class = bundle.item_classes.get(&base_type)?;
        let identified = self.flags.is_some_and(|flags| flags & IDENTIFIED != 0);
        let named = || {
            sub_type
                .and_then(|sub| class.sub_types.get(&sub))
                .is_some_and(|sub| name.contains(&format!("of {}", sub.name)))
        };
        let known = !class.kinds_hidden || identified || named();
        bundle.item(base_type, sub_type.filter(|_| known))
    }
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

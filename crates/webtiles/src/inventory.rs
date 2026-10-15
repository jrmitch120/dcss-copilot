//! The player's inventory, merged from the `inv` deltas of webtiles `player` messages.

use std::collections::BTreeMap;

use copilot_core::InventoryItem;
use copilot_gk::Bundle;
use serde_json::Value;

/// What an item is called when the game hasn't named it, which only a stream that missed the
/// item's first appearance could cause.
const UNNAMED: &str = "unknown item";

/// The inventory slots the game has described, by slot number: 0 is `a`, 26 is `A`.
#[derive(Debug, Default)]
pub struct Inventory {
    slots: BTreeMap<u8, Slot>,
}

/// One slot's fields, merged across deltas. A field is sent only when it changed.
#[derive(Debug, Default)]
struct Slot {
    /// The item's `object_class_type` value.
    base_type: Option<i64>,
    /// Its value in its class's sub-type enum.
    sub_type: Option<i64>,
    quantity: Option<u32>,
    name: Option<String>,
}

impl Inventory {
    /// Merges a `player` message's `inv` object, keyed by slot number.
    pub fn merge(&mut self, inv: &Value) {
        let Some(slots) = inv.as_object() else { return };
        for (slot, delta) in slots {
            let Some(slot) = slot.parse().ok().filter(|&slot| letter(slot).is_some()) else {
                continue;
            };
            let fields = self.slots.entry(slot).or_default();
            if let Some(base_type) = delta.get("base_type").and_then(Value::as_i64) {
                fields.base_type = Some(base_type);
            }
            if let Some(sub_type) = delta.get("sub_type").and_then(Value::as_i64) {
                fields.sub_type = Some(sub_type);
            }
            if let Some(quantity) = delta.get("quantity").and_then(Value::as_u64) {
                fields.quantity = u32::try_from(quantity).ok();
            }
            if let Some(name) = delta.get("name").and_then(Value::as_str) {
                fields.name = Some(name.to_owned());
            }
        }
    }

    /// The items carried, by letter, identified through `knowledge` when there is a bundle for
    /// the game's Crawl Version. An empty slot has a quantity of zero.
    pub fn items(&self, knowledge: Option<&Bundle>) -> Vec<InventoryItem> {
        self.slots
            .iter()
            .filter_map(|(&slot, fields)| {
                let quantity = fields.quantity.filter(|&quantity| quantity > 0)?;
                let kind = knowledge.and_then(|bundle| {
                    let base_type = u8::try_from(fields.base_type?).ok()?;
                    let sub_type = fields.sub_type.and_then(|sub| u16::try_from(sub).ok());
                    bundle.item(base_type, sub_type)
                });
                Some(InventoryItem {
                    letter: letter(slot)?,
                    name: fields.name.clone().unwrap_or_else(|| UNNAMED.to_owned()),
                    quantity,
                    kind,
                })
            })
            .collect()
    }
}

/// The inventory letter of a slot number: `a` to `z`, then `A` to `Z`.
fn letter(slot: u8) -> Option<char> {
    match slot {
        0..26 => Some(char::from(b'a' + slot)),
        26..52 => Some(char::from(b'A' + slot - 26)),
        _ => None,
    }
}

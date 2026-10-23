//! The webtiles Game Source decodes a carried item's kind once the player has identified it, by
//! the item or by any other of its kind in the same game, even when crawl resends nothing of it.

// Helpers outside #[test] functions panic on bad fixtures too.
#![allow(clippy::unwrap_used)]

use copilot_core::{InventoryItem, SourceEvent};
use copilot_gk::{Bundle, EnumId, Library};
use copilot_webtiles::{Envelope, WebtilesSource};
use serde_json::{Value, json};

/// `ISFLAG_IDENTIFIED`, as webtiles sends it in an item's `flags`.
const IDENTIFIED: i64 = 0x1;

/// A source fed one tab of a crawl 0.34.1 game.
struct Game {
    source: WebtilesSource,
    library: Library,
    inventory: Vec<InventoryItem>,
}

impl Game {
    fn new() -> Self {
        let library = Library::shipped().unwrap();
        Self {
            source: WebtilesSource::new(library.clone()),
            library,
            inventory: Vec::new(),
        }
    }

    fn bundle(&self) -> &Bundle {
        self.library.for_version("0.34.1").unwrap()
    }

    /// The `object_class_type` value and sub-type value of the kind with these identifiers.
    fn kind(&self, class: &str, sub_type: &str) -> (u8, u16) {
        let (&base_type, item_class) = self
            .bundle()
            .item_classes
            .iter()
            .find(|(_, item_class)| item_class.id.0 == class)
            .unwrap();
        let (&sub_type, _) = item_class
            .sub_types
            .iter()
            .find(|(_, kind)| kind.id.0 == sub_type)
            .unwrap();
        (base_type, sub_type)
    }

    fn send(&mut self, msg: Value) {
        let envelope = Envelope {
            origin: "http://localhost:8080".to_owned(),
            tab: 1,
            ts: 0,
            game_version: Some("Dungeon Crawl Stone Soup 0.34.1".to_owned()),
            msg,
        };
        for event in self.source.ingest(&envelope) {
            if let SourceEvent::Visible(visible) = event
                && let Some(inventory) = visible.inventory
            {
                self.inventory = inventory;
            }
        }
    }

    /// A new game process starts, for a character of this name, and sends its turn.
    fn start(&mut self, name: &str, turn: u32) {
        self.send(json!({ "msg": "game_started" }));
        self.send(json!({ "msg": "player", "name": name, "species": "Minotaur", "turn": turn }));
    }

    fn carry(&mut self, slot: u16, item: &Value) {
        self.send(json!({ "msg": "player", "inv": { slot.to_string(): item } }));
    }

    fn message(&mut self, text: &str) {
        self.send(json!({ "msg": "msgs", "messages": [{ "text": text, "turn": 1 }] }));
    }

    /// The sub-type the item under `letter` is decoded as, if it is.
    fn sub_type(&self, letter: char) -> Option<EnumId> {
        self.inventory
            .iter()
            .find(|item| item.letter == letter)
            .unwrap()
            .kind
            .clone()
            .unwrap()
            .sub_type
    }
}

/// An item in its slot as crawl first sends it.
fn item(kind: (u8, u16), letter: char, name: &str, flags: i64) -> Value {
    json!({
        "base_type": kind.0,
        "sub_type": kind.1,
        "quantity": 1,
        "flags": flags,
        "letter": u32::from(letter),
        "name": name,
    })
}

#[test]
fn a_wand_identified_by_use_identifies_another_of_its_kind_in_another_slot() {
    let mut game = Game::new();
    let flame = game.kind("OBJ_WANDS", "WAND_FLAME");
    game.start("Tiv", 1);
    game.carry(0, &item(flame, 'a', "iron wand", 0));
    game.carry(1, &item(flame, 'b', "iron wand", 0));
    assert_eq!(game.sub_type('b'), None);

    // Zapping the first flags it and renames it; crawl resends nothing of the second.
    game.carry(
        0,
        &json!({ "flags": IDENTIFIED, "name": "wand of flame (11)" }),
    );

    assert_eq!(game.sub_type('a'), Some(EnumId("WAND_FLAME".to_owned())));
    assert_eq!(game.sub_type('b'), Some(EnumId("WAND_FLAME".to_owned())));
}

#[test]
fn a_scroll_whose_kind_the_player_learns_by_sight_is_decoded_and_a_way_out() {
    let mut game = Game::new();
    let teleportation = game.kind("OBJ_SCROLLS", "SCR_TELEPORTATION");
    game.start("Tiv", 1);
    game.carry(
        2,
        &item(teleportation, 'c', "scroll labelled YMAYFF GACUBBO", 0),
    );
    assert_eq!(game.sub_type('c'), None);

    game.message(
        "You learned that your scroll labelled YMAYFF GACUBBO is actually a scroll of \
         teleportation.",
    );

    let decoded = game.sub_type('c').unwrap();
    assert_eq!(decoded.0, "SCR_TELEPORTATION");
    assert!(
        game.bundle()
            .ways_out
            .items
            .iter()
            .any(|way_out| way_out.sub_type == decoded)
    );
}

#[test]
fn a_new_game_starts_knowing_no_kinds() {
    let mut game = Game::new();
    let flame = game.kind("OBJ_WANDS", "WAND_FLAME");
    game.start("Tiv", 1);
    game.carry(0, &item(flame, 'a', "wand of flame (11)", IDENTIFIED));
    assert_eq!(game.sub_type('a'), Some(EnumId("WAND_FLAME".to_owned())));

    game.start("Rook", 1);
    game.carry(0, &item(flame, 'a', "brass wand", 0));

    assert_eq!(game.sub_type('a'), None);
}

#[test]
fn an_item_of_a_kind_never_identified_stays_undecoded() {
    let mut game = Game::new();
    let flame = game.kind("OBJ_WANDS", "WAND_FLAME");
    let digging = game.kind("OBJ_WANDS", "WAND_DIGGING");
    let fog = game.kind("OBJ_SCROLLS", "SCR_FOG");
    game.start("Tiv", 1);
    game.carry(0, &item(flame, 'a', "wand of flame (11)", IDENTIFIED));
    game.carry(1, &item(digging, 'b', "brass wand", 0));
    game.carry(2, &item(fog, 'c', "scroll labelled XOB", 0));

    game.message(
        "You learned that your scroll labelled ZUN is actually a scroll of teleportation.",
    );

    assert_eq!(game.sub_type('b'), None);
    assert_eq!(game.sub_type('c'), None);
}

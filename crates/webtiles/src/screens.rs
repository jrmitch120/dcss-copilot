//! The screens the player has open over the map, merged from webtiles' menu and UI-layout
//! messages as the page's `ui.js` and `menu.js` stack them.

use std::collections::BTreeMap;

use copilot_core::{Colour, Screen, ScreenRead};
use serde_json::Value;

use crate::json::{flag, int, string};
use crate::markup::{plain, plain_html};

/// The `txt` text area crawl draws console-style screens into, such as the skills screen.
const CRT_TEXT_AREA: &str = "menu_txt";

/// What `menu.cc` names a menu with no tag, when the screen has no better name.
const UNTAGGED_MENU: &str = "menu";

/// The title crawl gives the mutations screen, which has no tag.
const MUTATIONS_TITLE: &str = "Innate Abilities, Weirdness & Mutations";

/// The heading crawl starts the dungeon overview with, which has no tag.
const DUNGEON_OVERVIEW_HEADING: &str = "Dungeon Overview and Level Annotations";

/// The open screens, bottom first, and the console-style text area.
#[derive(Debug, Default)]
pub struct Screens {
    stack: Vec<Frame>,
    /// The text area's rows, by row number.
    crt: BTreeMap<u32, String>,
}

/// One open screen.
#[derive(Debug)]
enum Frame {
    /// A menu: crawl sends its title and entries.
    Menu {
        tag: String,
        title: String,
        items: Vec<Item>,
    },
    /// A console-style screen, drawn into the text area. Crawl sends the text after it opens
    /// the screen, so until then the text area still holds whatever was drawn before.
    Crt { tag: String, drawn: bool },
    /// A UI layout, such as a scrolling text popup or a description.
    Layout {
        kind: String,
        tag: String,
        title: String,
        text: String,
    },
}

/// A menu entry: its text, and the colour crawl draws it in when it isn't the menu's own
/// (`Menu::webtiles_write_item` leaves the colour out then).
#[derive(Debug, Clone)]
struct Item {
    text: String,
    colour: Option<Colour>,
}

impl Screens {
    /// Merges a message, if it is one about screens.
    pub fn merge(&mut self, msg: &Value) {
        match msg.get("msg").and_then(Value::as_str) {
            Some("menu") => {
                // Crawl redraws an open menu in place by sending it again to replace the top one.
                if msg.get("replace").is_some_and(flag) {
                    self.stack.pop();
                }
                self.stack.push(menu(msg));
            }
            Some("update_menu") => {
                if let (Some(Frame::Menu { title, .. }), Some(text)) =
                    (self.stack.last_mut(), title_text(msg))
                {
                    *title = text;
                }
            }
            Some("update_menu_items") => self.update_items(msg),
            Some("close_menu") => {
                if matches!(
                    self.stack.last(),
                    Some(Frame::Menu { .. } | Frame::Crt { .. })
                ) {
                    self.stack.pop();
                }
            }
            Some("close_all_menus") => self.stack.clear(),
            Some("ui-push") => self.stack.push(layout(msg)),
            Some("ui-pop") => {
                if matches!(self.stack.last(), Some(Frame::Layout { .. })) {
                    self.stack.pop();
                }
            }
            Some("ui-state") => self.update_layout(msg),
            // A spectator's join, or a resync, sends the whole stack afresh.
            Some("ui-stack") => {
                self.stack.clear();
                for item in msg
                    .get("items")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    self.merge(item);
                }
            }
            Some("txt") if string(msg, "id").as_deref() == Some(CRT_TEXT_AREA) => self.draw(msg),
            _ => {}
        }
    }

    /// The screen on top, read as plain text, if any is open and crawl has drawn it.
    pub fn open(&self) -> Option<ScreenRead> {
        Some(match self.stack.last()? {
            Frame::Menu { tag, title, items } => ScreenRead {
                screen: match tag.as_str() {
                    "spell" => Screen::Spells,
                    "ability" => Screen::Abilities,
                    "" if title == MUTATIONS_TITLE => Screen::Mutations,
                    "" => Screen::Other(UNTAGGED_MENU.to_owned()),
                    tag => Screen::Other(tag.to_owned()),
                },
                title: non_empty(title),
                lines: items.iter().map(|item| item.text.clone()).collect(),
                colours: items
                    .iter()
                    .enumerate()
                    .filter_map(|(index, item)| Some((index, item.colour?)))
                    .collect(),
            },
            Frame::Crt { drawn: false, .. } => return None,
            Frame::Crt { tag, .. } => ScreenRead {
                screen: match tag.as_str() {
                    "skills" => Screen::Skills,
                    tag => Screen::Other(tag.to_owned()),
                },
                title: None,
                lines: self.crt_lines(),
                colours: BTreeMap::new(),
            },
            Frame::Layout {
                kind,
                tag,
                title,
                text,
            } => {
                let lines = lines(text);
                ScreenRead {
                    screen: match (kind.as_str(), tag.as_str()) {
                        ("formatted-scroller", "resists") => Screen::CharacterOverview,
                        ("formatted-scroller", "dump") => Screen::CharacterDump,
                        ("formatted-scroller", "")
                            if lines
                                .first()
                                .is_some_and(|line| line.trim() == DUNGEON_OVERVIEW_HEADING) =>
                        {
                            Screen::DungeonOverview
                        }
                        ("formatted-scroller", tag) if !tag.is_empty() => {
                            Screen::Other(tag.to_owned())
                        }
                        (kind, _) => Screen::Other(kind.to_owned()),
                    },
                    title: non_empty(title),
                    lines,
                    colours: BTreeMap::new(),
                }
            }
        })
    }

    /// Replaces a run of the top menu's entries, from `chunk_start`.
    fn update_items(&mut self, msg: &Value) {
        let Some(Frame::Menu { items, .. }) = self.stack.last_mut() else {
            return;
        };
        let start = int::<usize>(msg, "chunk_start").unwrap_or(0);
        for (offset, item) in entries(msg).into_iter().enumerate() {
            let index = start + offset;
            if index < items.len() {
                items[index] = item;
            } else {
                items.push(item);
            }
        }
    }

    /// A UI layout's state change: a scrolling text popup's text can change while it is open.
    fn update_layout(&mut self, msg: &Value) {
        if let Some(Frame::Layout { kind, text, .. }) = self.stack.last_mut()
            && string(msg, "type").as_deref() == Some(kind.as_str())
            && let Some(new_text) = string(msg, "text")
        {
            *text = new_text;
        }
    }

    /// Draws a `txt` delta into the text area: `clear` blanks it first.
    fn draw(&mut self, msg: &Value) {
        if let Some(Frame::Crt { drawn, .. }) = self.stack.last_mut() {
            *drawn = true;
        }
        if msg.get("clear").and_then(Value::as_bool) == Some(true) {
            self.crt.clear();
        }
        for (row, html) in msg
            .get("lines")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
        {
            if let (Ok(row), Some(html)) = (row.parse(), html.as_str()) {
                self.crt.insert(row, plain_html(html));
            }
        }
    }

    /// The text area's rows, without the blank ones after the last.
    fn crt_lines(&self) -> Vec<String> {
        let Some(&last) = self.crt.keys().max() else {
            return Vec::new();
        };
        let mut lines: Vec<String> = (0..=last)
            .map(|row| {
                self.crt
                    .get(&row)
                    .map_or("", |line| line.trim_end())
                    .to_owned()
            })
            .collect();
        while lines.last().is_some_and(String::is_empty) {
            lines.pop();
        }
        lines
    }
}

fn menu(msg: &Value) -> Frame {
    let tag = string(msg, "tag").unwrap_or_default();
    if string(msg, "type").as_deref() == Some("crt") {
        return Frame::Crt { tag, drawn: false };
    }
    Frame::Menu {
        tag,
        title: title_text(msg).unwrap_or_default(),
        items: entries(msg),
    }
}

fn layout(msg: &Value) -> Frame {
    // A description popup has a body where a scrolling text popup has text.
    let text = string(msg, "text").or_else(|| string(msg, "body"));
    Frame::Layout {
        kind: string(msg, "type").unwrap_or_default(),
        tag: string(msg, "tag").unwrap_or_default(),
        title: string(msg, "title")
            .map(|title| plain(&title))
            .unwrap_or_default(),
        text: text.unwrap_or_default(),
    }
}

/// A menu message's title, which crawl sends as `{"title": {"text": ...}}`.
fn title_text(msg: &Value) -> Option<String> {
    msg.get("title")
        .and_then(|title| title.get("text"))
        .and_then(Value::as_str)
        .map(|text| plain(text).trim_end().to_owned())
}

/// A menu message's entries, as plain text, each with its colour.
fn entries(msg: &Value) -> Vec<Item> {
    msg.get("items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|item| {
            let text = item.get("text").and_then(Value::as_str).unwrap_or_default();
            Item {
                text: plain(text).trim_end().to_owned(),
                colour: int::<u8>(item, "colour").and_then(Colour::from_crawl),
            }
        })
        .collect()
}

/// Colour-tagged text as plain lines, without trailing spaces.
fn lines(text: &str) -> Vec<String> {
    plain(text)
        .lines()
        .map(|line| line.trim_end().to_owned())
        .collect()
}

fn non_empty(text: &str) -> Option<String> {
    (!text.trim().is_empty()).then(|| text.to_owned())
}

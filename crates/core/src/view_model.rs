use serde::Serialize;
use specta::Type;

/// Everything the panel renders, pushed by the core to the webview.
///
/// The webview holds no game logic: it only draws this. TypeScript types are
/// generated from it, so the two sides can't drift apart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ViewModel {
    /// The running app's version, shown in the panel footer and About.
    pub app_version: String,
    /// How the app is connected to the browser.
    pub connection: Connection,
    /// The character in the game being followed, once one is identified.
    pub character: Option<Character>,
}

impl ViewModel {
    /// The view before any browser has connected.
    #[must_use]
    pub fn initial(app_version: &str) -> Self {
        Self {
            app_version: app_version.to_owned(),
            connection: Connection::NoBrowser,
            character: None,
        }
    }
}

/// How the app is connected to the browser extension.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Connection {
    /// No browser is connected.
    NoBrowser,
    /// A browser asked to connect for the first time: the player is asked "Allow this browser?".
    AllowBrowser {
        /// The browser extension's origin, such as `chrome-extension://…`.
        browser: String,
    },
    /// A browser is connected.
    Connected {
        /// A tab's buffer overflowed before the app connected, so its game is missing its start:
        /// the player should reload the game page, which sends no game input.
        buffer_overflowed: bool,
    },
    /// The app can't listen for browsers, so none can connect.
    RelayUnavailable {
        /// Why, for the player.
        reason: String,
    },
}

/// The character in the game being followed. A stat is `None` until the game reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Character {
    /// The character's name.
    pub name: String,
    /// The character's species.
    pub species: String,
    /// Current hit points.
    pub hp: Option<i32>,
    /// Maximum hit points.
    pub hp_max: Option<i32>,
    /// The game's turn count.
    pub turn: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_with_camel_case_fields_for_the_webview() {
        let json = serde_json::to_value(ViewModel::initial("1.2.3")).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "appVersion": "1.2.3", "connection": { "state": "noBrowser" }, "character": null })
        );
    }
}

//! The frames the browser extension and the app exchange over the localhost websocket. See
//! `docs/relay-protocol.md`.

use serde::{Deserialize, Serialize};

use copilot_core::Appearance;

use crate::Envelope;

/// The only compatibility contract between the extension and the app. Bump it whenever a frame
/// changes shape; the app also accepts the previous version.
pub const RELAY_PROTOCOL_VERSION: u32 = 2;

/// The port the app listens on for the extension, on 127.0.0.1 only.
pub const RELAY_PORT: u16 = 47_821;

/// A frame the extension sends the app.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ExtensionFrame {
    /// The first frame on every connection.
    Hello {
        /// The extension's relay protocol version.
        protocol: u32,
    },
    /// One captured webtiles message, buffered ones first.
    Envelope {
        /// The message, as relayed.
        envelope: Envelope,
    },
    /// A tab's buffer overflowed before it could be replayed.
    Overflow {
        /// The game server's page origin.
        origin: String,
        /// The tab whose buffer overflowed.
        tab: i64,
    },
    /// The browser opened a side panel, where the panel now shows, or has one open as it is
    /// welcomed. Since protocol 2.
    SidePanelOpened,
    /// The player pressed something in the side panel. The app answers with
    /// [`AppFrame::PanelCommandDone`]. Since protocol 2.
    PanelCommand {
        /// Chosen by the extension, to match the answer to it.
        id: u32,
        /// What the player asked for.
        command: PanelCommand,
    },
    /// Keeps the extension's service worker alive.
    Ping,
}

/// What the player can do from the side panel. The player answers "Allow this browser?" in the
/// app alone, so a browser can never allow itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum PanelCommand {
    /// Draw monsters and items as tiles or glyphs.
    SetAppearance {
        /// The player's choice.
        appearance: Appearance,
    },
    /// Open the app's Settings window.
    OpenSettings,
}

/// A frame the app sends the extension.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum AppFrame {
    /// The browser is allowed and its protocol is supported: envelopes may follow.
    Welcome {
        /// The app's relay protocol version.
        protocol: u32,
    },
    /// The app is asking the player "Allow this browser?".
    AwaitingApproval,
    /// The panel's view model, as the app's own panel draws it, for the side panels and the
    /// toolbar badge: on welcome, then each time it changes. Since protocol 2.
    View {
        /// The view model, in the shape of the panel's generated bindings.
        view: serde_json::Value,
    },
    /// The app did a [`PanelCommand`], or couldn't. Since protocol 2.
    PanelCommandDone {
        /// The command's id.
        id: u32,
        /// Why it couldn't, for the player, or `None` if it was done.
        error: Option<String>,
    },
    /// The app won't take this connection, and closes it.
    Refused {
        /// Why.
        reason: Refusal,
    },
}

/// Why the app refused a connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Refusal {
    /// The extension's protocol is neither the app's current nor previous version.
    UnsupportedProtocol,
    /// The player answered no to "Allow this browser?", now or earlier.
    BrowserDenied,
}

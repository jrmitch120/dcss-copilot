//! The frames the browser extension and the app exchange over the localhost websocket. See
//! `docs/relay-protocol.md`.

use serde::{Deserialize, Serialize};

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
    /// Where the browser window holding the game is, each time it moves, resizes or another
    /// window takes its place. Since protocol 2.
    Window {
        /// Its outer bounds.
        bounds: WindowBounds,
        /// Whether it is minimized, maximized or fullscreen.
        state: WindowState,
    },
    /// Keeps the extension's service worker alive.
    Ping,
}

/// A browser window's outer bounds on screen, as the browser's `windows` API gives them: in
/// screen pixels, which the desktop treats as logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowBounds {
    /// Its left edge.
    pub left: i32,
    /// Its top edge.
    pub top: i32,
    /// Its width.
    pub width: u32,
    /// Its height.
    pub height: u32,
}

/// A browser window's state, as the browser's `windows` API names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WindowState {
    /// Neither minimized, maximized nor fullscreen.
    Normal,
    /// Minimized out of sight.
    Minimized,
    /// Filling the screen's work area.
    Maximized,
    /// Filling the whole screen.
    Fullscreen,
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
    /// Move and resize the browser window holding the game to these bounds, to make room for the
    /// panel beside it. Since protocol 2.
    ResizeWindow {
        /// Its new outer bounds.
        bounds: WindowBounds,
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

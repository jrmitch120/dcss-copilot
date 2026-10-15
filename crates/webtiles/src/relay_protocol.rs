//! The frames the browser extension and the app exchange over the localhost websocket. See
//! `docs/relay-protocol.md`.

use serde::{Deserialize, Serialize};

use crate::Envelope;

/// The only compatibility contract between the extension and the app. Bump it whenever a frame
/// changes shape; the app also accepts the previous version.
pub const RELAY_PROTOCOL_VERSION: u32 = 1;

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
    /// Keeps the extension's service worker alive.
    Ping,
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

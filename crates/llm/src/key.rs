//! A provider's API key: a secret that can be sent to its provider and shown masked, and
//! nothing else.

use std::fmt;

/// A player's API key for one provider.
///
/// It can't be serialized, and it formats only as its masked form, so a key can't reach
/// settings, logs, Recordings, the LLM sidecar or crash files by accident. The one way to read
/// it is [`ApiKey::expose`], for the provider's request header and the keychain.
#[derive(Clone, PartialEq, Eq)]
pub struct ApiKey(String);

/// How many characters of a key's start and end its masked form shows.
const SHOWN_PREFIX: usize = 7;
const SHOWN_SUFFIX: usize = 4;

impl ApiKey {
    /// Wraps a key, without the whitespace a paste often brings along.
    #[must_use]
    pub fn new(key: &str) -> Self {
        Self(key.trim().to_owned())
    }

    /// Whether there is no key at all, such as an empty paste.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The key itself. Only for the provider's request header and the keychain.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// The key as the player sees it, such as `sk-ant-…a1b2`: enough to recognise, not to use.
    #[must_use]
    pub fn masked(&self) -> String {
        let chars: Vec<char> = self.0.chars().collect();
        // A key too short to mask safely shows none of itself.
        if chars.len() < 3 * (SHOWN_PREFIX + SHOWN_SUFFIX) {
            return "…".to_owned();
        }
        let start: String = chars[..SHOWN_PREFIX].iter().collect();
        let end: String = chars[chars.len() - SHOWN_SUFFIX..].iter().collect();
        format!("{start}…{end}")
    }
}

impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ApiKey({})", self.masked())
    }
}

impl fmt::Display for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.masked())
    }
}

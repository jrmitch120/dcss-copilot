//! The player's API keys: one per provider, kept in the OS keychain.
//!
//! A key lives only in the keychain and in memory. It is never written to settings, logs,
//! Recordings, the LLM sidecar or crash files, and [`ApiKey`] can't be serialized or printed
//! unmasked, so it can't get there by accident.

use std::collections::HashMap;

use copilot_llm::{ApiKey, Provider};

/// The keychain service the app files its keys under, one entry per provider.
const KEYCHAIN_SERVICE: &str = "io.github.jrmitch120.dcss-copilot";

/// Somewhere keys outlive the app: the OS keychain, or a stand-in in tests.
pub trait Keychain: Send + Sync {
    /// The key stored for `provider`, if any.
    ///
    /// # Errors
    /// If the keychain can't be reached.
    fn read(&self, provider: Provider) -> Result<Option<ApiKey>, KeychainError>;

    /// Stores `key` for `provider`, replacing any key stored before.
    ///
    /// # Errors
    /// If the keychain can't be reached or refuses the key.
    fn write(&self, provider: Provider, key: &ApiKey) -> Result<(), KeychainError>;

    /// Deletes the key stored for `provider`. Deleting a key that isn't there succeeds.
    ///
    /// # Errors
    /// If the keychain can't be reached.
    fn delete(&self, provider: Provider) -> Result<(), KeychainError>;
}

/// Why the keychain couldn't be used, for the player.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("the system keychain can't be used: {0}")]
pub struct KeychainError(pub String);

/// The OS keychain: Keychain Services on macOS, Credential Manager on Windows and the Secret
/// Service elsewhere.
pub struct OsKeychain;

impl OsKeychain {
    fn entry(provider: Provider) -> Result<keyring::Entry, KeychainError> {
        keyring::Entry::new(KEYCHAIN_SERVICE, provider.id()).map_err(keychain_error)
    }
}

// The keyring's errors never hold the secret, only what went wrong.
#[allow(clippy::needless_pass_by_value)] // Used as a `map_err` callback.
fn keychain_error(error: keyring::Error) -> KeychainError {
    KeychainError(error.to_string())
}

impl Keychain for OsKeychain {
    fn read(&self, provider: Provider) -> Result<Option<ApiKey>, KeychainError> {
        match Self::entry(provider)?.get_password() {
            Ok(key) => Ok(Some(ApiKey::new(&key))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(keychain_error(error)),
        }
    }

    fn write(&self, provider: Provider, key: &ApiKey) -> Result<(), KeychainError> {
        Self::entry(provider)?
            .set_password(key.expose())
            .map_err(keychain_error)
    }

    fn delete(&self, provider: Provider) -> Result<(), KeychainError> {
        match Self::entry(provider)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(keychain_error(error)),
        }
    }
}

/// Where the key the app uses for a provider comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeySource {
    /// The player's key, stored in the keychain.
    Keychain,
    /// The player's key, held in memory until the app exits because the keychain can't be used.
    Session {
        /// Why the keychain can't be used.
        reason: String,
    },
    /// The provider's environment variable, such as `ANTHROPIC_API_KEY`.
    Environment {
        /// The variable's name.
        variable: &'static str,
    },
}

/// The key a provider is called with, and where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldKey {
    /// The key.
    pub key: ApiKey,
    /// Where it came from.
    pub source: KeySource,
}

/// Why a key couldn't be saved or removed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum KeyError {
    /// The paste was empty.
    #[error("that key is empty")]
    Empty,
    /// The keychain refused to delete the key, so it would come back next launch.
    #[error(transparent)]
    Keychain(#[from] KeychainError),
}

/// Every provider's key, as the app holds it.
pub struct Keys {
    keychain: Box<dyn Keychain>,
    /// Keys the player gave the app, from the keychain or for this session.
    players: HashMap<Provider, HeldKey>,
    /// Keys found in the environment at launch, used when the player gave none.
    environment: HashMap<Provider, ApiKey>,
}

impl std::fmt::Debug for Keys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Keys")
            .field("players", &self.players)
            .field("environment", &self.environment)
            .finish_non_exhaustive()
    }
}

impl Keys {
    /// Reads each provider's key from `keychain`, and its environment variable through
    /// `environment`. A keychain that can't be read leaves the player's keys unknown until
    /// they are given again.
    pub fn load(keychain: Box<dyn Keychain>, environment: impl Fn(&str) -> Option<String>) -> Self {
        let mut players = HashMap::new();
        let mut from_environment = HashMap::new();
        for provider in Provider::ALL {
            match keychain.read(provider) {
                Ok(Some(key)) => {
                    players.insert(
                        provider,
                        HeldKey {
                            key,
                            source: KeySource::Keychain,
                        },
                    );
                }
                Ok(None) => {}
                Err(error) => crate::log::error(format_args!(
                    "keys: can't read the {} key: {error}",
                    provider.name()
                )),
            }
            if let Some(key) = environment(provider.key_variable())
                .map(|key| ApiKey::new(&key))
                .filter(|key| !key.is_empty())
            {
                from_environment.insert(provider, key);
            }
        }
        Self {
            keychain,
            players,
            environment: from_environment,
        }
    }

    /// The key `provider` is called with: the player's own, else the environment's.
    #[must_use]
    pub fn get(&self, provider: Provider) -> Option<HeldKey> {
        self.players.get(&provider).cloned().or_else(|| {
            self.environment.get(&provider).map(|key| HeldKey {
                key: key.clone(),
                source: KeySource::Environment {
                    variable: provider.key_variable(),
                },
            })
        })
    }

    /// Stores the player's key for `provider` in the keychain, or for this session only if the
    /// keychain can't be used.
    ///
    /// # Errors
    /// If the key is empty.
    pub fn save(&mut self, provider: Provider, key: ApiKey) -> Result<(), KeyError> {
        if key.is_empty() {
            return Err(KeyError::Empty);
        }
        let source = match self.keychain.write(provider, &key) {
            Ok(()) => KeySource::Keychain,
            Err(KeychainError(reason)) => KeySource::Session { reason },
        };
        self.players.insert(provider, HeldKey { key, source });
        Ok(())
    }

    /// Forgets the player's key for `provider`, here and in the keychain. A key from the
    /// environment stays, since the player set it outside the app.
    ///
    /// # Errors
    /// If the keychain can't delete it, so it would return next launch. The key is still
    /// forgotten until then.
    pub fn remove(&mut self, provider: Provider) -> Result<(), KeyError> {
        let removed = self.players.remove(&provider);
        match removed.map(|held| held.source) {
            Some(KeySource::Keychain) => Ok(self.keychain.delete(provider)?),
            _ => Ok(()),
        }
    }
}

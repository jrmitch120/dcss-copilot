//! The player's API keys: one per provider, kept in the OS keychain.
//!
//! A key lives only in the keychain and in memory, unless a player with no keychain opts in to a
//! file only their user can read. It is never written to settings, logs, Recordings, the LLM
//! sidecar or crash files, and [`ApiKey`] can't be serialized or printed unmasked, so it can't get
//! there by accident.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Write as _;
use std::path::{Path, PathBuf};

use copilot_llm::{Access, ApiKey, Provider};

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

/// The file a player with no keychain may opt in to, holding keys unencrypted, and gone once it
/// holds no key. Only their user can read it: its mode is 0600 on Unix, and on Windows it sits in
/// their own profile, whose permissions it inherits.
#[derive(Debug, Clone)]
pub struct KeyFile {
    path: PathBuf,
}

/// Why the key file couldn't be used. It never quotes the file, which holds keys.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("the unencrypted key file can't be used: {0}")]
pub struct KeyFileError(String);

impl KeyFile {
    /// The key file at `path`, which needn't exist yet.
    #[must_use]
    pub fn new(path: &Path) -> Self {
        Self {
            path: path.to_owned(),
        }
    }

    fn read(&self) -> Result<BTreeMap<String, String>, KeyFileError> {
        let bytes = match std::fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(BTreeMap::new());
            }
            Err(error) => return Err(KeyFileError(error.kind().to_string())),
        };
        // serde_json's error can quote what it read, which is a key.
        serde_json::from_slice(&bytes).map_err(|_| KeyFileError("it is malformed".to_owned()))
    }

    /// Replaces the file whole, so it is never seen half-written or readable by anyone else.
    fn write(&self, keys: &BTreeMap<String, String>) -> Result<(), KeyFileError> {
        let io = |error: std::io::Error| KeyFileError(error.kind().to_string());
        if keys.is_empty() {
            return match std::fs::remove_file(&self.path) {
                Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(io(error)),
                _ => Ok(()),
            };
        }
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(io)?;
        }
        let json = serde_json::to_vec_pretty(keys)
            .map_err(|_| KeyFileError("the keys can't be written".to_owned()))?;
        let partial = self.path.with_extension("partial");
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        let mut file = options.open(&partial).map_err(io)?;
        file.write_all(&json).map_err(io)?;
        drop(file);
        std::fs::rename(&partial, &self.path).map_err(io)
    }

    fn store(&self, provider: Provider, key: &ApiKey) -> Result<(), KeyFileError> {
        let mut keys = self.read()?;
        keys.insert(provider.id().to_owned(), key.expose().to_owned());
        self.write(&keys)
    }

    fn forget(&self, provider: Provider) -> Result<(), KeyFileError> {
        let mut keys = self.read()?;
        if keys.remove(provider.id()).is_some() {
            self.write(&keys)?;
        }
        Ok(())
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
    /// The player's key, in the unencrypted file they opted in to because the keychain couldn't
    /// be used.
    File,
    /// The provider's environment variable, such as `ANTHROPIC_API_KEY`, which the player said to
    /// use.
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
    /// The key file couldn't be changed.
    #[error(transparent)]
    File(#[from] KeyFileError),
    /// There is no key to store, or it is in the keychain already.
    #[error("there is no key held for this session to store")]
    NotInSession,
}

/// Every provider's key, as the app holds it.
pub struct Keys {
    keychain: Box<dyn Keychain>,
    file: KeyFile,
    /// Keys the player gave the app: from the keychain, the key file or for this session.
    players: HashMap<Provider, HeldKey>,
    /// Keys found in the environment at launch. Each is only offered until the player accepts it.
    environment: HashMap<Provider, ApiKey>,
    /// The providers whose environment variable the player said to use.
    accepted: HashSet<Provider>,
}

impl std::fmt::Debug for Keys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Keys")
            .field("players", &self.players)
            .field("environment", &self.environment)
            .field("accepted", &self.accepted)
            .finish_non_exhaustive()
    }
}

impl Keys {
    /// Reads each provider's key from `keychain`, else from `file`, and its environment variable
    /// through `environment`, used if the player's settings have `accepted` it. A keychain or file
    /// that can't be read leaves the player's keys unknown until they are given again.
    pub fn load(
        keychain: Box<dyn Keychain>,
        file: KeyFile,
        environment: impl Fn(&str) -> Option<String>,
        accepted: &[Provider],
    ) -> Self {
        let mut players = HashMap::new();
        let mut from_environment = HashMap::new();
        let in_file = file.read().unwrap_or_else(|error| {
            crate::log::error(format_args!("keys: {error}"));
            BTreeMap::new()
        });
        for provider in Provider::ALL {
            let Access::Key { variable } = provider.access() else {
                continue;
            };
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
            if let Some(key) = in_file.get(provider.id()).map(|key| ApiKey::new(key)) {
                // A key in both needs no plaintext copy. One that differs went to the file while
                // the keychain couldn't be used, so it is the newer.
                let in_keychain = players.get(&provider).map(|held| &held.key);
                let source = if in_keychain.is_some_and(|held| *held == key)
                    || (in_keychain.is_some() && keychain.write(provider, &key).is_ok())
                {
                    if let Err(error) = file.forget(provider) {
                        crate::log::error(format_args!("keys: {error}"));
                    }
                    KeySource::Keychain
                } else {
                    KeySource::File
                };
                players.insert(provider, HeldKey { key, source });
            }
            if let Some(key) = environment(variable)
                .map(|key| ApiKey::new(&key))
                .filter(|key| !key.is_empty())
            {
                from_environment.insert(provider, key);
            }
        }
        Self {
            keychain,
            file,
            players,
            environment: from_environment,
            accepted: accepted.iter().copied().collect(),
        }
    }

    /// The key `provider` is called with: the player's own, else the environment's if they said to
    /// use it.
    #[must_use]
    pub fn get(&self, provider: Provider) -> Option<HeldKey> {
        self.players.get(&provider).cloned().or_else(|| {
            let Access::Key { variable } = provider.access() else {
                return None;
            };
            self.environment
                .get(&provider)
                .filter(|_| self.accepted.contains(&provider))
                .map(|key| HeldKey {
                    key: key.clone(),
                    source: KeySource::Environment { variable },
                })
        })
    }

    /// The key in `provider`'s environment variable, to offer while no key is in use.
    #[must_use]
    pub fn offered(&self, provider: Provider) -> Option<&ApiKey> {
        if self.get(provider).is_some() {
            return None;
        }
        self.environment.get(&provider)
    }

    /// The player said to use the key in `provider`'s environment variable, or to stop.
    pub fn use_environment(&mut self, provider: Provider, on: bool) {
        if on {
            self.accepted.insert(provider);
        } else {
            self.accepted.remove(&provider);
        }
    }

    /// The providers whose environment variable the player said to use, for their settings.
    #[must_use]
    pub fn accepted(&self) -> Vec<Provider> {
        Provider::ALL
            .into_iter()
            .filter(|provider| self.accepted.contains(provider))
            .collect()
    }

    /// Stores the player's key for `provider` in the keychain. If the keychain can't be used it is
    /// held for this session only, unless the player already opted in to the key file for this
    /// provider. A key the keychain takes leaves no copy behind in the file.
    ///
    /// # Errors
    /// If the key is empty, or the key file can't be changed. The key is still held until the app
    /// exits.
    pub fn save(&mut self, provider: Provider, key: ApiKey) -> Result<(), KeyError> {
        if key.is_empty() {
            return Err(KeyError::Empty);
        }
        let opted_in = self.source(provider) == Some(&KeySource::File);
        let (source, changed) = match self.keychain.write(provider, &key) {
            Ok(()) => (KeySource::Keychain, self.file.forget(provider)),
            Err(_) if opted_in => (KeySource::File, self.file.store(provider, &key)),
            Err(KeychainError(reason)) => (KeySource::Session { reason }, Ok(())),
        };
        self.players.insert(provider, HeldKey { key, source });
        Ok(changed?)
    }

    /// The player opted in to storing `provider`'s session key unencrypted, or out again. Opting
    /// out takes it off the disk, and tries the keychain once more before holding it for the
    /// session.
    ///
    /// # Errors
    /// If there is no such key, or the key file can't be changed, which leaves the key as it was.
    pub fn store_unencrypted(&mut self, provider: Provider, on: bool) -> Result<(), KeyError> {
        let held = self
            .players
            .get_mut(&provider)
            .ok_or(KeyError::NotInSession)?;
        match (&held.source, on) {
            (KeySource::Session { .. }, true) => {
                self.file.store(provider, &held.key)?;
                held.source = KeySource::File;
            }
            (KeySource::File, false) => {
                self.file.forget(provider)?;
                held.source = match self.keychain.write(provider, &held.key) {
                    Ok(()) => KeySource::Keychain,
                    Err(KeychainError(reason)) => KeySource::Session { reason },
                };
            }
            (KeySource::File, true) | (KeySource::Session { .. }, false) => {}
            (KeySource::Keychain | KeySource::Environment { .. }, _) => {
                return Err(KeyError::NotInSession);
            }
        }
        Ok(())
    }

    /// Forgets the player's key for `provider`, here, in the keychain and in the key file. A key
    /// from the environment stays, since the player set it outside the app.
    ///
    /// # Errors
    /// If the keychain or key file can't delete it, so it would return next launch. The key is
    /// still forgotten until then.
    pub fn remove(&mut self, provider: Provider) -> Result<(), KeyError> {
        let removed = self.players.remove(&provider);
        let from_file = self.file.forget(provider);
        if let Some(KeySource::Keychain) = removed.map(|held| held.source) {
            self.keychain.delete(provider)?;
        }
        Ok(from_file?)
    }

    fn source(&self, provider: Provider) -> Option<&KeySource> {
        self.players.get(&provider).map(|held| &held.source)
    }
}

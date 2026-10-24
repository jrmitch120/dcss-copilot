//! The player's settings, saved as JSON in the app's config directory.

use std::path::{Path, PathBuf};

use copilot_core::Appearance;
use copilot_llm::{ModelChoice, ModelSlot};
use serde::{Deserialize, Serialize};

/// The player's settings and where they are saved.
#[derive(Debug)]
pub struct Settings {
    path: PathBuf,
    saved: Saved,
}

/// What the settings file holds. A setting missing from the file takes its default, so files
/// from older versions still load.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Saved {
    appearance: Appearance,
    quick_model: ModelChoice,
    deep_model: ModelChoice,
}

impl Default for Saved {
    fn default() -> Self {
        Self {
            appearance: Appearance::default(),
            quick_model: ModelChoice::default_for(ModelSlot::Quick),
            deep_model: ModelChoice::default_for(ModelSlot::Deep),
        }
    }
}

/// Why the settings couldn't be read or written.
#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    /// The file couldn't be read or written.
    #[error("settings at {path} can't be read or written: {source}")]
    Io {
        /// The settings file.
        path: PathBuf,
        /// What went wrong.
        source: std::io::Error,
    },
    /// The file isn't the JSON the app writes.
    #[error("settings at {path} are malformed: {source}")]
    Malformed {
        /// The settings file.
        path: PathBuf,
        /// What was wrong with it.
        source: serde_json::Error,
    },
}

impl Settings {
    /// Every setting at its default, to be saved at `path` when one changes.
    #[must_use]
    pub fn defaults(path: &Path) -> Self {
        Self {
            path: path.to_owned(),
            saved: Saved::default(),
        }
    }

    /// Reads the saved settings, or the defaults if none have been saved yet.
    ///
    /// # Errors
    /// If the file exists but can't be read or parsed.
    pub fn load(path: &Path) -> Result<Self, SettingsError> {
        let saved = match std::fs::read(path) {
            Ok(bytes) => {
                serde_json::from_slice(&bytes).map_err(|source| SettingsError::Malformed {
                    path: path.to_owned(),
                    source,
                })?
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Saved::default(),
            Err(source) => {
                return Err(SettingsError::Io {
                    path: path.to_owned(),
                    source,
                });
            }
        };
        Ok(Self {
            path: path.to_owned(),
            saved,
        })
    }

    /// Whether monsters and items are drawn as tiles or glyphs.
    #[must_use]
    pub fn appearance(&self) -> Appearance {
        self.saved.appearance
    }

    /// Changes how monsters and items are drawn, and saves it.
    ///
    /// # Errors
    /// If the settings can't be saved. The change still applies until the app exits.
    pub fn set_appearance(&mut self, appearance: Appearance) -> Result<(), SettingsError> {
        self.saved.appearance = appearance;
        self.save()
    }

    /// The model a slot calls.
    #[must_use]
    pub fn model(&self, slot: ModelSlot) -> &ModelChoice {
        match slot {
            ModelSlot::Quick => &self.saved.quick_model,
            ModelSlot::Deep => &self.saved.deep_model,
        }
    }

    /// Changes the model a slot calls, and saves it.
    ///
    /// # Errors
    /// If the settings can't be saved. The change still applies until the app exits.
    pub fn set_model(&mut self, slot: ModelSlot, choice: ModelChoice) -> Result<(), SettingsError> {
        match slot {
            ModelSlot::Quick => self.saved.quick_model = choice,
            ModelSlot::Deep => self.saved.deep_model = choice,
        }
        self.save()
    }

    fn save(&self) -> Result<(), SettingsError> {
        let io = |source| SettingsError::Io {
            path: self.path.clone(),
            source,
        };
        let json = serde_json::to_vec_pretty(&self.saved)
            .map_err(std::io::Error::other)
            .map_err(io)?;
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(io)?;
        }
        std::fs::write(&self.path, json).map_err(io)
    }
}

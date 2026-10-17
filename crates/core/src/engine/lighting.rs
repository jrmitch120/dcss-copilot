//! How the light falls on the player as monsters aim at them: crawl's Lighting terms in a
//! monster's to-hit, ported closely (ADR 0002). Line numbers are crawl 0.34.1's.

/// `BACKLIGHT_TO_HIT_BONUS` (`attack.h:16`): how much easier a backlit player is to hit.
const BACKLIGHT_TO_HIT_BONUS: i32 = 5;

/// `UMBRA_TO_HIT_MALUS` (`attack.h:17`): how much harder a player in an umbra is to hit, for a
/// monster without nightvision.
const UMBRA_TO_HIT_MALUS: i32 = -3;

/// How the light falls on the player, which changes how well monsters aim at them: crawl's
/// `player::backlit(false)` and `player::umbra()` (`player.cc:7850`, `player.cc:7864`), which
/// never hold together.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Lighting {
    /// Neither backlit nor in an umbra.
    #[default]
    Plain,
    /// Backlit: lit by a halo or their own glow, such as a corona.
    Backlit,
    /// In an umbra that no halo lights.
    Umbra,
}

impl Lighting {
    /// What the light adds to the to-hit of a monster aiming at the player, with or without
    /// `nightvision`: the lighting terms of `attack::post_roll_to_hit_modifiers`
    /// (`attack.cc:244`), which `bolt::apply_lighting` (`beam.cc:3483`) doubles.
    pub(crate) fn to_hit_modifier(self, nightvision: bool) -> i32 {
        match self {
            Self::Backlit => BACKLIGHT_TO_HIT_BONUS,
            Self::Umbra if !nightvision => UMBRA_TO_HIT_MALUS,
            Self::Umbra | Self::Plain => 0,
        }
    }
}

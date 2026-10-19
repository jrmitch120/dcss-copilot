//! The player's resistances, holiness and god, as crawl's damage takes them.
//!
//! Line numbers are crawl 0.34.1's (`crawl-ref/source/` at tag 0.34.1).

/// What about the player changes the damage a beam deals them, or a brand adds to a hit: their
/// resistances, as the character overview shows them, and their holiness and god.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Resists {
    /// `rFire`, from -3 to 3.
    pub(crate) fire: i32,
    /// `rCold`, from -3 to 3.
    pub(crate) cold: i32,
    /// `rNeg`, from 0 to 3.
    pub(crate) negative: i32,
    /// `rPois`: -1 to 1, or 3 for immunity.
    pub(crate) poison: i32,
    /// `rElec`: -1 to 1.
    pub(crate) electricity: i32,
    /// `rCorr`: 0 or 1.
    pub(crate) corrosion: i32,
    /// What they are, and whom they worship.
    pub(crate) holiness: Holiness,
}

/// What the player is, and whom they worship, as the damage crawl deals them tells it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Holiness {
    /// Undead in whole or part, or demonic: `player::undead_or_demonic` (`player.cc:6893`),
    /// whom holy wrath hurts and foul flame spares.
    pub(crate) undead_or_demonic: bool,
    /// Holy, or a good god's worshipper: `player::is_holy` (`player.cc:6906`), whom holy fire
    /// spares and foul flame hurts more.
    pub(crate) holy: bool,
    /// Natural or a plant (`MH_NATURAL | MH_PLANT`), as draining needs its defender to be
    /// (`attack::drain_defender`, `attack.cc:670`).
    pub(crate) drainable: bool,
    /// Nonliving or lifeless undead, whom miasma doesn't harm (`player::res_miasma`,
    /// `player.cc:6987`).
    pub(crate) miasma_immune: bool,
}

impl Holiness {
    /// A living mortal who worships no good god.
    pub(crate) const MORTAL: Self = Self {
        undead_or_demonic: false,
        holy: false,
        drainable: true,
        miasma_immune: false,
    };
}

impl Resists {
    /// A living mortal who resists nothing and worships no good god: how the player is taken when
    /// the Advisor hasn't read their resistances.
    pub(crate) const NONE: Self = Self {
        fire: 0,
        cold: 0,
        negative: 0,
        poison: 0,
        electricity: 0,
        corrosion: 0,
        holiness: Holiness::MORTAL,
    };
}

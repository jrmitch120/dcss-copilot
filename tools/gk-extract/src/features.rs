//! Dungeon features from `feat_defs` in `feature-data.h`: crawl's name for each, what it is to
//! someone walking onto it, and whether it is a staircase the player can take.

use std::collections::{BTreeMap, HashMap};

use copilot_gk::{EnumId, Feature, StairDirection, Terrain};

use crate::ExtractError;
use crate::cpp::{Source, Token};

/// How many fields a `feat_defs` entry has once its colour macros are expanded: the feature,
/// its name and vault name, two dchars, five colours, its flags and its minimap colour.
const FEATURE_FIELDS: usize = 12;

/// The `feature_flag_type` flag (`feature.h`) of a feature that blocks movement.
pub const SOLID_FLAG: &str = "FFT_SOLID";

/// The `map_feature` minimap colours (`map-feature.h`) that tell a feature's terrain.
pub const MAP_FEATURES: [&str; 3] = [DOOR, DEEP_WATER, LAVA];
const DOOR: &str = "MF_DOOR";
const DEEP_WATER: &str = "MF_DEEP_WATER";
const LAVA: &str = "MF_LAVA";

/// The `dungeon_char_type` glyphs (`dungeon-char-type.h`) of a staircase up and down.
pub const STAIR_CHARS: [&str; 2] = [STAIRS_UP, STAIRS_DOWN];
const STAIRS_UP: &str = "DCHAR_STAIRS_UP";
const STAIRS_DOWN: &str = "DCHAR_STAIRS_DOWN";

/// What a crawl release series decides about features in code rather than in `feat_defs`,
/// ported by hand. Every feature named must be in `dungeon_feature_type`.
pub struct FeatureRules {
    /// The closed doors that never open: a vault warden's seals (`feat_is_sealed`,
    /// `terrain.cc`).
    pub sealed_doors: &'static [&'static str],
    /// The features drawn as staircases that take the player to no other level: sealed stairs
    /// (`feat_is_sealed`), and the staircase out of the dungeon, which ends the game.
    pub not_stairs: &'static [&'static str],
    /// The water, shallow or deep (`feat_is_water`), each named by one of `water_functions` in
    /// `terrain.cc`.
    pub water: &'static [&'static str],
    /// The functions of `terrain.cc` that name the water.
    pub water_functions: &'static [&'static str],
}

/// What crawl 0.34 decides about features in code.
pub const FEATURE_RULES_0_34: FeatureRules = FeatureRules {
    sealed_doors: &["DNGN_SEALED_DOOR", "DNGN_SEALED_CLEAR_DOOR"],
    not_stairs: &[
        "DNGN_SEALED_STAIRS_UP",
        "DNGN_SEALED_STAIRS_DOWN",
        "DNGN_EXIT_DUNGEON",
    ],
    water: &[
        "DNGN_SHALLOW_WATER",
        "DNGN_TOXIC_BOG",
        "DNGN_MANGROVE",
        "DNGN_DEEP_WATER",
        "DNGN_OPEN_SEA",
    ],
    water_functions: &["feat_is_shallow_water", "feat_is_deep_water"],
};

/// The file whose functions name the water.
const TERRAIN_FILE: &str = "terrain.cc";

/// Every feature `feat_defs` defines, by its value in `dungeon_feature_type`, whose enumerators
/// are `enumeration`, with `terrain`, `terrain.cc`, to check the water against.
pub fn features(
    source: &Source,
    file: &str,
    enumeration: &[(String, i64)],
    terrain: &Source,
    rules: &FeatureRules,
) -> Result<BTreeMap<u16, Feature>, ExtractError> {
    let values: HashMap<&str, i64> = enumeration
        .iter()
        .map(|(id, value)| (id.as_str(), *value))
        .collect();
    for &id in rules
        .sealed_doors
        .iter()
        .chain(rules.not_stairs)
        .chain(rules.water)
    {
        if !values.contains_key(id) {
            return Err(ExtractError::Missing {
                file: "dungeon-feature-type.h".to_owned(),
                what: id.to_owned(),
            });
        }
    }
    let water_bodies = rules
        .water_functions
        .iter()
        .map(|function| terrain.function_body(function))
        .collect::<Result<Vec<_>, _>>()?;
    for &id in rules.water {
        let ident = Token::Ident(id.to_owned());
        if !water_bodies.iter().any(|body| body.contains(&ident)) {
            return Err(ExtractError::Missing {
                file: TERRAIN_FILE.to_owned(),
                what: format!("{id}, in {}", rules.water_functions.join(" or ")),
            });
        }
    }
    let unreadable = |what: String| ExtractError::Unreadable {
        file: file.to_owned(),
        what,
    };
    let mut features = BTreeMap::new();
    for fields in source.entries("feat_defs")? {
        let [
            id,
            name,
            _vault_name,
            dchar,
            _magic_dchar,
            _,
            _,
            _,
            _,
            _,
            flags,
            minimap,
        ] = fields.as_slice()
        else {
            return Err(unreadable(format!(
                "an entry of feat_defs without {FEATURE_FIELDS} fields"
            )));
        };
        let (
            [Token::Ident(id)],
            [Token::Str(name)],
            [Token::Ident(dchar)],
            [Token::Ident(minimap)],
        ) = (
            id.as_slice(),
            name.as_slice(),
            dchar.as_slice(),
            minimap.as_slice(),
        )
        else {
            return Err(unreadable("an entry of feat_defs".to_owned()));
        };
        let value = values
            .get(id.as_str())
            .ok_or_else(|| ExtractError::Missing {
                file: "dungeon-feature-type.h".to_owned(),
                what: format!("{id}, which {file} defines"),
            })?;
        let solid = flags.contains(&Token::Ident(SOLID_FLAG.to_owned()));
        let terrain = match minimap.as_str() {
            _ if rules.sealed_doors.contains(&id.as_str()) => Terrain::Solid,
            DOOR if solid => Terrain::ClosedDoor,
            _ if solid => Terrain::Solid,
            DEEP_WATER => Terrain::DeepWater,
            LAVA => Terrain::Lava,
            _ => Terrain::Open,
        };
        let stairs = match dchar.as_str() {
            _ if rules.not_stairs.contains(&id.as_str()) => None,
            STAIRS_UP => Some(StairDirection::Up),
            STAIRS_DOWN => Some(StairDirection::Down),
            _ => None,
        };
        features.insert(
            crate::in_range(id, *value)?,
            Feature {
                id: EnumId(id.clone()),
                name: name.clone(),
                terrain,
                stairs,
                water: rules.water.contains(&id.as_str()),
            },
        );
    }
    Ok(features)
}

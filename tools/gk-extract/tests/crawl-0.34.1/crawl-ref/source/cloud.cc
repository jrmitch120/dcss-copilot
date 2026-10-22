#include "AppHdr.h"

#include "cloud.h"

// (trimmed for the extractor test)

/// A portrait of a cloud_type.
struct cloud_data
{
    /// A (relatively) short name for the cloud. May be referenced from lua.
    const char* terse_name;
    /// Another name for the cloud. If nullptr, defaults to terse name.
    const char* verbose_name;
    /// The colour of the cloud in console.
    colour_t colour;
    /// Info for calculating cloud tiles.
    cloud_tile_info tile_info;
    /// The associated "beam" (effect) for this cloud type.
    beam_type beam_effect;
    /// How much damage the cloud does before defenses & resists.
    cloud_damage damage;
    /// Do multiple squares of this cloud block LOS?
    bool opaque;
};

/// A map from cloud_type to cloud_data.
static const cloud_data clouds[] = {
    // CLOUD_NONE,
    { "?", "?",                                 // terse, verbose name
    },
    // CLOUD_FIRE,
    { "flame", "blazing flames",                // terse, verbose name
       COLOUR_UNDEF,                            // colour
       { TILE_CLOUD_FIRE, CTVARY_DUR },         // tile
       BEAM_FIRE,                               // beam_effect
       NORMAL_CLOUD_DAM,                        // base, random damage
    },
    // CLOUD_MEPHITIC,
    { "noxious fumes", nullptr,                 // terse, verbose name
      GREEN,                                    // colour
      { TILE_CLOUD_MEPHITIC, CTVARY_DUR },      // tile
      BEAM_MEPHITIC,                            // beam_effect
    },
    // CLOUD_COLD,
    { "freezing vapour", "freezing vapours",    // terse, verbose name
      COLOUR_UNDEF,                             // colour
      { TILE_CLOUD_COLD, CTVARY_DUR },          // tile
      BEAM_COLD,                                // beam_effect
      NORMAL_CLOUD_DAM,                         // base, random damage
    },
    // CLOUD_POISON,
    { "poison gas", nullptr,                    // terse, verbose name
      LIGHTGREEN,                               // colour
      { TILE_CLOUD_POISON, CTVARY_DUR },        // tile
      BEAM_POISON,                              // beam_effect
      {0, 10},                                  // base, random damage
    },
    // CLOUD_BLACK_SMOKE,
    { "black smoke",  nullptr,                  // terse, verbose name
      DARKGREY,                                 // colour
      { TILE_CLOUD_BLACK_SMOKE, CTVARY_NONE },  // tile
      BEAM_NONE, {},                            // beam & damage
      true,                                     // opacity
    },
    // CLOUD_GREY_SMOKE,
    { "grey smoke",  nullptr,                   // terse, verbose name
      LIGHTGREY,                                // colour
      { TILE_CLOUD_GREY_SMOKE, CTVARY_NONE },   // tile
      BEAM_NONE, {},                            // beam & damage
      true,                                     // opacity
    },
    // CLOUD_BLUE_SMOKE,
    { "blue smoke",  nullptr,                   // terse, verbose name
      LIGHTBLUE,                                // colour
      { TILE_CLOUD_BLUE_SMOKE, CTVARY_NONE },   // tile
      BEAM_NONE, {},                            // beam & damage
      true,                                     // opacity
    },
    // CLOUD_PURPLE_SMOKE,
    { "purple smoke",  nullptr,                 // terse, verbose name
      MAGENTA,                                  // colour
      { TILE_CLOUD_TLOC_ENERGY, CTVARY_NONE },  // tile
      BEAM_NONE, {},                            // beam & damage
      true,                                     // opacity
    },
    // CLOUD_TLOC_ENERGY,
    { "translocational energy",  nullptr,       // terse, verbose name
      MAGENTA,                                  // colour
      { TILE_CLOUD_TLOC_ENERGY, CTVARY_NONE },  // tile
      BEAM_NONE, {},                            // beam & damage
      true,                                     // opacity
    },
    // CLOUD_FOREST_FIRE,
    { "spreading flames", "a forest fire",      // terse, verbose name
      COLOUR_UNDEF,                             // colour
      { TILE_CLOUD_FOREST_FIRE },               // tile
      BEAM_FIRE,                                // beam_effect
      NORMAL_CLOUD_DAM,                         // base, random damage
    },
    // CLOUD_STEAM,
    { "steam", "a cloud of scalding steam",     // terse, verbose name
      LIGHTGREY,                                // colour
      { TILE_CLOUD_GREY_SMOKE, CTVARY_NONE },   // tile
      BEAM_STEAM,                               // beam_effect
      {0, 16},                                  // base, random damage
      true,                                     // opacity
    },
#if TAG_MAJOR_VERSION == 34
    // CLOUD_GLOOM,
    { "gloom", "thick gloom",                   // terse, verbose name
      MAGENTA,                                  // colour
      { TILE_CLOUD_GLOOM },                     // tile
    },
#endif
    // CLOUD_INK,
    { "ink",  nullptr,                          // terse, verbose name
      DARKGREY,                                 // colour
      { TILE_CLOUD_INK },                       // tile
      BEAM_NONE, {},                            // beam_effect & damage
      true,                                     // opacity
    },
    // CLOUD_PETRIFY,
    { "calcifying dust",  nullptr,              // terse, verbose name
      WHITE,                                    // colour
      { TILE_CLOUD_PETRIFY, CTVARY_DUR },       // tile
      BEAM_PETRIFYING_CLOUD, {},                // beam_effect & damage
      true,                                     // opacity
    },
    // CLOUD_HOLY,
    { "blessed fire", nullptr,                  // terse, verbose name
      ETC_HOLY,                                 // colour
      { TILE_CLOUD_YELLOW_SMOKE },              // tile
      BEAM_HOLY,                                // beam_effect
      {4, 12, true},                            // base, random damage
      true,                                     // opacity
    },
    // CLOUD_MIASMA,
    { "foul pestilence", "dark miasma",         // terse, verbose name
      DARKGREY,                                 // colour
      { TILE_CLOUD_MIASMA, CTVARY_DUR },        // tile
      BEAM_MIASMA,                              // beam_effect
      { 0, 12 },                                // base, random damage
    },
    // CLOUD_MIST,
    { "thin mist", nullptr,                     // terse, verbose name
      ETC_MIST,                                 // colour
      { TILE_CLOUD_MIST, CTVARY_NONE },         // tile
    },
    // CLOUD_CHAOS,
    { "seething chaos", nullptr,                // terse, verbose name
      ETC_RANDOM,                               // colour
      { TILE_CLOUD_CHAOS, CTVARY_RANDOM },      // tile
      BEAM_CHAOS,                               // beam_effect
    },
    // CLOUD_RAIN,
    { "rain", "the rain",                       // terse, verbose name
      ETC_MIST,                                 // colour
      { TILE_CLOUD_RAIN, CTVARY_RANDOM },       // tile
      BEAM_NONE,                                // unused
      { 0, 9 },                                 // base, random damage
                                                // but only for fiery mons
    },
    // CLOUD_MUTAGENIC,
    { "mutagenic fog",  nullptr,                // terse, verbose name
      ETC_MUTAGENIC,                            // colour
      { TILE_ERROR, CTVARY_MUTAGENIC },         // tile
    },
    // CLOUD_MAGIC_TRAIL,
    { "magical condensation", nullptr,          // terse, verbose name
      ETC_MAGIC,                                // colour
      { TILE_CLOUD_MAGIC_TRAIL, CTVARY_DUR },   // tile
    },
    // CLOUD_VORTEX,
    { "whirling frost", nullptr,                // terse, verbose name
      ETC_VORTEX,                               // colour
      { TILE_ERROR, CTVARY_VORTEX },            // tile
    },
    // CLOUD_DUST,
    { "sparse dust",  nullptr,                  // terse, verbose name
      ETC_EARTH,                                // colour
      { TILE_CLOUD_DUST, CTVARY_DUR },          // tile
    },
    // CLOUD_SPECTRAL,
    { "spectral mist", nullptr,                 // terse, verbose name
      ETC_ELECTRICITY,                          // colour
      { TILE_CLOUD_SPECTRAL, CTVARY_DUR },      // tile
      BEAM_NONE,                                // beam_effect
      { 4, 15 },                                // base, random damage
    },
    // CLOUD_ACID,
    { "acidic fog", nullptr,                    // terse, verbose name
      YELLOW,                                   // colour
      { TILE_CLOUD_ACID, CTVARY_DUR },          // dur
      BEAM_ACID,                                // beam_effect
      { 8, 22, true },                          // base, random damage
    },
    // CLOUD_STORM,
    { "thunder", "a thunderstorm",              // terse, verbose name
      ETC_DARK,                                 // colour
      { TILE_CLOUD_STORM, CTVARY_RANDOM },      // tile
      BEAM_ELECTRICITY,                         // beam_effect
      { 23, 27 },
    },
    // CLOUD_MISERY,
    { "excruciating misery", nullptr,           // terse, verbose name
      ETC_INCARNADINE,                          // colour
      { TILE_CLOUD_MISERY, CTVARY_DUR },           // tile
    },
    // CLOUD_FLUFFY,
    { "white fluffiness",  nullptr,             // terse, verbose name
      WHITE,                                    // colour
      { TILE_CLOUD_WHITE_SMOKE, CTVARY_NONE },  // tile
      BEAM_NONE, {},                            // beam & damage
      true,                                     // opacity
    },
    // CLOUD_XOM_TRAIL,
    { "magical condensation", nullptr,          // terse, verbose name
      ETC_RANDOM,                               // colour
      { TILE_CLOUD_MAGIC_TRAIL, CTVARY_DUR },   // tile
      // TODO: another tile?
    },
    // CLOUD_SALT,
    { "salt",  nullptr,                           // terse, verbose name
      ETC_AIR,                                    // colour
      { TILE_CLOUD_WHITE_SMOKE, CTVARY_NONE },    // tile
      BEAM_NONE, {},                              // beam & damage
      true,                                       // opacity
    },
    // CLOUD_GOLD_DUST,
    { "golden dust",  nullptr,                    // terse, verbose name
      ETC_HOLY,                                   // colour
      { TILE_CLOUD_GOLD_DUST, CTVARY_DUR },       // tile
      BEAM_NONE, {},                              // beam & damage
      true,                                       // opacity
    },
#if TAG_MAJOR_VERSION == 34
    // CLOUD_EMBERS,
    { "smouldering embers", "embers",
        ETC_SMOKE,
        { TILE_CLOUD_BLACK_SMOKE, CTVARY_NONE },
    },
#endif
    // CLOUD_FLAME,
    { "wisps of flame", nullptr,                  // terse, verbose name
      ETC_FIRE,                                   // colour
      { TILE_CLOUD_FLAME, CTVARY_RANDOM },        // tile
    },
    // CLOUD_ALCOHOL,
    { "alcoholic mist",  nullptr,                 // terse, verbose name
      ETC_DARK,                                   // colour
      { TILE_CLOUD_DEGENERATION, CTVARY_NONE },   // tile
      BEAM_NONE, {},                              // beam & damage
      false,                                      // opacity
    },
    // CLOUD_BLASTMOTES,
    { "blastmotes", "volatile sparks",           // terse, verbose name
        ETC_SMOKE,                                // colour
      { TILE_CLOUD_BLASTMOTES, CTVARY_RANDOM },  // tile
    },
    // CLOUD_ELECTRICITY,
    { "sparks", nullptr,         // terse, verbose name
      ETC_ELECTRICITY,                                   // colour
      { TILE_CLOUD_ELECTRICITY, CTVARY_RANDOM },        // tile
    },
    // CLOUD_FAINT_MIASMA,
    { "faint pestilence", nullptr,                // terse, verbose name
      DARKGREY,                                      // colour
      { TILE_CLOUD_FAINT_MIASMA, CTVARY_RANDOM },    // tile
    },
    // CLOUD_MAGNETISED_DUST,
    { "magnetised fragments", nullptr,                  // terse, verbose name
      ETC_ELECTRICITY,                                 // colour
      { TILE_CLOUD_MAGNETISED_DUST, CTVARY_RANDOM },   // tile
    },
    // CLOUD_BATS,
    { "bats", nullptr,                                 // terse, verbose name
      ETC_DARK,                                        // colour
      { TILE_CLOUD_BATS, CTVARY_DUR },                 // tile
      BEAM_BAT_CLOUD,
      { 4, 11, true },
    },
    // CLOUD_RUST,
    { "rust", nullptr,                            // terse, verbose name
        BROWN,                                    // colour
        { TILE_CLOUD_RUST, CTVARY_DUR },          // tile
        BEAM_ACID,                                // beam_effect
        { 2, 3, false },                          // base, random damage
      },

};
COMPILE_CHECK(ARRAYSZ(clouds) == NUM_CLOUD_TYPES);

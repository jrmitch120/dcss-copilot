#pragma once

#include "tag-version.h"

// When adding:
// * Add an entry in feature-data.h for the feature.
// * edit dat/descript/features.txt and add a
//      long description if appropriate.
// * check the feat_* functions in terrain.cc and make sure
//      they return sane values for your new feature.
// * edit mapdef.cc and add a symbol to _glyph_to_feat() for the feature,
//      if you want vault maps to be able to use it directly . If you do, also
//      update docs/develop/levels/syntax.txt with the new symbol.
enum dungeon_feature_type
{
    DNGN_UNSEEN = 0,                   // must be zero
    DNGN_CLOSED_DOOR,
#if TAG_MAJOR_VERSION > 34
    DNGN_BROKEN_DOOR,
    DNGN_CLOSED_CLEAR_DOOR,
    DNGN_BROKEN_CLEAR_DOOR,
#endif
    DNGN_RUNED_DOOR,
    DNGN_SEALED_DOOR,
#if TAG_MAJOR_VERSION > 34
    DNGN_SEALED_CLEAR_DOOR,
#endif
    DNGN_ROCK_WALL,

#if TAG_MAJOR_VERSION == 34
    DNGN_LAVA            = 30,
#else
    DNGN_LAVA,
#endif
    DNGN_DEEP_WATER,

    DNGN_SHALLOW_WATER,
    DNGN_FLOOR,
    DNGN_STONE_STAIRS_DOWN_I,

    // corresponding up stairs (same order as above)
    DNGN_STONE_STAIRS_UP_I,

    DNGN_EXIT_DUNGEON,

    DNGN_SEALED_STAIRS_UP,
    DNGN_SEALED_STAIRS_DOWN,

#if TAG_MAJOR_VERSION == 34
    DNGN_SEALED_CLEAR_DOOR,
    DNGN_TOXIC_BOG,
    DNGN_MANGROVE,
#endif

    // (trimmed for the extractor test)
    DNGN_OPEN_SEA,

    NUM_FEATURES
};
COMPILE_CHECK(NUM_FEATURES <= 256);

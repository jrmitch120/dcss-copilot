#pragma once

#include "tag-version.h"

// In the default case, these translations hold:
// unseen_colour -> seen_colour
// seen_colour -> seen_em_colour
// colour -> em_colour
// So use a macro:
#define COLOURS(colour, map) colour, map, map, colour, map
// And with the default (darkgrey) map colour:
#define COLOUR_IS(colour) COLOURS(colour, DARKGREY)
// And for when colour and unseen_colour are equal:
#define COLOUR_AND_MAP(colour) COLOURS(colour, colour)
static feature_def feat_defs[] =
{

{
    // feat, name, vaultname
    DNGN_UNSEEN, "", "unseen",
    // dchar, magic_dchar
    NUM_DCHAR_TYPES, NUM_DCHAR_TYPES,
    // colour, unseen_colour, seen_colour, em_colour, seen_em_colour
    BLACK, DARKGREY, DARKGREY, BLACK, DARKGREY,
    // flags, minimap
    FFT_OPAQUE | FFT_SOLID, MF_UNSEEN,
},

{
    DNGN_CLOSED_DOOR, "closed door", "closed_door",
    DCHAR_DOOR_CLOSED, NUM_DCHAR_TYPES,
    COLOUR_IS(LIGHTGREY),
    FFT_OPAQUE | FFT_SOLID, MF_DOOR,
},

{
    DNGN_RUNED_DOOR, "runed door", "runed_door",
    DCHAR_DOOR_CLOSED, NUM_DCHAR_TYPES,
    COLOUR_AND_MAP(LIGHTBLUE),
    FFT_OPAQUE | FFT_SOLID, MF_DOOR,
},

{
    DNGN_SEALED_DOOR, "sealed door", "sealed_door",
    DCHAR_DOOR_CLOSED, NUM_DCHAR_TYPES,
    COLOUR_AND_MAP(LIGHTGREEN),
    FFT_OPAQUE | FFT_SOLID, MF_DOOR,
},

{
    DNGN_ROCK_WALL, "rock wall", "rock_wall",
    DCHAR_WALL, DCHAR_WALL_MAGIC,
    COLOUR_IS(ETC_ROCK),
    FFT_OPAQUE | FFT_WALL | FFT_SOLID, MF_WALL,
},

{
    DNGN_LAVA, "some lava", "lava",
    DCHAR_WAVY, NUM_DCHAR_TYPES,
    COLOUR_IS(RED),
    FFT_NONE, MF_LAVA,
},

{
    DNGN_DEEP_WATER, "some deep water", "deep_water",
    DCHAR_WAVY, NUM_DCHAR_TYPES,
    COLOUR_IS(BLUE),
    FFT_NONE, MF_DEEP_WATER,
},

{
    DNGN_FLOOR, "floor", "floor",
    DCHAR_FLOOR, DCHAR_FLOOR_MAGIC,
    COLOUR_IS(ETC_FLOOR),
    FFT_NONE, MF_FLOOR,
},

#define STONE_STAIRS_DOWN(num, num2)\
{\
    DNGN_STONE_STAIRS_DOWN_##num, "stone staircase leading down", "stone_stairs_down_"#num2,\
    DCHAR_STAIRS_DOWN, NUM_DCHAR_TYPES,\
    RED, RED, RED, WHITE, WHITE,\
    FFT_NONE, MF_STAIR_DOWN,\
}

STONE_STAIRS_DOWN(I, i),

#define STONE_STAIRS_UP(num, num2)\
{\
    DNGN_STONE_STAIRS_UP_##num, "stone staircase leading up", "stone_stairs_up_"#num2,\
    DCHAR_STAIRS_UP, NUM_DCHAR_TYPES,\
    GREEN, GREEN, GREEN, WHITE, WHITE,\
    FFT_NONE, MF_STAIR_UP,\
}

STONE_STAIRS_UP(I, i),

{
    DNGN_EXIT_DUNGEON, "staircase leading out of the dungeon", "exit_dungeon",
    DCHAR_STAIRS_UP, NUM_DCHAR_TYPES,
    LIGHTBLUE, GREEN, LIGHTBLUE, LIGHTBLUE, LIGHTBLUE,
    FFT_NONE, MF_STAIR_UP,
},

{
    DNGN_SEALED_STAIRS_UP, "sealed passage leading up", "sealed_stair_up",
    DCHAR_STAIRS_UP, NUM_DCHAR_TYPES,
    COLOUR_AND_MAP(LIGHTGREEN),
    FFT_NONE, MF_STAIR_UP,
},

{
    DNGN_SEALED_STAIRS_DOWN, "sealed passage leading down", "sealed_stair_down",
    DCHAR_STAIRS_DOWN, NUM_DCHAR_TYPES,
    COLOUR_AND_MAP(LIGHTGREEN),
    FFT_NONE, MF_STAIR_DOWN,
},

};

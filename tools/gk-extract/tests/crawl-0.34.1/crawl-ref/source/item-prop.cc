/**
 * @file
 * @brief Misc functions.
**/

// Trimmed from crawl 0.34.1 for the extractor test: each table and function
// keeps a few of its entries.

#include "AppHdr.h"

#include "item-prop.h"

// The DRAGON_ARMOUR macro is used to make sure all dragon armour is consistent.
// leads to static initialization races (plus 'gold' special case)
#if TAG_MAJOR_VERSION == 34
#define DRAGON_ARMOUR(id, name, ac, evp, prc, res)                          \
    { ARM_ ## id ## _DRAGON_HIDE, "removed " name " dragon hide", 0, 0, 0,  \
      SLOT_BODY_ARMOUR, SIZE_LITTLE, SIZE_GIANT, false, 0, {}, res },             \
    { ARM_ ## id ## _DRAGON_ARMOUR, name " dragon scales",  ac, evp, prc,   \
      SLOT_BODY_ARMOUR, SIZE_LITTLE, SIZE_GIANT, false, 37, {}, res }
#else
#define DRAGON_ARMOUR(id, name, ac, evp, prc, res)                          \
    { ARM_ ## id ## _DRAGON_ARMOUR, name " dragon scales",  ac, evp, prc,   \
      SLOT_BODY_ARMOUR, SIZE_LITTLE, SIZE_GIANT, false, 37, {}, res }
#endif

static int Armour_index[NUM_ARMOURS];
static const armour_def Armour_prop[] =
{
    { ARM_ANIMAL_SKIN,          "animal skin",            2,   0,     3,
        SLOT_BODY_ARMOUR, SIZE_LITTLE, SIZE_GIANT, true, 1 },
    { ARM_ROBE,                 "robe",                   2,   0,     9,
        SLOT_BODY_ARMOUR, SIZE_LITTLE, SIZE_LARGE, true, 100, {
            { SPARM_RESISTANCE,      1 },
            { SPARM_COLD_RESISTANCE, 2 },
            { SPARM_FIRE_RESISTANCE, 2 },
            { SPARM_POSITIVE_ENERGY, 2 },
            { SPARM_NORMAL,          4 },
            { SPARM_WILLPOWER,       4 },
    }},
    DRAGON_ARMOUR(STORM,       "storm",                  10, -150,  650,
        ARMF_RES_ELEC),

#undef DRAGON_ARMOUR
};

static int Weapon_index[NUM_WEAPONS];
static const weapon_def Weapon_prop[] =
{
    // Maces & Flails
    { WPN_CLUB,              "club",                5,  3, 13,
        SK_MACES_FLAILS, SIZE_LITTLE, SIZE_LITTLE,
        DAMV_CRUSHING, 10, 0, 5, CLUB_BRANDS },
#if TAG_MAJOR_VERSION == 34
    { WPN_SPIKED_FLAIL,      "old spiked flail",    5,  3, 13,
        SK_MACES_FLAILS, SIZE_LITTLE, SIZE_LITTLE,
        DAMV_CRUSHING, 0, 0, 0, {} },
#endif
    { WPN_MACE,              "mace",                8,  3, 14,
        SK_MACES_FLAILS, SIZE_LITTLE, SIZE_LITTLE,
        DAMV_CRUSHING, 9, 0, 30, M_AND_F_BRANDS },
};

static int Staff_index[NUM_STAVES];
static const staff_def Staff_prop[] =
{
#if TAG_MAJOR_VERSION == 34
    { STAFF_WIZARDRY,    "wizardry" },
#endif
    { STAFF_FIRE,        "fire",        SK_FIRE_MAGIC,
        63, ac_type::normal, BEAM_FIRE },
};

static int Missile_index[NUM_MISSILES];
static const missile_def Missile_prop[] =
{
    { MI_DART,          "dart",          0, 12, 3  },
    { MI_STONE,         "stone",         2, 8,  1  },
};

string talisman_type_name(int type)
{
    switch (type)
    {
    case TALISMAN_QUILL:    return "quill talisman";
    case TALISMAN_INKWELL:  return "inkwell talisman";
    default: return "buggy talisman";
    }
}

/**
 * @file
 * @brief Monsters' starting equipment.
 **/

// Trimmed from crawl 0.34.1 for the extractor test: each table keeps a few of
// its entries, and the code around them is left out.

#include "AppHdr.h"

#include "mon-gear.h"

int make_mons_weapon(monster_type type, int level, bool melee_only)
{
    rng::subgenerator item_rng;

    static const weapon_list GOBLIN_WEAPONS =
    {   { WPN_DAGGER,           1 },
        { WPN_CLUB,             1 }};
    static const weapon_list ORC_KNIGHT_WEAPONS =
    {   { WPN_GREAT_SWORD,      8 },
        { WPN_LONG_SWORD,       8 },
        { WPN_BATTLEAXE,        8 },
        { WPN_WAR_AXE,          8 }, };
    static const mon_weapon_spec ORC_KNIGHT_WSPEC =
        { ORC_KNIGHT_WEAPONS, {4, 1, 3} };

    static const map<monster_type, mon_weapon_spec> primary_weapon_specs = {
        { MONS_GOBLIN,                  { GOBLIN_WEAPONS } },
        { MONS_ORC_KNIGHT,              ORC_KNIGHT_WSPEC },
        { MONS_GOBLIN_RIDER, { { { WPN_SPEAR, 1 } } } },
        { MONS_CENTAUR,
            { { { WPN_SHORTBOW,         1 },
                { WPN_ORCBOW,           1 },
        } } },
        { MONS_ORC_WIZARD,      { { { WPN_DAGGER, 1 } } } },
    };

    static const weapon_list ORC_KNIGHT_BOWS =
    {   { WPN_ARBALEST,                 1 },
        { NUM_WEAPONS,                  8 }, }; // 1/9 chance of ranged weapon


    static const map<monster_type, mon_weapon_spec> secondary_weapon_specs = {
        { MONS_ORC_KNIGHT,              { ORC_KNIGHT_BOWS } },
        { MONS_NAGA_SHARPSHOOTER,
            { { { WPN_SHORTBOW,         4 },
                { WPN_ARBALEST,         2 },
                { WPN_LONGBOW,          1 }
        } } },
    };

    // (trimmed for the extractor test)
}

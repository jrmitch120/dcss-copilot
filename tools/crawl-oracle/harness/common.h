/*
 * What the crawl oracle's harnesses share: crawl set up without a dungeon, a human player dressed
 * at random until their AC, EV and SH are exact on the display, and a monster's removal.
 *
 * Included by each harness, which is compiled against a crawl build's objects in place of
 * main.o (oracle.mk).
 */

#pragma once

#include "AppHdr.h"

#include "fake-main.hpp"

#include "areas.h"
#include "coordit.h"

#include <cstdio>
#include <cstdlib>
#include <map>
#include <string>
#include <vector>

#include "item-prop.h"
#include "items.h"
#include "item-name.h"
#include "item-status-flag-type.h"
#include "los.h"
#include "mapdef.h"
#include "message.h"
#include "mon-util.h"
#include "monster.h"
#include "player.h"
#include "player-equip.h"
#include "skills.h"
#include "version.h"

// As crawl's util/monster: what crawl's startup does before a game, without the UI.
static void _initialise_crawl()
{
    init_monsters();
    init_properties();
    init_item_name_cache();
    init_zap_index();
    init_spell_descs();
    init_monster_symbols();
    init_mon_name_cache();
    init_spell_name_cache();
    init_mons_spells();
    init_element_colours();
    init_show_table();

    dgn_reset_level();
    for (rectangle_iterator ri(0); ri; ++ri)
        env.grid(*ri) = DNGN_FLOOR;

    you.species = SP_HUMAN;
    // Enough that no attack ends the run.
    you.hp = you.hp_max = 500;
    you.set_position(coord_def(GXM / 2, GYM / 2));
    los_changed();
}

// The player's defences for one case, as the game displays them and as crawl's melee code uses
// them. Only defences whose displayed value is exact are kept: the engine only sees the display.
struct defences
{
    int ac;
    int ev;
    int sh;
};

static void _take_off(equipment_slot slot)
{
    if (item_def *item = you.equipment.get_first_slot_item(slot))
    {
        unequip_item(*item, false);
        item->clear();
    }
}

static void _wear(int inv_slot, equipment_slot slot, armour_type type, int plus)
{
    item_def &item = you.inv[inv_slot];
    item.base_type = OBJ_ARMOUR;
    item.sub_type = type;
    item.plus = plus;
    item.quantity = 1;
    item.link = inv_slot;
    item.pos = ITEM_IN_INVENTORY;
    item.flags |= ISFLAG_IDENTIFIED;
    equip_item(slot, inv_slot, false, true);
}

static const vector<armour_type> BODY_ARMOURS = {
    ARM_ROBE, ARM_LEATHER_ARMOUR, ARM_RING_MAIL, ARM_SCALE_MAIL, ARM_CHAIN_MAIL,
    ARM_PLATE_ARMOUR, ARM_CRYSTAL_PLATE_ARMOUR,
};

static const vector<armour_type> SHIELDS = { ARM_BUCKLER, ARM_KITE_SHIELD, ARM_TOWER_SHIELD };

static double _random_skill()
{
    return random2(271) / 10.0;
}

// Half the time a shield, rerolled with Dexterity and Shields skill until the SH crawl rolls
// with is twice a whole displayed SH. Returns false if none was found.
static bool _random_shield()
{
    _take_off(SLOT_OFFHAND);
    set_skill_level(SK_SHIELDS, 0, true);
    you.base_stats[STAT_DEX] = 3 + random2(25);
    if (coinflip())
        return true;
    for (int attempt = 0; attempt < 100000; ++attempt)
    {
        _take_off(SLOT_OFFHAND);
        you.base_stats[STAT_DEX] = 3 + random2(25);
        set_skill_level(SK_SHIELDS, _random_skill(), true);
        _wear(1, SLOT_OFFHAND, SHIELDS[random2(SHIELDS.size())], random2(7) - 1);
        // player_shield_class rounds to whole SH before the display halves it.
        if (player_shield_class(100, false) % 200 == 0)
            return true;
    }
    return false;
}

// Body armour most of the time, Strength and Dodging skill, rerolled until AC and EV are whole.
// Armour skill stays 0, which keeps AC whole. Returns false if none was found.
static bool _random_body()
{
    set_skill_level(SK_ARMOUR, 0, true);
    for (int attempt = 0; attempt < 100000; ++attempt)
    {
        _take_off(SLOT_BODY_ARMOUR);
        you.base_stats[STAT_STR] = 3 + random2(25);
        set_skill_level(SK_DODGING, _random_skill(), true);
        if (x_chance_in_y(3, 4))
        {
            _wear(0, SLOT_BODY_ARMOUR, BODY_ARMOURS[random2(BODY_ARMOURS.size())],
                  random2(7) - 1);
        }
        const int ev = you.evasion_scaled(100);
        if (you.armour_class_scaled(100) % 100 == 0 && ev % 100 == 0 && ev >= 0)
            return true;
    }
    return false;
}

// Dresses the player at random until their AC, EV and SH are exact on the display, so the
// displayed values, which are all the engine sees, are the ones crawl rolls with.
static defences _random_player()
{
    for (int attempt = 0; attempt < 1000; ++attempt)
    {
        if (_random_shield() && _random_body())
        {
            return { you.armour_class_scaled(1), you.evasion_scaled(1),
                     player_displayed_shield_class() };
        }
    }
    fprintf(stderr, "no player with exact AC, EV and SH found\n");
    exit(1);
}

// Takes a monster off the map without its death and what dying sets off, as util/monster does.
static void _remove(monster &mon)
{
    const coord_def pos = mon.pos();
    mon.reset();
    env.mgrid(pos) = NON_MONSTER;
    // Its halo or umbra, if it had one, no longer reaches the player.
    invalidate_agrid(true);
}

static string _histogram(const map<int, int> &counts)
{
    string out = "{";
    for (const auto &entry : counts)
    {
        if (out.size() > 1)
            out += ",";
        out += make_stringf("\"%d\":%d", entry.first, entry.second);
    }
    return out + "}";
}


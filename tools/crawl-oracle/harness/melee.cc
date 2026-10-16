/*
 * The crawl oracle's harness: runs crawl's own melee code for randomised monster attackers and
 * player defenders, and prints what came out, one JSON object per line, for `crawl-oracle` to
 * diff against the engine's ported formulas.
 *
 * It links against a crawl build's objects in place of main.o, as crawl's `util/monster` does,
 * and is compiled with -fno-access-control so it can call the protected steps of
 * melee_attack::attack (test_hit, attack_shield_blocked, calc_damage) one at a time, without the
 * messages, brands and death that follow a hit. Built by oracle.mk and run by tools/crawl-oracle/oracle.sh.
 *
 * Usage: melee <seed> <cases> <trials>
 */

#include "AppHdr.h"

#include "fake-main.hpp"

#include "areas.h"

#include <cstdio>
#include <cstdlib>
#include <map>
#include <string>
#include <vector>

#include "item-prop.h"
#include "items.h"
#include "item-name.h"
#include "los.h"
#include "mapdef.h"
#include "melee-attack.h"
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

static const coord_def ATTACKER_POS(GXM / 2 + 1, GYM / 2);

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
    mon.reset();
    env.mgrid(ATTACKER_POS) = NON_MONSTER;
    // Its halo or umbra, if it had one, no longer reaches the player.
    invalidate_agrid(true);
}

// A random monster with a damaging melee attack, placed next to the player, awake, with no
// items and no statuses: the attacker the engine's melee port models.
static monster *_random_monster(int &attack_number)
{
    while (true)
    {
        const monster_type type = static_cast<monster_type>(random2(NUM_MONSTERS));
        if (invalid_monster_type(type) || mons_class_is_zombified(type)
            || mons_is_unique(type) || mons_class_flag(type, M_CANT_SPAWN)
            || mons_class_is_animated_object(type))
        {
            continue;
        }

        mons_spec spec(type);
        monster *mon = dgn_place_monster(spec, ATTACKER_POS, true, true, false);
        if (!mon)
            continue;
        mon->destroy_inventory();
        while (!mon->enchantments.empty())
            mon->del_ench(mon->enchantments.begin()->first, true, false);
        mon->behaviour = BEH_SEEK;
        mon->foe = MHITYOU;

        vector<int> attacks;
        for (int i = 0; i < MAX_NUM_ATTACKS; ++i)
        {
            const mon_attack_def attk = mons_attack_spec(*mon, i, true);
            if (attk.type != AT_NONE && attk.damage > 0)
                attacks.push_back(i);
        }
        // A halo backlights the player and an umbra hides them, which changes the monster's
        // to-hit; the engine doesn't model either.
        if (attacks.empty() || !mon->visible_to(&you) || !you.visible_to(mon)
            || you.backlit(false) || you.umbra())
        {
            _remove(*mon);
            continue;
        }
        attack_number = attacks[random2(attacks.size())];
        return mon;
    }
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

static void _run_case(int index, int trials)
{
    const defences def = _random_player();
    int attack_number = 0;
    monster *mon = _random_monster(attack_number);

    melee_attack attk(mon, &you, attack_number, attack_number);
    const bool crush = attk.attk_flavour == AF_CRUSH;

    map<int, int> to_hit, damage;
    int hits = 0, blocks = 0;
    for (int t = 0; t < trials; ++t)
    {
        // melee_attack::attack, step by step, without the effects of a hit.
        you.shield_blocks = 0;
        attk.to_hit = attk.calc_to_hit(true);
        ++to_hit[attk.to_hit];
        const int ev = you.evasion(false, mon);
        const int margin = attk.test_hit(attk.to_hit, ev, true);
        const bool blocked = attk.attack_shield_blocked();
        if (margin >= 0)
            ++hits;
        if (blocked)
            ++blocks;
        else if (margin >= 0)
            ++damage[attk.calc_damage()];
    }

    printf("{\"case\":%d,\"monster\":\"%s\",\"hd\":%d,\"fighter\":%s,"
           "\"attack\":{\"damage\":%d,\"crush\":%s},"
           "\"player\":{\"ac\":%d,\"ev\":%d,\"sh\":%d},"
           "\"trials\":%d,\"to_hit\":%s,\"hits\":%d,\"blocks\":%d,\"damage\":%s}\n",
           index, mon->name(DESC_PLAIN, true).c_str(), mon->get_hit_dice(),
           mon->is_fighter() ? "true" : "false", attk.attk_damage, crush ? "true" : "false",
           def.ac, def.ev, def.sh, trials, _histogram(to_hit).c_str(), hits, blocks,
           _histogram(damage).c_str());

    _remove(*mon);
}

int main(int argc, char *argv[])
{
    if (argc != 4)
    {
        fprintf(stderr, "usage: %s <seed> <cases> <trials>\n", argv[0]);
        return 2;
    }
    crawl_state.test = true;
    msg::suppress quiet;
    rng::seed(strtoull(argv[1], nullptr, 10));
    _initialise_crawl();

    printf("{\"crawl\":\"%s\",\"seed\":%s}\n", Version::Long, argv[1]);
    const int cases = atoi(argv[2]);
    const int trials = atoi(argv[3]);
    for (int i = 0; i < cases; ++i)
        _run_case(i, trials);
    return 0;
}

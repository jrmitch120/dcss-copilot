/*
 * What the crawl oracle's harnesses share: their arguments and seeding, crawl set up without a
 * dungeon, a human player dressed at random until their AC, EV and SH are exact on the display,
 * and a monster's removal.
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

#include "beam.h"
#include "item-prop.h"
#include "items.h"
#include "item-name.h"
#include "item-status-flag-type.h"
#include "los.h"
#include "mapdef.h"
#include "message.h"
#include "mon-cast.h"
#include "mon-util.h"
#include "monster.h"
#include "ouch.h"
#include "player.h"
#include "player-equip.h"
#include "skills.h"
#include "version.h"

// What a harness is asked to run, from its arguments:
//
//   <seed> <cases> <trials> [<ported spells>] [<shard>/<count>]
//
// It draws only its shard's cases (_in_shard), so `shards` harnesses, one per shard, draw a run's
// cases between them. Without a shard it draws them all.
struct oracle_run
{
    uint64_t seed;
    int cases;
    int trials;
    // The file of ported spells, for the harnesses that take one.
    const char *ported = nullptr;
    int shard = 0;
    int shards = 1;
};

// Reads a harness's arguments, `ported` saying whether it takes a file of ported spells, or exits
// with its usage.
static oracle_run _read_run(int argc, char *argv[], bool ported)
{
    const int required = ported ? 5 : 4;
    oracle_run run;
    bool read = argc == required || argc == required + 1;
    if (read)
    {
        run.seed = strtoull(argv[1], nullptr, 10);
        run.cases = atoi(argv[2]);
        run.trials = atoi(argv[3]);
        if (ported)
            run.ported = argv[4];
        if (argc > required)
        {
            read = sscanf(argv[required], "%d/%d", &run.shard, &run.shards) == 2
                   && run.shards > 0 && run.shard >= 0 && run.shard < run.shards;
        }
    }
    if (!read)
    {
        fprintf(stderr, "usage: %s <seed> <cases> <trials>%s [<shard>/<count>]\n", argv[0],
                ported ? " <ported spells>" : "");
        exit(2);
    }
    return run;
}

// Whether case `index` falls in the run's shard, for a harness whose cases take turns at `kinds`
// kinds. Each kind's cases are dealt round the shards in turn, so every shard gets its share of
// every kind: dealing the cases themselves would hand a shard only the kinds whose turn lines up
// with it, and some kinds take far longer than others.
static bool _in_shard(const oracle_run &run, int index, int kinds)
{
    return index / kinds % run.shards == run.shard;
}

// The dump's header line, which the oracle reads first.
static void _print_header(const oracle_run &run)
{
    printf("{\"crawl\":\"%s\",\"seed\":%llu,\"shard\":{\"index\":%d,\"count\":%d}}\n",
           Version::Long, static_cast<unsigned long long>(run.seed), run.shard, run.shards);
}

// The player as crawl was set up, before any case dressed, hurt or moved them.
static player fresh_player;

// A level of floor with no monsters, items or clouds, the player at its middle.
static void _reset_level()
{
    dgn_reset_level();
    for (rectangle_iterator ri(0); ri; ++ri)
        env.grid(*ri) = DNGN_FLOOR;
    you.set_position(coord_def(GXM / 2, GYM / 2));
    los_changed();
}

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

    you.species = SP_HUMAN;
    // Enough that no attack ends the run.
    you.hp = you.hp_max = 500;
    _reset_level();
    fresh_player = you;
}

// Starts case `index` with the player as crawl was set up, on a fresh level, and crawl's RNG
// seeded from the run's seed and the index alone, so the case comes out the same whichever shard
// draws it and whatever the cases before it left behind: the gear an earlier case dressed the
// player in, or a missile on the floor. The seed is mixed as splitmix64 mixes, so neighbouring
// cases draw unrelated streams.
static void _start_case(const oracle_run &run, int index)
{
    you = fresh_player;
    _reset_level();
    uint64_t z = run.seed + 0x9e3779b97f4a7c15ULL * (static_cast<uint64_t>(index) + 1);
    z = (z ^ (z >> 30)) * 0xbf58476d1ce4e5b9ULL;
    z = (z ^ (z >> 27)) * 0x94d049bb133111ebULL;
    rng::seed(z ^ (z >> 31));
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

// How the light falls on the player as monsters aim at them, as the engine takes it: backlit
// (player::backlit(false)), else in an umbra (player::umbra()), else neither.
static const char *_lighting()
{
    if (you.backlit(false))
        return "backlit";
    if (you.umbra())
        return "umbra";
    return "normal";
}

// A third of the time puts the player under a corona, which backlights them, and a third, when
// `shadow`, gives them the Foul Shadow mutation, an umbra of their own; otherwise leaves the
// light as the level and its monsters make it. A monster's halo or umbra reaches them on top.
static void _random_light(bool shadow)
{
    const int roll = random2(3);
    if (roll == 0)
        you.duration[DUR_CORONA] = 1000;
    else if (roll == 1 && shadow)
        you.mutation[MUT_FOUL_SHADOW] = 1;
    invalidate_agrid(true);
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

// The player, unhurt and unaffected, as the next action starts: what hit them may have
// poisoned, corroded, blinded, burnt or mutated them, laid a bane on them, which changes the
// monsters they meet, or left them liable to teleport, which stops a sojourning bolt being cast
// at them again.
static void _restore_player()
{
    you.hp = you.hp_max;
    for (int &duration : you.duration)
        duration = 0;
    you.banes.init(0);
    you.mutation.init(0);
    you.temp_mutation.init(0);
    you.props.erase(CORROSION_KEY);
    you.props.erase(TELEPORTITIS_SOURCE);
    you.redraw_armour_class = true;
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

// What many beams a monster cast or zapped at the player came to.
struct beam_tally
{
    // The beam's accuracy, AUTOMATIC_HIT when it never misses.
    int hit = 0;
    int hits = 0;
    int blocks = 0;
    map<int, int> damage;
};

// Casts `spell` at the player `trials` times, or zaps it from a wand when `evoke`, each set up by
// crawl's setup_mons_cast and rolled as bolt::affect_player rolls it: bolt::misses_player for the
// shield and evasion, then the damage dice, bolt::apply_AC and check_your_resists. The beam's
// flight (bolt::fire) is skipped, but not what it reads from the caster first.
static beam_tally _cast_at_player(monster &mon, spell_type spell, bool evoke, int trials)
{
    beam_tally tally;
    for (int t = 0; t < trials; ++t)
    {
        bolt beam;
        beam.target = you.pos();
        setup_mons_cast(&mon, beam, spell, evoke);
        beam.source = mon.pos();
        // As bolt::fire aims it; a block ends the beam where its ray has got to.
        find_ray(beam.source, beam.target, beam.ray, opc_solid_see);
        beam.chose_ray = true;
        // As bolt::initialise_fire reads them from the caster: its nightvision, which the light on
        // the player needs, and whether it sees the invisible.
        beam.precalc_agent_properties();
        tally.hit = beam.hit;

        you.shield_blocks = 0;
        const bool missed = beam.misses_player();
        if (you.shield_blocks > 0)
            ++tally.blocks;
        if (missed)
            continue;
        ++tally.hits;
        // bolt::affect_player's damage, without what the hit sets off.
        int taken = beam.apply_AC(&you, beam.damage.roll());
        taken = check_your_resists(max(0, taken), beam.flavour, "", &beam, false);
        ++tally.damage[taken];
    }
    return tally;
}

// A beam tally's fields, as the oracle reads them.
static string _beam_json(const beam_tally &tally, int trials)
{
    return make_stringf("\"to_hit\":%s,\"trials\":%d,\"hits\":%d,\"blocks\":%d,\"damage\":%s",
                        tally.hit == AUTOMATIC_HIT ? "null"
                                                   : make_stringf("%d", tally.hit).c_str(),
                        trials, tally.hits, tally.blocks, _histogram(tally.damage).c_str());
}

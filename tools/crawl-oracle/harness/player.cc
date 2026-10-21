/*
 * The crawl oracle's player harness: runs crawl's own code for the player attacking randomised
 * monsters, and prints what came out, one JSON object per line, for `crawl-oracle` to diff
 * against the engine's port of the player's attacks.
 *
 * Four kinds of case take turns:
 *
 * - melee: a human player of random Strength, Dexterity and skills, unarmed or wielding a random
 *   melee weapon with a random enchantment and a brand of none, heavy or speed, attacks a random
 *   monster beside them many times, rolled as melee_attack::attack rolls it: attack::calc_to_hit,
 *   attack::test_hit against monster::evasion, and for a hit attack::calc_damage. Each time it
 *   also rolls how long the attack takes (player::attack_delay).
 * - shot: such a player, wielding a random launcher with a random enchantment and brand of none,
 *   heavy or speed, or throwing a javelin, a boomerang or a stone, shoots at a random monster a
 *   few squares away many times, rolled as ranged_attack::attack rolls it, with whether a thrown
 *   missile breaks as it lands, which makes it hit harder, and how long the shot takes.
 * - beam: one of the spells the engine ports as the player casts them, at a random power up to
 *   its cap, fired at a random monster a few squares away many times, each set up by crawl's
 *   zappy and rolled by its own bolt::affect_monster: the hit points the monster lost, 0 for a
 *   miss. It prints the beam's accuracy, and the power a wand of the spell is zapped with at a
 *   random Evocations skill (wand_power).
 * - hp: the hit points crawl gives a random kind of monster as it is made, many times
 *   (hit_points, as define_monster calls it for a kind it gives no hit points of its own).
 *
 * The monster has no items and no statuses, is awake, and is of a kind whose AC and EV are its
 * kind's own: not a draconian of a job, which adds its colour's, a derived undead, which takes
 * its base's, an animated object or an ancestor, nor a monster made with its own ghost. Nor does
 * it stand in a halo or umbra, which change the player's aim. Each skill
 * is printed as you.skill(sk, 179200), a fine enough scale that every scale crawl reads a skill at
 * (7, 10, 100 and 256) divides it.
 *
 * It links against a crawl build's objects in place of main.o (oracle.mk). Compiled with
 * -fno-access-control.
 *
 * Usage: player <seed> <cases> <trials> <player zaps> [<shard>/<count>]
 *   <player zaps> is a file of the spell_type values the engine ports as the player casts them,
 *   one per line, as `crawl-oracle player-zaps` prints them. Each case is seeded from <seed> and
 *   its index; with a shard, only its cases are drawn (common.h).
 */

#include "common.h"

#include <fstream>
#include <set>

#include "evoke.h"
#include "melee-attack.h"
#include "ranged-attack.h"
#include "spl-util.h"
#include "spl-zap.h"

// The kinds of case, which take turns: melee, shot, beam and hp.
static const int KINDS = 4;

// The scale skills are printed at: every scale crawl reads a skill at divides it.
static const int SKILL_SCALE = 179200;

// The thrown missiles a case throws: those that deal damage of their own and a human throws.
static const vector<missile_type> THROWN = { MI_JAVELIN, MI_BOOMERANG, MI_STONE };

// The brands a player's weapon is given: none, and those whose effect the engine ports, on its
// damage and delay.
static const vector<brand_type> HEFTS = { SPWPN_NORMAL, SPWPN_HEAVY, SPWPN_SPEED };

// Whether a thrown missile breaks as it lands: throw.cc's static _thrown_object_destroyed, which
// the harness can't include beside fake-main.hpp, made of the same calls.
static bool _thrown_object_destroyed(const item_def &item)
{
    if (item.base_type != OBJ_MISSILES)
        return false;
    if (ammo_always_destroyed(item))
        return true;
    if (ammo_never_destroyed(item))
        return false;
    return one_chance_in(ammo_destroy_chance(item));
}

static set<spell_type> _read_zaps(const char *path)
{
    set<spell_type> zaps;
    ifstream file(path);
    int value;
    while (file >> value)
        zaps.insert(static_cast<spell_type>(value));
    if (zaps.empty())
    {
        fprintf(stderr, "no player zaps in %s\n", path);
        exit(1);
    }
    return zaps;
}

// Sets every skill a case reads at random, from none to 27, and Strength and Dexterity from 3
// to 27.
static void _random_player_skills()
{
    for (skill_type sk : { SK_FIGHTING, SK_SHORT_BLADES, SK_LONG_BLADES, SK_AXES,
                           SK_MACES_FLAILS, SK_POLEARMS, SK_STAVES, SK_RANGED_WEAPONS,
                           SK_THROWING, SK_UNARMED_COMBAT, SK_EVOCATIONS })
    {
        set_skill_level(sk, random2(2701) / 100.0, true);
    }
    you.base_stats[STAT_STR] = 3 + random2(25);
    you.base_stats[STAT_DEX] = 3 + random2(25);
}

// The player's Strength, Dexterity and Fighting, as the oracle reads them.
static string _striker_json()
{
    return make_stringf("{\"strength\":%d,\"dexterity\":%d,\"fighting\":%d}", you.strength(),
                        you.dex(), you.skill(SK_FIGHTING, SKILL_SCALE));
}

// Puts an item of `base_type` and `sub_type` in inventory slot 0 and wields it.
static item_def &_wield(object_class_type base_type, int sub_type, int plus, brand_type brand)
{
    item_def &item = you.inv[0];
    item.clear();
    item.base_type = base_type;
    item.sub_type = sub_type;
    item.plus = plus;
    item.brand = brand;
    item.quantity = 1;
    item.link = 0;
    item.pos = ITEM_IN_INVENTORY;
    item.flags |= ISFLAG_IDENTIFIED;
    equip_item(SLOT_WEAPON, 0, false, true);
    return item;
}

// A random kind of melee weapon a human wields: one crawl still makes, neither a launcher nor
// the retired blowgun, and not too big for them.
static weapon_type _random_melee_weapon()
{
    while (true)
    {
        const auto type = static_cast<weapon_type>(random2(NUM_WEAPONS));
        item_def item;
        item.base_type = OBJ_WEAPONS;
        item.sub_type = type;
        if (!item_type_removed(OBJ_WEAPONS, type) && !is_ranged_weapon_type(type)
            && item_attack_skill(OBJ_WEAPONS, type) != SK_THROWING && can_equip_item(item))
        {
            return type;
        }
    }
}

// A random kind of launcher a human wields, of those crawl still makes.
static weapon_type _random_launcher()
{
    while (true)
    {
        const auto type = static_cast<weapon_type>(random2(NUM_WEAPONS));
        item_def item;
        item.base_type = OBJ_WEAPONS;
        item.sub_type = type;
        if (!item_type_removed(OBJ_WEAPONS, type) && is_ranged_weapon_type(type)
            && can_equip_item(item))
        {
            return type;
        }
    }
}

// A random kind of monster `distance` squares east of the player, awake, with no items and no
// statuses, its hit points too many for any attack to kill it, of a kind whose AC and EV are its
// kind's own.
static monster *_random_target(int distance)
{
    const coord_def pos(GXM / 2 + distance, GYM / 2);
    while (true)
    {
        const monster_type type = static_cast<monster_type>(random2(NUM_MONSTERS));
        if (invalid_monster_type(type) || mons_class_is_zombified(type)
            || mons_class_flag(type, M_CANT_SPAWN) || mons_class_is_animated_object(type)
            || mons_is_draconian_job(type) || mons_is_hepliaklqana_ancestor(type)
            || mons_is_ghost_demon(type) || type == MONS_ARMOUR_ECHO)
        {
            continue;
        }
        mons_spec spec(type);
        monster *mon = dgn_place_monster(spec, pos, true, true, false);
        if (!mon)
            continue;
        mon->destroy_inventory();
        while (!mon->enchantments.empty())
            mon->del_ench(mon->enchantments.begin()->first, true, false);
        mon->behaviour = BEH_SEEK;
        mon->foe = MHITYOU;
        mon->max_hit_points = mon->hit_points = 10000;
        // A halo or umbra on it changes the player's aim, which the engine doesn't model.
        if (mon->armour_class() != get_monster_data(type)->AC
            || mon->evasion() != get_monster_data(type)->ev || !mon->visible_to(&you)
            || !you.can_see(*mon) || mon->backlit(false) || mon->umbra())
        {
            _remove(*mon);
            continue;
        }
        return mon;
    }
}

// The monster as an attack left it: unhurt, without the statuses an attack may have put on it.
static void _restore(monster &mon)
{
    mon.hit_points = mon.max_hit_points;
    while (!mon.enchantments.empty())
        mon.del_ench(mon.enchantments.begin()->first, true, false);
    mon.behaviour = BEH_SEEK;
    mon.foe = MHITYOU;
}

// A weapon of the player's as the oracle reads it: its sub-type, enchantment and brand.
static string _player_weapon_json(const item_def &item)
{
    return make_stringf("{\"weapon\":\"%d:%d\",\"plus\":%d,\"brand\":\"%s\"}", item.base_type,
                        item.sub_type, item.plus, _brand_id(get_weapon_brand(item)));
}

static void _melee_case(int index, int trials)
{
    _random_player_skills();
    const bool unarmed = one_chance_in(4);
    string weapon = "null";
    if (!unarmed)
    {
        const item_def &item = _wield(OBJ_WEAPONS, _random_melee_weapon(),
                                      random_range(MIN_PLUS, MAX_PLUS),
                                      HEFTS[random2(HEFTS.size())]);
        weapon = _player_weapon_json(item);
    }
    const skill_type skill =
        unarmed ? SK_UNARMED_COMBAT : item_attack_skill(*you.weapon());
    monster *mon = _random_target(1);

    melee_attack attk(&you, mon);
    map<int, int> to_hit, damage, delay;
    int hits = 0;
    for (int t = 0; t < trials; ++t)
    {
        attk.to_hit = attk.calc_to_hit(true);
        ++to_hit[attk.to_hit];
        const int ev = mon->evasion(false, &you);
        if (attk.test_hit(attk.to_hit, ev, false) >= 0)
        {
            ++hits;
            ++damage[attk.calc_damage()];
        }
        ++delay[you.attack_delay().roll()];
        _restore(*mon);
    }

    printf("{\"case\":%d,\"kind\":\"player-melee\",\"striker\":%s,\"weapon\":%s,\"skill\":%d,"
           "\"monster\":%d,\"name\":\"%s\",\"trials\":%d,\"to_hit\":%s,\"hits\":%d,"
           "\"damage\":%s,\"delay\":%s}\n",
           index, _striker_json().c_str(), weapon.c_str(), you.skill(skill, SKILL_SCALE),
           mon->type, mon->name(DESC_PLAIN, true).c_str(), trials, _histogram(to_hit).c_str(),
           hits, _histogram(damage).c_str(), _histogram(delay).c_str());
    _remove(*mon);
}

static void _shot_case(int index, int trials)
{
    _random_player_skills();
    const bool launcher = coinflip();
    item_def thrown;
    const item_def *item = nullptr;
    if (launcher)
    {
        item = &_wield(OBJ_WEAPONS, _random_launcher(), random_range(MIN_PLUS, MAX_PLUS),
                       HEFTS[random2(HEFTS.size())]);
    }
    else
    {
        thrown.base_type = OBJ_MISSILES;
        thrown.sub_type = THROWN[random2(THROWN.size())];
        thrown.quantity = 1;
        thrown.flags |= ISFLAG_IDENTIFIED;
        item = &thrown;
    }
    const skill_type skill = launcher ? SK_RANGED_WEAPONS : SK_THROWING;
    monster *mon = _random_target(2 + random2(5));

    ranged_attack attk(&you, mon, item);
    map<int, int> to_hit, damage, delay;
    int hits = 0, broke = 0;
    for (int t = 0; t < trials; ++t)
    {
        // As _player_shoot decides it before the missile flies.
        attk.will_mulch = _thrown_object_destroyed(*item);
        if (attk.will_mulch)
            ++broke;
        attk.to_hit = attk.calc_to_hit(true);
        ++to_hit[attk.to_hit];
        const int ev = mon->evasion(false, &you) + mon->missile_repulsion();
        if (attk.test_hit(attk.to_hit, ev, false) >= 0)
        {
            ++hits;
            ++damage[attk.calc_damage()];
        }
        ++delay[(launcher ? you.attack_delay() : you.attack_delay(item)).roll()];
        _restore(*mon);
    }

    printf("{\"case\":%d,\"kind\":\"player-shot\",\"striker\":%s,\"weapon\":\"%d:%d\",\"plus\":%d,"
           "\"brand\":\"%s\",\"skill\":%d,\"monster\":%d,\"name\":\"%s\",\"trials\":%d,"
           "\"to_hit\":%s,\"hits\":%d,\"damage\":%s,\"broke\":%d,\"delay\":%s}\n",
           index, _striker_json().c_str(), item->base_type, item->sub_type,
           launcher ? item->plus : 0,
           launcher ? _brand_id(get_weapon_brand(*item)) : "SPWPN_NORMAL",
           you.skill(skill, SKILL_SCALE), mon->type, mon->name(DESC_PLAIN, true).c_str(),
           trials, _histogram(to_hit).c_str(), hits, _histogram(damage).c_str(), broke,
           _histogram(delay).c_str());
    _remove(*mon);
}

static void _beam_case(int index, int trials, const set<spell_type> &zaps)
{
    auto it = zaps.begin();
    advance(it, random2(zaps.size()));
    const spell_type spell = *it;
    const zap_type zap = spell_to_zap(spell);
    const int power = random2(spell_power_cap(spell) + 1);
    monster *mon = _random_target(2 + random2(4));

    map<int, int> damage;
    int hit = 0;
    for (int t = 0; t < trials; ++t)
    {
        bolt beam;
        beam.thrower = KILL_YOU;
        beam.source_id = MID_PLAYER;
        beam.attitude = ATT_FRIENDLY;
        beam.source = you.pos();
        beam.target = mon->pos();
        beam.origin_spell = spell;
        zappy(zap, power, false, beam);
        // As bolt::fire aims it, so the beam has a square it has got to.
        find_ray(beam.source, beam.target, beam.ray, opc_solid_see);
        beam.chose_ray = true;
        // An explosion strikes as it bursts.
        beam.in_explosion_phase = beam.is_explosion;
        hit = beam.hit;
        const int before = mon->hit_points;
        beam.affect_monster(mon);
        ++damage[before - mon->hit_points];
        _restore(*mon);
    }

    // A wand of the spell, at a random Evocations skill.
    set_skill_level(SK_EVOCATIONS, random2(2701) / 100.0, true);

    printf("{\"case\":%d,\"kind\":\"player-beam\",\"spell\":%d,\"spell_name\":\"%s\",\"power\":%d,"
           "\"monster\":%d,\"name\":\"%s\",\"to_hit\":%s,\"trials\":%d,\"damage\":%s,"
           "\"evocations\":%d,\"wand_power\":%d}\n",
           index, spell, spell_title(spell), power, mon->type,
           mon->name(DESC_PLAIN, true).c_str(),
           hit == AUTOMATIC_HIT ? "null" : make_stringf("%d", hit).c_str(), trials,
           _histogram(damage).c_str(), you.skill(SK_EVOCATIONS, SKILL_SCALE),
           wand_power(spell));
    _remove(*mon);
}

static void _hp_case(int index, int trials)
{
    while (true)
    {
        const monster_type type = static_cast<monster_type>(random2(NUM_MONSTERS));
        if (invalid_monster_type(type) || mons_class_flag(type, M_CANT_SPAWN)
            || mons_is_ghost_demon(type))
        {
            continue;
        }
        map<int, int> hp;
        for (int t = 0; t < trials; ++t)
            ++hp[hit_points(get_monster_data(type)->avg_hp_10x)];
        printf("{\"case\":%d,\"kind\":\"hit-points\",\"monster\":%d,\"name\":\"%s\",\"trials\":%d,"
               "\"hp\":%s}\n",
               index, type, mons_type_name(type, DESC_PLAIN).c_str(), trials,
               _histogram(hp).c_str());
        return;
    }
}

int main(int argc, char *argv[])
{
    const oracle_run run = _read_run(argc, argv, true);
    crawl_state.test = true;
    msg::suppress quiet;
    rng::seed(run.seed);
    _initialise_crawl();
    const set<spell_type> zaps = _read_zaps(run.ported);

    _print_header(run);
    for (int i = 0; i < run.cases; ++i)
    {
        if (!_in_shard(run, i, KINDS))
            continue;
        _start_case(run, i);
        switch (i % KINDS)
        {
        case 0: _melee_case(i, run.trials); break;
        case 1: _shot_case(i, run.trials); break;
        case 2: _beam_case(i, run.trials, zaps); break;
        default: _hp_case(i, run.trials); break;
        }
    }
    return 0;
}

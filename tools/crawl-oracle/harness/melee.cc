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
 * Usage: melee <seed> <cases> <trials> [<shard>/<count>]
 *   Each case is seeded from <seed> and its index; with a shard, only its cases are drawn
 *   (common.h).
 */

#include "common.h"

#include "melee-attack.h"

// Where the attacker stands: next to the player.
static const coord_def ATTACKER_POS(GXM / 2 + 1, GYM / 2);

// A random monster with a damaging melee attack, placed next to the player, awake, with no
// items and no statuses: the attacker the engine's melee port models. In an umbra of the
// player's own, it sees through it or not as a coin falls, as few kinds do.
static monster *_random_monster(int &attack_number)
{
    const bool in_umbra = you.umbra();
    const bool nightvision = coinflip();
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
        if (attacks.empty() || !mon->visible_to(&you) || !you.visible_to(mon)
            || in_umbra && mon->nightvision() != nightvision)
        {
            _remove(*mon);
            continue;
        }
        attack_number = attacks[random2(attacks.size())];
        return mon;
    }
}

static void _run_case(int index, int trials)
{
    const defences def = _random_player();
    _random_light(true);
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

    printf("{\"case\":%d,\"type\":%d,\"monster\":\"%s\",\"hd\":%d,\"fighter\":%s,"
           "\"nightvision\":%s,\"attack\":{\"damage\":%d,\"crush\":%s},"
           "\"player\":{\"ac\":%d,\"ev\":%d,\"sh\":%d},\"lighting\":\"%s\","
           "\"trials\":%d,\"to_hit\":%s,\"hits\":%d,\"blocks\":%d,\"damage\":%s}\n",
           index, mon->type, mon->name(DESC_PLAIN, true).c_str(), mon->get_hit_dice(),
           mon->is_fighter() ? "true" : "false", mon->nightvision() ? "true" : "false",
           attk.attk_damage, crush ? "true" : "false", def.ac, def.ev, def.sh, _lighting(),
           trials, _histogram(to_hit).c_str(), hits, blocks, _histogram(damage).c_str());

    _remove(*mon);
}

int main(int argc, char *argv[])
{
    const oracle_run run = _read_run(argc, argv, false);
    crawl_state.test = true;
    msg::suppress quiet;
    rng::seed(run.seed);
    _initialise_crawl();

    _print_header(run);
    for (int i = 0; i < run.cases; ++i)
    {
        if (!_in_shard(run, i, 1))
            continue;
        _start_case(run, i);
        _run_case(i, run.trials);
    }
    return 0;
}

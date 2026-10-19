/*
 * The crawl oracle's harness: runs crawl's own melee code for randomised monster attackers and
 * player defenders, and prints what came out, one JSON object per line, for `crawl-oracle` to
 * diff against the engine's ported formulas.
 *
 * It links against a crawl build's objects in place of main.o, as crawl's `util/monster` does,
 * and is compiled with -fno-access-control so it can call the protected steps of
 * melee_attack::attack (test_hit, attack_shield_blocked, calc_damage, apply_damage_brand) one at
 * a time, without the other effects that follow a hit. Every other case's monster holds melee
 * weapons: a random one in its main hand, with a random enchantment and brand, and when it
 * wields two weapons, half the time another in its off hand. The player is given a species, a
 * god and resistances at random (common.h), which change what a brand adds. Each case also
 * tallies the delay of a melee action by its monster (monster::attack_delay), which a weapon's
 * brand changes. Built by oracle.mk and run by tools/crawl-oracle/oracle.sh.
 *
 * Usage: melee <seed> <cases> <trials> [<shard>/<count>]
 *   Each case is seeded from <seed> and its index; with a shard, only its cases are drawn
 *   (common.h).
 */

#include "common.h"

#include "melee-attack.h"
#include "state.h"
#include "unwind.h"

// The kinds of case, which take turns: unarmed, then armed.
static const int KINDS = 2;

// Where the attacker stands: next to the player.
static const coord_def ATTACKER_POS(GXM / 2 + 1, GYM / 2);

// A random kind of melee weapon, of those crawl still makes: neither a launcher nor the retired
// blowgun, trained by Throwing, which Game Knowledge doesn't count as either.
static weapon_type _random_melee_weapon()
{
    while (true)
    {
        const auto type = static_cast<weapon_type>(random2(NUM_WEAPONS));
        if (!item_type_removed(OBJ_WEAPONS, type) && !is_ranged_weapon_type(type)
            && item_attack_skill(OBJ_WEAPONS, type) != SK_THROWING)
        {
            return type;
        }
    }
}

// Gives `mon` a random melee weapon in its main hand, and when it wields two weapons, half the
// time another in its off hand, each with a random enchantment and brand. Returns false if it
// isn't left holding them.
static bool _arm(monster &mon)
{
    const bool both = mons_wields_two_weapons(mon) && coinflip();
    for (int hand = 0; hand < (both ? 2 : 1); ++hand)
    {
        _give_weapon(mon, _random_melee_weapon(), random_range(MIN_PLUS, MAX_PLUS),
                     _random_brand(mon));
    }
    return _held(mon, true) && (!both || _held(mon, false));
}

// A random monster with a damaging melee attack, placed next to the player, awake, with no
// statuses and, unless `armed`, no items: the attacker the engine's melee port models. An armed
// one is of a kind that uses its starting equipment, holds melee weapons (_arm) and makes at
// least one of its damaging attacks with one. In an umbra of the player's own, it sees through
// it or not as a coin falls, as few kinds do.
static monster *_random_monster(int &attack_number, bool armed)
{
    const bool in_umbra = you.umbra();
    const bool nightvision = coinflip();
    while (true)
    {
        const monster_type type = static_cast<monster_type>(random2(NUM_MONSTERS));
        if (invalid_monster_type(type) || mons_class_is_zombified(type)
            || mons_is_unique(type) || mons_class_flag(type, M_CANT_SPAWN)
            || mons_class_is_animated_object(type)
            || armed && mons_class_itemuse(type) < MONUSE_STARTING_EQUIPMENT)
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
        bool with_weapon = false;
        if (!armed || _arm(*mon))
        {
            for (int i = 0; i < MAX_NUM_ATTACKS; ++i)
            {
                const mon_attack_def attk = mons_attack_spec(*mon, i, true);
                // A hydra's heads past its kind's attacks repeat its first, which its Game
                // Knowledge doesn't list.
                if (get_monster_data(type)->attack[i].type != AT_NONE && attk.type != AT_NONE
                    && attk.damage > 0)
                {
                    attacks.push_back(i);
                    with_weapon |= mon->weapon(i) != nullptr;
                }
            }
        }
        if (attacks.empty() || armed && !with_weapon || !mon->visible_to(&you)
            || !you.visible_to(mon) || in_umbra && mon->nightvision() != nightvision)
        {
            mon->destroy_inventory();
            _remove(*mon);
            continue;
        }
        attack_number = attacks[random2(attacks.size())];
        return mon;
    }
}

static void _run_case(int index, int trials)
{
    _random_species();
    const defences def = _random_player();
    _random_light();
    int attack_number = 0;
    monster *mon = _random_monster(attack_number, index % KINDS == 1);
    const string resists = _random_resists(def);

    melee_attack attk(mon, &you, attack_number, attack_number);

    map<int, int> to_hit, damage, delay;
    int hits = 0, blocks = 0;
    // A brand that drains takes the player's maximum hit points away for good.
    unwind_var<FixedBitVector<NUM_DISABLEMENTS>> no_afflictions(crawl_state.disables);
    crawl_state.disables.set(DIS_AFFLICTIONS);
    const auto durations = you.duration;
    for (int t = 0; t < trials; ++t)
    {
        // melee_attack::attack, step by step, without the effects of a hit but its brand's
        // damage (melee_attack::handle_phase_hit), which acts on what the hit dealt.
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
        {
            attk.damage_done = attk.calc_damage();
            attk.apply_damage_brand();
            ++damage[attk.damage_done + attk.special_damage];
            // The brand's damage is taken, and its element may leave a status.
            you.hp = you.hp_max;
            you.duration = durations;
        }
        ++delay[mon->attack_delay().roll()];
    }

    printf("{\"case\":%d,\"type\":%d,\"monster\":\"%s\",\"hd\":%d,\"fighter\":%s,"
           "\"nightvision\":%s,\"slot\":%d,\"attack\":{\"damage\":%d},\"main\":%s,"
           "\"off\":%s,\"player\":{\"ac\":%d,\"ev\":%d,\"sh\":%d},\"resists\":%s,"
           "\"lighting\":\"%s\",\"trials\":%d,\"to_hit\":%s,\"hits\":%d,\"blocks\":%d,"
           "\"damage\":%s,\"delay\":%s}\n",
           index, mon->type, mon->name(DESC_PLAIN, true).c_str(), mon->get_hit_dice(),
           _json_bool(mon->is_fighter()), _json_bool(mon->nightvision()), attack_number,
           attk.attk_damage, _weapon_json(_held(*mon, true)).c_str(),
           _weapon_json(_held(*mon, false)).c_str(), def.ac, def.ev, def.sh, resists.c_str(),
           _lighting(), trials, _histogram(to_hit).c_str(), hits, blocks,
           _histogram(damage).c_str(), _histogram(delay).c_str());

    mon->destroy_inventory();
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
        if (!_in_shard(run, i, KINDS))
            continue;
        _start_case(run, i);
        _run_case(i, run.trials);
    }
    return 0;
}

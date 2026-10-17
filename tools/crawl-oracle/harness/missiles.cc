/*
 * The crawl oracle's missile harness: runs crawl's own code for random monsters made with their
 * gear, shooting, throwing and zapping wands at randomly dressed players, and prints what came
 * out, one JSON object per line, for `crawl-oracle` to diff against the engine.
 *
 * Five kinds of case take turns:
 *
 * - gear: a random kind of monster that uses its starting equipment is given its items by
 *   crawl's give_item many times, as made on D:5 outside the Orcish Mines and not in a band, with
 *   the game's item sets picked afresh each time. It tallies the launcher, or else the throwing
 *   missile, it got, and the wand it got and will zap (monster::likes_wand).
 * - missile: a monster with a launcher or throwing missile from its gear, made +0 and unbranded,
 *   fires it at the player many times, rolled as ranged_attack::attack rolls it: the to-hit,
 *   attack::test_hit against the player's evasion, attack_shield_blocked, then calc_damage, which
 *   applies armour. It skips the effects of a hit.
 * - fire: such a monster a few squares from the player decides whether to shoot or throw, many
 *   times, through crawl's handle_throw, which fires for real. It tallies the energy each action
 *   took, 0 when it didn't fire. Nets are left out: one that hits catches the player.
 * - wand: a random monster zaps a wand of a spell the engine ports at the player many times,
 *   set up by setup_mons_cast as a wand zap and rolled as the spell harness rolls a beam.
 * - zap: a monster given such a wand, which it likes, a few squares from the player decides
 *   whether to zap it, many times, through crawl's _handle_wand, which zaps for real. It tallies
 *   the energy each action took.
 *
 * Items are named "<object_class_type>:<sub_type>", or "none". It includes mon-act.cc, whose
 * _handle_wand is static, and links against a crawl build's objects without mon-act.o and
 * main.o (oracle.mk). Compiled with -fno-access-control.
 *
 * Usage: missiles <seed> <cases> <trials> <ported spells> [<shard>/<count>]
 *   <ported spells> is a file of the spell_type values the engine ports, one per line, as
 *   `crawl-oracle ported-spells` prints them. Each case is seeded from <seed> and its index;
 *   with a shard, only the cases whose index leaves <shard> when divided by <count> are drawn.
 */

#include "common.h"

#include "mon-act.cc"

#include <fstream>
#include <set>

#include "cloud.h"
#include "libunix.h"
#include "mon-gear.h"
#include "mon-place.h"
#include "ranged-attack.h"
#include "spl-book.h"
#include "viewgeom.h"

// The item level gear is made at: D:5, deeper than every depth give_item asks about.
static const int GEAR_LEVEL = 4;

static set<spell_type> _read_ported(const char *path)
{
    set<spell_type> ported;
    ifstream file(path);
    int value;
    while (file >> value)
        ported.insert(static_cast<spell_type>(value));
    if (ported.empty())
    {
        fprintf(stderr, "no ported spells in %s\n", path);
        exit(1);
    }
    return ported;
}

static string _item_name(const item_def *item)
{
    return item ? make_stringf("%d:%d", item->base_type, item->sub_type) : "none";
}

static string _tally(const map<string, int> &counts)
{
    string out = "{";
    for (const auto &entry : counts)
    {
        if (out.size() > 1)
            out += ",";
        out += make_stringf("\"%s\":%d", entry.first.c_str(), entry.second);
    }
    return out + "}";
}

// The launcher a monster has, or else its throwing missile.
static item_def *_missile_weapon(const monster &mon)
{
    if (item_def *launcher = mon.launcher())
        return launcher;
    return mon.missiles();
}

// The wand a monster has and will zap.
static item_def *_liked_wand(const monster &mon)
{
    item_def *wand = mon.mslot_item(MSLOT_WAND);
    return wand && mon.likes_wand(*wand) ? wand : nullptr;
}

// Crawl's own way of giving `mon` its gear, as on GEAR_LEVEL in the Dungeon, not in a band and
// in a game whose item sets are picked afresh.
static void _regear(monster &mon)
{
    mon.destroy_inventory();
    initialise_item_sets(true);
    active_monster_band = BAND_NO_BAND;
    give_item(&mon, GEAR_LEVEL, false);
}

// A random kind of monster placed `distance` squares east of the player, awake, hunting them,
// with no statuses, of a kind that uses its starting equipment and for which `wanted` holds.
static monster *_random_monster(int distance, bool (*wanted)(monster_type))
{
    const coord_def pos(GXM / 2 + distance, GYM / 2);
    while (true)
    {
        const monster_type type = static_cast<monster_type>(random2(NUM_MONSTERS));
        if (invalid_monster_type(type) || mons_class_is_zombified(type)
            || mons_class_flag(type, M_CANT_SPAWN) || mons_class_is_animated_object(type)
            || mons_class_itemuse(type) < MONUSE_STARTING_EQUIPMENT || !wanted(type)
            // Hepliaklqana's ancestors are made from the player and are only ever their
            // allies; crawl never lets one zap at them.
            || mons_is_hepliaklqana_ancestor(type)
            // Its polymorph status can't be taken off without the form it would take.
            || type == MONS_ASPIRING_FLESH)
        {
            continue;
        }
        mons_spec spec(type);
        monster *mon = dgn_place_monster(spec, pos, true, true, false);
        if (!mon)
            continue;
        // A unique can be made again.
        you.unique_creatures.set(type, false);
        while (!mon->enchantments.empty())
            mon->del_ench(mon->enchantments.begin()->first, true, false);
        mon->behaviour = BEH_SEEK;
        mon->foe = MHITYOU;
        mon->target = you.pos();
        if (!mon->visible_to(&you) || !you.visible_to(mon) || you.backlit(false) || you.umbra())
        {
            _remove(*mon);
            continue;
        }
        return mon;
    }
}

static bool _any_kind(monster_type)
{
    return true;
}

// Gives `mon` its gear until it has a launcher or a throwing missile, +0, unbranded and wielded
// if a launcher, and nothing else. Returns false if a few tries gave it none.
static bool _arm(monster &mon, bool nets)
{
    for (int attempt = 0; attempt < 40; ++attempt)
    {
        _regear(mon);
        item_def *weapon = _missile_weapon(mon);
        if (!weapon || is_artefact(*weapon)
            || (!nets && weapon->is_type(OBJ_MISSILES, MI_THROWING_NET)))
        {
            continue;
        }
        item_def kept = *weapon;
        kept.plus = 0;
        kept.brand = 0;
        kept.flags &= ~ISFLAG_CHAOTIC;
        kept.quantity = max<short>(kept.quantity, 1);
        mon.destroy_inventory();
        give_specific_item(&mon, kept);
        // A launcher in the off hand would be swapped in first, which costs energy the engine
        // doesn't play out.
        if (mon.inv[MSLOT_ALT_WEAPON] != NON_ITEM && mon.inv[MSLOT_WEAPON] == NON_ITEM)
            swap(mon.inv[MSLOT_WEAPON], mon.inv[MSLOT_ALT_WEAPON]);
        if (_missile_weapon(mon))
            return true;
    }
    return false;
}

// A random monster with a launcher or throwing missile from its gear, placed `distance`
// squares east of the player.
static monster *_random_shooter(int distance, bool nets)
{
    while (true)
    {
        monster *mon = _random_monster(distance, _any_kind);
        if (_arm(*mon, nets))
            return mon;
        _remove(*mon);
    }
}

// Whether a few tries at making `mon` gave it a launcher, a throwing missile or a wand.
static bool _may_shoot_or_zap(monster &mon)
{
    for (int attempt = 0; attempt < 40; ++attempt)
    {
        _regear(mon);
        if (_missile_weapon(mon) || _liked_wand(mon))
            return true;
    }
    return false;
}

// Three gear cases in four are of a kind that may be made with something to shoot or zap: most
// kinds never are.
static void _gear_case(int index, int trials)
{
    const bool armed = !one_chance_in(4);
    monster *mon = _random_monster(1, _any_kind);
    while (armed && !_may_shoot_or_zap(*mon))
    {
        mon->destroy_inventory();
        _remove(*mon);
        mon = _random_monster(1, _any_kind);
    }
    map<string, int> missile, wand;
    for (int t = 0; t < trials; ++t)
    {
        _regear(*mon);
        const item_def *launcher = mon->launcher();
        const item_def *thrown = mon->missiles();
        // The bundle says a monster is made with at most one.
        ++missile[launcher && thrown ? "both" : _item_name(_missile_weapon(*mon))];
        ++wand[_item_name(_liked_wand(*mon))];
    }
    printf("{\"case\":%d,\"kind\":\"gear\",\"monster\":%d,\"name\":\"%s\",\"trials\":%d,"
           "\"missile\":%s,\"wand\":%s}\n",
           index, mon->type, mon->name(DESC_PLAIN, true).c_str(), trials,
           _tally(missile).c_str(), _tally(wand).c_str());
    mon->destroy_inventory();
    _remove(*mon);
}

static void _missile_case(int index, int trials)
{
    const defences def = _random_player();
    monster *mon = _random_shooter(1, true);
    const item_def weapon = *_missile_weapon(*mon);
    const bool harmless = weapon.is_type(OBJ_MISSILES, MI_DART)
                          || weapon.is_type(OBJ_MISSILES, MI_THROWING_NET);

    map<int, int> to_hit, damage;
    int hits = 0, blocks = 0;
    for (int t = 0; t < trials; ++t)
    {
        // Its to-hit is rolled as it is made (attack::init_attack).
        ranged_attack atk(mon, &you, &weapon);
        ++to_hit[atk.to_hit];
        you.shield_blocks = 0;
        const int ev = you.evasion(false, mon) + you.missile_repulsion();
        const int margin = atk.test_hit(atk.to_hit, ev, true);
        const bool blocked = atk.attack_shield_blocked();
        hits += margin >= 0;
        blocks += blocked;
        if (blocked || margin < 0)
            continue;
        // ranged_attack::handle_phase_hit: a dart or net deals no damage itself.
        ++damage[harmless ? 0 : atk.calc_damage()];
    }

    printf("{\"case\":%d,\"kind\":\"missile\",\"monster\":%d,\"name\":\"%s\",\"hd\":%d,"
           "\"weapon\":\"%s\",\"weapon_name\":\"%s\","
           "\"player\":{\"ac\":%d,\"ev\":%d,\"sh\":%d},"
           "\"trials\":%d,\"to_hit\":%s,\"hits\":%d,\"blocks\":%d,\"damage\":%s}\n",
           index, mon->type, mon->name(DESC_PLAIN, true).c_str(), mon->get_hit_dice(),
           _item_name(&weapon).c_str(), weapon.name(DESC_DBNAME, true).c_str(), def.ac,
           def.ev, def.sh, trials, _histogram(to_hit).c_str(), hits, blocks,
           _histogram(damage).c_str());
    mon->destroy_inventory();
    _remove(*mon);
}

static void _fire_case(int index, int trials)
{
    const int distance = 1 + random2(LOS_DEFAULT_RANGE);
    monster *mon = _random_shooter(distance, false);
    const item_def weapon = *_missile_weapon(*mon);

    map<int, int> energy;
    for (int t = 0; t < trials; ++t)
    {
        _restore_player();
        mon->speed_increment = 200;
        bolt beem = setup_targeting_beam(*mon);
        const bool fired = handle_throw(mon, beem, false, false, false);
        ++energy[fired ? 200 - mon->speed_increment : 0];
        // A blocked missile may leave what it was in the air: nothing should linger.
        delete_all_clouds();
    }
    _restore_player();

    printf("{\"case\":%d,\"kind\":\"fire\",\"monster\":%d,\"name\":\"%s\","
           "\"weapon\":\"%s\",\"weapon_name\":\"%s\",\"distance\":%d,\"trials\":%d,"
           "\"energy\":%s}\n",
           index, mon->type, mon->name(DESC_PLAIN, true).c_str(), _item_name(&weapon).c_str(),
           weapon.name(DESC_DBNAME, true).c_str(), distance, trials,
           _histogram(energy).c_str());
    mon->destroy_inventory();
    _remove(*mon);
}

// The wands whose spells the engine ports.
static vector<wand_type> _ported_wands(const set<spell_type> &ported)
{
    vector<wand_type> wands;
    for (int i = 0; i < NUM_WANDS; ++i)
    {
        const wand_type wand = static_cast<wand_type>(i);
        if (!item_type_removed(OBJ_WANDS, wand) && ported.count(spell_in_wand(wand)))
            wands.push_back(wand);
    }
    if (wands.empty())
    {
        fprintf(stderr, "no wand zaps a spell the engine ports\n");
        exit(1);
    }
    return wands;
}

static void _wand_case(int index, int trials, const vector<wand_type> &wands)
{
    const defences def = _random_player();
    monster *mon = _random_monster(1, _any_kind);
    mon->destroy_inventory();
    const wand_type wand = wands[random2(wands.size())];
    const spell_type spell = spell_in_wand(wand);
    const int power = mons_wand_power(mon->get_hit_dice(), spell);
    const int range = spell_range(spell, mon, power);
    const beam_tally tally = _cast_at_player(*mon, spell, true, trials);

    printf("{\"case\":%d,\"kind\":\"wand\",\"monster\":%d,\"name\":\"%s\",\"hd\":%d,"
           "\"spell\":%d,\"spell_name\":\"%s\","
           "\"player\":{\"ac\":%d,\"ev\":%d,\"sh\":%d},"
           "\"power\":%d,\"range\":%d,%s}\n",
           index, mon->type, mon->name(DESC_PLAIN, true).c_str(), mon->get_hit_dice(), spell,
           spell_title(spell), def.ac, def.ev, def.sh, power, range,
           _beam_json(tally, trials).c_str());
    _remove(*mon);
}

static void _zap_case(int index, int trials, const vector<wand_type> &wands)
{
    const wand_type wand = wands[random2(wands.size())];
    const spell_type spell = spell_in_wand(wand);
    // Farther than any blast reaches back: crawl's tracer weighs a zapper's harm to itself
    // (mons_should_fire), which the engine doesn't port.
    const int nearest = zap_explodes(spell_to_zap(spell)) ? 3 : 1;
    const int distance = nearest + random2(LOS_DEFAULT_RANGE - nearest + 1);
    monster *mon = nullptr;
    while (true)
    {
        mon = _random_monster(distance, _any_kind);
        mon->destroy_inventory();
        item_def made;
        made.base_type = OBJ_WANDS;
        made.sub_type = wand;
        made.quantity = 1;
        made.charges = 100;
        if (mon->likes_wand(made))
        {
            give_specific_item(mon, made);
            if (mon->mslot_item(MSLOT_WAND))
                break;
        }
        _remove(*mon);
    }

    map<int, int> energy;
    for (int t = 0; t < trials; ++t)
    {
        _restore_player();
        mon->speed_increment = 200;
        mon->mslot_item(MSLOT_WAND)->charges = 100;
        const bool zapped = _handle_wand(*mon);
        ++energy[zapped ? 200 - mon->speed_increment : 0];
        delete_all_clouds();
    }
    _restore_player();

    printf("{\"case\":%d,\"kind\":\"zap\",\"monster\":%d,\"name\":\"%s\",\"hd\":%d,"
           "\"spell\":%d,\"spell_name\":\"%s\",\"distance\":%d,\"trials\":%d,"
           "\"energy\":%s}\n",
           index, mon->type, mon->name(DESC_PLAIN, true).c_str(), mon->get_hit_dice(), spell,
           spell_title(spell), distance, trials, _histogram(energy).c_str());
    mon->destroy_inventory();
    _remove(*mon);
}

int main(int argc, char *argv[])
{
    const oracle_run run = _read_run(argc, argv, true);
    crawl_state.test = true;
    // Firing and zapping redraw the view, which needs a console: crawl's headless one.
    enter_headless_mode();
    console_startup();
    crawl_view.init_geometry();
    crawl_state.disables.set(DIS_DELAY);
    msg::suppress quiet;
    rng::seed(run.seed);
    _initialise_crawl();
    const vector<wand_type> wands = _ported_wands(_read_ported(run.ported));

    _print_header(run);
    const int trials = run.trials;
    for (int i = 0; i < run.cases; ++i)
    {
        if (!_in_shard(run, i))
            continue;
        _start_case(run, i);
        switch (i % 5)
        {
        case 0: _gear_case(i, trials); break;
        case 1: _missile_case(i, trials); break;
        case 2: _fire_case(i, trials); break;
        case 3: _wand_case(i, trials, wands); break;
        default: _zap_case(i, trials, wands); break;
        }
    }
    return 0;
}

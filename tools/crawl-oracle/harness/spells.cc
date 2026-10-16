/*
 * The crawl oracle's spell harness: runs crawl's own code for random monsters casting the spells
 * the engine ports at randomly dressed players, and prints what came out, one JSON object per
 * line, for `crawl-oracle` to diff against the engine's ported formulas.
 *
 * Two kinds of case alternate:
 *
 * - beam: a monster casts one spell at the player many times. Each cast is set up by crawl's
 *   setup_mons_cast, then rolled as bolt::affect_player rolls it: bolt::misses_player for the
 *   shield and evasion, then the damage dice, bolt::apply_AC and check_your_resists. The beam's
 *   flight (bolt::fire) is skipped, so what flight changes, such as a bounce or a chaos beam's
 *   flavour, isn't diffed.
 * - choice: a monster a few squares from the player picks a spell many times, through crawl's
 *   _choose_spell_to_cast, from its book cut down to the spells the engine ports, as the engine
 *   assumes the others aren't worth casting. A monster with an explosion or a cloud among them is
 *   kept three squares or more away, out of its own blast: the engine doesn't port how crawl's
 *   tracer weighs a caster's harm to itself.
 *
 * It includes mon-cast.cc, whose spell choice is static, and links against a crawl build's
 * objects without mon-cast.o and main.o (oracle.mk). Compiled with -fno-access-control.
 *
 * Usage: spells <seed> <cases> <trials> <ported spells>
 *   <ported spells> is a file of the spell_type values the engine ports, one per line, as
 *   `crawl-oracle ported-spells` prints them.
 */

#include "common.h"

#include "mon-cast.cc"

#include <algorithm>
#include <fstream>
#include <set>

#include "cloud.h"
#include "los.h"
#include "mon-book.h"
#include "ouch.h"

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

// The slots of a monster's spells that the engine ports. A monster made by dgn_place_monster
// holds its kind's book, as mspell_list has it, unless crawl fills its book when it is made
// (MST_GHOST), which the engine can't know and the harness leaves out.
static monster_spells _ported_book(const monster &mon, const set<spell_type> &ported)
{
    monster_spells slots;
    if (get_spellbook(monster_info(&mon)) == MST_GHOST)
        return slots;
    for (const mon_spell_slot &slot : mon.spells)
        if (ported.count(slot.spell))
            slots.push_back(slot);
    return slots;
}

// A random kind of monster with a spellbook holding a spell the engine ports, placed `distance`
// squares east of the player, awake, hunting them, with no items and no statuses, and with only
// those spells. Draconians are left out: crawl gives them a breath their book doesn't hold.
static monster *_random_caster(const set<spell_type> &ported, int distance)
{
    const coord_def pos(GXM / 2 + distance, GYM / 2);
    while (true)
    {
        const monster_type type = static_cast<monster_type>(random2(NUM_MONSTERS));
        if (invalid_monster_type(type) || mons_class_is_zombified(type)
            || mons_is_unique(type) || mons_class_flag(type, M_CANT_SPAWN)
            || mons_class_is_animated_object(type)
            || mons_genus(type) == MONS_DRACONIAN
            // Its polymorph status can't be taken off without the form it would take.
            || type == MONS_ASPIRING_FLESH)
        {
            continue;
        }
        mons_spec spec(type);
        monster *mon = dgn_place_monster(spec, pos, true, true, false);
        if (!mon)
            continue;
        const monster_spells book = _ported_book(*mon, ported);
        if (book.empty())
        {
            _remove(*mon);
            continue;
        }
        mon->destroy_inventory();
        while (!mon->enchantments.empty())
            mon->del_ench(mon->enchantments.begin()->first, true, false);
        mon->behaviour = BEH_SEEK;
        mon->foe = MHITYOU;
        mon->target = you.pos();
        mon->spells = book;
        if (!mon->visible_to(&you) || !you.visible_to(mon) || you.backlit(false) || you.umbra())
        {
            _remove(*mon);
            continue;
        }
        return mon;
    }
}

static void _beam_case(int index, int trials, const set<spell_type> &ported)
{
    const defences def = _random_player();
    monster *mon = _random_caster(ported, 1);
    const spell_type spell = mon->spells[random2(mon->spells.size())].spell;
    const int power = mons_spellpower(*mon, spell);
    const int range = spell_range(spell, mon, power);

    int hit = 0;
    int hits = 0, blocks = 0;
    map<int, int> damage;
    for (int t = 0; t < trials; ++t)
    {
        bolt beam;
        beam.target = you.pos();
        setup_mons_cast(mon, beam, spell);
        beam.source = mon->pos();
        // As bolt::fire aims it; a block ends the beam where its ray has got to.
        find_ray(beam.source, beam.target, beam.ray, opc_solid_see);
        beam.chose_ray = true;
        hit = beam.hit;

        you.shield_blocks = 0;
        const bool missed = beam.misses_player();
        if (you.shield_blocks > 0)
            ++blocks;
        if (missed)
            continue;
        ++hits;
        // bolt::affect_player's damage, without what the hit sets off.
        int taken = beam.apply_AC(&you, beam.damage.roll());
        taken = check_your_resists(max(0, taken), beam.flavour, "", &beam, false);
        ++damage[taken];
    }

    printf("{\"case\":%d,\"kind\":\"beam\",\"monster\":%d,\"name\":\"%s\",\"hd\":%d,"
           "\"spell\":%d,\"spell_name\":\"%s\","
           "\"player\":{\"ac\":%d,\"ev\":%d,\"sh\":%d},"
           "\"power\":%d,\"range\":%d,\"to_hit\":%s,"
           "\"trials\":%d,\"hits\":%d,\"blocks\":%d,\"damage\":%s}\n",
           index, mon->type, mon->name(DESC_PLAIN, true).c_str(), mon->get_hit_dice(),
           spell, spell_title(spell), def.ac, def.ev, def.sh, power, range,
           hit == AUTOMATIC_HIT ? "null" : make_stringf("%d", hit).c_str(), trials, hits,
           blocks, _histogram(damage).c_str());

    _remove(*mon);
}

// Whether a monster casting `spell` close by may catch itself in what it leaves: an explosion's
// blast, at most two squares across, or a cloud. Crawl's tracer weighs that harm against the
// harm to the player (mons_should_fire), which the engine doesn't port.
static bool _may_catch_caster(spell_type spell)
{
    return zap_explodes(spell_to_zap(spell)) || (get_spell_flags(spell) & spflag::cloud);
}

static void _choice_case(int index, int trials, const set<spell_type> &ported)
{
    int distance = 1 + random2(LOS_DEFAULT_RANGE);
    monster *mon = _random_caster(ported, distance);
    // Farther than any blast reaches back, where the weighing doesn't arise.
    if (distance < 3
        && any_of(mon->spells.begin(), mon->spells.end(),
                  [](const mon_spell_slot &slot) { return _may_catch_caster(slot.spell); }))
    {
        _remove(*mon);
        distance = 3 + random2(LOS_DEFAULT_RANGE - 2);
        mon = _random_caster(ported, distance);
    }

    map<int, int> chosen;
    for (int t = 0; t < trials; ++t)
    {
        bolt beam = setup_targeting_beam(*mon);
        const mon_spell_slot slot = _choose_spell_to_cast(*mon, beam, mon->spells, false);
        ++chosen[slot.spell == SPELL_NO_SPELL ? -1 : slot.spell];
    }

    printf("{\"case\":%d,\"kind\":\"choice\",\"monster\":%d,\"name\":\"%s\",\"hd\":%d,"
           "\"distance\":%d,\"trials\":%d,\"chosen\":%s}\n",
           index, mon->type, mon->name(DESC_PLAIN, true).c_str(), mon->get_hit_dice(),
           distance, trials, _histogram(chosen).c_str());

    _remove(*mon);
}

int main(int argc, char *argv[])
{
    if (argc != 5)
    {
        fprintf(stderr, "usage: %s <seed> <cases> <trials> <ported spells>\n", argv[0]);
        return 2;
    }
    crawl_state.test = true;
    msg::suppress quiet;
    rng::seed(strtoull(argv[1], nullptr, 10));
    _initialise_crawl();
    const set<spell_type> ported = _read_ported(argv[4]);

    printf("{\"crawl\":\"%s\",\"seed\":%s}\n", Version::Long, argv[1]);
    const int cases = atoi(argv[2]);
    const int trials = atoi(argv[3]);
    for (int i = 0; i < cases; ++i)
    {
        // A blocked beam ends where it was blocked, and may leave a cloud there.
        delete_all_clouds();
        if (i % 2 == 0)
            _beam_case(i, trials, ported);
        else
            _choice_case(i, trials, ported);
    }
    return 0;
}

/*
 * The crawl oracle's turn harness: runs crawl's own code for what decides whether a monster
 * gets to attack before the player's next standard-speed action, and how often, and prints what
 * came out, one JSON object per line, for `crawl-oracle` to diff against the engine.
 *
 * Three kinds of case:
 *
 * - block: for no shield and each shield the player can wear, how many attacks it blocks before
 *   the player's next action (actor::shield_block_limit).
 * - reach: for every kind of monster with a damaging melee attack, made with no items, how far
 *   its melee reaches (monster::reach_range) and which of its attacks melee_attack::can_reach
 *   lets it make from each distance within that.
 * - turn: a random monster a few squares from the player, awake, hunting them, with no items
 *   and no statuses and with its book cut down to the spells the engine ports, plays out the
 *   player's next standard-speed action many times. Each time it starts with the energy crawl
 *   leaves a monster that has acted (monster::drain_action_energy), the player is restored and
 *   the floor around them laid again, and crawl's own handle_monsters runs its turns with
 *   you.time_taken at BASELINE_DELAY, attacking and casting for real. It tallies the melee
 *   actions it took against the player, the spells it cast and the squares it closed. Turn
 *   cases take turns at drawing from kinds of monster that
 *   each exercise a rule: the slowest and the fastest that act, any, faster or slower than the
 *   player, stationary, reaching, breathing, casting an instant spell, and casting a blast or
 *   cloud. A rule no kind exercises is left out.
 *
 * It switches off monster special abilities (mon_special_ability), which the engine doesn't
 * port, and leaves out monsters that crawl moves otherwise than the engine does (#86), casters of
 * a blinkbolt, which carries them along it (#85), draconians, whose breath their book doesn't
 * hold, books crawl fills when a monster is made (MST_GHOST), and shapeshifters, which take
 * another kind's shape as they are made. Any kind that dies in its own
 * turns, moves the player or moves away from them is left out as it is found, and said so on
 * standard error. A caster of an explosion or a cloud starts three squares or more away: crawl's
 * tracer weighs a caster's harm to itself (mons_should_fire), which the engine doesn't port.
 *
 * It includes mon-act.cc, to count what a monster does as it calls into the code that does it,
 * and links against a crawl build's objects without mon-act.o and main.o (oracle.mk). Compiled
 * with -fno-access-control.
 *
 * Usage: turns <seed> <cases> <trials> <ported spells>
 *   <ported spells> is a file of the spell_type values the engine ports, one per line, as
 *   `crawl-oracle ported-spells` prints them.
 */

#include "common.h"

#include "fight.h"
#include "mon-abil.h"
#include "mon-cast.h"

// What the monster did this action, counted as mon-act.cc calls into the code that does it.
static int melee_actions = 0;
static int spells_cast = 0;

static bool _counted_fight_melee(actor *attacker, actor *defender, bool is_rampage = false,
                                 bool *did_hit = nullptr, bool simu = false);
static bool _counted_handle_mon_spell(monster *mons);
static bool _no_special_ability(monster *mons);

#define fight_melee _counted_fight_melee
#define handle_mon_spell _counted_handle_mon_spell
#define mon_special_ability _no_special_ability
#include "mon-act.cc"
#undef fight_melee
#undef handle_mon_spell
#undef mon_special_ability

#include <fstream>
#include <set>

#include "cloud.h"
#include "libunix.h"
#include "melee-attack.h"
#include "mon-book.h"
#include "spl-util.h"
#include "viewgeom.h"

static bool _has_melee(const monster &mon);

// A monster without a damaging attack walking into the player fights them without effect.
static bool _counted_fight_melee(actor *attacker, actor *defender, bool is_rampage,
                                 bool *did_hit, bool simu)
{
    if (defender->is_player() && _has_melee(*attacker->as_monster()))
        ++melee_actions;
    return fight_melee(attacker, defender, is_rampage, did_hit, simu);
}

static bool _counted_handle_mon_spell(monster *mons)
{
    const bool cast = handle_mon_spell(mons);
    spells_cast += cast;
    return cast;
}

static bool _no_special_ability(monster *)
{
    return false;
}

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

static void _block_cases(int &index)
{
    const vector<int> shields = { -1, ARM_BUCKLER, ARM_KITE_SHIELD, ARM_TOWER_SHIELD };
    for (int shield : shields)
    {
        _take_off(SLOT_OFFHAND);
        if (shield >= 0)
            _wear(1, SLOT_OFFHAND, static_cast<armour_type>(shield), 0);
        printf("{\"case\":%d,\"kind\":\"block\",\"shield\":\"%s\",\"limit\":%d}\n", index++,
               shield < 0 ? "none" : make_stringf("%d:%d", OBJ_ARMOUR, shield).c_str(),
               you.shield_block_limit());
    }
    _take_off(SLOT_OFFHAND);
}

// Whether crawl may make `type` here and the harness can play it out.
static bool _playable_kind(monster_type type)
{
    return !invalid_monster_type(type) && !mons_class_is_zombified(type)
           && !mons_class_flag(type, M_CANT_SPAWN) && !mons_class_is_animated_object(type)
           && mons_genus(type) != MONS_DRACONIAN
           // Hepliaklqana's ancestors are made from the player and are only ever their allies.
           && !mons_is_hepliaklqana_ancestor(type)
           // Its polymorph status can't be taken off without the form it would take.
           && type != MONS_ASPIRING_FLESH;
}

// Takes away every status `mon` has.
static void _clear_statuses(monster &mon)
{
    while (!mon.enchantments.empty())
        mon.del_ench(mon.enchantments.begin()->first, true, false);
}

// Takes every monster but `kept` off the map, and lets go of the player: placing some kinds
// places others with them, such as a tentacle's segments, and a monster may grab the player.
static void _clear_monsters(const monster *kept = nullptr)
{
    you.stop_being_constricted(true);
    for (monster_iterator mi; mi; ++mi)
    {
        mi->stop_constricting_all(false, true);
        if (*mi != kept)
            _remove(**mi);
    }
}

// Floor everywhere within sight of the player, as it was set up: what a monster casts or leaves
// as it dies may change the terrain, which changes how fast another walks.
static void _reset_terrain()
{
    for (radius_iterator ri(you.pos(), LOS_DEFAULT_RANGE + 1, C_SQUARE); ri; ++ri)
        env.grid(*ri) = DNGN_FLOOR;
}

// `type` placed `distance` squares east of the player, alone, awake, hunting them, with no
// items and no statuses, or nullptr if crawl won't place it there or it and the player can't
// see each other.
static monster *_place(monster_type type, int distance)
{
    _clear_monsters();
    _reset_terrain();
    mons_spec spec(type);
    const coord_def pos(GXM / 2 + distance, GYM / 2);
    monster *mon = dgn_place_monster(spec, pos, true, true, false);
    if (!mon)
        return nullptr;
    _clear_monsters(mon);
    // A shapeshifter takes another kind's shape as it is made, which the engine can't know.
    if (mon->pos() != pos || mon->type != type)
    {
        _clear_monsters();
        return nullptr;
    }
    // A unique can be made again.
    you.unique_creatures.set(type, false);
    mon->destroy_inventory();
    _clear_statuses(*mon);
    mon->flags &= ~MF_JUST_SUMMONED;
    mon->behaviour = BEH_SEEK;
    mon->foe = MHITYOU;
    mon->target = you.pos();
    if (!mon->visible_to(&you) || !you.visible_to(mon) || you.backlit(false) || you.umbra())
    {
        _clear_monsters();
        return nullptr;
    }
    return mon;
}

// Whether any of `mon`'s attacks deals damage.
static bool _has_melee(const monster &mon)
{
    for (int i = 0; i < MAX_NUM_ATTACKS; ++i)
        if (mons_attack_spec(mon, i).damage > 0)
            return true;
    return false;
}

static void _reach_cases(int &index)
{
    for (int i = 0; i < NUM_MONSTERS; ++i)
    {
        const monster_type type = static_cast<monster_type>(i);
        if (!_playable_kind(type))
            continue;
        monster *mon = _place(type, 1);
        if (!mon)
            continue;
        if (!_has_melee(*mon))
        {
            _clear_monsters();
            continue;
        }
        const int reach = mon->reach_range();
        string from = "{";
        for (int distance = 1; distance <= reach; ++distance)
        {
            string slots;
            for (int slot = 0; slot < MAX_NUM_ATTACKS; ++slot)
            {
                if (get_monster_data(type)->attack[slot].type == AT_NONE)
                    continue;
                melee_attack swing(mon, &you, slot, slot);
                if (swing.can_reach(distance))
                    slots += make_stringf("%s%d", slots.empty() ? "" : ",", slot);
            }
            from += make_stringf("%s\"%d\":[%s]", distance > 1 ? "," : "", distance,
                                 slots.c_str());
        }
        printf("{\"case\":%d,\"kind\":\"reach\",\"monster\":%d,\"name\":\"%s\",\"reach\":%d,"
               "\"attacks_from\":%s}}\n",
               index++, type, mon->name(DESC_PLAIN, true).c_str(), reach, from.c_str());
        _clear_monsters();
    }
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

// Whether a monster casting `spell` close by may catch itself in what it leaves: an explosion's
// blast or a cloud.
static bool _may_catch_caster(spell_type spell)
{
    return zap_explodes(spell_to_zap(spell)) || (get_spell_flags(spell) & spflag::cloud);
}

static bool _any_slot(const monster_spells &book, bool (*wanted)(const mon_spell_slot &))
{
    return any_of(book.begin(), book.end(), wanted);
}

// Whether a monster with `book` may catch itself in what it casts close by.
static bool _book_may_catch_caster(const monster_spells &book)
{
    return _any_slot(book,
                     [](const mon_spell_slot &slot) { return _may_catch_caster(slot.spell); });
}

// What a kind of monster that a turn case may draw is like.
struct turn_kind
{
    monster_type type;
    int speed;
    bool stationary;
    int reach;
    monster_spells book;
};

// The rules a turn case's monster exercises, one per case in turn.
static const vector<pair<const char *, bool (*)(const turn_kind &)>> TURN_RULES = {
    { "any", [](const turn_kind &) { return true; } },
    { "speed", [](const turn_kind &k) { return k.speed > 0 && k.speed != 10; } },
    { "stationary", [](const turn_kind &k) { return k.stationary; } },
    { "reaching", [](const turn_kind &k) { return k.reach > 1; } },
    { "breath",
      [](const turn_kind &k) {
          return _any_slot(k.book, [](const mon_spell_slot &slot) {
              return bool(slot.flags & MON_SPELL_BREATH);
          });
      } },
    { "instant",
      [](const turn_kind &k) {
          return _any_slot(k.book, [](const mon_spell_slot &slot) {
              return bool(slot.flags & MON_SPELL_INSTANT);
          });
      } },
    { "blast", [](const turn_kind &k) { return _book_may_catch_caster(k.book); } },
};

// Kinds handle_monster_move acts or moves for with code of their own, rather than by walking,
// fighting and casting.
static const set<monster_type> OWN_ACTIONS = {
    MONS_FULMINANT_PRISM, MONS_SHADOW_PRISM, MONS_SPLINTERFROST_BARRICADE, MONS_BLAZEHEART_CORE,
    MONS_BLAZEHEART_GOLEM, MONS_RENDING_BLADE, MONS_BURSTSHROOM, MONS_SIXFIRHY, MONS_JIANGSHI,
    MONS_BOULDER, MONS_HELLFIRE_MORTAR, MONS_DIAMOND_SAWBLADE, MONS_LIGHTNING_SPIRE,
    MONS_SOLAR_EMBER, MONS_BOUNDLESS_TESSERACT,
    // They won't move while the player watches them (_fungal_move_check).
    MONS_WANDERING_MUSHROOM, MONS_DEATHCAP, MONS_LURKING_HORROR,
};

// Whether crawl moves a kind the way the engine does (#86): straight at the player across the
// floor, unless it never moves. A tentacle or vine moves with its parent, a batty monster flits
// away after it attacks, one that keeps its range won't close in to fight, and a monster of lava
// or water can't leave it.
static bool _walks_as_engine_does(monster_type type)
{
    return !OWN_ACTIONS.count(type) && !mons_is_projectile(type) && !mons_is_seeker(type)
           && !mons_is_tentacle_or_tentacle_segment(type) && !mons_class_flag(type, M_BATTY)
           && !mons_class_flag(type, M_MAINTAIN_RANGE) && monster_habitable_feat(type, DNGN_FLOOR);
}

// Every kind a turn case may draw: one that attacks in melee or casts a spell the engine ports.
static vector<turn_kind> _turn_kinds(const set<spell_type> &ported)
{
    vector<turn_kind> kinds;
    for (int i = 0; i < NUM_MONSTERS; ++i)
    {
        const monster_type type = static_cast<monster_type>(i);
        if (!_playable_kind(type) || !_walks_as_engine_does(type))
            continue;
        monster *mon = _place(type, 1);
        if (!mon)
            continue;
        const monster_spells book = _ported_book(*mon, ported);
        const attack_flavour first = mons_attack_spec(*mon, 0, true).flavour;
        // A blinkbolt carries its caster along it (bolt::affect_endpoint), which the engine
        // doesn't model (#85).
        const bool blinks = _any_slot(book, [](const mon_spell_slot &slot) {
            return slot.spell == SPELL_BLINKBOLT;
        });
        // Crawl seldom casts a cloud again through the clouds its last cast left, which the
        // engine doesn't model (#86): only a monster faster than the player casts twice before
        // the player's next action.
        const bool recasts_clouds = mon->speed > 10
                                    && _any_slot(book, [](const mon_spell_slot &slot) {
                                           return bool(get_spell_flags(slot.spell)
                                                       & spflag::cloud);
                                       });
        if ((_has_melee(*mon) || !book.empty()) && first != AF_SWOOP && first != AF_FLANK
            && !blinks && !recasts_clouds)
        {
            kinds.push_back({ type, mon->speed, mon->is_stationary(), mon->reach_range(),
                              book });
        }
        _clear_monsters();
    }
    return kinds;
}

// A rule a turn case's monster exercises, and the kinds of monster that exercise it.
struct turn_rule
{
    const char *name;
    vector<const turn_kind *> kinds;
};

// The rules some kind of monster exercises. A rule no kind exercises, such as an instant spell
// when the engine ports none, is left out, and said so.
static vector<turn_rule> _turn_rules(const vector<turn_kind> &kinds)
{
    // The slowest and fastest kinds that act at all, which a draw from every other speed seldom
    // reaches.
    int slowest = INT_MAX, fastest = 0;
    for (const turn_kind &kind : kinds)
    {
        if (kind.speed > 0)
        {
            slowest = min(slowest, kind.speed);
            fastest = max(fastest, kind.speed);
        }
    }
    turn_rule slow = { "slowest", {} }, fast = { "fastest", {} };
    for (const turn_kind &kind : kinds)
    {
        if (kind.speed == slowest)
            slow.kinds.push_back(&kind);
        if (kind.speed == fastest)
            fast.kinds.push_back(&kind);
    }
    vector<turn_rule> rules = { slow, fast };
    for (const auto &rule : TURN_RULES)
    {
        turn_rule drawn = { rule.first, {} };
        for (const turn_kind &kind : kinds)
            if (rule.second(kind))
                drawn.kinds.push_back(&kind);
        if (drawn.kinds.empty())
            fprintf(stderr, "no kind of monster exercises the %s rule: left out\n", rule.first);
        else
            rules.push_back(drawn);
    }
    return rules;
}

// What a monster did over many plays of the player's next action.
struct turn_tally
{
    map<int, int> melee;
    map<int, int> casts;
    map<int, int> closed;
};

// Plays out the player's next standard-speed action `trials` times against `mon`, `distance`
// squares away, each time as it starts. Returns why the engine can't follow it, or nullptr if it
// can: a monster may burst or crumble in its own turns, move the player, as a trampling or
// pulling hit does, or move away from them, as a blinking one does.
static const char *_play_turns(monster &mon, int distance, int trials, turn_tally &tally)
{
    const coord_def start = mon.pos();
    const coord_def player = you.pos();
    for (int t = 0; t < trials; ++t)
    {
        _clear_monsters(&mon);
        _restore_player();
        _reset_terrain();
        delete_all_clouds();
        if (mon.pos() != start)
            mon.move_to(start);
        _clear_statuses(mon);
        mon.hit_points = mon.max_hit_points;
        mon.foe = MHITYOU;
        mon.target = you.pos();
        // As crawl leaves a monster that has acted.
        mon.speed_increment = 200;
        mon.drain_action_energy();

        melee_actions = 0;
        spells_cast = 0;
        you.time_taken = BASELINE_DELAY;
        handle_monsters(false);
        if (!mon.alive())
            return "doesn't survive its own turns";
        if (you.pos() != player)
            return "moves the player";
        if (grid_distance(mon.pos(), you.pos()) > distance)
            return "moves away from the player";
        ++tally.melee[melee_actions];
        ++tally.casts[spells_cast];
        ++tally.closed[distance - grid_distance(mon.pos(), you.pos())];
    }
    _restore_player();
    return nullptr;
}

// Kinds that do what the engine doesn't model in their own turns.
static set<monster_type> left_out;

static void _turn_case(int index, int trials, const turn_rule &rule)
{
    while (true)
    {
        vector<const turn_kind *> kinds;
        for (const turn_kind *kind : rule.kinds)
            if (!left_out.count(kind->type))
                kinds.push_back(kind);
        if (kinds.empty())
        {
            fprintf(stderr, "every kind of monster for the %s rule is left out\n", rule.name);
            exit(1);
        }
        const turn_kind &kind = *kinds[random2(kinds.size())];
        // Farther than any blast reaches back, where the weighing doesn't arise.
        const int nearest = _book_may_catch_caster(kind.book) ? 3 : 1;
        const int distance = nearest + random2(LOS_DEFAULT_RANGE - nearest + 1);
        monster *mon = _place(kind.type, distance);
        if (!mon)
            continue;
        mon->spells = kind.book;
        const string name = mon->name(DESC_PLAIN, true);
        const int hd = mon->get_hit_dice();
        const int speed = mon->speed;

        turn_tally tally;
        const char *unfollowed = _play_turns(*mon, distance, trials, tally);
        _clear_monsters();
        if (unfollowed)
        {
            // Back where it was, for the cases to come.
            you.set_position(coord_def(GXM / 2, GYM / 2));
            los_changed();
            fprintf(stderr, "%s %s: left out\n", name.c_str(), unfollowed);
            left_out.insert(kind.type);
            continue;
        }
        printf("{\"case\":%d,\"kind\":\"turn\",\"rule\":\"%s\",\"monster\":%d,\"name\":\"%s\","
               "\"hd\":%d,\"speed\":%d,\"distance\":%d,\"trials\":%d,"
               "\"melee\":%s,\"casts\":%s,\"closed\":%s}\n",
               index, rule.name, kind.type, name.c_str(), hd, speed, distance, trials,
               _histogram(tally.melee).c_str(), _histogram(tally.casts).c_str(),
               _histogram(tally.closed).c_str());
        return;
    }
}

int main(int argc, char *argv[])
{
    if (argc != 5)
    {
        fprintf(stderr, "usage: %s <seed> <cases> <trials> <ported spells>\n", argv[0]);
        return 2;
    }
    crawl_state.test = true;
    // Attacks and spells redraw the view, which needs a console: crawl's headless one.
    enter_headless_mode();
    console_startup();
    crawl_view.init_geometry();
    crawl_state.disables.set(DIS_DELAY);
    msg::suppress quiet;
    rng::seed(strtoull(argv[1], nullptr, 10));
    _initialise_crawl();
    you.where_are_you = BRANCH_DUNGEON;
    const set<spell_type> ported = _read_ported(argv[4]);

    printf("{\"crawl\":\"%s\",\"seed\":%s}\n", Version::Long, argv[1]);
    const int cases = atoi(argv[2]);
    const int trials = atoi(argv[3]);
    if (cases == 0)
        return 0;
    int index = 0;
    _block_cases(index);
    _reach_cases(index);
    const vector<turn_kind> kinds = _turn_kinds(ported);
    const vector<turn_rule> rules = _turn_rules(kinds);
    for (int i = 0; i < cases; ++i)
        _turn_case(index++, trials, rules[i % rules.size()]);
    return 0;
}

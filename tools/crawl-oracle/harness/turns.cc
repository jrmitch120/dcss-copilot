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
 *   its melee reaches (monster::reach_range) and which of its attacks it makes from each
 *   distance within that: those melee_attack::can_reach lets it make, but an attack only a
 *   weapon makes, which it doesn't make unarmed (attack::init_attack). A kind that uses its
 *   starting equipment and hits has a second reach case holding a +0 spear, a polearm, in its
 *   main hand, and when it wields two weapons a +0 dagger in its off hand.
 * - turn: a random monster a few squares from the player, awake, hunting them, with no items
 *   and no statuses and with its book cut down to the spells the engine ports, plays out the
 *   player's next standard-speed action many times. Each time it starts hunting the player,
 *   with the energy crawl leaves a monster that has acted (monster::drain_action_energy), with
 *   its hit points and its hit dice, which a spore attack spends, the player is restored and
 *   the floor around them laid again, and crawl's own handle_monsters runs its turns with you.time_taken at
 *   BASELINE_DELAY, attacking and casting for real. It tallies the melee
 *   actions it took against the player, the spells it cast and the squares it closed, less
 *   those it backed off. Turn cases take turns at drawing from kinds of monster that
 *   each exercise a rule: the slowest and the fastest that act, any, faster or slower than the
 *   player, stationary, reaching, breathing, casting an instant spell, casting a blast or
 *   cloud, casting a blinkbolt, faster than the player and casting a cloud, which it may cast
 *   again before the player's next action but for the clouds its last cast left, watched,
 *   stepping toward the player only from beside them or only by chance (_fungal_move_check),
 *   keeping its range (_set_firing_pos), and batty, flitting away after it attacks or casts
 *   (_handle_battiness), near enough to attack. A kind that keeps its range is drawn for that
 *   rule with its own book or with the book of another kind that casts a spell the engine
 *   ports, as none that keeps its range casts one in 0.34.1: such a book holds it at a firing
 *   position three squares from the player, from which it backs off when it starts nearer. A
 *   rule no kind exercises is left out.
 *
 * It switches off monster special abilities (mon_special_ability; NotModelled::SpecialAbilities),
 * which the engine doesn't port, and leaves out monsters that crawl moves otherwise than the
 * engine does, each named by the NotModelled variant the Fact Sheet gives it
 * (_walks_as_engine_does), draconians, whose breath their book doesn't hold, books crawl
 * fills when a monster is made (MST_GHOST), and shapeshifters, which take another kind's shape
 * as they are made. Any kind that dies in its own turns, moves the player
 * (NotModelled::MovesThePlayer, BlinksWithThePlayer) or moves away from them
 * (NotModelled::BlinksAway), but for one that keeps its range or is batty, which moves away on
 * purpose, is left out of the case it is found in, and
 * said so on standard error. A caster of an explosion or a cloud starts three squares or more
 * away: crawl's tracer weighs a caster's harm to itself (mons_should_fire), which the engine
 * doesn't port. A kind that blows itself up into torment as it attacks (mons_self_destructs,
 * explode_monster), the lurking horror, isn't left out for dying in its own turns: a play in
 * which it does so after a melee action counts it as having closed on the player, and it is
 * placed again. A caster of a blinkbolt has the square behind the player walled, so crawl lands
 * it beside them, as the engine does, and not past them; said so on standard error, and a case
 * that lands it anywhere else is left out.
 *
 * It includes mon-act.cc, to count what a monster does as it calls into the code that does it,
 * and links against a crawl build's objects without mon-act.o and main.o (oracle.mk). Compiled
 * with -fno-access-control.
 *
 * Usage: turns <seed> <cases> <trials> <ported spells> [<shard>/<count>]
 *   <ported spells> is a file of the spell_type values the engine ports, one per line, as
 *   `crawl-oracle ported-spells` prints them. <cases> counts the turn cases. The block and reach
 *   cases are the same every run and are printed by shard 0. Each turn case is seeded from
 *   <seed> and its index; with a shard, only its turn cases are drawn (common.h), by their
 *   number among the turn cases, each rule's dealt round the shards.
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

static void _block_cases(int &index, bool print)
{
    const vector<int> shields = { -1, ARM_BUCKLER, ARM_KITE_SHIELD, ARM_TOWER_SHIELD };
    for (int shield : shields)
    {
        _take_off(SLOT_OFFHAND);
        if (shield >= 0)
            _wear(1, SLOT_OFFHAND, static_cast<armour_type>(shield), 0);
        if (print)
        {
            printf("{\"case\":%d,\"kind\":\"block\",\"shield\":\"%s\",\"limit\":%d}\n",
                   index, shield < 0 ? "none" : make_stringf("%d:%d", OBJ_ARMOUR, shield).c_str(),
                   you.shield_block_limit());
        }
        ++index;
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
           && type != MONS_ASPIRING_FLESH
           // Its facets, given as it is made, may add attacks its kind lacks, such as a reaching
           // sting, which the engine doesn't model (#95).
           && type != MONS_MUTANT_BEAST;
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
    if (!mon->visible_to(&you) || !you.visible_to(mon))
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

// Whether any of `mon`'s damaging attacks may be made with a weapon (monster::weapon).
static bool _hits(const monster &mon)
{
    for (int i = 0; i < MAX_NUM_ATTACKS; ++i)
    {
        const mon_attack_def attk = mons_attack_spec(mon, i);
        if ((attk.type == AT_HIT || attk.type == AT_WEAP_ONLY) && attk.damage > 0)
            return true;
    }
    return false;
}

// Prints `mon`'s reach case, and counts it in `index`.
static void _reach_case(int &index, bool print, monster &mon)
{
    const int reach = mon.reach_range();
    string from = "{";
    for (int distance = 1; distance <= reach; ++distance)
    {
        string slots;
        for (int slot = 0; slot < MAX_NUM_ATTACKS; ++slot)
        {
            if (get_monster_data(mon.type)->attack[slot].type == AT_NONE)
                continue;
            melee_attack swing(&mon, &you, slot, slot);
            // An attack only a weapon makes, made without one (attack::init_attack).
            if (swing.attk_type == AT_NONE)
                continue;
            if (swing.can_reach(distance))
                slots += make_stringf("%s%d", slots.empty() ? "" : ",", slot);
        }
        from += make_stringf("%s\"%d\":[%s]", distance > 1 ? "," : "", distance,
                             slots.c_str());
    }
    if (print)
    {
        printf("{\"case\":%d,\"kind\":\"reach\",\"monster\":%d,\"name\":\"%s\","
               "\"main\":%s,\"off\":%s,\"reach\":%d,\"attacks_from\":%s}}\n",
               index, mon.type, mon.name(DESC_PLAIN, true).c_str(),
               _weapon_json(_held(mon, true)).c_str(), _weapon_json(_held(mon, false)).c_str(),
               reach, from.c_str());
    }
    ++index;
}

static void _reach_cases(int &index, bool print)
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
        _reach_case(index, print, *mon);
        if (mons_class_itemuse(type) >= MONUSE_STARTING_EQUIPMENT && _hits(*mon))
        {
            _give_weapon(*mon, WPN_SPEAR, 0, SPWPN_NORMAL);
            if (mons_wields_two_weapons(*mon))
                _give_weapon(*mon, WPN_DAGGER, 0, SPWPN_NORMAL);
            const item_def *main = _held(*mon, true);
            // Crawl may refuse a weapon, as a draconian monk refuses any.
            if (main && main->sub_type == WPN_SPEAR)
                _reach_case(index, print, *mon);
            mon->destroy_inventory();
        }
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

// Whether a monster with `book` casts a blinkbolt, which carries it along the bolt.
static bool _casts_blinkbolt(const monster_spells &book)
{
    return _any_slot(book,
                     [](const mon_spell_slot &slot) { return slot.spell == SPELL_BLINKBOLT; });
}

// The square behind the player from a monster east of them, walled for a caster of a blinkbolt.
// Crawl blinks it to the farthest square of the bolt's path it can stand on
// (bolt::affect_endpoint), which on open floor lies past the player, as the bolt pierces; the
// engine lands it beside them. The wall stops the bolt, which a blinkbolt doesn't bounce off
// (bolt::is_bouncy), so crawl lands it beside them too.
static coord_def _behind_player()
{
    return you.pos() + coord_def(-1, 0);
}

// Floor everywhere within sight of the player, the square behind them walled if `walled`.
static void _lay_terrain(bool walled)
{
    _reset_terrain();
    if (walled)
        env.grid(_behind_player()) = DNGN_ROCK_WALL;
}

// What a kind of monster that a turn case may draw is like.
struct turn_kind
{
    monster_type type;
    int speed;
    bool stationary;
    int reach;
    monster_spells book;
    // The kind whose book it casts from: its own, or another's.
    monster_type book_of;
};

// Kinds _fungal_move_check lets step toward a foe that watches them only from beside them, and
// only by chance, as Game Knowledge's series rules list them (WatchedStep).
static const set<monster_type> STEPS_FROM_BESIDE = { MONS_WANDERING_MUSHROOM, MONS_DEATHCAP };
static const set<monster_type> STEPS_BY_CHANCE = { MONS_LURKING_HORROR };

// Kinds explode_monster blows up into torment as they die, as Game Knowledge's series rules list
// them: they blow themselves up as they attack (mons_self_destructs), which the engine plays.
static const set<monster_type> TORMENTS_AS_IT_DIES = { MONS_LURKING_HORROR };

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
    { "blinkbolt", [](const turn_kind &k) { return _casts_blinkbolt(k.book); } },
    { "recasts cloud",
      [](const turn_kind &k) {
          return k.speed > 10 && _any_slot(k.book, [](const mon_spell_slot &slot) {
                     return bool(get_spell_flags(slot.spell) & spflag::cloud);
                 });
      } },
    { "steps from beside",
      [](const turn_kind &k) { return STEPS_FROM_BESIDE.count(k.type) > 0; } },
    { "steps by chance", [](const turn_kind &k) { return STEPS_BY_CHANCE.count(k.type) > 0; } },
};

// Kinds handle_monster_move acts or moves for with code of their own, rather than by walking,
// fighting and casting (NotModelled::OwnActions), as Game Knowledge's series rules list them.
static const set<monster_type> OWN_ACTIONS = {
    MONS_FULMINANT_PRISM, MONS_SHADOW_PRISM, MONS_SPLINTERFROST_BARRICADE, MONS_BLAZEHEART_CORE,
    MONS_BLAZEHEART_GOLEM, MONS_RENDING_BLADE, MONS_BURSTSHROOM, MONS_SIXFIRHY, MONS_JIANGSHI,
    MONS_BOULDER, MONS_HELLFIRE_MORTAR, MONS_DIAMOND_SAWBLADE, MONS_LIGHTNING_SPIRE,
    MONS_SOLAR_EMBER, MONS_BOUNDLESS_TESSERACT,
};

// Whether crawl moves a kind the way the engine does: straight at the player across the floor,
// unless it never moves or, watched, doesn't step (_fungal_move_check). Each kind it doesn't is
// one the Fact Sheet names by a NotModelled variant.
static bool _walks_as_engine_does(monster_type type)
{
    // OwnActions, with projectiles, which iood_act flies.
    return !OWN_ACTIONS.count(type) && !mons_is_projectile(type)
           // SpecialAbilities: a seeker bursts by its own, switched off here.
           && !mons_is_seeker(type)
           // MovesWithParent.
           && !mons_is_tentacle_or_tentacle_segment(type)
           // BoundToTerrain: a monster of lava or water can't leave it.
           && monster_habitable_feat(type, DNGN_FLOOR);
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
        // A first attack that swoops or flanks is NotModelled::SwoopOrFlank.
        if ((_has_melee(*mon) || !book.empty()) && first != AF_SWOOP && first != AF_FLANK)
        {
            kinds.push_back({ type, mon->speed, mon->is_stationary(), mon->reach_range(),
                              book, type });
        }
        _clear_monsters();
    }
    return kinds;
}

// Each kind among `kinds` that keeps its range, with the book of each other kind that casts a
// spell the engine ports and doesn't weigh its own safety or blink as it casts: a book no kind
// that keeps its range holds in 0.34.1, which holds it at a firing position away from the
// player (_set_firing_pos, monster::threat_range).
static vector<turn_kind> _borrowers(const vector<turn_kind> &kinds)
{
    vector<turn_kind> borrowing;
    for (const turn_kind &kind : kinds)
    {
        if (!mons_class_flag(kind.type, M_MAINTAIN_RANGE))
            continue;
        for (const turn_kind &lender : kinds)
        {
            if (lender.book.empty() || _book_may_catch_caster(lender.book)
                || _casts_blinkbolt(lender.book))
            {
                continue;
            }
            turn_kind borrower = kind;
            borrower.book = lender.book;
            borrower.book_of = lender.type;
            borrowing.push_back(borrower);
        }
    }
    return borrowing;
}

// A rule a turn case's monster exercises, and the kinds of monster that exercise it.
struct turn_rule
{
    const char *name;
    vector<const turn_kind *> kinds;
    // The farthest from the player its monster starts.
    int farthest;
};

// The farthest from the player a batty kind starts for its rule: near enough that the fastest,
// at speed 30, attacks before the player's next action and flits after it (_handle_battiness).
static const int BATTY_FARTHEST = 2;

// The rules some kind of monster exercises, drawing from `kinds` and, for the rule of keeping
// range, from the kinds in `borrowing` too. A batty kind starts for its rule no farther than
// BATTY_FARTHEST. A rule no kind exercises, such as an instant spell when the engine ports none,
// is left out, and said so.
static vector<turn_rule> _turn_rules(const vector<turn_kind> &kinds,
                                     const vector<turn_kind> &borrowing)
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
    turn_rule slow = { "slowest", {}, LOS_DEFAULT_RANGE };
    turn_rule fast = { "fastest", {}, LOS_DEFAULT_RANGE };
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
        turn_rule drawn = { rule.first, {}, LOS_DEFAULT_RANGE };
        for (const turn_kind &kind : kinds)
            if (rule.second(kind))
                drawn.kinds.push_back(&kind);
        if (drawn.kinds.empty())
            fprintf(stderr, "no kind of monster exercises the %s rule: left out\n", rule.first);
        else
            rules.push_back(drawn);
    }
    // Kinds that keep their range, drawn with their own book as often as with one they borrow.
    turn_rule keeping = { "keeps range", {}, LOS_DEFAULT_RANGE };
    for (const turn_kind &kind : kinds)
    {
        if (!mons_class_flag(kind.type, M_MAINTAIN_RANGE))
            continue;
        keeping.kinds.push_back(&kind);
        for (const turn_kind &borrower : borrowing)
        {
            if (borrower.type == kind.type)
            {
                keeping.kinds.push_back(&borrower);
                keeping.kinds.push_back(&kind);
            }
        }
    }
    if (keeping.kinds.empty())
        fprintf(stderr, "no kind of monster exercises the %s rule: left out\n", keeping.name);
    else
        rules.push_back(keeping);
    turn_rule batty = { "batty", {}, BATTY_FARTHEST };
    for (const turn_kind &kind : kinds)
        if (mons_class_flag(kind.type, M_BATTY))
            batty.kinds.push_back(&kind);
    if (batty.kinds.empty())
        fprintf(stderr, "no kind of monster exercises the %s rule: left out\n", batty.name);
    else
        rules.push_back(batty);
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
// squares away, each time as it starts, the player under a corona if `corona` and walled behind
// if `walled`. Returns why the engine can't follow it, or nullptr if it can: a monster may burst
// or crumble in its own turns, move the player, as a trampling, dragging or blinking hit does
// (NotModelled::MovesThePlayer, BlinksWithThePlayer), move away from them, as a blinking hit
// does (NotModelled::BlinksAway), or cast a blinkbolt and land anywhere but beside them.
static const char *_play_turns(monster *mon, const turn_kind &kind, int distance, bool corona,
                               bool walled, int trials, turn_tally &tally)
{
    const coord_def start = mon->pos();
    const coord_def player = you.pos();
    const int hit_dice = mon->get_hit_dice();
    for (int t = 0; t < trials; ++t)
    {
        _clear_monsters(mon);
        _restore_player();
        if (corona)
            you.duration[DUR_CORONA] = 1000;
        _lay_terrain(walled);
        delete_all_clouds();
        if (mon->pos() != start)
            mon->move_to(start);
        _clear_statuses(*mon);
        mon->hit_points = mon->max_hit_points;
        // A spore attack costs its attacker a hit die (melee_attack::mons_apply_attack_flavour).
        // Set directly: monster::set_hit_dice also resets an oklob plant's spitting frequency.
        mon->hit_dice = hit_dice;
        mon->behaviour = BEH_SEEK;
        mon->foe = MHITYOU;
        mon->target = you.pos();
        // One that keeps its range finds its firing position afresh.
        mon->firing_pos.reset();
        // A batty one that flitted after it attacked hunts the player again.
        mon->props.erase(BATTY_TURNS_KEY);
        // As crawl leaves a monster that has acted.
        mon->speed_increment = 200;
        mon->drain_action_energy();

        melee_actions = 0;
        spells_cast = 0;
        you.time_taken = BASELINE_DELAY;
        handle_monsters(false);
        if (!mon->alive())
        {
            // One that blows itself up into torment as it attacks does so from beside the
            // player, and is placed again for the next play.
            if (!TORMENTS_AS_IT_DIES.count(kind.type) || melee_actions == 0)
                return "doesn't survive its own turns";
            ++tally.melee[melee_actions];
            ++tally.casts[spells_cast];
            ++tally.closed[distance - 1];
            mon = _place(kind.type, distance);
            if (!mon)
                return "isn't placed again after blowing itself up";
            mon->spells = kind.book;
            continue;
        }
        if (you.pos() != player)
            return "moves the player";
        // One that keeps its range backs off to its firing position, and a batty one flits
        // away, as the engine plays them.
        if (grid_distance(mon->pos(), you.pos()) > distance
            && !mons_class_flag(kind.type, M_MAINTAIN_RANGE)
            && !mons_class_flag(kind.type, M_BATTY))
        {
            return "moves away from the player";
        }
        if (walled && spells_cast > 0 && grid_distance(mon->pos(), you.pos()) > 1)
        {
            return "lands its blinkbolt away from the player";
        }
        ++tally.melee[melee_actions];
        ++tally.casts[spells_cast];
        ++tally.closed[distance - grid_distance(mon->pos(), you.pos())];
    }
    _restore_player();
    return nullptr;
}

// Kinds already said to be left out or walled behind the player, so each is said once.
static set<monster_type> reported;
static set<monster_type> walled_for;

static void _turn_case(int index, int trials, const turn_rule &rule)
{
    // Kinds found doing what the engine doesn't model in their own turns. Kept to this case, so
    // the case doesn't depend on what the cases before it found.
    set<monster_type> left_out;
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
        const int farthest = max(nearest, rule.farthest);
        const int distance = nearest + random2(farthest - nearest + 1);
        monster *mon = _place(kind.type, distance);
        if (!mon)
            continue;
        mon->spells = kind.book;
        const string name = mon->name(DESC_PLAIN, true);
        const int hd = mon->get_hit_dice();
        const int speed = mon->speed;
        const bool nightvision = mon->nightvision();
        _random_light();
        // Restoring the player between plays ends a corona, so each play lights it again.
        const bool corona = you.duration[DUR_CORONA] > 0;
        // As the monster starts; a halo of its own may reach the player only as it closes, which
        // changes no tally here.
        const char *lighting = _lighting();

        const bool walled = _casts_blinkbolt(kind.book);
        if (walled)
        {
            _lay_terrain(true);
            los_changed();
            if (walled_for.insert(kind.type).second)
                fprintf(stderr, "%s casts a blinkbolt: walled behind the player\n", name.c_str());
        }

        turn_tally tally;
        const char *unfollowed = _play_turns(mon, kind, distance, corona, walled, trials, tally);
        _clear_monsters();
        if (walled)
        {
            _lay_terrain(false);
            los_changed();
        }
        if (unfollowed)
        {
            // Back where it was, for the next kind this case draws.
            you.set_position(coord_def(GXM / 2, GYM / 2));
            los_changed();
            if (reported.insert(kind.type).second)
                fprintf(stderr, "%s %s: left out\n", name.c_str(), unfollowed);
            left_out.insert(kind.type);
            continue;
        }
        printf("{\"case\":%d,\"kind\":\"turn\",\"rule\":\"%s\",\"monster\":%d,\"name\":\"%s\","
               "\"book\":%d,\"hd\":%d,\"speed\":%d,\"distance\":%d,\"nightvision\":%s,"
               "\"lighting\":\"%s\",\"hp\":%d,\"trials\":%d,\"melee\":%s,\"casts\":%s,"
               "\"closed\":%s}\n",
               index, rule.name, kind.type, name.c_str(), kind.book_of, hd, speed, distance,
               _json_bool(nightvision), lighting, you.hp_max, trials,
               _histogram(tally.melee).c_str(), _histogram(tally.casts).c_str(),
               _histogram(tally.closed).c_str());
        return;
    }
}

// The hit points a torment case gives the player before each lurking horror attacks them.
static const vector<int> TORMENT_HP = { 1, 2, 3, 4, 10, 21, 40, 99, 250, 500 };

// Lets a lurking horror beside the player attack them on each of TORMENT_HP, blowing itself up
// into torment (mons_self_destructs, explode_monster), and prints the hit points each attack
// took, with what about the player the engine reads: their resistances, species and god
// (_resists_json), and the status lights of their form, `form_light`, and of petrification.
static void _torment_case(int &index, const char *form_light)
{
    string lost = "{";
    for (int hp : TORMENT_HP)
    {
        monster *mon = _place(MONS_LURKING_HORROR, 1);
        if (!mon || hp > you.hp_max)
        {
            fprintf(stderr, "no lurking horror beside a player on %d HP\n", hp);
            exit(1);
        }
        you.hp = hp;
        fight_melee(mon, &you);
        if (mon->alive())
        {
            fprintf(stderr, "a lurking horror attacked without blowing itself up\n");
            exit(1);
        }
        lost += make_stringf("%s\"%d\":%d", lost.size() > 1 ? "," : "", hp, hp - you.hp);
        _clear_monsters();
    }
    printf("{\"case\":%d,\"kind\":\"torment\",\"resists\":%s,\"form\":%s,\"petrified\":%s,"
           "\"lost\":%s}}\n",
           index, _resists_json().c_str(),
           form_light ? make_stringf("\"%s\"", form_light).c_str() : "null",
           _json_bool(you.petrified()), lost.c_str());
    ++index;
}

// The torment cases: each species the harnesses draw at each level of rNeg its mutation gives,
// then a human in each form, and a petrified human.
static void _torment_cases(int &index)
{
    const species_type species = you.species;
    for (species_type drawn : SPECIES)
    {
        you.species = drawn;
        for (int negative = 0; negative <= 3; ++negative)
        {
            you.mutation[MUT_NEGATIVE_ENERGY_RESISTANCE] = negative;
            _torment_case(index, nullptr);
        }
    }
    you.species = SP_HUMAN;
    you.mutation[MUT_NEGATIVE_ENERGY_RESISTANCE] = 0;
    for (int form = 1; form < NUM_TRANSFORMS; ++form)
    {
        you.form = static_cast<transformation>(form);
        _torment_case(index, get_form()->short_name.c_str());
    }
    you.form = transformation::none;
    you.duration[DUR_PETRIFIED] = 1000;
    _torment_case(index, nullptr);
    you.species = species;
    _restore_player();
}

int main(int argc, char *argv[])
{
    const oracle_run run = _read_run(argc, argv, true);
    crawl_state.test = true;
    // Attacks and spells redraw the view, which needs a console: crawl's headless one.
    enter_headless_mode();
    console_startup();
    crawl_view.init_geometry();
    crawl_state.disables.set(DIS_DELAY);
    msg::suppress quiet;
    rng::seed(run.seed);
    _initialise_crawl();
    const set<spell_type> ported = _read_ported(run.ported);

    _print_header(run);
    if (run.cases == 0)
        return 0;
    // Every shard plays the block and reach cases and finds the kinds turn cases draw from, from
    // the run's seed, so the turn cases are numbered and drawn alike in each.
    int index = 0;
    _block_cases(index, run.shard == 0);
    _reach_cases(index, run.shard == 0);
    const vector<turn_kind> kinds = _turn_kinds(ported);
    const vector<turn_kind> borrowing = _borrowers(kinds);
    const vector<turn_rule> rules = _turn_rules(kinds, borrowing);
    for (int i = 0; i < run.cases; ++i, ++index)
    {
        if (!_in_shard(run, i, static_cast<int>(rules.size())))
            continue;
        _start_case(run, index);
        _turn_case(index, run.trials, rules[i % rules.size()]);
    }
    // After the turn cases, which keep their numbers; the same every run. They start with the
    // player as crawl was set up, as a turn case may leave their maximum hit points lowered.
    if (run.shard == 0)
    {
        _start_case(run, index);
        _torment_cases(index);
    }
    return 0;
}

/*
 * The crawl oracle's speed harness: runs crawl's own code for the time the player's actions and
 * moves take, and prints what came out, one JSON object per line, for `crawl-oracle` to diff
 * against the engine's port.
 *
 * Each case makes the player one of SPEED_SPECIES at random, at a random experience level with the
 * mutations their species has by then, and gives them at random: more levels of each mutation
 * that changes their speed, some of them transient; the bane of lethargy; a form; haste, slow,
 * a stat at zero, a berserker rage, swiftness or the sluggishness after it, being frozen,
 * petrifying, liquefying the ground, flight and paralysis; Cheibriados, at the most piety a rank
 * of stars holds, or Beogh, at any piety, either under penance or not; body armour of
 * ponderousness, worn or only carried, and for a species that wears barding, the lightning
 * scales; and shallow or deep water underfoot. It then tallies, many times, player_speed(),
 * player_movement_speed() and the time a move takes, div_rand_round(player_speed() *
 * player_movement_speed(), BASELINE_DELAY), as move_player_action rolls it.
 *
 * It prints the player as the game shows them, which is all the engine reads: their species as
 * the game names it, their experience level, the light of each status the status pane lists
 * (fill_status_info, as tileweb.cc sends them), their god, piety rank and penance as the status
 * pane shows them, the rows of the mutations screen (A) as mutation_desc and bane_desc describe
 * them, each item they carry by name with whether they wear it, and the feature underfoot.
 *
 * A form is set as the player's without transforming, so nothing is melded. It links against a
 * crawl build's objects in place of main.o (oracle.mk). Compiled with -fno-access-control.
 *
 * Usage: speed <seed> <cases> <trials> [<shard>/<count>]
 *   Each case is seeded from <seed> and its index; with a shard, only its cases are drawn
 *   (common.h).
 */

#include "common.h"

#include "artefact.h"
#include "mutation.h"
#include "status.h"
#include "transform.h"

// The species a player is drawn as: of each size and swimming that water tells apart, with the
// mutations that slow them and with barding.
static const vector<species_type> SPEED_SPECIES = {
    SP_HUMAN, SP_NAGA, SP_MERFOLK, SP_OCTOPODE, SP_TROLL, SP_FELID, SP_SPRIGGAN, SP_ARMATAUR,
};

// The mutations that change the player's speed, each with the most levels it has.
static const vector<pair<mutation_type, int>> SPEED_MUTATIONS = {
    { MUT_FAST, 3 }, { MUT_SLOW, 3 }, { MUT_FROG_LEGS, 2 }, { MUT_CONSTRICTING_TAIL, 2 },
    { MUT_NIMBLE_SWIMMER, 2 },
};

// The most piety each rank of stars holds, from none: one short of the next piety_breakpoint, and
// the most a follower can have for six stars.
static const vector<int> MOST_PIETY = { 29, 49, 74, 99, 119, 159, MAX_PIETY };

// The features a case stands the player on.
static const vector<dungeon_feature_type> UNDERFOOT = {
    DNGN_FLOOR, DNGN_SHALLOW_WATER, DNGN_DEEP_WATER,
};

// `text` as a JSON string.
static string _json_string(const string &text)
{
    string out = "\"";
    for (char c : text)
    {
        if (c == '"' || c == '\\')
            out += '\\';
        out += c;
    }
    return out + "\"";
}

// `texts` as a JSON array of strings.
static string _json_strings(const vector<string> &texts)
{
    string out = "[";
    for (const string &text : texts)
    {
        if (out.size() > 1)
            out += ",";
        out += _json_string(text);
    }
    return out + "]";
}

// Gives the player their species' mutations up to their experience level, as crawl gives them
// as they level up, and nothing else.
static void _innate_mutations()
{
    for (int i = 0; i < NUM_MUTATIONS; ++i)
        you.mutation[i] = you.innate_mutation[i] = you.temp_mutation[i] = 0;
    for (const auto &gained : get_species_def(you.species).level_up_mutations)
    {
        if (gained.xp_level > you.experience_level)
            continue;
        you.mutation[gained.mut] += gained.mut_level;
        you.innate_mutation[gained.mut] += gained.mut_level;
    }
}

// Gives the player, a third of the time each, more levels of each mutation that changes their
// speed, up to its most, a quarter of them transient.
static void _random_speed_mutations()
{
    for (const auto &[mut, most] : SPEED_MUTATIONS)
    {
        if (!one_chance_in(3) || you.mutation[mut] >= most)
            continue;
        const int more = 1 + random2(most - you.mutation[mut]);
        you.mutation[mut] += more;
        if (one_chance_in(4))
            you.temp_mutation[mut] += more;
    }
    if (one_chance_in(4))
        you.banes[BANE_LETHARGY] = 100;
}

// Half the time puts the player in a form at random, without melding anything.
static void _random_form()
{
    you.form = transformation::none;
    if (coinflip())
    {
        you.form = static_cast<transformation>(1 + random2(NUM_TRANSFORMS - 1));
        // The status pane lights a form while it lasts.
        you.duration[DUR_TRANSFORMATION] = 100;
    }
}

// Gives the player each status that changes their speed a quarter of the time, and paralysis
// one time in twenty.
static void _random_statuses()
{
    for (duration_type dur : { DUR_HASTE, DUR_SLOW, DUR_BERSERK, DUR_FROZEN, DUR_PETRIFYING,
                               DUR_LIQUEFYING, DUR_FLIGHT })
    {
        if (one_chance_in(4))
            you.duration[dur] = 100;
    }
    if (one_chance_in(4))
        you.attribute[ATTR_STAT_ZERO] = 1;
    if (one_chance_in(4))
    {
        you.duration[DUR_SWIFTNESS] = 100;
        // Swift while the spell lasts, sluggish as it wears off.
        you.attribute[ATTR_SWIFTNESS] = coinflip() ? 100 : -1;
    }
    if (one_chance_in(20))
        you.duration[DUR_PARALYSIS] = 100;
}

// Makes the player, a third of the time each, a follower of no god, of Cheibriados at the most
// piety a random rank holds, or of Beogh at any piety, either under penance half the time.
static void _random_god()
{
    switch (random2(3))
    {
    case 0:
        you.religion = GOD_NO_GOD;
        return;
    case 1:
        you.religion = GOD_CHEIBRIADOS;
        you.raw_piety = MOST_PIETY[random2(MOST_PIETY.size())];
        break;
    default:
        you.religion = GOD_BEOGH;
        you.raw_piety = random2(MAX_PIETY + 1);
        break;
    }
    you.penance[you.religion] = coinflip() ? 10 : 0;
}

// The item in inventory slot `slot`, made identified and carried.
static item_def &_carried(int slot)
{
    item_def &item = you.inv[slot];
    item.quantity = 1;
    item.link = slot;
    item.pos = ITEM_IN_INVENTORY;
    item.flags |= ISFLAG_IDENTIFIED;
    return item;
}

// Gives the player, a third of the time, plate armour of ponderousness, which they wear half the
// time and otherwise only carry; and a species that wears barding the lightning scales half the
// time. Returns each item carried by name, with whether it is worn.
static vector<pair<string, bool>> _random_gear()
{
    vector<pair<string, bool>> carried;
    if (one_chance_in(3))
    {
        item_def &armour = _carried(0);
        armour.base_type = OBJ_ARMOUR;
        armour.sub_type = ARM_PLATE_ARMOUR;
        armour.brand = SPARM_PONDEROUSNESS;
        const bool worn = coinflip();
        if (worn)
            equip_item(SLOT_BODY_ARMOUR, 0, false, true);
        carried.emplace_back(armour.name(DESC_PLAIN), worn);
    }
    if (species::wears_barding(you.species) && coinflip())
    {
        item_def &scales = you.inv[1];
        make_item_unrandart(scales, UNRAND_LIGHTNING_SCALES);
        _carried(1);
        equip_item(SLOT_BARDING, 1, false, true);
        carried.emplace_back(scales.name(DESC_PLAIN),
                             you.unrand_equipped(UNRAND_LIGHTNING_SCALES));
    }
    return carried;
}

// The lights of the statuses the status pane lists, in its order.
static vector<string> _lights()
{
    vector<string> lights;
    status_info inf;
    for (unsigned int status = 0; status <= STATUS_LAST_STATUS; ++status)
        if (fill_status_info(status, inf) && !inf.light_text.empty())
            lights.push_back(inf.light_text);
    return lights;
}

// The rows of the mutations screen: each mutation the player has, as mutation_desc describes it
// there, and each bane, with the experience levels until it lifts (MutationMenu).
static vector<string> _mutations_screen()
{
    vector<string> rows;
    for (int i = 0; i < NUM_MUTATIONS; ++i)
    {
        const mutation_type mut = static_cast<mutation_type>(i);
        if (you.get_base_mutation_level(mut) > 0)
            rows.push_back(mutation_desc(mut, -1, false));
    }
    for (int i = 0; i < NUM_BANES; ++i)
    {
        const bane_type bane = static_cast<bane_type>(i);
        if (you.has_bane(bane))
        {
            rows.push_back(make_stringf("%s [%.1f]", bane_desc(bane).c_str(),
                                        (float)xl_to_remove_bane(bane) / 10.0f));
        }
    }
    return rows;
}

// The player's god as the status pane shows them, or null.
static string _god_json()
{
    if (you_worship(GOD_NO_GOD))
        return "null";
    return make_stringf("{\"name\":%s,\"piety_rank\":%d,\"penance\":%s}",
                        _json_string(god_name(you.religion)).c_str(), max(0, piety_rank()),
                        _json_bool(player_under_penance()));
}

static void _speed_case(int index, int trials)
{
    you.species = SPEED_SPECIES[random2(SPEED_SPECIES.size())];
    you.experience_level = 1 + random2(27);
    // As much experience as the level takes, which the levels until a bane lifts count from.
    you.experience = you.total_experience = exp_needed(you.experience_level);
    _innate_mutations();
    _random_speed_mutations();
    _random_form();
    _random_statuses();
    _random_god();
    const vector<pair<string, bool>> carried = _random_gear();
    env.grid(you.pos()) = UNDERFOOT[random2(UNDERFOOT.size())];

    map<int, int> action, delay, time;
    for (int t = 0; t < trials; ++t)
    {
        action[player_speed()]++;
        delay[player_movement_speed()]++;
        time[div_rand_round(player_speed() * player_movement_speed(), BASELINE_DELAY)]++;
    }

    string carried_json = "[";
    for (const auto &[name, worn] : carried)
    {
        if (carried_json.size() > 1)
            carried_json += ",";
        carried_json += make_stringf("{\"name\":%s,\"worn\":%s}", _json_string(name).c_str(),
                                     _json_bool(worn));
    }
    carried_json += "]";
    printf("{\"case\":%d,\"kind\":\"speed\",\"species\":%s,\"xl\":%d,\"lights\":%s,"
           "\"god\":%s,\"mutations\":%s,\"carried\":%s,\"feature\":%d,\"trials\":%d,"
           "\"action\":%s,\"delay\":%s,\"time\":%s}\n",
           index, _json_string(species::name(you.species)).c_str(), you.experience_level,
           _json_strings(_lights()).c_str(), _god_json().c_str(),
           _json_strings(_mutations_screen()).c_str(), carried_json.c_str(),
           static_cast<int>(env.grid(you.pos())), trials, _histogram(action).c_str(),
           _histogram(delay).c_str(), _histogram(time).c_str());
}

int main(int argc, char *argv[])
{
    const oracle_run run = _read_run(argc, argv, false);
    crawl_state.test = true;
    msg::suppress quiet;
    rng::seed(run.seed);
    _initialise_crawl();
    // What crawl's startup does besides, for the mutations screen and the status lights.
    init_mut_index();
    init_duration_index();

    _print_header(run);
    for (int i = 0; i < run.cases; ++i)
    {
        if (!_in_shard(run, i, 1))
            continue;
        _start_case(run, i);
        _speed_case(i, run.trials);
    }
    return 0;
}

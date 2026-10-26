// Trimmed from crawl 0.34.1's skills.cc: the messages a skill's level changes with, and the
// table of skill titles, whose first column names each skill.

#include "tag-version.h"

static const char *skill_titles[NUM_SKILLS][7] =
{
  //  Skill name        levels 1-7       levels 8-14        levels 15-20       levels 21-26      level 27       skill abbr
    {"Fighting",       "Trooper",       "Fighter",         "Warrior",         "Slayer",         "Conqueror",    "Fgt"},
    {"Short Blades",   "Cutter",        "Slicer",          "Swashbuckler",    "Cutthroat",      "Politician",   "SBl"},
    {"Long Blades",    "Slasher",       "Carver",          "Fencer",          "@Adj@ Blade",    "Swordmaster",  "LBl"},
    {"Axes",           "Chopper",       "Cleaver",         "Severer",         "Executioner",    "Axe Maniac",   "Axs"},
    {"Maces & Flails", "Cudgeller",     "Basher",          "Bludgeoner",      "Shatterer",      "Skullcrusher", "M&F"},
    {"Polearms",       "Poker",         "Spear-Bearer",    "Impaler",         "Phalangite",     "@Adj@ Porcupine", "Pla"},
    {"Staves",         "Twirler",       "Cruncher",        "Stickfighter",    "Pulveriser",     "Chief of Staff", "Stv"},
#if TAG_MAJOR_VERSION == 34
    {"Slings",         "Vandal",        "Slinger",         "Whirler",         "Slingshot",      "@Adj@ Catapult", "Slg"},
#endif
    {"Ranged Weapons", "Shooter",       "Skirmisher",      "Marks@genus@",    "Crack Shot",     "Merry @Genus@",  "Rng"},
#if TAG_MAJOR_VERSION == 34
    {"Crossbows",      "Bolt Thrower",  "Quickloader",     "Sharpshooter",    "Sniper",         "@Adj@ Arbalest", "Crb"},
#endif
    {"Throwing",       "Chucker",       "Thrower",         "Deadly Accurate", "Hawkeye",        "@Adj@ Ballista", "Thr"},
    {"Armour",         "Covered",       "Protected",       "Tortoise",        "Impregnable",    "Invulnerable", "Arm"},
    {"Dodging",        "Ducker",        "Nimble",          "Spry",            "Acrobat",        "Intangible",   "Ddg"},
    {"Stealth",        "Sneak",         "Covert",          "Unseen",          "Imperceptible",  "Ninja",        "Sth"},
#if TAG_MAJOR_VERSION == 34
    {"Stabbing",       "Miscreant",     "Blackguard",      "Backstabber",     "Cutthroat",      "Politician",   "Stb"},
#endif
    {"Shields",        "Shield-Bearer", "Blocker",         "Peltast",         "Hoplite",        "@Adj@ Barricade", "Shd"},
#if TAG_MAJOR_VERSION == 34
    {"Traps",          "Scout",         "Disarmer",        "Vigilant",        "Perceptive",     "Dungeon Master", "Trp"},
#endif
    // STR based fighters, for DEX/martial arts titles see below. Felids get their own category, too.
    {"Unarmed Combat", "Ruffian",       "Grappler",        "Brawler",         "Wrestler",       "@Weight@weight Champion", "UC"},

    {"Spellcasting",   "Magician",      "Thaumaturge",     "Eclecticist",     "Sorcerer",       "Archmage",     "Spc"},
    {"Conjurations",   "Conjurer",      "Destroyer",       "Devastator",      "Ruinous",        "Annihilator",  "Conj"},
    {"Hexes",          "Vexing",        "Jinx",            "Bewitcher",       "Maledictor",     "Spellbinder",  "Hex"},
#if TAG_MAJOR_VERSION == 34
    {"Charms",         "Charmwright",   "Infuser",         "Anointer",        "Gracecrafter",   "Miracle Worker", "Chrm"},
#endif
    {"Summonings",     "Caller",        "Summoner",        "Convoker",        "Worldbinder",    "Planerender",  "Summ"},
    {"Necromancy",     "Grave Robber",  "Reanimator",      "Necromancer",     "Thanatomancer",  "@Genus_Short@ of Death", "Necr"},
    {"Translocations", "Grasshopper",   "Placeless @Genus@", "Blinker",       "Portalist",      "Plane @Walker@", "Tloc"},
    {"Forgecraft",     "Tinkerer",      "Fabricator",      "Mechanist",       "Siegecrafter",   "Architect of Ages", "Frge"},

    {"Fire Magic",     "Firebug",       "Arsonist",        "Scorcher",        "Pyromancer",     "Infernalist",  "Fire"},
    {"Ice Magic",      "Chiller",       "Frost Mage",      "Gelid",           "Cryomancer",     "Englaciator",  "Ice"},
    {"Air Magic",      "Gusty",         "Zephyrmancer",    "Stormcaller",     "Cloud Mage",     "Meteorologist", "Air"},
    {"Earth Magic",    "Digger",        "Geomancer",       "Earth Mage",      "Metallomancer",  "Petrodigitator", "Erth"},
    {"Alchemy",        "Apothecary",    "Toxicologist",    "Hermetic",        "Philosopher",    "Quintessent", "Alch"},

    // These titles apply to atheists only, worshippers of the various gods
    // use the god titles instead, depending on piety or, in Gozag's case, gold.
    // or, in U's case, invocations skill.
    {"Invocations",    "Unbeliever",    "Agnostic",        "Dissident",       "Heretic",        "Apostate",     "Invo"},
    {"Evocations",     "Charlatan",     "Prestidigitator", "Fetichist",       "Evocator",       "Ex Machina",  "Evo"},
    {"Shapeshifting",  "Changeling",    "Mimic",           "Metamorph",       "Skinwalker",     "Shapeless @Genus@", "Shft"},
};

// (trimmed for the extractor test)

static void _change_skill_level(skill_type exsk, int n)
{
    ASSERT(n != 0);
    bool need_reset = false;

    you.skills[exsk] = max(0, you.skills[exsk] + n);

    take_note(Note(n > 0 ? NOTE_GAIN_SKILL : NOTE_LOSE_SKILL,
                   exsk, you.skills[exsk]));

    // are you drained/crosstrained/ash'd in the relevant skill?
    const bool specify_base = you.skill(exsk, 1) != you.skill(exsk, 1, true);
    if (you.skills[exsk] == MAX_SKILL_LEVEL)
        mprf(MSGCH_INTRINSIC_GAIN, "You have mastered %s!", skill_name(exsk));
    else if (abs(n) == 1 && you.num_turns)
    {
        mprf(MSGCH_INTRINSIC_GAIN, "Your %s%s skill %s to level %d!",
             specify_base ? "base " : "",
             skill_name(exsk), (n > 0) ? "increases" : "decreases",
             you.skills[exsk]);
    }
    else if (you.num_turns)
    {
        mprf(MSGCH_INTRINSIC_GAIN, "Your %s%s skill %s %d levels and is now "
             "at level %d!",
             specify_base ? "base " : "",
             skill_name(exsk),
             (n > 0) ? "gained" : "lost",
             abs(n), you.skills[exsk]);
    }

    // (trimmed for the extractor test)
}

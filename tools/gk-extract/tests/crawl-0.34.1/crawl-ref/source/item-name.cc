/**
 * @file
 * @brief Misc functions.
**/

// Trimmed from crawl 0.34.1 for the extractor test: each function keeps a few
// of its cases.

#include "AppHdr.h"

#include "item-name.h"

static const char* _wand_type_name(int wandtype)
{
    switch (wandtype)
    {
    case WAND_FLAME:           return "flame";
    case WAND_PARALYSIS:       return "paralysis";
    default:                   return item_type_removed(OBJ_WANDS, wandtype)
                                    ? "removedness"
                                    : "bugginess";
    }
}

const char* potion_type_name(int potiontype)
{
    switch (static_cast<potion_type>(potiontype))
    {
    case POT_CURING:            return "curing";
    case POT_HEAL_WOUNDS:       return "heal wounds";
    case POT_HASTE:             return "haste";
    case POT_INVISIBILITY:      return "invisibility";
    case POT_BERSERK_RAGE:      return "berserk rage";

    // FIXME: Remove this once known-items no longer uses this as a sentinel.
    default:
                                return "bugginess";
    CASE_REMOVED_POTIONS(potiontype); // TODO: this will crash, is that correct??
    }
}

const char* scroll_type_name(int scrolltype)
{
    switch (static_cast<scroll_type>(scrolltype))
    {
    case SCR_IDENTIFY:           return "identify";
    case SCR_TELEPORTATION:      return "teleportation";
    case SCR_FEAR:               return "fear";
    case SCR_BLINKING:           return "blinking";
    case SCR_FOG:                return "fog";
#if TAG_MAJOR_VERSION == 34
    case SCR_HOLY_WORD:          return "holy word";
#endif
    default:                     return item_type_removed(OBJ_SCROLLS,
                                                          scrolltype)
                                     ? "removedness"
                                     : "bugginess";
    }
}

const char* jewellery_effect_name(int jeweltype, bool terse)
{
    if (!terse)
    {
        switch (static_cast<jewellery_type>(jeweltype))
        {
        case RING_PROTECTION:            return "protection";
        case AMU_FAITH:                  return "faith";
        default: return "buggy jewellery";
        }
    }
    else
    {
        switch (static_cast<jewellery_type>(jeweltype))
        {
        case RING_PROTECTION:            return "AC";
        case AMU_FAITH:                  return "Faith";
        default: return "buggy";
        }
    }
}

const char* rune_type_name(short p)
{
    switch (static_cast<rune_type>(p))
    {
    case RUNE_DIS:         return "iron";
    case RUNE_GEHENNA:     return "obsidian";
    default:               return "buggy";
    }
}

static const char* _book_type_name(int booktype)
{
    switch (static_cast<book_type>(booktype))
    {
    case BOOK_MINOR_MAGIC:            return "Minor Magic";
    case BOOK_CONJURATIONS:           return "Conjurations";
    default:                          return "Bugginess";
    }
}

static string misc_type_name(int type)
{
#if TAG_MAJOR_VERSION == 34
    if (is_deck_type(type))
        return "removed deck";
#endif

    switch (static_cast<misc_item_type>(type))
    {
    case MISC_BOX_OF_BEASTS:             return "box of beasts";
    case MISC_HORN_OF_GERYON:            return "horn of Geryon";
    default:
        return "buggy miscellaneous item";
    }
}

const char* special_armour_type_name(special_armour_type ego, bool terse)
{
    if (!terse)
    {
        switch (ego)
        {
        case SPARM_NORMAL:            return "";
        case SPARM_PONDEROUSNESS:     return "ponderousness";
        case SPARM_LIGHT:             return "light";
        default:                      return "bugginess";
        }
    }
    return "buggy";
}
static const char *weapon_brands_terse[] =
{
    "", "flame", "freeze", "holy", "elec",
#if TAG_MAJOR_VERSION == 34
    "obsolete", "obsolete",
#endif
    "venom", "protect", "drain", "speed", "heavy",
#if TAG_MAJOR_VERSION == 34
    "obsolete", "obsolete",
#endif
    "vamp", "pain", "antimagic", "distort",
#if TAG_MAJOR_VERSION == 34
    "obsolete", "obsolete",
#endif
    "chaos",
#if TAG_MAJOR_VERSION == 34
    "obsolete", "confuse",
#endif
    "penet", "reap", "spect", "rebuke", "valour",
    "entangle", "sunder", "concuss", "devious",
    "num_special", "acid",
#if TAG_MAJOR_VERSION > 34
    "confuse",
#endif
    "weak",
    "vuln",
    "foul flame",
    "debug",
};

static const char *weapon_brands_verbose[] =
{
    "", "flaming", "freezing", "holy wrath", "electrocution",
#if TAG_MAJOR_VERSION == 34
    "obsolescence", "obsolescence",
#endif
    "venom", "protection", "draining", "speed", "heavy",
#if TAG_MAJOR_VERSION == 34
    "obsolescence", "obsolescence",
#endif
    "vampirism", "pain", "antimagic", "distortion",
#if TAG_MAJOR_VERSION == 34
    "obsolescence", "obsolescence",
#endif
    "chaos",
#if TAG_MAJOR_VERSION == 34
    "obsolescence", "confusion",
#endif
    "penetration", "reaping", "spectralising", "rebuke", "valour",
    "entangling", "sundering", "concussion", "devious",
    "num_special", "acid",
#if TAG_MAJOR_VERSION > 34
    "confusion",
#endif
    "weakness",
    "vulnerability",
    "foul flame",
    "debug",
};

static const char *weapon_brands_adj[] =
{
    "", "flaming", "freezing", "holy", "electric",
#if TAG_MAJOR_VERSION == 34
    "obsolete", "obsolete",
#endif
    "venomous", "protective", "draining", "fast", "heavy",
#if TAG_MAJOR_VERSION == 34
    "obsolete", "obsolete",
#endif
    "vampiric", "painful", "antimagic", "distorting",
#if TAG_MAJOR_VERSION == 34
    "obsolete", "obsolete",
#endif
    "chaotic",
#if TAG_MAJOR_VERSION == 34
    "obsolete", "confusing",
#endif
    "penetrating", "reaping", "spectral", "rebuking", "valourous",
    "entangling", "sundering", "concussing", "devious",
    "num_special", "acidic",
#if TAG_MAJOR_VERSION > 34
    "confusing",
#endif
    "weakening",
    "will-reducing",
    "foul flame",
    "debug",
};

COMPILE_CHECK(ARRAYSZ(weapon_brands_terse) == NUM_SPECIAL_WEAPONS);
COMPILE_CHECK(ARRAYSZ(weapon_brands_verbose) == NUM_SPECIAL_WEAPONS);
COMPILE_CHECK(ARRAYSZ(weapon_brands_adj) == NUM_SPECIAL_WEAPONS);

static const set<brand_type> brand_prefers_adj =
            { SPWPN_VAMPIRISM, SPWPN_ANTIMAGIC, SPWPN_HEAVY, SPWPN_SPECTRAL, SPWPN_DEVIOUS };

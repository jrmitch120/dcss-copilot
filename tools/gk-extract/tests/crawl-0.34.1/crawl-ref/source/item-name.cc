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
        case SPARM_LIGHT:             return "light";
        default:                      return "bugginess";
        }
    }
    return "buggy";
}

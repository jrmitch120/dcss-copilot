/**
 * @file
 * @brief Spellbook/rod contents array and management functions
**/

// Trimmed from crawl 0.34.1 for the extractor test: the table keeps a few of
// its entries.

#include "AppHdr.h"

#include "spl-book.h"

static const map<wand_type, spell_type> _wand_spells =
{
    { WAND_FLAME, SPELL_THROW_FLAME },
    { WAND_PARALYSIS, SPELL_PARALYSE },
};

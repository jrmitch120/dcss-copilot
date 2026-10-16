#pragma once

#include "mon-book.h"

/* *************************************************************************

    This file determines which spells are contained in monster spellbooks.
    It is used by /util/gen-mst.pl to generate spellbook enums, which are
    listed in mon-mst.h.

    Template Format:

    {    mon_spellbook_type,
        {
            { spell type, frequency, flags },
            { spell type, frequency, flags },
            [...]
        }
    }

    Spellbooks are organized by shared generic books to non-uniques to
    uniques / vault monsters, base user glyph (alphabetical, lowercase
    to uppercase to numbers to misc), then loose thematic tiering.

************************************************************************* */

static const mon_spellbook mspell_list[] =
{

    {  MST_BLINKER,
      {
       { SPELL_BLINK, 29, MON_SPELL_MAGICAL },
      }
    },
    {  MST_ORC_WIZARD,
      {
       { SPELL_MAGIC_DART, 9, MON_SPELL_WIZARD },
       { SPELL_THROW_FLAME, 9, MON_SPELL_WIZARD },
       { SPELL_CONFUSE, 18, MON_SPELL_WIZARD },
       { SPELL_INVISIBILITY, 9, MON_SPELL_WIZARD },
      }
    },
    {  MST_STOKER,
      {
       {
        { SPELL_WIND_BLAST, 20, MON_SPELL_WIZARD },
        { SPELL_STOKE_FLAMES, 20, MON_SPELL_WIZARD },
       }
      }
    },
    // (trimmed for the extractor test)
};

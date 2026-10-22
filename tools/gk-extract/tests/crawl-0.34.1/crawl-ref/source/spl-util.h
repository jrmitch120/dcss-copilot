/**
 * @file
 * @brief data handlers for player spell list
 * (trimmed for the extractor test)
**/

#pragma once

enum spell_highlight_colours
{
    COL_UNKNOWN      = LIGHTGRAY,   // spells for which no known brand applies.
    COL_USEFUL_NOW   = LIGHTBLUE,   // spell not in library which you could use now
    COL_USEFUL_IN_FUTURE = LIGHTGRAY,   // spell not in library, can't use now
    COL_USELESS      = DARKGRAY,    // ability would have no useful effect / spell already in library
    COL_INAPPLICABLE = COL_USELESS, // ability cannot be meaningfully applied (e.g., no targets)
    COL_FORBIDDEN    = LIGHTRED,    // The player's god hates this ability
    COL_DANGEROUS    = LIGHTRED,    // ability/spell use could be dangerous
};

// (trimmed for the extractor test)

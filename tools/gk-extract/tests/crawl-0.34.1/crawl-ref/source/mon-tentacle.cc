/**
 * @file
 * @brief Monster tentacle-related code.
**/

#include "AppHdr.h"

#include "mon-tentacle.h"

static monster_type _head_child_segment[][3] =
{
    { MONS_KRAKEN, MONS_KRAKEN_TENTACLE,
        MONS_KRAKEN_TENTACLE_SEGMENT },
    { MONS_TENTACLED_STARSPAWN, MONS_STARSPAWN_TENTACLE,
        MONS_STARSPAWN_TENTACLE_SEGMENT },
};

static monster_type _solo_tentacle_to_segment[][2] =
{
    { MONS_ELDRITCH_TENTACLE, MONS_ELDRITCH_TENTACLE_SEGMENT },
    { MONS_SNAPLASHER_VINE,   MONS_SNAPLASHER_VINE_SEGMENT },
};

// (trimmed for the extractor test)

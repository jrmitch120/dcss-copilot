// Trimmed from crawl 0.34.1 for the extractor test: the bane that changes the player's speed.

#include "tag-version.h"

#include "bane-type.h"

struct bane_def
{
    bane_type   type;
    short       duration;    ///< Duration of bane in XP units
    const char* name;        ///< Name of this bane.
    const char* description; ///< What appears on the 'A' screen.
};

static const bane_def bane_data[] =
{
    {
        BANE_LETHARGY,
        BANE_DUR_LONG,
        "Lethargy",
        "You cover ground slowly.",
    },

    // (trimmed for the extractor test)
};

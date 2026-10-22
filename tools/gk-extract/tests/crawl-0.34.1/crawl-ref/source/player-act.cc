/**
 * @file
 * @brief Implementing the actor interface for player.
 * (trimmed for the extractor test)
**/

#include "AppHdr.h"

// (trimmed for the extractor test)

bool player::can_go_berserk() const
{
    return can_go_berserk(false);
}

bool player::can_go_berserk(bool intentional, bool potion, bool quiet,
                            string *reason, bool temp) const
{
    const bool verbose = (intentional || potion) && !quiet;
    string msg;
    bool success = false;

    if (berserk() && temp)
        msg = "You're already berserk!";
    else if (duration[DUR_BERSERK_COOLDOWN] && temp)
        msg = "You're still recovering from your berserk rage.";
    else if (duration[DUR_DEATHS_DOOR] && temp)
        msg = "You can't enter a blood rage from death's door.";
    else if (beheld() && !you.unrand_equipped(UNRAND_DEMON_AXE) && temp)
        msg = "You are too mesmerised to rage.";
    else if (!intentional && !potion && clarity() && temp)
        msg = "You're too calm and focused to rage.";
    else if (is_lifeless_undead(temp))
        msg = "You cannot raise a blood rage in your lifeless body.";
    else if (stasis())
        msg = "Your stasis prevents you from going berserk.";
    else
        success = true;

    if (!success)
    {
        if (verbose)
            mpr(msg);
        if (reason)
            *reason = msg;
    }
    return success;
}

bool player::berserk() const
{
    return duration[DUR_BERSERK];
}

// (trimmed for the extractor test)

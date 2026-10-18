/**
 * @file
 * @brief Spell definitions and descriptions. See spell_desc struct in
 *        spl-util.cc.
 * Flag descriptions are in spl-cast.h.
**/

#pragma once

/*
struct spell_desc
{
    enum, spell name,
    spell schools,
    flags,
    level,
    power_cap,
    min_range, max_range, (-1 if not applicable)
    effect_noise
    tile
}
*/

#include "tag-version.h"

static const struct spell_desc spelldata[] =
{

{
    SPELL_CAUSE_FEAR, "Cause Fear",
    spschool::hexes,
    spflag::WL_check,
    4,
    200,
    LOS_RADIUS, LOS_RADIUS,
    0,
    TILEG_CAUSE_FEAR,
},

{
    SPELL_MAGIC_DART, "Magic Dart",
    spschool::conjuration,
    spflag::dir_or_target | spflag::needs_tracer,
    1,
    25,
    LOS_RADIUS, LOS_RADIUS,
    0,
    TILEG_MAGIC_DART,
},

{
    SPELL_FIREBALL, "Fireball",
    spschool::conjuration | spschool::fire,
    spflag::dir_or_target | spflag::needs_tracer,
    5,
    200,
    5, 5,
    0,
    TILEG_FIREBALL,
},

{
    SPELL_BLINK, "Blink",
    spschool::translocation,
    spflag::escape | spflag::selfench,
    2,
    50,
    -1, -1,
    0,
    TILEG_BLINK,
},

{
    SPELL_SWIFTNESS, "Swiftness",
    spschool::air,
    spflag::hasty | spflag::selfench,
    3,
    100,
    -1, -1,
    0,
    TILEG_SWIFTNESS,
},

{
    SPELL_PASSWALL, "Passwall",
    spschool::earth,
    spflag::target | spflag::escape | spflag::not_self | spflag::silent,
    3,
    100,
    3, 3,
    0,
    TILEG_PASSWALL,
},

{
    SPELL_GOLUBRIAS_PASSAGE, "Passage of Golubria",
    spschool::translocation,
    spflag::target | spflag::aim_at_space | spflag::escape | spflag::selfench,
    4,
    100,
    2, LOS_RADIUS,
    8, // when it closes
    TILEG_PASSAGE_OF_GOLUBRIA,
},

// (trimmed for the extractor test)

#if TAG_MAJOR_VERSION == 34
#define AXED_SPELL(tag, name) \
    { tag, name, spschool::none, spflag::none, 7, 0, -1, -1, 0, TILEG_ERROR },

AXED_SPELL(SPELL_AURA_OF_ABJURATION, "Aura of Abjuration")
AXED_SPELL(SPELL_BOLT_OF_INACCURACY, "Bolt of Inaccuracy")
AXED_SPELL(SPELL_CHANT_FIRE_STORM, "Chant Fire Storm")
#endif

};

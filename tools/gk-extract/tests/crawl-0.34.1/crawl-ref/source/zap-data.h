/**
 * @file
 * @brief Zap definitions. See zap_info struct in beam.cc.
**/

#pragma once

/*
struct zap_info
{
    zap_type ztype;
    const char* name;
    dam_deducer* player_damage;
    tohit_deducer* player_tohit;       // Enchantments have power modifier here
    dam_deducer* monster_damage;
    tohit_deducer* monster_tohit;      // Enchantments have power modifier here
    colour_t colour;
    bool is_enchantment;
    beam_type flavour;
    dungeon_char_type glyph;
    bool can_beam;
    bool is_explosion;
    tileidx_t tile_beam;
    tileidx_t tile_explode;
}
*/

/// Boilerplate monster hex.
static zap_info _mon_hex_zap(zap_type ztype, beam_type beam,
                             colour_t colour = BLACK)
{
    return {
        ztype,
        "",
        nullptr,
        nullptr,
        nullptr,
        new tohit_calculator<0, 1, 1>,
        colour,
        true,
        beam,
        NUM_DCHAR_TYPES,
        false,
        false,
    };
}

static const zap_info zap_data[] =
{

{
    ZAP_MAGIC_DART,
    "magic dart",
    new dicedef_calculator<1, 3, 1, 5>,
    new tohit_calculator<AUTOMATIC_HIT>,
    new dicedef_calculator<3, 4, 1, 100>,
    new tohit_calculator<AUTOMATIC_HIT>,
    LIGHTMAGENTA,
    false,
    BEAM_MMISSILE,
    DCHAR_FIRED_ZAP,
    false,
    false,
    TILE_BOLT_MAGIC_DART,
},

{
    ZAP_FIREBALL,
    "fireball",
    new calcdice_calculator<3, 10, 1, 2>,
    new tohit_calculator<40>,
    new dicedef_calculator<3, 7, 1, 10>,
    new tohit_calculator<40>,
    RED,
    false,
    BEAM_FIRE,
    DCHAR_FIRED_ZAP,
    false,
    true,
    TILE_BOLT_FLAME,
    TILE_BOLT_DEFAULT_RED,
},

{
    ZAP_MIASMA_BREATH,
    "foul vapour",
    nullptr,
    nullptr,
    new dicedef_calculator<3, 5, 1, 24>,
    new tohit_calculator<17, 1, 20>,
    DARKGREY,
    false,
    BEAM_MIASMA,
    DCHAR_FIRED_ZAP,
    true,
    false,
},

{
    ZAP_HOLY_BREATH,
    "blast of cleansing flame",
    nullptr,
    nullptr,
    new dicedef_calculator<3, 0, 1, 12>,
    new tohit_calculator<18, 1, 25>,
    ETC_HOLY,
    false,
    BEAM_HOLY,
    DCHAR_FIRED_ZAP,
    true,
    false,
},

_mon_hex_zap(ZAP_SLOW, BEAM_SLOW),

// (trimmed for the extractor test)

};

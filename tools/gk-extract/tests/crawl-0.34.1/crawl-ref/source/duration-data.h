/*
 * Trimmed from crawl 0.34.1 for the extractor test: the durations whose status lights or texts
 * tell the player's Lighting or holiness, or stop them using their Ways Out.
 */

static const duration_def duration_data[] =
{
    { DUR_BERSERK,
      BLUE, "Berserk",
      "berserking", "berserk",
      "You are possessed by a berserker rage.", D_EXPIRES,
      {{ "You are no longer berserk.", player_end_berserk }}, 6},
    // (trimmed for the extractor test)
    { DUR_CONF,
      RED, "Conf",
      "confused", "conf",
      "You are confused.", D_DISPELLABLE | D_NEGATIVE,
      {{ "You feel less confused." }}},
    // (trimmed for the extractor test)
    { DUR_CORONA, // Has custom long_text
      YELLOW, "Corona",
      "lit by a corona", "corona",
      "", D_DISPELLABLE | D_NEGATIVE,
    // (trimmed for the extractor test)
    { DUR_STICKY_FLAME,
      RED, "Fire",
      "on fire", "liquid fire",
      "You are covered in liquid fire.", D_DISPELLABLE /*but special-cased*/ | D_NEGATIVE},
    // (trimmed for the extractor test)
    { DUR_PETRIFIED,
      RED, "Stone",
      "petrified", "",
      "You are petrified.", D_DISPELLABLE | D_NEGATIVE},
    // (trimmed for the extractor test)
    { DUR_QUAD_DAMAGE,
      BLUE, "Quad",
      "quad damage", "",
      "", D_EXPIRES,
    // (trimmed for the extractor test)
    { DUR_HEAVENLY_STORM, 0, "", "in a heavenly storm", "heavenly storm",
      "Heavenly clouds are increasing your accuracy and damage.", D_NO_FLAGS,
      {{ "", wu_jian_decrement_heavenly_storm }}},
    // (trimmed for the extractor test)
    { DUR_DIMENSION_ANCHOR,
      RED, "-Tele",
      "untranslocatable", "dimension anchor",
      "You are firmly anchored to this plane.", D_DISPELLABLE | D_NEGATIVE,
      {{ "You are no longer firmly anchored in space." }}},
    // (trimmed for the extractor test)
    { DUR_NO_CAST, RED, "-Cast",
      "unable to cast spells", "no cast",
      "You are unable to cast spells.", D_DISPELLABLE | D_NEGATIVE,
      {{ "You regain access to your magic." }}},
    // (trimmed for the extractor test)
};

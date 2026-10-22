/*
 * Trimmed from crawl 0.34.1 for the extractor test: the durations whose status lights or texts
 * tell the player's Lighting, holiness or speed, or stop them using their Ways Out.
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
      {{ "", []() {
          if (!you.backlit())
              mprf(MSGCH_DURATION, "You are no longer glowing.");
      }}}},
    // (trimmed for the extractor test)
    { DUR_STICKY_FLAME,
      RED, "Fire",
      "on fire", "liquid fire",
      "You are covered in liquid fire.", D_DISPELLABLE /*but special-cased*/ | D_NEGATIVE},
    // (trimmed for the extractor test)
    { DUR_PARALYSIS,
      RED, "Para",
      "paralysed", "paralysis",
      "You are paralysed.", D_DISPELLABLE | D_NEGATIVE},
    { DUR_PETRIFIED,
      RED, "Stone",
      "petrified", "",
      "You are petrified.", D_DISPELLABLE | D_NEGATIVE},
    // (trimmed for the extractor test)
    { DUR_PETRIFYING,
      LIGHTRED, "Petr",
      "petrifying", "",
      "You are turning to stone.", D_DISPELLABLE /*but special-cased*/ | D_EXPIRES | D_NEGATIVE,
        {}, 1},
    // (trimmed for the extractor test)
    { DUR_SWIFTNESS,
      BLUE, "Swift",
      "swift", "swiftness",
      "You can move swiftly.", D_DISPELLABLE | D_EXPIRES, {}, 6},
    // (trimmed for the extractor test)
    { DUR_FROZEN,
      RED, "Frozen",
      "frozen", "",
      "You are partly encased in ice.", D_DISPELLABLE | D_NEGATIVE,
      {{ "The ice encasing you melts away." }, {}, true }},
    // (trimmed for the extractor test)
    { DUR_SLOW, 0, "", "", "slow", "", D_DISPELLABLE | D_NEGATIVE},
    // (trimmed for the extractor test)
    { DUR_QUAD_DAMAGE,
      BLUE, "Quad",
      "quad damage", "",
      "", D_EXPIRES,
      {{ "", []() { invalidate_agrid(true); }},
        { "Quad Damage is wearing off."}}, 3 }, // per client.qc
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

/*
 * Trimmed from crawl 0.34.1 for the extractor test: the durations whose status lights or texts
 * tell the player's Lighting.
 */

static const duration_def duration_data[] =
{
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
    { DUR_QUAD_DAMAGE,
      BLUE, "Quad",
      "quad damage", "",
      "", D_EXPIRES,
    // (trimmed for the extractor test)
    { DUR_HEAVENLY_STORM, 0, "", "in a heavenly storm", "heavenly storm",
      "Heavenly clouds are increasing your accuracy and damage.", D_NO_FLAGS,
      {{ "", wu_jian_decrement_heavenly_storm }}},
    // (trimmed for the extractor test)
};

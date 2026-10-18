// Trimmed from crawl 0.34.1 for the extractor test: the abilities that are Ways Out, and the
// message for having none.

static vector<ability_def> &_get_ability_list()
{
    static vector<ability_def> Ability_List =
    {
        // (trimmed for the extractor test)
        { ABIL_HOP, "Hop",
            0, 0, 0, -1, {}, abflag::none }, // range special-cased
        { ABIL_BLINKBOLT, "Blinkbolt",
            0, 0, 0, LOS_MAX_RANGE, {}, abflag::none },
        // (trimmed for the extractor test)
        { ABIL_HEAL_WOUNDS, "Heal Wounds",
            0, 0, 0, -1, {fail_basis::xl, 45, 2}, abflag::none },
        // (trimmed for the extractor test)
        { ABIL_EVOKE_BLINK, "Evoke Blink",
            0, 0, 0, -1, {fail_basis::evo, 40, 2}, abflag::none },
        // (trimmed for the extractor test)
        { ABIL_ZIN_SANCTUARY, "Sanctuary",
            7, 0, 15, -1, {fail_basis::invo, 80, 4, 25}, abflag::none },
        // (trimmed for the extractor test)
        { ABIL_TROG_BERSERK, "Berserk",
            0, 0, 1, -1, {fail_basis::invo, 45, 0, 2}, abflag::none },
        // (trimmed for the extractor test)
        { ABIL_ELYVILON_HEAL_SELF, "Heal Self",
            2, 0, 3, -1, {fail_basis::invo, 40, 5, 20}, abflag::none },
        // (trimmed for the extractor test)
        { ABIL_NEMELEX_DRAW_ESCAPE, "Draw Escape",
            0, 0, 0, -1, {fail_basis::invo}, abflag::card },
        // (trimmed for the extractor test)
        { ABIL_CHEIBRIADOS_TIME_BEND, "Bend Time",
            3, 0, 1, -1, {fail_basis::invo, 40, 4, 20}, abflag::none },
        // (trimmed for the extractor test)
        { ABIL_CHEIBRIADOS_TIME_STEP, "Step From Time",
            10, 0, 12, -1, {fail_basis::invo, 80, 4, 25}, abflag::none },
        // (trimmed for the extractor test)
        { ABIL_DITHMENOS_SHADOWSLIP, "Shadowslip",
            4, 60, 4, -1, {fail_basis::invo, 50, 6, 30}, abflag::instant },
        // (trimmed for the extractor test)
        { ABIL_RU_DRAW_OUT_POWER, "Draw Out Power",
            0, 0, 0, -1, {fail_basis::invo},
            abflag::exhaustion | abflag::max_hp_drain | abflag::conf_ok },
        // (trimmed for the extractor test)
        { ABIL_WU_JIAN_SERPENTS_LASH, "Serpent's Lash",
            0, 0, 2, -1, {fail_basis::invo},
            abflag::exhaustion | abflag::instant },
        // (trimmed for the extractor test)
        { ABIL_IGNIS_RISING_FLAME, "Rising Flame",
            0, 0, 0, -1, {fail_basis::invo}, abflag::none },
        // (trimmed for the extractor test)
    };
    return Ability_List;
}

// (trimmed for the extractor test)

void no_ability_msg()
{
    mpr("Sorry, you're not good enough to have a special ability.");
}

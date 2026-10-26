// Trimmed from crawl 0.34.1 for the extractor test: the spells the player learns without
// memorising them, and the god whose followers' skills differ from their base levels.

        mprf("The power to cast %s wells up from within.", spell_title(spell));

// (trimmed for the extractor test)

    mprf("(You can now cast %s.)", spell_title(spell));

// (trimmed for the extractor test)

int player::skill(skill_type sk, int scale, bool real, bool temp) const
{
    // If you add another enhancement/reduction, be sure to change
    // SkillMenuSwitch::get_help() to reflect that

    // wizard racechange, or upgraded old save
    if (is_useless_skill(sk))
        return 0;

    // skills[sk] might not be updated yet if this is in the middle of
    // skill training, so make sure to use the correct value.
    int actual_skill = skills[sk];
    unsigned int effective_points = skill_points[sk];
    if (!real)
        effective_points += get_crosstrain_points(sk);
    effective_points = min(effective_points, skill_exp_needed(MAX_SKILL_LEVEL, sk));
    actual_skill = calc_skill_level_change(sk, actual_skill, effective_points);

    int level = actual_skill * scale
      + get_skill_progress(sk, actual_skill, effective_points, scale);
    if (real)
        return level;

    if (you.unrand_equipped(UNRAND_HERMITS_PENDANT))
    {
        if (sk == SK_INVOCATIONS)
            return 14 * scale;
        if (sk == SK_EVOCATIONS)
            return 0;
    }

    if (penance[GOD_ASHENZARI])
    {
        if (temp)
            level = max(level - 4 * scale, level / 2);
    }
    else if (ash_has_skill_boost(sk))
            level = ash_skill_boost(sk, scale);

    // (trimmed for the extractor test)
}

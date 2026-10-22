// Trimmed from crawl 0.34.1 for the extractor test: the abilities that are Ways Out, the checks
// on them, and the message for having none.

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

    return activate_talent(talents[selected]);
}

static bool _can_movement_ability(bool quiet)
{
    const string reason = movement_impossible_reason();
    if (reason.empty())
        return true;
    if (!quiet)
        mpr(reason);
    return false;
}

static bool _can_hop(bool quiet)
{
    if (you.duration[DUR_NO_HOP])
    {
        if (!quiet)
            mpr("Your legs are too worn out to hop.");
        return false;
    }
    return _can_movement_ability(quiet);
}

static bool _can_blinkbolt(bool quiet)
{
    if (you.duration[DUR_BLINKBOLT_COOLDOWN])
    {
        if (!quiet)
            canned_msg(MSG_CANNOT_DO_YET);
        return false;
    }
    return true;
}

static bool _can_rising_flame(bool quiet)
{
    ASSERT(can_do_capstone_ability(GOD_IGNIS));
    if (you.duration[DUR_RISING_FLAME])
    {
        if (!quiet)
            mpr("You're already rising!");
        return false;
    }
    if (you.where_are_you == BRANCH_DUNGEON && you.depth == 1)
    {
        if (player_has_orb())
            return true;
        else
        {
            if (!quiet)
                mpr("You can't rise from this level without the Orb!");
            return false;
        }
    }
    if (!level_above().is_valid())
    {
        if (!quiet)
            mpr("You can't rise from this level!");
        return false;
    }
    return true;
}

// Check prerequisites for a number of abilities.
// Abort any attempt if these cannot be met, without losing the turn.
// TODO: Many more cases need to be added!
// Check prerequisites for a number of abilities.
// Abort any attempt if these cannot be met, without losing the turn.
// TODO: Many more cases need to be added!
static bool _check_ability_possible(const ability_def& abil, bool quiet = false)
{
    if (you.confused() && !testbits(abil.flags, abflag::conf_ok))
    {
        if (!quiet)
            canned_msg(MSG_TOO_CONFUSED);
        return false;
    }

    if (you.berserk() && !testbits(abil.flags, abflag::berserk_ok))
    {
        if (!quiet)
            canned_msg(MSG_TOO_BERSERK);
        return false;
    }

    if (abil.ability == ABIL_TROG_BERSERK
        && !you.can_go_berserk(true, false, quiet))
    {
        return false;
    }

    // (trimmed for the extractor test)

    switch (abil.ability)
    {
    // (trimmed for the extractor test)

    case ABIL_ZIN_SANCTUARY:
        if (env.sanctuary_time)
        {
            if (!quiet)
                mpr("There's already a sanctuary in place on this level.");
            return false;
        }
        return true;

    // (trimmed for the extractor test)

    case ABIL_ELYVILON_HEAL_SELF:
        if (you.hp == you.hp_max)
        {
            if (!quiet)
                canned_msg(MSG_FULL_HEALTH);
            return false;
        }
        if (you.duration[DUR_DEATHS_DOOR])
        {
            if (!quiet)
                mpr("You can't heal while in death's door.");
            return false;
        }
        return true;

    case ABIL_ELYVILON_PURIFICATION:
        if (!you.duration[DUR_SICKNESS]
            && !you.duration[DUR_POISONING]
            && !you.duration[DUR_CONF] && !you.duration[DUR_SLOW]
            && !you.petrifying()
            && !player_drained()
            && !you.duration[DUR_WEAK])
        {
            if (!quiet)
                mpr("Nothing ails you!");
            return false;
        }
        return true;

    // (trimmed for the extractor test)

    case ABIL_RU_DRAW_OUT_POWER:
        if (you.duration[DUR_EXHAUSTED])
        {
            if (!quiet)
                mpr("You're too exhausted to draw out your power.");
            return false;
        }
        if (you.hp == you.hp_max && you.magic_points == you.max_magic_points
            && !you.duration[DUR_CONF]
            && !you.duration[DUR_SLOW]
            && !you.attribute[ATTR_HELD]
            && !you.petrifying()
            && !you.beheld()
            && !you.afraid()
            && !you.is_constricted())
        {
            if (!quiet)
                mpr("You have no need to draw out power.");
            return false;
        }
        return true;

    // (trimmed for the extractor test)

#if TAG_MAJOR_VERSION == 34
    case ABIL_HEAL_WOUNDS:
        if (you.hp == you.hp_max)
        {
            if (!quiet)
                canned_msg(MSG_FULL_HEALTH);
            return false;
        }
        if (you.duration[DUR_DEATHS_DOOR])
        {
            if (!quiet)
                mpr("You can't heal while in death's door.");
            return false;
        }
        if (get_real_mp(false) < 1)
        {
            if (!quiet)
                mpr("You don't have enough innate magic capacity.");
            return false;
        }
        return true;
#endif

    case ABIL_SHAFT_SELF:
        return you.can_do_shaft_ability(quiet);

    case ABIL_HOP:
        return _can_hop(quiet);

    // (trimmed for the extractor test)

    // Note: *Not* identical to blink. Is not affected by -Tele.
    case ABIL_SPIDER_JUMP:
        if (you.duration[DUR_BLINK_COOLDOWN])
        {
            if (!quiet)
                canned_msg(MSG_CANNOT_DO_YET);
            return false;
        }
        return true;

    case ABIL_EVOKE_BLINK:
        if (you.duration[DUR_BLINK_COOLDOWN])
        {
            if (!quiet)
                canned_msg(MSG_CANNOT_DO_YET);
            return false;
        }
        // fallthrough
    case ABIL_BLINKBOLT:
    {
        const string no_tele_reason = you.no_tele_reason(true);
        if (no_tele_reason.empty())
            return true;

        if (!quiet)
             mpr(no_tele_reason);
        return false;
    }

    case ABIL_WATERY_GRAVE:
        if (you.props.exists(WATERY_GRAVE_XP_KEY))
        {
            if (!quiet)
                mpr("You must recover your energy before doing this again.");
            return false;
        }
        for (radius_iterator ri(you.pos(), 4, C_SQUARE, LOS_NO_TRANS, false); ri; ++ri)
        {
            if (feat_is_water(env.grid(*ri)))
                return true;
        }

        if (!quiet)
            mpr("There is no water in range!");

        return false;

    case ABIL_TROG_BERSERK:
        return you.can_go_berserk(true, false, true)
               && (quiet || berserk_check_wielded_weapon());

    case ABIL_EVOKE_TURN_INVISIBLE:
        if (you.duration[DUR_INVIS])
        {
            if (!quiet)
                mpr("You are already invisible!");
            return false;
        }
        return true;

    // (trimmed for the extractor test)

    case ABIL_WU_JIAN_SERPENTS_LASH:
        if (you.attribute[ATTR_SERPENTS_LASH])
        {
            if (!quiet)
                mpr("You are already lashing out.");
            return false;
        }
        if (you.duration[DUR_EXHAUSTED])
        {
            if (!quiet)
                mpr("You are too exhausted to lash out.");
            return false;
        }
        return true;

    // (trimmed for the extractor test)

    case ABIL_IGNIS_RISING_FLAME:
        return _can_rising_flame(quiet);

    // (trimmed for the extractor test)

    default:
        return true;
    }
}

// (trimmed for the extractor test)

static spret _do_ability(const ability_def& abil, bool fail, dist *target,
                         bolt beam)
{
    // (trimmed for the extractor test)

    switch (abil.ability)
    {
    // (trimmed for the extractor test)

    case ABIL_HOP:
        if (_can_hop(false))
            return frog_hop(fail, target);
        else
            return spret::abort;

    case ABIL_BLINKBOLT:
        if (!_can_blinkbolt(false))
            return spret::abort;
        {
            const int power = get_form()->get_level(200) / 27;
            return your_spells(SPELL_BLINKBOLT, power, false, nullptr, target);
        }

    case ABIL_SIPHON_ESSENCE:
        return _siphon_essence(fail);

    // (trimmed for the extractor test)

    case ABIL_TROG_BERSERK:
        fail_check();
        // Trog abilities don't use or train invocations.
        you.go_berserk(true);
        break;

    // (trimmed for the extractor test)

    case ABIL_IGNIS_RISING_FLAME:
        if (!_can_rising_flame(false))
            return spret::abort;
        mpr("You begin to rise into the air.");
        // slightly faster than teleport
        you.set_duration(DUR_RISING_FLAME, 2 + random2(3));
        you.one_time_ability_used.set(GOD_IGNIS);
        return spret::success;

    // (trimmed for the extractor test)

    default:
        die("invalid ability");
    }

    return spret::success;
}

// (trimmed for the extractor test)

void no_ability_msg()
{
    mpr("Sorry, you're not good enough to have a special ability.");
}

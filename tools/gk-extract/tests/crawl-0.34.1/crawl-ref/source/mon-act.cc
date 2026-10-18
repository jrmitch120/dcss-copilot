/**
 * @file
 * @brief Monsters doing stuff (monsters acting).
**/

static bool _fungal_move_check(monster &mon)
{
    // These monsters have restrictions on moving while you are looking.
    if ((mon.type == MONS_WANDERING_MUSHROOM || mon.type == MONS_DEATHCAP)
            && mon.foe_distance() > 1 // can attack if adjacent
        || (mon.type == MONS_LURKING_HORROR
            // 1 in los chance at max los
            && mon.foe_distance() > random2(you.current_vision + 1)))
    {
        if (!mon.wont_attack() && is_sanctuary(mon.pos()))
            return true;

        if (mon_enemies_around(&mon))
            return false;
    }
    return true;
}

static void _pre_monster_move(monster& mons)
{
    // Check for golem summoner proximity whether we have energy to act or not.
    // (Avoids awkward situations of moving next to a dormant golem with very
    // fast actions not appearing to wake it up until the following turn)
    if (mons.type == MONS_BLAZEHEART_GOLEM)
        _check_blazeheart_golem_link(mons);
}

void handle_monster_move(monster* mons)
{
    if (mons->type == MONS_FULMINANT_PRISM
        || mons->type == MONS_SHADOW_PRISM)
    {
        ++mons->prism_charge;
    }

    // Melt barricades whose creator has moved too far away.
    if (mons->type == MONS_SPLINTERFROST_BARRICADE)
        return;

    if (mons->type == MONS_BLAZEHEART_CORE)
    {
        mons->suicide();
        return;
    }

    if (mons->type == MONS_RENDING_BLADE)
        mons->number = 0;

    if (mons->type == MONS_BURSTSHROOM)
    {
        _burstshroom_grow(*mons);
        return;
    }

    // Please change _slouch_damage to match!
    if (mons->cannot_act()
        || mons->type == MONS_SIXFIRHY // these move only 8 of 24 turns
            && ++mons->move_spurt / 8 % 3 != 2  // but are not helpless
        || mons->type == MONS_JIANGSHI // similarly, but more irregular (48 of 90)
            && (++mons->move_spurt / 6 % 3 == 1 || mons->move_spurt / 3 % 5 == 1))
    {
        mons->speed_increment -= non_move_energy;
        return;
    }

    if (mons->type == MONS_BOULDER)
    {
        _handle_boulder_movement(*mons);
        return;
    }

    if (mons->type == MONS_HELLFIRE_MORTAR)
    {
        _handle_hellfire_mortar(*mons);
        return;
    }

    if (mons->type == MONS_DIAMOND_SAWBLADE)
    {
        try_mons_cast(*mons, SPELL_SHRED);
        return;
    }

    if (mons->type == MONS_LIGHTNING_SPIRE)
    {
        _handle_lightning_spire(*mons);
        return;
    }

    if (mons->type == MONS_SOLAR_EMBER && adjacent(you.pos(), mons->pos()))
    {
        mons->lose_energy(EUT_MOVE);
        return;
    }

    if (mons->type == MONS_BOUNDLESS_TESSERACT)
    {
        tesseract_action(*mons);
        mons->lose_energy(EUT_MOVE);
        return;
    }
}

// (trimmed for the extractor test)

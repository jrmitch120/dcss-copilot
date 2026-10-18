/**
 * @file
 * @brief Monster abilities.
**/

bool mon_special_ability(monster* mons)
{
    bool used = false;

    const monster_type mclass = (mons_genus(mons->type) == MONS_DRACONIAN)
                                  ? draconian_subspecies(*mons)
                                  : mons->type;

    switch (mclass)
    {
    case MONS_UGLY_THING:
    case MONS_VERY_UGLY_THING:
        // A (very) ugly thing may mutate if it's next to other ones (or
        // next to you if you're contaminated).
        used = ugly_thing_mutate(*mons, false);
        break;

    case MONS_SLIME_CREATURE:
        // Slime creatures may split or merge depending on the
        // situation.
        used = _slime_split_merge(mons);
        if (!mons->alive())
            return true;
        break;

    case MONS_CLOCKWORK_BEE_INACTIVE:
    {
        // Note: the player is not a monster, so this will never happen to them.
        monster* summ = monster_by_mid(mons->summoner);
        if (summ && adjacent(summ->pos(), mons->pos()))
            clockwork_bee_recharge(*summ, *mons);
        break;
    }

    default:
        break;
    }

    if (used)
        mons->lose_energy(EUT_SPECIAL);

    return used;
}

// (trimmed for the extractor test)

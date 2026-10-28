// Trimmed from crawl 0.34.1 for the extractor test: what crawl says as a monster dies.

item_def* monster_die(monster& mons, killer_type killer,
                      int killer_index, bool silent, bool wizard, bool fake)
{
    switch (killer)
    {
        case KILL_YOU:
        case KILL_YOU_MISSILE:
        case KILL_YOU_CONF:
        {
            if (death_message)
            {
                if (killer == KILL_YOU_CONF
                    && (anon || !invalid_monster_index(killer_index)))
                {
                    mprf(MSGCH_MONSTER_DAMAGE, MDAM_DEAD, "%s is %s!",
                         mons.name(DESC_THE).c_str(),
                         exploded   ? "blown up" :
                         destroyed  ? "destroyed"
                                    : "killed");
                }
                else
                {
                    mprf(MSGCH_MONSTER_DAMAGE, MDAM_DEAD, "You %s %s!",
                         exploded  ? "blow up" :
                         destroyed ? "destroy"
                                   : "kill",
                         mons.name(DESC_THE).c_str());
                }
            }
            break;
        }

        case KILL_MON:
        case KILL_MON_MISSILE:
        {
            if (death_message)
            {
                const char* msg =
                    exploded  ? " is blown up!" :
                    destroyed ? " is destroyed!"
                              : " dies!";
                simple_monster_message(mons, msg, false, MSGCH_MONSTER_DAMAGE,
                                       MDAM_DEAD);
            }
            break;
        }
    }
}

/**
 * @file
 * @brief Pseudo spells triggered by gods and various items.
**/

int torment_player(const actor *attacker, torment_source_type taux)
{
    ASSERT(!crawl_state.game_is_arena());

    int hploss = 0;

    if (!you.res_torment())
    {
        // Negative energy resistance can alleviate torment.
        hploss = max(0, you.hp * (50 - player_prot_life() * 5) / 100 - 1);
        // Statue form is only partial petrification.
        // Vampire form is only partially undead.
        if (you.form == transformation::statue
            || you.form == transformation::vampire
            || you.form == transformation::bat_swarm)
        {
            hploss /= 2;
        }
        if (you.has_mutation(MUT_TORMENT_RESISTANCE))
            hploss /= 2;
    }

    // (trimmed for the extractor test)
    return hploss;
}

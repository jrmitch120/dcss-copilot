/**
 * @file
 * @brief Monsters exploding as they die.
**/

bool explode_monster(monster* mons, killer_type killer, bool pet_kill)
{
    // (trimmed for the extractor test)

    if (type == MONS_LURKING_HORROR)
        torment(mons, TORMENT_LURKING_HORROR, mons->pos());

    // Exploding kills the monster a bit earlier than normal.
    mons->hit_points = -16;

    // (trimmed for the extractor test)
    return true;
}

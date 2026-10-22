#include "AppHdr.h"

#include "beam.h"

#include "cloud.h"

// (trimmed for the extractor test)

cloud_type bolt::get_cloud_type() const
{
    if (origin_spell == SPELL_NOXIOUS_CLOUD)
        return CLOUD_MEPHITIC;

    if (origin_spell == SPELL_POISONOUS_CLOUD)
        return CLOUD_POISON;

    if (origin_spell == SPELL_HOLY_BREATH)
        return CLOUD_HOLY;

    if (origin_spell == SPELL_FLAMING_CLOUD)
        return CLOUD_FIRE;

    if (origin_spell == SPELL_CHAOS_BREATH)
        return CLOUD_CHAOS;

    if (origin_spell == SPELL_MIASMA_BREATH)
        return CLOUD_MIASMA;

    if (origin_spell == SPELL_FREEZING_GUST)
        return CLOUD_COLD;

    if (origin_spell == SPELL_SPECTRAL_CLOUD)
        return CLOUD_SPECTRAL;

    if (origin_spell == SPELL_RAVENOUS_SWARM)
        return CLOUD_BATS;

    return CLOUD_NONE;
}

void bolt::affect_place_clouds()
{
    if (in_explosion_phase)
        affect_place_explosion_clouds();

    const coord_def p = pos();

    // Is there already a cloud here?
    if (cloud_struct* cloud = cloud_at(p))
    {
        // fire cancelling cold & vice versa
        if ((cloud->type == CLOUD_COLD && is_fiery())
            || (cloud->type == CLOUD_FIRE && flavour == BEAM_COLD))
        {
            if (player_can_hear(p))
                mprf(MSGCH_SOUND, "You hear a sizzling sound!");

            delete_cloud(p);
            extra_range_used += 5;
            return;
        }
        // blastmote explosions
        if (cloud->type == CLOUD_BLASTMOTES && is_fiery())
            explode_blastmotes_at(p);
    }

    const dungeon_feature_type feat = env.grid(p);

    if (origin_spell == SPELL_POISONOUS_CLOUD)
        place_cloud(CLOUD_POISON, p, random2(5) + 3, agent());

    if (origin_spell == SPELL_HOLY_BREATH)
        place_cloud(CLOUD_HOLY, p, random2(4) + 2, agent());

    if (origin_spell == SPELL_FLAMING_CLOUD)
        place_cloud(CLOUD_FIRE, p, random2(4) + 2, agent());

    if (origin_spell == SPELL_RAVENOUS_SWARM)
        place_cloud(CLOUD_BATS, p, random2(4) + 4, agent());

    // Fire/cold over water/lava
    if (feat == DNGN_LAVA && flavour == BEAM_COLD
        || feat_is_water(feat) && is_fiery())
    {
        place_cloud(CLOUD_STEAM, p, 2 + random2(5), agent(), 11);
    }

    if (feat_is_water(feat) && flavour == BEAM_COLD
        && damage.num * damage.size > 35)
    {
        place_cloud(CLOUD_COLD, p, damage.num * damage.size / 30 + 1, agent());
    }

    if (flavour == BEAM_MIASMA)
        place_cloud(CLOUD_MIASMA, p, random2(5) + 2, agent());


    if (flavour == BEAM_STEAM)
    {
        if (origin_spell == SPELL_STEAM_BREATH)
            place_cloud(CLOUD_STEAM, p, div_rand_round(ench_power, 3) + 3, agent());
        else
            place_cloud(CLOUD_STEAM, p, random2(5) + 2, agent());
    }

    //XXX: these use the name for a gameplay effect.
    if (name == "poison gas")
        place_cloud(CLOUD_POISON, p, random2(4) + 3, agent());

    if (origin_spell == SPELL_PETRIFYING_CLOUD)
        place_cloud(CLOUD_PETRIFY, p, random2(4) + 4, agent());

    if (origin_spell == SPELL_SPECTRAL_CLOUD)
        place_cloud(CLOUD_SPECTRAL, p, random2(6) + 5, agent());

    if (origin_spell == SPELL_DEATH_RATTLE)
        place_cloud(CLOUD_MIASMA, p, random2(4) + 4, agent());

    if (origin_spell == SPELL_CAUSTIC_BREATH)
    {
        if (!actor_at(p))
        {
            place_cloud(CLOUD_ACID, p, 3 + random_range(ench_power / 2,
                                                        ench_power * 4 / 5),
                        agent());
        }
    }

    if (origin_spell == SPELL_NOXIOUS_BREATH)
    {
        place_cloud(CLOUD_MEPHITIC, p,
                    2 + random_range(ench_power / 2, ench_power * 4 / 5), agent());

        for (adjacent_iterator ai(pos()); ai; ++ai)
        {
            if (x_chance_in_y(max(0, ench_power - 10), ench_power - 5))
            {
                place_cloud(CLOUD_MEPHITIC, *ai,
                        2 + random_range(ench_power / 3, ench_power / 2), agent());
            }
        }
    }

    if (origin_spell == SPELL_RUST_BREATH)
        place_cloud(CLOUD_RUST, p, 5 + ench_power * 2 / 3 + random2(2), agent());

    if (origin_spell == SPELL_GOLDEN_BREATH)
        place_cloud(CLOUD_POISON, p, random_range(4, 8), agent());

    // Only place clouds on creatures who are immune to them
    if (origin_spell == SPELL_MOURNING_WAIL)
    {
        if (!actor_at(p) || actor_at(p)->res_negative_energy() == 3)
            place_cloud(CLOUD_MISERY, p, random2(4) + 2, agent());
    }

    if (origin_spell == SPELL_MARCH_OF_SORROWS)
    {
        place_cloud(CLOUD_MISERY, p, random2(5) + 8, agent());

        if (actor_at(p) && !mons_aligned(actor_at(p), agent()))
            for (adjacent_iterator ai(pos()); ai; ++ai)
                place_cloud(CLOUD_MISERY, *ai, random2(4) + 2, agent());
    }

}

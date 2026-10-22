// Trimmed from crawl 0.34.1 for the extractor test: the passives of the gods that change the
// player's speed.

static const vector<god_passive> god_passives[] =
{
    // (trimmed for the extractor test)


    // Beogh
    {
        {  1, passive_t::convert_orcs },
        {  5, passive_t::water_walk },
    },

    // (trimmed for the extractor test)

    // Cheibriados
    {
        { -1, passive_t::no_haste },
        { -1, passive_t::slowed },
        {  0, passive_t::slow_orb_run },
        {  0, passive_t::stat_boost },
        {  0, passive_t::slow_abyss },
        // TODO: this one should work regardless of penance, maybe?
        {  0, passive_t::slow_zot },
        {  0, passive_t::slow_poison },
    },


    // (trimmed for the extractor test)
};

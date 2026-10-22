// Trimmed from crawl 0.34.1 for the extractor test: the water a player wades through.

/** Is this feature a type of water that is too deep for most creatures to
*   wade through?
*/
bool feat_is_deep_water(dungeon_feature_type feat)
{
    return feat == DNGN_DEEP_WATER
           || feat == DNGN_OPEN_SEA;
}

/** Is this feature a type of water that is shallow enough to wade through?
*/
bool feat_is_shallow_water(dungeon_feature_type feat)
{
    return feat == DNGN_SHALLOW_WATER
           || feat == DNGN_TOXIC_BOG
           || feat == DNGN_MANGROVE;
}

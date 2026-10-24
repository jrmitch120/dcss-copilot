// (trimmed for the extractor test)

    const PlaceInfo& place = you.get_place_info();
    string short_name = branches[place.branch].shortname;

    if (brdepth[place.branch] == 1)
    {
        // Definite articles
        if (place.branch == BRANCH_ABYSS)
            short_name.insert(0, "The ");
        // Indefinite articles
        else if (place.branch != BRANCH_PANDEMONIUM &&
                 !is_connected_branch(place.branch))
        {
            short_name = article_a(short_name);
        }
    }
    _update_string(force_full, c.place, short_name, "place");
    _update_int(force_full, c.depth, brdepth[place.branch] > 1 ? you.depth : 0, "depth");

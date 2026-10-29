// Trimmed from crawl 0.34.1 for the extractor test: what crawl says of the items on the square
// the player comes to.

static string _item_message(vector<const item_def *> const &items)
{
    vector<string> colour_names;
    for (const item_def *it : items)
        colour_names.push_back(menu_colour_item_name(*it, DESC_A));

    return join_strings(colour_names.begin(), colour_names.end(), "; ");
}

void item_check()
{
    ostream& strm = msg::streams(MSGCH_FLOOR_ITEMS);

    auto items = const_item_list_on_square(you.visible_igrd(you.pos()));

    if (items.size() == 1)
    {
        const item_def& it(*items[0]);
        string name = menu_colour_item_name(it, DESC_A);
        strm << "You see here " << name << '.' << endl;
        return;
    }

    string desc_string = _item_message(items);
    if (static_cast<int>(items.size()) >= Options.item_stack_summary_minimum)
        mprf_nojoin(MSGCH_FLOOR_ITEMS, "Items here: %s.", desc_string.c_str());
    else if (items.size() <= msgwin_lines() - 1)
    {
        mpr_nojoin(MSGCH_FLOOR_ITEMS, "Things that are here:");
        mprf_nocap("%s", desc_string.c_str());
    }
    else
        strm << "There are many items here." << endl;
}

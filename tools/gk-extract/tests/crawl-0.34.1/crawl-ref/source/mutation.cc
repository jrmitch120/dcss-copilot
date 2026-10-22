// Trimmed from crawl 0.34.1 for the extractor test: the messages crawl gives as a bane comes and
// goes, and how it names a bane.

    if (you.banes[bane] == 0)
        mprf(MSGCH_WARN, "You are stricken with the %s.", bane_name(bane).c_str());
    else
        mprf(MSGCH_WARN, "Your %s grows stronger.", bane_name(bane).c_str());

// (trimmed for the extractor test)

void remove_bane(bane_type bane)
{
    mprf(MSGCH_RECOVERY, "The %s upon you is lifted.", bane_name(bane).c_str());
    you.banes[bane] = 0;

// (trimmed for the extractor test)

const string bane_name(bane_type bane, bool dbkey)
{
    string short_name = bane_data[bane_index[bane]].name;

    if (dbkey)
    {
        if (starts_with(short_name, "the"))
            short_name = short_name.substr(4, short_name.length());

        return lowercase(short_name);
    }
    else
        return make_stringf("Bane of %s", short_name.c_str());
}

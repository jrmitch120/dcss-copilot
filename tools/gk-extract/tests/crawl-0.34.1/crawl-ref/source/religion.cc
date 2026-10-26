// Trimmed from crawl 0.34.1 for the extractor test: the god whose followers have a halo, the good
// gods, the gods that change the player's speed or skills, and the messages a god gives as
// the player gains or loses one of their powers.

string god_name(god_type which_god, bool long_name)
{
    switch (which_god)
    {
    // (trimmed for the extractor test)
    case GOD_ZIN:           return "Zin";
    case GOD_SHINING_ONE:   return "the Shining One";
    // (trimmed for the extractor test)
    case GOD_ELYVILON:      return "Elyvilon";
    // (trimmed for the extractor test)
    case GOD_BEOGH:         return "Beogh";
    // (trimmed for the extractor test)
    case GOD_CHEIBRIADOS:   return "Cheibriados";
    // (trimmed for the extractor test)
    case GOD_ASHENZARI:     return "Ashenzari";
    // (trimmed for the extractor test)
    }
}

// (trimmed for the extractor test)

                    power.display(true, "You can now %s.");

// (trimmed for the extractor test)

                    power.display(false, "You can no longer %s.");

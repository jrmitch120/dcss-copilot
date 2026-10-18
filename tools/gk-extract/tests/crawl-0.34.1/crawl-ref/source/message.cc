// Trimmed from crawl 0.34.1 for the extractor test: the message crawl gives for the spell list
// when the player knows no spells.

void canned_msg(canned_message_type which_message)
{
    switch (which_message)
    {
        // (trimmed for the extractor test)
        case MSG_NO_SPELLS:
            mpr("You don't know any spells.");
            break;
        // (trimmed for the extractor test)
    }
}

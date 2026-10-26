// Trimmed from crawl 0.34.1 for the extractor test: the message as the player finishes
// memorising a spell.

void MemoriseDelay::finish()
{
#ifdef USE_SOUND
    parse_sound(MEMORISE_SPELL_SOUND);
#endif
    mpr("You finish memorising.");

    // (trimmed for the extractor test)
}

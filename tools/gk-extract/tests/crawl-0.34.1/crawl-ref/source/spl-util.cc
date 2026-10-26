// Trimmed from crawl 0.34.1 for the extractor test: the message as the player forgets a spell.

bool del_spell_from_memory_by_slot(int slot)
{
    ASSERT_RANGE(slot, 0, MAX_KNOWN_SPELLS);

    if (you.last_cast_spell == you.spells[slot])
        you.last_cast_spell = SPELL_NO_SPELL;

    spell_skills(you.spells[slot], you.skills_to_hide);

    mprf("Your memory of %s unravels.", spell_title(you.spells[slot]));

    // (trimmed for the extractor test)
}

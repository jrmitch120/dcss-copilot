// Trimmed from crawl 0.34.1 for the extractor test: the status lights that tell the player's
// Lighting, and the statuses that stop them reading, drinking or casting.

    case STATUS_NO_POTIONS:
        if (you.duration[DUR_NO_POTIONS] || player_in_branch(BRANCH_COCYTUS)
            || (you.has_mutation(MUT_RENOUNCE_POTIONS)
                && you.props.exists(RENOUNCE_POTIONS_TIMER_KEY)))
        {
            inf.light_colour = !you.can_drink(false) ? DARKGREY : RED;
            inf.light_text   = "-Potion";
            inf.short_text   = "unable to drink";
            inf.long_text    = "You cannot drink potions.";
        }

// (trimmed for the extractor test)

    case DUR_STICKY_FLAME:
    {
        int intensity = you.props[STICKY_FLAME_POWER_KEY].get_int();

        // These thresholds are fairly arbitrary and likely could use adjusting.
        if (intensity >= 13)
        {
            inf.light_colour = LIGHTRED;
            inf.light_text = "Fire++";
        }
        else if (intensity > 7)
            inf.light_text = "Fire+";
        else
            inf.light_text = "Fire";
    }

// (trimmed for the extractor test)

    // Also handled by DUR_SILENCE, see duration-data.h
    case STATUS_SILENCE:
        if (silenced(you.pos()) && !you.duration[DUR_SILENCE])
        {
            // Only display the status light if not using the noise bar.
            if (Options.equip_bar)
            {
                inf.light_colour = LIGHTRED;
                inf.light_text   = "Sil";
            }
            inf.short_text   = "silenced";
            inf.long_text    = "You are silenced.";
        }
        if (Options.equip_bar && you.duration[DUR_SILENCE])
        {
            inf.light_colour = LIGHTMAGENTA;
            inf.light_text = "Sil";
        }
        break;

// (trimmed for the extractor test)

        if (you.duration[DUR_NO_SCROLLS] || player_in_branch(BRANCH_GEHENNA)
            || (you.has_mutation(MUT_RENOUNCE_SCROLLS)
                && you.props.exists(RENOUNCE_SCROLLS_TIMER_KEY)))
        {
            inf.light_colour = RED;
            inf.light_text   = "-Scroll";
            inf.short_text   = "unable to read";
            inf.long_text    = "You cannot read scrolls.";
        }

// (trimmed for the extractor test)

static void _describe_glow(status_info& inf)
{
    // Don't show a status light until we have contam that does something.
    if (!player_harmful_contamination())
        return;

    if (you.magic_contamination >= 2000)
        inf.light_colour = RED;
    else
        inf.light_colour = YELLOW;
    inf.light_text = "Contam";
}

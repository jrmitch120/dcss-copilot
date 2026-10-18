// Trimmed from crawl 0.34.1 for the extractor test: the status lights that tell the player's
// Lighting.

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

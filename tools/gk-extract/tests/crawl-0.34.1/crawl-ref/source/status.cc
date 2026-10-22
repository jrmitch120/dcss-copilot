// Trimmed from crawl 0.34.1 for the extractor test: the status lights that tell the player's
// Lighting or speed, the statuses that stop them reading, drinking or casting, and lights built
// as they are drawn.

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

// (trimmed for the extractor test)

        else
        {
            inf.light_colour = MAGENTA;

            if (player_has_ability(ABIL_YRED_HURL_TORCHLIGHT))
            {
                inf.light_text = make_stringf("Torch (%d)",
                                    yred_get_torch_power());
            }
            else
                inf.light_text = "Torch";

            inf.short_text = "lit torch";

// (trimmed for the extractor test)

            inf.short_text   = "unable to drink";
            inf.long_text    = "You cannot drink potions.";
        }
        break;

    case DUR_SWIFTNESS:
        if (you.attribute[ATTR_SWIFTNESS] < 0)
        {
            inf.light_text   = "-Swift";
            inf.light_colour = RED;

// (trimmed for the extractor test)

    case STATUS_LIQUEFIED:
    {
        if (you.liquefied_ground() || you.duration[DUR_LIQUEFYING])
        {
            inf.light_colour = BROWN;
            inf.light_text   = "SlowM";
            inf.short_text   = "slowed movement";
            inf.long_text    = "Your movement is slowed on this liquid ground.";
        }
        break;
    }

// (trimmed for the extractor test)

static void _describe_speed(status_info& inf)
{
    bool slow = you.duration[DUR_SLOW] || have_stat_zero();
    bool fast = you.duration[DUR_HASTE];

    if (slow && fast)
    {
        inf.light_colour = MAGENTA;
        inf.light_text   = "Fast+Slow";
        inf.short_text   = "hasted and slowed";
        inf.long_text = "You are under both slowing and hasting effects.";
    }
    else if (slow)
    {
        inf.light_colour = RED;
        inf.light_text   = "Slow";
        inf.short_text   = "slowed";
        inf.long_text    = "You are slowed.";
    }
    else if (fast)
    {
        inf.light_colour = _dur_colour(BLUE, dur_expiring(DUR_HASTE));
        inf.light_text   = "Fast";
        inf.short_text = "hasted";
        inf.long_text = "Your actions are hasted.";
        _mark_expiring(inf, dur_expiring(DUR_HASTE));
    }
}

static void _describe_airborne(status_info& inf)
{
    if (!you.airborne())
        return;

    const bool perm      = you.permanent_flight();
    const bool expiring  = (!perm && dur_expiring(DUR_FLIGHT));
    const bool emergency = you.props[EMERGENCY_FLIGHT_KEY].get_bool();

    inf.light_colour = perm ? WHITE : emergency ? LIGHTRED : BLUE;
    inf.light_text   = "Fly";
    inf.short_text   = "flying";
    inf.long_text    = "You are flying.";
    inf.light_colour = _dur_colour(inf.light_colour, expiring);
    _mark_expiring(inf, expiring);
}


// (trimmed for the extractor test)

static void _describe_transform(status_info& inf)
{
    if (you.form == transformation::none)
        return;

    const Form * const form = get_form();
    inf.light_text = form->short_name;
    inf.short_text = form->get_long_name();
    inf.long_text = form->get_description();
}

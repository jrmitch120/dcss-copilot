#include "AppHdr.h"

#include "spl-zap.h"

#include "beam.h"
#include "stepdown.h"
#include "zap-type.h"

static pair<spell_type, zap_type> _spl_zaps[] =
{
    { SPELL_MAGIC_DART, ZAP_MAGIC_DART },
    { SPELL_FIREBALL, ZAP_FIREBALL },
    { SPELL_SLOW, ZAP_SLOW },
    { SPELL_UNLEASH_DESTRUCTION, ZAP_UNLEASH_DESTRUCTION },
    { SPELL_UNLEASH_DESTRUCTION, ZAP_UNLEASH_DESTRUCTION },
    { SPELL_UNLEASH_DESTRUCTION, ZAP_UNLEASH_DESTRUCTION_PIERCING },
    // (trimmed for the extractor test)
};

zap_type spell_to_zap(spell_type spell)
{
    for (const auto &spzap : _spl_zaps)
        if (spzap.first == spell)
            return spzap.second;

    return NUM_ZAPS;
}

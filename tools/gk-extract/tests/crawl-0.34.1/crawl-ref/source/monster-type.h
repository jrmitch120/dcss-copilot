#pragma once

#include "tag-version.h"

// NOTE: Changing this order will break saves! Appending does not.
enum monster_type                      // env.mons[].type
{
    MONS_PROGRAM_BUG,
        MONS_0 = MONS_PROGRAM_BUG,

#if TAG_MAJOR_VERSION > 34
    MONS_GIANT_LIZARD,          // genus
#endif
    MONS_FRILLED_LIZARD,
#if TAG_MAJOR_VERSION == 34
    MONS_LEOPARD_GECKO,
#endif
    MONS_IGUANA,
    MONS_KOMODO_DRAGON,
    MONS_BASILISK,
#if TAG_MAJOR_VERSION > 34
    MONS_SEISMOSAURUS,
#endif
    MONS_BAT,
    MONS_FIRE_BAT,
#if TAG_MAJOR_VERSION > 34
    MONS_OBSIDIAN_BAT,
    MONS_SNAKE,                // genus
#endif
    MONS_BALL_PYTHON,
    MONS_ADDER,
    MONS_WATER_MOCCASIN,
    MONS_BLACK_MAMBA,
    MONS_ANACONDA,
    MONS_SEA_SNAKE,
#if TAG_MAJOR_VERSION > 34
    MONS_SHOCK_SERPENT,
    MONS_MANA_VIPER,
#endif
    MONS_RAT,
#if TAG_MAJOR_VERSION == 34
    MONS_GREY_RAT,
#endif
    MONS_RIVER_RAT,
    MONS_HELL_RAT,
#if TAG_MAJOR_VERSION == 34
    MONS_LABORATORY_RAT,
#endif
    MONS_QUOKKA,         // Quokka are a type of wallaby, returned -- bwr 382
#if TAG_MAJOR_VERSION == 34
    MONS_PORCUPINE,
#endif
#if TAG_MAJOR_VERSION > 34
    MONS_CRYSTAL_ECHIDNA,
    MONS_ARMATAUR,
#endif
    MONS_JACKAL,
    MONS_HOUND,
#if TAG_MAJOR_VERSION == 34
    MONS_WAR_DOG,
#else
    MONS_HOWLER_MONKEY,
#endif
    MONS_WOLF,
    // (trimmed for the extractor test)
    MONS_SNAPPING_TURTLE,
    // (trimmed for the extractor test)
    MONS_GOBLIN_RIDER,
    // (trimmed for the extractor test)
    MONS_ORC_WIZARD,
    // (trimmed for the extractor test)
    MONS_BLACK_DRACONIAN,
    // (trimmed for the extractor test)
    MONS_KOBOLD,
    // (trimmed for the extractor test)
    MONS_CENTAUR,
    // (trimmed for the extractor test)
    MONS_MAURICE,
    // (trimmed for the extractor test)
    MONS_FUNGUS,
    // (trimmed for the extractor test)
    MONS_KRAKEN,
    MONS_KRAKEN_TENTACLE,
    MONS_KRAKEN_TENTACLE_SEGMENT,
    // (trimmed for the extractor test)
    MONS_SNAPLASHER_VINE,
    MONS_SNAPLASHER_VINE_SEGMENT,
    // (trimmed for the extractor test)
    MONS_TENTACLED_STARSPAWN,
    // (trimmed for the extractor test)
    MONS_STARSPAWN_TENTACLE,
    MONS_STARSPAWN_TENTACLE_SEGMENT,
    // (trimmed for the extractor test)
    MONS_ELDRITCH_TENTACLE,
    MONS_ELDRITCH_TENTACLE_SEGMENT,
    // (trimmed for the extractor test)
    MONS_TEST_SPAWNER,
    // (trimmed for the extractor test)
    MONS_TEST_STATUE,
    // (trimmed for the extractor test)
    MONS_TEST_BLOB,
    // (trimmed for the extractor test)
    MONS_PHANTOM,
    // (trimmed for the extractor test)
    MONS_DEATH_KNIGHT,
    // (trimmed for the extractor test)
    MONS_FULMINANT_PRISM,
    MONS_SHADOW_PRISM,
    MONS_SPLINTERFROST_BARRICADE,
    MONS_BLAZEHEART_CORE,
    MONS_BLAZEHEART_GOLEM,
    MONS_RENDING_BLADE,
    MONS_BURSTSHROOM,
    MONS_SIXFIRHY,
    MONS_JIANGSHI,
    MONS_BOULDER,
    MONS_HELLFIRE_MORTAR,
    MONS_DIAMOND_SAWBLADE,
    MONS_LIGHTNING_SPIRE,
    MONS_SOLAR_EMBER,
    MONS_BOUNDLESS_TESSERACT,
    MONS_WANDERING_MUSHROOM,
    MONS_DEATHCAP,
    MONS_LURKING_HORROR,
    MONS_UGLY_THING,
    MONS_VERY_UGLY_THING,
    MONS_SLIME_CREATURE,
    MONS_CLOCKWORK_BEE_INACTIVE,
    MONS_ORB_SPIDER,
    MONS_LAVA_SNAKE,
    // (trimmed for the extractor test)

    NUM_MONSTERS,               // used for polymorph

    // MONS_NO_MONSTER can get put in savefiles, so it shouldn't change
    // when NUM_MONSTERS increases.
    MONS_NO_MONSTER = 1000,

    RANDOM_MONSTER = 2000, // used to distinguish between a random monster and using program bugs for error trapping {dlb}
    RANDOM_MOBILE_MONSTER, // used for monster generation (shadow creatures)
    RANDOM_COMPATIBLE_MONSTER, // used for player shadow creatures (prevents repulsing summons)
    RANDOM_BANDLESS_MONSTER,
    RANDOM_POLYMORPH_MONSTER, // choose from a per-monster set

    // A random draconian, either base coloured drac or specialised.
    RANDOM_DRACONIAN,
    // Any random base draconian colour.
    RANDOM_BASE_DRACONIAN,
    // Any random specialised draconian, such as a draconian knight.
    RANDOM_NONBASE_DRACONIAN,

    RANDOM_DEMON_LESSER,               //    0: Class V
    RANDOM_DEMON_COMMON,               //    1: Class III-IV
    RANDOM_DEMON_GREATER,              //    2: Class I-II
    RANDOM_DEMON,                      //    any of the above

    MONS_ORB_OF_APPROPRIATENESS,    // The orb type chosen to populate Zot on this seed.

    RANDOM_MODERATE_OOD, // +5 depth, AKA '9' glyph on maps
    RANDOM_SUPER_OOD, // *2 + 4 depth, AKA '8'

    WANDERING_MONSTER = 3500, // only used in monster placement routines - forced limit checks {dlb}

    // (trimmed for the extractor test) The monsters the gear rules name, appended so that no
    // value above moves.
    MONS_KOBOLD_BRIGAND,
    MONS_GOBLIN,
    MONS_CYCLOPS,
    MONS_STONE_GIANT,
    MONS_TAINTED_LEVIATHAN,
    MONS_PARGHIT,
    MONS_SONJA,
    MONS_SPRIGGAN_RIDER,
    MONS_ORC_WARRIOR,
    MONS_ORC,
    MONS_URUG,
    MONS_CHUCK,
    MONS_POLYPHEMUS,
    MONS_MERFOLK_JAVELINEER,
    MONS_MINOTAUR,
    MONS_MERFOLK,
    MONS_DRACONIAN_KNIGHT,
    MONS_GNOLL,
    MONS_HAROLD,
};
COMPILE_CHECK(NUM_MONSTERS < MONS_NO_MONSTER);

#pragma once

#include "tag-version.h"

// (trimmed for the extractor test)

// This is done to avoid duplicating the Depths list and can be
// changed once TAG_MAJOR_VERSION > 35
#define POP_DEPTHS \
{ /* Depths (OOD cap: 13)*/ \
  {  0,  2,  280, FALL, MONS_UGLY_THING },\
  {  0,  7,  600, FALL, MONS_STONE_GIANT },\
}

// This list must be in the same order as the branch-type enum values.
static const vector<pop_entry> population[] =
{

{ // Dungeon (OOD cap: 27)

// The weakest of the weak.
  {  1,  3, 1000, FLAT, MONS_BAT },
  {  1,  3,  175, FLAT, MONS_JACKAL },
  {  4,  5,  175, FALL, MONS_JACKAL },
  {  2,  6, 1000, FLAT, MONS_ADDER },
  {  6, 12,  600, PEAK, MONS_ORC_WARRIOR },
  { 21, 27,  310, FALL, MONS_STONE_GIANT },
},

{ // Temple
},

{ // Orcish Mines
  {  1,  4,  190, FLAT, MONS_ORC_WARRIOR },
  {  1,  4,   24, FLAT, MONS_ORC_WIZARD },
},

{ // Elven Halls (OOD cap: 7)
},
#if TAG_MAJOR_VERSION == 34
{ // Dwarven Hall
  {  1,  1,  690, FLAT, MONS_DEATH_KNIGHT },
},
#endif

{ // Lair (OOD cap: 11)
  { -2,  6,  310, SEMI, MONS_BASILISK },
  { -1,  5,  310, SEMI, MONS_WOLF },
},

{ // Swamp
  {  1,  4,   500, FALL, MONS_SPRIGGAN_RIDER },
},

{ // Shoals
  {  0,  6,  110, SEMI, MONS_CYCLOPS },
},

{ // Snake Pit
  {  0,  8, 1125, SEMI, MONS_BLACK_MAMBA },
},

{ // Spider Nest
  {  1,  4,   200, FLAT, MONS_ORB_SPIDER },
},

{ // Slime Pits
  {  1,  5,  950, FLAT, MONS_SLIME_CREATURE },
},

{ // The Vaults (OOD cap: 12)
  {  1,  4,  400, FLAT, MONS_SLIME_CREATURE },
},

#if TAG_MAJOR_VERSION == 34
{ // Hall of Blades
},
#endif

{ // Crypt (OOD cap: 5)
  { -3,  3,   75, SEMI, MONS_DEATH_KNIGHT },
  { -1,  5,   75, PEAK, MONS_JIANGSHI },
},

{ // Tomb (OOD cap: 5)
},
#if TAG_MAJOR_VERSION > 34
POP_DEPTHS,
#endif

{ // Hell
  {  1,  1,   89, FLAT, MONS_HELL_RAT },
},

// Hell branches: OOD Cap 14
// "cross-hell" threats (good for regular rolls, fall off and
// let branch flavour take over as depth goes 8-12)
#define CROSS_HELL_POP \
  {  1,  7,  100, FLAT, MONS_HELLION },\
  {  8, 12,  100, FALL, MONS_HELLION }

{ // Dis
  CROSS_HELL_POP,
},

{ // Gehenna
  CROSS_HELL_POP,
},

{ // Cocytus
  CROSS_HELL_POP,
},

{ // Tartarus
  CROSS_HELL_POP,
  {  1,  7,  400, RISE, MONS_TAINTED_LEVIATHAN },
},

{ // Zot
  {  1,  5,  100, FLAT, MONS_BLACK_DRACONIAN },
  {  1,  5,   67, FLAT, MONS_DRACONIAN_KNIGHT },
},
#if TAG_MAJOR_VERSION == 34
{ // Forest
  {  1,  5,  120, FALL, MONS_WOLF },
},
#endif

{ // Abyss
  {  1,  8,   91, FALL, MONS_SIXFIRHY },
},

{ // Pandemonium - only used for 1 in 40 random Pan spawns (the rest are drawn
  // from the pandemonium lord's list)
},

{ // Ziggurat
},

#if TAG_MAJOR_VERSION == 34
{ // Labyrinth
},
#endif

{ // Bazaar
},

{ // Trove
},

{ // Sewer
  {  1,  1, 1000, FLAT, MONS_BAT },
},

{ // Ossuary
},

{ // Bailey
},

#if TAG_MAJOR_VERSION > 34
{ // Gauntlet
},
#endif

{ // Ice Cave
  {  1,  1,   90, FLAT, MONS_WOLF },
},

{ // Volcano
},

{ // Wizlab
},

#if TAG_MAJOR_VERSION == 34
POP_DEPTHS,
#endif

{ // Desolation
},

#if TAG_MAJOR_VERSION == 34
{ // Gauntlet
},
#endif

{ // Arena
},

{ // Crucible
},

{ // Necropolis
},
};

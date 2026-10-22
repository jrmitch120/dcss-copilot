// Trimmed from crawl 0.34.1 for the extractor test: the mutations that change the player's speed.

#include "tag-version.h"

struct mutation_def
{
    mutation_type mutation;
    short       weight;     ///< Commonality of the mutation; bigger = appears
                            /// more often.
    short       levels;     ///< The number of levels of the mutation.
    mutflags    flags;      ///< Bitfield holding information about sources of
                            /// this mutation and how it interacts with forms.
                            /// (mutflag::*)
    const char* short_desc; ///< What appears on the '%' screen.
    const char* have[3];    ///< What appears on the 'A' screen.
    const char* gain[3];    ///< Message when you gain the mutation.
    const char* lose[3];    ///< Message when you lose the mutation.
    tileidx_t   tile;       ///< Icon for the mutation.
    const char* will_gain[3]; ///< Message on the 'A' screen future XL gain of the mutation.
};

struct mutation_category_def
{
  mutation_type mutation;
  const char* short_desc;
};

static const mutation_def mut_data[] =
{

{ MUT_FAST, 0, 3, mutflag::good,
  "speed",

  {"You move with supernatural speed. (Speed+)",
   "You move with supernatural speed. (Speed++)",
   "You move with supernatural speed. (Speed+++)"},

  {"You feel quick.",
   "You feel quick.",
   "You feel quick."},

  {"You feel sluggish.",
   "You feel sluggish.",
   "You feel sluggish."},
},

{ MUT_SLOW, 0, 3, mutflag::bad,
  "slowness",

  {"You cover ground slowly.",
   "You cover ground very slowly.",
   "You cover ground extremely slowly."},

  {"You feel sluggish.",
   "You feel sluggish.",
   "You feel sluggish."},

  {"You feel quick.",
   "You feel quick.",
   "You feel quick."},
},
// (trimmed for the extractor test)

{ MUT_CONSTRICTING_TAIL, 0, 2, mutflag::good | mutflag::anatomy,
  "naga tail",

  {"Your snake-like lower body moves slowly.",
   "Your snake-like lower body moves slowly, but can constrict enemies.", ""},
  {"Your lower body turns into a snake tail.",
   "Your tail grows strong enough to constrict your enemies.", ""},
  {"Your lower body returns to normal.",
   "Your snake tail weakens and can no longer constrict your enemies.", ""},
   TILEG_MUT_NAGA_TAIL,
  {0, "Your tail will grow strong enough to constrict your foes."},
},

{ MUT_FROG_LEGS, 0, 2, mutflag::good | mutflag::anatomy,
  "frog-like legs",

  {"You can hop short distances, but move slowly.",
   "You can hop long distances, but move slowly.",
   ""},

  {"", "Your legs feel stronger.", ""},

  {"", "", ""},

  TILEG_MUT_HOP,
},

// (trimmed for the extractor test)
{ MUT_NIMBLE_SWIMMER, 0, 2, mutflag::good | mutflag::anatomy,
  "nimble swimmer",

  {"You are camouflaged when in or above water. (Stealth+)",
   "You are very nimble when in or above water. (Stealth+, EV+, Speed+++)", ""},
  {"You feel comfortable near water.",
   "You feel very comfortable near water.", ""},
  {"You feel less comfortable near water.",
   "You feel less comfortable near water.", ""},
},
};

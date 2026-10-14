// PROTOTYPE. Six hand-written Fact Sheets. Numbers are invented but meant to be internally
// consistent; monster details need a DCSS-realism check before the run.
// `expected` is the pick we agreed on beforehand; it is shown to the judge only after rating.

export const SCENARIOS = [
  {
    id: "elf-pack",
    title: "Mid-game fight: deep elves on Elf:2",
    kind: "tactical",
    expected: "read-tele",
    trap: false,
    sheet: `Character: Gargoyle Fighter XL13, Elf:2, turn 21,840
HP 71/118 (60%)  MP 6/6  AC 24 EV 6 SH 15  Speed: normal (10)
Statuses: none   Resists: rPois, rN+, rElec (gargoyle), Torment immune
Inventory (relevant): 2 ?teleportation, 1 ?blinking, 2 !heal wounds, 1 !haste, 0 !curing

THREAT LEVEL: LETHAL
  P(death before your next action): 1.1%
  P(death within 5 turns if you stay in melee range): 31%
  Worst-case hit this turn (99th pct): 38
  Rule: LETHAL because P(death within 5 turns) ≥ 25%

Monsters (can act before your next action):
  deep elf sorcerer  dist 5  speed 10  spells: Haste Other, Banishment, Paralyse, Invisibility  (spell set assumed full from Game Knowledge)
  deep elf knight ×2  dist 2 and 2  speed 10  melee 2×(up to 30), reach none
  Missing: sorcerer's current HP unknown (out of reach of examine)

Fight outcome (≤3 monsters, simulated 10,000×):
  Melee (+7 broad axe):  42% win, ~70% HP lost when won
  Not modelled: Haste Other on the knights, Banishment, Paralyse duration (treated as 1 cast, average)

Ways Out:
  ✓ Upstairs '<'  4 steps, open corridor   ✕ both knights are as fast as you and start 2 away; expected 3 knight attacks before you climb
  ✓ ?teleportation  acts in 3–5 turns; no -Tele
  ✓ ?blinking  up to 7 squares; nearest blink square still in the sorcerer's line of sight
  ✓ !heal wounds  heals ~29 avg
  ✓ !haste  doubles your speed for ~16 turns

Candidate Actions:
  fight-melee     Step in and fight the knights (sim: 42% win)
  flee-upstairs   Walk 4 steps to '<' (expected 3 knight attacks first)
  read-tele       Read ?teleportation, survive 3–5 turns, land elsewhere on Elf:2
  quaff-haste     Quaff !haste, then walk to '<' (outpaces the knights)
  read-blink      Read ?blinking away from the knights (stays in sorcerer's LOS)

Suggested Action (rules): flee-upstairs   — rule: "LETHAL and stairs within 5 steps → take stairs"`,
  },
  {
    id: "sigmund",
    title: "Early unique: Sigmund on D:4 (trap)",
    kind: "tactical",
    expected: "back-to-stairs",
    trap: true,
    sheet: `Character: Hill Orc Berserker XL5, D:4, turn 3,912
HP 30/50 (60%)  MP 0/0  AC 7 EV 9  Speed: normal (10)
Statuses: none   Resists: none
God: Trog (piety ** ), ability: Berserk (available)
Inventory (relevant): 1 !curing, 0 ?teleportation, 1 unknown scroll (×1), 1 unknown potion (×2)

THREAT LEVEL: DANGEROUS
  P(death before your next action): 0.4%
  P(death in a full fight, see below): 23%
  Worst-case hit this turn (99th pct): 19
  Rule: DANGEROUS because worst-case hit ≥ 33% of max HP

Monsters:
  Sigmund (unique)  dist 3  speed 10  weapon: scythe  spells: Confuse, Haste Self, Invisibility  (spell set from Game Knowledge)
  Missing: Sigmund's current HP unknown

Fight outcome (simulated 10,000×):
  Melee (hand axe):              61% win, ~65% HP lost when won
  Berserk then melee:            77% win, ~50% HP lost when won
  Not modelled: Confuse while berserk (treated as one turn lost), Haste Self duration, Invisibility

Ways Out:
  ✓ Upstairs '<'  3 steps, corridor   Sigmund is as fast as you and starts 3 away; he reaches you as you arrive, expected 0–1 attacks
  ✓ !curing  cures confusion; heals ~6 avg
  ✕ Berserk as an escape  cannot flee effectively while berserk; slowed afterwards

Candidate Actions:
  berserk-fight     Berserk and fight (sim: 77% win)
  melee-fight       Fight without berserking (sim: 61% win)
  back-to-stairs    Walk 3 steps to '<', climb, fight him at the top or leave him
  read-unknown      Read the unknown scroll (could be anything)

Suggested Action (rules): berserk-fight   — rule: "melee build, best sim win ≥ 75% → fight with best plan"`,
  },
  {
    id: "branch-choice",
    title: "Decision Point: Lair cleared, choose next branch",
    kind: "decision",
    expected: "orc",
    trap: false,
    sheet: `Character: Minotaur Fighter XL12, Lair:5 (cleared), turn 16,200
HP 112/112  MP 3/3  AC 19 EV 9 SH 0  Speed: normal
Resists: rF+ (ring), rPois — no, rC — no, SInv — no
Weapon: +4 great sword (Long Blades 14, Fighting 12, Armour 9)
Runes: 0. Gold: 512.

DECISION POINT: Lair cleared; branches known: Orc:1 (entrance on D:13), Shoals:1 (entrance on Lair:3), Spider:1 (entrance on Lair:2)
THREAT LEVEL: CALM

Branch facts (from Game Knowledge and community stats for this species/XL band):
  Orc:1–2    typical monsters: orc warriors, orc priests, orc knights, ogres; poison: rare
             death share at this XL band (community logs): 3%
  Shoals:1–4 typical monsters: merfolk impalers/javelineers, sirens, water moccasins, Ilsuiw (unique)
             poison: low; water and ranged attackers; death share: 9%
  Spider:1–4 typical monsters: wolf spiders, demonic crawlers, emperor scorpions, Arachne (unique)
             poison: very high (rPois strongly recommended); webs; death share: 14%
  Missing: what's still unexplored on D:12–D:15 (D:13 explored, D:14–15 not)

Run Plan (current):
  route: D:1–D:12 ✓, Lair ✓, ???, Vaults:1–4, Depths, Vaults:5
  skill_targets: Long Blades 18, Fighting 15, Armour 12
  needs: rPois (urgent before Spider/Snake), SInv (before Depths), better body armour
  goal: 3-rune win

Candidate Actions:
  orc       Go to Orc:1 next (Orc:1–2)
  shoals    Go to Shoals:1 next (Shoals:1–4)
  spider    Go to Spider:1 next (Spider:1–4)

Suggested Action (rules): orc   — rule: "lowest community death share at this XL band"`,
  },
  {
    id: "ogre-mage",
    title: "Out-of-depth ogre mage, unnoticed (trap)",
    kind: "tactical",
    expected: "leave-quietly",
    trap: true,
    sheet: `Character: Deep Elf Conjurer XL8, D:7, turn 6,410
HP 31/34 (91%)  MP 18/22  AC 3 EV 13  Speed: normal
Spells: Magic Dart (fail 1%), Searing Ray (fail 4%), Fulminant Prism (fail 11%)
Inventory (relevant): 1 ?blinking, 1 !heal wounds

THREAT LEVEL: CALM
  P(death before your next action): 0%
  Rule: CALM because no monster that can act has noticed you

Monsters:
  ogre mage  dist 7  speed 10  NOT NOTICED YOU (wandering)  out of depth (usual depth D:12+)
             spells: Haste Other, Invisibility, Paralyse, Slow  (spell set from Game Knowledge)
  Missing: ogre mage HP and whether it has unseen allies (unexplored corridor behind it)

Fight outcome if you attack first (simulated 10,000×):
  Searing Ray then Magic Dart:   70% win, ~40% HP lost when won
  P(death in this fight):        24%
  Not modelled: Haste Other / Invisibility on unseen allies, Paralyse chain length (treated as average)

Ways Out:
  ✓ Downstairs '>'  14 steps, behind you, away from the ogre mage
  ✓ Upstairs '<'  22 steps, behind you
  ✓ ?blinking  up to 7 squares
  ✓ !heal wounds  heals ~29 avg

Candidate Actions:
  fight-searing    Open with Searing Ray, finish with Magic Dart (sim: 70% win)
  leave-quietly    Walk back to '>' (14 steps) and leave the level without waking it
  explore-other    Keep exploring away from it on this level

Suggested Action (rules): fight-searing   — rule: "best sim win ≥ 65% and Threat CALM → fight"`,
  },
  {
    id: "quick-blade",
    title: "Great Find spotted: +2 quick blade in a shop",
    kind: "decision",
    expected: "save-and-return",
    trap: false,
    sheet: `Character: Spriggan Enchanter XL9, D:8, turn 7,330
HP 38/38  MP 14/14  AC 2 EV 24  Stealth: Superb
Weapon: +1 dagger (Short Blades 10, Stealth 13, Hexes 8)
Gold: 450

DECISION POINT: Great Find spotted (not obtained)
  +2 quick blade, shop on D:8 ("Hrund's Antique Weapons"), price 600 gold
  Engine tier: Run-changing (fills urgent need "short blade upgrade"; same skill as current weapon)
  Comparison vs current +1 dagger, against reference set (D:9–Lair monsters):
    damage per turn: 7.9 vs 4.6   turns to kill (avg): 3.1 vs 5.4   stab damage: about equal
  Gold: you have 450, need 600 (short by 150)
  Your gold income: 210 over the last 3 floors (D:5–D:7)
  Missing: other shops on D:9+ not yet seen

THREAT LEVEL: CALM

Run Plan (current):
  route: D:1–D:11, Lair, Orc, Shoals or Swamp, Vaults:1–4
  skill_targets: Short Blades 16, Stealth 15, Hexes 12, Dodging 10
  needs: short blade upgrade (urgent), rPois, ranged option for things you can't stab
  goal: 3-rune win

Candidate Actions:
  save-and-return   Keep exploring; return to this shop at 600 gold
  buy-other         Spend gold now on this shop's +0 rapier (220 gold) instead
  skip              Ignore the shop

Suggested Action (rules): save-and-return   — rule: "Run-changing Great Find unaffordable → save and return"`,
  },
  {
    id: "golem-walk",
    title: "Low HP, no good Way Out, slow monster",
    kind: "tactical",
    expected: "walk-to-stairs",
    trap: false,
    sheet: `Character: Formicid Fighter XL15, Depths:2, turn 27,150
HP 34/136 (25%)  MP 2/2  AC 27 EV 7 SH 18  Speed: normal (10)
Statuses: -Tele (Formicid: permanent stasis — no teleport, blink or haste)
Inventory (relevant): 0 !heal wounds, 1 !curing, 1 ?fog, 1 ?fear

THREAT LEVEL: DANGEROUS
  P(death before your next action): 0%  (iron golem is 2 squares away and slower)
  Worst-case hit when adjacent (99th pct): 56
  Rule: DANGEROUS because worst-case hit ≥ HP would be reached if it closes

Monsters:
  iron golem  dist 2  speed 7 (slower than you)  melee up to 55  no ranged attack
  Missing: none

Fight outcome (simulated 10,000×):
  Melee (+5 executioner's axe):   22% win
  Not modelled: none for this fight

Ways Out:
  ✓ Upstairs '<'  9 steps, clear path; golem is slower: it never reaches you if you walk now
     P(death en route): 1.2% (only if the path is blocked by something unseen)
  ✓ !curing  heals ~6 avg
  ✓ ?fog  blocks line of sight; golem still follows by path
  ✓ ?fear  golem is mindless: ✕ immune to fear
  ✕ ?teleportation / ?blinking  none in inventory, and -Tele

Candidate Actions:
  walk-to-stairs   Walk 9 steps to '<' and climb (golem never reaches you)
  fight            Melee the iron golem (sim: 22% win)
  quaff-curing     Quaff !curing first (heals ~6), then decide
  read-fog         Read ?fog, then walk

Suggested Action (rules): walk-to-stairs   — rule: "a faster-than-all-threats path to stairs exists → take it"`,
  },
];

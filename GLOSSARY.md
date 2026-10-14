# DCSS Copilot

A read-only companion to Dungeon Crawl Stone Soup that watches a game as it is played and offers information and advice to the player.

## Language

**Advisor**:
The companion's role: it reads the game and advises, but never sends input to the game. The player always presses the keys.
_Avoid_: Bot, assistant, autopilot

**Game Source**:
A place a live game's state can be read from, such as a webtiles server or local webtiles.
_Avoid_: Integration, connector, backend

**Recording**:
A saved copy of everything a Game Source sent during one game, which can be replayed to reproduce what the Advisor saw. The player can opt in to recording and export a game to report a problem.
_Avoid_: Log, trace, capture (when meaning the saved file)

### Game situation

**Game Situation**:
Everything the Advisor knows when it reasons about a moment in a game: Visible State, Accumulated State and Game Knowledge together.
_Avoid_: Context, game state (when meaning the whole)

**Visible State**:
What the game currently reports to the player: map in view, monsters, stats, inventory, statuses, messages, and any screen the player has open (overview, skills, spells, character dump).
_Avoid_: Screen, snapshot

**Accumulated State**:
What the Advisor has remembered across turns of one game that the game no longer shows, such as explored levels, items seen, uniques encountered, what it read from screens the player opened, and the Run Plan. Each fact remembers where it came from and how recent it is.
_Avoid_: History, memory, cache

**Game Knowledge**:
Static facts about DCSS itself for one Crawl Version: monster, spell, item, god and species data. When the Advisor has none for the exact Crawl Version being played, it uses the nearest it has and tells the player its advice is approximate.
_Avoid_: Wiki data, database

**Crawl Version**:
The exact build of DCSS a game is played on: a stable release such as 0.34.1, or a particular trunk build.
_Avoid_: Game version, release (when trunk is included)

### Analysis

**Fact Sheet**:
Everything the Advisor has computed about one moment of a Game Situation, such as Threat Level, Ways Out, item comparisons, failure rates and which inputs are missing. Every number the Advisor shows or says comes from it.
_Avoid_: Situation, snapshot, context, prompt data

**Threat Level**:
How dangerous the current moment is, as one of LETHAL, DANGEROUS or CALM, judged from the chance of dying before the player's next action and the worst-case hit.
_Avoid_: Danger score, threat score

**Way Out**:
An escape or emergency option, such as a consumable, ability, spell or staircase, together with whether it can be used right now and why not.
_Avoid_: Escape (when meaning the whole list), panic button

**Gap**:
A fact the Advisor needs about this game but doesn't have, or has only an out-of-date copy of. The Advisor says so rather than guessing, and tells the player which screen to open to fill it.
_Avoid_: Unknown, missing data

**Candidate Action**:
Something the player could legally do right now, such as attacking, fleeing to a staircase or using a Way Out, together with its facts from the Fact Sheet. The Advisor only ever recommends a Candidate Action.
_Avoid_: Option, move, choice

**Suggested Action**:
The Candidate Action the Advisor's own rules pick for a dangerous moment, shown before (and without) any LLM judgment.
_Avoid_: Default action, rule pick

**Encounter**:
A stretch of danger, from the moment a hostile comes into view or the Threat Level leaves CALM until the player is All Clear. The Advisor raises each Tactical Alert at most once per Encounter for the same cause.
_Avoid_: Fight, combat, engagement

**All Clear**:
The player has stayed out of danger for a few actions in a row: the Threat Level is CALM, no hostile is in view, and no dangerous status is active. It ends an Encounter and releases held Strategic Advice.
_Avoid_: Calm (which is a Threat Level), safe, lull

### Advice

**Alert**:
Advice the Advisor raises on its own, without being asked, because something meaningful happened in the game.
_Avoid_: Notification, ping

**Tactical Advice**:
Guidance about the immediate situation: threat assessment, retreat, which consumable or ability to use.

**Strategic Advice**:
Guidance spanning the run: branch order, god choice, skill training.

**Decision Point**:
A moment in a game that calls for Strategic Advice, such as finding an item, gaining an experience level, discovering a branch or clearing a floor. Strategic Advice is raised at Decision Points and held back during danger.
_Avoid_: Trigger, event (when meaning this)

**Run Plan**:
The Advisor's current plan for one game: intended route, skill targets, outstanding needs and overall goal. The Advisor keeps it updated and the player can steer it.
_Avoid_: Build, strategy (when meaning this artifact)

**Great Find**:
An item, spell or upgrade that is exceptionally valuable to this particular game, judged against its Run Plan rather than general rarity. Tiered as Nice, Great or Run-changing. Ones seen but not yet obtained (on the floor, in a shop) are Spotted.
_Avoid_: Loot alert, rare drop

**Information**:
Factual lookups the player asks for or that are surfaced alongside play, such as monster details or a resistance breakdown. Not a recommendation.
_Avoid_: Advice (when no recommendation is made)

### Cost

**Quick Model**:
The model the player has chosen for everyday advice: Alerts, Decision Point nudges, Coach advice and ordinary questions.
_Avoid_: Fast model, default model, cheap model

**Deep Model**:
The model the player has chosen for questions they explicitly ask the Advisor to think harder about.
_Avoid_: Smart model, big model

**Game Budget**:
The most the player is willing to spend on LLM advice in one game. The player can top it up during a run.
_Avoid_: Run budget, quota, cap

**Step Down**:
What the Advisor does when the Game Budget runs out: it drops to a lower assistance level rather than going quiet.
_Avoid_: Throttle, cut off

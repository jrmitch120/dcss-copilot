# DCSS Copilot

A read-only companion to Dungeon Crawl Stone Soup that watches a game as it is played and offers information and advice to the player.

## Language

**Advisor**:
The companion's role: it reads the game and advises, but never sends input to the game. The player always presses the keys.
_Avoid_: Bot, assistant, autopilot

**Game Source**:
A place a live game's state can be read from, such as a webtiles server or local webtiles.
_Avoid_: Integration, connector, backend

### Game situation

**Game Situation**:
Everything the Advisor knows when it reasons about a moment in a game: Visible State, Accumulated State and Game Knowledge together.
_Avoid_: Context, game state (when meaning the whole)

**Visible State**:
What the game currently reports to the player: map in view, monsters, stats, inventory, statuses, messages.
_Avoid_: Screen, snapshot

**Accumulated State**:
What the Advisor has remembered across turns of one game that the game no longer shows, such as explored levels, items seen and uniques encountered.
_Avoid_: History, memory, cache

**Game Knowledge**:
Static facts about DCSS itself for a specific game version: monster, spell, item, god and species data.
_Avoid_: Wiki data, database

### Advice

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

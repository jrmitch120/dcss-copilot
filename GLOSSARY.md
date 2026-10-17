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

**Scenario**:
A short Recording of a wizard-mode game set up by hand to reach one moment, kept beside the tests with a manifest naming its Crawl Version, the keystrokes that set it up (so it can be recorded again), the turns to snapshot, and the advice the Advisor should give.
_Avoid_: Fixture, test case

**Corpus**:
The games tests and evals replay: Scenarios and short Recordings in the main repo, and full games played by people, kept as release assets of a separate corpus repo and pinned by hash. Every one is tagged with its Crawl Version.
_Avoid_: Dataset, test data

**LLM Sidecar**:
The LLM calls made while a Recording was made, saved beside it and looked up by each request's hash, so a replay gets the same answers without calling a model.
_Avoid_: Cassette, fixture, mock

**Envelope**:
One raw message from a Game Source, unchanged, wrapped with where and when it was received. The browser extension relays envelopes to the app, and a Recording is a sequence of them.
_Avoid_: Packet, event, frame (a frame is one websocket message of the relay protocol)

**Relay protocol version**:
The one integer the browser extension and the app agree on to understand each other. The app accepts its own version and the previous one, so the two can update separately.
_Avoid_: API version, app version

**Tournament Game**:
A game that counts for the official DCSS tournament: played on the tournament's own Crawl Version, on an official server, within the tournament's dates. Games on other versions during those dates are not Tournament Games. The Advisor gives only Information in a Tournament Game.
_Avoid_: Tourney game, competitive game

### Game situation

**Game Situation**:
Everything the Advisor knows when it reasons about a moment in a game: Visible State, Accumulated State and Game Knowledge together.
_Avoid_: Context, game state (when meaning the whole)

**Visible State**:
What the game currently reports to the player: map in view, monsters, stats, inventory, statuses, messages, and any screen the player has open (overview, skills, spells, character dump).
_Avoid_: Screen, snapshot

**Screen Read**:
What a screen the player opened shows, such as the character overview, skills, spells, mutations, the dungeon overview, the character dump or any other menu, as crawl's own plain text. A Game Source reports one each time a screen opens or what it shows changes, and the Advisor reads facts off it.
_Avoid_: Screen scrape, menu dump

**Session Event**:
Something that happened to the Game Source's view of a game rather than in the game: the Advisor joined it, the source resynced it in full, the connection to it was lost, or it ended.
_Avoid_: Connection state, lifecycle event

**Accumulated State**:
What the Advisor has remembered across turns of one game that the game no longer shows, such as explored levels, items seen, uniques encountered, what it read from screens the player opened, and the Run Plan. Each fact remembers where it came from and how recent it is.
_Avoid_: History, memory, cache

**Known, Stale, Missing**:
How current a fact the Advisor remembers is. It is Known once read from the game, Stale when something the Advisor has seen since may have changed it, and Missing until it is read. A fact goes Stale only on such an event, never from turns passing, and every remembered fact keeps the turn it was read on, so it is shown as of that turn rather than as current.
_Avoid_: Fresh, expired, cached

**Game Knowledge**:
Static facts about DCSS itself for one Crawl Version: monster, spell, item, god and species data. When the Advisor has none for the exact Crawl Version being played, it uses the nearest it has and tells the player its advice is approximate.
_Avoid_: Wiki data, database

**Game Knowledge bundle**:
The Game Knowledge for one Crawl Version as one file, extracted from crawl's source at that version's tag and shipped with the app. It records the crawl commit it came from, and holds the version's own enum numbering, so a Game Source decodes a game through the bundle for its Crawl Version.
_Avoid_: Data pack, dataset

**Firewood**:
Crawl's name for a harmless monster that never moves and gives no experience, such as a fungus, a plant or a toadstool. Game Knowledge says which monsters are firewood; the panel treats them as scenery and leaves them out of what is in view, though the engine still sees them, as obstacles for instance.
_Avoid_: Scenery monster, plant (for the whole group)

**Crawl Version**:
The exact build of DCSS a game is played on: a stable release such as 0.34.1, or a particular trunk build.
_Avoid_: Game version, release (when trunk is included)

**Community Statistics**:
What finished games on public servers show for one major release, measured among games that reached the same point, such as the share of games entering a level that died there. Shown as evidence alongside advice, never as a reason on its own, because it reflects who chose to go where rather than cause.
_Avoid_: Win rates, meta, server stats

### Analysis

**Fact Sheet**:
Everything the Advisor has computed about one moment of a Game Situation, such as Threat Level, Ways Out, item comparisons, failure rates and which inputs are missing. Every number the Advisor shows or says comes from it.
_Avoid_: Situation, snapshot, context, prompt data

**Threat Level**:
How dangerous the current moment is, as one of LETHAL, DANGEROUS or CALM, judged from the chance of dying before the player's next action and the worst-case hit.
_Avoid_: Danger score, threat score

**Worst-Case Hit**:
The total damage the player could take before their next action, as bad as it gets barring one time in a hundred (the 99th percentile of the Threat Level's simulation). It counts every monster that can attack, not only the hardest single blow.
_Avoid_: Max damage, max hit

**Displayed Defences**:
The player's AC, EV and SH as the game shows them: whole points. Crawl keeps each in hundredths and rolls with the fraction the display drops, so the Threat Level guesses that fraction, any as likely as another, in each of its trials.
_Avoid_: True AC, real defences

**Not Modelled**:
What the Threat Level's simulation leaves out at a moment, listed on the Fact Sheet so its numbers are read for what they are: a monster's spells it doesn't play out, what each monster really carries rather than what its kind may be made with, statuses, the player's resistances and the like. The Advisor names what it leaves out rather than leaving it to be assumed.
_Avoid_: Unsupported, ignored, edge cases

**Formula Variant**:
One version of a crawl formula as the engine ports it, such as a monster's to-hit as crawl 0.34 computes it. A Game Knowledge bundle names the variant of each formula its Crawl Version uses, so the engine picks its code by name and never compares version strings.
_Avoid_: Formula version, ruleset

**Crawl Oracle**:
Real crawl, built at a Game Knowledge bundle's commit, used to check the engine's Formula Variants: a harness runs crawl's own code for randomised attackers and defenders, and the engine must roll the same distributions. `docs/crawl-oracle.md` covers it.
_Avoid_: Reference implementation, ground truth

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

### Panel

**Now Pill**:
The line at the top of the panel showing the Threat Level and one sentence about this moment. When the Advisor can't judge a Threat Level, because a Gap leaves an input it needs Missing, it shows UNRATED rather than CALM.
_Avoid_: Status bar, banner

**Focus Card**:
The one card under the Now Pill holding what deserves the player's attention now: a fight, a Decision Point, a Great Find, the join card, or what is in view.
_Avoid_: Main card, hero card

**Run Plan Rail**:
The Run Plan summarised along the bottom of the panel, one row per part of the plan.
_Avoid_: Plan bar, sidebar

**Appearance**:
The player's choice to draw monsters and items as DCSS tiles (the default) or as glyphs in DCSS's colours. Tiles are drawn only at whole-number scales; anything without a tile is drawn as its glyph.
_Avoid_: Theme, skin, tileset

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

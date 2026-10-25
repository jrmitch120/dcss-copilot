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

**Game Key**:
What tells one game apart from every other: the server, the account, the character's name and species, and when a Game Source first saw the game. A game process for a character whose latest game hasn't ended, at a turn no earlier than that game was last seen at, picks that game up again under its key; one for a game that ended, or whose turn went backwards, is a new game with a key of its own. A game picked up again on a later turn than it was last seen at, with the turns between unwatched, may be another game of the same character, so the panel asks the player whether it is the same game before it shows or saves anything of it. A new game then has a key of its own, first seen as its game process named it, and the game it was taken for is left as it was last saved.
_Avoid_: Game id, save, run id

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

**Settled Update**:
What the game reports once it has finished reacting to the player and waits for their next command. The Advisor's detectors judge each one once, against the one before; a player action is a Settled Update whose turn has moved on.
_Avoid_: Tick, frame, turn (when meaning this)

**Accumulated State**:
What the Advisor has remembered across turns of one game that the game no longer shows, such as explored levels, items seen, uniques encountered, what it read from screens the player opened, and the Run Plan. Each fact remembers where it came from and how recent it is. It is saved as it changes, by the game's Game Key, so the Advisor picks a game up again with it after the app restarts; a game watched rather than played is never saved.
_Avoid_: History, memory, cache

**Change Log**:
Every change to a game's Accumulated State, in the order it was made, each with the turn and time of the message that made it. It is only ever added to, so it tells what the Advisor knew of the game at any moment; it goes only with its game, when the player clears history or the game is pruned.
_Avoid_: History (when meaning this), audit log, journal

**Game History**:
Every game the Advisor remembers between runs of the app, kept on the player's computer only: each one's Game Key, its Accumulated State and its Change Log. A game not played for 90 days is forgotten unless the player turns that off, and the player can clear the history of every game but the one they are playing.
_Avoid_: Database, saves, history (alone, when meaning Accumulated State)

**Known, Stale, Missing**:
How current a fact the Advisor remembers is. It is Known once read from the game, Stale when something the Advisor has seen since may have changed it, and Missing until it is read. A fact goes Stale only on such an event, never from turns passing, or when the Advisor picks a game up again at a later turn than it last saw it on, as the turns between could have changed anything; every remembered fact keeps the turn it was read on, so it is shown as of that turn rather than as current.
_Avoid_: Fresh, expired, cached

**Carried Gear**:
What one particular monster has been seen to wield, quiver or carry, as the game describes it. It belongs to that monster only while it stays in view, except a unique's, which is its own across sightings.
_Avoid_: Equipment, inventory, loadout

**Sighting**:
One stretch of a monster in view, from when it comes into view until it leaves. Crawl gives a monster that comes back a new Sighting on purpose, so the player can't be sure it is the one they saw before; Carried Gear read in one Sighting isn't carried over to the next, except a unique's.
_Avoid_: Encounter (which is a stretch of danger), visit

**Possible Gear**:
What a monster of its kind may be made with to shoot, throw or zap, and how likely each item is, from crawl's own tables. The Advisor falls back on it for a monster whose Carried Gear it hasn't read. It holds no melee weapons: a monster whose weapon hasn't been read fights as if it wields nothing.
_Avoid_: Kind gear, starting equipment

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
Everything the Advisor has computed about one moment of a Game Situation, such as Threat Level, Ways Out, the Fight Outcome, item comparisons, failure rates and which inputs are missing. Every number the Advisor shows or says comes from it.
_Avoid_: Situation, snapshot, context, prompt data

**Threat Level**:
How dangerous the current moment is, as one of LETHAL, DANGEROUS or CALM, judged from the chance of dying before the player's next action and the worst-case hit, and how likely the best Attack Plan is to win the fight in view.
_Avoid_: Danger score, threat score

**Worst-Case Hit**:
The total damage the player could take before their next action, as bad as it gets barring one time in a hundred (the 99th percentile of the Threat Level's simulation). It counts every monster that can attack, not only the hardest single blow.
_Avoid_: Max damage, max hit

**Displayed Defences**:
The player's AC, EV and SH as the game shows them: whole points. Crawl keeps each in hundredths and rolls with the fraction the display drops, so the Threat Level guesses that fraction, any as likely as another, in each of its trials.
_Avoid_: True AC, real defences

**Resistances**:
The player's resistances to fire, cold, negative energy, poison, electricity and corrosion, as the character overview (`%`) or dump last showed them, temporary ones included. The Threat Level cuts or raises the damage it plays by them, Known or Stale, and takes the player to resist nothing until they are read.
_Avoid_: Resists, rFire etc. as a name for all of them

**Holiness**:
What the player is, as crawl's damage tells it: living, undead, demonic, nonliving or a plant, from their species, a form the status pane lights, or petrification, together with whether they worship a good god. Holy fire, holy wrath, foul flame, draining and miasma depend on it.
_Avoid_: Alignment, nature

**Lighting**:
How the light falls on the player, which changes how well monsters aim at them: Backlit, in an Umbra, or plain.
_Avoid_: Visibility, light level

**Halo**:
Light cast around a holy monster, some gear, or the player, such as a seraph's or a follower of the Shining One's. A square both a Halo and an Umbra reach is lit by neither.
_Avoid_: Glow, aura

**Backlit**:
The player lit up so monsters hit them more easily: by their own glow, such as a corona, or by a Halo they stand in when they have none of their own.
_Avoid_: Glowing, haloed (of the player)

**Umbra**:
Darkness cast around a monster or the player, the opposite of a Halo. A player standing in one that no Halo lights is harder to hit for monsters without Nightvision.
_Avoid_: Shadow

**Nightvision**:
A monster's sight through an Umbra, so an Umbra around the player doesn't spoil its aim: undead, servants of Yredelemnul, and monsters with an Umbra of their own have it.
_Avoid_: Darkvision, see in the dark

**Not Modelled**:
What the Threat Level's simulation, the Fight Outcome or the judging of Ways Out leaves out at a moment, listed on the Fact Sheet so its numbers are read for what they are: a monster's spells it doesn't play out, the ways crawl moves a monster otherwise than walking straight at the player, such as a tentacle moving with its parent, Possible Gear played in place of Carried Gear it hasn't read, statuses, the player's resistances, the player's own speed and the like. The Advisor names what it leaves out rather than leaving it to be assumed. The Not Modelled ledger (`docs/not-modelled.md`) lists everything the engine leaves out, whether or not the Fact Sheet names it, as the backlog of what to model next.
_Avoid_: Unsupported, ignored, edge cases

**Out of Depth**:
A monster at least three levels shallower than the shallowest level crawl makes its kind on at random in the branch the player is in. A kind crawl doesn't make at random in that branch, such as a unique, is never Out of Depth.
_Avoid_: OOD

**Exceptional Monster**:
A hostile in view the Advisor singles out for the player to recognise: a unique, or one Out of Depth. It has a Fact Card while it stays in view, and raises an Alert as it comes into view if it alone makes the moment DANGEROUS.
_Avoid_: Notable monster, special monster

**Dangerous Status**:
A status on the player the Advisor raises an Alert for as it lands, whatever is in view: poison that will kill them, petrification, liquid fire, or confusion beside deep water or lava. While one lasts the player isn't All Clear.
_Avoid_: Bad status, debuff

**Formula Variant**:
One version of a crawl formula as the engine ports it, such as a monster's to-hit as crawl 0.34 computes it. A Game Knowledge bundle names the variant of each formula its Crawl Version uses, so the engine picks its code by name and never compares version strings.
_Avoid_: Formula version, ruleset

**Crawl Oracle**:
Real crawl, built at a Game Knowledge bundle's commit, used to check the engine's Formula Variants: a harness runs crawl's own code for randomised attackers and defenders, and the engine must roll the same distributions. `docs/crawl-oracle.md` covers it.
_Avoid_: Reference implementation, ground truth

**Attack Plan**:
A fixed way for the player to fight what is in view: open with one attack, a spell they know, a wand or missiles they throw, while its magic, charges or missiles last, then fall back on what they hold, a weapon, a launcher or their fists; or fight with what they hold throughout.
_Avoid_: Strategy, tactic, build

**Fight Outcome**:
Each Attack Plan played out many times against the hostiles in view, in a small fight of about three monsters or fewer: its chance to win, that every hostile falls before the player does, and the hit points it costs on average. The player stands their ground, and the hostiles fight as the Threat Level plays them. A best plan likely to lose makes the moment DANGEROUS.
_Avoid_: Fight odds, combat sim, win rate

**Way Out**:
An escape or emergency option, such as a consumable, ability, spell or staircase, together with whether it can be used right now and why not. A staircase whose square, or a square on the player's path to it, is out of view is judged over remembered squares, where a door may have closed or a monster may stand now, and is shown as approximate (`~`); one the player stands on never is. An ability is stopped by the statuses that stop it in particular as soon as the status pane shows them; one the ability menu greyed out for a reason the Advisor couldn't see stays stopped as of the turn the menu was read, and approximate once the ability list is Stale. A Way Out is trusted when it can be used now and its verdict isn't approximate: not judged over remembered squares, nor from a Stale menu.
_Avoid_: Escape (when meaning the whole list), panic button

**Cut Off**:
A staircase is cut off when a hostile in view can stand beside it by the time the player has walked there at their Pace and takes it, to attack them or follow them off the level: reckoned from the monster's speed at the most energy it could have, over the explored map and the unexplored squares beyond it. A monster standing on the player's path, rather than racing them, is in the way instead; and a staircase the player stands on isn't cut off, as nothing races them to it, but hostiles beside them would follow them off the level.
_Avoid_: Blocked (when meaning this)

**Pace**:
How long each of the player's moves takes, as crawl works it out from their statuses, form, species, mutations, banes, god, gear and the water they step into, at the slowest crawl may roll it: a staircase is Cut Off or not at the player's Pace.
_Avoid_: Speed (crawl's word for a monster's energy), movement speed

**Corridor**:
A square monsters can attack the player from at most two sides of, so they come at them one or two at a time. Anywhere else is open ground.
_Avoid_: Chokepoint, hallway

**Gap**:
A fact the Advisor needs about this game but doesn't have, or has only an out-of-date copy of. The Advisor says so rather than guessing, and tells the player which screen to open to fill it.
_Avoid_: Unknown, missing data

**Candidate Action**:
Something the player could legally do right now, such as attacking, fleeing to a staircase or using a Way Out, together with its facts from the Fact Sheet. The Advisor only ever recommends a Candidate Action.
_Avoid_: Option, move, choice

**Suggested Action**:
The Candidate Action the Advisor's own rules pick for a DANGEROUS or LETHAL moment, shown before (and without) any LLM judgment, with the rule that picked it. At LETHAL the rules try, in order, leaving by stairs, healing, the best escape (one that works at once before one that works after a delay) and, as a last resort, fighting; at DANGEROUS, fighting on when the best Attack Plan wins at least 90% and leaves 30% of hit points, holding a Corridor among several hostiles, falling back to stairs, and fighting carefully with the Way Out to take should it turn LETHAL. The pick holds through the Encounter until it can no longer be done, a rule higher in the order applies, an Attack Plan wins 10 points more often, a fight on falls clearly short of the odds it was picked at, or a last-resort fight could be traded for a Way Out.
_Avoid_: Default action, rule pick

**Encounter**:
A stretch of danger, from the moment a hostile comes into view, the Threat Level leaves CALM or a Dangerous Status lands until the player is All Clear. The Advisor raises each Tactical Alert at most once per Encounter for the same cause, and at most one at each player action.
_Avoid_: Fight, combat, engagement

**All Clear**:
The player has stayed out of danger for three actions in a row: the Threat Level is CALM, no hostile is in view, and no Dangerous Status is active. It ends an Encounter and releases held Strategic Advice.
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

**Assistance Level**:
How much the Advisor offers beyond Information, as the player sets it in the panel's header: Info (facts only, no LLM), Hints (adds the Suggested Action and a one-line nudge at each Decision Point) or Coach (adds LLM judgment of Tactical picks and full Strategic Advice). Every detector runs at every level; the level decides only what the panel shows.
_Avoid_: Mode, tier, verbosity

**Information**:
Factual lookups the player asks for or that are surfaced alongside play, such as monster details or a resistance breakdown. Not a recommendation.
_Avoid_: Advice (when no recommendation is made)

### Panel

**Now Pill**:
The line at the top of the panel showing the Threat Level and one sentence about this moment. When the Advisor can't judge a Threat Level, because a Gap leaves an input it needs Missing, it shows UNRATED rather than CALM.
_Avoid_: Status bar, banner

**Side Panel**:
The browser's own panel at the side of its window, where the extension shows the panel for a webtiles game. It is part of the browser window, so it moves, minimizes and comes back to the front with it. Browsers open it only when the player clicks, from the extension's toolbar button.
_Avoid_: Sidebar (except as Firefox's name for it), docked window

**Toolbar Badge**:
The panel's stand-in on the extension's toolbar button while no Side Panel is open: its tooltip shows the Now Pill's label, any Alert and its line, and its badge counts the Decision Points that wait. Only a LETHAL moment with no trusted Way Out turns the badge to a call to open the panel.
_Avoid_: Collapsed pill, notification

**Focus Card**:
The one card under the Now Pill holding what deserves the player's attention now: a fight, a Decision Point, a Great Find, the join card, or what is in view.
_Avoid_: Main card, hero card

**Fact Card**:
A card in the Focus Card singling out an Exceptional Monster while it stays in view: what makes it exceptional, and the Threat Level it makes on its own.
_Avoid_: Monster card, info card

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
What the Advisor does when the Game Budget runs out: it drops to a lower Assistance Level rather than going quiet.
_Avoid_: Throttle, cut off

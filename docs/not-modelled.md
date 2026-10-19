# The Not Modelled ledger

Everything the engine leaves out of the Threat Level and the judging of Ways Out, in one list: the backlog to read when deciding what to model next. The Fact Sheet lists what it leaves out at a moment as Not Modelled, but some of what the engine leaves out has no `NotModelled` variant, and the crawl oracle's harnesses leave out more besides (`docs/crawl-oracle.md`). Each entry here says:

- **What crawl does**, citing crawl 0.34.1's function as the code cites the formulas it ports.
- **What the engine assumes** instead.
- **Surfaced as**: the `NotModelled` variant, by the name the Fact Sheet gives it; or *accepted simplification*, where it was decided that the Fact Sheet needn't say, and why; or *harness only*, where the crawl oracle leaves it out but the Fact Sheet doesn't say so, a gap worth closing.
- **Harness**: which of the crawl oracle's harnesses leave it out, if any.
- **Impact and cost**: how much it moves the Threat Level or the Ways Out, and what modelling it would take.
- **Issue**: the open issue that covers it, if there is one.

An entry is deleted when what it describes is modelled. `cargo test -p copilot-core` checks that every `NotModelled` variant has an entry, and that every variant an entry names exists. The entries aren't ranked: the impact and cost notes inform that choice.

## The Threat Level

### Monster equipment

- **What crawl does:** a monster that wields a weapon fights with its damage, accuracy, plus, brand and reach (`attack::calc_damage`, `attack::calc_pre_roll_to_hit`, `monster::reach_range`), and its armour and shield count toward its defences. An animated object, such as a dancing weapon, fights as the weapon it is.
- **What the engine assumes:** a monster whose Carried Gear shows what it holds fights with it: the weapon `monster::weapon` gives each attack, its accuracy, damage, enchantment and the brands `brands.rs` ports, its reach, and its brand's delay, and it makes no attack only a weapon makes when it holds no melee weapon. A kind that uses no items holds none. Any other monster fights with its kind's attacks alone, as if it wields nothing, but still makes the attacks only a weapon makes: its Carried Gear isn't read, its listing (a group's sighting message) may leave out a plain weapon in its main hand, it holds something the Advisor can't name as a weapon, such as a magical staff or an artefact whose name doesn't show its kind, or it is an animated object. No monster's armour or shield is counted, which matters only to the player's attacks, and the engine doesn't play those.
- **Surfaced as:** `monsterEquipment`, while a monster in view that can fight in melee may wield a weapon the Advisor hasn't read. A weapon's enchantment or brand its name doesn't show, and a brand that isn't ported, are listed with the monster's gear as unplayed.
- **Harness:** melee leaves out animated objects; the turn harness makes its monsters with no items, and the missile harness with nothing but what they shoot.
- **Impact and cost:** large for an armed monster seen only in a group's sighting message, or out of view since, such as an orc with a good weapon; little for beasts. Reading its look text (`x`) closes it; animated objects need their item read from their name.

### Attack flavours

- **What crawl does:** a hit carries its attack's flavour, such as poison, draining or a net (`melee_attack::mons_apply_attack_flavour`), and a beam may leave a cloud or a status behind (`bolt::affect_player`).
- **What the engine assumes:** attacks, spells and missiles count only the damage they deal as they land.
- **Surfaced as:** `attackFlavours`, on every rated Threat Level.
- **Harness:** melee and missile rolls skip the effects of a hit; the fire cases leave out nets, as one that hits catches the player; the spell harness doesn't diff a chaos beam's flavour.
- **Impact and cost:** within one player action it is mostly extra damage a turn later, but paralysis, a net or a confusing hit changes what the player can do next. Each flavour is its own port, and some need statuses the engine doesn't track.

### Statuses

- **What crawl does:** statuses on either side change the fight: a hasted monster gains energy faster (`monster::calc_speed`), and a confused, slowed or mighty monster, or a hasted or berserk player, fights otherwise.
- **What the engine assumes:** no statuses on either side, beyond those that backlight the player.
- **Surfaced as:** `statuses`, on every rated Threat Level.
- **Harness:** every harness makes its monsters with no statuses, and the turn harness takes them off.
- **Impact and cost:** a hasted monster acts about half as often again, which can move a rating a whole level. The game shows monster statuses; each would be read, then ported where it changes energy, to-hit or damage.

### Paths

- **What crawl does:** a monster walks round walls and other monsters (`_monster_move`, `mon_can_move_to_pos`). A blinkbolt carries its caster to the farthest square of its path it can stand on (`bolt::affect_endpoint`): past the player on open floor, as the bolt pierces, or short of them behind a wall.
- **What the engine assumes:** monsters walk straight at the player, through walls and each other, and a blinkbolt lands its caster beside the player, the most dangerous landing.
- **Surfaced as:** `paths`, on every rated Threat Level.
- **Harness:** the turn harness lays open floor around the player for each trial, and walls the square behind them for a caster of a blinkbolt, so crawl lands it beside them too.
- **Impact and cost:** errs high: a monster round a corner or behind another is counted sooner than it can arrive. The Ways Out already search the map for paths; the Threat Level would need the same search, per monster, inside the simulation.

### Awareness

- **What crawl does:** a monster may be asleep, wandering or not yet aware of the player (`handle_behaviour`), and doesn't come for them until it notices them.
- **What the engine assumes:** monsters are awake and hunting the player.
- **Surfaced as:** `awareness`, on every rated Threat Level.
- **Harness:** the turn harness makes its monsters awake and hunting the player.
- **Impact and cost:** errs high, and that is mostly the right way to err for a monster in view. The game shows sleeping and wandering monsters, so reading that is cheap; what a sleeping monster does when it wakes isn't.

### Spells not played out

- **What crawl does:** a monster picks from all the spells in its book (`_choose_spell_to_cast`): summons, hexes, self-buffs and spells that aren't a single bolt or explosion among them. A blink frog's book holds a blink (`SPELL_BLINK`), besides the hit that blinks it away.
- **What the engine assumes:** a monster never casts a spell it doesn't port, and fights instead.
- **Surfaced as:** `otherSpells`, when a monster that can attack has such a spell; its row names the spells.
- **Harness:** the spell and turn harnesses cut each monster's book down to the spells the engine ports.
- **Impact and cost:** either way: a hex can be worse than a hit, a self-buff is lost time. Each spell is its own port; damaging ones are cheap, summons and hexes need what follows them modelled too.

### Spells given as a monster is made

- **What crawl does:** some monsters, such as a player ghost or a Pandemonium lord, are given their own spells as they are made (`ghost_demon::init_player_ghost`, `ghost_demon::init_pandemonium_lord`, a book of `MST_GHOST`).
- **What the engine assumes:** which spells it has isn't known, so none are played out.
- **Surfaced as:** `spellsPerMonster`, when such a monster is in view.
- **Harness:** the spell and turn harnesses leave out kinds whose books crawl fills when they are made.
- **Impact and cost:** large when one is in view, but they are rare. The monster's describe screen lists its spells, so it would be read as Carried Gear is.

### Draconian breath

- **What crawl does:** a draconian breathes by its colour, a breath its kind's book doesn't hold (`drac_breath`).
- **What the engine assumes:** draconians don't breathe.
- **Surfaced as:** `draconianBreath`, when a draconian is in view.
- **Harness:** the spell and turn harnesses leave out draconians.
- **Impact and cost:** moderate in the Zot and Depths levels where they gather. Game Knowledge would carry each colour's breath, and the turn harness would draw draconians.

### A mutant beast's facets

- **What crawl does:** a mutant beast is made at a tier, which sets its HD and its hit's damage, and with two facets, which may add a reaching sting, a trample and armour, a constricting crush, double speed and flight, fire breath or a blinkbolt (`init_mutant_beast`, `_mutant_beast_facet_attack`).
- **What the engine assumes:** it is its kind's Game Knowledge entry: HD 15, one hit for 27, no facets.
- **Surfaced as:** `mutantBeastFacets`, when a hostile mutant beast is in view.
- **Harness:** the turn harness leaves out mutant beasts.
- **Impact and cost:** large when one is in view, mostly erring low: its reach, attacks, damage, speed and spells may all be wrong. Its name tells its tier and facets, so they could be read.
- **Issue:** [#95], [#120]

### The player's resistances

- **What crawl does:** a beam's damage, and what a launcher's or melee weapon's elemental, draining or pain brand adds, is cut or raised by the player's resistances (`check_your_resists`, `resist_adjust_damage`), and holy fire, holy wrath, foul flame, draining and miasma by their holiness and god (`player::res_holy_energy`, `player::res_foul_flame`, `attack::drain_defender`, `player::res_miasma`).
- **What the engine assumes:** the resistances the character overview or dump last showed, Known or Stale, and the holiness the player's species, form, petrification and god give (`resists.rs`). Steam is resisted by `rFire` alone, as if the player had no resistance to steam of their own, from steam dragon scales or a mutation, which the overview doesn't show; miasma harms them unless they are nonliving or lifeless undead, as if no form, mutation or artefact resisted it. Before the overview is read, the player resists nothing and is a living mortal who worships no good god. While their species, statuses or god are unreported, or their species isn't in the Game Knowledge, they are a living mortal of no good god with the resistances read.
- **Surfaced as:** `playerResistances`, when a monster that can attack may deal damage whose adjustment the engine can't settle from what it knows: the resistances haven't been read and the damage is one they change; it is steam or miasma, which need a resistance the overview doesn't show; or it is holy fire, holy wrath, foul flame, draining or miasma and the player's holiness or god can't be told.
- **Harness:** the spell harness casts at a human without resistances, and the melee and missile harnesses attack one who worships no god or Yredelemnul.
- **Impact and cost:** steam and miasma err low for a player with steam dragon scales, a foul stench or an artefact against miasma, which are rare; unread resistances err high by half or more of an elemental caster's damage, and low for an undead or demonic player shot with holy wrath. Opening the character overview (`%`) closes most of it; the rest would need the player's mutations and body armour read.

### The dragonskin cloak

- **What crawl does:** the dragonskin cloak gives each of `rFire`, `rCold`, `rNeg`, `rPois`, `rElec` and `rCorr` a pip half the time it is checked (`player_res_fire` and the like, with `allow_random`), which the character overview leaves out.
- **What the engine assumes:** the resistances the overview shows.
- **Surfaced as:** accepted simplification: it is one unrandart, and leaving out its half pips errs high.
- **Harness:** no harness dresses the player in it.
- **Impact and cost:** errs high by up to half of an elemental hit, while the cloak is worn. Telling it from the inventory and rolling each check would close it.

### Attackers a brand spares

- **What crawl does:** a holy monster's foul flame weapon or launcher, and an undead or demonic one's holy wrath weapon or launcher, add nothing, nor does pain in the hands of a follower of Trog (`attack::apply_damage_brand`).
- **What the engine assumes:** a brand adds the same whoever attacks with it.
- **Surfaced as:** accepted simplification: crawl makes almost none of these monsters with such a weapon, and holy wrath hurts only an undead or demonic player.
- **Harness:** the melee and missile harnesses give neither a holy monster foul flame nor a follower of Trog pain.
- **Impact and cost:** errs high, and rarely. Game Knowledge would carry each kind's holiness and god.

### Caster safety

- **What crawl does:** a caster's tracer weighs the harm an explosion or cloud does itself and its allies against the harm to the player, and often holds off up close (`mons_should_fire`).
- **What the engine assumes:** the caster casts anyway.
- **Surfaced as:** `casterSafety`, when a monster that can attack has such a spell or wand within reach.
- **Harness:** the spell, missile and turn harnesses keep such a caster three squares or more from the player.
- **Impact and cost:** errs high, and only up close. Porting the tracer means knowing what stands near the blast, which the simulation, with no paths, doesn't place.

### Carried Gear

- **What crawl does:** a monster carries the launcher, missiles and wand it was made with or picked up, with their enchantments and brands (`give_item`), and lists them as it comes into view and in its look text (`get_monster_equipment_desc`).
- **What the engine assumes:** a monster whose Carried Gear the Advisor hasn't read is judged by its kind's Possible Gear, the odds crawl makes its kind with, its launcher or missiles +0 and unbranded. That is one that came back into view, which crawl doesn't list again, or came with others whose message didn't list it or listed it among others of its name. Where a listing leaves out what a monster may still carry, an ordinary launcher in a group's main hand or unbranded large rocks, Possible Gear is played given that it is nothing the listing would have named, and an unidentified wand by its kind's wand odds given that it has one.
- **Surfaced as:** `carriedGear`, when a monster whose kind may shoot, throw or zap is in view without its Carried Gear read; and each monster's gear on the Fact Sheet, with what of its Carried Gear isn't played: an enchantment or brand its name doesn't show, a brand the engine doesn't port, or a wand neither named nor one its kind is made with.
- **Harness:** the missile harness draws a monster's launcher or missile from its gear, so never both: a monster with both, which takes its launcher two times in three (`handle_throw`), isn't checked.
- **Impact and cost:** large for a monster whose gear isn't read, with a good launcher or a wand its kind seldom has. Looking at it (`x`) reads its gear; reading picked-up gear off the pick-up and wield messages, which only make it Stale, would keep more of it Known.
- **Issue:** [#113], [#114], [#116]

### Wands not played out

- **What crawl does:** a monster zaps the wand it carries, whatever it is (`_handle_wand`).
- **What the engine assumes:** a monster never zaps a wand the engine doesn't port, such as paralysis or polymorph, and shoots or fights instead.
- **Surfaced as:** `otherWands`, when a monster that can attack may carry one.
- **Harness:** the missile harness's wand and zap cases draw only the wands the engine ports.
- **Impact and cost:** a wand of paralysis can be deadly beside other monsters. Each wand is a port of its spell, as for spells not played out.

### The worn shield

- **What crawl does:** a shield blocks a number of attacks before the player's next action by its kind (`actor::shield_block_limit`).
- **What the engine assumes:** at most one block, as without a shield, until the character overview (`%`) says which shield is worn.
- **Surfaced as:** `wornShield`, when the player has SH and the worn shield isn't known.
- **Harness:** none; the turn harness checks each shield's limit.
- **Impact and cost:** errs high, and only against several attackers. Opening the character overview fixes it.

### The player's own halo

- **What crawl does:** a halo of the player's own keeps another's from backlighting them (`player::backlit`, `player::halo_radius`).
- **What the engine assumes:** a player carrying halo gear the Advisor can't tell they wear has no halo of their own, and is backlit.
- **Surfaced as:** `ownHalo`, when the player stands in a halo and the Advisor can't tell.
- **Harness:** none; the melee harness lights the player at random.
- **Impact and cost:** errs high, by the to-hit backlighting gives. Opening the character overview fixes it.

### Fractions the display drops

- **What crawl does:** the player's AC, EV and SH carry fractions crawl rolls with, which the display drops (`player_shield_class` and the like).
- **What the engine assumes:** the Displayed Defences, with a fraction guessed afresh for each trial.
- **Surfaced as:** `defenceFractions`, when any of the displayed AC, EV and SH is above 0. The guess ([#77]) spreads over what the fraction could be, but [#92] measured that it moves ratings near Dangerous: across 560 melee situations whose chance of death is 0.25% to 1%, a fraction of 0 rather than the most it could be raises the chance of death by a median 0.27 percentage points (at most 1.29), twice the Monte Carlo's noise, and moves the worst-case hit by a median 2 (at most 6), which flips the rating in 1 situation in 5, nearly always across the worst-case hit's 40% of HP line. Low AC matters most.
- **Harness:** melee leaves out players whose defences carry a fraction, and hands the engine their defences exactly.
- **Impact and cost:** near a threshold, enough to change the rating. Working out a character's own fraction from their skills, Dexterity and gear would close it.
- **Issue:** [#121]

### Translucent Skin

- **What crawl does:** each level of the player's Translucent Skin takes 2 off a monster's to-hit in melee and with missiles, but not with bolts (`attack::post_roll_to_hit_modifiers`).
- **What the engine assumes:** no Translucent Skin.
- **Surfaced as:** harness only: the Advisor doesn't read mutations yet, and the Fact Sheet doesn't say.
- **Harness:** every harness plays a human without mutations.
- **Impact and cost:** errs high, modestly. Reading mutations comes first ([#40]); then it is a term in two to-hit formulas.
- **Issue:** [#94]

### A beam's or missile's flight

- **What crawl does:** a beam or missile flies its path (`bolt::fire`): a bolt may bounce off a wall and strike the player twice (`bolt::bounce`), and a piercing one goes on past them.
- **What the engine assumes:** each beam or missile strikes the player once, if it reaches them.
- **Surfaced as:** harness only.
- **Harness:** the spell and missile harnesses roll a beam or missile as it strikes the player, skipping its flight.
- **Impact and cost:** small in the open, more in corridors where bolts bounce. Needs the map inside the simulation, as paths do.

### Swapping in a launcher

- **What crawl does:** a monster holding its launcher in its off hand swaps it in before it fires, which costs a fifth of an attack's energy (`monster::swap_weapons`).
- **What the engine assumes:** the launcher is already in hand.
- **Surfaced as:** harness only.
- **Harness:** the missile harness wields each monster's launcher first.
- **Impact and cost:** errs high, slightly, on the first shot. Cheap to port: Carried Gear says which hand holds it, as a launcher in the off hand is listed as carried.

### Special abilities

- **What crawl does:** some monsters use special abilities of their own, outside their spells (`mon_special_ability`): a slime creature splits, a ball lightning explodes, and the seekers, the foxfire and shooting star, burst on what they reach.
- **What the engine assumes:** no special abilities.
- **Surfaced as:** `specialAbilities`, when a monster that can attack has one; its row names it.
- **Harness:** the turn harness switches them off, and leaves out the seekers, which burst by theirs.
- **Impact and cost:** varies by kind. Each is its own port.

### Swooping and flanking

- **What crawl does:** a monster whose first attack swoops or flanks moves about the player as it attacks (`_handle_swoop_or_flank`).
- **What the engine assumes:** it walks straight up and attacks.
- **Surfaced as:** `swoopOrFlank`, when such a monster is in view, whether or not it can attack, as a swoop can let it strike from farther than the simulation judges it can; its row names it.
- **Harness:** the turn harness leaves out kinds whose first attack swoops or flanks.
- **Impact and cost:** either way: a swoop can let a monster attack from farther than the simulation judges, a flank spends an action moving round the player. A port needs the free squares round the player, which the simulation, with no paths, doesn't know.

### Batty monsters

- **What crawl does:** a batty monster, such as the shard shrike, flits away after it attacks (`_handle_battiness`).
- **What the engine assumes:** it stays beside the player and attacks with every action.
- **Surfaced as:** `batty`, when a monster that can attack is batty; its row names it.
- **Harness:** the turn harness leaves out batty kinds.
- **Impact and cost:** errs high, badly for fast batty monsters.
- **Issue:** [#103]

### Keeping range

- **What crawl does:** a monster that keeps its range, such as the orb spider, holds a firing position away from the player rather than close in (`_decide_monster_firing_position`).
- **What the engine assumes:** it walks up and fights in melee.
- **Surfaced as:** `keepsRange`, when a monster that can attack keeps its range; its row names it.
- **Harness:** the turn harness leaves out kinds that keep their range.
- **Impact and cost:** errs high when its melee is worse than what it fires.
- **Issue:** [#102]

### Won't move while watched

- **What crawl does:** the wandering mushroom, deathcap and lurking horror won't step toward the player while the player watches them (`_fungal_move_check`).
- **What the engine assumes:** they walk up like anything else.
- **Surfaced as:** `wontMoveWhileWatched`, when a monster that can attack is one of them; its row names it.
- **Harness:** the turn harness leaves them out.
- **Impact and cost:** errs high. Crawl hard-codes the kinds, and Game Knowledge mirrors its list.
- **Issue:** [#101]

### Moving with a parent

- **What crawl does:** tentacles and vines, such as the snaplasher vine, move with their parent (`mons_is_tentacle_or_tentacle_segment`, which `move_solo_tentacle` and `move_child_tentacles` act on).
- **What the engine assumes:** they walk on their own.
- **Surfaced as:** `movesWithParent`, when a tentacle or segment is in view, whether or not it can attack, as one may strike from where the simulation judges it out of reach; its row names it.
- **Harness:** the turn harness leaves out tentacles and their segments.
- **Impact and cost:** either way. Needs the parent's position and the segment's length.

### Acting by code of their own

- **What crawl does:** `handle_monster_move` acts or moves for some kinds with code of its own: prisms, the hellfire mortar, the lightning spire, the diamond sawblade, the boulder, the solar ember and others, and the blazeheart golem, which `_pre_monster_move` keeps by its summoner. Projectiles, such as an orb of destruction, fly at their target and burst on it by code of their own too (`mons_is_projectile`, `iood_act`).
- **What the engine assumes:** they walk and fight like anything else.
- **Surfaced as:** `ownActions`, when a monster that can attack is one of them; its row names it.
- **Harness:** the turn harness leaves them out.
- **Impact and cost:** large for the few that explode or roll over the player, small for projectiles, mostly summoned by others or by the player. Each is its own port; crawl hard-codes the list, and Game Knowledge mirrors it.

### Bound to lava or water

- **What crawl does:** a monster that lives in lava or water, such as the lava snake, can't leave it (`monster_habitable_feat`).
- **What the engine assumes:** it walks over the floor to the player.
- **Surfaced as:** `boundToTerrain`, when a monster that can attack, and that moves, can't stand on the floor; its row names it.
- **Harness:** the turn harness leaves out kinds that can't stand on floor.
- **Impact and cost:** errs high. Needs the terrain between it and the player in the simulation.

### Moving the player

- **What crawl does:** some hits trample or drag the player, as the Serpent of Hell's, an elephant's or a golden dragon's do (`melee_attack::mons_attack_effects`, by the attack flavours `AF_TRAMPLE` and `AF_DRAG`).
- **What the engine assumes:** the player stays where they stand.
- **Surfaced as:** `movesThePlayer`, when a monster that can attack has such a hit; its row names it.
- **Harness:** the turn harness leaves such a kind out of the case it is found in, and says so on standard error.
- **Impact and cost:** either way: a trampled player is farther from the next attacker, a dragged one closer.

### Blinking away

- **What crawl does:** a monster whose hit blinks, such as a blink frog or Mnoleg, may blink away from the player one hit in three (`AF_BLINK`, `melee_attack::mons_apply_attack_flavour`).
- **What the engine assumes:** it stays beside the player and attacks again.
- **Surfaced as:** `blinksAway`, when a monster that can attack has such a hit; its row names it.
- **Harness:** the turn harness leaves out of a case a monster that ends farther from the player, and says so on standard error.
- **Impact and cost:** errs high, slightly: it spends actions coming back. Needs where it lands, which the simulation, with no map, doesn't know.

### Blinking with the player

- **What crawl does:** a monster whose hit blinks with its target, such as a phantom or a wyrmhole, may blink away and take the player along, half the time it hits (`AF_BLINK_WITH`, `melee_attack::mons_apply_attack_flavour`).
- **What the engine assumes:** both stay where they stand.
- **Surfaced as:** `blinksWithThePlayer`, when a monster that can attack has such a hit; its row names it.
- **Harness:** the turn harness leaves such a kind out of the case it is found in, as it moves the player, and says so on standard error.
- **Impact and cost:** small for the fight itself, as the monster lands beside the player; large for the Ways Out, as the player may land away from a staircase they meant to take.

### Breathing again through its own clouds

- **What crawl does:** a monster that breathes a cloud, such as a pearl dragon or a death drake, seldom breathes again through the clouds its last breath left, even once its breath is caught. Where in crawl this comes from isn't known yet.
- **What the engine assumes:** a fast one may breathe twice before the player's next action.
- **Surfaced as:** harness only.
- **Harness:** the turn harness leaves out monsters faster than the player that cast or breathe a cloud.
- **Impact and cost:** errs high, and only for fast cloud breathers. Finding the rule decides whether to port it or add a variant.
- **Issue:** [#104]

### Shapeshifters

- **What crawl does:** a shapeshifter takes another kind's shape as it is made, and changes shape as it goes (`monster::is_shapeshifter`, `mon-poly.cc`).
- **What the engine assumes:** the monster is the kind the game shows it as.
- **Surfaced as:** harness only.
- **Harness:** the turn harness leaves out shapeshifters.
- **Impact and cost:** small: it fights as the shape it has until it changes. Listing it needs the game's sign that a monster is a shapeshifter.

### Dying in its own turns

- **What crawl does:** some monsters die in their own turns (`monster_die`), as one that explodes does.
- **What the engine assumes:** it goes on acting until the player's next action.
- **Surfaced as:** harness only.
- **Harness:** the turn harness leaves such a kind out of the case it is found in, and says so on standard error.
- **Impact and cost:** small, and either way: an explosion may hurt more than the attacks it replaces. Each kind is its own port.

## Ways Out

### The player's speed

- **What crawl does:** the player moves at the speed their form, species, statuses and gear give (`player_movement_speed`).
- **What the engine assumes:** the player walks a square an action. The Threat Level assumes the same: its simulation plays out one standard-speed action, as the turn harness does, but doesn't list it.
- **Surfaced as:** `playerSpeed`, on the Ways Out whenever there are staircases.
- **Harness:** the turn harness plays the player's next action at standard speed.
- **Impact and cost:** a hasted player sees staircases cut off that they could reach first; in the Threat Level, a hasted player faces fewer attacks before acting. Needs the player's speed read from the game.
- **Issue:** [#106]

### Swimming and flying

- **What crawl does:** a monster or player that swims or flies crosses deep water and lava (`monster_habitable_feat`, `player::can_swim`).
- **What the engine assumes:** everyone walks, and deep water and lava stop them.
- **Surfaced as:** `swimmingAndFlying`, whenever the ground is judged.
- **Harness:** none.
- **Impact and cost:** a flying monster may cut off a staircase the Ways Out call clear. Needs who flies or swims, from Game Knowledge and the player's statuses.

### Unexplored squares

- **What crawl does:** monsters walk the whole level, while the player knows only what they have seen (`is_travelsafe_square`).
- **What the engine assumes:** monsters can cross and stand on every unexplored square, and the player walks only over explored ones.
- **Surfaced as:** `unexploredSquares`, whenever the ground is judged.
- **Harness:** none.
- **Impact and cost:** errs toward caution for the player. There is nothing better to assume about squares no one has seen.

### Hits on the way

- **What crawl does:** monsters act after each of the player's steps toward a staircase (`handle_monsters`), striking, shooting and casting as they go.
- **What the engine assumes:** a staircase is cut off only by a monster that can stand beside it as the player takes it.
- **Surfaced as:** `hitsOnTheWay`, on the Ways Out whenever there are staircases.
- **Harness:** none.
- **Impact and cost:** errs low: a staircase called reachable may cost a lot of hit points to reach. Needs the Threat Level's simulation played out along the path.

### Each ability's and spell's own conditions

- **What crawl does:** each ability and spell has conditions of its own, such as piety, a cooldown, an exhausted berserker or a free square to blink to (`check_ability_possible`, `can_cast_spells`).
- **What the engine assumes:** an ability or spell is usable if the player has the magic for it and no status stops it.
- **Surfaced as:** `ownConditions`, when an ability or spell is among the Ways Out.
- **Harness:** none.
- **Impact and cost:** a Way Out shown usable may not be. The ability menu greys out some of these ([#108]); the rest are each their own port.

[#40]: https://github.com/jrmitch120/dcss-copilot/issues/40
[#77]: https://github.com/jrmitch120/dcss-copilot/issues/77
[#113]: https://github.com/jrmitch120/dcss-copilot/issues/113
[#114]: https://github.com/jrmitch120/dcss-copilot/issues/114
[#116]: https://github.com/jrmitch120/dcss-copilot/issues/116
[#92]: https://github.com/jrmitch120/dcss-copilot/issues/92
[#94]: https://github.com/jrmitch120/dcss-copilot/issues/94
[#95]: https://github.com/jrmitch120/dcss-copilot/issues/95
[#101]: https://github.com/jrmitch120/dcss-copilot/issues/101
[#102]: https://github.com/jrmitch120/dcss-copilot/issues/102
[#103]: https://github.com/jrmitch120/dcss-copilot/issues/103
[#104]: https://github.com/jrmitch120/dcss-copilot/issues/104
[#106]: https://github.com/jrmitch120/dcss-copilot/issues/106
[#108]: https://github.com/jrmitch120/dcss-copilot/issues/108
[#120]: https://github.com/jrmitch120/dcss-copilot/issues/120
[#121]: https://github.com/jrmitch120/dcss-copilot/issues/121

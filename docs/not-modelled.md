# The Not Modelled ledger

Everything the engine leaves out of the Threat Level, the Fight Outcome and the judging of Ways Out, in one list: the backlog to read when deciding what to model next. The Fact Sheet lists what it leaves out at a moment as Not Modelled, but some of what the engine leaves out has no `NotModelled` variant, and the crawl oracle's harnesses leave out more besides (`docs/crawl-oracle.md`). Each entry here says:

- **What crawl does**, citing crawl 0.34.1's function as the code cites the formulas it ports.
- **What the engine assumes** instead.
- **Surfaced as**: the `NotModelled` variant, by the name the Fact Sheet gives it; or *accepted simplification*, where it was decided that the Fact Sheet needn't say, and why; or *harness only*, where the crawl oracle leaves it out but the Fact Sheet doesn't say so, a gap worth closing.
- **Harness**: which of the crawl oracle's harnesses leave it out, if any.
- **Impact and cost**: how much it moves the Threat Level, the Fight Outcome or the Ways Out, and what modelling it would take.
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

- **What crawl does:** a monster walks round walls and other monsters (`_monster_move`, `mon_can_move_to_pos`). A blinkbolt carries its caster to the farthest square of its path it can stand on (`bolt::affect_endpoint`): past the player on open floor, as the bolt pierces, or short of them behind a wall. A monster that keeps its range, such as the orb spider, steps to a square around it that it can stand on and see the player from (`_set_firing_pos`); hemmed in, with none, it may step toward a farther one, and into the player to fight them. A batty monster, such as the shard shrike, flitting after it attacks or casts, steps toward a square nearby it picks at random, only one it can stand on (`set_random_target`), and round what stands in its way: hemmed in beside the player, it more often steps into them, attacking them again. Two squares of an opaque cloud between a monster and the player hide them from it (`opacity_default`, `_losight_quadrant`), along whichever line its sight takes, and a cloud spell's beam leaves its cloud along the path the beam takes (`bolt::affect_place_clouds`) and round where it ends, spreading at random (`big_cloud`): a pearl dragon that breathes blessed fire at the player seldom sees them again till it is two squares away, even from off a straight line to them, and the clouds stay a few of the player's actions.
- **What the engine assumes:** monsters walk straight at the player, through walls and each other, and a blinkbolt lands its caster beside the player, the most dangerous landing. A monster that keeps its range always has a square to step to, so it never steps into the player to fight them. A batty monster flits as on open floor, where every square nearby is one it may pick. A monster's own opaque clouds hide the player from it only along a row, a column or a diagonal through the player, where the beam's path and its sight run through the same squares: one that casts from anywhere else leaves none the engine follows, one that stands anywhere else sees the player, the cloud round where a beam ends doesn't hide the player, and its clouds are gone at the player's next action.
- **Surfaced as:** `paths`, on every rated Threat Level.
- **Harness:** the turn harness lays open floor around the player for each trial, and walls the square behind them for a caster of a blinkbolt, so crawl lands it beside them too. It places each monster in a row with the player, where it walks straight at them, so its opaque clouds lie on that row.
- **Impact and cost:** errs high: a monster round a corner or behind another is counted sooner than it can arrive, and one that cast an opaque cloud from off a straight line to the player casts again sooner than crawl lets it. It errs low only for a monster hemmed in beside the player that keeps its range, which seldom happens, or that is batty, which in a corridor often flits back into the player. The Ways Out already search the map for paths; the Threat Level would need the same search, per monster, inside the simulation.

### Awareness

- **What crawl does:** a monster may be asleep, wandering or not yet aware of the player (`handle_behaviour`), and doesn't come for them until it notices them.
- **What the engine assumes:** monsters are awake and hunting the player, and a batty one not already flitting away.
- **Surfaced as:** `awareness`, on every rated Threat Level.
- **Harness:** the turn harness makes its monsters awake and hunting the player, and has a batty one stop flitting before each play.
- **Impact and cost:** errs high, and that is mostly the right way to err for a monster in view. The game shows sleeping and wandering monsters, so reading that is cheap; what a sleeping monster does when it wakes isn't.

### Spells not played out

- **What crawl does:** a monster picks from all the spells in its book (`_choose_spell_to_cast`): summons, hexes, self-buffs and spells that aren't a single bolt or explosion among them. A blink frog's book holds a blink (`SPELL_BLINK`), besides the hit that blinks it away.
- **What the engine assumes:** a monster never casts a spell it doesn't port, and fights instead. One with nothing else to strike the player with from where it stands, such as an orb spider, which keeps its range rather than bite and whose Orb of Destruction isn't ported, doesn't attack at all.
- **Surfaced as:** `otherSpells`, when a monster in view has such a spell; the row of one that can attack names the spells.
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
- **Harness:** the spell, melee and missile harnesses give the player a species, a god and resistances from what the overview shows (`docs/crawl-oracle.md`), but no form and no steam or miasma resistance of their own.
- **Impact and cost:** steam and miasma err low for a player with steam dragon scales, a foul stench or an artefact against miasma, which are rare; unread resistances err high by half or more of an elemental caster's damage, and low for an undead or demonic player shot with holy wrath. Opening the character overview (`%`) closes most of it; the rest would need the player's mutations and body armour read.

### The dragonskin cloak

- **What crawl does:** the dragonskin cloak gives each of `rFire`, `rCold`, `rNeg`, `rPois`, `rElec` and `rCorr` a pip half the time it is checked (`player_res_fire` and the like, with `allow_random`), which the character overview leaves out.
- **What the engine assumes:** the resistances the overview shows.
- **Surfaced as:** accepted simplification: it is one unrandart, and leaving out its half pips errs high.
- **Harness:** no harness dresses the player in it.
- **Impact and cost:** errs high by up to half of an elemental hit, while the cloak is worn. Telling it from the inventory and rolling each check would close it.

### Torment resistance

- **What crawl does:** torment takes about half the player's hit points (`torment_player`), half that with the Torment Resistance mutation, a demonspawn's, and none with two levels of it or the unrandart "Eternal Torment" (`player::res_torment`). Kikubaaqudgha shields a worshipper from it now and then, more often the more pious they are.
- **What the engine assumes:** none of these: torment, as a lurking horror sets it off, is cut only by `rNeg`, a form, petrification or being a plant.
- **Surfaced as:** accepted simplification: one mutation, one unrandart and one god, and leaving them out errs high.
- **Harness:** the torment cases give the player none of them.
- **Impact and cost:** errs high, by up to the whole of a lurking horror's torment. Reading mutations ([#40]), the inventory and piety would close it.

### Attackers a brand spares

- **What crawl does:** a holy monster's foul flame weapon or launcher, and an undead or demonic one's holy wrath weapon or launcher, add nothing, nor does pain in the hands of a follower of Trog (`attack::apply_damage_brand`).
- **What the engine assumes:** a brand adds the same whoever attacks with it.
- **Surfaced as:** accepted simplification: crawl makes almost none of these monsters with such a weapon, and holy wrath hurts only an undead or demonic player.
- **Harness:** the melee and missile harnesses give no holy monster foul flame, no undead or demonic monster holy wrath and no follower of Trog pain.
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

### The player's sight

- **What crawl does:** the player sees `you.current_vision` squares: 7, one more with the Daystalker mutation, and fewer with the Nightstalker mutation, a scarf of shadows or a meteoran's bezotting (`update_vision_range`). A monster's missile flies that far, and a lurking horror watched by the player steps toward them only when they are no farther than `random2(you.current_vision + 1)` (`_fungal_move_check`).
- **What the engine assumes:** 7.
- **Surfaced as:** accepted simplification: only a Daystalker sees farther, by one square, and narrower sight errs high.
- **Harness:** every harness plays a player with crawl's default sight.
- **Impact and cost:** small: for a Daystalker, a missile flies one square farther and a lurking horror steps a little more often. Reading mutations ([#40]) and the scarf would close it.

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

### Shapeshifters

- **What crawl does:** a shapeshifter takes another kind's shape as it is made, and changes shape as it goes (`monster::is_shapeshifter`, `mon-poly.cc`).
- **What the engine assumes:** the monster is the kind the game shows it as.
- **Surfaced as:** harness only.
- **Harness:** the turn harness leaves out shapeshifters.
- **Impact and cost:** small: it fights as the shape it has until it changes. Listing it needs the game's sign that a monster is a shapeshifter.

### Dying in its own turns

- **What crawl does:** some monsters die in their own turns (`monster_die`), as one that explodes does.
- **What the engine assumes:** it goes on acting until the player's next action, but for the lurking horror, which blows itself up into torment as it attacks.
- **Surfaced as:** harness only.
- **Harness:** the turn harness leaves such a kind out of the case it is found in, and says so on standard error, but for the lurking horror, which it places again.
- **Impact and cost:** small, and either way: an explosion may hurt more than the attacks it replaces. Each kind is its own port.

## The Fight Outcome

The hostiles fight as the Threat Level plays them, over as many of the player's actions as the fight takes, so what the Threat Level leaves out (above) the Fight Outcome leaves out too. These are what it leaves out besides.

### Summons

- **What crawl does:** a monster casts the summoning spells in its book (`_choose_spell_to_cast`), and the player casts theirs, each bringing allies into the fight.
- **What the engine assumes:** a monster never summons, and fights instead; the player's summoning spells make no Attack Plan.
- **Surfaced as:** `summons`, on every judged fight.
- **Harness:** the player harness plays one attack at a time against one monster.
- **Impact and cost:** either way, and large for a summoner such as an orc sorcerer or a player who summons. Summons need the summoned monsters played as monsters, and the player's allies as fighters.

### Allies

- **What crawl does:** allies in view, the player's and others', fight beside them or against them (`handle_monsters`).
- **What the engine assumes:** allies take no part.
- **Surfaced as:** `allies`, on every judged fight.
- **Harness:** none.
- **Impact and cost:** errs low for a player with allies, as no one else fights the hostiles. Needs allies played as monsters with foes of their own.

### Hexes

- **What crawl does:** a monster casts the hexes in its book, such as paralysis or confusion, and the player casts theirs, such as Slow or Confuse.
- **What the engine assumes:** neither side casts a hex: a monster fights instead, and the player's hexes make no plan.
- **Surfaced as:** `hexes`, on every judged fight.
- **Harness:** none.
- **Impact and cost:** either way, and large: a monster's paralysis can lose a fight the numbers call won, and a player's Slow can win one they call lost. Each hex is its own port, with the willpower roll and the status it leaves.

### Self-buffs

- **What crawl does:** either side may haste, might or otherwise buff itself: a monster with its spells, the player with spells, potions, scrolls and abilities.
- **What the engine assumes:** nobody buffs themselves.
- **Surfaced as:** `selfBuffs`, on every judged fight.
- **Harness:** none.
- **Impact and cost:** either way. A potion of haste or berserk rage is often what wins a hard fight. Each buff is a status to model in both sides' formulas.

### Mixing wands

- **What crawl does:** the player chooses each action afresh: a spell, a wand, a missile or a swing.
- **What the engine assumes:** an Attack Plan opens with one spell, wand or kind of missile until it runs out, then falls back on what the player holds.
- **Surfaced as:** `mixingWands`, on every judged fight.
- **Harness:** none.
- **Impact and cost:** errs low: a player who zaps one wand, then another, does better than either plan. Mixed plans multiply the plans played out, each 2,000 fights.

### Moving between attacks

- **What crawl does:** the player moves where they like: back into a corridor, away from a slow monster while shooting it, or up to a monster that won't come.
- **What the engine assumes:** the player stands their ground, waits for a monster that walks up and steps toward one that never will.
- **Surfaced as:** `movingBetweenAttacks`, on every judged fight.
- **Harness:** none.
- **Impact and cost:** errs low against slow monsters a ranged player could kite, and in the open where a corridor would hold monsters to one at a time. Needs the map in the simulation, as the Threat Level's paths do.

### Monster wounds

- **What crawl does:** a monster keeps the wounds it has taken.
- **What the engine assumes:** each hostile starts the fight unhurt, with hit points rolled as crawl makes a monster of its kind (`hit_points`).
- **Surfaced as:** `monsterWounds`, on every judged fight.
- **Harness:** the player harness's hit points cases roll a monster's hit points as it is made.
- **Impact and cost:** errs low in the middle of a fight. Webtiles doesn't send a monster's health on the map; its look text (`x`) describes it in words, which could narrow its hit points.

### The player's attack extras

- **What crawl does:** a weapon's other brands add damage or effects on a hit (`attack::apply_damage_brand`); auxiliary attacks such as kicks and headbutts follow a melee attack (`melee_attack::player_do_aux_attacks`); an axe cleaves; a sleeping monster is stabbed; rings and artefacts give slaying; and a shield or heavy armour slows the player's attacks (`player::attack_delay_with`).
- **What the engine assumes:** none of these: a weapon's damage and delay with the heavy and speed brands, unarmed attacks with no claws or form.
- **Surfaced as:** `ownAttackExtras`, on every judged fight.
- **Harness:** the player harness gives its player none of them, and weapons a brand of none, heavy or speed.
- **Impact and cost:** mostly errs low, as most of them add damage; a shield's or armour's delay errs high. Each is a port; slaying and the worn armour need the character overview read for more than it is now.

### The player's weapon

- **What crawl does:** the player attacks with what they hold, a weapon, a magical staff or anything else, at its enchantment and with its brand.
- **What the engine assumes:** something it doesn't know as a melee weapon or launcher, such as a magical staff or an artefact whose name doesn't show its kind, is attacked with as nothing at all, so no plan falls back on it; a weapon whose enchantment or brand its name doesn't show, or whose brand isn't ported, is +0 and plain.
- **Surfaced as:** `ownWeapon`, when the player holds such an item.
- **Harness:** the player harness wields only weapons with a brand it ports.
- **Impact and cost:** errs low. Identifying the item closes most of it; a magical staff is a weapon of its own to port.

### Fractions the player's numbers drop

- **What crawl does:** crawl rolls with each skill to finer fractions of a level than the tenths the skills screen shows (`player::skill`), and outside wizard mode the spell list shows a spell's power only as a percentage of its cap (`spell_power_percent`).
- **What the engine assumes:** each fight draws where a skill lies within its tenth, and the power among those that show as the same percentage, any as likely as another.
- **Surfaced as:** `ownFractions`, on every judged fight.
- **Harness:** the player harness hands the engine each skill exactly, as `you.skill(sk, 179200)`, and a beam's power.
- **Impact and cost:** small: a tenth of a skill level is a few points of to-hit or damage at most, and a percent of a cap a power point or two.

### Miscasts

- **What crawl does:** a spell the player fails to cast may miscast, with effects by its schools, such as damage to the player (`miscast_effect`).
- **What the engine assumes:** a failed cast spends its magic and its time, and does nothing else.
- **Surfaced as:** `miscasts`, when a plan casts a spell.
- **Harness:** none.
- **Impact and cost:** errs low, for spells with a high failure rate. A port of `miscast_effect` for each school's damage.

### Blasts and bolts

- **What crawl does:** an explosion strikes everything in its radius, and a beam or missile that passes through what it hits, such as a javelin, strikes everything on its path (`bolt::fire`).
- **What the engine assumes:** each strikes only its target.
- **Surfaced as:** `blastsAndBolts`, when a plan fires an explosion or a beam or missile that passes through.
- **Harness:** the player harness fires each beam at one monster.
- **Impact and cost:** errs low against monsters bunched together. Needs where each monster stands relative to the others, which the simulation doesn't place.

### Monster lighting

- **What crawl does:** a monster in a halo is easier for the player to hit, and one in an umbra harder, unless the player sees in the dark (`attack::post_roll_to_hit_modifiers`, `bolt::apply_lighting`).
- **What the engine assumes:** the player aims as if no halo or umbra reached the monster.
- **Surfaced as:** `monsterLighting`, when a hostile stands in a halo or umbra on the map.
- **Harness:** the player harness leaves out monsters in a halo or umbra.
- **Impact and cost:** a few points of to-hit, twice that for beams. The map already shows the light on each square; what remains is whether a halo is the monster's own, which doesn't light it up.

### Monster armour

- **What crawl does:** a monster that uses items wears armour and carries a shield, which add to its AC and EV and block the player's attacks (`monster::armour_class`, `monster::evasion`, `attack::attack_shield_blocked`).
- **What the engine assumes:** only its kind's own AC and EV count, and it blocks nothing.
- **Surfaced as:** `monsterArmour`, when a hostile is of a kind that uses items.
- **Harness:** the player harness's monsters carry nothing.
- **Impact and cost:** errs high for the player against well-armoured orcs and the like. Its Carried Gear lists what it wears; Game Knowledge would carry armour's AC and EV.

## Ways Out

### The player's speed

- **What crawl does:** an action takes the player `player_speed()` and a step onto a square `div_rand_round(player_speed() * player_movement_speed(), BASELINE_DELAY)`, the delay that of the square stepped onto (`move_player_action`): their statuses, form, species and acquired mutations, banes, god, gear and the water underfoot change them.
- **What the engine assumes:** the stair race plays the player's Pace, ported from those functions and proved by the crawl oracle's speed harness, at the slowest crawl may roll it, and reads it from what the Advisor sees: the status lights, a form by its light, the species' mutations by experience level and the rest off the mutations screen (`A`), a Stale reading as it was read, the god and stars of piety, the worn armour by name and the water on each square of the path. It can't see, and plays as absent: Cheibriados' slowing of a player under his penance who worships another god; water walking that lasts only until solid ground (`TEMP_WATERWALK_KEY`); an ego on armour the game hasn't named; and whether liquefied ground lies beyond the player's own square, taken to cover the whole path while the status pane shows slowed movement. The Threat Level plays one standard-speed action, as the turn harness does, but doesn't list it; the Fight Outcome plays the player's casts, zaps and waits at standard speed.
- **Surfaced as:** `playerSpeed`, on the Ways Out when there are staircases and the mutations screen hasn't been read, the status pane hasn't been reported or shows a status or form Game Knowledge doesn't classify, the species isn't known, or the player carries armour that changes their speed and the Advisor doesn't know whether they wear it; and on every judged fight.
- **Harness:** the speed harness checks the Pace against crawl's `player_speed` and `player_movement_speed`; the turn harness plays the player's next action at standard speed.
- **Impact and cost:** in the Threat Level, a hasted player faces fewer attacks before acting; in a fight, a fast player gets more free shots and a slow one fewer. Both could play the Pace the stair race does.

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

### Monsters out of view

- **What crawl does:** it forgets a moving monster once it leaves view (`show_update_at`, `clear_map`), but keeps detected monsters and stationary ones on its map.
- **What the engine assumes:** only monsters in view stand in the way of a staircase or race the player to it; the adapter drops detected ones too.
- **Surfaced as:** `monstersOutOfView`, on the Ways Out whenever there are staircases.
- **Harness:** none.
- **Impact and cost:** errs low: a hostile just out of view may cut off a staircase called clear, or stand in the way to it. Reading detected monsters from the map would close part of it.

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
[#108]: https://github.com/jrmitch120/dcss-copilot/issues/108
[#120]: https://github.com/jrmitch120/dcss-copilot/issues/120
[#121]: https://github.com/jrmitch120/dcss-copilot/issues/121

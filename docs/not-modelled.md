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

- **What crawl does:** a monster that wields a weapon fights with its damage, plus and brand (`attack::calc_damage`, `attack::adjusted_weapon_damage`), and its armour and shield count toward its defences. An animated object, such as a dancing weapon, fights as the weapon it is.
- **What the engine assumes:** monsters fight with their kind's attacks alone, as if they wield nothing and wear nothing.
- **Surfaced as:** `monsterEquipment`, on every rated Threat Level.
- **Harness:** melee leaves out monsters that wield items and animated objects; the missile and turn harnesses make their monsters with no items but what they shoot.
- **Impact and cost:** large for armed monsters, such as orcs with a good weapon; little for beasts. Reading a monster's weapon comes with its Carried Gear ([#83]); then the weapon formulas and brands are ported for monster melee.
- **Issue:** [#98]

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

- **What crawl does:** a monster walks round walls and other monsters (`_monster_move`, `mon_can_move_to_pos`).
- **What the engine assumes:** monsters walk straight at the player, through walls and each other.
- **Surfaced as:** `paths`, on every rated Threat Level.
- **Harness:** the turn harness lays open floor around the player for each trial.
- **Impact and cost:** errs high: a monster round a corner or behind another is counted sooner than it can arrive. The Ways Out already search the map for paths; the Threat Level would need the same search, per monster, inside the simulation.

### Awareness

- **What crawl does:** a monster may be asleep, wandering or not yet aware of the player (`handle_behaviour`), and doesn't come for them until it notices them.
- **What the engine assumes:** monsters are awake and hunting the player.
- **Surfaced as:** `awareness`, on every rated Threat Level.
- **Harness:** the turn harness makes its monsters awake and hunting the player.
- **Impact and cost:** errs high, and that is mostly the right way to err for a monster in view. The game shows sleeping and wandering monsters, so reading that is cheap; what a sleeping monster does when it wakes isn't.

### Spells not played out

- **What crawl does:** a monster picks from all the spells in its book (`_choose_spell_to_cast`): summons, hexes, self-buffs and spells that aren't a single bolt or explosion among them.
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

### The player's resistances

- **What crawl does:** a beam's damage is cut by the player's resistances (`check_your_resists`).
- **What the engine assumes:** the player resists nothing.
- **Surfaced as:** `playerResistances`, when a monster that can attack casts or zaps.
- **Harness:** the spell harness casts at a human without resistances.
- **Impact and cost:** errs high, often by half or more of an elemental caster's damage. The Advisor already remembers the resistances from the character overview; the engine needs to apply them.
- **Issue:** [#99]

### Caster safety

- **What crawl does:** a caster's tracer weighs the harm an explosion or cloud does itself and its allies against the harm to the player, and often holds off up close (`mons_should_fire`).
- **What the engine assumes:** the caster casts anyway.
- **Surfaced as:** `casterSafety`, when a monster that can attack has such a spell or wand within reach.
- **Harness:** the spell, missile and turn harnesses keep such a caster three squares or more from the player.
- **Impact and cost:** errs high, and only up close. Porting the tracer means knowing what stands near the blast, which the simulation, with no paths, doesn't place.

### Carried Gear

- **What crawl does:** a monster carries the launcher, missiles and wand it was made with or picked up, with their enchantments and brands (`give_item`).
- **What the engine assumes:** a monster is judged by its kind's Possible Gear, the odds crawl makes its kind with, its launcher or missiles +0 and unbranded.
- **Surfaced as:** `carriedGear`, when a monster whose kind may shoot, throw or zap is in view.
- **Harness:** the missile harness makes its launchers and missiles +0 and unbranded.
- **Impact and cost:** large for one monster with a good launcher or a wand its kind seldom has. Read off the sighting message and the describe screen.
- **Issue:** [#83]

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
- **Surfaced as:** accepted simplification: the guess spreads over what the fraction could be, so the chance it gives is an average rather than a blind spot ([#77]).
- **Harness:** melee leaves out players whose defences carry a fraction, and hands the engine their defences exactly.
- **Impact and cost:** small, except perhaps near a threshold; [#92] measures whether it changes ratings near Dangerous.
- **Issue:** [#92]

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
- **Impact and cost:** errs high, slightly, on the first shot. Cheap to port once Carried Gear says which hand holds what ([#83]).

### A blinkbolt carries its caster

- **What crawl does:** a blinkbolt carries its caster to the farthest square on its path it can stand on, beside the player or past them (`bolt::affect_endpoint`).
- **What the engine assumes:** the caster stays where it cast from, and walks the rest of the way.
- **Surfaced as:** harness only.
- **Harness:** the turn harness leaves out casters of a blinkbolt, the raiju and the spark wasp.
- **Impact and cost:** errs low: a spark wasp seven squares off can blinkbolt in and sting in the same action. Small to port: #85 lands the caster beside the player, the most dangerous landing and the one crawl gave in every oracle run, and makes this entry an accepted simplification with no `NotModelled` variant, since it errs high.
- **Issue:** [#85]

### Special abilities

- **What crawl does:** many monsters use special abilities of their own, outside their spells (`mon_special_ability`).
- **What the engine assumes:** no special abilities.
- **Surfaced as:** harness only.
- **Harness:** the turn harness switches them off.
- **Impact and cost:** varies by kind. Each is its own port; listing them as Not Modelled first needs Game Knowledge to say which kinds have one.
- **Issue:** [#86]

### Swooping and flanking

- **What crawl does:** a monster whose first attack swoops or flanks moves about the player as it attacks (`_handle_swoop_or_flank`).
- **What the engine assumes:** it walks straight up and attacks.
- **Surfaced as:** harness only.
- **Harness:** the turn harness leaves out kinds whose first attack swoops or flanks.
- **Impact and cost:** a swoop can let a monster attack from farther than the simulation judges it can. Game Knowledge has the attack flavours already.
- **Issue:** [#86]

### Batty monsters

- **What crawl does:** a batty monster, such as the shard shrike, flits away after it attacks (`_handle_battiness`).
- **What the engine assumes:** it stays beside the player and attacks with every action.
- **Surfaced as:** harness only.
- **Harness:** the turn harness leaves out batty kinds.
- **Impact and cost:** errs high, badly for fast batty monsters. Needs the batty flag in Game Knowledge.
- **Issue:** [#86], [#103]

### Keeping range

- **What crawl does:** a monster that keeps its range, such as the orb spider, holds a firing position away from the player rather than close in (`_set_firing_pos`).
- **What the engine assumes:** it walks up and fights in melee.
- **Surfaced as:** harness only.
- **Harness:** the turn harness leaves out kinds that keep their range.
- **Impact and cost:** errs high when its melee is worse than what it fires. Needs the flag in Game Knowledge.
- **Issue:** [#86], [#102]

### Won't move while watched

- **What crawl does:** the wandering mushroom, deathcap and lurking horror won't step toward the player while the player watches them (`_fungal_move_check`).
- **What the engine assumes:** they walk up like anything else.
- **Surfaced as:** harness only.
- **Harness:** the turn harness leaves them out.
- **Impact and cost:** errs high. Crawl hard-codes the kinds, so Game Knowledge would mirror its list.
- **Issue:** [#86], [#101]

### Moving with a parent

- **What crawl does:** tentacles and vines, such as the snaplasher vine, move with their parent (`move_solo_tentacle`).
- **What the engine assumes:** they walk on their own.
- **Surfaced as:** harness only.
- **Harness:** the turn harness leaves out tentacles and their segments.
- **Impact and cost:** either way, as a segment may strike from where the simulation judges it out of reach. Needs the parent's position and the segment's length.
- **Issue:** [#86]

### Acting by code of their own

- **What crawl does:** `handle_monster_move` acts or moves for some kinds with code of their own: prisms, the hellfire mortar, the lightning spire, the diamond sawblade, the boulder, the solar ember and others.
- **What the engine assumes:** they walk and fight like anything else.
- **Surfaced as:** harness only.
- **Harness:** the turn harness leaves them out.
- **Impact and cost:** large for the few that explode or roll over the player. Each is its own port; crawl hard-codes the list.
- **Issue:** [#86]

### Projectiles and seekers

- **What crawl does:** some monsters are projectiles, such as an orb of destruction, or seekers, the foxfire and shooting star, that fly at their target and burst on it (`mons_is_projectile`, `mons_is_seeker`).
- **What the engine assumes:** they walk up and fight like anything else.
- **Surfaced as:** harness only.
- **Harness:** the turn harness leaves them out.
- **Impact and cost:** small: most are summoned by others or by the player. Each is its own port.
- **Issue:** [#86]

### Bound to lava or water

- **What crawl does:** a monster that lives in lava or water, such as the lava snake, can't leave it (`monster_habitable_feat`).
- **What the engine assumes:** it walks over the floor to the player.
- **Surfaced as:** harness only.
- **Harness:** the turn harness leaves out kinds that can't stand on floor.
- **Impact and cost:** errs high. Needs each kind's habitat in Game Knowledge and the terrain between it and the player.
- **Issue:** [#86]

### Moving the player

- **What crawl does:** some hits trample, knock back or pull the player, as the Serpent of Hell's, an elephant's or a golden dragon's do (`melee_attack::mons_attack_effects`, `melee_attack::do_knockback`).
- **What the engine assumes:** the player stays where they stand.
- **Surfaced as:** harness only.
- **Harness:** the turn harness leaves such a kind out of the case it is found in, and says so on standard error.
- **Impact and cost:** either way: a trampled player is farther from the next attacker, a pulled one closer. Needs which attack flavours move the player.
- **Issue:** [#86]

### Blinking away

- **What crawl does:** a monster may blink away from the player, as a blink frog does (`monster_blink`).
- **What the engine assumes:** it stays and fights.
- **Surfaced as:** harness only.
- **Harness:** the turn harness leaves out of a case a monster that ends farther from the player, and says so on standard error.
- **Impact and cost:** errs high, slightly. It may turn out to be a special ability, and be folded into that entry.
- **Issue:** [#86]

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
- **Issue:** [#86]

### A mutant beast's facets

- **What crawl does:** a mutant beast is given facets as it is made, which may add attacks its kind lacks, such as a reaching sting (`_mutant_beast_facet_attack`).
- **What the engine assumes:** it has its kind's attacks only.
- **Surfaced as:** harness only.
- **Harness:** the turn harness leaves out mutant beasts.
- **Impact and cost:** errs low: its reach, its attacks and their damage may all be wrong. Its name tells its facets, so they could be read.
- **Issue:** [#95]

### Dying in its own turns

- **What crawl does:** some monsters die in their own turns (`monster_die`), as one that explodes does.
- **What the engine assumes:** it goes on acting until the player's next action.
- **Surfaced as:** harness only.
- **Harness:** the turn harness leaves such a kind out of the case it is found in, and says so on standard error.
- **Impact and cost:** small, and either way: an explosion may hurt more than the attacks it replaces. Each kind is its own port.
- **Issue:** [#86]

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
[#83]: https://github.com/jrmitch120/dcss-copilot/issues/83
[#85]: https://github.com/jrmitch120/dcss-copilot/issues/85
[#86]: https://github.com/jrmitch120/dcss-copilot/issues/86
[#92]: https://github.com/jrmitch120/dcss-copilot/issues/92
[#94]: https://github.com/jrmitch120/dcss-copilot/issues/94
[#95]: https://github.com/jrmitch120/dcss-copilot/issues/95
[#98]: https://github.com/jrmitch120/dcss-copilot/issues/98
[#99]: https://github.com/jrmitch120/dcss-copilot/issues/99
[#101]: https://github.com/jrmitch120/dcss-copilot/issues/101
[#102]: https://github.com/jrmitch120/dcss-copilot/issues/102
[#103]: https://github.com/jrmitch120/dcss-copilot/issues/103
[#104]: https://github.com/jrmitch120/dcss-copilot/issues/104
[#106]: https://github.com/jrmitch120/dcss-copilot/issues/106
[#108]: https://github.com/jrmitch120/dcss-copilot/issues/108

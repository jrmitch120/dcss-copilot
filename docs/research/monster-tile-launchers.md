# Can a monster's tile show its wielded launcher?

Research for #97, against crawl 0.34.1 (`~/Development/crawl` at tag `0.34.1`). Claims cite crawl's functions and tables. Tile numbers come from the generated `rltiles/tiledef-player.h` of a 0.34.1 webtiles build. The webtiles payloads were captured from a local 0.34.1 server; [Captured payloads](#captured-payloads) quotes them.

**Answer: partly.** A monster's doll parts show the weapon in its main hand only. For most monsters that may carry a launcher, they name the launcher's type and nothing more. That is the case #83 leaves to Possible Gear: a plain launcher in a group member's main hand. The tile can't see a launcher held in reserve, and seeing no weapon part never means the monster has no launcher.

## How the tile reaches webtiles

- `_tile_place_monster` (`tileview.cc`) calls `mcache_manager::register_monster` for every monster that isn't stationary. If an `mcache_*` kind accepts the monster, the cell's foreground tile becomes `TILEP_MCACHE_START + index` (7578 and up at 0.34.1). Otherwise it stays the monster's plain tile.
- When the cell's `fg` changes, `TilesFramework::_send_cell` (`tileweb.cc`) calls `send_mcache`. That writes an `mcache` array of `[tile, x offset, y offset]` triples, the entry's `info()` parts in drawing order, plus `"doll":[]`. A cell with a plain player-sheet tile gets `"doll":[[tile,32]]` and `"mcache":null` instead.
- `mcache_manager::clear_nonref` runs on every view update, and a monster's entry is registered again each time. So a monster's `fg` index changes on almost every update, and its `mcache` is sent again with it. The captures show this: one orc warrior's `fg` alternates between two indices from turn to turn. `mcache` is still only sent with a changed `fg`, so a decoder has to keep it for each cell.
- `fg` comes as a number, as a negative 32-bit number when high flag bits are set, or as a `[low, high]` pair (`write_tileidx`). The tile index is the low 16 bits (`TILE_FLAG_MASK`). The captures have all three forms. `crates/webtiles/src/map.rs`'s `low_word` already handles them.

## 1. Coverage: which monsters get doll parts

`mcache_manager::register_monster` tries the kinds in this order:

| Kind | Accepts | Weapon part |
|---|---|---|
| `mcache_monster`, when the monster has `MONSTER_TILE_KEY` | `mcache_monster::valid` | as below; any other kind is skipped |
| `mcache_demon` | `MONS_PANDEMONIUM_LORD` | none: head, body and wings |
| `mcache_mbeast` | `MONS_MUTANT_BEAST` | none: facet layers |
| `mcache_ghost` | player ghosts (`mons_is_pghost`) | **made up**, see below |
| `mcache_draco` | every draconian (`mons_is_draconian`) | always, at offset (-3, -1), whenever it wields something |
| `mcache_armour` | `MONS_ARMOUR_ECHO` | none: body armour |
| `mcache_monster` | `mcache_monster::valid` | when its tile has a weapon offset |

`mcache_monster::valid` accepts a monster when its tile has a weapon offset and it has an item in `MSLOT_WEAPON`. It also accepts one whose tile has a shield offset and that has a shield, or a second weapon if it wields two. The constructor sets the weapon part to `tilep_equ_weapon(*mon.inv[MSLOT_WEAPON])`. `mcache_monster::info` draws the part only when `get_weapon_offset(tile)` lists the tile. The table is keyed by **tile**, not monster type, and a tile is listed only if its artwork doesn't already show a weapon (the function's comment says so).

### Which weapon is sent

The weapon part is always `MSLOT_WEAPON`, the item in the monster's hand right now. Crawl swaps a monster's launcher in and out of that slot:

- A monster made with a launcher also gets a melee weapon roll (`_give_weapon` gives a second weapon after a launcher). After placement, `mon-place.cc` calls `monster::wield_melee_weapon`. That puts the melee weapon in hand and the launcher in `MSLOT_ALT_WEAPON`. If there's no melee weapon, the launcher still goes to the alt slot and the hand is empty (`weap && !alt` swaps).
- `handle_throw` (`mon-act.cc`) calls `swap_weapons` before a monster shoots, so the launcher is in hand while it fires. `melee_attack::run_monster_attack_set` calls `wield_melee_weapon` before a melee attack, and `monster::go_berserk` does when a monster berserks on purpose.
- Monsters from a vault or wizard-mode spec skip `wield_melee_weapon`. `_dgn_give_mon_spec_items` (`dungeon.cc`) "pre-wields ranged weapons", so they start with the launcher in hand.

So a freshly made monster usually holds its melee weapon or nothing. A monster that has shot holds its launcher until it next melees.

### Whether it is always sent

For a monster whose tile has a weapon offset, yes: `monster_info` copies every visible inventory slot whole (`monster_info::monster_info`, the loop over `MSLOT_LAST_VISIBLE_SLOT`). The player's knowledge of the item makes no difference. A weapon's sub-type is never hidden, and the tile depends only on its sub-type, its randart or unrandart status and its cosmetic flags (`tileidx_enchant_equ`). A monster the player can't see sends no monster tile at all.

Every monster kind that Possible Gear (the bundle's `gear.missile`) says may be made with a launcher has a tile with a weapon offset, **except the taurs and Nessos**:

| Monster kinds | Tile | Weapon part |
|---|---|---|
| goblin, orc warrior, orc knight, orc warlord, deep elf archer, knight, master archer and high priest, salamander, cherub, Joseph, Saint Roka, vault sentinel, vault warden, faun, satyr, tengu warrior, demonspawn warmonger, naga sharpshooter, Vashnia, kobold blastminer, nekomata | its own `TILEP_MONS_*` | yes |
| centaur, centaur warrior, yaktaur, yaktaur captain | `TILEP_MONS_CENTAUR` etc. when `_bow_offset` finds a shortbow, orcbow, longbow or arbalest in hand; the `*_MELEE` tile otherwise (`_tileidx_monster_no_props`) | no for those four launchers: the base tile has the bow drawn on, so `mcache_monster::valid` refuses it. The `*_MELEE` tiles have a weapon offset, so a sling, hand cannon or triple crossbow, or any melee weapon, is drawn as a part |
| Nessos | `TILEP_MONS_NESSOS` with a longbow in hand, `TILEP_MONS_NESSOS_BOWLESS` otherwise | no for the longbow (drawn on); yes for anything else |
| spriggan rider and others not in `get_weapon_offset` | | never |

For the bow-class launchers, the taurs' base tile is still a signal. `TILEP_MONS_CENTAUR` (7154) means "a shortbow, orcbow, longbow or arbalest in hand" but not which one. The tile number is in the cell's `doll` (`[[7154,32]]`) and in `fg`.

Any monster that uses weapons can also pick up a launcher. A kind whose tile has no weapon offset never shows it.

## 2. Discrimination: the launcher tiles

`tilep_equ_weapon` (`tilepick-p.cc`) maps each launcher sub-type to a `TILEP_HAND1_*` tile. Then `tileidx_enchant_equ` (`tilepick.cc`) swaps in an unrandart's own doll tile, or else the `%enchant_variation` tile for the item's `enchant_to_int` (1–3 for shiny, runed or glowing, 4 for a randart), plus `item.rnd % tile_player_count`. Every launcher tile has one image, so `rnd` changes nothing. All launchers in crawl 0.34.1 (`Weapon_prop` in `item-prop.cc`):

| Launcher | Plain | Shiny, runed or glowing | Randart |
|---|---|---|---|
| sling | `TILEP_HAND1_SLING` 8262 | 8262 | 8262 |
| shortbow | `TILEP_HAND1_SHORTBOW` 8264 | 8264 | 8264 |
| orcbow | `TILEP_HAND1_ORCBOW` 8265 | `TILEP_HAND1_ORCBOW_MAGIC` 8266 | `TILEP_HAND1_ORCBOW_RANDART` 8267 |
| **longbow** | **`TILEP_HAND1_ORCBOW` 8265** | **8266** | **8267** |
| hand cannon | `TILEP_HAND1_HAND_CANNON` 8270 | `TILEP_HAND1_HAND_CANNON_MAGIC` 8271 | `TILEP_HAND1_HAND_CANNON_RANDART` 8272 |
| arbalest | `TILEP_HAND1_ARBALEST` 8273 | 8273 | 8273 |
| triple crossbow | `TILEP_HAND1_TRIPLE_CROSSBOW` 8277 | 8277 | 8277 |
| old blowgun, old fustibalus (`TAG_MAJOR_VERSION == 34` only, never made) | none: `default: tile = 0` | | |

- **The orcbow and the longbow share a tile.** `tilep_equ_weapon` maps `WPN_LONGBOW` to `TILEP_HAND1_ORCBOW`. Their damage differs (11 and 14). No other two launchers share a tile, and no melee weapon or staff maps into 8262–8277. `TILEP_HAND1_BOW` (8263) and `ARBALEST2`–`4` (8274–8276) are in the tile list, but no code picks them.
- **Brands:** `item_set_appearance` (`makeitem.cc`) makes a weapon glowing or runed when `_weapon_is_visibly_special` finds a brand, a plus above 0 or an artefact. So for the orcbow, longbow and hand cannon, the tile tells a plain launcher from a branded or enchanted one, but not which brand or plus. For the others it tells nothing. A captured `+0 orcbow of flaming` sent 8266.
- **Unrandarts** have their own doll tiles (`unrandart_to_doll_tile`, generated from `TILE_EQ` in `art-data.txt`): "Punk" (sling) 8079, "Sniper" (triple crossbow) 8080, "Damnation" (arbalest, `TILEP_HAND1_FIERY_DEVIL`) 8081, "Zephyr" (longbow) 8083, "Mule" (hand cannon) 8084, "Piercer" (longbow, `TILEP_HAND1_GREAT_BOW`) 8268, the storm bow (longbow) 8269. The blowgun of the Assassin also maps to `GREAT_BOW`, but it is `deleted` and never made.
- **Vault-set tiles:** an item spec's `wtile:` sets `WORN_TILE_KEY`, which overrides the mapping. At 0.34.1 it's used once, on a mace (`dat/des/builder/uniques.des`).

### The Game Knowledge bundle doesn't have this yet

The 0.34.1 bundle names each monster's tile by its image path (`Monster::tile`, from `tools/gk-extract/src/tiles.rs`). It has no tile numbers and nothing from the `TILEP_HAND1_*` range. To decode the weapon part, it would need:

1. **A tile number for each weapon tile it decodes**, mapped to the weapon sub-type or types it means. At minimum that's the launcher rows above and the unrandart tiles, with 8265–8267 meaning "orcbow or longbow". Crawl numbers player-sheet tiles from `TILE_MAIN_MAX` (5215 at 0.34.1). That number is the count of every image in `dc-floor.txt`, `dc-wall.txt`, `dc-feat.txt` and `dc-main.txt`, chained by `%startvalue`, and the player sheet then counts on through `dc-player.txt`. The extractor already reads `dc-player.txt` the way `rltiles/tool` does, but it doesn't count images or read the other lists. The numbers change between versions as tiles are added, like every other enum the bundle keeps.
2. **The taurs' and Nessos's bow-drawn base tiles** (7152, 7154, 7156, 7158, 7160 at 0.34.1), if the coarse "bow-class launcher in hand" signal is to be used.

The numbers in this note are what a test of that extraction should check against: they come from crawl's generated header and the captured payloads.

## 3. Interference

- **Off-hand items and shields** are a separate part, `m_shd_tile` (`tilep_equ_shield`, `TILEP_HAND2_*`), after the weapon part. They never replace it. A monster that wields two weapons (`mons_class_wields_two_weapons`) gets its second weapon drawn mirrored (`mirror_weapon`, `TILEP_HAND1_MIRROR_*`, 8301 and up) in the shield's place. Only a part in 8030–8300 (`TILEP_HAND1_FIRST`–`LAST`) is the main hand.
- **Status icons** are sent in their own `icons` field, and flags like flying, net and threat sit in `fg`'s high bits (`tileidx_monster`). Neither changes `mcache`. The captured sleeping deep elf archer sent `"icons":[10233]` alongside its weapon part.
- **Submerged:** crawl 0.34.1 has no submerging. `MB_SUBMERGED` is only kept for old saves and is never set. In water, `send_mcache` passes `in_water` to `send_doll`, which only drops a doll's shadow part. Merfolk swap to their `*_WATER` tiles, which have weapon offsets.
- **Player ghosts:** `mcache_ghost` builds a whole player doll from the ghost's species, job, AC, best skill and a hash of its name. The weapon is **guessed from its best skill**: Ranged Weapons always draws a shortbow. That's not the ghost's gear, and the ghost's `mcache` is empty anyway. Never read a ghost's tile as Carried Gear.
- **Shadows and other set tiles:** a monster with `MONSTER_TILE_KEY` (Dithmenos's shadow, `tileidx_player_shadow`; an orc raised as fallen; a vault-set tile) goes straight to `mcache_monster`, keyed by that tile. The `PLAYER_SHADOW_*` tiles have weapon offsets.
- **Illusions and clones:** `clone_mons` (`mon-clone.cc`) copies the original's items into new item slots, so a clone shows and carries the same weapon. The tile and the describe popup agree. The phantom mirror's reflection of a nekomata gets `TILEP_MONS_NEKOMATA_PHANTOM`, which has no weapon offset.
- **Dancing and spectral weapons** are drawn with their item tile or a weapon-class tile, not a doll part, and are outside this question.

## 4. Recommendation: partial, yes for the wielded weapon

Use the weapon part as a Carried Gear source for **the launcher in the monster's hand**, by type only, for monsters drawn with `mcache_monster` or `mcache_draco`. That covers every kind that Possible Gear says may carry a launcher, except the four taurs and Nessos. It fills exactly the gap #83 leaves. A group's sighting message (`_monster_headsup` with more than one monster uses `DESC_NOTEWORTHY`) leaves out an unbranded launcher in hand, and a launcher is in hand mostly after the monster has shot or when a vault placed it. It still lists one held in reserve ("carrying a shortbow", since `_is_mon_equipment_worth_listing` counts any launcher in `MSLOT_ALT_WEAPON`), and that's where `wield_melee_weapon` leaves a fresh monster's launcher.

The rules, most important first:

1. **The tile only adds; no part removes nothing.** No weapon part, or a melee weapon part, says nothing about a launcher in reserve. The captures show both cases. Never take a launcher out of Carried Gear because the tile doesn't show it, and never conclude "no launcher" from the tile.
2. **Type only.** The tile gives the launcher's sub-type. For the orcbow, longbow and hand cannon, it also says whether it's branded, enchanted or a randart. It never gives the brand or the plus. Where the sighting message or describe popup named the same launcher, keep their detail. Where only the tile did, take the brand and plus as unknown and say so, as for any other approximation.
3. **8265–8267 means orcbow or longbow.** Settle it with the kind's Possible Gear when only one of the two is possible. That holds for every kind at 0.34.1: orc warriors and kobold blastminers have orcbows, and every elf, naga, satyr, cherub, warmonger and Vashnia has longbows. Otherwise keep both and label it, because a monster may have picked up the other one. 8268 and 8269 name one longbow each (Piercer and the storm bow), and the other unrandart tiles one launcher each.
4. **When the tile and the text disagree, the latest wins.** All three read the same `monster_info` at different moments: the sighting message on the turn it comes into view, the describe popup when the player opens it, and the tile on every view update. A tile showing a different wielded weapon from an older message means the monster swapped or picked something up. So the tile updates which weapon is in hand, and the older text's reserve item stays Carried Gear. A popup opened after the tile is the fullest and newest. It wins outright, and its plus and brand replace the tile's unknowns.
5. **Taurs and Nessos:** the bow-drawn base tile (7154 and the others) means "a shortbow, orcbow, longbow or arbalest in hand". Combined with Possible Gear, that's a centaur's orcbow or shortbow, a yaktaur's arbalest and Nessos's longbow. Because one tile covers several launchers, treat it as a weaker signal, behind the text, and only where Possible Gear narrows it.
6. **Leave out** player ghosts (made-up doll), Pandemonium lords, mutant beasts and armour echoes (no weapon part).

The follow-up implementation issue is a child of #24, linked from #97.

## Captured payloads

Three wizard-mode recordings from local webtiles (0.34.1, `combo = MiBe.handaxe`, `game_seed = 1`), made with `cargo run -p corpus -- record-scenario`. The creating keys were `&` `m` and a monster spec. They are throwaway captures and aren't committed. Each cell is quoted as the `map` message sent it.

**A group of launcher-wielding monsters made by spec, so each holds its launcher.** The `fg`s are mcache indices, 7578 + entry:

```json
{"mon":{"id":1,"name":"orc warrior","type":191,…},"t":{"fg":1056155,"doll":[],"mcache":[[7258,0,0],[8273,-2,-1]]}}
{"mon":{"id":3,"name":"kobold","type":187,…},"t":{"fg":1056160,"doll":[],"mcache":[[7048,0,0],[8262,-3,0]]}}
{"mon":{"id":4,"name":"deep elf archer","type":620,…},"t":{"fg":1056163,"icons":[10233],"doll":[],"mcache":[[7172,0,0],[8265,-2,-4]]}}
{"mon":{"id":5,"name":"naga sharpshooter","type":589,…},"t":{"fg":1056164,"doll":[],"mcache":[[7074,0,0],[8277,0,-4]]}}
{"mon":{"id":6,"name":"yaktaur captain","type":246,…},"t":{"fg":1056164,"doll":[],"mcache":[[7161,0,0],[8270,4,0]]}}
{"mon":{"id":7,"name":"centaur","type":243,…},"t":{"fg":1055730,"doll":[[7154,32]],"mcache":null}}
{"mon":{"id":8,"name":"orc warrior","type":191,…},"t":{"fg":1056164,"doll":[],"mcache":[[7258,0,0],[8266,-2,-1]]}}
{"mon":{"id":9,"name":"orc warrior","type":191,…},"t":{"fg":1056173,"doll":[],"mcache":[[7258,0,0],[8264,-2,-1]]}}
```

Their sighting messages were "wielding a +0 arbalest", "+0 sling", "+0 longbow" (the deep elf archer: `8265`, the orcbow's tile), "+0 triple crossbow", "+0 hand cannon" (on the captain's `*_MELEE` tile 7161), "+0 shortbow" (the centaur: no `mcache`, its bow is drawn on tile 7154), "+0 orcbow of flaming" (`8266`) and "+0 shortbow".

**An orc warrior with `arbalest . war axe` swapping.** "It is wielding a +0 arbalest and carrying a +0 war axe" sent `[[7258,0,0],[8273,-2,-1]]`. Beside the player, "The orc warrior unwields a +0 arbalest. The orc warrior wields a +0 war axe." came with `[[7258,0,0],[8250,-2,-1]]` (`TILEP_HAND1_WAR_AXE`). After the player blinked away, "unwields a +0 war axe … wields a +0 arbalest" came with `[[7258,0,0],[8273,-2,-1]]` again, and then it shot.

**An orc warrior with only an orcbow, meleeing:** "The orc warrior unwields a +0 orcbow." Its cell became the plain tile, `{"fg":7258,"doll":[[7258,32]],"mcache":null}`. It still had the orcbow.

**Not captured:** a group coming into view together with plain launchers in hand, which would show the message leaving them out next to the tile showing them. Wizard mode makes monsters next to the player one at a time, so each gets its own one-monster message. The two attempts to get a group back into view at once failed: `&B` asks to confirm, so the setup's Escape cancelled it, and `&d` left the monsters behind. The message's side of the gap rests on `_monster_headsup` and `get_monster_equipment_desc` above.

# refdata/worldgen: reference data for world generation and 32-bit maths

Produced by `tools/refdump_worldgen/` (a .NET Framework 4 **x86** console program that loads the real `TerrariaServer.exe` from `work/refserver/` by
reflection and runs the game's own code, headless, no socket). Nothing here was written by hand and no game code was modified.

| file | what | size |
|---|---|---|
| `ref_small.passes.jsonl` | one JSON object per pass (108 rows) | 1.5 MB |
| `ref_small.columns.bin` | 108 rows x 4200 u64 per-column tile hashes | 3.6 MB |
| `ref_small.before_pass_000.tfrm.z`, `ref_small.after_pass_NNN.tfrm.z`, `ref_small.after_generate_world.tfrm.z` | full tile snapshots (15 files), zlib | 35 MB |
| `x87/*.bin`, `x87/x87_env.json` | `System.Math` reference tables, 17 functions x 20000 records | 6 MB |
| `float_probes/probes.bin`, `probes.json`, `probes_source.cs` | 34 float-expression probes x 2000 inputs | 0.5 MB |
| `determinism/` | run summaries and `report.txt` of the determinism experiments | 0.2 MB |

Total about 45 MB. Python readers/verifiers: `tools/refdump_worldgen/summarize.py` (parses every format below and cross-checks the hashes),
`wld_diff.py` (field-level diff of two `.wld`), `check_determinism.py` (produces `determinism/report.txt`), `floatprobe_model.py` (float rule test).

## 1. Parameters

Original run (`work/refserver.log`): `TerrariaServer.exe -autocreate 1 -worldname ref_small -seed 12345 -world work\refsave\Worlds\ref_small.wld`.

| | value | where it comes from |
|---|---|---|
| seed text / int seed | `"12345"` / `12345` | `-seed` -> `Main.AutogenSeedName` (LaunchInitializer.cs:203-206); `WorldFileData.SetSeed` -> `TranslateSeed`: `int.TryParse` ok -> `Math.Abs` (WorldFileData.cs:393-410). Not a "copied seed" (`TryApplyingCopiedSeed("12345")` is false: no `.`), no special-seed option (`GetOptionFromSeedText` is null), so no secret seed is on. A non-numeric text would be `Crc32.Calculate(text)`. |
| size | 4200 x 1200 (small) | `-autocreate 1` -> `Main.autoCreate` (Main.cs:5055-5078) |
| game mode | 0 (classic) | `Main.GameMode` default; the log says `Difficulty: 0` |
| evil | `WorldGenParam_Evil = -1` = random; result: **crimson** (`WorldGen.crimson == true`) | `crimson = genRand.Next(2) == 0` (WorldGen.cs:11315), then overridden only if the param is 0/1 (11317-11326). `GenVars.crimsonLeft` is drawn at 11225-11231. |
| world name | `ref_small` | `-worldname` -> `Main.worldName` (Main.cs:5313-5316) |
| world id | 1904902962 | `Main.ActiveWorldFileData.WorldId = genRand.Next(int.MaxValue)` (WorldGen.cs:11347) |
| generator version | 1400159338497 | `WorldFile.CreateMetadata` (WorldFile.cs:610) |
| passes | 106 enabled, none skipped (no dual-dungeon seed) | table in section 4 |
| result | surface 331, rock layer 427, spawn (2095,232), dungeon (3413,190), 174 chests, 2 NPCs (Guide, Old Man) | last row of `ref_small.passes.jsonl` |

## 2. How the real dedicated server gets from the arguments to a world, and how the harness reproduces it

Real server (all paths below `decompile/Terraria/Terraria/`):

1. `Program.cs:180-186,219-229`: `LaunchParameters`, `SavePath`, `new Main()`, `Lang.InitializeLegacyLocalization()`, `LaunchInitializer.LoadParameters(main)`, `main.DedServ()`.
2. `LaunchInitializer.cs:127-129` `-worldname`; `:160-163` `-autocreate 1` (sets `maxTilesX/Y = 4200/1200`, `autoGen = true`); `:173-183` `-world <path>`: the file does not exist and `autoGen`
   is on, so `new WorldFileData(path,false).SetAsActive()` and `Main.autoGenFileLocation = path`; `:203-206` `-seed`.
3. `Main.DedServ` (Main.cs:5372-5394): `rand = new UnifiedRandom()` (Environment.TickCount seed), `dedServ = true`, `Initialize()` (netMode becomes 2, Main.cs:6640-6642).
   `Initialize` already reads `maxTilesX` (`WorldGen.RandomizeCaveBackgrounds`, Main.cs:6791), so the launch parameters must be applied **before** it.
4. `Main.cs:5853` `Task task2 = WorldGen.serverLoadWorld()` -> `Task.Factory.StartNew(serverLoadWorldCallBack)` (WorldGen.cs:6903-6906,6840): **generation runs on a thread-pool thread**;
   the main thread only prints the progress bar. (`WorldGen.CreateNewWorld`, WorldGen.cs:6581, is the client UI path; the server never calls it.)
5. `WorldFile.LoadWorld` (WorldFile.cs:650): world file missing and `autoGen` -> `CreateMetadata` (597-621: `UniqueId = Guid.NewGuid()`, `CreationTime = DateTime.Now`),
   seed text handling (680-705), `WorldGen.GenerateWorld(Main.AutogenProgress)` (706), `WorldFile.SaveNewWorld()` (708, 862), then the normal load of the file just written (711+).
6. `WorldGen.GenerateWorld` (WorldGen.cs:11080-11116): `new WorldGenerator(seed, configuration, progress, controller)` (11091), `clearWorld()` (11092; no random calls),
   `Reset()` (11093, 11118-...: `Main.rand = new UnifiedRandom(seed)` at 11153, then the draws for jungle hut, crimson-left, ore tiers, hell chest items, `WorldId`, backgrounds, moon, dungeon side...),
   `AddPasses()` (11094), `DisablePassesForSpecialSeeds` (11095), `_generator.GenerateWorld()` (11096), `Finish()` (11097), and the `finally` block (11100-11114: `RestoreTemporaryStateChanges`,
   `generatingWorld = false`, all `*WorldGen` flags reset).
7. `WorldGenerator.GenerateWorld` (WorldBuilding/WorldGenerator.cs:421-466): `_controller.SetGenerator(this)` -> `OnPassesLoaded` (98-113), then a loop that runs
   `PassResults.Add(RunPass(pass)); _controller.OnPassCompleted();` for every pass.
   **`RunPass` (488-517): `Main.rand = new UnifiedRandom(_seed)` at the start of every pass (line 499)**, then `pass.Apply(progress, configuration.GetPassConfiguration(pass.Name))`,
   `progress.End()`, `Invariant.Assert(Liquid.numLiquid == 0 && LiquidBuffer.numLiquidBuffer == 0)`, and the result `{Name, DurationMs, RandNext = WorldGen.genRand.Next()}`.

Harness (`tools/refdump_worldgen/Gen.cs`, `Common.cs`; `RefDumpWorldgen.exe gen <outdir> <tag> passes task`):

| step | what | game source |
|---|---|---|
| a | `Program.SavePath = work\worldgen-scratch\<tag>\save`, `Main.dedServ = true`, `showSplash = false`, language, `new Main()`, `Lang.InitializeLegacyLocalization()` | Program.cs:187,197,219,223; Main.cs:5392-5393 (identical to `tools/refdump` `InitGame`) |
| b | the launch parameters, **before** `Initialize()`: `maxTilesX = 4200; maxTilesY = 1200; autoGen = true`; `Main.worldName = "ref_small"`; `new WorldFileData(path,false).SetAsActive(); Main.autoGenFileLocation = path`; `Main.AutogenSeedName = "12345"` | Main.cs:5062-5065,5315; LaunchInitializer.cs:181-182,205 |
| c | `Main.rand = new UnifiedRandom(0)` (real server: tick-seeded; shown irrelevant, section 6) and `main.Initialize()` | Main.cs:5375,5394 |
| d | on a **thread-pool thread** (`Task.Factory.StartNew`, like the server; `main` instead of `task` as 5th argument runs it on the main thread, identical output): the autogen block of `LoadWorld` replayed call by call: `WorldFile.CreateMetadata`, `TryApplyingCopiedSeed`, `WorldGenerationOptions.GetOptionFromSeedText/SelectOption`, `AutoGenEnabled` options, `ActiveWorldFileData.SetSeed("12345")` | WorldFile.cs:679-705 |
| e | `WorldGen.GenerateWorld(Main.AutogenProgress, controller)` with a `WorldGenerator.Controller` whose **public `OnPassesLoaded`** callback (WorldGenerator.cs:30,109-111, called at the start of `_generator.GenerateWorld()`, i.e. after `clearWorld`+`Reset`+`AddPasses`) (1) writes the row "before pass 0" and (2) replaces every `GenPass` in `controller.Passes` by an emitted subclass `HookPass : GenPass` (same `Name`, `Weight`, `Enabled`) whose `ApplyPass` calls the original `inner.Apply(progress, configuration)` (GenPass.cs:30-33) and then writes the pass row. Only 104 of the 106 passes are `PassLegacy`; `Terrain` is `TerrainPass` and `Jungle` is `JunglePass`, hence a subclass instead of wrapping `PassLegacy._method`. The game's own `WorldGenerator.GenerateWorld` loop, `RunPass` and `OnPassCompleted` run unmodified | WorldGenerator.cs:421-517 |
| f | after `GenerateWorld` returns (that includes `Finish()` and the `finally` block): the last row; then `WorldFile.SaveNewWorld()` | WorldGen.cs:11097-11114; WorldFile.cs:708 |

`mode real` (`gen <outdir> <tag> real task`) skips the replay and the hooks and just runs `WorldGen.serverLoadWorldCallBack()` on a task thread: byte-for-byte the server's path.

**How a port should mirror the loop** (this is exactly what the game does, with the numbers above as targets):

```
Reset()                                  # Main.rand = UnifiedRandom(12345) at its start; row 0 = tile map after this + rand state at its end
for i, pass in enumerate(passes):        # 106 passes, order = table below, none disabled
    Main.rand = UnifiedRandom(12345)     # WorldGenerator.cs:499: EVERY pass starts from a freshly seeded generator
    pass.apply(...)                      # row i+1 is taken here: tiles, rand state (still before the next line)
    ref_next = genRand.Next()            # the game stores it as GenPassResult.RandNext; equals "rand.next" in the row
Finish()                                 # row 107
```

Consequences: the generator state at the start of pass N does not depend on how many numbers earlier passes consumed (only the tile/world state does), so a divergent pass can be
debugged in isolation from a snapshot of the previous pass. Per-pass `RandNext` values are also stored inside every `.wld` this game version writes
(manifest JSON in the world header, see `wld_diff.py`), together with `FinalHash` (4265538313 = `WorldGenerator.HashWorld()` of the final map).

## 3. File formats

### `ref_small.passes.jsonl`

UTF-8, `\n`, one object per line, 108 lines: row 0 = state after `clearWorld()`+`Reset()` and before pass 0 (`"i":-1`), rows 1..106 = after pass `i` = 0..105 (taken inside `ApplyPass`,
before `RunPass` calls `progress.End()` and `genRand.Next()`), row 107 = after `WorldGen.GenerateWorld` returned (`"i":106`, `"extra":true`).

| key | meaning |
|---|---|
| `row` | 0-based line number; also the row of `ref_small.columns.bin` |
| `i`, `name`, `extra` | pass index in `WorldGenerator._passes`, `GenPass.Name`; `extra` is true for rows 0 and 107 |
| `tiles_fnv64` | hex u64, FNV-1a 64 over the **whole** tile map, see "tile hash" below |
| `cols_fnv64` | hex u64, FNV-1a 64 over the 4200 column hashes of this row (each as 8 little-endian bytes) |
| `n_active`, `n_wall`, `n_liquid` | tiles with the active bit (sTileHeader & 0x20), with `wall != 0`, with `liquid != 0` |
| `rand` | `Main.rand` (= `WorldGen.genRand`) state: `inext` (u32 field of `UnifiedRandom`), `seed` (the 56-int `SeedArray`), `next` = the value `Next()` would return **without consuming** (computed on a copy). `Next()` advances `inext` first (UnifiedRandom.cs `InternalSample`). `next` equals the game's `GenPassResult.RandNext` for all 106 passes (checked: 0 mismatches). |
| `main` | selected statics of `Terraria.Main` after the pass: `maxTilesX/Y`, `worldSurface`, `rockLayer` (+ Low/High variants if static), `spawnTileX/Y`, `dungeonX/Y`, `time`, `dayTime`, `moonPhase`, `moonType`, `treeX/treeStyle`, `caveBackX/caveBackStyle`, background styles, `cloudBGActive`, `slimeRainTime`, weather values, `halloween`, `xMas`, `netMode`, `gameMenu`, `dedServ`, the seed-world flags, `worldName`, invasion fields. (Names that are not static fields were skipped; the list is in `Gen.cs:InitSelectors`.) |
| `worldgen_static` | **every** primitive/enum static field of `Terraria.WorldGen` (123 fields, e.g. `crimson`, `heartCount`, `generatingWorld`, `isGeneratingOrLoadingWorld`), name-sorted |
| `genvars` | **every** static field of `Terraria.WorldBuilding.GenVars` (not the properties, not `configuration`, not `tileSolid`), name-sorted |
| `counts` | `chests` (non-null `Main.chest`), `signs`, `npcs_active`, `items_active`, `tile_entities` (`TileEntity.ByID`), `tile_entity_positions` |
| `chests_signs_npcs_fnv64` | FNV-1a 64 of the UTF-8 text `x,y,name\|type:stack:prefix;...` per chest (`-;` for an empty slot), `S x,y,text` per sign, `N type,{position json}` per active NPC, one per line, in array order |

JSON value rules: ints/bools as JSON; enums as their integer; `double` with `"R"` round-trip text; `float` widened to double (exact); `NaN/Inf` as strings; structs and
classes as objects of their instance fields (`Point` = `{X,Y}`, `Rectangle` = `{X,Y,Width,Height}`, `Vector2D`, `Point16`, `DungeonGenVars` ...); arrays and lists of **up to 64** elements in full; longer ones as
`{"len":N,"fnv64":"hex"}` where the hash is FNV-1a 64 over the raw little-endian element bytes (bool = 1 byte, int = 4, double = 8, ...) for primitive element types and over the UTF-8 text of the
element JSON (comma separated) otherwise. (E.g. `snowMinX/snowMaxX` (1200 ints), `larvaX` (100 ints), `structures`.)

### Tile hash and `ref_small.columns.bin`

Tile record (14 bytes, little endian, exactly the fields of `Terraria.Tile`; a null tile is 14 zero bytes) - the layout of `refdata/framed/ref_small.post.bin`
(documented in `port/crates/terraria-sim/tests/tile_frame_reference_world.rs`):

```
u16 type | u16 wall | u8 liquid | u16 sTileHeader | u8 bTileHeader | u8 bTileHeader2 | u8 bTileHeader3 | i16 frameX | i16 frameY
```

Order: column-major (x outer 0..4199, y inner 0..1199) like the world file. FNV-1a 64: `h = 0xcbf29ce484222325; for each byte: h ^= b; h *= 0x100000001b3` (mod 2^64).

* `column_hash[x]` = FNV-1a 64 over the 1200 x 14 = 16800 bytes of column x (basis restarted per column).
* `tiles_fnv64` = FNV-1a 64 over all 4200 x 16800 bytes in order, **no restart** between columns.
* `cols_fnv64` = FNV-1a 64 over the 4200 column hashes written as 8-byte little-endian values.

`ref_small.columns.bin` = 108 rows x 4200 x u64 little endian (3,628,800 bytes), row `r` = `column_hash` array of line `r` of the jsonl. To localise a mismatch: compare
`cols_fnv64`, then the column rows, then fetch the snapshot of the nearest earlier row.

### `*.tfrm.z` snapshots

zlib stream (RFC 1950: `78 9C`, deflate, big-endian Adler-32) of exactly the file layout of `refdata/framed/ref_small.post.bin`: header `"TFRM" | u32 version=1 | i32 width | i32 height | u32 recordSize=14`
(20 bytes) followed by width x height records as above (70,560,020 bytes uncompressed, 0.07-3.6 MB compressed). Files: `before_pass_000` (= row 0), `after_pass_000` (row 1, "Terrain"),
`after_pass_001`, then every 10th pass (`010`..`100`), `after_pass_105` (row 106, "Final Cleanup"), `after_generate_world` (row 107). `summarize.py` decompresses them and recomputes
the column hashes (all match; `--full` also the whole-map hash).

## 4. Pass list (106)

Index = `"i"`; row = i + 1. Classes: all `PassLegacy` except 0 `TerrainPass` and 17 `JunglePass`.

| i | pass | i | pass | i | pass |
|---|---|---|---|---|---|
| 0 | Terrain | 36 | Gravitating Sand | 72 | Hellforge |
| 1 | Dunes | 37 | Create Ocean Caves | 73 | Spreading Grass |
| 2 | Ocean Sand | 38 | Shimmer | 74 | Surface Ore and Stone |
| 3 | Sand Patches | 39 | Clean Up Dirt | 75 | Place Fallen Log |
| 4 | Tunnels | 40 | Pyramids | 76 | Traps |
| 5 | Mount Caves | 41 | Dirt Rock Wall Runner | 77 | Piles |
| 6 | Dirt Wall Backgrounds | 42 | Living Trees | 78 | Spawn Point |
| 7 | Rocks In Dirt | 43 | Wood Tree Walls | 79 | Grass Wall |
| 8 | Dirt In Rocks | 44 | Altars | 80 | Guide |
| 9 | Clay | 45 | Wet Jungle | 81 | Sunflowers |
| 10 | Small Holes | 46 | Jungle Temple | 82 | Planting Trees |
| 11 | Dirt Layer Caves | 47 | Hives | 83 | Herbs |
| 12 | Rock Layer Caves | 48 | Jungle Chests | 84 | Dye Plants |
| 13 | Surface Caves | 49 | Settle Liquids | 85 | Webs And Honey |
| 14 | Wavy Caves | 50 | Remove Water From Sand | 86 | Weeds |
| 15 | Generate Ice Biome | 51 | Oasis | 87 | Glowing Mushrooms and Jungle Plants |
| 16 | Grass | 52 | Shell Piles | 88 | Jungle Plants |
| 17 | Jungle | 53 | Smooth World | 89 | Vines |
| 18 | Mud Caves To Grass | 54 | Waterfalls | 90 | Flowers |
| 19 | Full Desert | 55 | Ice | 91 | Mushrooms |
| 20 | Mushroom Patches | 56 | Wall Variety | 92 | Gems In Ice Biome |
| 21 | Marble | 57 | Life Crystals | 93 | Random Gems |
| 22 | Granite | 58 | Statues | 94 | Moss Grass |
| 23 | Floating Islands | 59 | Buried Chests | 95 | Muds Walls In Jungle |
| 24 | Dirt To Mud | 60 | Surface Chests | 96 | Larva |
| 25 | Silt | 61 | Jungle Chests Placement | 97 | Micro Biomes |
| 26 | Shinies | 62 | Water Chests | 98 | Settle Liquids Again |
| 27 | Webs | 63 | Spider Caves | 99 | Cactus, Palm Trees, & Coral |
| 28 | Underworld | 64 | Gem Caves | 100 | Tile Cleanup |
| 29 | Corruption | 65 | Moss | 101 | Lihzahrd Altars |
| 30 | Lakes | 66 | Temple | 102 | Water Plants |
| 31 | Slush | 67 | Cave Walls | 103 | Stalac |
| 32 | Dungeon | 68 | Jungle Trees | 104 | Remove Broken Traps |
| 33 | Mountain Caves | 69 | Floating Island Houses | 105 | Final Cleanup |
| 34 | Beaches | 70 | Quick Cleanup |  |  |
| 35 | Gems | 71 | Pots |  |  |

(The progress texts in `work/refserver.log` are the pass **messages** set via `progress.Message`, not the pass names; e.g. pass "Terrain" prints "Generating world terrain". There is no pass called "Reset":
`WorldGen.Reset()` runs before the first pass and is row 0.)

## 5. Determinism results (real game, 32-bit, this machine)

Every run below is a fresh process; `determinism/report.txt` is the output of `check_determinism.py`. Baseline = run `a`.

| run | what | tiles / chests / signs / NPCs / ... vs original `ref_small.wld` | per-pass jsonl + columns vs baseline |
|---|---|---|---|
| `a`, `a2`, `f` | instrumented, task thread | identical | identical (sha256 equal) |
| `m` | instrumented, **main thread** | identical | identical |
| `t` | instrumented, `Main.rand = new UnifiedRandom()` (tick-seeded) before `Initialize`, as the real server | identical | identical |
| `h0` | forced `Main.halloween = false` before every pass | identical | tiles/columns identical; only `main.halloween` differs |
| `x1` | forced `Main.xMas = true`, halloween false before every pass | identical | tiles/columns identical; only `main.halloween`, `main.xMas` differ |
| `r1`, `r2` | **unmodified server path** (`serverLoadWorldCallBack`, no hooks), two processes | identical | (no dump) |

**Byte-level difference to the original `work/refsave/Worlds/ref_small.wld` (2,987,873 bytes), for every run** (`wld_diff.py`): Tiles (2,954,062 bytes), Chests (21,759), Signs, NPCs, TileEntities, WeightedPressurePlates, TownManager,
Bestiary, CreativePowers and Footer are **byte-identical**. Only the FileFormatHeader pointers and the WorldHeader differ, in exactly these fields:

| field | bytes that differ | why |
|---|---|---|
| `WorldHeader.unique_id` (GUID, header offset 24 after name/seed/generator version) | 16 of 16 | `Guid.NewGuid()` in `CreateMetadata` (WorldFile.cs:611) |
| `creation_time` (i64 `DateTime.ToBinary`, flags offset 13) | 4-5 of 8 | `DateTime.Now` (WorldFile.cs:607) |
| `last_played` (i64, the next 8 bytes) | 4-5 of 8 | `DateTime.UtcNow` at save |
| world manifest JSON (`WorldGen.Manifest.Serialize()`, last string of the header) | the `DurationMs` numbers only | `Stopwatch` per pass (WorldGenerator.cs:498,514); `Name`, `RandNext`, `Hash`, `Skipped`, `Version`, `GitSHA`, `FinalHash` equal in all 106 entries. The number of digits changes the string length, hence the file length (original 2,987,873; unmodified reruns 2,987,874-2,987,875; instrumented runs about 2,988,005-2,988,021 because the hooks make the passes slower) |
| the 11 section pointers in the FileFormatHeader (file offsets 26..69) | 10-14 bytes | they move with the manifest length |

If the world is compared **with the same initialisation order as the real server** nothing else differs. (One earlier harness version initialised the game before applying `maxTilesX`:
`WorldHeader.cave_back_style[2..3]` were then 6 and 3 instead of 0 and 0, because for a small world `RandomizeCaveBackgrounds` (WorldGen.cs:8184-8195) only sets entries 0 and 1 and the others keep whatever the
static array held. That is a real header-content dependence on initialisation order; section 6.)

The tiles of the generator are therefore **deterministic given the seed**; the port can treat `ref_small.wld`'s tile section (and the per-pass dump) as exact targets.

## 6. Every source of non-determinism found (and whether it reaches the tiles)

| source | where | reaches tiles? | handling |
|---|---|---|---|
| `Guid.NewGuid()` world id | WorldFile.cs:611 (also 1983) | no, header only | listed above |
| `DateTime.Now` creation time, `DateTime.UtcNow` last played | WorldFile.cs:607; save | no | header only |
| `Stopwatch` pass durations | WorldGenerator.cs:498,514 | no | manifest JSON only |
| `Main.rand = new UnifiedRandom()` (`Environment.TickCount`) | Main.cs:5375 | no | replaced by `Reset()` (WorldGen.cs:11153) and per pass (WorldGenerator.cs:499); run `t` proves it |
| `Main.rand` created from `DateTime.Now.Ticks` when null | WorldFile.cs:652-655, NPC.cs:94522, Dust.cs:83, ... | no | **`Main.rand` is `[ThreadStatic]` (Main.cs:1367)**: the generation thread-pool thread starts with null, so this fires there; same reseeding as above |
| `Main.halloween`, `Main.xMas` from `DateTime.Now` | Main.cs:13323-13377, called at WorldFile.cs:658-659 and WorldGen.cs:11249-11250 | **no** (runs `h0`/`x1`) | used only by shop stock, `KillTile`/`ShakeTree` drops and `UpdateWorld` (WorldGen.cs:52895, 63791, 63884, 75417), never by a pass. Generation does depend on `isHalloweenDateNow()` for a **random** seed only (`SetSeedToRandomWithCurrentEvents`, WorldFileData.cs:417-425), not for seed 12345 |
| thread timing | `Task.Factory.StartNew` | no | runs `m` (main thread) = `a` (task thread); generation is single-threaded. `FastParallel.For` appears only in `HashWorld` (debug hash) |
| other unseeded `UnifiedRandom()` (clouds, hit tiles, tile renderer, ambient wind, `FlexibleTileWand`, sky classes, audio) | see `refdump_ticks` pin list | no | not used by passes |
| `FastRandom.CreateWithRandomSeed()` (`Guid.NewGuid().GetHashCode()`) | FastRandom.cs:37-40; Main.cs:59676 (wind counter), Star.cs, light scanners | no | not used by passes (the passes that use `FastRandom` seed it from the world seed: DesertHive.cs:232 `WithModifier(57005)`, GraniteBiome.cs:312 `WithModifier(65440)`) |
| secondary generators seeded from the main stream | `new UnifiedRandom(settings.RandomSeed)` in the dungeon code (GameContent/Generation/Dungeon/*, 25 sites incl. Biomes) | no (deterministic) | **the port must reproduce these separate streams too; they are invisible in the `genRand` state dump** |
| static leftovers from `Initialize` | `Main.caveBackStyle[2..3]` (section 5) | header | apply launch parameters before `Initialize` |

Not counted: the number of `genRand` calls per pass cannot be recovered without modifying the game (the 56-entry state is cyclic), so it is not dumped.

## 7. Things a porter must know

* **Each pass restarts `genRand` from the world seed** (`WorldGenerator.cs:499`). Only `Reset()` consumes a long prefix (jungle hut, crimson, ore tiers, backgrounds, moon, dungeon side, world id, ...), and its draws
  are consumed once, before pass 0 (their results are in the row-0 `genvars`/`main` values).
* `Main.rand` (and so `WorldGen.genRand`) is a **thread-static**: code that runs on another thread sees a different generator.
* The world file stores less than memory holds: non-important tiles' frames are not saved, so the state in `after_generate_world` is **not** the state of `refdata/framed/ref_small.pre*` (that is after `LoadWorld`:
  tile frames reset, `ClearTempTiles`, liquid settle). Compare generator output with the snapshots here, not with the framed "pre" data.
* The progress texts differ from the pass names (section 4). Passes depend on `Main.ActiveWorldFileData.Seed` not only through `genRand` (FastRandom in DesertHive/Granite, `RandomSeed` fields in the dungeon).
* The generator runs on one worker thread; the host thread is idle. Wall time: the unmodified `real` path takes 13-14 s on this machine; the instrumented run takes 75-100 s, almost all of it hashing/snapshotting the 5 M tiles after each of the 106 passes.

## 8. x87 maths tables (`x87/`)

The harness is a 32-bit .NET Framework 4 process (`IntPtr.Size == 4`, CLR 4.0.30319.42000, `/platform:x86`, optimised); `System.Math` is called directly, results stored through a `double[]` element.
`x87/x87_env.json` records the evidence that these are the x87 FPU instructions: `_controlfp` word `0x9001f` (precision control **53 bit**, round to nearest, all exceptions masked),
`Math.Sin(Math.PI) = 1.2246063538223773e-16` (the `fsin` signature; a correctly rounded libm gives 1.2246467991473532e-16), `Math.Sin(1e22) = 1e22` (`fsin` leaves out-of-range operands unchanged;
libm gives -0.8522008497671888), `Math.Cos(Math.PI/2) = 6.1230317691118863e-17` (libm 6.123233995736766e-17).

Layout of `x87/<Function>.bin` (little endian): `"X87T"`, `u32 version = 1`, `u32 nargs` (1 or 2), `u32 count` (20000), `u32 0`, then `count` records of `nargs + 1` raw IEEE-754 binary64 values:
argument 0 [, argument 1], result (NaN/inf/-0 exactly as produced). Functions: `Sin Cos Tan Asin Acos Atan Atan2(y,x) Sqrt Pow(x,y) Exp Log Log10 Floor Ceiling Round(banker's) RoundAway
(MidpointRounding.AwayFromZero) Abs`.

Inputs (deterministic xorshift64*, state `0x9E3779B97F4A7C15`, `x ^= x>>12; x ^= x<<25; x ^= x>>27; out = x * 0x2545F4914F6CDD1D`, uniform = `(out>>11) * 2^-53`): the file starts with the special values
(0, -0, +-1, 0.5, 2, 0.1, 1e-300, the smallest normal, denormals, +-inf, NaN, +-max, 1e6, 1e22, 2^52, 2^53, 0.49999999999999994, +-1.5/2.5/3.5, pi, e, 10, 100; for trig also every k*pi/2, k = -16..16, as double and as float;
for Atan2 all 7x7 combinations of {0,-0,1,-1,+inf,-inf,NaN}; for Pow 15 bases x 20 exponents incl. 0.5, 2, 3, 1.5, 1/3, (float)(1/3), 0.25, 1.25, negatives, plus infinities), then random draws up to 20000 records:
trig: 40% uniform [-10,10], 20% the same narrowed to float, 10% [-1e6,1e6], 10% [-100,100], 10% [-1,1], 10% [-2pi,2pi]; Sqrt: [0,1e6], float-valued, [0,1], [0,1e12], float products, [0,1e3]; Log/Log10: [0,1e6], [0,1], float,
[0,100], exp(uniform[-700,700]); Exp: [-5,5], [-50,50], float [-20,20], [-700,700]; Pow: x in [0,100] (also float, [0,1], [0,1e4], negative x with integer exponent) with y from the list above or uniform [-5,5];
Floor/Ceiling/Round/Abs: [-1e6,1e6], exact halves, float-valued, [-10,10], [-1e15,1e15].

Measured against a correctly rounded libm (Python `math`; `summarize.py`/section "x87" output): `Sqrt`, `Abs`, `Floor`, `Ceiling`, `Round`: bit-identical (the sign of zero is preserved);
`Atan2` 0.1% of records differ, `Pow` 0.1%, `Log`/`Log10` 0.04%, `Exp` 0.9%, `Atan` 3.4%, `Asin` 4.2%, `Acos` 5.5% (all <= 1 ulp);
`Sin`/`Cos`/`Tan`: 3% of the records with |x| <= 10, 5% for 10 < |x| <= 100 and **88-98% for |x| > 100** (the x87 reduction uses a 66-bit pi, so large and near-multiple-of-pi arguments differ by many ulp; `Math.Tan` up to 3e11 ulp).
A port that wants bit-exactness must emulate `fsin/fcos/fptan/fpatan/f2xm1/fyl2x` (and `fprem`-style reduction), not call libm.

## 9. Float expression probes (`float_probes/`)

Why: the 32-bit legacy JIT keeps `float` temporaries on the x87 stack. `probes_source.cs` is the exact C# of the 34 probes (compiled `/optimize+`, each `[MethodImpl(NoInlining)]`, the `Vector2` ones use the
real `Microsoft.Xna.Framework.Vector2` from `work/refserver`). `probes.json` lists name and result kind in file order.

Layout of `probes.bin` (little endian): `"FPRB"`, `u32 version = 1`, `u32 nprobes = 34`, `u32 ninputs = 2000`, `u32 0`; then 2000 input records `f32 x | f32 y | f32 z | i32 n`; then 34 blocks of 2000 `u64`
results in the order of `probes.json`: kind `f` = the f32 bit pattern in the low 32 bits (high 32 zero), `d` = the f64 bit pattern, `i` = the int32 sign-extended.
Inputs: xorshift64* seed `0xD1B54A32D192ED03` (same recurrence as above); per component 1/12 a typical game constant (0.97f, 0.1f, 0.5f, 1/3f, 16f, 0.0625f, 0.99f, 1.5f, 2f, -1f, 0f, 0.01f), otherwise 3/8 uniform [-1,1], 3/8 [-100,100],
2/8 [-5000,5000] narrowed to float; `n` uniform in [-20,63]. Results are stored through a `float[]`/`double[]` cell by the caller (a store to memory).

`floatprobe_model.py` tests the translator's current rule ("expression trees in f64, round at stores / args / returns / casts", **model A**) against the data, and a second rule (**model B**: float *locals* are not rounded):

| probes | result |
|---|---|
| P02, P04, P05, P06, P07, P09, P16-P30 (returns, static/instance field stores, `(int)` casts, call arguments, `double d = x*y`, `Math.Sqrt/Pow/Sin/Cos/Atan2` casts, `Math.Max/Min`, int/float mixes) | model A exact, 2000/2000 |
| P01, P03, P08, P10, P11 (float **locals**: `float a = x*y+z; return a-x;`, compound assignment on locals, `float p = x*y; p == x*y`, loop accumulators) | model A wrong for 521, 614, 1856, 1265, 742 of 2000; **model B exact, 2000/2000** |
| P12-P15 (`Vector2` locals: fields updated in place, operators, `Length`, `Normalize`) | model A wrong for 721, 764, 243, 634; model B exact (inlined operator temporaries and `Vector2.Length`'s `num` local are not rounded; `Normalize`'s `(float)Math.Sqrt` cast and the field stores are) |
| P32 (local live across a call), P33 (accumulator with a call in the loop), P34 (local passed by `ref`) | model A exact; model B wrong (the value is spilled to memory: rounded to float) |
| P31 (ten float locals all live at the end) | neither: a brute-force search over which locals are rounded finds exactly one assignment that matches all 2000 results: **a0..a5 unrounded, a6..a9 rounded**, i.e. the JIT keeps about the first six FP locals on the x87 stack and spills the rest |

So the JIT's rule is: a float value that stays in an x87 register keeps the 53-bit double precision (control word, section 8), and is rounded to float only when it is stored to memory (field, static, array element,
argument, return slot, spilled local) or cast explicitly. Whether a *local* is in a register depends on the JIT's register allocation (calls, address-taken, more than about six live FP values spill), so a purely syntactic
"round at stores to locals" rule is wrong for single-assignment temporaries, and "never round locals" is wrong across calls and in big methods. The data for P01-P34 is there to test any refined rule.

## 10. Re-running

`tools/refdump_worldgen/run.cmd` (build + gen + x87 + probes; about 2.5 minutes; writes into this folder, scratch worlds under `work/worldgen-scratch/`; the game folder `work/refserver` is only read).
Determinism experiments (each a fresh process; environment variables are read by the harness):

```
RefDumpWorldgen.exe gen <out> <tag> passes task            baseline (add "nosnap" as 6th argument to skip the snapshots)
RefDumpWorldgen.exe gen <out> <tag> passes main nosnap     main thread instead of a task thread
set REFDUMP_TICKRAND=1  ... gen ...                        Main.rand = new UnifiedRandom() (tick seeded) before Initialize, like the server
set REFDUMP_FORCE_DATE=halloween0 | xmas1 | halloween1 ... forces Main.halloween / Main.xMas before every pass
RefDumpWorldgen.exe gen <out> <tag> real task              the unmodified server path (no per-pass dump)
python tools/refdump_worldgen/check_determinism.py work/worldgen-scratch work/refsave/Worlds/ref_small.wld a a2 m t h0 x1 f r1 r2
```

Every process has a 7-minute watchdog; no process opens a socket (`Netplay.StartServer` is never called), no window or console input is used, nothing is written below `work/refserver` or the Documents\Games folder.

## 11. More x87 data: `x87_raw/`, `x87_dense/`, `x87_more/` (second harness program)

Produced by `tools/refdump_worldgen/bin/RefDumpX87Ext.exe` (`build_ext.cmd`, sources `ExtProgram.cs`, `ExtX87.cs`, `ExtProbes.cs`,
`ExtProbesGen.cs`; a separate 32-bit .NET Framework 4 console program, so `RefDumpWorldgen.exe` and the files of sections 1 to 10 are
untouched: their sha256 were compared before and after). Consumer: `port/crates/terraria-x87` (`tests/tables.rs`), whose README has
the results. Sizes: `x87_raw` 32 MB, `x87_dense` 62 MB, `x87_more` 12 MB.

Random inputs everywhere: the xorshift64* of section 8 (`s ^= s>>12; s ^= s<<25; s ^= s>>27; out = s * 0x2545F4914F6CDD1D`,
`u = (out>>11) * 2^-53`, `Range(lo, hi) = lo + (hi - lo) * u`), seeds as listed.

### `x87_raw/<Insn>.bin`: the instructions themselves, at register width

`"X87R" | u32 version = 1 | u32 nargs | u32 count | u32 0`, then `count` records `nargs x f64 arguments | 10 bytes | f64`: the 10 bytes
are the x87 register after the instruction (`fstp tbyte`: u64 significand, u16 sign + exponent, little endian), the f64 is what
`System.Math` returns for the same arguments in the same process. The instruction runs in a machine-code thunk of 15 to 20 bytes
(`mov eax,[esp+4]; fld qword [eax]; <insn>; mov eax,[esp+8]; fstp tbyte [eax]; ret`) placed in a `VirtualAlloc` page and called
through a delegate; the thunks are listed in `ExtX87.cs` (`Raw`).

| file | instruction | `System.Math` column | records |
|---|---|---|---|
| `Fsin` `Fcos` | `fsin` / `fcos` | `Math.Sin` / `Math.Cos` | 244,000 each |
| `Fptan` | `fptan; fstp st(0)` | `Math.Tan` | 244,000 |
| `Fpatan1` | `fld1; fpatan` | `Math.Atan` | 170,000 |
| `Fpatan` | `fpatan` (arguments y, x) | `Math.Atan2` | 220,000 |
| `Fyl2x` | `fyl2x` (arguments y, x) | `y * Math.Log(x, 2)` (not the same computation: for inspection only) | 50,000 |
| `Fsqrt` | `fsqrt` | `Math.Sqrt` | 20,000 |

Arguments: first the arguments of the matching table of section 8 (`x87/Sin.bin` ... `Sqrt.bin`), then (seed `0xA5A5A5A55A5A5A5A`, one
generator running through all files in the order of the table) for the three trigonometric files 100,000 uniform in [-10, 10], 40,000
in [-1000, 1000], 30,000 with magnitude `2^Range(-60, 64)` and random sign, 30,000 near multiples of pi/2
(`k * (pi/2) * (1 + (u - 0.5) * 2^-j)`, k in 1..1000, j in 0..49), 20,000 float-valued in [-10, 10], 2,000 consecutive doubles around 2^62
and 2,000 around 2^63; `Fpatan1`: 100,000 in [-10, 10], 50,000 with magnitude `2^Range(-80, 80)`; `Fpatan`: 80,000 pairs in
[-1000, 1000]^2, 60,000 float-valued pairs, 30,000 pairs of magnitudes `2^Range(-40, 40)` with random signs, 30,000 pairs of integers in
[-32, 32]. Findings: the `System.Math` column equals the register value rounded to double for every record (Atan2: except when both
arguments are infinite, where the CLR returns NaN); the register value is the correctly rounded sine / cosine / tangent of the
argument reduced with the 66-bit pi in 97 to 99% of the records and one unit of the last place next to it otherwise.

### `x87_dense/<Name>.bin`: dense `System.Math` tables, results only

`"X87D" | u32 version = 1 | u32 nargs | u32 count | u32 mode | u64 seed | f64 lo0 | f64 hi0 | f64 lo1 | f64 hi1 | u64 fnv | count x f64 result`
(header 68 bytes). The arguments are not stored: record i has `arg0 = Range(lo0, hi0)` and, when `nargs == 2`, `arg1 = Range(lo1, hi1)`
drawn in that order from a generator started at `seed`; `mode == 1` narrows every argument to float and widens it again. `fnv` is the
FNV-1a 64 over the argument bit patterns (8 little-endian bytes each, in drawing order) so that a reader can check its regenerated
arguments (the Rust test does).

| file | function | arguments | records | seed |
|---|---|---|---|---|
| `Sin_m10_10`, `Sin_0_1000`, `Sin_f32_m7_7` | `Math.Sin` | [-10, 10], [0, 1000], float in [-7, 7] | 1,000,000 / 1,000,000 / 250,000 | `0x1111..`, `0x2222..`, `0x3333..` |
| `Cos_m10_10`, `Cos_0_1000`, `Cos_f32_m7_7` | `Math.Cos` | the same | the same | `0x4444..`, `0x5555..`, `0x6666..` |
| `Tan_m10_10`, `Atan_m100_100` | `Math.Tan`, `Math.Atan` | [-10, 10], [-100, 100] | 250,000 each | `0x7777..`, `0x8888..` |
| `Atan2_m1000_1000`, `Atan2_f32_m1000_1000` | `Math.Atan2(arg0, arg1)` | [-1000, 1000]^2, doubles / floats | 250,000 each | `0x9999..`, `0xAAAA..` |
| `Pow_0_100_m5_5`, `Pow_f32_0_10_0_4` | `Math.Pow(arg0, arg1)` | [0, 100] x [-5, 5]; floats in [0, 10] x [0, 4] | 250,000 each | `0xBBBB..`, `0xCCCC..` |
| `Log_0_1000`, `Log10_0_1000`, `Exp_m20_20` | `Math.Log`, `Log10`, `Exp` | [0, 1000], [0, 1000], [-20, 20] | 250,000 each | `0xDDDD..`, `0xEEEE..`, `0x1212..` |
| `Acos_m1_1`, `Asin_m1_1` | `Math.Acos`, `Asin` | [-1, 1] | 250,000 each | `0x2323..`, `0x3434..` |
| `Sinh_m10_10`, `Cosh_m10_10`, `Tanh_m10_10` | `Math.Sinh`, `Cosh`, `Tanh` | [-10, 10] | 250,000 each | `0x4545..`, `0x5656..`, `0x6767..` |

(Each seed is the two hex digits repeated eight times; the header of the file has the exact value.)

### `x87_more/<Name>.bin`: functions and shapes the first dump did not cover

Format X87T of section 8 (20,000 records unless noted, seed `0x0F1E2D3C4B5A6978`, one generator through all files in the order of
`ExtX87.cs` `More`), special values first:
`Sinh`, `Cosh`, `Tanh`; `Truncate`; `Sign` (result as double, -99 = `ArithmeticException` for NaN); `Round0` `Round1` `Round2` `Round3`
`Round4` `Round15` = `Math.Round(x, digits)`; `Round2Away` = `Math.Round(x, 2, MidpointRounding.AwayFromZero)`; two-argument tables
`IEEERemainder`, `Min`, `Max`, `Fmod` (`x % y`) with 17 x 17 special pairs, random pairs, angle wrapping by 6.283185307179586 and exact
ties; `PowSpecial` (900 records: 30 x 30 special operands of `Math.Pow`).
Results used without a store (each a `[NoInlining]` method, source in `ExtX87.cs`): `SinMul` `CosMul` `TanMul` `AtanMul`
(`Math.F(x) * y`), `SinAdd` (`Math.Sin(x) + y`), `Atan2Mul3` (`Math.Atan2(y, x) * 57.29577951308232`), `SinLocalMul`
(`double s = Math.Sin(x); double c = Math.Cos(x); return s * y + c;`), `SqrtMul`, `PowMul` (`Math.Pow(x, 1.5) * y`), `DblLocal` /
`DblField` (the same double expression through a double local / a static double field).
Float tables, format `"X87F" | u32 version = 1 | u32 nargs | u32 count | u32 0 | count x { nargs x f32 | f32 result }`: `SinF`, `CosF`
(`(float)Math.Sin(f)`), `SqrtF`, `Atan2F` (`(float)Math.Atan2(y, x)`), `PowF`, `AbsMulF` (`Math.Abs(x * y)` with float operands).

## 12. Float probes, second set (`float_probes2/`)

64 probes x the same 2,000 inputs as section 9 (same generator and seed; `probes2.bin` repeats them). Layout of `probes2.bin` as
`probes.bin` with the magic `"FPR2"`; `probes2.json` lists name, kind (`f` / `i`) and IL size in file order (ordinal name order);
`probes2_source.cs` is the C# of every probe. 48 probes are generated from `tools/refdump_worldgen/float_model/probes.py`
(`gen_cs.py` writes `ExtProbesGen.cs`), 16 are written by hand in `gen_cs.py` (real XNA `Vector2` / `MathHelper` members, helpers
without `NoInlining`). Groups: `L..` number and competition of live locals, `C..` calls, `S..` control flow and parameters, `T..`
try / catch, `M..` memory kinds and casts, `B..` large methods (4,000 filler statements, 70 to 300 hot int locals), `I..` inlining,
`V..` XNA members.

`probes2_code.bin`: `"JITC" | u32 version = 1 | u32 nprobes`, then per probe `u32 name length | name | u32 address | u32 nbytes | bytes`:
the memory at the native entry point of the jitted method, read after the probe ran (the method is shorter than `nbytes`;
`float_model/disasm.py NAME` prints it up to the last reachable `ret`, `calls.py` counts call instructions and float spills).
The addresses change from run to run, so this file is not byte-reproducible; the instructions are.

What the probes show, the rule derived from them, the list of undetermined cases and the scores of three rules on all 77 scorable
probes (translator `x87`: 33 exact, `x87-locals`: 52, the new rule: 72) are in `port/crates/terraria-x87/FLOAT_RULE.md`.
`float_model/score.py` reproduces the scores, `hand.py` the readings of the hand-written probes.

## 13. Re-running the second program

`tools/refdump_worldgen/run_ext.cmd` (build + `x87raw` + `x87dense` + `x87more` + `floatprobe2`; a few seconds; headless; writes only
into the four folders above; every process has a 7-minute watchdog). `x87raw` reads the argument lists of `x87/*.bin`, so `run.cmd`
must have produced those first. The Rust generator of the C runtime transliteration is separate:
`python tools/refdump_worldgen/crt_sse2/gen_crt_sse2.py` (needs `capstone`; reads `C:\Windows\SysWOW64\ucrtbase_clr0400.dll`).

## 14. The matrix: other seeds, sizes, game modes and special seeds (`matrix/`)

Produced by a THIRD program, `tools/refdump_worldgen/bin/RefDumpWorldgenX.exe` (`build_x.cmd`; sources `ProgramX.cs`, `GenX.cs`,
`JitDump.cs` + the shared `Common.cs`; `set_laa.py` marks the harness's own executable large-address-aware, as `TerrariaServer.exe`
is). `RefDumpWorldgen.exe`, `Gen.cs`, `Program.cs` and every file of sections 1 to 13 are untouched (sha256 of all 114 files
compared before and after). `sh tools/refdump_worldgen/run_matrix.sh check` reruns the "12345" / small scenario through the new
program into a scratch folder: `ref_small.passes.jsonl` and `ref_small.columns.bin` come out byte-identical to the ones here
(sha256 `1965499b...aca597` and `7329e40d...06eb8d`), so the new mode records what the first harness recorded.

```
RefDumpWorldgenX.exe genx <outdir> <tag> <seedtext> <small|medium|large> <classic|expert|master|journey>
                          <random|corruption|crimson> <flags|-> <passes|real> <none|default|i,j,..> [watchdog seconds]
sh tools/refdump_worldgen/run_matrix.sh [tag ...]        # the table below; SNAPS=default|i,j,.. adds tile snapshots
```

| tag | seed text | size | game mode | evil (param / result) | flag | passes / rows | real generation (instrumented) | peak working set |
|---|---|---|---|---|---|---|---|---|
| `ref_medium` | `2718281` | medium 6400 x 1800 | expert (1) | corruption (0) / corruption | - | 106 / 108 | 134 s | 503 MB |
| `ref_large` | `31415926` | large 8400 x 2400 | classic (0) | random (-1) / crimson | - | 106 / 108 | 248 s | 735 MB |
| `ref_bees` | `404` | small | classic | random / corruption | `notthebees` | 106 / 108 | 61 s | 353 MB |
| `ref_drunk` | `5162` | small | classic | random / corruption (a drunk world has both) | `drunk` | 106 / 108 | 74 s | 378 MB |
| `ref_worthy` | `1337` | small | master (2) | crimson (1) / crimson | `fortheworthy` | 106 / 108 | 63 s | 374 MB |

Files per world (57 MB in all; no tile snapshots are stored: `SNAPS=...` writes `<tag>.<name>.tfrm.z` on demand, format of
section 3, streamed column by column so a large world's 282 MB never sit in memory):

| file | what |
|---|---|
| `<tag>.passes.jsonl` | the rows of section 3, same keys and value rules. A pass disabled by `WorldGen.DisablePassesForSpecialSeeds` has no row (none is disabled in these five worlds) |
| `<tag>.columns.bin` | rows x `maxTilesX` u64 column hashes |
| `<tag>.wld` | the file the game's `WorldFile.SaveNewWorld` wrote |
| `<tag>.summary.json` | the arguments, `seed_int`, `world_id`, `crimson`, the options enabled, the `Main.*World` flags, pass names / classes / enabled, the game's own manifest (`RandNext`, `Hash`), the captured trace lines, times, peak working set |

Where each argument goes in the real dedicated server (all in the header comment of `GenX.cs` with line references):
size = `-autocreate 1|2|3` (`Main.autoCreate`), tag = `-worldname` + `-world <path>`, game mode = server config
`difficulty=` (`Main.GameMode`, which writes the `WorldFileData` of `-world`), flag = server config `seed_<name>=1`
(`WorldGenerationOptions.TryEnablingFlagFrom` -> `AutoGenEnabled`, turned into `Enabled` by `WorldFile.LoadWorld`:690-696),
seed text = `-seed`. Evil has no server parameter: the harness writes `WorldGen.WorldGenParam_Evil` as the client's world
creation page does (UIWorldCreation.cs:133-143).

**Special seeds are given as flags, not as their seed text, because the game does not generate from that text.** For a text
that names an option ("not the bees", "for the worthy", "5162020" ...; the names are `SpecialSeedNames` / `SpecialSeedValues`
of the `WorldSeedOption_*` classes, compared after lower-casing and dropping everything but letters and digits:
WorldGenerationOptions.cs:66-90) both `WorldFile.LoadWorld` (WorldFile.cs:697-700) and the client
(UIWorldCreation.cs:984-987) call `SetSeedToRandomWithCurrentEvents()`: `new UnifiedRandom().Next()`, tick seeded. Such a
world cannot be reproduced from its text. The server-config flag enables the same option and keeps the `-seed` text as the
seed, through the game's own code; `genx` refuses a seed text that maps to an option, with this reason.

Not covered: secret seeds (`WorldGen.SecretSeed`, the `|`-separated texts of a copied seed), copied seeds
(`size.mode.evil.options.seed`: `genx` accepts one, none is in the table), journey mode, the options `celebration`,
`theconstant`, `notraps`, `remix`, `zenith`, `skyblock`, and any world where a pass is disabled.

### `jitdump`: the JIT's machine code of game methods

`RefDumpWorldgenX.exe jitdump <outdir> <listfile>` (`JitDump.cs`): for every `Full.Type::Method[/N|#K]` of the list,
`RuntimeHelpers.PrepareMethod`, the native entry point behind the precode stub, and the bytes there (`ReadProcessMemory`).
Output `jit_code.bin` (the `JITC` container of section 12) and `jit_code.json` (IL size, IL bytes, parameters, IL locals with
their types). Nothing of the game runs except what the JIT triggers. It is the ground truth for the float-allocation
question (FLOAT_RULE.md) when a divergence needs it.

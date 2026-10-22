# cs2rs: the translated code against the real game

Hand-maintained summary of `port\crates\terraria-gen\tests\reference.rs` (one test, 38 s). The full output of a run
is written to `port\target*\tmp\cs2rs-reference.txt`. Reference data: `refdata\` (the game's own code run headless
by `tools\refdump*`). Reproduce:

```
sh tools/cs2rs/run.sh                                         # regenerate (12 s)
cd port && cargo test -p terraria-gen-rt -j 4                 # runtime + 35 snippets (6 s)
cargo test -p terraria-gen -j 4 --test reference -- --nocapture   # build 5.5 - 7 min, run 38 s
```

Result with the configuration in the repository (`core.json`, `"floatMode": "x87"`): **the test passes**.

Last run: 2026-10-11, after monomorphisation, the equality / enum-boxing / event-accessor changes and the LINQ
additions: generation 16 s, test build 7 min 40 s, run 38 s, 1 passed. The numbers of the table below did not
change. The runtime crate: 70 unit tests and 38 snippets pass (`cargo test -p terraria-gen-rt`).

## Status (2026-10-11, Proof 1)

**A. The whole game is generated and builds.** `game.json`: 2,763 files, 23,077 bodies (22,560 clean, 19 with
something unported inside, 163 refused, 335 not emitted). Generation 22 - 28 s; a second run changes 0 files. Test
build (opt-level 0, no debug info): 3.5 - 4.5 min, largest rustc about 5.5 GB. A change of a test file alone relinks
in 3 s.

**B. Proof 1: passes.** `port\crates\terraria-gen-game\tests\proof1.rs` (`sh tools/cs2rs/p1.sh`) performs the
sequence of `refdata\MANIFEST.json` with translated code only: no hand-sequenced initialisers, no survey mode, no
host-supplied names or texts. `main.Initialize()` is ONE call (0.65 s). Then every dump is compared, in the order
the harness wrote them (the order matters: `SetDefaults` draws from `Main.rand`, and the comparison reproduces the
state of the generator at every record).

| step | state |
|---|---|
| `Program.SavePath = <scratch>` | ok |
| `Main.dedServ = true; Main.showSplash = false` | ok |
| `LanguageManager.Instance.SetLanguage(GameCulture.DefaultCulture)` (the seven en-US JSON resources through the translated `LanguageManager` and the host crate's Newtonsoft implementation) | ok, 0.6 s |
| `main = new Main()` | ok |
| `Lang.InitializeLegacyLocalization()` | ok |
| `Main.rand = new UnifiedRandom(0)` | ok |
| `main.Initialize()` | ok, 0.65 s |

How the comparison reads the translated state: as `tools\refdump\RefDump.cs` read the real one. `tests\p1\dump.rs`
is a port of its reflection serialiser (`S` / `Members`: public fields, simple properties, the private-field
fall-back, dictionaries sorted by key, `HashSet`s sorted, delegates as `<delegate:Type.Method>`, the special cases
for `Item`, `RecipeGroup`, `LocalizedText`, `Color`) over the generated reflection data (`gen::meta`) and
`rt::obj_reflect`. Every scalar of a dump counts as one value; a structural difference makes everything below it
"not exact"; a member the translated side has and the dump lacks counts as a mismatch too.

| refdata file | values compared | exact | not exact | excluded / note |
|---|---:|---:|---:|---|
| `tile_arrays.json` | 31,247 | 31,247 | 0 | 38 tile arrays, 7 wall arrays, `tileMerge` pairs |
| `id_sets.json` | 1,084,953 | 1,084,953 | 0 | all 555 entries of 46 `Sets` types; the 31 `SetFactory` fields are skipped as in the dump. 43 of the values are delegates on lambdas: the dump has the compiler-generated method name (`NPCID+Sets+<>c.<.cctor>b__96_0`), which translated code does not have; they are compared as "is a delegate whose target is a lambda" and counted exact. Method-group delegates compare by name |
| `id_constants.json` | 12,465 | 12,465 | 0 | 52 classes / enums |
| `items.json` | 770,136 | 770,136 | 0 | 6,195 items, every dumped member incl. `ToolTip`, `Name`, `HoverName`, `UseSound` |
| `npcs.json` | 932,950 | 932,950 | 0 | 762 records, incl. the nested arrays and structs |
| `projectiles.json` | 738,635 | 738,635 | 0 | 1,135 projectiles, incl. `Name` and the values `SetDefaults` draws from `Main.rand` (78 values the core-set comparison had to leave out) |
| `buffs.json` | 5,214 | 5,214 | 0 | 13 arrays |
| `recipes.json` | 137,737 | 137,737 | 0 | 3,603 recipes, the recipe groups (with their localised names), `TileUsedInRecipes`, `TileCountsAs` |
| `tile_object_data.json` | 188,349 | 188,349 | 0 | 389 tile types with styles and alternates, every readable public property, 269 placement-hook delegates by method name |
| `framing_tables.json` | 704 | 704 | 0 | the 8 static tables |
| `lang_en.json` | 8,990 | 8,990 | 0 | item, NPC, projectile, buff names and descriptions, prefixes |
| **total** | **3,911,380** | **3,911,380** | **0** | |

That the comparison is not blind: on the way it reported 156 values (three runtime types without reflection data),
then 10 (`ItemID.Sets.BannerStrength[3837..3846].DamageReceived.keys[0].output`: 0.9166666 here, 0.9166667 in the
game), which is the float rule of the next section. There is no separate negative control in this test.

What the run reached that is not translated game code (printed by the test; anything else fails it):

- opaque library objects that are created and never used: `new TcpClient()` (`TcpSocket..ctor`), `new
  UTF8Encoding(true, true)` x 2 (`FavoritesFile`). Any use of them stops the run.
- members declared inert in `game.json`, with reasons: `Attribute..ctor` x 13, `Game..ctor` x 1,
  `TcpClient.NoDelay.set` x 1 (a limit: no socket host exists).
- configured values: `DateTime.UtcNow` x 5 (zero; `CreativePowers` field initialisers, a send throttle: never part of
  a compared value), `Guid.GetHashCode()` x 2 (zero; `Main.UnpausedUpdateSeed`, cosmetic randomness that differs in
  every run of the original).
- gaps (`rt::gap`), unimplemented `void` members: none.

**C. Collision, lighting and tile framing on this initialisation** (`tests\sim.rs` of the whole-game crate: the
sections of the core set's `reference.rs` with the Proof 1 set-up; build shared with `proof1`, run 87 s):

| section | compared | exact | not exact |
|---|---:|---:|---:|
| `Collision`, 62 families x 20,000 calls | 1,240,000 calls | 1,237,727 | 2,273 in 5 families (float bits; 16 calls of `CanHitLine` and 2 of `CheckLinevLine` differ in a decision) |
| `WorldGen.SectionTileFrame`, reference world | 5,040,000 tiles x 9 fields + next `Main.rand.Next()` | all | 0 |
| the same, scrambled region | 5,040,000 tiles x 9 fields + next `Main.rand.Next()` | all | 0 |
| `Lighting.GetColor`, ten areas | 175,900 tiles | 175,900 | 0 |
| raw light map, ten areas | 175,900 tiles x 3 floats | 174,972 tiles (4 areas exact) | 928 tiles, at most 4 ulp |

The core set (`terraria-gen`, 34 hand-sequenced steps) is no longer needed for coverage: every comparison it makes
now runs on the whole-game crate with the real initialisation, and more (names, tooltips, the random draws). It is
kept in this stage only as the regression gate the brief names (`cargo test -p terraria-gen --test reference`);
recommendation: retire it and move its `gc` section into `sim.rs`.

**D. Float evaluation `jit32`** (DESIGN.md section 5; the whole game's mode). Collision families that are not
bit-exact, mismatching calls of 20,000:

| family | `x87` (core set) | `jit32` (whole game) | `jit32`, 2026-10-13 (mscorlib inlining) | `jit32`, 2026-10-14 (reference run's own code) |
|---|---:|---:|---:|---:|
| AimingLaserScan | 595 | 595 | 595 | 0 |
| CanHitLine | 16 | 16 | 0 | 0 |
| CheckAABBvLineCollision6 | 1,333 | 503 | 503 | 0 |
| CheckLinevLine | 1,019 | 859 | 859 | 0 |
| TileCollisionInStepsOf16 | 300 | 300 | 300 | 0 |
| the other 57 | 0 | 0 | 0 | 0 |
| total | 3,263 | 2,273 | 2,257 | 0 |

2026-10-14: all 62 families bit-exact (1,240,000 of 1,240,000 calls), light map unchanged (928 tiles, at most
4 ulp; `GetColor` exact). How: `tools\refdump_worldgen\bin\RefDumpCollisionJit.exe` is the collision harness itself
(its sources compiled in unchanged, `build_colljit.cmd`); it repeats the whole reference run (64 of 64 `.bin` files
byte-identical to `refdata\collision`, `colljit_compare.py`) and then reads the machine code of that process
(`work\jit\collref`). What it showed, against the earlier separate jitdump: `Vector2.Length` and `Utils.RotatedBy`
are CALLS in these families, not inlined (the earlier "inlined" reading came from unnamed call stubs). The four
families were made of three things, each read from that code and modelled against the 20,000 inputs before
changing the translator:
- XNA `Vector2.Length()`'s own body sums `X*X + Y*Y` at double precision (`fstp qword`), `fsqrt`, rounds once;
  the f32 `nativeMembers` template rounded each product and the sum (AABB6 503 -> 0, TileCollisionInStepsOf16,
  LaserScan inside AimingLaserScan). `Vector2.Normalize(Vector2)` keeps `1 / sqrt` unrounded (AimingLaserScan via
  the inlined `SafeNormalize`). Config `jitBodies` (the x87 bodies) at the sites of `jit_bodies.tsv`
  (`gen_jit_bodies.py`: caller / callee pairs of that dump where every IL site is a call instruction).
- `CheckLinevLine`: `num` is in memory (`fstp dword [ebp-4]`; the JIT's common subexpressions hold the registers):
  `floatInMemory`; `num4` is spilled as a float at the `newarr` helper call of `new Vector2[1] {...}`: new config
  `floatSpillAtAlloc` (859 -> 0; a model of exactly that code gives 0 of 6,485 decided cases, 859 without).

2026-10-14, the light map: raw `Vector3` bit-exact on all 175,900 tiles of the ten areas (928 tiles, at most 4 ulp,
before; `GetColor` exact before and after). How: `tools\refdump_worldgen\bin\RefDumpLightJit.exe <data> <jit>`
(`build_lightjit.cmd`, `LightJit.cs`) is `tools\refdump_light\RefDumpLight.cs` compiled in unchanged with another entry
point that sets its paths and calls its `Run`; data to `work\jit\lightref\data` (21 of 21 `.bin` files byte-identical
to `refdata\light`: `python tools\refdump_worldgen\colljit_compare.py work\jit\lightref\data refdata\light`), then the
code of `Lighting`, `LightingEngine`, `LightMap`, `TileLightScanner`, `LegacyLighting`, their callees (two levels) and
XNA `Vector2/3/4`, `Color` (`work\jit\lightref`; `jit_code.json` "compiled_before" marks the 118 of 624 methods that
ran). What that code does at the mismatching tiles, each checked before regenerating:
- hell (447) and edge_bottom (6): `Tile.slope()` is a CALL in `ApplyHellLight` (the size rule thought it inlined); on
  the path that reaches it (active tile with `tileNoSunLight`) num4 = `0.55f + sin * 0.08f` is spilled as a float, so
  `num2 = num4 * 0.6f` differs by 1 ulp there (modelled: 0.3236471 rounded vs 0.32364714). Rule: config `jitRefCalls`
  (`jit_refcalls.tsv` from that dump) + the rounding emitted at the call when the call is conditional. Same call in
  `ApplySurfaceLight` (num7); there num6 is in memory (`floatInMemory`, like num4 / num5). In the wall-93 case of
  `ApplyHellLight` the JIT folded `0.7f - 0.2f` to the float 0.5 (`floatInMemory` num7).
- scramble_surface / deep / hell (7 / 7 / 3): `num4 += (float)localRandom.Next(0, 50) * 0.0005f` (ApplyWallLight) and
  num13 in ApplyTileLight: `Next(int, int)` is inlined but calls `Next(int)`, num4 is spilled before it (C05 for `+=`).
- scramble_shimmer (458): XNA `Vector4.Lerp` (a call in `GetShimmerBaseColor`) keeps `(v2 - v1) * t + v1` unrounded;
  config `jitBodies` template, sites from `gen_jit_bodies.py` (now collref + lightref). Model with terraria-x87's fsin
  against `refdata\light\shimmer.bin`: 0 of 888 points differ with the x87 body, 104 with the f32 template.
Gates after regeneration: p1 3,911,380 exact, client-whole, rt 106, sim (collision 62 of 62 bit-exact, light map
175,900 of 175,900), ticks-player, ticks-world, host 109, app 102, viewer SHA-1 35b0971b..., scripts 14 of 14.

2026-10-14 (later), real-JIT facts from EVERY reference run. `sh tools/refdump_worldgen/run_refjit.sh` (build:
`build_refjit.cmd`) repeats the 20 processes that wrote the reference data, each with its harness sources compiled in
unchanged and an entry point that sets the harness's statics (data folder `work\jit\ref\<scenario>\data`, never
refdata), calls its `Run` and then reads the code of every TerrariaServer / ReLogic / XNA method that has jitted code
in that process (`JitDump.cs DumpRan`: compiled-ness and exact size from the CLR's code header, nothing compiled for
the dump; calls through a not-yet-compiled callee's MethodDesc slot named). Proof per run
(`python tools/refdump_worldgen/refjit_compare.py`, `work\jit\ref\compare.txt`):

| run | refdata files byte-identical | differing only in |
|---|---|---|
| p1 (`refdump`) | 12 of 12 | |
| framed, framed_scrambled (`RefDump.exe framed`) | 3 of 4 each | `ref_small*.json`: `frameRunMs`, `totalMs` |
| collision | 65 of 65 | |
| light | 21 of 22 | `MANIFEST.txt`: `elapsed_ms` |
| sky; liquid x6 | 1 of 1; 1 / 1 / 1 / 1 / 1 / 3 of 3 | |
| ticks_world, ticks_player | 6 of 7, 7 of 8 | `*.timing.json` (wall clock; README: the one file that differs between runs) |
| worldgen ref_small (`Gen.cs`) | 17 of 17 (passes, columns, 14 snapshots) | |
| matrix medium / large / bees / drunk / worthy (`GenX.cs`) | passes.jsonl + columns.bin, 2 of 2 each | `summary.json`: date, wall clock, peak memory, `.wld` hash / size; `.wld`: GUID, time stamps, pass durations (`wld_diff.py`: every data section identical) |

Methods that ran: 1,593 (p1) to 2,840 (ticks_player) per run, 4,256 in the union, 309 with a float or double local.
Tables (`gen_jit_refcalls.py`, `gen_jit_calls.py`, `gen_jit_bodies.py`, all from `work\jit\ref`):

| | before (collision + lighting dumps) | after (20 runs) |
|---|---:|---:|
| `jit_refcalls.tsv` callers / pairs | 10 / 58 | 296 / 4,612 |
| call sites decided by it in the translation | 172 | 10,632 |
| `jit_bodies.tsv` callers / pairs (all-call pairs) | 114 / 232 (72) | 506 / 1,148 (551) |
| XNA call sites evaluated by their x87 body (`jitBodies`) | 11 | 147 |
| `jit_calls.tsv` pairs | 2,985 (whole-game precompile) | 2,971 (reference runs for 180 callers, precompile for 875) |
| methods with different answers in two runs (left out) | | 3 (`jit_refcalls_conflicts.tsv`) |

12 generated files changed (367 lines; collision, liquid, main, npc, player, world_gen, utils, tile_light_scanner,
portal_helper, scene_state, aurora_sky, legacy_dungeon_hall). Float differences: none appeared, none disappeared in any
comparison with the final tables: collision 62 of 62 families bit-exact, light map 175,900 of 175,900 raw and GetColor,
ticks identical (600 / 3,600 ticks, 135 / 185 comparisons), worldgen 6 of 6 worlds at 108 of 108 rows, p1 3,911,380
values. On the way one did: a dump without the IL of always-inlined callees lost the "inlined but still calls"
answer (`FastRandom.Next(int, int)` in ApplyWallLight / ApplyTileLight, `Utils.NextFloat` in ApplySurfaceLight) and the
light map went to 3 / 7 / 1 differing tiles in scramble_surface / deep / hell, 1 - 2 ulp; DumpRan now writes the IL
callees of two levels and the answer is back. Note: the `sim` gate stays "ok" with raw light-map differences (it prints
them); the light map has to be read in `sim.txt`.

`float_mem_candidates.tsv` (`gen_float_candidates.py`, needs `CS2RS_FLOAT_REPORT=work/jit/float_alloc.tsv sh run.sh`):
98 of 223 compared methods have more 32-bit float frame slots in the reference runs' code than the translation rounds
(struct fields in memory, JIT temporaries and split IL slots included). A review list; no `floatInMemory` entry was
added, since no comparison needs one.

Gates after regeneration (`sh tools/cs2rs/gates.sh --slow`, all ok): p1 3,911,380 exact, client-whole, rt 106, sim
(collision 62 of 62, light map 175,900 of 175,900), ticks-player, ticks-world, host 109, app 102, viewer SHA-1
35b0971b..., scripts 14 of 14, worldgen ref_small / medium / large / bees / drunk / worthy 108 of 108 each,
multiplayer (A and B exit 0), window-modes (20 steps).

`jit32` loses nothing that `x87` had exact (`x87-locals` broke `HitLine`, `HitLineWall`, `LaserScan`), fixes the ten
`BannerStrength` values, and brings the raw light map from 22,240 differing tiles (18 ulp) to 928 (4 ulp). 2,789 float
locals are stack variables, 20 methods are "MinOpts" (all locals in memory), no method has an undetermined register
tie. NOT measured: the 77 probes of `refdata\worldgen\float_probes*` (the probe sources are not translated as
snippets yet), and what is still missing of the rule (float returns, parameters, struct fields, x87 `Math`).

**Proof 2 (world generation), 2026-10-11: the reference world is reproduced.** Seed text "12345", small world,
classic, random evil: the scenario of `refdata\worldgen` (the real game run headless by `tools\refdump_worldgen`).

`port\crates\terraria-gen-game\tests\worldgen.rs` performs the harness's call sequence with translated code only
(README of `refdata\worldgen`, section 2): the launch parameters before `Main.Initialize()`, the autogen block of
`WorldFile.LoadWorld`, `WorldGen.GenerateWorld` statement by statement, and the loop of
`WorldGenerator.GenerateWorld` / `RunPass` written out so that a record is taken where the harness takes it (after
`pass.Apply`, before `progress.End()` and `genRand.Next()`).

| compared | rows / bytes | identical |
|---|---|---|
| per row (after `clearWorld` + `Reset`, after each of the 106 passes, after `GenerateWorld`): FNV-1a 64 of the whole tile map (14-byte records), the hash of the 4,200 column hashes, active / wall / liquid tile counts, the value `Next()` would return for `Main.rand`, `worldSurface`, `rockLayer`, spawn and dungeon position, chest count | 108 rows | 108 of 108 |
| the saved file of `WorldGen.serverLoadWorldCallBack()` as ONE call (the server's own path: generate, `SaveNewWorld`, load) against `work\refsave\Worlds\ref_small.wld`, by `tools\refdump_worldgen\wld_diff.py`: sections Tiles (2,954,062 bytes), Chests (21,759), Signs, NPCs, TileEntities, WeightedPressurePlates, TownManager, Bestiary, CreativePowers, Footer | 10 sections | 10 of 10, byte for byte |
| the same file: FileFormatHeader, WorldHeader, manifest | | differ only where the reference README says a file must: the 11 section pointers (the manifest's durations have other lengths), GUID, creation time, last played, the manifest's `DurationMs`. Manifest `FinalHash` 4265538313 on both sides; `Name` / `RandNext` / `Hash` / `Skipped` identical for all 106 passes |

Run (unoptimised test build; the pass-by-pass run takes 167 s, the server path 57 s):

```
cd port && export CARGO_PROFILE_DEV_OPT_LEVEL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=target-gen/fast
WORLDGEN_REQUIRE=108 cargo test -p terraria-gen-game -j 4 --test worldgen -- --nocapture
WORLDGEN_REAL=1      cargo test -p terraria-gen-game -j 4 --test worldgen -- --nocapture
python ../tools/refdump_worldgen/wld_diff.py target-gen/fast/tmp/worldgen-save/Worlds/ref_small.wld ../work/refsave/Worlds/ref_small.wld
```

No pass diverged, so nothing of the float rule had to change for this world: the `jit32` mode as it stood after
Proof 3 (expression trees unrounded, the allocation rule for locals, `System.Math` through `terraria-x87`, the one
`floatInMemory` entry) reproduces all 106 passes. What was missing was not arithmetic:

| found on the way | cause | fix (in the tool, generally) |
|---|---|---|
| pass 1 (Dunes) stopped | `WorldGenRange` (pass configuration) has no parameterless constructor and a `[JsonConverter(typeof(StringEnumConverter))]` member | member tables call the one public constructor as Newtonsoft does (parameters by PARAMETER name, members set afterwards) and read / write `StringEnumConverter` enums |
| the server path stopped after the last pass | `string.Join<T>(string, IEnumerable<T>)` (`Trace.WriteLine` of the pass results) had no mapping | `rt::sys::join_seq_any` |
| manifest: `FinalHash` missing | a get-only public property was not written | member tables write every property with a public getter |
| manifest: the last pass's `Hash` (= `WorldGenerator.HashWorld()`) differed although the tiles were identical | `TileSnapshot.TileStruct` is `[StructLayout(LayoutKind.Explicit)]`: its `_i0` .. `_i2` overlay the other fields; the translator laid all fields out one after the other | explicit layout for structs of primitive fields: the fields that own their bytes at their offsets, the union members as pointer casts (`ProjectileKey`, `FloatIntUnion`, ReLogic's `ReinterpretCast` unions were wrong in the same way) |

Limits of this proof, stated: ONE seed and ONE size. The comparison covers what the table lists; the other statics
of the rows (`worldgen_static`, `genvars`) and the chest / sign / NPC content hash of each row are not compared
row by row (the saved file covers the final chests, signs, NPCs and tile entities). No second seed, no medium or
large world, no special seed and no hardmode conversion were generated on the real side: the harness
(`tools\refdump_worldgen\Gen.cs`) has the scenario's parameters built in, and a new mode for other seeds / sizes
was not written. A world of another size or seed can still meet a float decision this one did not (the rule's
open parts are listed in section D), and the client's path (`WorldGen.CreateNewWorld` from the menu) was not run.

**Proof 3 (per-tick simulation against the real server), 2026-10-11: both scenarios of `refdata\ticks` are
reproduced.** `port\crates\terraria-gen-game\tests\ticks.rs` performs the call sequence of the harness
(`tools\refdump_ticks\Ticks.cs`: `InitGame`, `WorldFile.GetAllMetadata(..).SetAsActive()`, `Main.Initialize()`,
`WorldGen.serverLoadWorldCallBack()` with the load thread's `Main.rand` pinned, the server state of
`Netplay.InitializeServer` without sockets, the pins of `Main.rand`, the two GUID seeds and the wind counters,
`Player.Spawn(SpawningIntoWorld)` and the control flags per tick) with translated code only and compares:

| scenario | compared | identical |
|---|---|---|
| player (600 ticks, one scripted remote player) | the player after every tick, 45 fields (position, velocity, old position / velocity, direction, `jump`, `wingTime`, `rocketTime`, `fallStart`, control flags, frames, speeds ...; floats by their bits) | 600 of 600 ticks |
| player | after each of the first 120 ticks: the value `Next()` would return for `Main.rand` and for every `Main._rngs` stream (a stream missing on one side counts); at the 15 record ticks: tile-map FNV-1a 64, active / wall / liquid tile counts, 22 time and weather statics, 13 liquid counters, entity counts, the NPCs' fields in the full rows | 135 of 135 comparisons |
| world (3,600 ticks, no player) | the same world comparisons (120 RNG rows, 65 record rows) | 185 of 185 comparisons |

Negative control of the player comparison: the state after tick t equals the RECORD OF TICK t + 1 in 201 of 599
ticks (the ticks in which the player stands still), so the comparison does distinguish ticks.

One difference was found on the way and removed: `Main.windSpeedCurrent` differed by one ulp at tick 1,560 of the
world scenario (and was equal again from tick 1,680 on, when the wind has reached its target). Cause: in
`Main.UpdateWeather` the real JIT keeps the local `num7` in memory (rounded to `float` at its store), the
allocation rule (FLOAT_RULE.md section 3, computed on the decompiler's locals) puts it on the x87 stack. Shown by
modelling the 60 wind steps from the tick-1,500 state both ways: rounded gives the server's
0.20025914907455444, unrounded 0.20025913417339325. The rule itself was not changed: the local is listed in
`game.json` `floatInMemory` with this evidence (a new config key: measured exceptions to the rule). The likely
reason is that the JIT allocates IL locals, and the C# compiler reuses an IL slot for several source variables,
so the slot has references and calls that the decompiled variable does not show; reading the allocation from the
IL instead of the decompiled source would be the general fix. Also changed on the way (FLOAT_RULE.md section 2,
no effect on this value): `Math.Abs(float)` inside an x87 expression is no longer rounded.

Not compared by this test: the 41 other `UnifiedRandom` instances the harness re-seeds by a reflection walk
(visuals and sounds), items and projectiles (none exist in either scenario), `worldgen_static` / `npc_static`
(246 statics), the per-column hashes (the whole-map hash is compared). The scenarios run 13 - 18 ms per tick in
the unoptimised test build, the comparisons included.

Run: `cargo test -p terraria-gen-game --test ticks -- --nocapture` (`TICKS_SCENARIO=world` for the second
scenario; `CARGO_TARGET_DIR=target-gen/fast CARGO_PROFILE_DEV_OPT_LEVEL=0 CARGO_PROFILE_DEV_DEBUG=0` to share the
build of `p1.sh`).

## Client start-up (2026-10-11): runs, compared with nothing yet

`port\crates\terraria-gen-game\tests\client.rs` (`sh tools/cs2rs/p1.sh --client-whole`): `Program.LaunchGame` /
`RunGame` (`src\host\launch.rs`), `Main.Initialize` with `LoadContent` inside, 900 ticks of `Main.Update` / `Main.Draw`
through the host's driver protocol with no GPU (the splash with asset loading, `Initialize_AlmostEverything` at its end,
the first-run language menu). It passes: no stop, no gap, no unimplemented `void` host member; two opaque objects are
created and never used (`TcpClient`, as in Proof 1). This is a "does it run" gate. Nothing is compared with the real
game here: there is no reference capture of the menus. Proof 1, `sim` and the snippets (44, with `yield`, anonymous
types and out-of-order named constructor arguments run on real .NET) still pass with `System.Math`'s transcendental
functions mapped to `terraria-x87` (collision: 1,237,727 of 1,240,000 exact, unchanged).

## Core set (`terraria-gen`): the earlier comparison, kept as the regression gate

| section | compared | exact | not exact | not compared |
|---|---:|---:|---:|---|
| `Item.SetDefaults`, ids 1..6195 | 737,205 values (119 members) | 737,205 | 0 | `ToolTip` (built from `Lang.GetTooltip`; `Lang` is not translated) |
| `NPC.SetDefaults`, ids -65..696 | 157,734 values (207 members) | 157,734 | 0 | `nextDialogue` (always null in the dump; its type has no reflection in the generator) |
| `Projectile.SetDefaults`, ids 1..1135 | 132,717 values (117 members) | 132,717 | 0 | `Name` (Lang); 5 members whose type has no reflection (`MinionSpawnInfo`, `WhipPointsForCollision`, `customHitbox`, `hostileDamageScaling`, `key`); 78 values of 6 projectiles that `SetDefaults` draws from `Main.rand` |
| `Collision`, 62 families x 20,000 calls | 1,240,000 calls | 1,236,737 | 3,263 in 5 families | - |
| `WorldGen.SectionTileFrame`, reference world | 5,040,000 tiles x 9 fields + next `Main.rand.Next()` | all | 0 | - |
| the same, scrambled region | 5,040,000 tiles x 9 fields + next `Main.rand.Next()` | all | 0 | - |
| `Lighting.GetColor`, ten areas | 175,900 tiles | 175,900 | 0 | - |
| raw light map (`LightingEngine.GetColor`), ten areas | 175,900 tiles x 3 floats | 153,660 tiles | 22,240 tiles, at most 18 ulp | - |
| collector | collection on the 5.3 million object heap, then the item comparison again with poison on | 737,205 | 0 | - |

Each entity comparison has a negative control (every object against the next id's record: 5,985 of 6,194 items,
761 of 761 NPCs, 1,134 of 1,134 projectiles differ), so a blind comparison would be noticed.

## Game state

What the harnesses did (`tools\refdump\RefDump.cs`): `Main.dedServ = true`, `new Main()`,
`Main.rand = new UnifiedRandom(0)`, `main.Initialize()`. The test performs the same with translated code, step by
step (34 steps, each listed with its source line in the report): the static initialisers of `Main`, `Player`,
`NPC`, `Item`, all `*ID.Sets`; `new Main()`; `Initialize_Entities`, `Initialize_TileAndNPCData1/2`,
`Initialize_Items`, the projectile loop, `TileObjectData.Initialize`, `Chest` / `Wiring` / `Framing` / `TorchID`
/ `Mount` / `Minecart` initialisers, `WorldGen.RandomizeBackgrounds`, `Recipe.SetupRecipeGroups` + `SetupRecipes`,
`ItemID.Sets.PostSetupContent`, `DyeInitializer.Load`, the three `SetDefaults` loops of `ContentSamples.Initialize`.

Hand set-up of the pilot that is gone: `ItemID.Sets.*` tables from `terraria-data`, `Main.projHook` from the
dump, `CanBeQuickusedOnGamepad` from the dump. Still supplied by the host (not game code): item and NPC names
(`refdata\lang_en.json`), localisation texts (the game's `en-US` json resources).

Not performed, and why (all are classes outside the translated set): `Lang.InitializeLegacyLocalization`,
`PlayerInput`, `CreativePowerManager`, the bestiary and item-drop databases, `ItemSorting`, `Netplay` and the
chat / network initialisers, the shops (`Chest.SetupShop` needs the bestiary and pylon systems), the name maps
of `ContentSamples` (`IdDictionary`: reflection over the id classes, stubbed null). Calling the translated
`Main.Initialize()` whole is possible (`CS2RS_WHOLE_INIT=1`) and stops at the first of these.

The world of the collision, lighting and framing sections is `work\refsave\Worlds\ref_small.wld`, parsed by
`terraria_formats::wld::read_world` and copied into the translated `Main.tile` by a 60-line adapter in the test
(`tests\suite\game.rs`). Decision: `WorldFile.LoadWorld` itself was not translated; it pulls in chests, signs,
NPCs, tile entities, the bestiary and creative powers through `BinaryReader`, none of which the comparisons need.
The liquid state after `LoadWorld`'s settle pass is taken from the dump, as the hand port's tests do. Every
comparison first proves the starting tiles equal the game's by the per-column hashes the harness wrote.

Values the config supplies for things outside the translated set (each with its reason in `core.json`):
`WorldFileData.GameMode = 0`, `Program.IsXna / SavePath / LaunchParameters`, `Netplay.ListenPort`,
`CaptureManager.IsCapturing = false`, `Environment.ProcessorCount = 1`, `Platform.IsWindows = true`, seeds taken
from the clock (`Guid.NewGuid`, `DateTime.Now.Ticks`: 0), graphics objects (null).

## Float evaluation: three rules against the game

The same generated code under the three values of `floatMode` (DESIGN.md section 5).

| | `f32` | `x87` (chosen) | `x87-locals` | hand port (`terraria-sim`) |
|---|---:|---:|---:|---:|
| NPC values not exact (of 157,734) | 301 | 0 | 0 | - |
| projectile values not exact (of 132,717) | 57 | 0 | 0 | - |
| collision families bit-exact (of 62) | 48 | 57 | 55 | 48 |
| collision calls not exact (of 1,240,000) | 7,537 | 3,263 | 3,267 | - |
| lighting areas with a bit-exact raw map (of 10) | not measured | 3 | 5 | 3 |
| `Lighting.GetColor` not exact (of 175,900) | not measured | 0 | 0 | 0 |

The `f32` mismatches of the entity dumps are all of one kind: `height = (int)((float)height * scale)` gives 36
where the game has 35, `Opacity => 1f - (float)alpha / 255f` differs in the last bit: the game used the
unrounded intermediate. Under `x87` all of them are exact.

Collision families that are not bit-exact (mismatching calls of 20,000; "nf" = of which something other than
float bits differs):

| family | `f32` | `x87` | `x87-locals` | hand port |
|---|---:|---:|---:|---:|
| AdvancedTileCollision | 5 | 0 | 0 | 5 |
| AimingLaserScan | 595 | 595 | 715 | 3,152 |
| AnyCollision | 8 | 0 | 0 | 8 |
| AnyCollisionWithSpecificTiles | 2 | 0 | 0 | 2 |
| ApplyConveyorBeltMovementToVelocity | 12 | 0 | 0 | 12 |
| CanHitLine | 16 (16 nf) | 16 (16 nf) | 0 | 16 (16 nf) |
| CheckAABBvLineCollision6 | 2,966 (2 nf) | 1,333 (2 nf) | 1,203 (1 nf) | 2,966 (2 nf) |
| CheckLinevLine | 3,581 | 1,019 (2 nf) | 859 | 3,581 |
| HitLine | 0 | 0 | 57 (57 nf) | 0 |
| HitLineWall | 0 | 0 | 21 (21 nf) | 0 |
| LaserScan | 0 | 0 | 112 | 0 |
| SlopeCollision | 3 | 0 | 0 | 3 |
| StepDown | 20 | 0 | 0 | 20 |
| StepUp | 2 | 0 | 0 | 2 |
| TileCollision | 4 | 0 | 0 | 4 |
| TileCollisionInStepsOf16 | 313 | 300 | 300 | 313 |
| noSlopeCollision | 10 | 0 | 0 | 10 |
| SwitchTiles | 0 | 0 | 0 | 0 (flags word not compared there) |

Reading:

- With `f32` the translation reproduces the hand port's numbers family by family (same count in 13 of the 14
  families): two independent ports of the same C# agree with each other and differ from the game in the same
  calls, which is what a systematic float-evaluation difference looks like. `SwitchTiles` is compared here with
  the flags word (the hand port skips it: its `Wiring` is not ported; the translated `Wiring.HitSwitch` runs).
- `x87` (expression trees unrounded, every store rounds) removes 9 families' differences completely and more than
  halves two others. It is the rule kept.
- `x87-locals` (unrounded `float` locals too) fixes `CanHitLine` and improves two families, but breaks three
  families that `x87` has exact, and its effect on the light map is better (5 areas exact against 3). So the JIT
  keeps SOME locals on the x87 stack and spills others; "all address-free locals" is wrong as a rule. Not adopted;
  the switch stays in the tool for the experiment (`cs2rs --float-mode x87-locals`).
- What is left under `x87` (5 families, 0.26% of the calls; the light map within 18 ulp) is therefore register
  allocation of the legacy JIT, per method. Closing it needs the JIT's decision for each local, not a better
  general rule. Candidates not modelled: a `float` return value stays unrounded in ST0; `(float)int`.
- `CheckLinevLine` has 2 calls under `x87` where a point count differs, and `CanHitLine` 16 where the boolean
  differs: decisions, not last-bit noise. These are the cases a simulation could diverge on.

## Diagnoses made on the way (fixed in the tool, not in the output)

| symptom | C# construct | fix |
|---|---|---|
| `ItemID.Sets` unusable | `SetFactory.CreateCustomSet<T>`: `typeof(T)`, `(T)object` | generic boxing / reflection on `rt::Trace` |
| `Utils.Clamp<T>` refused | constrained call `value.CompareTo(max)` | `cs_compare` |
| `Main` statics unusable | `new X()` of 40 untranslated UI classes | opaque objects for untranslated constructors |
| `TileObjectData.Initialize` panics | `(Rectangle[,])value.Clone()` | `Arr2::clone_arr`; receiver shown converted to `Array` |
| `WorldGen` events refused | field-like events have no field in Roslyn's member list | synthesized `name__event` fields |
| writes to `Netplay.ListenPort` do not compile | assignment to a stubbed field | recorded gap |
| `Projectile.Opacity`, NPC `height` / `life` off by one | x87 intermediates | `floatMode` |
| `cargo check` 8 minutes | 880 nested `else { if }` levels | flat chains |

## What these comparisons do not cover

`NPC.AI`, `Projectile.AI`, `Player.Update` and the rest of the per-tick simulation, world generation, `Liquid`
(translated, never run: the settle pass of `LoadWorld` would be the first test), `Chest` contents, `Wiring`
beyond what `SwitchTiles` reaches. They compile; nothing has executed them.

(The paragraph above is the state of the core-set stage. Proofs 2 and 3 and the sections below cover world
generation and the per-tick simulation since.)

## Proof 2 beyond one world (2026-10-12): five more worlds of the real game, all reproduced

`tools/refdump_worldgen/GenX.cs` (a third harness program, `RefDumpWorldgenX.exe genx`; the first one and its
files are untouched) takes seed text, size, game mode, evil and special-seed flags as arguments. Its row format is
the first harness's: rerunning the "12345" / small scenario through it gives `ref_small.passes.jsonl` and
`ref_small.columns.bin` byte for byte (`sh tools/refdump_worldgen/run_matrix.sh check`). The matrix is in
`refdata/worldgen/matrix/` (57 MB; README of `refdata/worldgen`, section 14, has where every argument goes in the
real server and why a special seed is a FLAG plus a plain seed: for a seed text that names an option the game
draws a random seed).

`tests/worldgen.rs` with `WORLDGEN_WORLD=<tag> WORLDGEN_REQUIRE=all` performs that harness's steps with translated
code (`main.autoCreate`, `SetWorldName`, `Main.GameMode`, `WorldGenerationOptions.TryEnablingFlagFrom`, the
autogen block with `AutoGenEnabled -> Enabled`, the pass loop) and compares every row as before.

| world | seed text | size | game mode | evil | flag | rows identical | first divergence | time (unoptimised test build) |
|---|---|---|---|---|---|---|---|---|
| `ref_small` (the reference scenario) | 12345 | small | classic | random (crimson) | - | 108 of 108 | none | 167 s |
| `ref_medium` | 2718281 | medium 6400 x 1800 | expert | corruption | - | 108 of 108 | none | 246 s |
| `ref_large` | 31415926 | large 8400 x 2400 | classic | random (crimson) | - | 108 of 108 | none | 479 s |
| `ref_bees` | 404 | small | classic | random (corruption) | `notthebees` | 108 of 108 | none | 131 s |
| `ref_drunk` | 5162 | small | classic | random | `drunk` | 108 of 108 | none | 128 s |
| `ref_worthy` | 1337 | small | master | crimson | `fortheworthy` | 108 of 108 | none | 162 s |

No pass diverged in any world, so nothing of the float rule had to change and no `floatInMemory` /
`floatOnStack` entry was added: `jit32` as it stood reproduces 648 rows over six worlds. What the rows compare is
unchanged (tile-map hash, column hashes, tile counts, the next random number, surface / rock layer, spawn, dungeon,
chest count); the saved FILES of the five new worlds were not diffed section by section (the references are in
`matrix/<tag>.wld`; `WORLDGEN_REAL=1 WORLDGEN_WORLD=<tag>` writes the port's). Not covered: secret seeds, copied
seeds, journey mode, the options `celebration`, `theconstant`, `notraps`, `remix`, `zenith`, `skyblock`, and any
world in which a pass is disabled. Slow gate: `sh tools/cs2rs/gates.sh --slow` (or `--only worldgen`).

**The JIT ground-truth tool exists but was not needed.** `RefDumpWorldgenX.exe jitdump <outdir> <listfile>`
(`JitDump.cs`) forces the real 32-bit JIT (`RuntimeHelpers.PrepareMethod`) and reads the native code of the listed
game methods from the entry point; `tools/refdump_worldgen/jit_floats.py` lists, per method, the frame slots that
receive x87 stores (float values the JIT keeps in memory) and the x87 register moves. Tried on
`Main.UpdateWeather` (the method with the one measured `floatInMemory` entry): 8 float IL locals, 13 float frame
slots written by `fstp dword`. Naming a slot (turning "ebp-0x24 is in memory" into `Type.Method:variable`) is NOT
automated: the JIT allocates IL locals, the config names decompiler variables.

## World creation from the menu (2026-10-12): the client's path writes the reference world

`crates/terraria-app/scripts/g15_create_world.input`, hidden, on an empty save root (`NO_WORLD=1 sh
crates/terraria-app/scripts/run_hidden.sh ... g15_create_world`): Single Player -> new player -> the empty world list
-> New -> `UIWorldCreation` (elements found by their own snap points: `aim snap size 0` ...): Small, Classic, evil
Random, typed name "ref_small", typed seed "12345" -> Create -> the loading screen (`UIWorldLoad`) while
`WorldGen.worldGenCallback` runs on a worker fiber (1,462 yields, 731 frames) -> the world list -> Play -> in
the world at the reference spawn (33518, 3670). Screenshots `work/shots/g15a..g15e*.png`: the progress texts
"Growing granite", "Growing vines", "Generating structures" with the bars advancing (looked at). Evil: Random
gives crimson, as in the reference run: `crimson = genRand.Next(2) == 0` (WorldGen.cs:11315) is decided by the
seed.

The file against `work/refsave/Worlds/ref_small.wld` (`tools/refdump_worldgen/wld_diff.py` byte-wise and
`tests/saved_world.rs` through `terraria_formats::wld::read_world`; evidence `work/item1/`):

| part | result |
|---|---|
| Tiles (2,954,062 bytes), Chests (21,759), Signs, NPCs, TileEntities, WeightedPressurePlates, TownManager, Bestiary, CreativePowers, Footer | byte-identical, all ten; 0 of 5,040,000 tiles differ |
| manifest | `FinalHash` 4265538313 on both sides; `Name` / `RandNext` / `Hash` / `Skipped` identical for all 106 passes; `DurationMs` differs (wall clock) |
| FileFormatHeader | the 11 section pointers (the manifest has another length) |
| WorldHeader | GUID (`Guid.NewGuid()`, WorldFile.cs:611), creation time (:607), last played: as between two runs of the real game. **And `caveBackStyle[2]`, `[3]`: 1, 2 here, 0, 0 in the reference** |

That last difference is the client's legitimate behaviour, not a defect. `WorldGen.RandomizeCaveBackgrounds`
(WorldGen.cs:8181) sets only entries 0 and 1 for a small world (8184-8195); entries 2 and 3 keep what the static
array held. The dedicated server applies `-autocreate 1` before `Main.Initialize` (which calls it at Main.cs:6791
with `maxTilesX` already 4200), so they stay 0. A client runs `Main.Initialize` with the default 8401 x 2401
(the large-world branch sets all four from `Main.rand`, seeded from the clock in the original), and generating a
small world later (`Reset`, WorldGen.cs:11349) leaves entries 2 and 3 as they were: in the real client they are two
random numbers 0..7 that differ from start to start. The README of `refdata/worldgen` (section 5) recorded the same
dependence for the real game. Everything else the client's path does differently from the server's autogen block
ends in the same file: the seed text goes through `UIWorldCreation.OnFinishedSettingSeed` (Trim,
`TryApplyingCopiedSeed`, `TrimUserString`, `GetOptionFromSeedText`, `CheckInputForSecretSeed`: none applies to
"12345") and `FinishCreatingWorld` (`WorldFile.CreateMetadata(name, cloud: false, Main.GameMode)`,
`SetSeed("12345")`, UIWorldCreation.cs:978-1002); the options are the page's (`WorldGen.SetWorldSize`,
`Main.GameMode`, `WorldGen.WorldGenParam_Evil = -1`); `WorldGen.CreateNewWorld` (WorldGen.cs:6582) passes a null
progress object and the page's controller, then `SaveNewWorld` without the server's load of the file.

Three host members were missing on that path and were added: `ICollection<T>.Clear()` on a dictionary held as
`IDictionary` (`WorldGenSnapshot._snapshotSizeCache`), `Trace.WriteLine(object)` (106 pass results),
`Directory.EnumerateDirectories` (`WorldGenSnapshot.DeleteSnapshotsForOtherWorlds`, queued by every generation).

## Number formats per culture (2026-10-12)

`rt::fmt` formats with the `NumberFormatInfo` of the thread's current culture, which the game sets to the selected
language's (LanguageManager.cs:97-98). Data: `tools/cs2rs/tests/framework/culture_numbers.tsv`, written by
`culture_numbers.ps1` from the real .NET Framework 4.8 on this machine for the twelve `GameCulture`s and the
invariant culture (de-DE / it-IT / es-ES / pt-BR "1.234,5"; fr-FR "1 234,5" with U+202F; ru-RU / pl-PL with
U+00A0; percent "12,5 %" in de-DE, fr-FR, es-ES and invariant, "12,5%" elsewhere; NaN is "не число" in ru-RU and
"非數值" in zh-Hant). Expected texts: `culture_fmt_probe.out`, 11,167 lines (13 cultures x 23 floating-point values as
`float` and `double` x 17 formats, 7 integers x 11 formats: default `ToString()`, `F0 F2 N N0 N2 P P0 P1`, the
custom patterns `0 00 0.0 0.00 #.## 0.0% #,##0.00 +0;-#`, `D D4 X2`); the unit test
`rt::fmt::tests::culture_formats_match_dotnet` replays every line: all identical. The earlier probe (5,212 lines,
en-US and invariant) still passes. Sites: every `x.ToString()` and string concatenation of a `float` / `double`
(`rt::to_str`), every `ToString(format)` and `string.Format` (the formats of DESIGN.md section 14), `Vector2` /
`Vector3` / `Vector4` / `Matrix.ToString()`. Dates were culture-aware before (host `sys::time`, 1,235 values
against .NET). Not culture-aware: string comparison and case mapping (still the en-US model), parsing
(`float.Parse` is only called by debug commands).

Language switch by script: `g16_language.input` (title -> Settings -> Language -> German, Russian, Japanese,
Simplified Chinese, Korean, back to the title in German and in Japanese). Screenshots `work/shots/g16a..g16k*.png`:
the German title menu ("Einzelspieler", "Mehrspieler", "Erfolge", "Ressourcenpakete", "Einstellungen",
"Verlassen"), the language list and the title menu in Japanese with kana, kanji, hangul and Cyrillic glyphs drawn
from the CJK font pages (looked at: complete glyphs, no boxes).

## Refused bodies (2026-10-12): 48 of 23,065, none reachable in this build

62 before. Translated now (14): `ZipContentSource.Refresh`, `ChromaEngine.Draw`,
`SecretSeedsTracker.SetstringsFromConfig`, `WorldGenSnapshot.EstimatedDiskUsage` (collection interface ->
`IEnumerable<T>`); `WindowsIme.PreFilterMessage`, `WindowService.SetUnicodeTitle`, `WindowsMessageHook` constructor
and `Dispose`, `WindowsMouseNotifier.PreFilterMessage` (IntPtr conversions); both `HotkeyCollection.GetEnumerator`;
`ParticleOrchestrator.MagnetFakeFish`; `UIAchievementListItem.DrawSelf` (+ `GetTrackerValues`, which was "not
emitted": `decimal`); `ResourcePack.LoadManifest`. The INPUT union of the IME code has its explicit layout.

What is left, with why it cannot run (callers from the IL: `port_calls` / `port_field_refs` of `db/terraria.sqlite`):

| refused body | callers in the IL | why it cannot be reached |
|---|---|---|
| `DeepClassCloning<T[]>` x 36 (9 array types x `CloneRef`, `RestoreRef`, `CloneChildren`, `RestoreChildren`), `LoggingILGenerator.Emit(OpCode)`, `ArrayComponent.Restore(Array, ..)` | `DeepCloning.Clone` from `Projectile.Damage` (Projectile.cs:12527) under `DebugOptions.DrawHitboxes`; `StateSnapshot*` from `RecordReplay` | `DebugOptions.DrawHitboxes` is written by its initialiser (`false`) and by `ToolkitDebugCommands.DrawHitboxes` only; that is a debug chat command, and `DebugOptions.enableDebugCommands` has ONE writer in the whole assembly: its static initialiser (`false`). `RecordReplay.Mode` leaves `None` only through the `/record` and `/replay` debug commands. So the Reflection.Emit deep cloner is dead code in this build. It was NOT generated (38 bodies) |
| `InternalContentManager..ctor(IServiceProvider)` | the lambda of `XnbReader..ctor` (a `ThreadLocal` factory) | the factory only runs in `XnbReader.FromStream<T>`, whose body is replaced by the host's XNB reader (`game.json` "bodies") |
| `ReLogic.OS.OSX.Clipboard.GetClipboard / SetClipboard` (`pbpaste` / `pbcopy` through `Process.StartInfo`) | `ReLogic.OS.Base.Clipboard.Value` | constructed by `OsxPlatform` only; the platform is the host's (`Platform.Get<IClipboard>()` is native, `Platform.IsOSX` false) |
| `SteamFileReadStream..ctor(string)` | `CloudSocialModule.OpenRead` | Steam cloud; the port starts in `SocialMode.None` (`SocialAPI.Cloud == null`) |
| `FxReader.FromStream<T>` x 4, `DummyPipelineContext` field initialisers | the asset reader registered for `.fx` (AssetInitializer.cs:26) | compiles effect SOURCE with the XNA content pipeline; `Content\` has no `.fx` file (0 of 15,994), and the pipeline assembly is neither among the game's references nor its embedded libraries (the types are unresolved in the decompile: that is the refusal reason) |
| `GitStatus.GitRevParse()` | `GitStatus.Init` | `Init` returns at once: `_init` is initialised `true` (GitStatus.cs:13, 26-29) |

Not emitted: 326 (203 generic methods and 4 generic types without an instantiation in the program, 110 `extern`
methods, 5 finalizers, 4 signatures with a type that has no Rust representation).

## Multiplayer over real sockets (2026-10-12)

The game's own network code (`Netplay`, `NetMessage`, `MessageBuffer`, `RemoteClient`, `Terraria.Net.Sockets.TcpSocket`)
is the translated code; underneath it the host has `TcpClient` / `TcpListener` / `NetworkStream` / `Socket` /
`UdpClient` / `IPAddress` / `Dns` on non-blocking OS sockets (`terraria-host/src/sys/net.rs`, design in `DESIGN.md`
and the host README). Every run below is hidden, muted, scripted, with `TERRARIA_NET_LOOPBACK_ONLY=1` (listeners on
127.0.0.1 only, no LAN announcement from the port).

### Port to port: `sh port/crates/terraria-app/scripts/run_multiplayer.sh`

Three processes of the `client` executable: A (`mp_host.input`: Multiplayer -> Host & Play -> new player "Hoster" ->
`ref_small` -> empty password), the dedicated server A starts through the game's `Process.Start`
(`Main.HostAndPlay`: this executable with `-server`, port 47800 from `TERRARIA_HOSTPLAY_PORT`), and B
(`mp_join.input`: Join via IP -> new player "Joiner" -> `127.0.0.1` -> `47800`; started when the server listens).
Result of the final run: **A exit 0, B exit 0 after 51 s, the server ended by itself**; no host member missing, no
gap, no stop in any of the three.

| step | evidence (`work\item4\`, copies of `work\logs\mp_a.*`, `mp_b.*`, `server-last-run.log`) |
|---|---|
| A in the world of its own server | A `dump net` tick 2060: `netMode 1`, `connectionState 10`, "Receiving tile data: Complete!"; server console `127.0.0.1:61399 is connecting...`, `Hoster has joined.` |
| B joins | server console `Joiner has joined.`; `dump players` on BOTH sides lists slot 0 "Hoster" and slot 1 "Joiner" at the spawn (33518, 3670) |
| B digs (copper pickaxe on the tile two right, two down) | B `dump tile` 2097,232: active type 2 (grass) -> inactive; **A's `dump tile` of the same tile: active -> inactive** (A's script waits for exactly that); `mp_b3_dug.png`, `mp_a4_tile_dug_by_b.png` |
| B walks right 3 s | **A's `dump players`: Joiner at x 33518 -> 33711 (velocity 3.70) -> 33904**; B's own dump at the end: 34048; `mp_a5_b_walked.png` shows the second character to the right |
| A chats "hello from A" | server console `<Hoster> hello from A`; **B's `dump chat`: `[n:Hoster] hello from A`** (B's script waits for it) |
| B leaves through Save & Exit | server console `Joiner has left.`; A's `dump players` is back to one player; B at the title, `netMode 0`, `Joiner.plr` written |
| A saves and exits | server console `Hoster has left.` / `Local player left. Autoshutdown starting.` / `Saving before exit...`; the world in `work\port-save-runs\mp_a\Worlds` rewritten (2,989,056 bytes, `.bak` = the original) |

Screenshots (`work\shots\mp_a0..a8*.png`, `mp_b0..b7*.png`): the dig and the walk were looked at; the others were
taken, not looked at.

Three earlier runs failed and say something about the port, not only about the scripts: a host run died when the
machine was under four other compilers (`wgpu`: "Out of Memory", the process quits: no recovery from a lost
device); typed `\b` does not delete (the game reads Backspace as a KEY: script fixed); sending the chat line needs the
typed character 13, not the Enter key state (script fixed; whether a real keyboard's Enter arrives as both was never
tried in a visible window).

Costs seen in these runs, not fixed:
- **The pause that ends a mark phase is long in a network session**: A 10 cycles, mean 44 ms, max 72.5 ms; B 4
  cycles, mean 22.7 ms, max 54 ms (single player: about 1 - 3 ms). A network thread is always suspended in a
  session, so every cycle scans its stack conservatively and resolves the candidates with a pass over the object
  list (1.5 passes per cycle over 2.5 million objects).
- The server process runs the collector in its idle time without a frame budget: longest slice 440 - 700 ms in
  the three runs (57 - 352 s long), 13 - 248 slices above 4 ms. Nobody measured what a client sees of that.

### The port's client against the ORIGINAL server: `sh port/crates/terraria-app/scripts/run_original_server.sh`

`work\refserver\TerrariaServer.exe -world <scratch copy of ref_small.wld> -port 47801 -ip 127.0.0.1 -password ""
-autoshutdown -noupnp -savedirectory <scratch>`, working directory `work\interop-scratch`, no window, killed by PID
if it overruns (it did not: it ended by itself). `netstat`: `TCP 127.0.0.1:47801 LISTENING`, no other listener.
**Not avoidable and it happened: the original announces itself on the LAN** (`Netplay.BroadcastThread`, Netplay.cs:786-
832: one UDP datagram per second to 255.255.255.255:8888 with the world name, the machine name and the port); the
server ran about 35 s. Nothing under `work\refserver` was written (12 files before and after, none newer).

Client: `mp_join_original.input`, **exit 0 after 29 s**. The server's console (`work\logs\mpo_server.log`), complete
after the world load:

```
Listening on port 47801
Server started
127.0.0.1:61413 is connecting...
Joiner has joined.
<Joiner> hello from the port
Joiner has left.
Local player left. Autoshutdown starting.
Saving before exit...
```

Client side (`work\logs\mpo_client.state.jsonl`): `connectionState` 3 ("Found server") -> 10, in the world as slot
0; tile 2097,232 active -> inactive under the pickaxe (the server did not reject the dig: a rejected one comes back);
position 33518 -> 34120 after 3 s of walking; the chat line comes back from the server as
`[n:Joiner] hello from the port`; back at the title with `netMode 0`. The handshake, the world transfer (compressed
tile sections), player / item / NPC sync, tile edits and chat of version 1.4.5.8 therefore interoperate in both
roles. NOT tried: a second (original) client in the same world, a password, a kick / ban, PvP, a boss, a long
session, a medium or large world, the original CLIENT against the port's server (no original client was run: it
needs a window).

The world the ORIGINAL server saved after that session (`work\interop-scratch\Worlds\ref_small.wld`), read by the
verified reader and compared with the world it loaded (`tests\saved_world.rs`, `EXPECT_TILES=2097,232`): passes;
`(2097,232) type 2 -> inactive` and the two tiles above it (type 187, the object standing on it) gone, everything else of the kinds a world changes
by itself (`work\item4\interop_world_check.txt`). The server accepted the port's tile edit.

## Collector: suspended work, large buffers, yield points (2026-10-12)

All numbers from this machine while other compilers were running on it; wall-clock pauses are inflated by that, the
CPU-time figures are the ones to read.

**Collection while work is suspended** (conservative scan of suspended fiber stacks; unit test with negative
control `a_collection_keeps_what_only_a_suspended_fiber_holds`). World creation from the menu
(`g15_create_world.input`, hidden `client` run; logs in `work\item5\`):

| | before (no mark phase can end while the generation is suspended) | after |
|---|---|---|
| collections that finished DURING the generation | 0 | 1 (tick 2926: 5.90 million objects, 169.2 MB) |
| heap after the first collection after the generation | 10,724,013 objects, 224.5 MB | 8,185,484 objects, 192.4 MB |
| peak | 11,006,231 objects, 243.3 MB | 9,325,573 objects, 258.2 MB |
| pauses that ended a mark phase | 4: mean 1.3 ms, max 2.8 ms | 5: p50 3.0 ms, **max 254.9 ms** (CPU time max 127.9 ms) |

So: fewer objects at the peak, NOT fewer bytes, and one pause of a quarter second during the loading screen (the
pass over 8 million objects that resolves the conservatively scanned words). The headless survey of the same
script: 12 collections freeing 89.7 MB with the scan, 11 freeing 73.0 MB without (`CLIENT_GC_FIBER_SCAN=0`). A world
generation keeps almost everything it allocates; the scan matters for multiplayer (no cycle could end there
without it), where it costs 23 - 72 ms per cycle (section above).

**Poison / `gc-check` on the incremental collector with the scan** (freed objects filled with `0xDD`, every handle
dereference checked; headless survey): `g7_play` with the verifier on top (`CLIENT_GC_VERIFY=1`): 19 collections,
10.6 million objects / 604.5 MB freed, no report, no stop (`work\item5\gccheck_g7_play.txt`); world creation from
the menu (one generation, collections finishing while it is suspended): 12 collections, 1.60 million objects /
90.2 MB freed, no report, no stop (`gccheck_g15.txt`).

**Yield points on / off**: the same script with `CLIENT_LOAD_SLICE` default (the generation gives the frame back
every 1,000,000 loop iterations) and `off` (it runs to completion in one call). The two `.wld` files: all ten data
sections byte-identical, `FinalHash` and every pass's `RandNext` / `Hash` identical; they differ in GUID, the two
times, the manifest's durations and `caveBackStyle[2..3]` (1, 6 against 7, 1: the two random numbers a client
leaves there, see "World creation from the menu") (`work\item5\yield_on_vs_off.txt`; the script prints "UNEXPECTED
DIFFERENCES" for those two header bytes because it only knows the server's file).

**Sweep overshoot / large buffers**: buffers of 256 KB and more are now freed on a helper thread. `g12_stability`
(10,000 ticks) on `client-opt`, `work\item5\g12_stability_client_opt_after.log`:

| | before (previous stage) | after |
|---|---|---|
| pauses | 2,769: mean 1.49, p50 1.50, p99 1.73, max 5.59 ms; 6 above 4 ms (all sweep slices) | 2,737: mean 1.59, p50 1.50, wall p99 4.74 / max 79.3 ms, 36 above 4 ms; **CPU time p99 1.69, max 63.3 ms, 8 above 4 ms** |
| by kind (wall) | mark < 2.1, start < 0.7, end < 2.2 ms | start max 2.6; mark max 79.3 (16 above 4 ms); end max 1.8; sweep max 11.4 ms (20 above 4 ms) |

**No improvement was measured.** In CPU time 8 pauses are above 4 ms against 6 before; one of them (63 ms, counted
as a mark slice) is new and is the conservative pass of the one cycle that ran while the world load was suspended.
The wall-clock figures cannot be compared with the earlier quiet-machine run. Whether the remaining sweep
overshoots are large frees at all was not established: the helper thread's counters are not in the log.

## Entities (2026-10-14): every NPC, projectile and usable item against the real server

Reference: `tools/refdump_entities` (`refdata/entities`, format in its README). Every NPC type (696, no skip: every
spawn call returned a slot), every projectile type (1,135) and every item with a use style (4,629 of 6,195), in slots
of 31 per batch on a scratch copy of ref_small.wld, one remote player per slot, 120 ticks of the entity part of
`Main.DoUpdateInWorld`; per tick every leaf of every active NPC / projectile / player / world item (195 / 109 / 1,180 /
32 primitive fields, flat structs and array hashes) and the state of `Main.rand` and every `Main._rngs` stream.
`RefJitEntities.exe` (run_refjit.sh scenarios entities_npc / entities_proj / entities_item) writes byte-identical files
and dumps its machine code. Port: `tests/entities.rs` (gate `entities`, `gates.sh --slow`); negative control
`ENTITIES_PERTURB=1` (another seed, players facing left): every batch differs from tick 0 or 1.

| | types | bit-exact | differ |
|---|---:|---:|---:|
| NPC | 696 | 696 | 0 |
| projectile | 1,135 | 1,135 | 0 |
| item (use style) | 4,629 | 4,629 | 0 |

First run (before any fix): NPC 637, projectiles 1,028 exact. What closed the gap, all found from the reference runs'
own code, none in a generated file (FLOAT_RULE.md section 10):

| fix | where | types it fixed (examples) |
|---|---|---|
| JIT facts from all 23 runs; conflicts per (caller, callee) pair, not per caller | jit_calls.py `union` | NPC 651, proj 1,038 |
| x87 bodies of Vector2.Lerp, Vector3.Lerp, MathHelper.Lerp / SmoothStep, Vector2 / float, Distance, DistanceSquared, Dot, LengthSquared, Normalize(), CatmullRom, Reflect; unrounded "f64:" results; also at inlined sites for vector / f64 bodies | game.json `jitBodies`, Emit.cs | rockets (338-341, 803-810), 578, 818, golf balls 745-752 |
| `Math.Abs(float)` as a call rounds its argument (MinOpts methods, proven calls) | Emit.cs `AbsIsCall` | 243, 471 |
| `x op= tree` on an array element / property: one x87 expression | Emit.cs `Compound` | falling stars 9, 723-726, 1133 |
| `0f - x` is `neg` (sign of zero); XNA vector negation is `fchs` | Emit.cs `DecompiledNeg`, terraria-core `negate` | 326, 400, 601, 1109 |
| float methods return unrounded values (`__x87` twins: 227 methods, called unrounded at 2768 sites; getters in `op=`) | Model.TwinF64, Gen.cs, Emit.cs | 644, 658, 961 |
| the JIT's own float allocation from the CLR debug info, IL locals named through ILSpy's PDB; evaluation-stack values unrounded | gen_jit_float_alloc.py, FloatAlloc.cs (`jitFloatAlloc`) | 356, 653 (AI_065), AI_005 family 139 / 173 / 210 / 234 / 619 |
| a declaration is a write in the liveness (spills never before a declaration) | FloatAlloc.cs, Emit.cs | (compile error in Main.rgbToHsl once the facts applied) |

The last five (2026-10-14, late; `tools/cs2rs/entities_allow.txt` is empty now and the gate checks it both ways), each
found in the reference run's own machine code (`work/jit/ref/entities_proj`, `entities_npc`; `jit_disasm.py`), each
closed by a rule for inlined XNA `Vector2` operators (FLOAT_RULE.md section 6, "Inlined XNA Vector2 operators"):

| type | first difference | site | rule |
|---|---|---|---|
| proj 741, 742, 744 (golf balls) | localAI[0] / velocity, ticks 19, 23, 103 | `GolfHelper.ContactListener.OnCollision`: `collision.Normal * Vector2.Dot(velocity, collision.Normal)` with op_Multiply inlined: `fmul st(1)` on the call's result | a float argument of an inlined operator that is a call result, local, field or parameter is not rounded (`VecScalar`) |
| proj 797 | the world item it drops, velocity.X at tick 71 | `WorldItem.TryCombiningIntoNearbyItems`: `Vector2.Lerp(worldItem.velocity, velocity, amount)` inlined twice, `amount` never stored | the same for the float argument of an inlined XNA body (`jitBodies` template) |
| NPC 426 | its projectile 581, velocity.Y at tick 95 (the shot's velocity) | `Utils.FactorAcceleration` (called from `NPC.AI_003_Fighters`): the operator chain and the Vector2 local `vector` on the x87 stack, one store per component of the result | per method, from its code: operator results on the x87 stack ("stack", `jit_vec_temps.tsv`) or a zeroed frame temporary per operator ("memory"); promoted Vector2 locals of a "stack" method keep their components until a call / control flow (`VecX87`, `VecTrackable`) |
| (proj 1047) | velocity.Y at tick 26, after the rule above alone | `FactorAcceleration` INLINED into `Projectile.RainbowBoulder_JumpToPosition`: there its divisions and subtraction are calls | the operators of an inlined game method per caller (`jit_vec_ctx.tsv` from the caller's IL-to-native map); the call site calls a twin `factor_acceleration__vc0` |

Bisecting inside a tick: `ENTITIES_TRACE_STEPS=1` with `ENTITIES_TRACE=<batch>:<class>:<index>` prints the traced entity
after each part of the tick (players, NPCs, projectiles, items): projectile 797's world item is created during the
projectile part with velocity (-1.4, -3.0) and gets its first difference in the item part.

Generators: `tools/cs2rs/gen_jit_vec_temps.py`, `gen_jit_vec_ctx.py` (with the 23 runs: 173 methods, 60 stack / 113
memory, 24 inlined call sites; with the client run below added: 233 methods, 90 / 143, 29 sites). Translator report
(GAME_REPORT.md, now): 823 inlined operator sites, 265 with results on the x87 stack, 27 call sites to `__vcN` twins. Virtual calls returning float: none of the game's 24 virtual float members ran in any
reference process (FLOAT_RULE.md section 10, item 9); not changed.

Items run server-side here: ItemCheck of a remote player (use animation, rotation, location, hold / use styles).
Shooting, placing and consuming happen only on the owning client: section "Client-side item use" below.

Every other gate unchanged by these rules (`gates.sh --slow --skip window-modes`): p1 3,911,380 exact, collision 62 of
62, light map 175,900 of 175,900 raw, ticks player / world identical, scripts 14 of 14, worldgen 6 of 6 at 108 of 108,
multiplayer.

## Client-side item use (2026-10-14, late): every usable item used by the owning client

Reference: `RefDumpEntities.exe citem` (`tools/refdump_entities/ClientItems.cs`, format in `refdata/entities/README.md`).
Every item type with a use style (4,629), one per batch, used for 60 ticks by the OWNING player of a single player game
(`Main.myPlayer = 0`, `Main.netMode = 0`) holding the left button (`PlayerInput.Triggers.Current.MouseLeft`,
`Main.mouseLeft`) on a fixed target: pick / axe / hammer tools the ground two tiles right of the spawn tile, anything else
the air tile three right and one up. The player: 60 of 100 life, 100 of 200 mana, the item in slot 0 at its max stack,
18 kinds of ammo (arrows, bullets, rockets, seeds, darts, gel, fallen stars, sand, stakes, nails, snowballs, flares,
solution, candy corn, explosive jack 'o lanterns, stynger bolts, silver coins, bones) in slots 10 ... So the client's own
item code runs: `ItemCheck_OwnerOnlyCode`, `ItemCheck_Shoot`, `PlaceThing`, tile digging, potions / buffs, mana and
ammo use. The same five parts of `Main.DoUpdateInWorld` as the other kinds; per tick everything of the other kinds plus
the tiles of the box around the player (columns -30 .. +30, rows -45 .. +30) that differ from the loaded world.

What the server build lacks, set up identically on both sides (each measured as a stop in the first runs): the game
view (`Main.GameViewMatrix`: a `SpriteViewMatrix` without a device, viewport 1600 x 900; `Camera.ScaledSize` in
`ItemCheck_PlayInstruments`), the achievements (`AchievementInitializer.Load`; `AchievementsHelper.HandleRunning`), the
lighting engine (`Lighting.Initialize`; `Lighting.AddLight`), a stand-in font for the text measurements of
`CombatText` / `PopupText` (`FakeFonts`, the texts are not dumped). The player is spawned with `Main.myPlayer = 255`
meanwhile (the local part of `Spawn` moves the camera and clears the lighting engine), and starts with `releaseUseItem
= true` (the button was up before tick 1; ItemCheck starts a use on `controlUseItem && releaseUseItem`).
`Main.ignoreErrors = false`: an exception in an update is not swallowed (it is `true` by default, and the other
kinds' updates run with it). 5 of the 4,629 batches end with an exception in the real run, at a texture the server
build does not have (`Main.LoadProjectile` for whips 4680 / 6155 / 6173, `Gore.NewGore` reading
`TextureAssets.Gore[1225]` for 1450, `Projectile.AI` for 3541); the port throws at the same tick, which the test counts
as a match and reports.

`RefJitEntities.exe` scenario `entities_citem` writes byte-identical files (refjit_compare.py) and dumped the code of
the 3,484 game methods that ran (24 runs now). Facts regenerated from all 24 (`gen_jit_refcalls.py`, `gen_jit_calls.py`,
`gen_jit_bodies.py`, `gen_jit_float_alloc.py`, `gen_jit_vec_temps.py`, `gen_jit_vec_ctx.py`): call sites decided by the
reference runs' code 10,632 -> 30,348; XNA sites with their x87 body 147 -> 1,896; methods with the JIT's own float
allocation 389 -> 447.

| | types | first run (23-run facts) | after the facts of this run | after the rule below |
|---|---:|---:|---:|---:|
| item, owning client (citem) | 4,629 | 277 of the first 300 | 4,627 | 4,629 |

What it showed (FLOAT_RULE.md section 10, item 10): a float CALL RESULT that waits while another call runs is spilled as
a float (`fstp dword`), unlike an arithmetic temporary (`fstp qword`): `Utils.GetLerpValue(0.1f, 0.7f, t3, true) *
Utils.GetLerpValue(0.9f, 0.7f, t3, true) > 0.5f` in `Projectile.AI_165_Whip` (whips 5479 / 5480: one `Main.rand` draw
more here at tick 12 before the rule; all 11 such pairs in the method have `fstp dword [ebp-0xa4c]`). Translator:
Emit.cs `F64Pending`.

Port: `tests/entities.rs` with `ENTITIES_KIND=citem`, gate `client-items` (`gates.sh --slow`), the allow file of the
other kinds. The collector runs between batches (`rt::collect` every 25 batches, Main rooted): without it the test
held every object of all batches (about 9 MB per batch) and ran out of memory near the end. Negative control
`ENTITIES_PERTURB=1`: all 40 first batches differ. Run alone a batch can differ (`ENTITIES_ONLY`): state the reset does
not restore (world events started by an earlier item, achievements, `Main.SceneMetrics` zones) carries from batch to
batch, the same on both sides; the gate runs them in order. Real run 8 minutes, the gate 7.

## Draw output (2026-10-14, night): what the client DRAWS, call by call against the original client

Until now the client's drawing was only checked inside the port (the viewer screenshot, the scripts). This compares
the port's draw output with the original's at the XNA API boundary, for fixed scenes, every call bit for bit.

Reference: `tools\refdump_draw\RefDumpDraw.cs` runs the real `Terraria.exe` (a copy in `work\refclient`; the install is
only read, for Content) in-process on a recording XNA stand-in: `Microsoft.Xna.Framework.Graphics.dll` /
`.Game.dll` with XNA's identity (4.0.0.0, token 842cf8be1de50553; delay-signed with XNA's public key, then marked
signed, `fakesign.py`, so the strong-name bypass loads them), their public surface generated from the real assemblies
(`gen\XnaStubGen.cs`, throwing stubs) and the members the game uses written by hand (`xna\Graphics`, `xna\Game`,
`//@impl` markers). No graphics device: every call is recorded (`xna\Graphics\Rec.cs`). The real
`Microsoft.Xna.Framework.dll` (maths, content pipeline, input, audio) and Xact run as they are. The game's form is a
hidden WinForms window on a desktop of its own (`CreateDesktop`, never switched to), checked after every frame (a
visible or foreground window ends the run). Port: `terraria_host::gfx::trace` (sprite batch, device, effects, the same
line format), test `crates\terraria-gen-game\tests\draw.rs`, which repeats the scenario step for step.

Recorded per frame: SpriteBatch Begin (overload, sort mode, blend / sampler / depth / rasterizer state, effect,
matrix) / Draw (overload, texture, position or destination, source rectangle, colour, rotation, origin, scale,
effects, depth) / End; device render target switches, Clear, viewport, scissor, the four state objects, sampler and
texture bindings, vertex / index buffer bindings and data, DrawPrimitives, DrawIndexedPrimitives,
DrawUserIndexedPrimitives, Present; texture creation / SetData / Dispose, render target creation; effect technique,
parameter values and Apply. Floats as 8 hex digits (bit patterns), colours as packed values, buffer and pixel data as
FNV-1a 64 (`DRAW_RAW=1` writes the bytes), textures by their asset name (font pages `<font>#k`), others numbered in
creation order. Each frame also gets a digest (line count + FNV-1a 64 of its lines): `run.digest` covers all 730
frames of the run, the recorded scenes 3 frames each.

Pinned on both sides (each one found as a difference first): input idle (mouse at 640,360, no keys, no gamepad;
`Mouse.SetPosition` does nothing), `Environment.TickCount` (the seed of every `new UnifiedRandom()`), `DateTime.Now` /
`UtcNow` = 2026-06-15 12:00 (no seasonal event), the game thread's Stopwatch clock (map sections are drawn for 5 ms per
frame; `TimeLogger`), `Guid.NewGuid` = Guid.Empty on the game thread (the stars' `FastRandom.CreateWithRandomSeed`),
the asset loader thread served on the game thread, 64 actions after every Update (the port's `dispatcher_budget`),
`config.json` (1280 x 720 windowed, UI scale 1, zoom 1, language en-US, `VolumeMusic` / `VolumeSound` /
`VolumeAmbient` 0, RGB devices off), `Program.SkipAssemblyLoad` with the start-up force-load of every static
initialiser run synchronously before the first frame (the JIT's eager `beforefieldinit` initialisation order would
otherwise differ from the port's lazy one: `CaptureManager`), `FastParallel.ForceTasksOnCallingThread` (the original's
parallel lighting is not deterministic: two runs differ by 1 in a water colour), the driver protocol of
`terraria-host` (window 1280 x 720, active, display 1920 x 1080, 1/60 s per tick).

Scenes (fixed camera through the player position; world `ref_small`):

| scene | frames | what | calls | exact |
|---|---|---|---:|---:|
| boot | 1-3 | first frames (splash) | 34 | 34 |
| title | 301-303 | title menu | 2,124 | 2,124 |
| charcreate | 334-336 | character creation (`UICharacterCreation`) | 4,804 | 4,804 |
| world_day | 397-399 | in the world at the spawn, noon | 2,584 | 2,584 |
| world_night | 460-462 | the same at midnight (moon, stars) | 3,090 | 3,090 |
| ocean | 523-525 | sea floor at the left edge (water, the surface) | 2,468 | 2,468 |
| cave | 586-588 | below the rock layer (dark, lighting engine) | 2,213 | 2,213 |
| underworld | 649-651 | lava and its light | 9,553 | 9,553 |
| boss | 702-704 | night at the spawn, Eye of Cthulhu summoned, six projectiles (arrow, bullet, ball of fire, water bolt, magic missile, laser) | 5,228 | 5,228 |
| inventory | 715-717 | `Player.ToggleInv` | 6,007 | 6,007 |
| map | 728-730 | full-screen map (`TryOpeningFullscreenMap`); the minimap is in every world scene | 1,503 | 1,503 |
| all | | | 39,608 | 39,608 |

Digest: all 730 frames equal. Two reference runs and the JIT twin (`RefJitDraw.exe`) write byte-identical files.

What it found, each fixed where it came from (no generated file edited):
1. The generated `new UnifiedRandom()` read its own clock instead of the host's `Environment.TickCount`
   (`terraria-gen-game\src\host\mod.rs` now delegates to `terraria_host::sys::time`).
2. Float differences in drawing code (methods that never ran in a server reference run): the draw run's JIT dump
   (`work\jit\client\draw`) fills the fact tables for methods no reference run ran (`jit_calls.Dirs.fill`; the server
   runs keep precedence where both ran, because the client process compiles in another order).
3. `VertexStrip.AddVertexPair` (the magic missile's trail, 1-2 ulp): `pos + vector` / `pos - vector` where `vector`
   is `WrapAngle(..).ToRotationVector2() * num` with op_Multiply inlined: both products stay unrounded on the x87
   stack across the argument evaluation. Translator (Emit.cs `VecCallsOnlyFirst`, `VecArgsBeforeCall`): the
   components of a Vector2 local are tracked when the chain's calls all come before its first operator and when the
   statement after it is one call whose arguments read them.
4. `Dust.GetAlpha` (lava dust, red channel 0xae against 0xad at frame 647): `Color.get_R` is a call instruction at 2
   of its 14 sites in that method and inlined at the others; the translator had no per-site answer and treated all 14
   as calls, rounding `num` before `(float)newColor.R * num`, where the JIT keeps it on the x87 stack. New fact table
   `jit_refcall_sites.tsv` (`gen_jit_refcalls.py`): for every (caller, callee) pair with calls at some sites only,
   each call instruction is mapped to its IL site through the CLR's IL / native map (`jit_debug.bin`) and each IL site
   to its decompiled statement through ILSpy's PDB (`tmeta pdbseq` -> `work\pdb\pdb_seq.tsv`, placed in
   `decompile\` by Roslyn: the PDB was written for a layout with one folder per namespace and a one-line shift);
   FloatAlloc.cs `SiteIsCall`. 384 of 391 mixed pairs placed, 2,243 sites decided in the game.
5. Nondeterminism in the port (about 3 runs in 5 differed after a teleport: tin ore drawn from the Tiles_1
   texture): `TileBatch.LayerBatchKey.Equals` compares only the layer, its `GetHashCode` mixes in the texture.
   .NET's `Dictionary` / `HashSet` / `Distinct` match an entry only when the stored hash codes are equal and then
   `Equals`; the runtime's dictionaries called `Equals` for every entry whose short hash-map tag matched (a random
   seed per process). Fixed in `terraria-gen-rt` (`collections.rs`, `KeyW`: hash codes first). 5 of 5 runs equal
   since.

Re-run: `sh tools/refdump_draw/run_draw.sh [--build] [--refdata]` (the JIT twin, compared with `refdata\draw`; the
previous dump is moved to `work\_archive\jit_client`). Gate `draw` (`gates.sh`, about 25 s): every scene exact and
every frame's digest equal. Diagnosis switches of the test: `DRAW_REF=<dir>`, `DRAW_SCENES=a,b`,
`DRAW_EXTRA=<first>,<count>` (those frames' lines to `extra.trace`, the harness the same), `DRAW_RAW=1`.

Not covered: what the GPU makes of the calls (shaders, blending: the port's renderer is checked by the viewer gate
and screenshots); audio and more scenes (menus, biomes, events, UI screens, bosses): the two sections below. Before 2026-10-15
not covered: audio, the world-creation and multiplayer menus, scenes other than those above.

## Audio output (2026-10-15): every call the game's audio makes, against the original client

Reference: `tools\refdump_draw\AudioRec.cs`, in the draw harness (`RefDumpDraw.exe <dir> audio`, refdata\audio). It
records at the boundary between XNA's managed audio classes and their native engine: XNA 4's SoundEffect /
SoundEffectInstance / DynamicSoundEffectInstance do their checks and bookkeeping in managed code and call
`SoundEffectUnsafeNativeMethods.*` (Microsoft.Xna.Framework.dll); the XACT classes call `UnsafeNativeMethods.{Engine,
SoundBank, WaveBank, Cue}.*` (Microsoft.Xna.Framework.Xact.dll). Those static methods are replaced (Detour.cs) by a
deterministic engine model that writes one line per call; XNA's own managed code still runs (range checks,
exceptions, `isPacketSubmitted`, the State mapping, the Cue status bits). No XAudio2 voice and no XACT engine exist, so
nothing reaches the speakers. The draw run installs the same detours (refdata\draw is byte-identical with them).

Derived parameters from XNA's real kernel: SetVolume / SetPitch / SetPan run the kernel functions of the mixed-mode
assembly (`<Module>.Microsoft.Xna.Framework.Audio.KernelSoundEffectInstance.SetVolume / SetPitch / SetPan`, IL) on a
stand-in kernel object whose IXAudio2SourceVoice is a fake COM object; what the kernel hands XAudio2 (voice volume,
output matrix, frequency ratio) is written on the line. The kernel showed that the port's pan law (FNA's) was wrong
for stereo sources: XNA cross-feeds (pan <= 0: [1 + pan/2, -pan/2, 0, 1 + pan]; pan > 0: [1 - pan, 0, pan/2,
1 - pan/2], `0 * -0.5f` keeping its sign) and computes the ratio as `(float)Math.Pow(2, pitch)`. terraria-host
`audio::mixer::output_matrix` / `frequency_ratio` are those functions now, and the mixer mixes with them.

The engine model (the same rules on both sides; AudioRec.cs and terraria-host `audio::engine`): the kernel's voice
states (Playing 1, Stopping 2, Stopped 4, Paused 8; Stop(immediate) with a queued buffer is Stopping until the next
frame), time in exact integers (a non-looped buffer ends when sum(ratio * 2^24 * ticks * rate) reaches frames * 10^7 *
2^24; frames = whole blocks, MS ADPCM samples per block), XACT cue status bits (Prepared 4, Playing 8, Paused 0x40,
Stopped 0x20; a cue plays once), a wave bank prepared at once, 2 output channels. A cue does not end by itself in the
model; the port's cues do (their wave runs out), and the gate requires that none did.

Port: terraria-host `audio.rs` follows XNA's managed code over `audio::engine` (the native layer, which writes
`audio::trace` and the mixer commands). Lines: `se.create <asset> <format bytes>:<fnv> <data bytes>:<fnv> <frames>`,
`se.master`, `si.create si<k> <asset>`, `si.volume si<k> <f> v=<f>`, `si.pitch si<k> <f> r=<f>`, `si.pan si<k> <f>
m=<matrix>`, `si.submit`, `si.play / stop / pause <state>`, `si.state`, `xe.create / update`, `xs.getcue <name> xc<k>`,
`xc.play / stop / pause / setvar / status`, ... (table in AudioRec.cs). Mapping changes (terraria-host
tools/coverage.py rules): `SoundEffectInstance.Stop(bool)` and `DynamicSoundEffectInstance.PendingBufferCount` were
unimplemented (ActiveSound stops its looped sounds with Stop(true)); the AudioEngine / streaming WaveBank constructors
keep their look-ahead / offset / packet size.

The audio run: the draw run's scenario with config.json volumes music 0.75, sound 1, ambience 0.5; after the title
scene the cursor over three menu items and a click on Settings; after the map scene the audio scene: noon at the
spawn, walking right on the grass (D held), mining with an iron pickaxe (hotbar 2, the button held on the ground),
an iron broadsword swung at a blue slime (hit and killed), a lesser healing potion, then night, space, the ocean
and the underworld (music cues fading in and out). Input is scripted through XNA's Mouse / Keyboard (the detours)
and the port's input snapshot.

| | original | port |
|---|---:|---:|
| audio calls, all 1,779 frames | 748,270 | 748,270 exact |
| per-frame audio digest | 1,778 frames | all equal |
| draw scenes of the audio run (12) | 47,748 calls | 47,748 exact; digest of all 1,778 frames equal |
| XACT cues that ran out | (none in the model) | 0 |

First differences, each fixed at its root: the port labelled content sound effects "XnaAsset" (now the file the
content stream reads); the pan law and ratio (above); a cloud's red channel 1 off in the audio scene (FLOAT_RULE.md
section 10, item 13).

Mixer against a model of XAudio2's mixing (`tests/draw.rs` `mixer_check`, in the audio gate): for every distinct
voice start of the reference trace (sound, Volume, Pan, Pitch, IsLooped), 2,048 frames rendered by the port's mixer
(which derives its own matrix and ratio) and by a model fed the kernel's values from the trace (per source channel:
linear interpolation at n * ratio * rate / 44,100 times the voice volume, then the matrix), compared per sample.
Result: 55 voice starts, 36 distinct parameter sets, 119,395 of 147,456 samples bit-identical, largest difference
1.19e-7 of full scale (the two sum in different orders); the gate fails above 1e-6.
Not modelled: XAudio2's sample-rate converter (a filter, not public; linear interpolation on both sides) and its
10 ms quantum (1,024-frame blocks here), the XACT engine's own volume curve evaluation (the port's xact.rs; only the
SetVariable values are compared), DynamicSoundEffectInstance (resource-pack music; not reached by the scenario).
Re-run: `sh tools/refdump_draw/run_draw.sh --audio [--refdata]`; gate `audio` (`gates.sh`, about 2 minutes).

## The scenes run and float-rule coverage (2026-10-15)

Coverage measure (`tools/cs2rs/float_coverage.py`, report `FLOAT_COVERAGE.md`, list `float_uncovered.tsv`): a game
method does float arithmetic when its IL (from work\refclient\Terraria.exe) has a float or double value (ldc.r4/r8,
conv.r4/r8, a float / double / Vector field read, such a parameter, a call returning one or an XNA vector / MathHelper
member) and an arithmetic instruction; it is covered when a reference process's JIT dump (work\jit\ref\*,
work\jit\client\*) has the same method, IL equal with tokens zeroed.

| | methods | IL bytes | covered by a dump | IL bytes covered |
|---|---:|---:|---:|---:|
| before (24 server runs + draw run) | 3,742 | 5,547,702 | 1,860 | 4,557,978 (82.2 %) |
| + audio run | | | 1,883 | 4,588,718 |
| + scenes run, first round | | | 2,072 | 4,741,931 |
| + scenes run, second round (now) | | | 2,094 (56.0 %) | 4,803,334 (86.6 %) |

Methods inlined everywhere are never compiled on their own and so never "covered" (Utils.Lerp, 8 bytes, 31 call
sites), although their code runs inside covered callers.

The scenes run (`RefDumpDraw.exe <dir> scenes`, refdata\scenes; port `tests/draw.rs` with `DRAW_MODE=scenes`, gate
`scenes`): menus (multiplayer, character list, world list, world creation), nine biomes of ref_small (crimson, jungle,
snow, desert, mushroom, dungeon, temple, marble, granite; `REFDRAW_FINDBIOMES=1`), rain, blood moon, eclipse, slime
rain, the goblin army, a sandstorm, the four settings pages, crafting, housing, NPC chat (a guide) and shop (a
merchant), the bestiary, emotes, a chest of the world, camera mode, the slime mount, a pet bunny and a baby slime
minion, the smart cursor, the retro / trippy / white lighting modes, an item tooltip, the stylist and clothes
windows, the Terra Blade / True Night's Edge / True Excalibur swung, Angel Wings and Hermes Boots, a wrench (wires
drawn), King Slime's treasure bag opened, the snow biome at night (aurora), and all 18 bosses one after the other;
70 scenes recorded 2 frames each, the digest of every frame, and per scene a state line (Main.rand, the local player,
every active NPC and projectile).

| | original | port |
|---|---:|---:|
| scenes | 70 | 70 |
| draw calls of the recorded frames | 183,212 | 183,212 exact |
| frame digests | 3,621 | all equal |
| state lines | 70 | 70 equal |

The first round (59 scenes) was exact at once with its JIT facts. The second round showed three things, each fixed
in the translator / fact generators (FLOAT_RULE.md section 10, items 14 and 15; item 16 noted): the aurora's vertex
strip (facts of the new dump), a falling star's trail (an inlined `Vector2 / 100f`: reciprocal folded at float
precision) and a rain splash's dust rotation (a second form of operator temporaries in Rain.Update).

Largest groups still without a reference (FLOAT_COVERAGE.md): Main 100 methods / 95 KB (old background drawing,
DrawCapture, kite / fishing line / player item drawing), WorldGen 88 / 82 KB (secret-seed worlds, remix, meteor, tree
shaking), Player 122 / 47 KB (armor set bonuses, boss bag variants), ParticleOrchestrator 60 / 45 KB (particles of
particular items), MessageBuffer 4 / 42 KB (the client's message receiving: the multiplayer gate runs it, without a
JIT dump), Projectile 50 / 42 KB, NPC 66 / 21 KB, Wiring 5 / 15 KB, AmbientSky 50 / 10 KB, ItemSlot, ChestUI, DD2Event.
Re-run: `sh tools/refdump_draw/run_draw.sh --scenes [--refdata]`; gate `scenes` (about 2 minutes).

## Secret seeds, the gallery, the interaction run and multiplayer: float-rule coverage 92.8 % (2026-10-16)

Coverage (`float_coverage.py`, FLOAT_COVERAGE.md; same definitions as the section above):

| | float methods covered | IL bytes covered | IL bytes not covered |
|---|---:|---:|---:|
| before (2026-10-15, scenes run) | 2,094 of 3,742 | 4,803,334 (86.6 %) | 744,368 |
| + 10 worldgen references (secret seeds) and their JIT twins | | | 671,417 |
| + gallery run | | | 540,169 |
| + interaction, multiplayer server and client runs; fact generators read generic names right | 2,584 (69.1 %) | 5,150,585 (92.8 %) | 397,117 |

What is still not covered (1,158 methods, 397 KB) is a long tail: the largest single method is 6 KB
(`Projectile.AI_DisplayDoll`); then gamepad instructions (`ItemSlot.GetGamepadInstructions`), capture mode drawing
(`Main.DrawCapture`), legacy code no 1.4.5 path calls (`FishingCheck_RollItemDropOld`, `AI_019_Spears_Old`,
`AI_015_Flails_Old`, `LoadWorld_Version1_Old_BeforeRelease88`, `Unused_HellHouse`), the dedicated server console
(`Main.DedServ`, `startDedInputCallBack`), menus not opened by any run (world generation debug, virtual keyboard,
resource packs, workshop, achievements), PvP-only code (`Projectile.StatusPvP`). By type: Main 54 methods / 40 KB,
Projectile 39 / 35 KB, Player 83 / 24 KB, WorldGen 34 / 21 KB, NPC 52 / 17 KB.

**World generation, ten more worlds** (refdata/worldgen/matrix; `run_matrix.sh` rows; JIT twins `run_refjit.sh`
`wg_<tag>`, data equal to the reference except timing): the celebration (`celebrationmk10`), the constant, no traps,
remix (`dontdigup`), zenith (`getfixedboi`), skyblock, and four worlds with the 1.4.5 secret seeds switched on through
`WorldGen.SecretSeed.Enable` (flag `secret.<name>`): error world; desert surface, extra liquid, rainbow, round
landmasses, extra holes, poo, no traps, portal gun chests, random and team spawns; frozen world, dual dungeons, start in
hardmode, hallow on the surface, infected, bigger abandoned houses, teleporters, extra living trees, extra floating
islands; mushroom surface, no surface, surface in space, gray paint, echo coat, no infection, no spider caves,
graveyard blood moon start, halloween. 9 of the 10 are identical in every row (108 of 108). The frozen world
(`ref_sec_frozen`, 97 rows): rows 0 to 29 identical; row 30 (pass "Dungeon", dual dungeons) differs in 4 cells of 4
columns (x 1108, 2775, 3031, 3065: a cracked brick placed in the original and not in the port, two bricks the
original removed and the port did not, and an emptied tile whose type the original reset); the later rows inherit the
difference (26 columns at Wall Variety, all of them at the end). Not found yet: the Dungeon pass of dual dungeons is
the new 1.4.5 layout code (`GameContent.Generation.Dungeon`, `DualDungeonLayoutProvider`); trigonometry is not the
cause (`TERRARIA_X87_TRIG=model` gives the same 4 cells). Where the original changes the four cells (harness
`REFDUMP_WATCH=33:<x>,<y>;..`: a second thread polls the cells during the pass and records the generating thread's
stack at each change, `<tag>.watch.txt`; sampled, so the innermost frame can be a few calls late): (3031, 459) and
(3065, 459) are cleared by a `CircleRoom` / `QuadCircleRoom` room shape (`GenShapeDungeonRoom`, through the
`Expand` / `ClearTile` actions); (2775, 706) gets its cracked brick from `DungeonHall.GenerateDungeonSquareHall`
(`RegularDungeonHall`); (1108, 586) is cleared by `RegularDungeonRoom.RegularRoom`. The shapes' own arithmetic is
integer-exact in both, so the difference is in what the rooms and halls are given (their positions and sizes come
from `Vector2D` / `Vector2` arithmetic in the layout provider) or in an intermediate state that later rooms cover;
the port has no matching per-call trace yet. Diagnosis: `WORLDGEN_DUMPCOLS=<row>:<x>,..` (tests/worldgen.rs)
writes the port's columns in the harness's snapshot layout; `RefDumpWorldgenX.exe genx .. passes <row list>` writes
the reference snapshots (work/worldgen-scratch/frozen_snap).

Fixed on the way: `Double.CompareTo(object)`, `Single.CompareTo(object)`, `Byte.CompareTo(object)` (rt::sys::compare_to_obj),
`Enum.HasFlag` (rt::sys::enum_has_flag; the teleporter generator).

**The gallery run** (`RefDumpDraw.exe <dir> gallery`, Gallery.cs; port `DRAW_MODE=gallery`, gate `gallery`): the
backgrounds of 9 places with the new backgrounds off, all 83 ParticleOrchestrator particles, all 19 ambient sky
entities, every projectile type 1..1135 (30 per batch) and every NPC type 1..696 (20 per batch) spawned next to the
player, the world events (meteor, tree shaking, hardmode with its conversion, an altar smashed, the lunar pillars),
the player's death and respawn.

| | original | port |
|---|---:|---:|
| scenes | 123 | 123 |
| draw calls of the recorded frames | 304,725 | 304,725 exact |
| frame digests | 3,817 | all equal |
| state lines | 123 | 123 equal |

**The interaction run** (Interact.cs; `DRAW_MODE=interact`, gate `interact`): 163 held / channelled items used
(whips, kites, yoyos, flails, spears, fishing poles at the beach, guns, staves), wired devices switched on and off,
chest quick stack / loot all / deposit / restock, the Old One's Army at difficulties 1 to 3, shimmer, sitting, a
travelling and a homeless NPC, dodges, grappling, rocket boots, armor sets, ghost, dusk / dawn, buff tooltips,
resource sets, item pickup, placement preview, wings, a black belt, quick heal / mana / buff / mount, the angler's
reward, two mounts' abilities.

| | original | port |
|---|---:|---:|
| scenes | 211 | 211 |
| draw calls of the recorded frames | 264,792 | 264,792 exact |
| frame digests | 8,083 | all equal |
| state lines | 211 | 211 equal |

**Multiplayer, byte for byte** (gate `net`): the original server (TerrariaServer.exe in-process, NetServer.cs) and
the original client (RefDumpDraw.exe net, NetClient.cs) run in lock step over a named pipe instead of sockets
(virtual TcpSocket on both sides; port: `terraria_host::sys::net::set_virtual`): the client joins, walks, chats, mines,
places, drops an item, toggles PvP and team, emotes, dies and respawns; the server spawns a slime, starts rain, drops
items, broadcasts a chat line, kills a tile, sets the night. Every message both ways is logged with its tick.

| | original | port |
|---|---:|---:|
| server ticks: received and sent bytes, state | 1,519 | 1,519 exact (tests/net.rs) |
| client frames: sent bytes | 1,519 | 1,519 exact (tests/draw.rs, DRAW_MODE=net) |
| client frame digests | 1,855 | all equal |
| client state lines | 14 | 14 equal |

Two host fixes made it exact: compressed tile sections (message 10) are now zlib 1.3.1's level-6 raw deflate,
byte for byte the output of the original's Ionic.Zlib (terraria_host::sys::zlib_deflate; unit test against 5
vectors written by the original, the empty input gives no bytes as in Ionic), and `Guid.NewGuid` gives the empty GUID
in the reference harnesses and the tests (`set_new_guid_empty`; the client's message 68).

**Float rule** (FLOAT_RULE.md section 10, items 17 to 25): the trigonometric instructions executed by the processor
(hw.rs); inlined constructor operands; both `Vector2.Hermite` overloads from XNA's machine code; generic names of the
server runs canonical in the fact generators; an evaluation-stack float call result spilled as a float; only 8-byte
zeroings counted as Vector2 temporaries (74 methods now "stack"); `LightningGenerator.Generate` as a measured body
(config `bodies`); `array.Length` is not a call; only the innermost binary operation makes a stack local wait across
a call.

**Game files from the player's install only**: `install_resources` (native client, gate drivers, tests) reads the
embedded resources from `terraria_host::content::install_dir()/Terraria.exe` (`rt::resources::install_from_assembly`),
as the browser build does; `TERRARIA_RESOURCES=decompile` keeps the old source for diagnosis. Of the 100 resources,
the 90 data files are byte-identical with the decompiled copies; the other 10 are embedded library
assemblies (DLLs the decompilation does not extract as files), which no gate reads; no gate changed (Proof 1: 3,911,380 values exact).

## The frozen world's dual dungeons exact; why every remaining float method has no reference; the coverage run (2026-10-16, later)

**Stage 1: `ref_sec_frozen` row 30 (pass "Dungeon", dual dungeons).** All 16 worlds of Proof 2 are now identical in
every row (gate `worldgen`, below).

How it was found:
1. A structural dump of both dungeons on both sides: the harness (GenX.cs, `REFDUMP_DUNGEON=<pass>`) and the port
   (tests/worldgen.rs `WORLDGEN_DUNGEON=<pass>`, tests/worldgen_diag/dungeon.rs) write every room (type, settings,
   position, seed, style, shape and its radii, inner / outer bounds), hall (start / end / directions as double bits,
   seed, settings, bounds), door, platform, protected bound and feature of `DungeonCrawler.dungeonData`. The pass
   clears that list when it ends (WorldGen.cs:14917), so both take it DURING the pass, when `Main.statusText` becomes
   "WorldGeneration.DungeonLate" (the harness: a second thread suspends the generating thread; the port: the
   runtime's yield hook at every loop head). 2,944 lines each; all equal (style objects aside). So every room and
   hall was given the same inputs; the difference was inside a room's shape.
2. The cells (3031, 459) and (3065, 459) are the left and right edge of the inner HourglassRoom (34 x 28) of room
   0.16 at (3048, 447), row i = 12; (1108, 586) the right edge of room 1.44's (38 x 28), the same row. The row's
   half-width is `(int)((float)_width * Clamp(MultiLerp(WrappedLerp(0, 1, percent), ...))) / 2`: 17 in the original,
   16 in the port.
3. The machine code of `DungeonShapes.HourglassRoom.Perform` in the frozen world's own JIT twin
   (work/jit/ref/wg_ref_sec_frozen, jit_disasm.py): `percent` stays on the x87 stack and the inlined
   `Utils.WrappedLerp` multiplies the REGISTER by 2 (`fmul dword [2.0]`); the port called `wrapped_lerp(.., percent
   as f32)`. Rounded, `2 - 2 * 0.92857143f` = 0.1428571939 > 1/7 sends MultiLerp's loop one step further (its `num`
   is the unrounded 1/7 on the x87 stack, FLOAT_RULE.md 10.24) and the product lands below 1; unrounded it is
   0.1428571428 and the clamp gives exactly 1.

Root cause and fix (FLOAT_RULE.md section 10, item 26): the JIT substitutes a caller's local for an inlinee parameter
that the inlinee never writes, so the inlined body reads the caller's unrounded x87 value. New fact
`tools/cs2rs/jit_inlined.tsv` (gen_jit_inlined.py: game callees with a float parameter that no run compiled a call
instruction to, in every run that ran the caller; 206 pairs), config `jitInlined`, translator twins `name__u` /
`name__u__x87` (Model.cs `ArgTwinParams`, Gen.cs, Emit.cs `InlinedArgsTwin`, FloatAlloc `extraStack`): 51 twins, 39
call sites. The 4 cells, and the 26 columns that followed from them at Wall Variety, are gone: `ref_sec_frozen` 97 of
97 rows. No generated file was edited by hand.

**Stage 2: a reason for every float method without a reference.** `tools/cs2rs/float_reasons.py` (after
float_coverage.py) adds two columns to float_uncovered.tsv for each uncovered method: a reason and its evidence,
from the binary and the dumps, not by hand except a short table of conditions checked in the source (OVERRIDES, each
with its file and line):

| reason | how it is decided | methods | IL bytes |
|---|---|---:|---:|
| inlined | every covered caller that calls it has no call instruction to it in the machine code of every run that ran that caller | 16 | 705 |
| dead | no call / callvirt / newobj / ldftn of it in Terraria.exe (db port_calls), not virtual, not a static constructor; or only dead callers (fixed point); or behind `DebugOptions.enableDebugCommands`, false and never set | 140 | 52,419 |
| platform | Steam, WeGame, Steam Workshop publishing, the RGB device SDKs (each with the guard: SocialAPI.Initialize, ChromaInitializer) | 102 | 20,381 |
| unrunnable | reachable, but no JIT reference can be made: the dedicated server's own frame loop and console reader (wall clock, Console.ReadLine; its commands are compared another way, console_diff.py 0 differences), world files of release 87 or older, map files of release 91 or older or with the 0x8000 flag (this build writes neither), the F7 diagnostics (this machine's Win32 values; TimeLogger's own stopwatches: two runs differ) | 18 | 17,206 |
| notrun | reachable, no reference run executes it yet; the evidence names the covered caller and the guard of the call site (decompiled statement and the nearest enclosing condition), or the nearest covered ancestor, or for an override that no instance of its type was created | 772 | 232,703 |

**The coverage run** (`RefDumpDraw.exe <dir> cover`, tools/refdump_draw/Cover.cs; port `DRAW_MODE=cover`,
tests/draw/cover.rs; gate `cover`; `sh tools/refdump_draw/run_draw.sh --cover [--refdata]`): six display dolls holding
a weapon of every projectile aiStyle in every pose its use style supports (535 weapon / pose pairs, 90 scenes), two
hat racks, a door opened to both sides and closed, a trapdoor opened and closed, a decorated Christmas tree placed
and broken, minecart tracks, two wired teleporters, a logic gate with a lamp, the inventory's banner page / banner
grid / recipe grid / housing page, the map saved and loaded (the version-2 reader), all 509 accessories five at a
time, all 66 mounts ridden, the party / lantern-night / Martian skies, the Wall of Flesh summoned, seven UI states
shown in game (character list, achievements, controls, world creation, its advanced page and secret seeds, the
virtual keyboard), the four lighting modes, the torch god, the mystic sash's dodge, all 18 sentries carried, a
drunk world's music choice, the gamepad user interface (inventory, controls, emotes), the corruption / hallow /
crimson surface backgrounds and an item left in shimmer until it transforms. Its JIT twin reproduces the reference
files byte for byte (304 of 304).

| | original | port |
|---|---:|---:|
| scenes | 301 | 301 |
| draw calls of the recorded frames | 431,161 | 431,161 exact |
| frame digests | 3,887 | all equal |
| state lines | 301 | 301 equal |

What it showed (FLOAT_RULE.md section 10, item 27): XNA's `Vector2.Transform`, `Vector3.Transform` and `Matrix *
Matrix` sum their products on the x87 stack and round once per component; `Matrix.CreateRotationZ` stores the
processor's `fcos` / `fsin`. terraria_core did both in f32 per operation with the correctly rounded functions: a UFO
mount's sprite was 1 ulp off in y in one unrecorded frame (`DrawPlayer_TransformDrawData`). Config `jitBodies` now
has the four bodies (game.json). Also fixed on the way: the port test keeps UI states it holds across frames rooted
for the collector (`rt::Rooted`), as the harness's locals are.

Tried and left out, with the reason recorded: the world generation debug view (its constructor needs a running
generator; DebugOptions), the F7 diagnostics pages (machine values; the stopwatch page differs between two runs of
the original itself).

Coverage (float_coverage.py): 2,694 of 3,742 float methods (before 2,584), IL 5,224,288 of 5,547,702 bytes, 94.2 %
(before 92.8 %); not covered 1,048 methods / 323,414 bytes (before 1,158 / 397,117). 42 dumps (+ client/cover).

Gates (`sh tools/cs2rs/gates.sh --slow --skip window-modes`, from port\, regenerated; port/target-gen/gates/summary.txt):
41 of 41 rows ok. p1 3,911,380 values exact; client-whole; rt 106; sim; ticks-player; ticks-world; host 111; app 102;
viewer SHA-1 35b0971b...; scripts 14 of 14; draw 39,608 of 39,608 calls / 730 frames; audio 748,270 of 748,270 calls,
draw 47,748, 1,778 frames; scenes 70 / 183,212 / 3,621 / 70; gallery 123 / 304,725 / 3,817 / 123; interact 211 /
264,792 / 8,083 / 211; cover 301 / 431,161 / 3,887 / 301; net server 1,519 of 1,519 ticks, client 1,519 of 1,519
frames; worldgen 16 of 16 worlds identical in every row (15 at 108 of 108, `ref_sec_frozen` 97 of 97); entities npc
696, proj 1,135, item 4,629; client-items 4,629; multiplayer A and B exit 0.

Browser (game.wasm rebuilt: `sh web/ci/build_game_wasm.sh`, 58,617,355 bytes): headless `single_player_fast` in the
world at frame 4,030 (`work/shots/f_14_in_world_600.png`, log `work/logs/spf_final.txt`); the play page
(`--host --page play --viewport 1280x604 --actions test/actions/play_world.txt`): 107 of 107 actions, cursorcheck 0 px
at every size (`work/logs/play_world_final.txt`); `sh web/www/test/run-host-console.sh` exit 0, 75 of 75 actions.

## Every float method either confirmed against the original or listed with its reason: notrun 772 -> 0 (2026-10-17)

**The probe run** (`RefDumpDraw.exe <dir> probe`, tools/refdump_draw/Probe.cs and ProbeA.cs .. ProbeS.cs, 94 groups,
`PROBE_ONLY=<group,...>` runs a subset on both sides; JIT twin RefJitDraw; port `DRAW_MODE=probe`,
tests/draw/probe.rs and probe_a.rs .. probe_s.rs, step for step; new gate `probe`; `sh tools/refdump_draw/run_draw.sh
--probe [--refdata]`). Each group drives a part of the game the earlier runs never reached, in the world, in menus or
by a direct call where the game itself would call it: player helpers, projectiles, mounts, minecarts, whips on a
training dummy, NPC attacks, spawns and town NPCs, world generation helpers called directly (trees, gem locks, paint,
lightning, the travelling merchant, a legacy dungeon room asked for a dual dungeons biome chest), tile entities,
doors, pylons and the pylon map, capture mode, the camera modifiers, Moon Lord's death drama, sandstorm sky and
overlay, reflective dye on a body armour, torches of every style, seaweed in a pool, the sun low and on screen beside a
grown tree (the lit nature shader), the achievements / controls / bestiary / crafting / journey (research with its
cogs) / resource pack (a pack listed as enabled) / hairstyle menus, savings in a piggy bank, the gamepad pages' ends,
the time logger and more. Its JIT twin reproduces the reference files byte for byte (301 of 301). NetClient.cs gained
quick stacking to nearby chests and two teammates (name plates on and off screen); refdata/net regenerated.

| | original | port |
|---|---:|---:|
| scenes | 298 | 298 |
| draw calls of the recorded frames | 677,274 | 677,274 exact |
| frame digests | 10,698 | all equal |
| state lines | 1,514 | 1,514 equal |

**Before / after, per reason** (float_reasons.py; methods, IL bytes):

| reason | before | after |
|---|---:|---:|
| covered | 2,694 (5,224,288) | 3,401 (5,433,573) |
| inlined | 16 (705) | 17 (725) |
| dead | 140 (52,419) | 179 (66,462) |
| platform | 102 (20,381) | 114 (26,691) |
| unrunnable | 18 (17,206) | 31 (20,251) |
| notrun | 772 (232,703) | 0 (0) |

Coverage 97.9 % of the float methods' IL (5,433,573 of 5,547,702 bytes), 3,401 of 3,742 methods, from 47 dumps
(+ client/probe, four new worlds below). Every method that is not covered has its reason and evidence in
float_uncovered.tsv columns 7 and 8 (FLOAT_COVERAGE.md, "Why the uncovered methods have no reference").
The new dead / platform / unrunnable entries are each backed by the call graph or a source condition with file and
line (float_reasons.py `DEAD_TYPES`, `DEAD_METHODS`, `OVERRIDES`; "called only from platform code" is now propagated):
for example `Main.EndDraw` and `Main.SetDisplayModeAsBorderless` (the window, never in-process), the character
creation's template paste (the system clipboard), `WorldGen.playWorldCallBack` (the menu's world loading thread).

**New worlds** (Proof 2's matrix, tools/refdump_worldgen/run_matrix.sh and run_refjit.sh, gate `worldgen`,
refjit_compare.py): `ref_sec_liquid` (extra liquid, rain for a year, paint everything negative, dual dungeons),
`ref_trapsworthy` (no traps + for the worthy), `ref_shimmerocean` (drunk + celebration), `ref_dual_a` (dual
dungeons, classic). All four identical in every row. `ref_dual_b` covered nothing the others did not and was moved to
work/_archive/ref_dual_b_2026-10-17.

**What the probe run found and fixed at the root** (no generated file edited by hand):
- The JIT dumps did not see closed generic instantiations (FLOAT_RULE.md 10.29), struct methods reached through an
  interface (10.31) or the shared code of generic methods over reference types (10.34): JitDump.cs now resolves the
  TypeSpec / MethodSpec tables, follows unboxing stubs, and names shared code found as a call target of code that ran
  (its MethodDesc checked against the dumped assemblies' MethodTables before the runtime is asked for its name).
- XNA's by-reference `Vector2.Transform` (10.32: a rotated crit number 1 ulp off) and the instance
  `Vector3.Normalize` (10.35: reflective dye's light direction 1 ulp off) sum on the x87 stack: config `jitBodies`.
- A float local initialised with a constant and never written is a constant to the JIT, so `v / num` multiplies by the
  reciprocal folded at float precision (10.33: the flame particles' scale 1 ulp off): Emit.cs `JitConstLocal`.
- `Math.Max` / `Math.Min` of a NaN return the NaN operand (10.28); the fact chain must run twice (10.30; a third pass
  leaves all eight tables unchanged).
- Translator: a chained native receiver (`sb.Append(a).Append(b)`) is bound once; `OfType<interface>` tests the type;
  `ICollection<T>.Contains` added to the runtime (`rt::collection_contains`).
- Port test harness: an NPC slot the game replaces (`NPC.NewNPCInstanceInSlot`) is read through a rooted reference, as
  the reference's local keeps the old object.

Gates (`sh tools/cs2rs/gates.sh --slow --skip window-modes`, from port\, regenerated; port/target-gen/gates/summary.txt):
46 of 46 rows ok. p1 3,911,380 values exact; client-whole; rt 106; sim; ticks-player; ticks-world; host 111; app 102;
viewer SHA-1 35b0971b...; scripts 14 of 14; draw 39,608 of 39,608 calls / 730 frames; audio 748,270 of 748,270 calls,
draw 47,748, 1,778 frames; scenes 70 / 183,212 / 3,621 / 70; gallery 123 / 304,725 / 3,817 / 123; interact 211 /
264,792 / 8,083 / 211; cover 301 / 431,161 / 3,887 / 301; probe 298 / 677,274 / 10,698 / 1,514; net server 1,557 of
1,557 ticks, client 1,557 of 1,557 frames; worldgen 20 of 20 worlds identical in every row (17 at 108 of 108,
`ref_sec_frozen`, `ref_sec_liquid` and `ref_dual_a` 97 of 97); entities npc 696, proj 1,135, item 4,629;
client-items 4,629; multiplayer A and B exit 0.

Browser (game.wasm rebuilt: `sh web/ci/build_game_wasm.sh`, 58,808,789 bytes): headless `single_player_fast` in the
world at tick 3,467, shot at frame 4,150 (`work/shots/f_14_in_world_600.png`, log `work/logs/spf_probe.txt`); the play
page (`--host --page play --viewport 1280x604 --actions test/actions/play_world.txt`): 107 actions run to the last,
cursorcheck worst 0 px at all nine checks (`work/logs/play_world_probe.txt`); `sh web/www/test/run-host-console.sh`
exit 0 (`work/logs/host_console_probe.txt`).

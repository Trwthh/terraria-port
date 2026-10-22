# VAEL - TERRARIA: plan

Goal: a Rust rewrite of Terraria 1.4.5.8 on the Bevy engine, built against a SQLite reference
database of the decompiled game. The port reads the player's own `Content/` folder at run time;
no game assets are shipped.

Status key: `[ ]` not started, `[~]` in progress, `[x]` done and verified.
Update this file at every milestone.

## Inputs (read-only)

| Thing | Path |
|---|---|
| Game install (832 MB) | `C:\Users\user\Documents\Games\Terraria` |
| Main assembly | `Terraria.exe` 1.4.5.8, 26.6 MB, .NET Framework 4 / XNA 4.0, sha256 `960a03bf...6ba2f3` |
| Engine library | `ReLogic.dll` 1.0.0.0, stored inside `Terraria.exe` as a resource |
| gamedb CLI | `VAEL - GAMELAB\tools\gamedb`, commit `7054201` (= GitHub `main` on 2026-10-10), `target\release\gamedb.exe` |
| ILSpy CLI | `VAEL - GAMELAB\tools\ilspycmd`, version 9.1.0.7988 |
| XNA 4.0 reference assemblies | `VAEL - GAMELAB\tools\xna40` (unpacked from the game's own `xnafx40_redist.msi`, not installed) |

Nothing in the game folder is written to. `OnlineFix.dll` and `steam_api.dll` in that folder are not
part of the game's code and are not read, decompiled or indexed.

## Layout

| Path | Contents |
|---|---|
| `decompile\Terraria\`, `decompile\ReLogic\` | ILSpy C# trees (1,549 + 167 files) plus the 90 embedded data resources (JSON, TSV) |
| `meta\<assembly>\*.jsonl` | ground truth read from the binary: types, methods, fields, IL call sites, field accesses, strings |
| `meta\src\*.jsonl` | every C# declaration with its exact line span (Roslyn) |
| `meta\embedded\`, `meta\refs\` | the 10 libraries stored inside `Terraria.exe` |
| `db\terraria.sqlite` | the reference database: gamedb tables + `port_*` tables (158 MB) |
| `tools\` | `pipeline.py` (everything), `port_extend.py`, `parity.py`, `modules_gen.py`, `tmeta\` (C# extractor) |
| `logs\parity-report.md` | latest parity result |
| `_archive\` | earlier decompile trees (replaced, never deleted) |

One command rebuilds and re-checks everything: `python tools\pipeline.py`.

## Phase 0 - Setup  [x]

- [x] gamedb: own folder, read README and `--help`, confirmed `gamedb index -r SRC --db PATH [--force] [--rules FILE]`.
- [x] Fingerprint the install: managed .NET assembly, not obfuscated, no packer.
- [x] ILSpy CLI installed as a local tool (not global).

## Phase 1 - Decompile and index  [x]

- [x] Decompile `Terraria.exe` and `ReLogic.dll`. First attempt produced invalid C# in 349 files because the
      XNA and embedded libraries could not be resolved; fixed by giving ILSpy the real references. Now 0 of 1,716
      files have a parse error.
- [x] `gamedb index`: 1,716 files, 14,916 functions, 10,539 strings, 52,476 symbols, 517,116 edges.
- [x] Module rules generated from the namespaces (`tools\modules.txt`, 50 modules in use).
- [x] `port_*` tables (see README for the schema): every file with its bytes, 3,208 types, 22,248 methods,
      33,660 fields, 264,508 call sites, 447,671 field accesses, 11,720 string literals, all tied to source lines.
- [x] Re-runnable: unchanged input gives identical table fingerprints, including after a fresh decompile.

## Phase 2 - Parity  [x] PASS, 42 checks, 0 failed, 12 gaps

Definition and results: `logs\parity-report.md`. Gaps the port has to plan around:

1. gamedb's own parser misses 2,703 method bodies (mostly property accessors). Use `port_methods` for anything exhaustive.
2. gamedb's call edges are name matches. Use `port_calls` (exact, from IL).
3. 107,671 call sites leave the indexed code (XNA 49,584, .NET 42,892, the rest third-party). This is the API surface to replace.
4. XNA, the .NET Framework, 9 third-party libraries and 4 native DLLs are not in the database.
5. `Content\` (textures, sounds, fonts, shaders) is not indexed. Phase 4 documents those formats.
6. `TerrariaServer.exe` is not indexed. It is built from the same source; diff it against `Terraria.exe` by IL hash before the multiplayer phase.

## Phase 3 - Make the reference usable for a port  [ ]

- [ ] Rank modules by size and by how many others call into them (`port_callgraph` grouped by `files.module`), and
      write the porting order into this file. Expected shape: data tables first, then world, then entities.
- [ ] Inventory the static data: `ID\*` constant classes, `Item.SetDefaults`, `NPC.SetDefaults`, `Projectile.SetDefaults`,
      tile and wall property arrays in `Main`. Decide per table: generate Rust from the database, or load at run time.
- [ ] List every XNA and System API the game calls (from `port_calls` where `dst_assembly IS NULL`) and map each to a
      Bevy / Rust equivalent or mark it "needs own implementation". Output `docs\api-surface.md`.
- [ ] Find where determinism matters (world generation seed, `UnifiedRandom`, fixed 60 Hz update) and record the
      functions involved.
- [ ] Add `port_status` tracking: `gamedb set-module` per module, plus a table mapping Rust items to method tokens.

## Phase 4 - Asset and file formats  [ ]

- [ ] `.xnb` containers: Texture2D, SoundEffect, SpriteFont, Effect. Shaders need a rewrite to WGSL (XNA effects are
      compiled HLSL bytecode); list them and their parameters from the calling code.
- [ ] Audio: `.xwb` wave bank, `.xsb` sound bank, `.xgs` settings; OGG / MP3 music through the embedded decoders' logic.
- [ ] World file (`.wld`) and player file (`.plr`, AES-encrypted with a fixed key in the source) read and write, verified by
      loading a world saved by the original and saving it back byte-identical.
- [ ] Localization JSON and the embedded data resources (already in `port_files`).

## Phase 5 - Bevy foundation  [ ]

- [ ] Cargo workspace: `terraria-formats` (file readers, no Bevy), `terraria-data` (generated tables), `terraria-sim`
      (game rules, no rendering), `terraria-app` (Bevy). Pin the Bevy version here when the workspace is created.
- [ ] Fixed 60 Hz simulation schedule separate from rendering, matching `Main.Update`.
- [ ] Asset source that reads `Content\` from the user's install path.
- [ ] Tile world on screen: load a `.wld`, draw tiles, walls and liquids with the original framing rules.

## Phase 6 - Systems, in dependency order  [ ]

1. [ ] Core types: `Tile`, `Entity`, `Item`, `Player`, `NPC`, `Projectile` data layouts; `UnifiedRandom` bit-exact.
2. [ ] World: tile framing, collision (`Collision.cs`), liquids, lighting, wiring.
3. [ ] Player: movement, inventory, equipment, buffs, mounts.
4. [ ] Items and projectiles: use styles, shooting, damage.
5. [ ] NPCs: AI styles (the `AI_xxx` methods in `NPC.cs`), spawning, town NPCs, bosses.
6. [ ] World generation (`WorldGen.cs`, `GameContent\Biomes`, `WorldBuilding`), checked seed-for-seed against the original.
7. [ ] UI: inventory, crafting, map, menus, chat.
8. [ ] Audio.
9. [ ] Events and progression: invasions, moons, hardmode.
10. [ ] Multiplayer (`NetMessage.cs`, `MessageBuffer.cs`), after diffing `TerrariaServer.exe`.

Each system: read the reference methods, write Rust, verify against a repeatable capture of the original
(`reference-capture` and `recreate-fidelity` skills), then mark the module `IMPLEMENTED --verified`.

## Phase 7 - Finish  [ ]

- [ ] A new world can be generated, played from start to the first boss, saved, and reloaded.
- [ ] Every module verified in `module_status`; parity report re-run clean; coverage of `port_methods` reported.

## Known limits (stated up front)

- Decompiled C# is an approximation of the original source: local variable names are gone, some control flow is
  restructured. Behaviour is the same; the text is not what Re-Logic wrote.
- The biggest methods are very large (`Main.cs` 89,727 lines, `Player.cs` 65,770). They need splitting by behaviour,
  not a line-by-line translation.
- Size of the job: 19,783 hand-written methods and 7.25 MB of IL. The reference is done; the rewrite is a long project.

## Log

- 2026-10-10: project created. Decompiled, indexed, port tables built, parity PASS (42 checks, 0 failed, 12 gaps).
  Next: Phase 3, starting with the module ranking and the static data inventory.

## Status snapshot 2026-10-17 (every float method confirmed against the original or listed with its reason)

Goal of the stage: every game method that does decimal (float) arithmetic either confirmed bit for bit against the
original client or server, or listed with the reason it cannot be. Done: notrun 772 -> 0.

- The probe run (tools/refdump_draw/Probe*.cs, 94 groups; port tests/draw/probe*.rs; gate `probe`): 298 scenes,
  677,274 of 677,274 draw calls exact, 10,698 frame digests equal, 1,514 of 1,514 state lines equal; JIT twin 301 of
  301 files equal.
- Coverage 3,401 of 3,742 float methods, 97.9 % of their IL (before 2,694, 94.2 %). Not covered 341, each with its
  reason and evidence (float_uncovered.tsv): inlined 17, dead 179, platform 114, unrunnable 31, notrun 0
  (before 16 / 140 / 102 / 18 / 772).
- Four new worlds in Proof 2 (`ref_sec_liquid`, `ref_trapsworthy`, `ref_shimmerocean`, `ref_dual_a`): all identical.
- Root fixes (FLOAT_RULE.md section 10, items 28 to 35): Math.Max/Min NaN; the JIT dumps now include generic
  instantiations, struct methods behind interfaces and shared generic code; the fact chain twice; XNA's by-reference
  Vector2.Transform and Vector3.Normalize on the x87 stack; constant float locals folded by the JIT. Translator: native
  receiver bound once, OfType of an interface, ICollection<T>.Contains.
- Gates `--slow --skip window-modes`: 46 of 46 ok. game.wasm rebuilt (58,808,789 bytes); headless single player in
  the world, play page 107 actions with cursorcheck 0 px, host console exit 0.

Details: tools/cs2rs/VERIFICATION.md, last section.

## Status snapshot 2026-10-15 (audio output against the original; float-rule coverage and the scenes run)

Stage 1, audio. `tools\refdump_draw\AudioRec.cs` replaces XNA's native audio layer (`SoundEffectUnsafeNativeMethods`,
the XACT `UnsafeNativeMethods`) in the draw harness with a deterministic engine model and records every call; XNA's
managed audio code runs as it is, and the XAudio2 parameters (voice volume, output matrix, frequency ratio) come from
XNA's real kernel functions (IL in the mixed-mode assembly) run on a fake voice. Port: terraria-host `audio.rs` now
follows XNA's managed code over `audio::engine` (the native layer, same model, writes `audio::trace`). The audio run
(`RefDumpDraw.exe <dir> audio`, refdata\audio): the draw scenario with sound on, menu clicks, walking on grass, mining,
a slime hit and killed, a potion, music across day / night / space / ocean / underworld. Gate `audio`:

| | result |
|---|---|
| audio calls, all 1,779 frames | 748,270 of 748,270 exact, audio digest of all frames equal |
| draw of that run | 12 scenes, 47,748 of 47,748; 1,778 frame digests equal |
| mixer vs a model of XAudio2's per-voice mixing (kernel values) | 55 voice starts / 36 parameter sets, 119,395 of 147,456 samples bit-identical, max 1.19e-7 of full scale |

Fixed at the root: the stereo pan law and frequency ratio (now XNA's kernel law, mixer `output_matrix` /
`frequency_ratio`), sound effect labels, `SoundEffectInstance.Stop(bool)` and `PendingBufferCount` mapped (they were
unimplemented), a cloud colour (FLOAT_RULE.md section 10, item 13: "no location" float locals are JIT stack locals).

Stage 2, float coverage (`tools\cs2rs\float_coverage.py`, `FLOAT_COVERAGE.md`): 3,742 game methods do float arithmetic
(5.55 MB IL). Covered by a reference process's JIT dump: 1,860 before (82.2 % of the IL), 2,094 now (86.6 %; audio run
+23, scenes run +211). New scenes run (`RefDumpDraw.exe <dir> scenes`, refdata\scenes, gate `scenes`): menus, nine
biomes, six events, the in-game UI screens, mount / pets / minion, smart cursor, three lighting modes, item tooltip,
stylist / clothes windows, three projectile swords, wings and boots, wrench, a boss bag, the aurora, all 18 bosses:
70 scenes, 183,212 of 183,212 calls exact, 3,621 frame digests equal, 70 of 70 state lines equal. Fixed: FLOAT_RULE.md
section 10 items 14 (inlined `Vector2 / constant`: reciprocal folded at float precision) and 15 (second form of
operator frame temporaries); item 16 notes a history dependence of jit_calls.py, not changed. Still without a
reference: 1,648 methods / 744 KB, largest Main 95 KB, WorldGen 82 KB, Player 47 KB, ParticleOrchestrator 45 KB,
MessageBuffer 42 KB, Projectile 42 KB, NPC 21 KB, Wiring 15 KB (VERIFICATION.md, "The scenes run").

Gates (`sh tools/cs2rs/gates.sh --slow --skip window-modes`, from port\): all 24 rows ok: p1 3,911,380 exact,
client-whole, rt 106, sim (collision 62 of 62, light map 175,900 of 175,900 raw), ticks-player, ticks-world, host 110, app 102, viewer, scripts 14 of
14, draw 39,608 of 39,608, audio, scenes, worldgen 6 of 6 at 108 of 108, entities npc 696 / proj 1,135 / item 4,629,
client-items 4,629, multiplayer. window-modes skipped (real window).
Browser: `web/ci/build_game_wasm.sh` (58,579,074 bytes), headless `single_player_fast` in the world at frame 4,780
(`work\shots\f_14_in_world_600.png`, log `work\logs\spf_after_audio.txt`), `run-host-console.sh` exit 0, 75 of 75 actions
(`work\logs\host_console_after_audio2.txt`; a first run under load from other projects' builds stopped at the save check
while the server was at 38 % of its save).
Re-run the references: `sh tools/refdump_draw/run_draw.sh [--audio | --scenes] [--refdata]` (byte-identical JIT twins).

## Status snapshot 2026-10-14, late night (draw output: every draw call against the original client)

What the client DRAWS is now compared with the original, call by call (VERIFICATION.md "Draw output"). Reference
`tools\refdump_draw\RefDumpDraw.cs`: the real Terraria.exe (copy in `work\refclient`) in-process on a recording XNA
stand-in with XNA's identity (no GPU device; the form hidden on a desktop of its own), every SpriteBatch / device /
effect call per frame. Port: `terraria_host::gfx::trace`, test `tests\draw.rs`, reference `refdata\draw`, gate `draw`.
Re-run the reference with `sh tools/refdump_draw/run_draw.sh [--build] [--refdata]`.

| scene | calls exact |
|---|---:|
| boot, title, character creation | 34, 2,124, 4,804 |
| world day / night, ocean, cave, underworld | 2,584 / 3,090, 2,468, 2,213, 9,553 |
| boss + 6 projectiles, inventory, full-screen map | 5,228, 6,007, 1,503 |
| all 11 scenes | 39,608 of 39,608; digest of all 730 frames equal |

Fixed at the root: the generated `new UnifiedRandom()` read its own clock (host delegation); client-only methods get
the draw run's JIT facts (`jit_calls.Dirs.fill`); Vector2 products kept on the x87 stack across a call's arguments
(`VertexStrip.AddVertexPair`, Emit.cs); callees inlined at some sites only, decided site by site through the CLR's
IL / native map and ILSpy's PDB (`jit_refcall_sites.tsv`, `tmeta pdbseq`, FloatAlloc.cs `SiteIsCall`; `Dust.GetAlpha`);
runtime dictionaries now compare hash codes before `Equals`, as .NET does (`terraria-gen-rt` collections.rs `KeyW`;
`TileBatch.LayerBatchKey` made tile batches nondeterministic). FLOAT_RULE.md section 10 items 11 and 12.

Gates (`sh tools/cs2rs/gates.sh --slow --skip window-modes`, from port\): p1 3,911,380 exact, client-whole, rt 106,
sim (collision 62 of 62 bit-exact, light map 175,900 of 175,900), ticks-player, ticks-world, host 109, app 102,
viewer, scripts 14 of 14, draw 39,608 of 39,608, worldgen 6 of 6 at 108 of 108, entities npc 696 / proj 1,135 /
item 4,629, client-items 4,629, multiplayer: all 23 ok. window-modes skipped (real window).
Browser: `web/ci/build_game_wasm.sh` (58,423,386 bytes), headless `single_player_fast` in the world at frame 4,030
(`work\shots\f_14_in_world_600.png`, log `work\logs\spf_after_draw.txt`), `run-host-console.sh` exit 0
(`work\logs\host_console_after_draw.txt`). Experiment output: `work\_archive\draw`.

## Status snapshot 2026-10-14, night (entities: all exact; client-side item use by the owning player, all exact)

Stage 1, the last five entity types (all bit-exact now; `tools\cs2rs\entities_allow.txt` empty, checked both ways).
Every site read in the reference run's own machine code (`work\jit\ref\entities_*`, `jit_disasm.py`); the rules are for
inlined XNA `Vector2` operators (FLOAT_RULE.md section 6 "Inlined XNA Vector2 operators", section 10 items 8 - 10):

| type | site | rule (translator) |
|---|---|---|
| golf balls 741 / 742 / 744 | `GolfHelper.ContactListener.OnCollision`, `collision.Normal * Vector2.Dot(..)` with op_Multiply inlined: `fmul st(1)` on the call's result | float argument of an inlined operator / XNA body: a local, field, parameter or call result enters unrounded, any other expression through a 4-byte slot (`VecScalar`) |
| proj 797 (its world item) | `WorldItem.TryCombiningIntoNearbyItems`, `Vector2.Lerp(.., .., amount)` inlined, `amount` never stored | the same, for `jitBodies` templates at inlined sites |
| NPC 426 (its projectile 581) | `Utils.FactorAcceleration`: operator chain and the Vector2 local on the x87 stack | per method, operator results on the x87 stack or a zeroed frame temporary per operator (`jit_vec_temps.tsv`, `gen_jit_vec_temps.py`); promoted Vector2 locals keep their components until a call / control flow (`VecX87`, `VecTrackable`) |
| (proj 1047, broken by the rule above alone) | `FactorAcceleration` inlined into `RainbowBoulder_JumpToPosition`, its divisions / subtraction are calls there | an inlined game method's operators per caller (`jit_vec_ctx.tsv`, `gen_jit_vec_ctx.py`, from the caller's IL-to-native map); twins `name__vcN` |

Virtual calls returning float: none of the game's 24 virtual / override / abstract float members ran in any of the 24
reference processes; not changed. New test switch `ENTITIES_TRACE_STEPS=1` (traced entity after each part of a tick).

Stage 2, client-side item use: `RefDumpEntities.exe citem` (`tools\refdump_entities\ClientItems.cs`, format in
`refdata\entities\README.md`; JIT twin scenario `entities_citem`, byte-identical): every item with a use style (4,629),
one per batch, used 60 ticks by the OWNING player of a single player game (`Main.myPlayer = 0`, `netMode = 0`) holding
the left button on a fixed target (tools: the ground; others: the air tile above it), 60/100 life, 100/200 mana, 18 kinds
of ammo; per tick everything the other kinds dump plus the tiles of a box around the player. Client pieces the server
build lacks are set up identically on both sides (game view, achievements, lighting engine, a stand-in font for text
measurement), `Main.ignoreErrors = false`; 5 batches end at an exception in the real run (textures the server build
does not have), the port throws at the same tick (counted as a match, reported). Gate `client-items` (`gates.sh --slow`).

| | types | first run | with the run's facts | now |
|---|---:|---:|---:|---:|
| item, owning client (citem) | 4,629 | 277 of the first 300 | 4,627 | 4,629 |
| NPC / projectile / item (server) | 696 / 1,135 / 4,629 | | | all exact |

Facts regenerated from 24 runs: call sites decided by the reference runs' code 10,632 -> 30,348; XNA sites with their x87
body 147 -> 1,896; methods with the JIT's own float allocation 389 -> 447; jit_vec_temps 233 methods, jit_vec_ctx 29
sites. New rule from the client run: a float CALL RESULT waiting while another call runs is spilled as a float
(`fstp dword`; `GetLerpValue(..) * GetLerpValue(..)` in `AI_165_Whip`, whips 5479 / 5480), Emit.cs `F64Pending`.
Test harness: the collector now runs between citem batches (`rt::collect`, Main rooted): without it the test kept every
object of all batches and ran out of memory.

Gates (`sh tools/cs2rs/gates.sh --slow --skip window-modes`, from port\; regeneration deterministic, 0 files changed):
p1 3,911,380 exact, client-whole, rt 106, sim (collision 62 of 62 bit-exact, light map 175,900 of 175,900 raw from
sim.txt), ticks-player, ticks-world, host 109, app 102, viewer, scripts 14 of 14, worldgen 6 of 6 at 108 of 108,
entities npc 696 / proj 1,135 / item 4,629, client-items 4,629, multiplayer: all ok. window-modes skipped (real window).
Browser: `web/ci/build_game_wasm.sh` (58,273,384 bytes), headless `single_player_fast` in the world at frame 4,030
(`work\shots\f_14_in_world_600.png`, log `work\logs\spf_after_citem.txt`), `run-host-console.sh` exit 0.
Logs of this session's runs: `work\_archive\citem\logs`.

## Status snapshot 2026-10-14, evening (entities: every NPC, projectile and usable item against the real server)

New reference scenario and gate. `tools\refdump_entities\Entities.cs` (RefDumpEntities.exe npc|proj|item, refdata\entities,
format in its README): on a scratch copy of ref_small.wld, server state as the ticks harness, every NPC type (696), every
projectile type (1,135) and every item with a use style (4,629 of 6,195; held by its slot's remote player, "use item"
every tick) in slots of 31 per batch, one remote player per slot, each batch from the same reset (changed tiles restored,
NPC / Projectile.ClearAll, fresh players and world items, Main.rand / Main._rngs re-seeded), 120 ticks of the entity part
of Main.DoUpdateInWorld; every leaf of every active NPC / projectile / player / world item (195 / 109 / 1,180 / 32) and every
RNG stream dumped per tick. No type skipped (every spawn call returned a slot). JIT twin `RefJitEntities.exe`
(EntitiesJit.cs; run_refjit.sh entities_npc / entities_proj / entities_item): byte-identical output (refjit_compare.py), so
its machine code is the code that produced the data. Port: `tests\entities.rs` (per type exact / root / after, trace and
perturbation switches), gate `entities` in gates.sh --slow with `tools\cs2rs\entities_allow.txt` (checked both ways).

| | types | exact first run | exact now |
|---|---:|---:|---:|
| NPC | 696 | 637 | 695 |
| projectile | 1,135 | 1,028 | 1,131 |
| item (use style, server side) | 4,629 | 4,629 | 4,629 |

Fixes (translator, runtime, config; no generated file edited; FLOAT_RULE.md section 10, VERIFICATION.md "Entities"):
real-JIT facts now from 23 runs, conflicts dropped per (caller, callee) pair; the CLR's own variable locations of every
method that ran (`JitDump.cs WriteDebugInfo`, `jit_debuginfo.py`) named through ILSpy's PDB of the same decompilation
(`tmeta pdblocals`, `work\pdb`) -> `jit_float_alloc.tsv` (config jitFloatAlloc: 389 methods, 3,367 locals in memory, 286 on
the x87 stack; decompiled locals that are IL stack values stay unrounded); `__x87` twins for float methods (unrounded
returns); `Math.Abs(float)` as a call rounds its argument; `0f - x` is `neg`; vector negation `fchs` (terraria-core);
`x op= tree` on array elements / properties; x87 bodies of 11 more XNA members (also at inlined sites for vector / f64
bodies); spills never before a declaration. Tools: `jit_disasm.py`, `jit_debuginfo.py`, `gen_jit_float_alloc.py`.

Left (entities_allow.txt, all last-bit): golf balls 741, 742, 744 (inlined Vector2 operator temporaries in
GolfHelper.ContactListener.OnCollision keep unrounded fields; not modelled), projectile 797 (dropped world item, tick 71),
NPC 426 (its projectile 581, tick 95). Item use on a server is ItemCheck of a remote player only: shooting, placing,
consuming (owner-client code) are not covered; a client-side item scenario (Main.myPlayer = the user) is the next step.

Gates (`sh tools/cs2rs/gates.sh --slow --skip window-modes`, regenerated): p1 3,911,380 exact, client-whole, rt 106, sim
(collision 62 of 62, light map 175,900 of 175,900 raw), ticks-player, ticks-world, host 109, app 102, viewer, scripts 14 of 14,
worldgen 6 of 6 at 108 of 108, entities npc / proj / item, multiplayer: all ok. window-modes skipped (the one gate with a
real window; this task allowed none). Browser: `web/ci/build_game_wasm.sh` (58,270,550 bytes), headless
`single_player_fast` in the world at frame 4,030 (`work\shots\f_14_in_world_600.png`), `run-host-console.sh` exit 0.

## Status snapshot 2026-10-14, later (float mode: real-JIT facts from every reference run)

The call / inline facts that made collision and lighting bit-exact now come from all 20 processes that wrote the
reference data, not from two. All gates green (`gates.sh --slow`), no float difference appeared or disappeared.

Ground truth tooling (new, `tools\refdump_worldgen`): `build_refjit.cmd` builds RefJitP1 / RefJitLiquid / RefJitSky
(`RefJit.cs`), RefJitTicks (`TicksJit.cs`), RefJitWorldgen (`WorldgenJit.cs`, Gen.cs) and RefJitWorldgenX
(`WorldgenXJit.cs`, GenX.cs, LAA): each is a harness's own sources plus an entry point that sets its statics (data to
`work\jit\ref\<scenario>\data`), calls its Run, then `JitDump.cs DumpRan` reads every TerrariaServer / ReLogic / XNA
method that has jitted code (CLR code header: `[entry-4]` MethodDesc, `[entry-8]` GC info whose first value is the
exact code size; nothing compiled for the dump; calls through a not-yet-compiled callee's MethodDesc slot named;
`jit_targets.json`). CollisionJit / LightJit use DumpRan too. `run_refjit.sh` runs all 20 (about 20 min), then
`refjit_compare.py`: every refdata file byte-identical, apart from wall-clock fields (timing.json, `*Ms`, matrix
summary date / memory / .wld hash; the .wld data sections identical by `wld_diff.py`).
Generators (`tools\cs2rs`): `jit_calls.py call_answers / union` merge the runs; a method with different answers in two
runs is left out (`jit_refcalls_conflicts.tsv`: 3 methods, all on generic mscorlib callees). `gen_jit_refcalls.py`,
`gen_jit_bodies.py` read `work\jit\ref`; `gen_jit_calls.py` takes the reference runs' answer where one ran the method
(33 mscorlib pairs differ from the whole-game precompile) and work\jit\all elsewhere. Translator: `CS2RS_FLOAT_REPORT`
writes its allocation per body; `gen_float_candidates.py` -> `float_mem_candidates.tsv` (98 methods with more float
frame slots than the translation rounds; review list, nothing applied).

| | before | after |
|---|---:|---:|
| methods with refcall facts / pairs | 10 / 58 | 296 / 4,612 |
| call sites decided by the reference runs' code | 172 | 10,632 |
| XNA sites with the x87 body (jitBodies) | 11 | 147 |
| methods that ran (union of the runs) | | 4,256 (309 with float locals) |

Found on the way: (1) earlier dumps took a call through a callee's MethodDesc slot for "inlined" (NPC.AI calls
CanHit 69 times; named in the player run, 0 in the world run); (2) without the IL of always-inlined callees the
"inlined but still calls" answer of `FastRandom.Next(int, int)` was lost and the light map went to 3 / 7 / 1 differing
tiles (scramble surface / deep / hell): fixed in DumpRan; the `sim` gate does not fail on light-map differences.
Gates (`sh tools/cs2rs/gates.sh --slow`, regenerated, 12 generated files / 367 lines changed): p1 3,911,380 exact,
client-whole, rt 106, sim (collision 62 of 62, light map 175,900 of 175,900), ticks-player, ticks-world, host 109, app
102, viewer, scripts 14 of 14, worldgen 6 of 6 at 108 of 108, multiplayer, window-modes: all ok. Browser:
`web/ci/build_game_wasm.sh` (58,243,979 bytes), `run-host-console.sh` exit 0, headless `single_player_fast` in the
world at frame 4,030 (`work\shots\f_14_in_world_600.png`).

Open: floats of methods no reference run executes (client drawing, most item / projectile AI) still use the size rule
and jit_calls.tsv; a client-side reference harness would be needed to extend further. The 98 candidates are unreviewed.

## Status snapshot 2026-10-14 (float mode: the lighting reference run's own machine code; light map bit-exact)

Raw light map: 175,900 of 175,900 tiles bit-exact (928 differing, at most 4 ulp, before); `GetColor` exact as before.

Ground truth tooling (new): `tools\refdump_worldgen\bin\RefDumpLightJit.exe <data dir> <jit dir>` (`build_lightjit.cmd`,
`LightJit.cs`) = `tools\refdump_light\RefDumpLight.cs` unchanged + an entry point that calls its `Run` (data to
`work\jit\lightref\data`, never refdata), then dumps that process's code to `work\jit\lightref`.
`python tools\refdump_worldgen\colljit_compare.py work\jit\lightref\data refdata\light`: 21 of 21 identical.
`JitDump.cs DumpJitted` now writes `compiled_before` per method (ran in the process vs compiled for the dump).

What that run's code does at the mismatching tiles, and the rules (no generated file edited):
1. `Tile.slope()` is a call in `ApplyHellLight` / `ApplySurfaceLight`, inside the right operand of `||`: the x87
   values are spilled as floats on that path only (hell 447, edge_bottom 6). New config `jitRefCalls`
   (`tools\cs2rs\jit_refcalls.tsv`, `gen_jit_refcalls.py`: call / inlined per IL site in the methods that ran, game
   callees included, an inlined body that still calls counts as a call) decides rounding for those pairs; FloatAlloc
   marks them proven and Emit rounds at a conditional proven call itself (`{ v = v as f32 as f64; call }`).
2. `num4 += (float)localRandom.Next(0, 50) * 0.0005f`: Next(int, int) inlined, calls Next(int); num4 spilled
   before it: C05 for compound assignment, only at proven calls (scramble 7 / 7 / 3).
3. `floatInMemory` ApplySurfaceLight:num6 (stored like num4 / num5), ApplyHellLight:num7 (`0.7f - 0.2f` folded to 0.5f).
4. XNA `Vector4.Lerp` x87 body in `jitBodies` at `GetShimmerBaseColor` (`gen_jit_bodies.py` now reads collref and
   lightref); model against `refdata\light\shimmer.bin`: 0 of 888 points differ, 104 with the f32 template
   (scramble_shimmer 458 -> 0).

Gates: `sh tools/cs2rs/gates.sh` (regenerated): p1 3,911,380 exact, client-whole, rt 106, sim (collision 62 of 62
bit-exact, 1,240,000 exact; light map 175,900 of 175,900), ticks-player, ticks-world, host 109, app 102, viewer SHA-1
35b0971b..., scripts 14 of 14: all ok. `gates.sh --nogen --only worldgen`: 6 of 6 worlds, 108 of 108 rows each.

Open: `jitRefCalls` covers only methods that ran in the lighting run; the rest of the game keeps jit_calls.tsv / the
size rule (a dump of a ticks / AI reference run would extend it the same way).

## Status snapshot 2026-10-10 22:40 (port started; supersedes Phase 3-5 checkboxes where they differ)

User goal set 2026-10-10: "Finish the game almost 1:1". Honest size: 19,783 hand-written methods; this is many milestones.

Architecture decided (details in `port\PORTING.md`):
- The port is a line-by-line translation. Statics become fields of `terraria_sim::Game`; one Rust module per C# class;
  names by one fixed snake_case rule; anything not ported yet is an `unported!("Class.Method")` call at the original call site.
- Rendering keeps XNA's model: `terraria-app\src\gfx` records `SpriteBatch.Begin/Draw/End`, `SetRenderTarget`, `Clear` into a
  draw list and replays it with wgpu passes on Bevy's device, into `Rgba8Unorm` targets (no sRGB, premultiplied alpha, like XNA).
  The original drawing code can then be ported call for call. Verified: `work\shots\m0.png`.
- Game data is not retyped: `tools\refdump` runs the game's own initialisation headless (TerrariaServer by reflection) and dumps
  tables to `refdata\`; `tools\gen_tables.py` turns them into the `terraria-data` crate.

Crates in `port\` (Bevy 0.19.1, toolchain stable-x86_64-pc-windows-gnu, app builds into `target-app\`):

| Crate | Contents | State |
|---|---|---|
| `terraria-core` | `Tile` (original bit layout), `TileMap`, `UnifiedRandom` (bit-exact vs .NET), `Framing` (8-way + wall) | tests pass; XNA value types being added |
| `terraria-data` | generated: 12,465 ID constants, 41 `Main` arrays, 518 `Sets` tables | builds |
| `terraria-formats` | XNB reader (all 15,994 content files decode, 0 errors), WLD reader + writer (byte-identical round trip of `ref_small.wld`) | tests pass |
| `terraria-sim` | `Game` state, `unported!`, world load into state | builds; tile framing being ported |
| `terraria-app` | Bevy app, `gfx` SpriteBatch layer, content loader, hidden screenshot mode (`TERRARIA_HIDDEN=1 TERRARIA_SCREENSHOT=file.png`) | runs |

Reference artifacts: `work\refsave\Worlds\ref_small.wld` (real generator, seed 12345, small), `refdata\*.json` (12 files, two-run identical),
`reference\xna\*.cs` (XNA value types for exact maths).

Milestones (each ends with a check against the original):
- [x] M0 formats, data dump, renderer, RNG.
- [~] M1 see the world: tile framing (in progress), XNA types (in progress), then lighting, tile + wall drawing, backgrounds, liquids. Check: screenshot of the port vs the original at the same world position.
- [ ] M2 walk the world: Collision.cs, Player update (movement, gravity, jumping, slopes), camera, input mapping.
- [ ] M3 touch the world: item use (pickaxe, axe, placement), WorldGen.KillTile / PlaceTile / TileFrameImportant, item drops, inventory, UI basics.
- [ ] M4 things that move: projectiles, NPC core + first AI styles (slimes, zombies), combat, spawning, day/night.
- [ ] M5 the rest of NPC AI, bosses, events, town NPCs, buffs, mounts, wiring, liquids simulation.
- [ ] M6 world generation, seed-for-seed against `ref_small.wld`.
- [ ] M7 UI complete, audio, saving players and worlds, settings, multiplayer.

## Status snapshot 2026-10-11 (strategy change: translator first)

- Proven: `tools\cs2rs` (Roslyn + semantic model) translates C# to Rust mechanically. Pilot: translated `Item.SetDefaults` equals the real
  game on all 6,195 items x 119 members (737,205 values), 0 mismatches. Design: `tools\cs2rs\DESIGN.md`. Output crate: `port\crates\terraria-gen`.
- Decision: the translated crate becomes the simulation of record. Hand-porting stops once the workers already running finish.
  The hand ports that exist (tile framing: 0 mismatches on 5,040,000 tiles; lighting: 0 on ~180k packed colours; collision: 48 of 62
  families bit-exact, the rest x87 rounding; tile/wall/tree drawing) stay as oracles and as the visible prototype until the
  translated code replaces them.
- Known limit, measured: the original runs on the legacy x86 JIT (x87). Literal f32 code differs in last bits in some results.
  PORTING.md has the rule. The translator is testing an f64-temporaries mode against the collision dump.
- Work now: (1) scale the translator to Player, NPC, Projectile, WorldGen, Collision, Wiring, Liquid, Utils, Lighting, with a collector,
  interface dispatch, and a generated host API list (`tools\cs2rs\HOST_API.md`) for everything XNA/ReLogic/System; verified on four
  real-game dumps. (2) sky/backgrounds/time and (3) liquids, hand-ported, finishing.
- Then: implement the host API natively on the existing `gfx` SpriteBatch layer (graphics, content, input, audio, file IO), translate
  `Main` (update + draw) and `GameContent`, and switch the app from `terraria_sim::Game` to the generated statics.
- Final check for the simulation: translated world generation with seed 12345 must reproduce `work\refsave\Worlds\ref_small.wld`.

## Status snapshot 2026-10-11 (whole game compiles)

- Whole game translated: 2,776 files, 1.13M lines of Rust, 23,077 bodies (22,532 clean, 182 refused, 335 not emitted), 0 rustc errors.
  Crate `port\crates\terraria-gen-game`; config `tools\cs2rs\game.json`; generation 25 s; check 3 min; test build ~5 min, 5.5 GB RAM.
- Host layer `terraria-host` (no Bevy): XNA graphics object model + command list, content, input, audio (waveOut mixer, XACT music, OGG/MP3),
  System.IO, JSON (byte-identical to Newtonsoft on the game's 87 files), regex engine, zip, PNG, platform "no Steam" answers.
- Shaders: `terraria-shaders` translates all 136 passes of the three effect files (D3D9 bytecode -> WGSL), verified interpreter vs GPU.
- App shell (`terraria-app\src\shell`): XNA `Game` loop order, readback, window commands, scripted input for hidden tests.
- Real-game oracles added: `refdata\worldgen` (106 passes, per-pass hashes + RNG + GenVars, x87 maths tables, float probes), `refdata\ticks`.
- Not running yet: translated `Main.Initialize()` stops at attribute reflection. Proofs 1-3 open.
- Floating point: original is x87; sin/cos/tan differ from libm; float locals keep extra precision depending on JIT register allocation.
  Work started on an x87-faithful maths crate and a tested float rule. This is the main limit on exactness.

Order of work from here:
1. Proof 1: translated startup == every refdata dump.
2. Client boot: translated `Main` under the shell until the title menu draws; then world load and in-game draw; compare with screenshots.
3. Proof 2: translated world generation == `ref_small.wld`. Proof 3: server ticks == `refdata\ticks`.
4. Play test by script: move, dig, place, fight; compare player trace with `refdata\ticks\player.*`.
5. Remaining refusals, memory/collector policy, performance, release build, saves, settings, multiplayer.

## Status snapshot 2026-10-11 (client: the translated game boots to the menus under the Bevy shell)

Client status. The machine-translated client (`terraria-gen-game`) runs inside the Bevy app (`terraria-app --bin terraria`,
feature `client`, profile `client`), hidden, driven by input scripts (`port\crates\terraria-app\scripts\g*.input`).

Reached, with screenshots in `work\shots` (run logs in `work\logs`):

| stage | state | evidence |
|---|---|---|
| 0 baseline | Proof 1 passes, viewer regression SHA-1 `35b0971b...` | `sh tools/cs2rs/p1.sh`; the command is `TERRARIA_WORLD=../work/refsave/Worlds/ref_small.wld TERRARIA_VIEW_TILE=2100,600 TERRARIA_UNLIT=1 TERRARIA_HIDDEN=1 TERRARIA_SCREENSHOT=<png> TERRARIA_SCREENSHOT_FRAME=60 ./target-app/debug/terraria-app.exe` |
| 1 client start-up, no GPU | done: `Program.LaunchGame` / `RunGame`, `new Main()`, `Main.Initialize` -> `ClientInitialize` -> `LoadContent`, 620 splash ticks (assets through the translated ReLogic content system), `Initialize_AlmostEverything`, `PostContentLoadInitialize`, menu ticks: 0 stops, 0 gaps, 0 unimplemented `void` host members | `sh tools/cs2rs/p1.sh --client` (survey), `--client-whole` (single calls; a gate) |
| 2 title menu on screen | done | `g0_splash.png`, `g1a_language_select.png` (a first run shows the language list, as the original), `g1_title.png` |
| 3 menu navigation by script | done up to the world list: hover highlight, Single Player, New, character creation, name prompt (typed text), the `.plr` written (AES) and listed again from disk, world list with `ref_small` | `g1b_title_hover.png`, `g2_player_select.png`, `g3_character_creation.png`, `g3b_name_prompt.png`, `g3c_player_list.png`, `g4_world_select.png` |
| 4 in-game | STARTED, not reached: the world loads (`WorldGen.playWorld` -> translated `WorldFile.LoadWorld` of `ref_small.wld`, `Main.gameMenu == false`), then stops (below) | `sh tools/cs2rs/p1.sh --client` with `CLIENT_SCRIPT` / `CLIENT_WORLD` |
| 5 stability / cost | not started (menu numbers only: README of `terraria-app`) | |

Next stops, in the order they are met when entering the world (each is a missing piece, none is papered over):

1. `new MemoryStream(byte[])` must WRAP the array (`CachedBuffer`: a `BinaryWriter` over the stream writes the packet
   bytes into the shared `byte[]`; `rt::io::Stream::from_bytes` copies and is read-only) -> `Stream was not writable` in
   `Player.Spawn` (`CreativePowers` -> `NetPacket`). Runtime: a stream backed by an `Arr<u8>`.
2. `unsafe` / `fixed` pointer code is not translated (15 refused bodies, "statement: None"): `LiquidRenderer.InternalPrepareDraw`
   is the first one drawn; `TileBatch.RenderBatch` (`sizeof`), lighting and map code follow.
3. Whatever `Main.DoUpdate` / `DoDraw` reach in-game after that: not surveyed yet (the headless scripted test lists them:
   `CLIENT_SCRIPT=port/crates/terraria-app/scripts/g6_enter_world.input CLIENT_WORLD=work/refsave/Worlds/ref_small.wld sh tools/cs2rs/p1.sh --client --nogen`).
4. Known, not on the path yet: `Stream` close hook (`CryptoStream` on `Dispose`: achievements file; compressing
   `DeflateStream`: map save), `IVertexType` for `VertexStrip`, `IEnumerable<T>.GetEnumerator()` called by hand, `Type.GetField`
   on XNA classes (`GraphicsUtils`), JPEG, sockets (`TcpClient` is an opaque object), the RGB vendor SDKs (left out by
   config, `game.json`), `float` register forms of `Math.Sin(x) * y` (`terraria_x87::*_ext`: the plain double forms are
   mapped now), `yield` inside `try`, local functions, tuples, `ref` reassignment (still refused; not reached so far).

Visibly wrong or missing in the screenshots: nothing found in the menus after the one defect seen and fixed
(`StringBuilder.ToString()` through `object` printed the Rust type name in the character creation description and in
the game tip; fixed, and `g3_character_creation.png` was taken again with the fix). Not verified against captures of the real game
(none exist in `refdata` for the menus): layout, fonts and colours look right by eye only. Audio was muted in every run
(`TERRARIA_MUTE=1`); the title music cue path (`LegacyAudioSystem`) ran without stops but was never listened to.
The first click of the scripts lands on the sun, which is behind "English" at that moment: the game grabs the sun and
awards "Going Oldschool", exactly as it would for a player clicking there (the popup is in `g1_title.png`).

## Status snapshot 2026-10-11 late (client: in-game, scripted play, tick fidelity against the real server)

The machine-translated client now enters the world and is played by script, hidden, through its own input code.
Scripts: `port\crates\terraria-app\scripts\` (`make_play_script.py` writes `g7_play.input` and `g12_stability.input`;
`run_hidden.sh <script> [name]` runs one session on a fresh save root `work\port-save-runs\<name>`). Screenshots
`work\shots\g5..g12*.png`, logs and state dumps `work\logs\<name>.log` / `.state.jsonl`.

| stage | state | evidence |
|---|---|---|
| A in-game | done | `g5_ingame_spawn.png` (player, 5 hearts, mana star, hotbar with the three copper tools, item name, minimap, buff icon, sky / mountains / clouds / tree / lighting, cursor); state dump: spawn at (33518, 3670), the position of the real server's `Player.Spawn`. Cross-check with the hand-ported viewer (`g5_viewer_spawn.png`): terrain identical once the 11-pixel camera difference is removed (viewer centres on the tile top, the game on the player), sky colour identical, the dark underground identical; 55 - 88 % of the dirt pixels bit-identical, patches differ by up to 36 / 255 (not diagnosed: one of the two light ports, or the light area's origin) |
| B scripted play | done, one session (`g7_play`): walk, jump, dig (grass tile gone, Dirt Block picked up, the three mushrooms the swing cut too), place it back, chop the tree (13 Wood, Acorn), inventory (Escape, pickaxe moved slot 1 -> 12 -> 1 by mouse), crafting list (Work Bench focused and crafted, placed: tiles 2129/2130,231 type 18), `Main.Moondialing()` to night, sword fight (slimes killed: Gel, copper coins; damage numbers; player hurt), map (M), settings window, Save & Exit to the title | `g6_walk`, `g6b_jump`, `g7_dig`, `g7b_place`, `g7c_tree`, `g8_inventory`, `g8b_craft_list`, `g8b_craft`, `g9_combat_*`, `g10_map`, `g10b_settings`, `g10c_title_after_exit` + `work\logs\g7_play.state.jsonl` |
| B save / load | the saved `.wld` parses with `terraria_formats::wld::read_world`; diff against the original: header time / weather / moondial cooldown / last-played, 15 tiles where the session worked, 3,679 tiles of natural change (liquid settling, growth: 60x world updates while the moondial ran); chests, signs, tile entities, town rooms identical. A second session (`g11_reload.input`, `KEEP_SAVE=1`) loads config, player and world again: `g11_reloaded.png` | `cargo test -p terraria-gen-game --test saved_world` with `SAVED_WORLD` / `ORIGINAL_WORLD` / `EXPECT_TILES`; `work\logs\g11_reload.state.jsonl` |
| C fidelity | `tests\ticks.rs` mirrors `tools\refdump_ticks` (same server path, pins, control flags). Player scenario: 600 of 600 ticks identical in all 45 compared fields (float bits included). World comparisons (the value `Next()` would return for `Main.rand` and all 16 `_rngs` streams after each of the first 120 ticks; tile-map FNV hash, tile / wall / liquid counts, 22 time and weather statics, 13 liquid counters, entity counts, NPC fields at the record ticks): player scenario 135 of 135, world scenario (3,600 ticks) 185 of 185 identical | `cargo test -p terraria-gen-game --test ticks` (`TICKS_SCENARIO=world` for scenario 1) |
| D stability / cost | 10,030 in-game ticks of walking, jumping, digging and fighting (4 deaths and respawns) without a stop; heap after a collection flat (5,807,793 -> 5,807,931 objects, 167.9 MB); poison + `gc-check` session of 4,370 in-game ticks clean; numbers in `port\crates\terraria-app\README.md` | `g12_stability.input`, `work\logs\g12_stability.log`; `CLIENT_POISON=1 cargo test -p terraria-gen-game --features gc-check --test client` with `CLIENT_SCRIPT=g7_play.input` |
| E runnable | `run-terraria.cmd` (visible window, saves in `work\port-save`). NOT run by the worker in visible mode (rule): the visible-window paths are untested | |

What was built for this (by layer):
- translator: pointer code (`fixed`, `*p`, `p->f`, `p[i]`, pointer arithmetic / comparison / difference, `sizeof`,
  pointer casts), `ref` reassignment and `_ = ref a[i]`, the decompiler's `M(out var x, out x)`, `List<T>.Enumerator`
  as a value, `ArraySegment<T>`, array covariance (`Derived[]` as `object[]`, `string[]` as `object[]`), `x.P++` on an
  impure receiver, `list.Sort()` of `IComparable<T>` classes, `Math.Abs(float)` unrounded inside x87 expressions,
  `floatInMemory` (measured exceptions to the float-allocation rule: one entry). Refused bodies 113 -> 62.
- runtime: `MemoryStream` over a caller's array wraps it (writable, not growable, `GetBuffer` / `Capacity` / zero
  fill as .NET); stream close hook (`CryptoStream`, compressing `DeflateStream` finish on `Dispose` and close the
  inner stream); `Enum.GetUnderlyingType`; array buffers aligned to 16; `fixed_arr`; scan blocks for the collector
  (handles inside a snapshot enumerator's buffer or a boxed lazy enumerator owned by a suspended iterator were
  missed: fixed, with a negative control); survey mode for refused bodies (`CS2RS_SURVEY=1`).
- host: fibers for queued work (a worker's `while (flag) Thread.Sleep(0)` wait for the main thread now gives the
  frame back: `WorldGen.playWorldCallBack` waits for `Main.DoDraw` to draw a loaded map); `File.Open(path, mode)`,
  `File.GetAttributes`, `StreamWriter(path, append)`, `TextWriter.WriteLine / Flush / Dispose`, `Control.Bounds`,
  `DynamicVertexBuffer.SetData(offset, ...)`, `GraphicsResource.Name` get, `Disposing` on textures, `ContentLost`
  on render targets, the sprite queue behind `GraphicsUtils.PendingDrawCallCount`.
- app: script commands for the game adapter (`dump ...`, `aim ...`, `game moondial`, `set time`; one implementation
  shared with the headless survey: `terraria-gen-game\src\host\debug_state.rs`), a fixed display for scripted runs
  (the game derives its default UI scale from the display height), the save root as working directory, collector
  triggered by allocation volume, timing output (`TERRARIA_FRAME_TIMES`).

Cost (details in the app README): `client` profile `Main.Update` 1.4 ms / `Main.Draw` 4.8 ms per tick in play,
`client-opt` (generated crate at opt-level 2: first build 24 min, largest rustc 5.4 GB, exe 265 MB) 0.67 ms / 2.05 ms;
GPU replay about 1 ms on the render thread; heap flat at 5.81 million objects / 168 MB; 139 KB of garbage per tick.

Known problems and next stops: `port\crates\terraria-app\README.md` "Known issues". The largest: the frame stalls
for a collection about every 6 seconds of play (0.13 s in `client-opt`, 0.3 s in `client`: the collector walks the
whole heap and the tile map is 5 million objects); achievements are not saved (no generated Newtonsoft member
tables for `Achievement`); loading screens do not animate; nothing was ever run in a visible window; no
networking; world generation from the menu (Proof 2) is untested; `IVertexType` (trails), the 62 refused bodies
and the deep cloner are still open, none reached by the sessions.

Gates at this snapshot (all green): `sh tools/cs2rs/p1.sh` (3,911,380 values exact), `--client-whole` (0 failures),
`cargo test -p terraria-gen-rt` (92), `--test sim` (collision 1,237,727 of 1,240,000 exact as before, lighting and
framing exact), `--test ticks` in both scenarios, `cargo test -p terraria-host` (95), `cargo test -p terraria-app`,
the viewer regression SHA-1, `cargo check -p terraria-gen` after `sh tools/cs2rs/run.sh`.

## Status snapshot 2026-10-11 night (play blockers; Proof 2: the reference world is reproduced)

Two blocks of work: (I) what breaks or spoils actual play, (II) Proof 2 (world generation). State per item, blunt.

### Block I

| item | state | evidence |
|---|---|---|
| 1 collector stalls | DONE. Incremental mark-sweep in slices with a store barrier (design and decision record: `tools\cs2rs\DESIGN.md` section 9). Pauses in the 10,000-tick stability script: `client-opt` before 127 ms mean / 163 ms max, one every 6 s; after 2,769 pauses of mean 1.49 / p50 1.50 / p99 1.73 / max 5.59 ms (the same in CPU time of the game thread). `client` before 286 / 457 ms; after mean 1.50 / p99 1.71 / max 5.15 ms. **The 4 ms target is missed by 6 of the 2,769 pauses (4.0 - 5.6 ms), all of them sweep slices**; mark slices stay below 2.1 ms, the start of a cycle below 0.7 ms, the end of a mark phase below 2.2 ms. Probable cause, not measured: single frees of large buffers inside one slice. Heap after a cycle 6.0 million objects / 180 MB (5.81 / 168 before: what dies during a cycle goes in the next one) | `rt::gc` tests (barrier, its negative control, verifier, model-checked incremental stress), `bench_pauses`; `work\logs\g12_stability.log`; the scripted play sessions with `TERRARIA_GC_VERIFY=1` (27 cycles, no report) |
| 2 vertex strips (trails) | MAPPED: `DrawUserIndexedPrimitives<T>` for a translated vertex struct goes to the host's raw-vertex path with the struct's own `VertexDeclaration` (template placeholder `{VD0}`); the one user in the game is `VertexStrip.DrawTrail` (all the trail drawers go through it; `TileBatch` / `SpriteDrawBuffer` use `DrawIndexedPrimitives`, which was mapped before). `g14_trails.input` gives Zenith, Light Disc and Magic Missile and uses them without a stop; the Zenith's strips are in `work\shots\g14a_zenith.png` (looked at), the other shots were taken (`g14b*`, `g14c*`; the Magic Missile one was looked at: missile and sparkles, the trail itself is faint). Not compared with the real game; Flamelash, Rainbow Rod and the Empress blades were not tried (they use the same call) | `g14_trails.input`, `game give` (the game's `Player.QuickSpawnItem`) |
| 3 deep cloner | NOT DONE, and not a play blocker: its only call site in game code is `Projectile.Damage` (Projectile.cs:12527) under `DebugOptions.DrawHitboxes`; the other users are the debug tools `Testing.StateSnapshot*`. Reaching it abandons that call (item 8) | `grep DeepClon decompile\Terraria` |
| 4 achievements | DONE. Member tables for `Achievement` and the condition classes (config `jsonTypes`, subclasses by derivation, written by run-time type, member order and BSON types as the game's Newtonsoft dll writes them). The file decrypts with the game's key and has the game's structure (137 achievements, 180 conditions, 8 with a `Value`); a second session does not award again | `terraria-host\tests\dotnet\achievements_probe.ps1` + `.expected`; host tests `achievements_bson_matches_newtonsoft`, `achievements_file_of_a_session`; `work\logs\g7_play.log` / `g11_reload.log` |
| 5 loading freezes the frame | DONE as a mechanism, NOT LOOKED AT. Work on a worker fiber gives the frame back at loop heads (`rt::yield_point`, emitted by the translator): after 8 ms in a normal run, after 1,000,000 loop iterations in a scripted run (repeatable; the script's clock stands still meanwhile). What makes that correct: `[ThreadStatic]` fields per work item (`Main.rand`), monitors with owners, no yield inside a static initialiser. Simulation results unchanged: Proof 1, `sim`, both tick scenarios and Proof 2 pass with the yield points in the code. No screenshot of an animating loading screen was taken or looked at | `rt::threads` tests, host `yield_tests` (4), gates |
| 6 walking speed | EXPLAINED, the client is right: Happy! buff (146) from the sunflowers at the spawn, local player only: 3 x (1 + 0.1) x 1.1 = 3.63; the server recording simulates a remote player without it. Code references and a real-client check in `port\crates\terraria-app\README.md` ("Fidelity") | state dump: `maxRunSpeed` 3.63, buff 146 |
| 7 audio clipping | DONE on the host side, NOT LISTENED TO. Nothing in XNA or the game limits the sum; the original hands a float mix to the system's audio engine. The mixer now sums in float and opens the device as 32-bit float (16-bit fallback with a soft knee). Statistics per run in the log: at most 8 voices at once, 739 of 11.2 million samples above full scale, mix peak +4.2 dBFS in the 10,000-tick session. The voice count is the game's own doing | `[shell] audio mixer:` line of every run; app tests |
| 8 robustness | DONE where scripts can reach. Normal run: a stop abandons that `Update` / `Draw` and the game goes on; a host member that is not implemented did nothing; both go to `port-crashlog.txt` (with the last 200 log lines) and into the window title; an uncaught C# exception ends the game as the original (`Program.DisplayException`, `client-crashlog.txt`, a real message box in a visible run). Scripted window events (`focus lost / gained`, `minimise`, `restore`, `resize`): found and fixed that a resize never reached the game (the host overwrote `PreferredBackBufferWidth`, which the game compares with the viewport to notice a resize). `sh tools/cs2rs/scripts.sh` runs every `g*.input` headless and fails on any stop; `sh tools/cs2rs/gates.sh` runs every gate and prints one table | `g13_window.input`, `work\shots\g13*.png`; the gates table below |

Found on the way and fixed: `[StructLayout(LayoutKind.Explicit)]` was ignored (union members of `ProjectileKey`,
`FloatIntUnion`, `TileSnapshot.TileStruct`, ReLogic's `ReinterpretCast` read zero); inline work that gave the
frame back came back under the wrong thread-static context and could not leave its lock (a start-up hang in one
build; regression test `inline_work_that_yields_keeps_its_context_and_its_lock`).

### Block II: Proof 2

| | result |
|---|---|
| passes identical | 106 of 106; all 108 rows (tile-map hash, column hashes, tile counts, next random number, surface / rock layer, spawn, dungeon, chest count) |
| first divergence | none |
| causes fixed | none were numeric. Four missing pieces stopped or distorted the run: a member table for a class without a parameterless constructor and `StringEnumConverter` (`WorldGenRange`), `string.Join<T>`, a get-only property not serialised (`FinalHash`), explicit struct layout (`HashWorld`) |
| final file | the server's own path (`serverLoadWorldCallBack`: generate, save, load) writes a `.wld` whose 10 data sections are byte-identical to `work\refsave\Worlds\ref_small.wld`; headers differ only in section pointers, GUID, creation time, last played and the manifest's durations; manifest `FinalHash` and every pass's `RandNext` / `Hash` identical |
| extra seeds / sizes | NOT DONE. The real harness has seed "12345" / small built in; no new mode was written, so no second world exists on the real side |
| world creation from the menu | NOT DONE. `UIWorldCreation` -> `WorldGen.CreateNewWorld` was not scripted or run |
| JIT disassembly tool | not needed (no divergence), not built |

Run and table: `tools\cs2rs\VERIFICATION.md` "Proof 2". 167 s for the pass-by-pass run in the unoptimised test build.

### Gates at this snapshot

One command, `sh tools/cs2rs/gates.sh --nogen`, after the last source change (both executables were built from
the same sources: `client` before the run, `client-opt` after it):

| gate | state | detail |
|---|---|---|
| p1 | ok | 11 dumps, 3,911,380 values exact |
| client-whole | ok | start-up as single calls |
| rt | ok | 100 tests |
| sim | ok | collision / framing / lighting as before |
| ticks-player, ticks-world | ok | both scenarios identical to the real server |
| host | ok | 101 tests |
| app | ok | 58 tests |
| viewer | ok | SHA-1 `35b0971bcf5e7ddfc1bba3d86445de457c79a41a` |
| scripts | ok | 11 of 11 `g*.input` without a stop (headless survey) |

Also green, not part of `gates.sh`: `tests\worldgen.rs` with `WORLDGEN_REQUIRE=108` and `WORLDGEN_REAL=1` + `wld_diff.py`
(Proof 2). Hidden sessions on the final builds: `g7_play` + `g11_reload` + `g14_trails` (`client`), `g12_stability`
(`client-opt`), `g6_enter_world` once on the real audio device at 3 % (32-bit float output opened).

Disk: the app's debug build directory had grown to 29 GB and the disk ran full during this work (3 GB free; a
session died of a failed allocation). Deleted, all regenerable: `port\target-app\debug\incremental`, the debug
test and demo executables in `port\target-app\debug`, `port\target-host`, `port\target-gen\debug` (38 GB in
all). 17 GB free at the end.

### Next stops

1. A visible run by a person (`run-terraria.cmd`): everything about a real window, real input, the sound and the
   loading screens is unseen. `work\port-save\port-crashlog.txt` and `last-run.log` say what went wrong.
2. Proof 2 beyond one world: a harness mode with seed / size / evil / difficulty as arguments
   (`tools\refdump_worldgen`, new file), then a medium world and a special seed through `tests\worldgen.rs` (it
   reads the rows by file name; the launch parameters are in one step). A divergence there is the first real use
   for the JIT disassembly tool.
3. World creation from the menu (item 12): script `UIWorldCreation` (name, seed "12345", the defaults), compare the
   file with `ref_small.wld` (same seed and options: it should be the same world), look at the loading screen.
4. The deep cloner (debug overlay only), `CreativeUnlocksTracker.ValidateWorld` (a translated function that calls
   itself; journey-mode worlds), the IME `INPUTUNION` layout, multiplayer.
5. Collector while work is suspended: a mark phase cannot end then, so a long generation keeps its garbage
   (5.5 GB in a hung run that was killed). Conservative scanning of fiber stacks would remove that limit.

## Status snapshot 2026-10-12 (completion stage: menu world creation, more worlds, multiplayer)

Details and evidence for every line: `tools\cs2rs\VERIFICATION.md` (sections dated 2026-10-12), design reasons:
`tools\cs2rs\DESIGN.md` ("Changes from the completion stage"). Blunt state per item.

| item | state | result |
|---|---|---|
| 1 world creation from the menu | DONE | `g15_create_world.input` (Small / Classic / random evil, name `ref_small`, seed `12345`): exit 0; the loading screen shows different progress texts (`work\shots\g15c_generating_00..03.png`, looked at); the created file's 10 data sections are byte-identical to `work\refsave\Worlds\ref_small.wld` (0 of 5,040,000 tiles differ, `FinalHash` equal); header differences: section pointers, GUID, creation time, last played, manifest durations, and `caveBackStyle[2..3]` (two random numbers a CLIENT leaves there: Main.cs:6791, WorldGen.cs:8181-8195; they differ from run to run) |
| 2 worldgen beyond one world | DONE, no divergence | new harness mode `tools\refdump_worldgen\bin\RefDumpWorldgenX.exe` (seed text, size, difficulty, evil, special-seed flags); it reproduces the existing `ref_small` outputs byte for byte; five new reference worlds in `refdata\worldgen\matrix` (57 MB): medium / expert / corruption, large / classic, not the bees, drunk, for the worthy / master. The translated generator: 108 of 108 rows identical for each (tile-map hash after every pass, column hashes, next random number). The JIT dumper exists (`jitdump` mode) and was shown on one method; it was never needed for a diagnosis, and deriving the float rule from its output per local is not automated |
| 3 translation completeness | PARTLY | refused bodies 62 -> 48, each of the 48 listed with callers and why it cannot run in this build; not emitted 326. `ValidateWorld` calling itself is the original's own IL. `INPUTUNION` layout honoured. Number formats for all 12 cultures from the real .NET (11,167 expected strings). Language switch scripted, CJK glyphs render. **NOT done: the deep cloner** (36 bodies, Reflection.Emit): its users are debug tools behind a switch nothing sets |
| 4 multiplayer | DONE for two scenarios | host sockets (`terraria-host\src\sys\net.rs`), the game's network threads as fibers, the dedicated server as `terraria.exe -server`. Port to port (`run_multiplayer.sh`): Host & Play + Join via IP on 127.0.0.1:47800, the joiner digs and walks, the host sees both, chat arrives, leave, save and exit: both exit 0. Port client against the ORIGINAL `TerrariaServer.exe` (`run_original_server.sh`, 127.0.0.1:47801): join, dig, walk, chat, leave, exit 0; the world the original saved has the dug tile. The original server announced itself on the LAN (UDP broadcast, about 35 s): not avoidable. Untried: everything else (see the app README, "Multiplayer") |
| 5 leftovers | PARTLY | fiber-stack scan: works (unit test, poison / `gc-check` sessions clean) but **costs a pause of 23 - 72 ms per cycle in multiplayer and up to 255 ms once during a world generation**; large buffers freed off-thread: **no measured improvement** of the sweep overshoot (8 pauses above 4 ms in CPU time against 6); yield-on / yield-off world files: same data; host members reached by the scripts: implemented (0 unimplemented, 0 gaps in every session); `gates.sh` has `--slow` (worldgen matrix, multiplayer) |
| 6 executables, documents | DONE | `client` and `client-opt` rebuilt from the final sources; this snapshot, VERIFICATION, DESIGN, GAME_REPORT (regenerated, 0 files changed on a second run), app and host README |

Mistake made and repaired during this stage: reinstalling the host tables (`coverage.py --install`) dropped a
mapping that had been installed by hand earlier (`DrawUserIndexedPrimitives<T>`: every trail), which the scripts
gate caught (`g14_trails`, 507 unimplemented calls). The mapping is now a rule in `coverage.py`.

### Gates at this snapshot

`sh tools/cs2rs/gates.sh --slow` after the last source change:

| gate | state | detail |
|---|---|---|
| p1 | ok | regeneration wrote 0 changed files; 11 dumps, 3,911,380 values exact |
| client-whole | ok | start-up as single calls |
| rt | ok | 102 tests |
| sim | ok | as before |
| ticks-player, ticks-world | ok | both scenarios identical to the real server |
| host | ok | 105 tests |
| app | ok | 58 tests |
| viewer | ok | SHA-1 `35b0971bcf5e7ddfc1bba3d86445de457c79a41a` |
| scripts | ok | 13 of 13 (`g1` .. `g16`) without a stop |
| worldgen (slow) | ok | six worlds (`ref_small`, medium, large, bees, drunk, worthy): 106 of 106 passes, 108 of 108 rows identical each; 111 - 470 s per world |
| multiplayer (slow) | ok | A exit 0, B exit 0 after 43 s |

### Next stops

1. A visible run by a person is still the first thing missing (`run-terraria.cmd`); now including the Multiplayer
   menu with a real keyboard (Backspace and Enter in the address / port / chat prompts were only scripted).
2. The pause of the conservative scan (an address-ordered heap index, or resolving in slices): it is the one
   thing that makes a multiplayer session less smooth than single player.
3. Multiplayer beyond the two scenarios: the original CLIENT against the port's server, more players, a medium
   world, a dropped connection, the server console.
4. The deep cloner only if the debug tools are ever wanted.

## Status snapshot 2026-10-12, later (first real-window test)

The client ran as a visible, presenting window for the first time, driven by window messages posted to that one
window (`port\crates\terraria-app\scripts\realwin\rw.py`, on top of the `bg-window-control` helper). The window
was shown without activation outside the desktop (`TERRARIA_NOACTIVATE=1`, `TERRARIA_WINDOW_POS=2800,1600`; the
desktop is one 2560 x 1440 monitor at 100 %). Nobody looked at it and nobody listened: the evidence is
`PrintWindow` captures (`work\shots\rw_*.png`), the game's state read through the shell
(`TERRARIA_COMMAND_FILE`), the saved files, the logs (`work\logs\rw_*`) and a loopback recording. Save root:
`work\port-save-realwin`.

One rule was broken once: the FIRST launch put the window on the monitor at (0,0) for about 20 seconds (the game
moves its window there itself and the shell applied it). It was not activated; the foreground window never
changed in any session (59,057 samples of `GetForegroundWindow` over all sessions, 0 with the client in front).
Fixed before the second launch; `rw.py launch` now kills the process if a window of it shows up on the desktop.

| step | state | result |
|---|---|---|
| 1 launch, language select, title, hover, click | works | `rw_40..42`, `rw_41_language_hover_strip.png` |
| 2 new character, typed name with Backspace and Enter | works | `rw_05..rw_12`; `Players\realwin.plr` |
| 3 the copied world in the list; a new small world from the menu with typed name and seed | works | loading screen with changing text and bars (`rw_17_gen_strip.png`), 20 s; `Worlds\rwworld.wld` |
| 4 play: walk, jump, hotbar keys and wheel, dig, chop, inventory, move an item, craft, place, right-click talk, map, chat | works | `rw_19..rw_29`; after Save & Exit and a new start the dug tile is gone and the bench is there (`rw_46_ref_small_reloaded.png`) |
| 5 input details | mixed | held keys repeat characters as the original; Backspace repeat follows the game's rule (10 in 60 ticks, 38 in 120, fixed-step run); Alt / Ctrl / Shift clicks, both Enter keys, double clicks, wheel in a list, mouse leave, resize at three sizes, focus lost / gained by messages (the game pauses and resumes), minimise / restore: work. A bare `WM_CHAR` is dropped (winit). Capitals, NumLock-on numpad, other DPI scales: not testable by messages on this machine |
| 6 frame pacing | fixed, then measured | 60 s in a world: frame interval mean 16.640 ms, p99 17.25, max 29.9; 98.9 % within 16.67 +- 1 ms; 60.2 game updates per second. Ten minutes: p99 38.8, max 250, 58.6 updates per second, bad in five of twelve minutes while another game was running on the machine (cause not proven) |
| 7 ten minutes of mixed play, Save & Exit, Exit | works | no stop, 0 unimplemented host members, 0 gaps, no `port-crashlog.txt`, exit code 0, files written |
| audio on the device, 19 s at 3 % | recorded, not heard | title: tonal (flatness 0.04), a beat every 0.60 s; with the music off each of six inputs is followed by a sound after 50 - 140 ms out of digital silence. The menu tick could not be separated from the title music |

Found and fixed:

1. **The game ran one `Update` per presented frame.** Terraria 1.4.5 paces itself in its `EndDraw` override, which
   the shell never called. Before: 60 updates per second idle, 87 while a key repeated, 143 while the mouse moved.
   `HostedGame::end_draw` now calls `Main.EndDraw` (not in a fixed-step run). `NewRuntimeMethods.GC_Collect`, a
   reflection call in it, is mapped to nothing in `tools\cs2rs\game.json`.
2. **A connected game pad stopped `Main.Update` every tick**: `GamePadState.IsButtonDown`, `.ThumbSticks`,
   `.Triggers` and their members, `new MouseState(...)` are implemented in `terraria-host` and mapped
   (`tools\coverage.py`). This machine has a pad device.
3. **`LinkedList<T>.AddAfter` was missing** (`FilterManager` with two active screen filters): 3,369 abandoned
   updates in the first session. `AddAfter` / `AddBefore` are in `terraria-gen-rt` with a test.
4. The mouse position was lost (0,0) when the cursor left the client area; it now stays at the last position.
5. bevy_winit calls `SetForegroundWindow` when `Window::focused` turns true on a window that is not in front:
   a `TERRARIA_NOACTIVATE` window keeps the field false and takes the focus from the `WindowFocused` messages.

New switches (app README): `TERRARIA_NOACTIVATE`, `TERRARIA_WINDOW_POS`, `TERRARIA_FRAME_LOG`,
`TERRARIA_COMMAND_FILE`, `TERRARIA_NO_GAMEPAD`, `TERRARIA_PRESENT_MODE`, `TERRARIA_VOLUME_FILE`. New tool:
`tools\loopback_capture`.

Open: bare `WM_CHAR` (IME text); the mouse outside the window; `Mouse.SetPosition` on the real pointer; pacing
when the machine is busy; a game pad with a button pressed and `SetVibration`; Backspace repeat not re-measured in
the real window after fix 1. The first regeneration of this stage also rewrote 4 files of the core set
`terraria-gen` that none of these changes touch: it was out of date.

### Gates at this snapshot

`sh tools/cs2rs/gates.sh` (fast gates) after the last source change; both executables rebuilt.

| gate          | state | time    | detail |
|---|---|---|---|
| p1            | ok   |   284 s | 11 dumps, 3911380 values exact |
| client-whole  | ok   |    13 s | start-up as single calls |
| rt            | ok   |     4 s | 102 tests passed |
| sim           | ok   |   108 s | 1 tests passed |
| ticks-player  | ok   |    30 s | 1 tests passed |
| ticks-world   | ok   |    61 s | 1 tests passed |
| host          | ok   |    26 s | 105 tests passed |
| app           | ok   |   419 s | 58 tests passed |
| viewer        | ok   |    19 s | SHA-1 35b0971bcf5e7ddfc1bba3d86445de457c79a41a |
| scripts       | ok   |   726 s | 13 of 13 scripts without a stop |

Disk: 7.6 GB (15 GB at the start: the rebuilt generated crate in three build folders, the recorder's 125 MB) free on C: at the end. Nothing was deleted.

### Next stops

1. A person in front of the window: Alt+Tab and back, the default borderless window over the monitor, Alt+Enter
   applied, the system cursor, capitals and another keyboard layout, a real game pad, how 60 frames per second
   look on the 143 Hz display, the sound by ear.
2. Text that does not come from a key press (IME, tools): winit drops it.
3. Pacing under load: find out what took the game thread off the processor in the bad minutes.

## Status snapshot 2026-10-12, input and window stage (what the first real-window test left open)

Everything was driven by programs again: unit tests without a window, hidden scripted runs, and the visible,
non-activated window outside the desktop (`rw.py`, now with `TERRARIA_FAKE_MONITOR`). Nobody looked at it, nobody
touched a key, a mouse or a pad. Proof that nothing was taken: 17 real-window sessions of this stage, 52,900
samples of `GetForegroundWindow` (`work\logs\rw_in_*.foreground.log`), 0 with the client in front; the guard
(now active for the whole run, not only the first 40 s: a client window inside the virtual screen or in front ends
the process) fired 0 times; the real pointer is never read or moved in a test run (`native::mouse_source`,
`set_position_action`: unit-tested). Details and switches: `port\crates\terraria-app\README.md` ("Input and focus in
a real window", "Known issues").

| item | state | result, evidence |
|---|---|---|
| 1 text input | DONE for what a program can send; the rest is on the human list | A message hook on the shell's own window (`shell\native.rs`), as the original's `WindowsMessageHook` / `WindowsIme`: text is every `WM_CHAR`, once; keys are the virtual keys of the key messages (layout aware, as `GetKeyboardState`); an input method's result string is read from `WM_IME_COMPOSITION`. Bevy's events are the source only where there is no hook. Real window: bare `WM_CHAR` for `A ! ß я 漢` arrive and are drawn (`work\shots\in_text_02_name_posted_chars.png`), key-typed `bc` after them arrive once each (`in_text_03_crop.png`), the player is saved as `A!ßя漢b.plr`, the chat holds `Hi A!ßя漢 ..xy` (`in_text_07_chat_sent.png`, `work\logs\in_text_focus.txt`). `WM_UNICHAR` gives nothing, which is the original's behaviour too. Headless: Shift+a, Shift+1, AltGr+q (German), dead key + vowel, Cyrillic, QWERTZ / AZERTY letters, numpad with NumLock on / off, both Enter keys, IME commit, through the Bevy adapter in a headless `App` and through the hook's message state. Found on the way: an emoji (outside the BMP) becomes two U+FFFD in the runtime's strings |
| 2 mouse outside the client area, `Mouse.SetPosition` | implemented, rule unit-tested, the real path UNPROVEN | XNA's `Mouse.GetState` is `GetCursorPos` + `ScreenToClient` at every call, focus or not (`reference\xna\Mouse.cs`). The shell does that in a normal run while its window is the foreground window or holds the capture; otherwise (and in every test run) the window's last mouse message. The game keeps the old position itself without the focus, so the difference is not visible to it. `Mouse.SetPosition` moves the real pointer only for the foreground window of a normal run. **The game never calls `Mouse.SetPosition` in 1.4.5.8** (the handoff assumed it does; no call in the decompiled source), and neither `GamePad.SetVibration` nor `GetCapabilities`. No real pointer was read or moved in any test |
| 3 game pad | DONE with synthetic input; no real device | `reference\xna\GamePad*.cs`, `XINPUT_*.cs` decompiled; the host keeps the raw XInput state and applies XNA's dead zones (7849 / 8689 / 30, three modes), `IsButtonDown` with the derived stick / trigger buttons, capabilities, vibration requests. Bevy's RAW gamepad events (gilrs) feed it; a headless `App` test injects connection / button / axis events. `g17_gamepad.input` (`pad connect`, `pad button`, `pad stick`, `pad trigger`: the same events into the same adapter): title menu by D-pad and A, the on-screen keyboard, into the world, walk, jump, use item, inventory, dead zone, disconnect; `PlayerInput.CurrentInputMode` 2 -> 4 -> 3 -> 4 -> 3 -> 2, button glyphs drawn (`work\shots\in_pad_02_title_down2.png`, `in_pad_07_name_typed.png`, `in_pad_15_inventory_cursor.png`; `work\logs\in_pad.state.jsonl`). The first pad button stopped `Main.Draw` (`GetType()` of a glyph snippet without reflection data): the four `TextSnippet` subclasses are in `game.json` `reflect` now, regenerated. Jump is the LEFT TRIGGER in the game's default profile, not A. `SetVibration` goes to Bevy's rumble requests; a scripted pad logs it as inert |
| 4 window modes | DONE against a fake monitor; nothing on the real one | `TERRARIA_FAKE_MONITOR`: one monitor of 1920 x 1080 outside the desktop, requests applied for real inside it. 20 steps, each with `GetWindowRect` / `GetClientRect` / style bits, the game's own view and a screenshot (`work\logs\in_window_title.jsonl`, `in_window_maximized.jsonl`, `work\shots\in_win_*.png`): default borderless start over the monitor, Alt+Enter and back, borderless off (800 x 600 framed), 1280 x 720, the menu's fullscreen and back, borderless again, the HUD in the world at 1920 x 1080 / 800 x 600 / 1280 x 720, a maximised start, maximised -> borderless -> framed. Two bugs of the NORMAL run found by it: the game took its own resize request back (a window that started at 1280 x 720 stayed there: the default borderless start never happened; `game::set_window_follows_apply_changes`), and leaving fullscreen left a framed window of the monitor's size. Fullscreen is a borderless window over the monitor at the monitor's resolution (no exclusive mode in wgpu, no mode switch); borderless keeps the style bits (winit removes the frame through `WM_NCCALCSIZE`: client rectangle = window rectangle) |
| 5 focus | DONE, with a change of rule | XNA's `Game.IsActive` follows `WM_ACTIVATEAPP` and nothing else (`reference\xna\WindowsGameForm.cs`); the game's own `FocusHelper` adds the foreground window and the form's minimised state. The shell did it from `WM_NCACTIVATE` / `WM_SETFOCUS` and treated a minimised window as inactive: both replaced. Unit test over message sequences (Alt+Tab, click elsewhere, a window of the same program, task bar minimise, Win+D, UAC with and without `WM_KILLFOCUS`, lock screen, close): written from the documentation of Windows, NOT recorded. Real messages (`in_text_focus.txt`): `WM_NCACTIVATE` + `WM_SETFOCUS` alone do not activate, `WM_ACTIVATEAPP` does; minimised while active (`SW_SHOWMINNOACTIVE` and a posted `SC_MINIMIZE`): `IsActive` stays, drawing stops (frame log), `IsSelectedApplication` false; restore; the same while inactive |
| 6 pacing under load | measured; no fix found that holds | see below |
| 7 disk | DONE | 7.52 GB free at the start. Deleted `port\target-app\debug` (34.2 GB of files; 25.2 GB came back) and, later, its 8 GB rebuild. The app gates build without debug info now (`CARGO_PROFILE_DEV_DEBUG=0`: about 5 GB instead of 24); the viewer gate builds its binary on demand, as before. Nothing else was deleted. Free at the end: see the gates section |
| 8 finish | see the gates section | `client` and `client-opt` rebuilt from the final sources; `run-terraria.cmd` untouched |

### Pacing under load (item 6)

The "other game" of the earlier ten-minute run was another session's program on this machine, and other sessions
compiled during every measurement of this stage: there was no idle machine to measure on. Each line: 60 s, the
client in a world with player-like input; "loaded" adds 5 busy-loop processes at normal priority (6 logical
processors), started and killed by `pace_load.py`.

| build, variant | machine CPU without the synthetic load | updates/s, p99 interval, frames above 33.4 ms: without / with the synthetic load |
|---|---|---|
| before (old `client-opt`) | 44 % | 60.40, 17.0 ms, 0 / 57.22, 47.3 ms, 248 |
| after (final `client-opt`) | 86 % (other compilers) | 57.70, 50.4 ms, 143 / 59.06, 45.7 ms, 115 |
| `client` build, as before this stage (task-pool schedules, coarse timer) | 69 % | 59.44, 21.1, 7 / 43.71, 53.1, 360 |
| `client` build, `timeBeginPeriod(1)` | 53 % | 60.39, 19.5, 1 / 50.17, 44.8, 198 |
| `client` build, one-thread schedules + `timeBeginPeriod(1)` | 62 % | 60.10, 22.5, 6 / 50.80, 48.3, 221 |
| `client` build, one-thread schedules, coarse timer | 60 % | 59.86, 19.4, 2 / 55.94, 43.0, 111 |
| `client` build, spin / sleep in turn (10 s blocks, one run) | 55 % | spin 60.08, sleep 57.51 / spin 54.92, sleep 47.10 |

What it says: the update rate follows the CPU the client still gets (it needs 1.16 cores: the game thread spins
through the rest of each frame as the original's does); two runs of the same configuration differ by more than
any switch does; sleeping instead of spinning is worse (the one comparison inside a single run). Kept:
`timeBeginPeriod(1)` (correct sleep lengths for the helper threads). Not kept: one-thread Bevy schedules, the
sleeping wait. The collector's long slices are preemption (wall-time budget), thread priorities are the original's
defaults. What the original does under the same load is unknown (it may not be started).
Logs: `work\logs\in_pacing_before.txt`, `in_pacing_ab2.txt`, `in_pacing_alt.txt`, `in_pacing_after.txt`
(`in_pacing_ab.txt` and `in_pacing_wait.txt`: runs spoiled by another session's build, kept for the record).

### Changed behaviour to know about

- A normal run (`run-terraria.cmd`) now starts as the original does by default: a borderless window with the
  monitor's size. Before, it would have been a 1280 x 720 borderless window in the corner. Nobody has seen either.
- `rw.py focus on | off` posts `WM_ACTIVATEAPP` too; without it the game stays inactive now.
- A minimised real window no longer counts as inactive by itself (XNA's rule); a scripted `minimise` still does.
- Hidden and scripted runs are unchanged (1280 x 720, Bevy events replaced by the script, no hook).

### Only a person can still check

1. Real focus: Alt+Tab away / back, click on another window, Win+D, UAC prompt, lock screen, task bar minimise.
2. Real pointer: position outside the client area and while dragging out, the hidden system cursor, buttons pressed
   outside the window.
3. Real key strokes: capitals, shifted symbols, AltGr, dead keys, a non-US layout, Caps Lock, numpad with NumLock.
4. An input method: composition, composition string in the field, commit; emoji input.
5. A real game pad: ranges and names as gilrs reports them, hot-plug, player order, vibration.
6. The real monitor: default borderless start, Alt+Enter, the video menu's switches, maximise / restore by the title
   bar, a second monitor, DPI scale other than 100 %; fullscreen at a resolution below the monitor's (not scaled:
   a difference to the original's exclusive fullscreen, which the port does not have).
7. The real message box, how 60 frames per second look on the 143 Hz display, the sound by ear, pacing on an
   idle machine and next to another game.

### Gates at this snapshot

`sh tools/cs2rs/gates.sh` (fast gates) after the last source change; `client` and `client-opt` rebuilt from the same
sources (08:59 / 08:58). The slow gate `window-modes` was not run through `gates.sh`; its two sessions were run by hand
with the final `client-opt` build (20 of 20 steps as expected).

| gate          | state | time    | detail |
|---|---|---|---|
| p1            | ok   |   514 s | 11 dumps, 3911380 values exact |
| client-whole  | ok   |    24 s | start-up as single calls |
| rt            | ok   |     3 s | 102 tests passed |
| sim           | ok   |   166 s | 1 tests passed |
| ticks-player  | ok   |    18 s | 1 tests passed |
| ticks-world   | ok   |    52 s | 1 tests passed |
| host          | ok   |    23 s | 106 tests passed |
| app           | ok   |   760 s | 98 tests passed |
| viewer        | ok   |     4 s | SHA-1 35b0971bcf5e7ddfc1bba3d86445de457c79a41a |
| scripts       | ok   |   659 s | 14 of 14 scripts without a stop |

The app gate has 98 tests now (58 before: the shell unit tests are compiled into each of the five binaries); the
script suite has `g17_gamepad`.

Disk: 21 GB free on C: at the end (7.5 GB at the start). Deleted: `port\target-app\debug`, twice (the 34 GB folder at
the start, its 8 GB rebuild before the gates); the gates built it again without debug info. Nothing else.

Not started / half-done: nothing is half-done in the tree. Not done: a run on an idle machine (item 6); the slow
gates `worldgen` and `multiplayer` were not re-run in this stage.

## Status snapshot 2026-10-12, evening (the multiplayer collector pause)

Next stop 2 of the completion stage: the pause of the conservative scan of suspended stacks. Code:
`port\crates\terraria-gen-rt\src\rt\gc.rs` (`Marker::resolve_candidates`, `Heap::wide`, `sweep_work`); design:
`tools\cs2rs\DESIGN.md` ("The collector can finish while generated code is suspended").

Measured first (new diagnostics: `rt::gc_profile` / `gc_profile_report`, printed by the shell and the server at
exit as `gc profile:` lines). The stack scan itself is cheap: about 140 distinct words (390 on the server), 4 - 18 KB
of stack, under 0.05 ms. The cost was resolving those words against the heap: every header of 1.6 - 2.6 million
objects read (a cache miss each), then about 740,000 "wide" objects (arrays, lists) read again for their buffers.

| change | effect (mp_a, end of a mark phase) |
|---|---|
| before | 33 ms mean, 46 ms max wall; 44 ms max CPU of the game thread |
| 1 the object pass reads addresses only (filter on the page), wide objects on a list of their own | resolution 20 - 40 ms: the wide list dominated |
| 2 a wide entry caches its range (large payload at allocation, array buffer when `alloc_init` makes it); lists read every time; unmarked entries dropped in sweep slices, not in the pause | 5 - 8 ms wall; resolution p50 6.6 ms, 38k headers read instead of 460k |
| 3 words already resolved in the cycle are tested only against moving (list) buffers | no measured gain: the network fibers run between slices, every resolution had new words. Kept (correct, tested, helps when stacks stand still) |
| 4 address-ordered index built in sorted runs during mark slices | REJECTED and reverted: 60 - 110 ms CPU per cycle to build against two passes of about 5 ms |

Result in the cleanest run (`gcfresh`, the final code): end of a mark phase 5.1 - 7.2 ms wall on the hosting
client (A), 11 - 15 ms on the joiner (B), 11 ms on the dedicated server (6 million objects); game-thread CPU max
14.3 ms (A), 11.3 ms (B) against 43.8 / 43.9 before. Single player is still better (1 - 3 ms): the remaining cost is
one sequential pass over the object addresses and the wide list per resolution, two resolutions per cycle.

Measurement caveat: other sessions compiled on this machine during every run (70 - 90 % CPU); wall-clock pauses
include preemption and two runs took 2.5 - 7 times their normal length. Game-thread CPU time is the reliable number.
Logs: `work\logs\mp_a.before-gcfix.log` / `mp_b.before-gcfix.log` (before), `mp_a.after-gcfix.log` /
`mp_b.after-gcfix.log` (the multiplayer gate on the final code, a loaded machine).

Correctness: unit tests with negative controls (each fails when its path is switched off):
`conservative_words_keep_objects_by_any_inner_address` (a word into an object's start, 15 KB into a 20 KB string,
element 90,000 of an array, item 1,500 of a list, a small array; incremental and whole cycles; reuse after free),
`an_old_word_under_a_moved_list_buffer_keeps_the_list` (a list buffer moves under a word resolved earlier in the
cycle). The existing fiber test got a deep-array case; it cannot tell the paths apart (the array's own handle stays
on the fiber stack). Verifier run: `TERRARIA_GC_VERIFY=1` multiplayer (`mpv_a`, `mpv_b`), 17 ends of a mark phase
checked by an independent walk, nothing missed, both exit 0.

### Gates at this snapshot

`sh tools/cs2rs/gates.sh --nogen` after the last source change (no translator change, so no regeneration):

| gate | state | detail |
|---|---|---|
| p1 | ok | 11 dumps, 3,911,380 values exact |
| client-whole | ok | |
| rt | ok | 104 tests (2 new) |
| sim, ticks-player, ticks-world | ok | |
| host | ok | 106 tests |
| app | ok | 98 tests |
| viewer | ok | SHA-1 35b0971bcf5e7ddfc1bba3d86445de457c79a41a |
| scripts | ok | 14 of 14 |
| multiplayer (slow) | ok | A exit 0, B exit 0 |

Not re-run: `worldgen` (slow; the collector change does not touch generation output, but the gate was not run).
`client` and `client-opt` rebuilt from the final sources. Nothing was deleted.

### Next stops

1. A person in front of the window (unchanged, see the earlier snapshots).
2. If multiplayer smoothness still matters after that: the remaining 5 - 15 ms is one pass per resolution;
   resolving at the cycle's start inside budgeted slices (cursor over the object list) would leave one pass in the
   pause.
3. Multiplayer beyond two scenarios (original client on the port's server, more players).

## Status snapshot 2026-10-13 (float mode: the real JIT's inlining of mscorlib calls)

`CanHitLine` (line of sight of NPC AI, projectiles, items) had 16 of 20,000 calls with a different DECISION than the
game. Cause: the translator's float model (`tools\cs2rs\src\FloatAlloc.cs`) took `Math.Sign(int)` for a call
instruction and rounded the x87 values live across it to float; the real JIT inlines it (no call, no rounding).

Ground truth tooling (new): `RefDumpWorldgenX.exe jitdump <dir> <list>` now accepts `*::*` (the whole game: 19,008
methods in 54 s; Steam / social types excluded), compiles the game's callees first, and writes `jit_callees.json`,
`jit_method_callees.json`, `jit_cells.json`; `tools\refdump_worldgen\jit_calls.py` names every call instruction;
`tools\cs2rs\gen_jit_calls.py` writes `tools\cs2rs\jit_calls.tsv` (config `jitCalls`), which `FloatAlloc` reads:
at a call site where the real JIT inlined an mscorlib callee without float values, nothing is rounded.

Measured findings (not assumptions):
1. The original's inlining of GAME methods depends on compile order: `CanHitLine` inlines `Tile.active()` when it
   is not compiled yet and calls it when it is (1 against 5 float locals in memory). The original has no single
   answer there, so only mscorlib (precompiled, order-independent) is taken from the table.
2. Taking the table for register allocation too breaks `HitLineWall`; adding rounding at calls the size rule
   thought inlined breaks `CheckAABBvLineCollision6` and the light map. Both not adopted; the size rule stays the
   input for allocation.
3. An inlined FLOAT callee (`Vector2.Length`, `Utils.RotatedBy`) keeps its own intermediates unrounded: that is
   what the remaining four families (`AimingLaserScan` 595, `CheckAABBvLineCollision6` 503, `CheckLinevLine` 859,
   `TileCollisionInStepsOf16` 300 calls of 20,000; float bits only, no decision differs) are made of.

Result: 58 of 62 collision families bit-exact (57 before), 0 decision differences (16 before), light map unchanged
(928 tiles, at most 4 ulp; `GetColor` exact). Gates (`gates.sh --nogen` after regeneration): all ok; p1 3,911,380
values exact, ticks identical, scripts 14 of 14. Slow `worldgen` re-run on the collector change (before this):
6 of 6 worlds, 108 of 108 rows each.

Next for float fidelity: x87-precise variants of the inlined float callees (`Vector2.Length`, `Utils.RotatedBy`,
`MathHelper.Lerp` ...) used at sites where the JIT inlines them.

### Multiplayer: three players and a server password (2026-10-13)

`sh crates/terraria-app/scripts/run_multiplayer3.sh` (from `port\`; scripts `mp3_*.input` written by
`make_mp3_scripts.py` from the two-player ones; runner: `run_multiplayer.sh` with `HOST_SCRIPT` / `JOIN_SCRIPT_C`):
A hosts with password `potato7` (the dedicated server gets `-password "potato7"`), B and C join via IP, see the
original's "Server Requires Password:" screen (`work\shots\mp3_c_password.png`), type it, enter; A sees three players
("Third has joined.", `mp3_a3_three_players.png`), B and C dig on either side of the spawn, A sees both tiles go,
chat reaches both, both leave, A saves and exits. All three exit 0, the server ends by itself (120 s). New wait
condition `menumode K` (`terraria-gen-game\src\host\debug_state.rs`).

### Collector: the start-of-cycle resolution in slices (2026-10-13)

`Marker::resolve_candidates` now runs as a `Resolution` with cursors over the object list and the wide list,
16,384 entries per piece, within the slice budget (DESIGN.md "The collector can finish while generated code is
suspended": safe because both lists only grow during a mark phase and objects made then are born marked; a list
buffer that moves under a resolved word is caught at the end of the mark phase). Test with a negative control:
`a_resolution_over_several_slices_keeps_what_the_words_hold` (fails when a resumed resolution skips the rest).
Multiplayer (`run_multiplayer.sh`, machine at 57 % CPU from other sessions): worst mark slice 11.4 ms wall (26
before), game-thread CPU max 7.9 ms host / 6.7 ms joiner (14.3 / 11.3 before; 44 at the start of the stage), end of
a mark phase 5.4 - 8 ms. Gates `gates.sh --nogen`: all 10 ok (rt 105 tests, scripts 14 of 14).

## Status snapshot 2026-10-14 (float mode: the collision reference run's own machine code; 62 of 62 families)

The four remaining collision families are bit-exact. The earlier reading ("inlined float callees keep their
intermediates") was wrong: it came from a separate jitdump whose calls into XNA went through unnamed stubs.

Ground truth tooling (new): `tools\refdump_worldgen\bin\RefDumpCollisionJit.exe <data dir> <jit dir>`
(`build_colljit.cmd`, `CollisionJit.cs`) is `tools\refdump_collision` compiled in unchanged with another entry point:
it repeats the whole reference run (InitGame, LoadWorld, stamps, all 62 families x 20,000 calls; data to
`work\jit\collref\data`, never refdata), then dumps that process's code for every `Collision` method, the game
methods they call (two levels) and the XNA Vector2 / Vector4 members (`JitDump.cs` refactored: `DumpJitted`).
`python tools\refdump_worldgen\colljit_compare.py work\jit\collref\data`: 64 of 64 files byte-identical to
`refdata\collision`, so the dump in `work\jit\collref` is the code that produced the reference values.

What that run's JIT did at the mismatching sites (each modelled in Python against the 20,000 inputs first):
1. `Vector2.Length()` is a CALL in CheckAABBvLineCollision6, TileCollisionInStepsOf16, LaserScan, AimingLaserScan
   (and `Utils.RotatedBy` too, whose body matches the translation). XNA's compiled Length keeps `X*X + Y*Y` at double
   precision and rounds once after `fsqrt`; the f32 template rounded three times. AABB6: 503 -> 0 in the model.
2. `Vector2.Normalize(Vector2)` (called by `SafeNormalize`, inlined into AimingLaserScan) keeps `1 / length` unrounded.
3. `CheckLinevLine`: `num` in memory (`fstp dword [ebp-4]`), `num4` spilled as a float at the `newarr` helper call.
   Model: 0 of 6,485 decided cases differ (859 with the old rounding).

Translator (no generated file edited): config `jitBodies` (x87 bodies of Length / Normalize) used only at the pairs
of `tools\cs2rs\jit_bodies.tsv` (`gen_jit_bodies.py` from `work\jit\collref`: caller / XNA callee with a call
instruction at every IL site, 49 pairs; the templates cover 7 sites); `floatInMemory` CheckLinevLine:num; new
`floatSpillAtAlloc` CheckLinevLine:num4 (FloatAlloc: an array creation is a spill point only for listed locals).

| family | before | after |
|---|---:|---:|
| AimingLaserScan | 595 | 0 |
| CheckAABBvLineCollision6 | 503 | 0 |
| CheckLinevLine | 859 | 0 |
| TileCollisionInStepsOf16 | 300 | 0 |
| total (62 families) | 1,237,743 exact, 58 bit-exact | 1,240,000 exact, 62 bit-exact |

Light map unchanged (928 tiles, at most 4 ulp; GetColor exact). `sh tools/cs2rs/gates.sh` (regenerated): p1
3,911,380 values exact, client-whole, rt 106, sim, ticks-player, ticks-world, host 109, app 102, viewer SHA-1
35b0971b..., scripts 14 of 14: all ok.
`gates.sh --nogen --only worldgen`: 6 of 6 worlds, 108 of 108 rows each.

Open, not done on purpose: the XNA bodies are proven for every call instruction, but `jitBodies` is applied only at
the sites the collision dump covers; the rest of the game keeps the f32 templates (XNA `LengthSquared`, `Dot`,
`DistanceSquared` even return unrounded sums). Extending it needs a dump of a run that exercises those sites
(ticks / AI) and their own gates.

## Status snapshot 2026-10-16 (float-rule coverage 92.8 %: secret seeds, gallery, interaction, multiplayer bytes)

Coverage of the real JIT's facts (tools/cs2rs/FLOAT_COVERAGE.md): 2,584 of 3,742 float methods, IL 5,150,585 of
5,547,702 bytes (92.8 %; before: 2,094 methods, 86.6 %); not covered 1,158 methods / 397,117 bytes (before 1,648 /
744,368). New reference runs, each compared bit for bit in the port (VERIFICATION.md, section of this date):
- worldgen: 10 more worlds (celebration, the constant, no traps, remix, zenith, skyblock, four with the 1.4.5 secret
  seeds) with JIT twins; 9 of 10 identical in every row; `ref_sec_frozen` differs from row 30 (dual dungeons, 4 cells);
- gallery (Gallery.cs): 123 scenes, 304,725 draw calls exact, 3,817 frame digests equal;
- interaction (Interact.cs): 211 scenes, 264,792 draw calls exact, 8,083 frame digests equal;
- multiplayer (NetServer.cs, NetClient.cs, NetVirt.cs): the original server and client in lock step, every message
  both ways; port server 1,519 of 1,519 ticks and client 1,519 of 1,519 frames byte-exact.

Translator / runtime / host changes (FLOAT_RULE.md section 10, items 17 to 25): hardware x87 trigonometry
(terraria-x87 hw.rs), exact zlib level-6 deflate (Ionic.Zlib's bytes), `Guid.NewGuid` pin, `CompareTo(object)` and
`Enum.HasFlag`, fact generators reading the server runs' generic names right, float-typed spills of float call
results, 8-byte-only Vector2 temporaries, both Vector2.Hermite overloads, `array.Length` not a call, innermost-binary
rule for stack locals waiting across a call, `LightningGenerator.Generate` as a measured body. Resources now come
from the install's Terraria.exe for native, gates and tests (`install_resources`).

### Gates at this snapshot

`sh tools/cs2rs/gates.sh --slow --skip window-modes` (regenerated): p1 3,911,380 values exact, client-whole, rt 106,
sim, ticks-player, ticks-world, host 111, app 102, viewer SHA-1 35b0971b..., scripts 14 of 14, draw, audio,
scenes, gallery, interact, net, worldgen 15 of 16 worlds (108 of 108 rows each; `ref_sec_frozen` FAIL, see above),
entities-npc 696, entities-proj 1,135, client-items 4,629, multiplayer: ok. entities-item failed once with an
out-of-memory (the item run never called the collector: its object list reached 4 GB); tests/entities.rs now
collects every 25 batches for every kind; `--only entities` rerun: npc 696, proj 1,135, item 4,629 exact.

### Next stops

1. `ref_sec_frozen` row 30: the dual-dungeon rooms and halls (`GenShapeDungeonRoom` circle shapes,
   `RegularDungeonHall`, `RegularDungeonRoom`) place / clear 4 cells differently; find what feeds them (positions,
   sizes) with a per-call trace on both sides (harness `REFDUMP_WATCH`, port side still to write).
2. Coverage tail (397 KB): display doll, gamepad UI, capture mode, menus no run opens, the dedicated server console.
3. Not a rule yet: a promoted Vector2 local / parameter kept on the x87 stack (item 23).

## Status snapshot 2026-10-16, evening (all 16 worldgen worlds exact; a reason for every float method without a reference; coverage run)

Stage 1, `ref_sec_frozen` row 30 (dual dungeons): every room, hall, door and platform of both dungeons is equal on both
sides (new structural dump: harness `REFDUMP_DUNGEON=<pass>`, port `WORLDGEN_DUNGEON=<pass>`, tests/worldgen_diag); the 4
cells are edge cells of HourglassRoom rows. Root cause (FLOAT_RULE.md section 10, item 26): the JIT substitutes a
caller's x87-stack local for a parameter of an inlined game method that the method never writes, so
`Utils.WrappedLerp(0, 1, percent)` inside `HourglassRoom.Perform` reads `percent` unrounded. New fact table
`jit_inlined.tsv` (gen_jit_inlined.py, 206 pairs), config `jitInlined`, translator twins `name__u` (51 twins, 39 call
sites). `ref_sec_frozen` 97 of 97 rows; all 16 worlds identical in every row.

Stage 2 (tools/cs2rs/FLOAT_COVERAGE.md, float_uncovered.tsv columns 7 / 8 by `float_reasons.py`): every uncovered float
method has a reason with evidence. New client reference run `cover` (Cover.cs / tests/draw/cover.rs, gate `cover`: 301
scenes, 431,161 calls exact, 3,887 frames, state lines equal): display dolls with every weapon kind and pose, hat
racks, doors, a trapdoor, a Christmas tree, minecart tracks, teleporters, a logic gate, inventory pages, the map saved
and loaded, every accessory, every mount, three skies, the Wall of Flesh, seven UI states, the lighting modes, the torch
god, the mystic sash, every sentry carried, a drunk world's music, the gamepad UI, three biome backgrounds, shimmer.
It found FLOAT_RULE item 27 (XNA Transform / Matrix multiply / CreateRotationZ evaluated as XNA's x87 code; config
`jitBodies`). Coverage 2,694 of 3,742 methods, IL 94.2 % (was 2,584, 92.8 %); not covered 1,048 methods / 323 KB:

| reason | methods | IL bytes |
|---|---:|---:|
| inlined into covered callers | 16 | 705 |
| dead (no caller, only dead callers, or behind DebugOptions.enableDebugCommands) | 140 | 52,419 |
| platform (Steam, WeGame, Workshop, RGB SDKs) | 102 | 20,381 |
| unrunnable (dedicated server console loop, pre-release-88 worlds, old map files, F7 diagnostics) | 18 | 17,206 |
| reachable, not run yet (caller and guard named) | 772 | 232,703 |

### Gates at this snapshot

`sh tools/cs2rs/gates.sh --slow --skip window-modes` (regenerated): 41 of 41 rows ok (VERIFICATION.md, section of this
date, "later"): p1 3,911,380; client-whole; rt 106; sim; ticks-player; ticks-world; host 111; app 102; viewer; scripts
14 of 14; draw; audio; scenes; gallery; interact; cover; net; worldgen 16 of 16 worlds; entities npc 696 / proj 1,135 /
item 4,629; client-items 4,629; multiplayer.
Browser: `web/ci/build_game_wasm.sh` (58,617,355 bytes), headless `single_player_fast` in the world at frame 4,030
(`work\shots\f_14_in_world_600.png`, log `work\logs\spf_final.txt`); play page `play_world.txt` 107 of 107 actions,
cursor 0 px; `run-host-console.sh` exit 0, 75 of 75 actions.

### Next stops

1. The "reachable, not run" list (772 methods, 233 KB; largest first in FLOAT_COVERAGE.md): gamepad instructions on a
   hovered slot (`ItemSlot.GetGamepadInstructions`), capture mode (`Main.DrawCapture`: render-target read-back and PNG
   encoding), a world seed with a desert floating island (`WorldGen.DesertCloudIsland`), PvP (`Projectile.StatusPvP`),
   the bestiary entry page, the housing query feedback.
2. Not a rule yet: a promoted Vector2 local / parameter kept on the x87 stack (item 23).

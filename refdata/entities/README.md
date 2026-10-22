# refdata/entities: every NPC type and every projectile type, simulated by the real dedicated server

Produced by `tools/refdump_entities/` (`run.cmd`; a .NET Framework 4 x86 console program that loads `work/refserver/TerrariaServer.exe` by reflection, as `tools/refdump_ticks` does). Headless, no window, no socket; the world is a scratch copy of `work/refsave/Worlds/ref_small.wld`. The JIT-dumping twin `RefJitEntities.exe` (`tools/refdump_worldgen/EntitiesJit.cs`, `run_refjit.sh` scenarios `entities_npc` / `entities_proj`) writes byte-identical files (`refjit_compare.py`), so the machine code in `work/jit/ref/entities_*` is the code that produced them.

| file | what |
|---|---|
| `npc.ents.zlib`, `proj.ents.zlib` | the dump (format below), zlib (RFC 1950) |
| `<kind>.summary.json` | batches, types, spawn results, errors, leaf counts, fields that are not leaves |
| `<kind>.timing.json` | wall clock (the only file that differs between two runs) |

Scenario (the comment at the top of `Entities.cs` is the full description): 696 NPC types (1 .. NPCID.Count-1) in 44 batches, 1135 projectile types in 37 batches, 31 slots per batch 115 tiles apart (a boss alone), one remote player per slot, 120 ticks. Before each batch: changed tiles restored, `NPC.ClearAll`, `Projectile.ClearAll`, fresh world items and slot players, `Main.rand = new UnifiedRandom(777000 + batch)`, `Main._rngs` emptied. One tick = the entity part of `Main.DoUpdateInWorld` in its `SwapRandom` scopes. Projectiles are owned by the server (255): the server's `NewProjectile` refuses any other owner. No type was skipped: every spawn call returned a slot (`errors` is empty in both summaries).

## Format (little endian)

```
"ENTS" u32 version=1  str kind  i32 ticks  i32 batches  i32 classes(=4)
per class (npc, projectile, player, item): str name  i32 n  n x (str leaf name, u8 kind: 0 int, 1 f32, 2 f64, 3 hash, 4 bool)
per batch:
  i32 batch  i32 seed  i32 tiles changed by the previous batch  u64 their FNV-1a 64 (x, y, record)  i32 slots
  per slot: i32 type, ex, ey (entity tile), px, py (player tile, Player.SpawnX / SpawnY), pl (player index), owner
            f32 vx, vy (projectile velocity)  i32 index (what the spawn call returned)
  per tick 0..ticks: i32 tick  u8 error (1: the tick threw, the batch ends)
     u32 Main.rand inext  u64 FNV-1a 64 of its SeedArray  u16 n  n x (str key, u32 inext, u64 fnv) of Main._rngs
     per class: u16 active  per active entity: u16 index  u16 n  n x (u16 leaf, u64 value)   (leaves that changed
                since the last record of that index in this batch; all zero before the first)
end: i32 -1  i32 tiles changed by the last batch  u64 their hash
str = u16 length + UTF-8.
```

Leaves: every instance field of the class and its bases (names with `<` left out) that is primitive / enum (`name`), a struct of primitives (`name.Field`), or a 1-D array of primitives / Vector2 (`name[]` = FNV-1a 64 of the u32 length and the elements' bytes, plus `name[i]` for arrays of at most 8 elements). WorldItem also `=type`, `=stack` (properties). NPC 195 leaves, projectile 109, player 1180, world item 32. Values: integers sign-extended, bool 0 / 1, float / double bits.

## citem: client-side item use (`citem.*`, 2026-10-14)

`RefDumpEntities.exe citem <outdir> 60` (`tools/refdump_entities/ClientItems.cs`, description at its top): the 4,629 item
types with a use style, one per batch, used for 60 ticks by the OWNING player of a single player game (`Main.myPlayer =
0`, `Main.netMode = 0`; `Main.dedServ` stays true, the server build loads no content) holding the left mouse button on a
fixed target (pick / axe / hammer: the ground two tiles right of the spawn tile; anything else: the air tile three right,
one up). Client pieces the server build lacks are set up the same way on both sides: screen 1600 x 900, a game view with
that viewport, the achievements, the lighting engine, a stand-in font (`FakeFonts`), `Main.ignoreErrors = false`. The
player is spawned with `Main.myPlayer = 255` meanwhile, then is the local player. Seeds 888000 + batch. Reset per batch:
the tiles of the reset region, chests / signs / tile entities created since the load, NPCs, projectiles, world items,
player 0. `RefJitEntities.exe` scenario `entities_citem` writes byte-identical files.

Format: the same as above with `kind = "citem"`, `ticks = 60`, one slot per batch, and:
```
after the class schemas: i32 screen width, height  4 x i32 tile box (x0, x1, y0, y1 relative to the spawn tile)
                         4 x i32 reset region (same)  i32 n  n x i32 ammo item types (inventory slots 10..)
slot record: ex, ey = the target tile; px, py = the spawn tile; pl = 0; owner = -1; vx, vy = Main.screenPosition;
             index = 0 (-2: the set-up threw)
after the slot record: i32 Main.mouseX, i32 Main.mouseY, i32 chests / signs / tile entities the reset removed
per batch the "tiles changed" pair counts the reset region only (every batch, the first too)
per tick, after the entities: i32 tiles of the box that differ from the loaded world, u64 FNV-1a 64 of (x, y, record)
```

Port side: `port/crates/terraria-gen-game/tests/entities.rs` (gates `entities` and `client-items` of `tools/cs2rs/gates.sh --slow`).

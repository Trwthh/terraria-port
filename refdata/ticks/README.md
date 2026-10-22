# refdata/ticks: reference data for the dedicated server's per-tick simulation

Produced by `tools/refdump_ticks/` (a .NET Framework 4 **x86** console program that loads the real `TerrariaServer.exe` from `work/refserver/` by reflection and calls the game's own `Main.Update`).
Headless, no window, **no socket** (`Netplay.StartServer` is never called). The world is a scratch copy of `work/refsave/Worlds/ref_small.wld` (sha256 `6c475077...1906`); the original and the game folder are never written.

| file | what | size |
|---|---|---|
| `world.*` | scenario 1: zero players, 3600 ticks | 4.9 MB |
| `player.*` | scenario 2: one scripted remote player, 600 ticks | 3.2 MB |
| `<scn>.ticks.jsonl` | world records: ticks 0, 1, 2, 3, 10 and every 60th (65 / 15 rows) | |
| `<scn>.columns.bin` | per-column tile hashes, one row per record | |
| `<scn>.rand_per_tick.jsonl` | all RNG states after **every** tick 1..120 | |
| `<scn>.tile_changes.bin` | every tile that differs from the loaded world at the last tick | |
| `<scn>.main_statics_changed.json` | primitive `Main` statics that changed between tick 0 and the last tick | |
| `<scn>.summary.json`, `<scn>.timing.json` | parameters, pins, notes; wall-clock timings (the only file that differs between runs) | |
| `player.per_tick.jsonl` | the scripted player's state after every tick | |

Python reader/verifier: `tools/refdump_ticks/summarize.py` (parses everything, recomputes `cols_fnv64` from the column file, prints records and the player trace).

## 1. How the real dedicated server advances (and what it does with zero players)

`Main.DedServ` (Main.cs:5372) loads the world on a worker task (`WorldGen.serverLoadWorld`, Main.cs:5853), then (Main.cs:5903-5960):

```
double num6 = 16.666666666666668;  Netplay.StartServer();  gameMenu = false;      // 5903-5906: StartServer opens the listening socket and starts the "Server Loop Thread"
while (!Netplay.Disconnect) {
    if (stopwatch + carry >= num6) {                                               // 60 Hz pacing with a carried remainder
        if (Netplay.HasFullyConnectedClients)  Update(new GameTime());             // 5920-5923  <- the simulation tick
        else { OnTickForThirdPartySoftwareOnly?.Invoke(); Netplay.UpdateInMainThread(); autosave handling }   // 5925-5940
        sleep to 16.67 ms
    }
}
```

**With no fully connected client the world does not advance at all** (no time, weather, liquids, NPCs, random tile updates): only `Netplay.UpdateInMainThread()` runs (it only drains `NetMessage.CheckBytes` for 256 buffers, Netplay.cs:96-102).
`HasFullyConnectedClients` is set by the socket thread (`UpdateConnectedClients`, Netplay.cs:313-335, client `State == 10`). Frame timing is a real-time pacing loop; nothing in the simulation reads the clock
(see section 4), so the harness simply calls the update in a tight loop.

One tick = `Main.Update(GameTime)` (Main.cs:16985; `protected override`, called by reflection) -> `DoUpdate` (17039-17503) -> `DoUpdateInWorld` (17942) -> `DoUpdateInWorld_Inner` (17960-18038).
For `dedServ`/`netMode == 2` `DoUpdate` does, in order: `UpdateWorldPreparationState` (-> `Ready`), `Netplay.UpdateInMainThread` (17106), `NPC.UpdateProtectedSpawnSlots`, the dedServ FPS counter, `DoUpdate_AutoSave`
(Stopwatch, 10 min: never reached in these runs), `cloudAlpha = maxRaining` (netMode 2, 17397), `UpdateWindyDayState`, delayed processes, `AmbienceServer.Update`, `updateCloudLayer`, `UpdateWeather` x `dayRate`
(Main.cs:59713-60019), `UnpausedUpdateSeed` advance, `Ambience`, `PortalHelper`, then `DoUpdateInWorld` if `ShouldUpdateEntities()` (`_worldPreparationState == Ready && !generatingWorld`) and `WorldUpdateStepper.ShouldUpdateWorld()`.
`DoUpdateInWorld_Inner`, each part inside its own `Main.SwapRandom(key)` (section 3): `UpdateParticleSystems_World`, `UpdateWorld_Players` (255 slots, `player[i].Update(i)` for active ones),
`NPC.RevengeManager.Update`, `UpdateWorld_SpawnNPCs` (`NPC.SpawnNPC`), `PressurePlateHelper.Update`, `UpdateWorld_NPCs`, `UpdateWorld_Gores`, `UpdateWorld_Projectiles`, `UpdateWorld_Items`, `UpdateWorld_Dust`,
`LeashedEntity.UpdateEntities`, `UpdateWorld_FloatingText`, `UpdateWorld_Time` (`UpdateTime`: time/day/night/blood moon/town NPC spawns), and (netMode 2) `UpdateWorld_WorldGenAndInvasion`
(`WorldGen.UpdateWorld` = wiring, tile entities, random tile ticks, liquid, plus `UpdateInvasion`) and `UpdateWorld_ServerOrClient` (`UpdateServer`: net counters, `SyncItem`, ...), `Chest.UpdateChestFrames`.

**Configuration chosen for the harness: `netMode = 2` (what `Main.Initialize` sets for `dedServ`), `myPlayer = 255`, `gameMenu = false`, `Netplay.Clients[0..255]` = fresh `RemoteClient`s, no listener.** This is the real
server with a client that has joined, minus the client. `netMode = 0` was rejected: it is the single-player branch (`Main.gameMenu`/`autoPause`/`PlayerInput`, no `UpdateServer`, different liquid constants:
`Liquid.UpdateLiquid` takes `cycles`/`curMaxLiquid` from the player count only when `netMode == 2`, Liquid.cs:996-1012). Nothing needs a socket: `NetMessage.SendData` loops over `Netplay.Clients[i].IsConnected()`, which is false for
fresh clients, so all broadcasts are no-ops. `Netplay.StartServer` (`InitializeServer`, Netplay.cs:235-290) is the only thing that opens a socket (`TcpListener`, `StartListening`, UPnP `OpenPort`, the broadcast thread);
the harness copies only its non-socket state changes (Netplay.cs:254-263).

## 2. Harness call sequence (`tools/refdump_ticks/Ticks.cs`, `Common.cs`)

```
RefDumpTicks.exe world  <outdir> 3600        scenario 1
RefDumpTicks.exe player <outdir> 600         scenario 2
```
1. `InitGame` (identical to `tools/refdump`): `Program.SavePath = work\ticks-scratch\<scn>_<outdir>\save`, `Main.dedServ = true`, `new Main()`, `Lang.InitializeLegacyLocalization()` [Program.cs:187-223]; before `Initialize`, as `-world <existing>` does
   (LaunchInitializer.cs:175-178, Main.cs:5308): `WorldFile.GetAllMetadata(scratch copy, false).SetAsActive()`; `Main.rand = new UnifiedRandom(0)`; `main.Initialize()` (Main.cs:5394; netMode becomes 2, `_rngs` is created, 6653).
2. `Task.Factory.StartNew(WorldGen.serverLoadWorldCallBack)` -> `WorldFile.LoadWorld()` on a thread-pool thread, exactly like Main.cs:5853 - with `Main.rand` of that thread set to `UnifiedRandom(1357)` first (it is thread-static, section 4).
   After the load: `Liquid.cycles/curMaxLiquid` = 10/0 (nothing settled), `Liquid.numLiquid = 0`, `LiquidBuffer.numLiquidBuffer = 171`, seed 12345, time 13500, day.
3. Server state without sockets (Netplay.cs:254-263): `myPlayer = 255`, `netMode = 2`, `Netplay.Disconnect = false`, 256 fresh `RemoteClient`; `gameMenu = false` (Main.cs:5906).
4. Pins (section 4): `Main.rand = new UnifiedRandom(424242)` on the main thread, 41 other generators re-seeded, `UnpausedUpdateSeed`, `TileFrameSeed`, `windCounter`, `extremeWindCounter`.
5. Row 0 = state right after load. Then for tick = 1..N: (player scenario: set the control flags for this tick) `Main.Update(new GameTime())` (the server loop's call, Main.cs:5923); after it: wind-counter pin check, RNG states for tick <= 120,
   a row for tick 1, 2, 3, 10 and every multiple of 60 (and the last tick). Exceptions from a tick would stop the run and be recorded in `summary.json` (`tick_error`): none occurred.
6. After the loop: final tile diff against the loaded world, `Main` statics diff, summary.

## 3. The RNG streams of one tick

* `Main.rand` is **`[ThreadStatic]`** (Main.cs:1367); everything below is on the main thread.
* `Main.SwapRandom(key)` (Main.cs:3385-3396): returns an `IDisposable` that swaps `Main.rand` with the generator `_rngs[key]` for the duration of a `using` block and restores it afterwards. `_rngs[key]` is created
  **on first use as `new UnifiedRandom(ActiveWorldFileData.Seed)`** (the world seed, 12345 here). So every part of the tick has its own stream that starts from the world seed, independent of everything else.
  Keys seen in the dumps (17): `DoUpdateInWorld`, `UpdateParticleSystems_World`, `UpdatePlayers`, `SpawnNPCs` (declared, `SpawnNPC` is the one used inside), `SpawnNPC`, `UpdatePressurePlates`, `UpdateNPCs`, `UpdateGores`,
  `UpdateProjectiles`, `UpdateItems`, `UpdateDust`, `UpdateLeashedEntities`, `UpdateFloatingText`, `UpdateTime`, `UpdateWorldGen`, `UpdateServerOrClient`, and `UpdateLiquid` (Liquid calls it itself).
  Streams that actually moved (first tick where `inext != 0`): zero players: `UpdateNPCs` 1 (the two town NPCs), `UpdateTime` 1, `UpdateWorldGen` 1, `UpdateLiquid` 40; with the player additionally `UpdatePlayers` 1 and `SpawnNPC` 1.
* Code that runs **outside** a swap uses the base `Main.rand`: `UpdateWeather`/wind (Main.cs:59713+), cloud layer, `Ambience`. It advances by 6 numbers per tick in these scenarios (`inext` 6, 12, 18, ...).
* The tick also has a `StateSnapshot`/`RecordReplay` machinery (`DoUpdateInWorld` wraps the work in `RecordReplay.RecordOrReplayInput()`, Main.cs:17945): inactive here.
* Weather/wind state at tick 0 (counters, `numCloudsTemp`, `cloudBGActive`, NPC spawn chances such as `NPC.fireFlyChance`) is set **during the load**, from the load thread's `Main.rand` (pinned) and the GUID-seeded wind counter (pinned),
  so a real server has random values there; they are included in the rows and `main_statics_changed.json` so a port can start from them.

## 4. Every unpinned source of randomness or timing found, and how it is pinned (all listed in `summary.json:rng_pins`)

| source | where | pin |
|---|---|---|
| `Main.rand` of the load thread: null on the thread-pool thread, so `LoadWorld` makes `new UnifiedRandom((int)DateTime.Now.Ticks)` | WorldFile.cs:652-655; Main.cs:1367 `[ThreadStatic]` | `Main.rand = UnifiedRandom(1357)` set inside the load task. Without this, `NPC.setFireFlyChance` (WorldFile.cs:774), `cloudBGActive`, weather counters differ at tick 0 on every start. |
| base `Main.rand` of the main thread (`new UnifiedRandom()` = `Environment.TickCount`) | Main.cs:5375 | `UnifiedRandom(424242)` after the load |
| 41 static / singleton `UnifiedRandom()` instances created with the tick count: `Cloud.rand`, `HitTile.rand`, `TilesRenderer._rand`, `AmbientWindSystem._random`, `Main._drawRand`, `Main._tempSeededRandom`, `Projectile._context.Random`, 33x `FlexibleTileWand.*._random`, `CustomSoundStyle.Random`, `LegacySoundStyle.Random`, `SoundStyle._random`, `MartianSky.Ufo.Random` | found by a reflection walk of all static fields, depth 3 | `SetSeed(9000 + n)` (the exact list is in `summary.json`). None of them influenced tiles, NPCs, items or projectiles in these runs (they feed visuals, sounds, tile-hit damage) |
| `Main.UnpausedUpdateSeed = (ulong)Guid.NewGuid().GetHashCode()`, advanced every tick, read nowhere else | Main.cs:6539, 17443 | set to 123456789 |
| `Main.TileFrameSeed = (ulong)Guid.NewGuid().GetHashCode()` (tile drawing / flame particles only) | Main.cs:420 | set to 987654321 |
| `Main.ResetWindCounter` draws `windCounter` / `extremeWindCounter` from `FastRandom.CreateWithRandomSeed()` = `new FastRandom(Guid.NewGuid().GetHashCode())`: at load (`clearWorld`, WorldGen.cs:6926) **and inside the tick whenever `windCounter` reaches 0** (Main.cs:59842-59918: wind target change every 900-2700 ticks) | Main.cs:59674-59681; FastRandom.cs:37-40 | not seedable: `windCounter = 1500`, `extremeWindCounter = 25` after the load; after any tick in which `windCounter` went up instead of down it is replaced by `900 + (n*773) % 1801` (n = number of resets so far). It happened at ticks 1500 and 2400 of the 3600-tick run (noted in `summary.json:notes`). Everything else of the wind (`windSpeedTarget += rand...`) uses the pinned `Main.rand`. A real server therefore has a random wind-change schedule; a port can only reproduce the dump by using these counters |
| `DateTime.Now`/`Stopwatch`: `Main.checkXMas/checkHalloween` (load), `DoUpdate_AutoSave` (10 min), `Player.lastDeathTime`, `dps*` timers | Main.cs:13323-13377, 19953+; Player.cs | nothing in a 3600-tick run reads them in a way that changes state; `halloween = true`, `xMas = false` (date 2026-10-11) are recorded in `summary.json`. Halloween affects drops/shops/`ShakeTree`, not the tick state dumped here |
| `WeightedRandom<T>()` default constructor (`new UnifiedRandom()`) | Utilities/WeightedRandom.cs:19 | only used for Sandstorm particle colours (client visuals) |
| thread timing | the load runs on a task thread, ticks on the main thread; the server's socket thread is never started | n/a |

Result: **two fresh processes produce byte-identical dump files** (every `*.jsonl`, `*.bin`, `*.json` except `*.timing.json`; checked for both scenarios: sha256 equal, `cmp` clean). Earlier, before the thread-static
load generator and the wind counters were found, the runs differed in `windSpeed*`, `cloudBGActive`, `NPC.fireFlyChance`/`butterflyChance`/`stinkBugChance` and `numCloudsTemp` while all tile hashes, the 120-tick RNG file and the
tile-change list were already identical.

## 5. Formats

### `<scn>.ticks.jsonl` (world / player records)

One JSON object per line: `row` (0-based, also the row of `<scn>.columns.bin`), `tick`, then

| key | meaning |
|---|---|
| `tiles_fnv64`, `cols_fnv64`, `n_active`, `n_wall`, `n_liquid` | exactly as in `refdata/worldgen/README.md`: 14-byte tile records (`u16 type, u16 wall, u8 liquid, u16 sTileHeader, u8 bTileHeader, bTileHeader2, bTileHeader3, i16 frameX, i16 frameY`), column-major, FNV-1a 64 (`0xcbf29ce484222325`, prime `0x100000001b3`); whole-map hash without restart between columns; `cols_fnv64` over the 4200 column hashes as 8-byte LE |
| `tiles_changed_vs_load`, `first_changed` | number of tiles whose 14 bytes differ from the loaded world, and `[x,y]` of the first one (column-major) |
| `main` | 55 values: `time`, `dayTime`, `moonPhase`, `moonType`, `bloodMoon`, `eclipse`, `raining`, `rainTime`, `maxRaining`, `cloudAlpha`, `windSpeedCurrent`, `windSpeedTarget`, `numClouds`, `windCounter`, `extremeWindCounter`, `weatherCounter`, `cloudBGActive`, `slimeRain`, `slimeRainTime`, invasion fields, `hardMode`, `checkForSpawns`, `dayRate`, `sundialCooldown`, ..., `netPlayCounter`, `ambientCounter`, `_gameUpdateCount`, `halloween`, `xMas`, background/tree arrays |
| `liquid` | `Liquid.numLiquid`, `wetCounter`, `panicCounter`, `stuckCount`, `stuckAmount`, `quickSettle`, `quickFall`, `cycles`, `curMaxLiquid`, `skipCount`, `stuck`, `panicMode`, `panicY`, `LiquidBuffer.numLiquidBuffer` |
| `worldgen_static` | all 123 primitive statics of `Terraria.WorldGen` (world-evil counters `totalD/totalX/totalGood/totalEvil/totalBlood/totalSolid(2)`, `tGood/tEvil/tBlood`, `hardModeWorldUpdates`, `AllowedToSpreadInfections`, `tileCounter*`, `tileReframeCount`, ...) |
| `npc_static` | all 123 primitive statics of `Terraria.NPC` (all `downedBoss1/2/3`, `downedSlimeKing`, ..., `fireFlyChance`, `butterflyChance`, `stinkBugChance`, `spawnSpaceX/Y`, `ShimmeredTownNPCs`, ...) |
| `rand` | `{"main_rand": {inext, seed[56], next}, "rngs": {key: {inext, seed[56], next}, ...}}`: `Main.rand` of the main thread and **every** `Main._rngs` stream that exists (keys sorted ordinally). `next` = value `Next()` would return, computed on a copy |
| `counts` | active `npcs`, `items`, `projectiles`, non-null `chests`, `tile_entities` |
| `npcs_fnv64`, `items_fnv64`, `projectiles_fnv64` | FNV-1a 64 over the UTF-8 text `index:{json}\n` of every active entity, in array order (fields below) |
| `npcs`, `items`, `projectiles` | only in the full rows (ticks 0, 1, 2, 3 and the last): `[{"index": i, "v": {...}}]` |

Entity fields (all of them by name, bit-exact, floats widened to double; `Vector2` = `{X,Y}`): NPC `type netID position velocity ai[4] localAI[4] life lifeMax direction directionY target homeTileX homeTileY homeless spriteDirection rotation timeLeft wet lavaWet honeyWet width height aiStyle friendly townNPC shimmerTransparency oldTarget knockBackResist`;
projectile `type position velocity ai localAI owner timeLeft damage penetrate aiStyle friendly hostile wet rotation`; item (`WorldItem`) `type stack position velocity timeSinceItemSpawned playerIndexTheItemIsReservedFor wet shimmered`.
(Items and projectiles: none exist in either scenario, so those lists are empty in every row.) Floats are printed through `double` `"R"` text, so a Rust `f32 as f64` comparison is exact.

### `<scn>.columns.bin`

`rows x 4200 x u64` little endian, rows aligned with `ticks.jsonl` (65 rows = 2,184,000 bytes for `world`, 15 rows for `player`). Row `r` holds the per-column FNV-1a 64 of the 16800-byte column.

### `<scn>.rand_per_tick.jsonl`

120 lines (ticks 1..120): `{"tick": t, "main_rand": {...}, "rngs": {key: {inext, seed, next}}}` after tick `t`, same state encoding as above. Use it to find the first tick at which a port's RNG stream diverges, per stream
(streams that never moved stay at `inext 0` with the seed state).

### `<scn>.tile_changes.bin`

`"TCHG" | u32 version=1 | i32 width | i32 height | u32 count`, then `count` records of 36 bytes: `i32 x | i32 y | 14-byte record in the loaded world | 14-byte record at the last tick`. World scenario: 7404 changed tiles after 3600 ticks
(4653 changed only in their flag/frame bytes, 1280 type changes, 1465 liquid-only changes, 6 both); player scenario: 2803 after 600 ticks. They come from `WorldGen.UpdateWorld` (random tile ticks: grass/vine/plant growth, the
unsettled liquid left by the load) and `Liquid.UpdateLiquid`.

### `<scn>.main_statics_changed.json`

`{"name": [old, new]}` for every primitive static of `Terraria.Main` (JSON text of the value) that differs between tick 0 and the last tick: includes `time`, `_gameUpdateCount`, `checkForSpawns`, `netPlayCounter`,
`lastItemUpdate`, `windCounter`, `windSpeedCurrent/Target`, `weatherCounter`, `cloudBGActive`, `slimeRainTime`, `numCloudsTemp`, `extremeWindCounter`, `ambientCounter`, `_worldPreparationState` (0 -> 2 Ready), `IsEnginePreloaded`, `screenWidth/Height`.

### `summary.json`, `timing.json`

`summary.json`: mode, ticks, `tick_error` (null), world sha256, seeds, `rng_pins` (46 strings), `notes` (spawn tile, wind resets with tick numbers, tile-change count), `intptr_size` (4), `main_halloween`. `timing.json`: wall-clock of the tick loop and the whole run (about 38 s and 42 s for 3600 ticks; 9 s and 13 s for 600).

## 6. Scenario 1 results (zero players, 3600 ticks)

* Time 13500 -> 17100 (+1 per tick, `dayRate 1`); the two town NPCs (Old Man type 37 at the dungeon, Guide type 22 at the spawn) walk (`ai`, `position` in the rows); no other NPC, item or projectile ever appears (no players: `NPC.SpawnNPC` has nobody to spawn near).
* The load leaves 171 liquid tiles in the buffer and `curMaxLiquid = 0`; `Liquid.numLiquid` is 0 at tick 1-3 and 1034 at tick 60, 416 at 120, falling to 0-5 by tick 840: the first minute settles the liquids.
* Tiles changed vs the loaded world: 8 at tick 1, 1814 at tick 60, 7404 at tick 3600.
* Wind: `windSpeedTarget` changes at the pinned wind-counter events (1500, 2400): see `main.windSpeed*` in the rows 25..65.
* Per-tick cost on this machine: about 10 ms (37.9 s / 3600).

## 7. Scenario 2: a player without a client

What the dedicated server does for a connected player: `player[i].Update(i)` for `active` slots inside `UpdateWorld_Players`. For a remote player `whoAmI != Main.myPlayer` (the server's `myPlayer` is 255), so `Player.Update` skips its local-input block (the part that reads
`PlayerInput.Triggers`) and the control flags (`controlLeft/Right/Up/Down/Jump`, `releaseJump`, selected item, ...) are whatever the last client packet (message 13) set. The harness therefore sets those fields directly before each tick, which is
exactly what `MessageBuffer` does when it receives a control packet. This keeps `netMode = 2` and uses no input device, so **nothing blocked the scenario**:

* `Main.player[0]`: `active = true`, `name = "Ref"`, 100 life / 20 mana, then the game's own `player.Spawn(PlayerSpawnContext.SpawningIntoWorld)` (placed at the world spawn tile (2095,232): position (33518, 3670), 20 x 42). With `netMode 2`
  `Spawn` only needs `Main.LocalPlayer` (slot 255) to exist; its `NetMessage.SendData` calls reach nobody. `Netplay.Clients[0]` stays a fresh, disconnected `RemoteClient` (`Clients[k].IsActive` is false, so `SpamUpdate` and the time-out counter are skipped; `RemoteClient.CheckSection(0, position)` in `UpdateServer` (Main.cs:65650) does run and needs no socket).
* Guards that matter: `Main.myPlayer == whoAmI` (false for slot 0: local-input code skipped), `Main.dedServ`/`netMode != 2` blocks (client-only drawing, `PlayerInput`, `Main.hasFocus` are never reached), `player.active`. `Main.hasFocus` and `PlayerInput` are not needed.
* Not done: the single-player path (`netMode = 0`, `controlRight` from `PlayerInput.Triggers.Current`) - it needs the game's input singletons and a graphics device for part of `Player.Update`; the remote-player path above is what the real server runs.
* With a player present the base game spawns NPCs (`SpawnNPC` stream active from tick 1): 2 more NPCs (types 46 and 1) by tick 120 and 4 in total from then on (rows of `player.ticks.jsonl`).

Control script (`ScriptPlayer`, tick numbers 1-based, applied before the tick): `controlRight` ticks 1-150; `controlJump` held ticks 121-150 and 361-420 and pulsed (one tick) at 201, 211, 221; `controlLeft` ticks 241-420; everything else idle.

`player.per_tick.jsonl`: 600 lines `{"tick": t, "player": {...}}` after tick `t`: `position velocity oldPosition oldVelocity direction oldDirection jump wingTime rocketTime fallStart fallStart2 gravDir wet lavaWet honeyWet statLife statMana
controlLeft controlRight controlUp controlDown controlJump releaseJump justJumped sliding slideDir fullRotation headPosition bodyFrame legFrame breath grappling mount maxFallSpeed moveSpeed runAcceleration maxRunSpeed runSlowdown gravity dead active ghost pulley autoJump`.

Sample (from `summarize.py`): tick 1 pos (33518.0781, 3670), vel.x 0.08; tick 60 vel.x 2.84; tick 120 pos.x 33819.8984, vel.x 2.88; tick 150 (jump held) pos.y 3573.6982; tick 201 (jump pulse) vel.y -4.61, `jump 15`, `justJumped`;
tick 300 moving left, vel.x -2.84, `direction -1`; tick 600 back at rest on the ground (pos.y 3686).

## 8. Things a porter must know

* **No client, no tick**: a real server with no fully connected client does not run the simulation at all (Main.cs:5920-5931). The first tick of the world is the first `Update` after a client reaches state 10.
* Each tick part has **its own RNG stream seeded from the world seed** (`SwapRandom`). Divergence in one subsystem does not disturb the others, which makes per-stream comparison against `rand_per_tick.jsonl` very effective. The base `Main.rand` is only used by weather/cloud/ambience code.
* `Main.rand` is thread-static; the load runs on another thread than the ticks.
* The game's own wind-change schedule is GUID-random (`FastRandom.CreateWithRandomSeed`); a bit-exact port has to take the counters from the dump or from its own choice, not derive them.
* Liquids are not settled after a dedicated-server load (`curMaxLiquid = 0` during `LoadWorld`), so the first ~800 ticks are full of liquid motion; `Liquid.UpdateLiquid` re-derives `cycles`/`curMaxLiquid` from the player count each call (Liquid.cs:996-1012).
* `Main._gameUpdateCount` (private static `uint`) is already 65536 at tick 0 in this harness (cause not investigated) and `Main.time` advances by exactly 1 per tick.
* Nothing in a tick touches the network when no client socket exists; do not call `Netplay.StartServer`.

## 9. Re-running

`tools/refdump_ticks/run.cmd` (build, `world` 3600 ticks, `player` 600 ticks; about 1.5 minutes; writes this folder; scratch copies under `work/ticks-scratch/`). Every process has an 8-minute watchdog, no window, no listener.
Determinism check: run twice into two folders and `cmp` every file except `*.timing.json`.

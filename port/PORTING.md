# Porting rules (every crate, every worker)

The port is a translation of the decompiled game, not a redesign. A reader must be able to put a Rust
function next to its C# original and check it line by line.

## Sources of truth

| What | Where |
|---|---|
| Decompiled C# | `decompile\Terraria\Terraria\**.cs`, `decompile\ReLogic\**.cs` |
| Where a method is, who calls it, what it calls | `db\terraria.sqlite` (`port_methods`, `port_calls`, `port_field_refs`; schema in `README.md`) |
| Values the running game produces | `refdata\*.json` (written by `tools\refdump`, the game's own code run headless) |
| XNA behaviour | `reference\xna\*.cs` (decompiled Microsoft.Xna.Framework value types) |
| A world made by the real game | `work\refsave\Worlds\ref_small.wld` (seed 12345, small) |

Big files (`Main.cs` 89k lines, `WorldGen.cs` 88k, `Player.cs` 58k, `NPC.cs`): never read whole. Get the
line range from the database, read that range.

```sql
SELECT f.path, m.start_line, m.end_line FROM port_methods m JOIN port_files f ON f.id = m.file_id
WHERE m.type_name = 'Terraria.WorldGen' AND m.name = 'TileFrame';
```

## Nothing invented

- Every constant, branch, order of operations and random call comes from the source. Keep the original
  statement order: the order of `rand.Next` calls is part of the behaviour.
- Do not "fix" bugs, simplify conditions, or merge branches. Odd code stays odd, with a comment `// as in the original`.
- If something cannot be ported yet (callee not ported, engine feature missing), call `unported!("Class.Method")`
  at that point (see below) and keep going. Never substitute a guess. Never leave a silent gap.
- Each ported function starts with a doc comment naming the original: ``/// `WorldGen.TileFrame` (WorldGen.cs:82239)``.

## Names: one fixed rule

Rust name = C# name through this algorithm (same as `snake()` in `tools\gen_tables.py`):
insert `_` between a lowercase letter or digit and an uppercase letter, and between an uppercase run and an
uppercase+lowercase pair; then lowercase.

| C# | Rust |
|---|---|
| `maxTilesX` | `max_tiles_x` |
| `TileFrame` | `tile_frame` |
| `AI_007_TownEntities` | `ai_007_town_entities` |
| `frameX`, `halfBrick()` | `frame_x`, `half_brick()` |
| `type` (keyword) | `type_` |
| constants and static tables: `TileID.Dirt`, `TileID.Sets.Falling` | `tile_id::DIRT`, `tile_id::sets::FALLING` |
| class `WorldGen` -> module | `world_gen` |
| setter overload `tile.active(true)` | `tile.set_active(true)` |
| overloads that differ by parameter types | suffix naming the difference: `kill_tile`, `kill_tile_no_item` only if the source has two; say which in the doc comment |

Local variables keep the decompiler's names (`num`, `num2`, `flag`) so the two texts line up. Do not rename them.

## State: statics become fields of `Game`

The original keeps everything in static fields. The port keeps one `terraria_sim::Game` and passes `g: &mut Game`.

| C# | Rust |
|---|---|
| `Main.maxTilesX` | `g.main.max_tiles_x` |
| `Main.tile[i, j]` (read) | `g.main.tile.get(i, j)` (a `&Tile`; copy it with `*` when you need a snapshot) |
| `Main.tile[i, j].frameX = 5` | `g.main.tile.get_mut(i, j).frame_x = 5` |
| `Main.tileSolid[type]` | `g.main.tile_solid[type_ as usize]` |
| `TileID.Sets.Falling[type]` | `tile_id::sets::FALLING[type_ as usize]` (from `terraria_data::sets`) |
| `WorldGen.genRand.Next(3)` | `g.main.rand.next(3)` (`genRand => Main.rand` in the source: one generator) |
| `Main.rand.Next(2, 5)` | `g.main.rand.next_range(2, 5)` |
| `WorldGen.TileFrame(i, j)` | `world_gen::tile_frame(g, i, j, false, false)` (default arguments written out) |
| instance method `npc.AI()` on `Main.npc[k]` | `npc::ai(g, k)` (index, not a reference, so the function can touch other state) |

`Tile` in C# is a class (reference). `Tile tile = Main.tile[i, j]; tile.frameX = 1;` writes the world.
In Rust take the copy only for reads; every write goes through `get_mut(i, j)`. When the original holds a
reference across calls that may change the same tile, re-read it after the call.

`Main.tile` is allocated once as `new Tile[8401, 2401]` (Main.cs:1087-1089) and a loaded world fills only
`[0, maxTilesX) x [0, maxTilesY)`. So there are three cases, and code that can reach the edges must keep them apart:
inside the world: a tile; outside the world but inside the 8401 x 2401 array: `null` (a `== null` check is true,
a member access throws NullReferenceException); outside the array: IndexOutOfRangeException. Most callers sit in
`try { } catch { }`. `terraria_sim::collision::utils` has `tile_slot`, `tile_at` and `tile_or_new` that model this; use them
(or `g.main.tile.try_get`) wherever the original can leave the world, and return as the catch would.

New static fields: add them to the struct for their class in `terraria-sim\src\game.rs` (`Main` -> `MainState`,
`WorldGen` -> `WorldGenState`, ...) with a doc comment naming the original and the same initial value.

## C# semantics that differ from Rust

- Integer overflow wraps in C#. Use `wrapping_*` wherever a value can leave its range (hashes, RNG, counters cast down).
- Narrowing casts truncate bits: `(byte)x` -> `x as u8`, `(short)x` -> `x as i16`. Same as Rust `as` between integers.
- `(int)someFloat` truncates toward zero. Rust `as i32` does the same in range, but saturates out of range and maps NaN to 0,
  where x86 .NET gives `int.MinValue`. If the source can reach that case, write it out and comment it.
- `/` and `%` on ints: same as Rust (truncating). Division by zero throws in C#; if the source relies on a surrounding catch, mirror it.
- Known, measured deviation: the game runs on the legacy x86 JIT, which keeps `float` temporaries in x87 registers at
  double precision and only rounds to 32 bits when a value is stored to a field or array. A literal `f32` port therefore
  differs from the game in the last bits of some results (collision reference: 48 of 62 method families bit-exact over
  20,000 cases each, the rest differ in under 0.1% to 18% of cases by rounding; `tests\collision_x87_evidence.rs`).
  Rule: translate with `f32` as the source says. Where a comparison against the real game shows a rounding flip that
  changes a decision (a `bool`, a tile index), compute that expression's temporaries in `f64` and round where the
  source stores, and say so in a comment with the test that shows it.
- `float` math: use `f32`. Where the source casts to `double` (`(double)x`, `Math.Sin`, `Math.Sqrt`), compute in `f64` and cast back exactly where the source does.
- `Math.Round(x)` is round-half-to-even. `Math.Abs(int.MinValue)` throws. `Math.Sign` returns -1, 0, 1.
- Arrays are bounds-checked in both. An out-of-range index in the original is an exception that a nearby `catch` often swallows:
  check where the source has `try`.
- `string` comparisons are ordinal unless the source says otherwise.
- `foreach` over `Dictionary`/`HashSet`: .NET iteration order is insertion order when nothing was removed. If order can matter, use a `Vec` or an insertion-ordered map.

## `unported!`

```rust
unported!("WorldGen.CheckOnTable1x1");            // statement position
let ok = unported!("Collision.SolidTiles", false); // expression position: second argument is the value to continue with
```

It records the name (once per name) in `g`-independent global state and logs it once. The value in expression
position must be the neutral one that keeps the caller's control flow closest to "did nothing" and must be
stated in a comment. `tools\unported.py` lists every site; that list is the port's to-do.

## Files

One Rust module per C# class, path following the namespace:
`Terraria.WorldGen` -> `terraria-sim\src\world_gen\` (split by topic into files when large: `tile_frame.rs`, `kill_tile.rs`),
`Terraria.GameContent.Drawing.TileDrawing` -> `terraria-app\src\game_content\drawing\tile_drawing.rs`.
Simulation (no rendering types) lives in `terraria-sim`. Anything that calls `SpriteBatch` lives in `terraria-app`.

## Checks before reporting done

- `cargo build` and `cargo test` for the crates you touched, no warnings from your files.
- A test for every table or pure function against values from `refdata\` or the reference world where one exists.
- A list of every `unported!` you added, with the reason.
- A list of every place you were unsure the translation is exact, with file:line on both sides.

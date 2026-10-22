# cs2rs coverage

**The dry run is retired (2026-10-11).** The whole game is generated for real from `game.json`; its numbers are in
`GAME_REPORT.md` (written by `sh run.sh`): 23,077 bodies, 22,532 clean, 28 with something unported inside, 182
refused, 335 not emitted; `cargo check` of the result: 0 errors. The tables of the last dry run and of the
`all_probe.json` probe (312 rustc errors) are kept below for the record only: they describe the generator BEFORE
monomorphisation, object equality, enum boxing, event accessors and native base classes. `all_probe.json` is
superseded by `game.json`.

Refused bodies of the whole game by first reason (`GAME_REPORT.md` has the examples): anonymous type 28,
reference conversion with no mapping 27, `yield` 16, unsafe statements 15, interpolation with alignment / format
13, library enumerator structs 10, binding errors of the decompile 9, is-type tests against library types 9,
unrepresentable types 8 + 7 + 2 + 1 + 1, compound assignment / `++` on a non-numeric type 10, named arguments out
of order 6, conversions with no rule 5, native property templates missing 4, native base constructor templates
missing 2, others 9. Not emitted: generic methods nothing instantiates 203, `extern` 110, unrepresentable
signature 13, finalizers 5, generic types nothing instantiates 4.

## What is tested (hand-maintained: `tools\cs2rs\coverage_tail.md`)

"Tested" means one of: (S) a snippet in `tools\cs2rs\tests\*.cs` that is run as C# and as translated Rust and
compared (`cargo test -p terraria-gen-rt`, test `snip::translated_snippets_match_csharp`, 38 snippets); (G) the
comparison with the real game in `port\crates\terraria-gen\tests\reference.rs` (`VERIFICATION.md`: items, NPCs,
projectiles, collision, tile framing, lighting); (R) a unit test of the runtime (`cargo test -p terraria-gen-rt`); (P1) reached by `tests\proof1.rs` of `terraria-gen-game` (it ran; nothing was compared with a dump).
Anything not in this table is either refused by the emitter (counted above) or emitted but unproven.

| semantics | how | where |
|---|---|---|
| `int` / `uint` / `long` overflow wraps; unary minus of `MinValue` | S | numbers.cs `IntOverflow` |
| narrowing casts (`byte`, `sbyte`, `short`, `ushort`, `int` from `long`, `char`) | S | `NarrowingCasts` |
| float to int: truncation, out of range and NaN give `int.MinValue`, `(int)((double)x * 0.6)` | S, P | `FloatToInt`; `Item.SetDefaults` drills |
| compound assignment with implicit narrowing (`byte b += 10`, `sb++`, `<<=`, `char +=`) | S | `CompoundNarrowing` |
| `/` and `%` of negatives, on `uint`, `long`, floats; `DivideByZeroException`; shifts mask the count | S | `DivMod` |
| `float` / `double` to string as .NET Framework prints them (G7 / G15), `bool`, `char` in concatenation | S (expected text written from the Framework rule, not run), R | `NumberFormatting`; `rt::string` tests |
| `switch` with stacked labels, `goto case`, `goto default`, `return`; `switch` on strings incl. null | S, P | `SwitchGoto`; `Item.SetDefaults1..5` (about 5,000 cases) |
| `break` / `continue` inside a `switch` inside a loop; `goto` out of nested loops; backward `goto`; forward `goto` over a declaration | S | `SwitchGoto`, `LabeledGoto` |
| `for` with bound and index changed in the body, `do/while` with `continue`, `while (true)`, `foreach` over an array, two loop variables | S | `Loops` |
| operand and argument order; named arguments out of order; index before value in `a[i++] = i` and `a[w] = w++`; `holder.id = (holder = other).id`; `x = y++ + y++`; chained assignment | S | `EvalOrder` |
| `&&` `||` short-circuit, `&` `|` do not; `?:`; `??` | S | `ShortCircuit` |
| `try/catch/finally`: typed catch, `catch { }`, `return` / `break` / `continue` out of `try`, nested, `throw;`, index and null failures as exceptions | S | `TryCatch` |
| struct copy on assignment and on call; class references alias; struct field of a class mutated in place; struct array elements in place; default null / zero elements | S | `StructVsClass` |
| `ref` / `out` to locals, array elements, fields of objects, statics, struct fields; two refs to one variable; `out var` | S | `RefOut` |
| static initialisation order: lazy, in source order, re-entrant (a class sees another's half-initialised statics); static constructor; field initialisers before the constructor body | S, P | `StaticInit`; `Item`, `SoundID` (800 statics), `ItemVariants` |
| virtual / abstract / override dispatch, `base.M()`, `is`, `as`, cast with `InvalidCastException` | S, P | `Inheritance`; `LegacySoundStyle : SoundStyle`, `CustomCurrencySingleCoin : CustomCurrencySystem` |
| `List<T>` (alias, insert/remove, `foreach`, modification during `foreach` throws), `Dictionary` order after removal, string keys by content, `TryGetValue`, collection initialiser | S, R | `Collections`; `rt::collections` tests |
| lambdas and method groups as delegates; capture of a never-reassigned local; `Func<>`; LINQ `Any` / `Count` | S, P | `Collections`; `ItemVariants` conditions |
| `int?` (`HasValue`, `Value`, `??`, `== null`, `== 5`), null dereference, null string in concatenation, `T[,]`, jagged arrays | S | `NullAndNullable` |
| `MemberwiseClone`, `params` arrays, optional parameters, generic methods with a type parameter, object initialisers | G | `Item.Clone`, `SetFactory.CreateBoolSet`, `ItemVariants.AddVariant`, `Utils.IndexInRange<T>` |
| boxing and unboxing of primitives, `string`, `T?`; `is` / `as` / casts on `object`; `InvalidCastException`, `NullReferenceException` on unboxing; type patterns `o is string s` | S, R | features.cs `Boxing`; `rt::gc`, `rt::object` tests |
| interfaces: dispatch to classes and to boxed structs, interface properties, `is` / `as` an interface, boxing copies the struct | S | `Interfaces` |
| `using` (nested, early `return`, exception, null resource), `IDisposable.Dispose` of translated classes | S | `UsingDispose` |
| field-like events, `+=` / `-=` with method groups and lambdas, multicast invocation order, removal of the last matching subscription, delegate `+=` on a local | S, R | `Events`; `rt::gc` multicast test |
| captured locals that are reassigned (heap cells), a fresh cell per loop iteration | S | `Captures` |
| lifted nullable arithmetic, comparison, unary minus, `int?` to `float?`, `T?` in a concatenation | S, R | `LiftedNullable` |
| `enum.ToString()`, `[Flags]` composition, values without a name | S, R | `EnumNames`; `rt::object::enum_names_like_net` |
| exception classes declared by the game: `throw`, `catch (GameType)`, `catch (Exception)`, rethrow, `Message` | S | `GameExceptions` |
| `HashSet` order after removal, `Stack` order, `Dictionary.Keys` / `.Values`, `List.Sort()` / `Sort(comparison)` / `Reverse`, `Tuple`, LINQ `Where` / `Select` / `ToList` / `ToArray` / `Range` / `Last`, `Array.Copy` / `Clear` / `Resize` | S, R | `MoreCollections`; `rt::sys` tests |
| the sort of .NET Framework 4.0 with ties (`DepthLimitedQuickSort`) | R (expected order derived by hand from the algorithm) | `rt::sys::sort_is_the_framework_4_0_quick_sort` |
| `string` members: `Substring`, `Split`, `Join`, `Trim`, `ToLower` / `ToUpper`, `Replace`, `StartsWith` / `EndsWith` / `Contains` / `IndexOf`, `IsNullOrWhiteSpace`, `int.Parse` / `TryParse`, `foreach` over a string | S, R | `Strings2`; `rt::sys` tests |
| generic code over `object`: `(T)obj`, `(object)t`, `typeof(T).IsPrimitive` / `IsClass` / nullable test, `Convert.ChangeType`, `where T : IComparable<T>` | S, G, R | `GenericObjects`; every `ItemID` / `NPCID` / `ProjectileID` set built by `SetFactory.CreateCustomSet<T>` |
| `if / else if` chains of five and more links (flat form), `out var` in a condition of the chain | S, G | `LongChains`; `NPC.SetDefaults`, `Projectile.SetDefaults` |
| `T[,,]`, nested member initialisers (`Member = { a, b }`), delegate from a struct method, `typeof` equality | S | `ArraysAndInitialisers` |
| the collector: cycles, arrays, strings, collections, delegate captures, statics, host roots, poison mode, stress against a model | R, G | `rt::gc` tests; `reference.rs` section `gc` |
| `floatMode` "x87" (float expression trees at x87 precision) | G | `VERIFICATION.md`: NPC / projectile / collision numbers per mode |
| static initialisers of whole classes (`Main`: 1,000 statics, `NPC`, `Player`, `ItemID.Sets`, `NPCID.Sets`, `ProjectileID.Sets`, `TileID.Sets`), `Main.Initialize_Entities`, `Initialize_TileAndNPCData1/2`, `Initialize_Items`, `TileObjectData.Initialize`, `Recipe.SetupRecipes` | G | game initialisation of `reference.rs` (each step listed in the report) |
| virtual dispatch across the big classes, struct keys in dictionaries (`Point16`), `DoubleStack` (hand port), `Wiring.HitSwitch` | G | collision family `SwitchTiles`, tile framing |
| natively mapped library CLASSES (`NATIVE_SCHEMA.md`): constructors, methods, properties inherited through an override (`MemoryStream.Position`), one Rust type for a C# hierarchy, `is` / `as` / cast from `object` by Rust type and by the table's `"is"`, `using`, an exception thrown by native code and caught by translated code, `BinaryReader` / `BinaryWriter` byte encoding | S, R | native.cs `NativeBinaryReader`, `NativeBinaryWriter`; `rt::io` tests |
| natively mapped library STRUCTS: zeroed fields and array elements, copies, `ref` / `out` to elements and fields, operators, boxing with the struct's own id, `T?`, dictionary key and `List.IndexOf` through `CsKey` | S, R | `NativeTimeSpan`; `rt::time` tests (.NET Framework rounding of `FromSeconds`) |
| P/Invoke (`static extern`): a void call is a host member, recorded, execution continues; lambda parameters that are discards (`(_, __) => ..`) | S | `NativeExtern` |
| duplicate key across native tables is an error naming both files | checked by hand (a second file defining `M:System.IO.Stream.Flush`: exit code 1, message names `rt_io.json` and the second file); no automated test | - |
| parsing with too little stack loses a file's types (CS8078): reproduced with `cs2rs --parse-stack-kb 256 Projectile.cs`; the tool now parses on 512 MB stacks and treats CS8078 as fatal | checked by hand; no automated test | DESIGN.md section 12 |
| generic classes and generic methods of the game, monomorphised: statics and type id per instantiation, instantiations created only by generic code, generic struct, static generic class, generic interface (class, boxed struct, constrained call in place), non-generic base dispatching to generic subclasses, generic base, `new T()`, constraint calls, `default(T)`, `t == null`, `typeof(T)`, `IComparable<T>`, `ref T`, generic delegate, virtual generic method | S, G | generics.cs `Generics`; `DoubleStack<T>` translated (collision family `SwitchTiles`), `SetFactory.CreateCustomSet<T>` |
| default-comparer equality: class overriding `Equals` / `GetHashCode` as `Dictionary` key / in `HashSet` / `List.Contains` / `IndexOf` / `Remove` / `Distinct`, `IEquatable<T>` on a class and a struct, identity for a plain class, field-wise struct equality, boxed values, `object.Equals(a, b)`, `EqualityComparer<T>.Default`, dictionary order after removal with such keys | S, R | equality.cs `Equality` |
| virtual `ToString()` through `object` and in concatenation, struct `ToString`, default `ToString` of a class | S | `Equality` |
| enum boxing with identity: `ToString` of a boxed enum, `is E`, enum / underlying unboxing, `Enum.GetName / GetNames / GetValues / Parse / TryParse / IsDefined`, `[Flags]` | S, R | `Equality`; `rt::sys` enum test |
| events with custom `add` / `remove` bodies, subscription through an interface | S | `Equality` |
| throw expression (`x ?? throw ...`) | S | `Equality` |
| `OfType`, `GroupBy`, `OrderBy / OrderByDescending / ThenBy / ThenByDescending` (stable), `List.RemoveAll`, `SelectMany`, `List.ForEach`, `LinkedList<T>` with node removal | S, R | `Equality`; `rt::sys` tests |
| reflection over the static fields of a translated class (`GetFields`, `FieldInfo.Name / FieldType / GetValue / IsLiteral`, the CLR's order), `Convert.ToInt32(object)`, `typeof(T).Name` | S | `Equality` |
| a translated class deriving from a native class (`Bag : List<int>`, `BigBag : Bag`): base members, indexer, collection initialiser, upcast to the native class and to `IEnumerable<T>`, `foreach`, type tests, downcast | S | native.cs `NativeBaseList` |
| JSON reader and `JsonConvert.DeserializeObject<Dictionary<string, Dictionary<string, string>>>` (document order, last value wins), embedded resources from files | R, P1 | `rt::json` tests; Proof 1 step `SetLanguage` (seven en-US files load) |
| en-US word-sort model of `string.Compare` (ASCII) | R (expected order written from the rule, not measured against .NET Framework) | `rt::sys::culture_tests` |

Emitted by the tool but **not** exercised by any test (treat as unproven): conditional access `?.`,
interpolated strings, `lock`, ref locals, lifted user-defined operators (`Vector2? + Vector2?`), dispatchers of
library interfaces other than `IDisposable`, `foreach` through a type's own `GetEnumerator` (the general
lowering), `Enumerable.Concat` / `Distinct` / `OrderBy` / `First`, `HashSet.UnionWith` / `ExceptWith`,
`Queue.Peek`, `Path.Combine` in generated code, `Enumerable.Cast`, covariance of `IEnumerable<T>` / arrays (`seq_up`, `arr_up_seq`), `string.Compare(a, b, kind)`, `string.Concat(object..)`, `ToDictionary`, `Array.FindAll`, `Type.GetType`, the native-override registry (`rt::native_override`), `new T { .. }` with an initialiser, casts to opaque library classes (`host::is_instance`),
`uint` / `long` array indices, the `to_u32` / `to_u64` float conversions, `floatMode` "x87-locals" beyond the
numbers in `VERIFICATION.md`.

Whole methods are "emitted but unproven" unless a reference section runs them. The comparison with the game
executes `SetDefaults` of the three entity classes, the initialisers listed above, `Collision` (62 public
methods), `WorldGen.SectionTileFrame` and what it calls (`TileFrame`, the framing of every tile type of the
reference world and of the scrambled region), `Wiring.HitSwitch`, and the lighting engine. `NPC.AI`,
`Projectile.AI`, `Player.Update`, world generation and everything else compile and have never run.

Mappings in `native.json` that no compiled code uses yet are equally unproven: a wrong template there shows
up as a rustc error the first time a translated method uses it, not as wrong behaviour.

## Whole game + ReLogic: where the generator stands (probe of 2026-10-11, `all_probe.json`)

`all_probe.json` is `core.json` plus `"namespaces": ["Terraria.*", "ReLogic.*", "BCrypt.*"]` and the ReLogic
sources: every type of both decompiles translated for real (not the dry run above, which treats untranslated
types as translated without giving them type ids and therefore overstates the cast / `is` refusals).
It is a measurement, not the build: `run.sh` still generates `core.json`.

```
dotnet bin/Release/net8.0/cs2rs.dll --config all_probe.json --out <dir>/gen --report <dir>/SCALE.md --host-api <dir>/HOST.md
```

| | measured |
|---|---|
| C# files / types | 1,716 files (1,549 Terraria + 167 ReLogic) |
| generation time | 22 s |
| generated Rust | 2,540 files, 995,000 lines (core set: 894 files, 684,000 lines) |
| bodies | 20,577: 18,724 clean, 960 with something unported inside, 244 refused, 649 not emitted |
| not emitted | generic type 322, event accessor 196, extern 110, signature 16, finalizer 5 |
| refused, by first reason | reference conversion with no mapping 28, `ToString()` of a non-primitive in a concatenation 26, anonymous type 24, `yield` 15, unsafe statements 15, binding errors 14, unrepresentable type 14, interpolation with format 13, library enumerators 19, `is` / type pattern 16, `typeof(T)` 9, `new T()` 7, compound assignment / `++` on a non-numeric type 10, others 34 |
| `cargo check -j 4` of the result | 3 min 42 s to the first error report: **312 errors**, the crate does not build |

The 312 rustc errors by cause (each is a defect of the generator or of the configuration, none was fixed in
the output):

| errors | cause |
|---:|---|
| 145 | a generic game class used as a type (`MenuTree<T>` 33, `SelectionHolder<T>` 17, `Ref<T>` 16, `GroupOptionButton<T>` 16, `AWorkshopPublishInfoState<T>` 16, `EntryFilterer<T, U>` 15, `AchievementTracker<T>` 12, ...): generic classes are still not emitted |
| 22 | statics of a generic class (`TileEntityType<T>`, `DeepStructCloning<T>`, `PacketTypeStorage<T>`, ...) |
| 39 | lambda parameter named `_`: fixed in the emitter after the probe (snippet `NativeExtern`), not re-measured |
| 32 + 20 | wrong argument or value types: not classified further |
| 18 | `LocalizedText` is still mapped to the host stand-in of `core.json` while the class itself is now translated: the mapping has to go when `Terraria.Localization` is translated (Proof 1) |
| 11 | assignment to a stubbed static (`Netplay.ListenPort = ..`): the stubs for `Netplay` / `Program` have to go when those classes are translated |
| 9 | collection initialisers and indexers of game classes (`ClusterGroup : List<Cluster>`, `DitherSnake[i]`), members of `Stream` on an unmapped subclass |
| 9 | locals of a lambda or local scope not found (`preference`, `g`, `x`): not diagnosed |
| 7 | `CsOrd` missing for `bool` / `DateTime`, call of a non-function, a captured variable mutated in a `Fn` closure |

Peak rustc memory was not measured for this run (the core set peaks at 5.9 GB). A crate split was not attempted.

---

# For the record: the last dry run (before this stage)

Generated by `sh coverage.sh` (`cs2rs --all --coverage COVERAGE.md`). Do not edit by hand; the section "What is tested" at the end is appended from `coverage_tail.md`.

Every method, accessor, operator and constructor of every class and struct declared in `decompile/Terraria` is fed through the emitter with every declared type treated as translated. Columns: **clean** = a body with nothing missing; **host calls** = translated, calls at least one library member through the host boundary (`HOST_API.md`), nothing else missing; **unported inside** = translated, but reaches a game member that has no Rust function (generic game class, extern, unrepresentable signature); **refused** = the emitter met a construct it does not support (the body would be `rt::unported_method`) or the member has no Rust signature. A dry run does not compile or run anything: what rustc accepted and what was compared with the real game is in `SCALE_REPORT.md` and in "What is tested" below. Static initialisers and field initialisers are not counted here (they are bodies in `SCALE_REPORT.md`).

Roslyn compilation of the decompile: 69 errors.

| namespace | types | methods | clean | host calls | unported inside | refused | lines translatable |
|---|---:|---:|---:|---:|---:|---:|---:|
| `(global)` | 1 | 18 | 1 | 2 | 0 | 15 | 15.8% |
| `BCrypt.Net` | 1 | 8 | 8 | 0 | 0 | 0 | 100.0% |
| `ReLogic.Utilities` | 2 | 85 | 81 | 4 | 0 | 0 | 100.0% |
| `Terraria` | 116 | 5383 | 4781 | 484 | 20 | 98 | 92.2% |
| `Terraria.Achievements` | 8 | 71 | 45 | 4 | 5 | 17 | 100.0% |
| `Terraria.Audio` | 18 | 187 | 103 | 71 | 0 | 13 | 95.3% |
| `Terraria.Chat` | 4 | 39 | 35 | 3 | 0 | 1 | 91.8% |
| `Terraria.Chat.Commands` | 14 | 46 | 40 | 4 | 1 | 1 | 94.5% |
| `Terraria.Cinematics` | 6 | 78 | 78 | 0 | 0 | 0 | 100.0% |
| `Terraria.DataStructures` | 113 | 687 | 538 | 94 | 2 | 53 | 90.4% |
| `Terraria.GameContent` | 141 | 970 | 876 | 67 | 1 | 26 | 96.9% |
| `Terraria.GameContent.Achievements` | 9 | 86 | 65 | 0 | 6 | 15 | 97.1% |
| `Terraria.GameContent.Ambience` | 2 | 17 | 15 | 2 | 0 | 0 | 100.0% |
| `Terraria.GameContent.Animations` | 41 | 178 | 153 | 7 | 10 | 8 | 99.2% |
| `Terraria.GameContent.Bestiary` | 52 | 276 | 237 | 23 | 1 | 15 | 91.2% |
| `Terraria.GameContent.Biomes` | 29 | 165 | 156 | 0 | 4 | 5 | 95.4% |
| `Terraria.GameContent.Biomes.CaveHouse` | 10 | 87 | 72 | 11 | 1 | 3 | 96.2% |
| `Terraria.GameContent.Biomes.Desert` | 12 | 48 | 47 | 0 | 0 | 1 | 80.5% |
| `Terraria.GameContent.Creative` | 53 | 405 | 352 | 34 | 8 | 11 | 96.5% |
| `Terraria.GameContent.Drawing` | 15 | 301 | 197 | 45 | 56 | 3 | 93.8% |
| `Terraria.GameContent.Dyes` | 5 | 13 | 9 | 0 | 1 | 3 | 74.2% |
| `Terraria.GameContent.Events` | 14 | 153 | 140 | 8 | 4 | 1 | 99.6% |
| `Terraria.GameContent.FishDropRules` | 14 | 57 | 57 | 0 | 0 | 0 | 100.0% |
| `Terraria.GameContent.Generation` | 12 | 53 | 51 | 2 | 0 | 0 | 100.0% |
| `Terraria.GameContent.Generation.Dungeon` | 20 | 171 | 160 | 0 | 0 | 11 | 84.7% |
| `Terraria.GameContent.Generation.Dungeon.Entrances` | 9 | 29 | 23 | 0 | 0 | 6 | 14.4% |
| `Terraria.GameContent.Generation.Dungeon.Features` | 31 | 100 | 84 | 0 | 0 | 16 | 68.1% |
| `Terraria.GameContent.Generation.Dungeon.Halls` | 13 | 42 | 34 | 1 | 0 | 7 | 21.9% |
| `Terraria.GameContent.Generation.Dungeon.LayoutProviders` | 7 | 37 | 27 | 4 | 0 | 6 | 63.2% |
| `Terraria.GameContent.Generation.Dungeon.Rooms` | 18 | 113 | 89 | 2 | 0 | 22 | 48.2% |
| `Terraria.GameContent.Golf` | 8 | 54 | 49 | 3 | 0 | 2 | 89.4% |
| `Terraria.GameContent.ItemDropRules` | 103 | 508 | 508 | 0 | 0 | 0 | 100.0% |
| `Terraria.GameContent.Items` | 18 | 89 | 89 | 0 | 0 | 0 | 100.0% |
| `Terraria.GameContent.LeashedEntities` | 23 | 102 | 101 | 1 | 0 | 0 | 100.0% |
| `Terraria.GameContent.Liquid` | 1 | 23 | 15 | 3 | 0 | 5 | 22.8% |
| `Terraria.GameContent.LootSimulation` | 4 | 16 | 12 | 2 | 0 | 2 | 84.8% |
| `Terraria.GameContent.LootSimulation.LootSimulatorConditionSetterTypes` | 4 | 16 | 16 | 0 | 0 | 0 | 100.0% |
| `Terraria.GameContent.Metadata` | 3 | 17 | 16 | 1 | 0 | 0 | 100.0% |
| `Terraria.GameContent.NetModules` | 13 | 52 | 51 | 0 | 0 | 1 | 95.7% |
| `Terraria.GameContent.ObjectInteractions` | 13 | 47 | 47 | 0 | 0 | 0 | 100.0% |
| `Terraria.GameContent.Personalities` | 19 | 46 | 44 | 0 | 0 | 2 | 99.0% |
| `Terraria.GameContent.Prefixes` | 3 | 3 | 3 | 0 | 0 | 0 | - |
| `Terraria.GameContent.RGB` | 59 | 166 | 45 | 84 | 37 | 0 | 100.0% |
| `Terraria.GameContent.Shaders` | 7 | 30 | 17 | 9 | 0 | 4 | 92.3% |
| `Terraria.GameContent.Skies` | 43 | 260 | 202 | 52 | 3 | 3 | 98.3% |
| `Terraria.GameContent.Skies.CreditsRoll` | 2 | 27 | 9 | 0 | 18 | 0 | 100.0% |
| `Terraria.GameContent.Tile_Entities` | 13 | 197 | 157 | 2 | 28 | 10 | 97.2% |
| `Terraria.GameContent.UI` | 31 | 212 | 166 | 31 | 6 | 9 | 89.5% |
| `Terraria.GameContent.UI.BigProgressBar` | 18 | 71 | 57 | 14 | 0 | 0 | 100.0% |
| `Terraria.GameContent.UI.Chat` | 15 | 57 | 49 | 8 | 0 | 0 | 100.0% |
| `Terraria.GameContent.UI.Elements` | 73 | 613 | 363 | 103 | 25 | 122 | 92.6% |
| `Terraria.GameContent.UI.Minimap` | 4 | 35 | 31 | 4 | 0 | 0 | 100.0% |
| `Terraria.GameContent.UI.ResourceSets` | 8 | 64 | 55 | 9 | 0 | 0 | 100.0% |
| `Terraria.GameContent.UI.States` | 37 | 724 | 474 | 70 | 66 | 114 | 94.4% |
| `Terraria.GameInput` | 11 | 296 | 280 | 12 | 0 | 4 | 100.0% |
| `Terraria.Graphics` | 23 | 134 | 94 | 30 | 0 | 10 | 83.9% |
| `Terraria.Graphics.CameraModifiers` | 4 | 22 | 22 | 0 | 0 | 0 | 100.0% |
| `Terraria.Graphics.Capture` | 13 | 89 | 64 | 23 | 0 | 2 | 96.9% |
| `Terraria.Graphics.Effects` | 10 | 66 | 36 | 15 | 1 | 14 | 89.9% |
| `Terraria.Graphics.Light` | 9 | 82 | 78 | 0 | 0 | 4 | 93.1% |
| `Terraria.Graphics.Renderers` | 21 | 155 | 130 | 20 | 0 | 5 | 98.6% |
| `Terraria.Graphics.Shaders` | 10 | 103 | 82 | 12 | 4 | 5 | 94.3% |
| `Terraria.ID` | 93 | 163 | 157 | 4 | 0 | 2 | 99.5% |
| `Terraria.IO` | 12 | 194 | 136 | 47 | 1 | 10 | 98.5% |
| `Terraria.Initializers` | 13 | 97 | 72 | 19 | 3 | 3 | 61.4% |
| `Terraria.Localization` | 6 | 107 | 72 | 22 | 0 | 13 | 85.9% |
| `Terraria.Map` | 15 | 93 | 76 | 12 | 0 | 5 | 81.8% |
| `Terraria.Modules` | 11 | 11 | 10 | 1 | 0 | 0 | 100.0% |
| `Terraria.Net` | 13 | 68 | 49 | 17 | 0 | 2 | 98.2% |
| `Terraria.Net.Sockets` | 5 | 51 | 24 | 19 | 2 | 6 | 78.0% |
| `Terraria.ObjectData` | 1 | 137 | 137 | 0 | 0 | 0 | 100.0% |
| `Terraria.Physics` | 6 | 26 | 26 | 0 | 0 | 0 | 100.0% |
| `Terraria.Server` | 1 | 46 | 38 | 0 | 0 | 8 | 100.0% |
| `Terraria.Social` | 1 | 8 | 6 | 1 | 0 | 1 | 93.9% |
| `Terraria.Social.Base` | 21 | 78 | 57 | 5 | 0 | 16 | 75.5% |
| `Terraria.Social.Steam` | 26 | 202 | 104 | 90 | 1 | 7 | 94.0% |
| `Terraria.Social.WeGame` | 26 | 266 | 128 | 109 | 0 | 29 | 94.4% |
| `Terraria.Testing` | 37 | 237 | 122 | 67 | 3 | 45 | 92.0% |
| `Terraria.Testing.ChatCommands` | 7 | 108 | 86 | 17 | 1 | 4 | 95.0% |
| `Terraria.Testing.Cloning` | 14 | 101 | 28 | 27 | 13 | 33 | 87.2% |
| `Terraria.UI` | 45 | 461 | 373 | 38 | 0 | 50 | 92.4% |
| `Terraria.UI.Chat` | 5 | 44 | 31 | 10 | 0 | 3 | 85.9% |
| `Terraria.UI.Gamepad` | 4 | 67 | 44 | 0 | 0 | 23 | 90.2% |
| `Terraria.Utilities` | 22 | 178 | 130 | 29 | 2 | 17 | 95.6% |
| `Terraria.Utilities.FileBrowser` | 3 | 6 | 5 | 1 | 0 | 0 | 100.0% |
| `Terraria.Utilities.Terraria.Utilities` | 1 | 7 | 7 | 0 | 0 | 0 | 100.0% |
| `Terraria.WorldBuilding` | 126 | 474 | 421 | 41 | 0 | 12 | 96.6% |
| **total** | 1927 | 17509 | 14255 | 1956 | 334 | 964 | 91.8% (636566 / 693535) |

Translatable (clean + host calls + unported inside): **16545 of 17509 (94.5%)**.

## Refusal histogram

"methods" = methods in which the construct occurs at least once; "only blocker" = methods where it is the only kind of refusal.

| construct | occurrences | methods | only blocker | example |
|---|---:|---:|---:|---|
| cast to a type that is not a translated class | 247 | 159 | 135 | `BiomeRuggedDungeonRoom.GenerateEarlyDungeonFeaturesInRoom(DungeonData)` BiomeRuggedDungeonRoom.cs:57 |
| is-type test with no rule | 145 | 65 | 37 | `AWorkshopEntry.TryGet<T>(Dictionary<string, object>, string, out T)` AWorkshopEntry.cs:104 |
| type pattern with no rule | 80 | 63 | 55 | `ArmorShaderData.Apply(Entity, DrawData?)` ArmorShaderData.cs:116 |
| reference conversion with no mapping | 37 | 28 | 25 | `ArmorSetBonuses.BuildLookup()` ArmorSetBonuses.cs:497 |
| anonymous type | 26 | 24 | 24 | `ContentRejectionFromSize.GetReason()` ContentRejectionFromSize.cs:26 |
| as to a type that is not a translated class | 31 | 24 | 14 | `AWorldListItem.UpdateGlitchAnimation(UIElement)` AWorldListItem.cs:40 |
| ToString() of a non-primitive in a concatenation | 23 | 20 | 18 | `CrashWatcher.Inititialize()` CrashWatcher.cs:31 |
| type with no Rust representation | 33 | 20 | 0 | `AmbientSky.Update(GameTime)` AmbientSky.cs:1382 |
| receiver type with no Rust representation | 54 | 16 | 1 | `AmbientSky.Update(GameTime)` AmbientSky.cs:1382 |
| statement: None | 16 | 16 | 15 | `CaptureCamera.DrawBytesToBuffer(byte[], byte[], int, int, Rectangle)` CaptureCamera.cs:240 |
| yield | 36 | 16 | 15 | `ChatManager.LayoutSnippets(DynamicSpriteFont, IEnumerable<TextSnippet>, Vector2, float)` ChatManager.cs:148 |
| interpolation with alignment / format | 16 | 13 | 12 | `CustomCurrencySingleCoin.GetPriceText(string[], ref int, long)` CustomCurrencySingleCoin.cs:106 |
| call returning a type with no Rust representation | 24 | 12 | 3 | `ItemSlot.ShouldHighlightSlotForMouseItem(int, int, Item)` ItemSlot.cs:1339 |
| binding error | 13 | 9 | 7 | `FxReader.FromStream<T>(Stream)` FxReader.cs:117 |
| conversion with no rule | 14 | 9 | 3 | `SoundPlayer.Play(SoundStyle, Vector2, SoundPlayOverrides)` SoundPlayer.cs:22 |
| new T() | 8 | 8 | 6 | `ChatCommandProcessor.AddCommand<T>()` ChatCommandProcessor.cs:26 |
| compound assignment on a non-numeric type | 11 | 7 | 7 | `AchievementsHelper.HandleNurseService(int)` AchievementsHelper.cs:408 |
| foreach: enumerator type with no Rust representation | 7 | 7 | 7 | `DesertHive.PlaceClustersArea(DesertDescription, ClusterGroup, Rectangle, PostPlacementEffect[,], Point, GenerationProgress, float, float)` DesertHive.cs:265 |
| host call returning a type with no Rust representation: Enumerator | 6 | 6 | 0 | `ArgumentListResult.GetEnumerator()` ArgumentListResult.cs:35 |
| named arguments out of order with side effects | 7 | 6 | 5 | `FlameParticle.Draw(ref ParticleRendererSettings, SpriteBatch)` FlameParticle.cs:61 |
| host signature uses a type with no Rust representation: byte* | 5 | 5 | 0 | `nativefiledialog.NFD_SaveDialog(string, string, out string)` nativefiledialog.cs:109 |
| typeof of a type parameter / pointer | 6 | 5 | 3 | `DeepCloneCodegen.GenerateClassDelegates<T>(ClassTypeInfo, out CloneClassChildren<T>, out RestoreClassChildren<T>)` DeepCloneCodegen.cs:16 |
| ++ / -- on a non-numeric type | 7 | 4 | 3 | `AchievementsHelper.PlantedAcorn()` AchievementsHelper.cs:403 |
| boxing of a type with no Rust representation | 3 | 3 | 0 | `ArgumentListResult.GetEnumerator()` ArgumentListResult.cs:35 |
| creation of a type with no Rust representation | 4 | 3 | 0 | `DebugNetworkStream.BeginRead(byte[], int, int, AsyncCallback, object)` DebugNetworkStream.cs:164 |
| event subscription | 2 | 2 | 2 | `MessageDispatcherClient.Init(string, string)` MessageDispatcherClient.cs:22 |
| field of a type with no Rust representation | 3 | 2 | 0 | `DebugNetworkStream.BeginRead(byte[], int, int, AsyncCallback, object)` DebugNetworkStream.cs:164 |
| interface method call on a value / type parameter | 6 | 2 | 1 | `CreativePowerManager.Register<T>(string)` CreativePowerManager.cs:40 |
| ref assignment | 21 | 2 | 2 | `Main.DrawNPCDirect(SpriteBatch, NPC, bool, Vector2)` Main.cs:23662 |
| sizeof of a non-constant type | 3 | 2 | 1 | `MapHelper.LoadMapVersionCompressed(BinaryReader, int)` MapHelper.cs:3392 |
| binary operator Divide on a non-numeric type | 1 | 1 | 0 | `UIAchievementListItem.DrawSelf(SpriteBatch)` UIAchievementListItem.cs:140 |
| comparison of a type with no Rust representation | 2 | 1 | 0 | `UIAchievementListItem.DrawSelf(SpriteBatch)` UIAchievementListItem.cs:96 |
| conversion involving a type with no Rust representation | 1 | 1 | 0 | `FxReader.FromStream<T>(Stream)` FxReader.cs:117 |
| decimal | 2 | 1 | 0 | `UIAchievementListItem.DrawSelf(SpriteBatch)` UIAchievementListItem.cs:96 |
| expression: None | 2 | 1 | 0 | `nativefiledialog.UTF8_ToManaged(IntPtr, bool)` nativefiledialog.cs:52 |
| host receiver Obj where Seq<terraria::id::content_samples::creative_helper::item_group_and_order_in_group::ItemGroupAndOrderInGroup> is expected | 1 | 1 | 0 | `CreativeHelper.SetCreativeMenuOrder()` ContentSamples.cs:731 |
| host signature uses a type with no Rust representation: char* | 1 | 1 | 0 | `nativefiledialog.UTF8_ToManaged(IntPtr, bool)` nativefiledialog.cs:62 |

## Members with no Rust signature

| reason | members |
|---|---:|
| generic type | 248 |
| event accessor | 190 |
| extern method | 40 |
| signature uses a type with no Rust representation | 17 |

## Types with no Rust representation (top 30)

| type | occurrences |
|---|---:|
| `ItemPair` | 33 |
| `Enumerator` | 28 |
| `ArraySegment<Item>` | 26 |
| `pointer` | 16 |
| `ArraySegment<T>` | 15 |
| `Interface` | 9 |
| `Class` | 8 |
| `ArraySegment<byte>` | 6 |
| `byte*` | 6 |
| `OpaqueDataDictionary` | 2 |
| `comparison of a type with no Rust representation` | 2 |
| `ContentBuildLogger` | 1 |
| `ContentIdentity` | 1 |
| `Error` | 1 |
| `ExternalReference<TInput>` | 1 |
| `ExternalReference<TOutput>` | 1 |
| `TargetPlatform` | 1 |
| `Tuple<decimal, decimal>` | 1 |
| `char*` | 1 |
| `conversion involving a type with no Rust representation` | 1 |

## Largest refused methods (source lines)

| method | lines | first refusal |
|---|---:|---|
| `Projectile.AI()` | 14069 | cast to a type that is not a translated class (WaterShaderData) (Projectile.cs:32345) |
| `Main.DrawProjDirect(Projectile, Player)` | 7338 | ref assignment (Main.cs:32582) |
| `MessageBuffer.GetData(int, int, out int)` | 4402 | as to a type that is not a translated class (NetServerSocialModule) (MessageBuffer.cs:187) |
| `Main.DrawNPCDirect(SpriteBatch, NPC, bool, Vector2)` | 3973 | ref assignment (Main.cs:23662) |
| `Dust.UpdateDust()` | 2073 | is-type test with no rule (object is Player) (Dust.cs:522) |
| `UILinksInitializer.Load()` | 1960 | is-type test with no rule (TileEntity is TEHatRack) (UILinksInitializer.cs:257) |
| `NetMessage.SendData(int, int, int, NetworkText, int, float, float, float, int, int, int)` | 1777 | compound assignment on a property with an impure receiver (NetMessage.cs:137) |
| `IngameOptions.Draw(Main, SpriteBatch)` | 1374 | ToString() of LocalizedText in a concatenation (IngameOptions.cs:722) |
| `TileDrawing.DrawTiles_EmitParticles(int, int, Tile, ushort, short, short, Color)` | 879 | binding error (IInvalidOperation) (TileDrawing.cs:7012) |
| `Gore.Update()` | 790 | cast to a type that is not a translated class (WaterShaderData) (Gore.cs:986) |
| `LegacyDungeonHall.LegacyHall(DungeonData, int, int, bool)` | 691 | cast to a type that is not a translated class (LegacyDungeonHallSettings) (LegacyDungeonHall.cs:62) |
| `DungeonGlobalEarlyDualDungeonFeatures.EarlyDungeonFeatures(DungeonData)` | 642 | is-type test with no rule (DungeonRoom is BiomeDungeonRoom) (DungeonGlobalEarlyDualDungeonFeatures.cs:134) |
| `LegacyDungeonEntrance.LegacyEntrance(DungeonData, int, int, bool)` | 629 | cast to a type that is not a translated class (LegacyDungeonEntranceSettings) (LegacyDungeonEntrance.cs:33) |
| `Mount.Draw(List<DrawData>, int, Player, Vector2, Color, SpriteEffects, float)` | 603 | cast to a type that is not a translated class (DrillMountData) (Mount.cs:5887) |
| `TowerDungeonEntrance.TowerEntrance(DungeonData, int, int, bool)` | 472 | cast to a type that is not a translated class (TowerDungeonEntranceSettings) (TowerDungeonEntrance.cs:42) |
| `LiquidRenderer.InternalPrepareDraw(Rectangle)` | 460 | statement: None (LiquidRenderer.cs:182) |
| `PlayerDrawLayers.DrawPlayer_09_Wings(ref PlayerDrawSet)` | 451 | named arguments out of order with side effects (in an indexer / property call) (PlayerDrawLayers.cs:1005) |
| `DomeDungeonEntrance.DomeEntrance(DungeonData, int, int, bool)` | 450 | cast to a type that is not a translated class (DomeDungeonEntranceSettings) (DomeDungeonEntrance.cs:42) |
| `PlayerDrawLayers.DrawPlayer_21_Head(ref PlayerDrawSet)` | 422 | named arguments out of order with side effects (in an indexer / property call) (PlayerDrawLayers.cs:2393) |
| `Main.DrawCapture(Rectangle, CaptureSettings, CaptureCamera)` | 417 | cast to a type that is not a translated class (WaterShaderData) (Main.cs:58009) |
| `DungeonPitTrap.PitTrap(DungeonData, int, int, DungeonGenerationStyleData, bool)` | 362 | cast to a type that is not a translated class (DungeonPitTrapSettings) (DungeonPitTrap.cs:45) |
| `DungeonDropTrap.DropTrap(DungeonData, int, int)` | 326 | cast to a type that is not a translated class (DungeonDropTrapSettings) (DungeonDropTrap.cs:36) |
| `EmoteBubble.ProbeExceptions(List<int>, Player, WorldUIAnchor)` | 306 | cast to a type that is not a translated class (NPC) (EmoteBubble.cs:766) |
| `DualDungeonLayoutProvider.ConvertSpecializedRooms(DungeonData, List<DungeonRoom>)` | 298 | is-type test with no rule (DungeonRoomSettings is GenShapeDungeonRoomSettings) (DualDungeonLayoutProvider.cs:688) |
| `MapHelper.InternalSaveMap()` | 298 | binding error (IInvalidOperation) (MapHelper.cs:3063) |
| `MapHelper.LoadMapVersion2(BinaryReader, int)` | 288 | binding error (IInvalidOperation) (MapHelper.cs:3950) |
| `LegacyEntranceDungeonHall.LegacyHall(DungeonData, int, int, bool)` | 260 | cast to a type that is not a translated class (LegacyEntranceDungeonHallSettings) (LegacyEntranceDungeonHall.cs:25) |
| `MapHelper.LoadMapVersionCompressed(BinaryReader, int)` | 240 | sizeof of a non-constant type (MapHelper.cs:3392) |
| `Mount.Hover(Player)` | 214 | cast to a type that is not a translated class (DrillMountData) (Mount.cs:3545) |
| `SceneState.ApplyVisuals(SceneMetrics)` | 212 | cast to a type that is not a translated class (SimpleOverlay) (SceneState.cs:157) |
| `DungeonCrawler.MakeDungeon(int, int, GenerationProgress)` | 171 | cast to a type that is not a translated class (LegacyDungeonHallSettings) (DungeonCrawler.cs:436) |
| `Mount.AimAbility(Player, Vector2)` | 166 | cast to a type that is not a translated class (DrillMountData) (Mount.cs:5528) |
| `WorldGen.MoveTownNPC(int, int, int, IRoomCheckFeedback)` | 159 | reference conversion string[] -> object[] (WorldGen.cs:4699) |
| `DesertHive.PlaceClustersArea(DesertDescription, ClusterGroup, Rectangle, PostPlacementEffect[,], Point, GenerationProgress, float, float)` | 158 | foreach: enumerator type with no Rust representation (Enumerator) (DesertHive.cs:265) |
| `DungeonCrawler.SetupDungeonGenVarVariables(DungeonGenVars, UnifiedRandom)` | 158 | cast to a type that is not a translated class (PreGenDungeonEntranceSettings) (DungeonCrawler.cs:210) |
| `ItemSlot.PickItemMovementAction(Item[], int, int, Item)` | 154 | as to a type that is not a translated class (TEDisplayDoll) (ItemSlot.cs:1545) |
| `BestiaryDatabaseNPCsPopulator.ModifyEntriesThatNeedIt()` | 150 | is-type test with no rule (IBestiaryInfoElement is NamePlateInfoElement) (BestiaryDatabaseNPCsPopulator.cs:552) |
| `LegacyDungeonRoom.LegacyRoom(DungeonData, int, int, bool)` | 146 | cast to a type that is not a translated class (LegacyDungeonRoomSettings) (LegacyDungeonRoom.cs:98) |
| `Netplay.InnerClientLoop()` | 140 | interpolation with alignment / format (Netplay.cs:542) |
| `BiomeRuggedDungeonRoom.BiomeRoom(DungeonData, int, int, bool)` | 136 | cast to a type that is not a translated class (BiomeDungeonRoomSettings) (BiomeRuggedDungeonRoom.cs:76) |


## Whole game + ReLogic: where the generator stands (probe of 2026-10-11, `all_probe.json`)

`all_probe.json` is `core.json` plus `"namespaces": ["Terraria.*", "ReLogic.*", "BCrypt.*"]` and the ReLogic
sources: every type of both decompiles translated for real (not the dry run above, which treats untranslated
types as translated without giving them type ids and therefore overstates the cast / `is` refusals).
It is a measurement, not the build: `run.sh` still generates `core.json`.

```
dotnet bin/Release/net8.0/cs2rs.dll --config all_probe.json --out <dir>/gen --report <dir>/SCALE.md --host-api <dir>/HOST.md
```

| | measured |
|---|---|
| C# files / types | 1,716 files (1,549 Terraria + 167 ReLogic) |
| generation time | 22 s |
| generated Rust | 2,540 files, 995,000 lines (core set: 894 files, 684,000 lines) |
| bodies | 20,577: 18,724 clean, 960 with something unported inside, 244 refused, 649 not emitted |
| not emitted | generic type 322, event accessor 196, extern 110, signature 16, finalizer 5 |
| refused, by first reason | reference conversion with no mapping 28, `ToString()` of a non-primitive in a concatenation 26, anonymous type 24, `yield` 15, unsafe statements 15, binding errors 14, unrepresentable type 14, interpolation with format 13, library enumerators 19, `is` / type pattern 16, `typeof(T)` 9, `new T()` 7, compound assignment / `++` on a non-numeric type 10, others 34 |
| `cargo check -j 4` of the result | 3 min 42 s to the first error report: **312 errors**, the crate does not build |

The 312 rustc errors by cause (each is a defect of the generator or of the configuration, none was fixed in
the output):

| errors | cause |
|---:|---|
| 145 | a generic game class used as a type (`MenuTree<T>` 33, `SelectionHolder<T>` 17, `Ref<T>` 16, `GroupOptionButton<T>` 16, `AWorkshopPublishInfoState<T>` 16, `EntryFilterer<T, U>` 15, `AchievementTracker<T>` 12, ...): generic classes are still not emitted |
| 22 | statics of a generic class (`TileEntityType<T>`, `DeepStructCloning<T>`, `PacketTypeStorage<T>`, ...) |
| 39 | lambda parameter named `_`: fixed in the emitter after the probe (snippet `NativeExtern`), not re-measured |
| 32 + 20 | wrong argument or value types: not classified further |
| 18 | `LocalizedText` is still mapped to the host stand-in of `core.json` while the class itself is now translated: the mapping has to go when `Terraria.Localization` is translated (Proof 1) |
| 11 | assignment to a stubbed static (`Netplay.ListenPort = ..`): the stubs for `Netplay` / `Program` have to go when those classes are translated |
| 9 | collection initialisers and indexers of game classes (`ClusterGroup : List<Cluster>`, `DitherSnake[i]`), members of `Stream` on an unmapped subclass |
| 9 | locals of a lambda or local scope not found (`preference`, `g`, `x`): not diagnosed |
| 7 | `CsOrd` missing for `bool` / `DateTime`, call of a non-function, a captured variable mutated in a `Fn` closure |

Peak rustc memory was not measured for this run (the core set peaks at 5.9 GB). A crate split was not attempted.

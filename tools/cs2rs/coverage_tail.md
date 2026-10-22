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
| `unsafe` code: `fixed` on an array element / an array (null for null and empty) / a field of a class object, `*p`, `p->f`, `p[i]`, pointer `+ - ++ += -=`, comparison and difference, `sizeof(struct)`, a `byte*` read as a struct pointer | S | pointers.cs `PointerBasics` |
| `ref` local re-pointed (`r = ref a[2]`), `_ = ref a[7]` (bounds check only) | S | pointers.cs `PointerBasics` |
| `new MemoryStream(byte[])` and the range / writable overloads wrap the array; `GetBuffer`, `Capacity`, `SetLength`, zero fill behind the end, not expandable | S, R | pointers.cs `MemoryStreamWraps`; `rt::io` tests |
| a `List<T>.Enumerator` held in a local (`using (var e = list.GetEnumerator()) while (e.MoveNext() && ...)`), boxed as `IEnumerator<T>` | S | ingame.cs `InGameConstructs` |
| `ArraySegment<T>` (a view: later writes to the array are seen), its argument checks | S | ingame.cs |
| array covariance: `Derived[]` as `Base[]` / `object[]` (the same array), `string[]` as `object[]` (a boxed copy: stated deviation) | S | ingame.cs |
| `x.GetThing().Value++` / `--` / `+=` on a property of a receiver with side effects: the receiver is evaluated once | S | ingame.cs |
| a suspended iterator method whose only references to objects are inside a snapshot enumerator's buffer or a boxed lazy enumerator survives a collection (negative control: fails without `register_scan_block`) | R | `rt::iter` test `handles_owned_by_enumerators_inside_a_suspended_iterator_survive_a_collection` |
| stream close hook (wrappers finish on `Dispose` and close the stream they wrap), `Enum.GetUnderlyingType` | R | `rt::io`, `rt::object` tests |
| the per-tick simulation (player movement, collision, liquids, world updates, NPC AI of the two town NPCs and the spawned ones, 17 RNG streams) | Proof 3 | `terraria-gen-game	ests	icks.rs` (VERIFICATION.md) |
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

# cs2rs: runtime model and translation rules

cs2rs translates the decompiled C# of Terraria 1.4.5.8 into Rust, method by method, using Roslyn's
semantic model (the `IOperation` tree, where every implicit conversion, default argument and overload
choice is already explicit). The output is two crates: `port\crates\terraria-gen-rt` (the hand-written runtime `rt`, the host
registry, the snippet tests) and `port\crates\terraria-gen` (generated modules `src\gen`, the generated host
boundary `src\host\api`, hand-written host providers `src\host\mod.rs`, the comparison with the real game in
`tests\reference.rs`).

The status of each rule (implemented and tested / implemented / designed only) is in the table at the end.
Nothing in this file is "supported" unless that table says a test exercises it.

## 1. The one idea: every C# value is a `Copy` Rust value, valid when all its bytes are zero

C# game code aliases freely: `Player p = Main.player[i]; p.velocity.X = 0; Main.item[j].position = p.position;`.
A borrow-checked ownership tree cannot express that mechanically, so the generated code does not use one.

| C# | Rust | all-zero means |
|---|---|---|
| `bool`, `byte` .. `ulong`, `float`, `double` | `bool`, `u8` .. `u64`, `f32`, `f64` | `false` / `0` |
| `char` | `u16` (one UTF-16 unit) | `'\0'` |
| `enum E : int` | the underlying integer type; members are literals with the name in a comment | `0` |
| `struct S` | `#[repr(C)] #[derive(Clone, Copy)] struct S` (assignment copies, as in C#) | `default(S)` |
| `class C` reference | `Gc<C>`: a raw pointer, `Copy`, compares by address | `null` |
| `string` | `Str`: pointer + length to immutable UTF-8, `Copy`, compares by content | `null` |
| `T[]`, `T[,]`, `T[,,]`, `T[][]` | `Arr<T>`, `Arr2<T>`, `Arr3<T>`, `Arr<Arr<T>>` (handles to a length + buffer) | `null` |
| `T?` | `Nullable<T> { has_value, value }` | `null` |
| delegate | `Delegate<dyn Fn(A, B) -> R>` (handle to a closure or an invocation list) | `null` |
| `List<T>`, `Dictionary<K,V>`, ... | `Gc<List<T>>`, `Gc<Dictionary<K,V>>` (runtime types) | `null` |
| `IEnumerable<T>` | `Seq<T>` (tagged handle: null / list / array / enumerator factory) | `null` |
| `object`, an interface, a class that is not translated (game or library) | `Obj` (a `Gc` whose target type is not known statically) | `null` |
| a library value type with no mapping (`DateTime`, `KeyboardState`) | an opaque struct declared by the host boundary | zeroes |

Consequences, all of them mechanical:

- Aliasing is exact. Two `Gc<Player>` to the same object are the same pointer; a write through one is seen
  through the other. Struct fields of classes are mutated in place: `p.velocity.X = 0` is
  `(*p.ptr()).velocity.x = 0.0`, a plain place expression. Nothing is borrowed, so nothing conflicts.
- `default(T)`, a fresh local, a fresh array element, a fresh object and a not-yet-initialised static are all
  "all bytes zero", exactly the CLR rule. `new C(...)` is: allocate zeroed, run field initialisers, run the
  constructor, in the CLR's order (derived initialisers, base constructor, derived body).
- No value needs `clone()`, `Drop` or a lifetime, so generated code is flat statements: cheap for rustc.
- Unwinding (section 7) leaves memory exactly as C# would: there are no guards to poison.

`Gc::ptr()` checks for null and throws `NullReferenceException`; `Arr::at(i)` checks null and bounds and throws
`IndexOutOfRangeException`. So a C# crash is a Rust panic with a typed payload, never undefined behaviour.

### Why not the alternatives

- `Rc<RefCell<T>>`: `a.x = a.y + f(a)` or two handles to one entity panic at run time with "already borrowed";
  every access needs `borrow()`; handles are not `Copy`, so structs holding them are not either and C# struct
  copies need `clone()` everywhere; cycles leak anyway. It adds failure modes C# does not have.
- Arena + index handle (`Handle<Item>` into `Vec<Item>`): same aliasing power as a pointer, but every field
  access is a global lookup, the arena cannot move while a `ref` parameter points into it, and inheritance
  (one handle type per class, upcasts) gets awkward. It buys serialisable handles, which nothing needs.
- The hand-port style of `PORTING.md` (`g: &mut Game`, indices instead of references, re-read after calls):
  needs a human to decide where the aliasing is. Fine for 50 methods, impossible for 19,783.

### Safety contract (the audited part)

Every generated function is `pub unsafe fn`. The contract of the crate is:

1. One thread runs the simulation. `rt::enter()` records the thread; debug builds assert it on every static
   initialiser. `Gc`, `Str`, `Arr` are `!Send`/`!Sync` (raw pointers).
2. Generated code never forms a long-lived Rust reference into a game object. Reads and writes are place
   expressions through raw pointers (`(*p).field`). `&mut` only exists for the duration of a call into a native
   method of a mapped value type (`Vector2::normalize`), and those do not call back into generated code.
3. Objects are never freed while reachable (section 9).

Hand-written callers (tests, the future app) call the generated functions inside `unsafe` and keep to rule 1.

## 2. Classes, inheritance, virtual calls, interfaces

- A class is `#[repr(C)] struct C { base: B, fields... }`. The base class is the first field, so a
  `Gc<Derived>` is also a valid `Gc<Base>` (pointer cast, no code). An inherited field is `(*p).base.width`.
- Every object is allocated by `rt::alloc::<C>(TYPE_ID)` with a 16-byte header in front of it holding the
  type id. `gen\types.rs` has one id per translated class and the parent table. `x is C`, `x as C`, `(C)x`
  walk that table (`rt::is_instance`, `rt::as_class`, `rt::cast_class`; the last throws `InvalidCastException`).
- Instance methods are free functions in the class's module with the receiver first:
  `item::set_defaults(this: *mut Item, type_: i32, variant: Gc<ItemVariant>)`. The call site passes `x.ptr()`,
  which is the null check C# does at `callvirt`. Inside the method `this` is known non-null, so its fields are
  `(*this).width`. Struct methods have the same shape (`this: *mut S`): C# passes a managed pointer too, so
  `v.Normalize()` on a field mutates the field.
- Virtual / abstract / override (closed world): the tool sees every override. For a virtual method declared on
  `B` it emits `b::m__v(this, ...)`, a `match` on the object's type id that calls the most derived
  implementation (`d::m`). Call sites of a virtual method call `m__v`; `base.M()` calls `b::m` directly.
  No vtables, no trait objects, and rustc can inline.
- Interfaces work the same way: `iface::m__v(this, ...)` matches on the type id over all translated implementers
  (classes, and structs through their box); `iface::implements(id)` answers `is` / `as` / casts. A value of
  interface type is `Obj`. A method of a LIBRARY interface that translated types implement (`IDisposable.Dispose`,
  `IComparable<T>.CompareTo`) gets a dispatcher in `gen\ifaces.rs`: translated implementations by type id, any
  other object through the host boundary.
- Boxing. Every object header holds a type id; ids below 64 are the runtime's (`rt::tid`: the primitives, `string`,
  the XNA structs, `Type`), translated classes AND structs are numbered from 64. `(object)x` for a value type is
  `rt::box_value(x, id)`: a fresh object holding a copy, as in the CLR; `(T)o` is `rt::unbox` (null:
  `NullReferenceException`, another id: `InvalidCastException`); `o is T` compares the id. A `T?` boxes as null or
  as a boxed `T`. `string` as `object` is a string object (`rt::str_obj`); arrays, lists, delegates are handles
  already and are cast back with a check of the Rust `TypeId` of their heap type (`rt::cast_rt`).
  Enums box with a type id of their own and `char` as `char` (section 8c). Known deviations: array covariance is
  modelled only where an array is passed as `IEnumerable<Base>` (a retyped snapshot).
- Generic code over `object`: every runtime and generated value type implements `rt::Trace`, which also carries
  the boxing rule (`BOX`, `to_obj`, `from_obj`) and `IComparable<T>` for primitives (`cs_compare`). That is what
  `(T)inputs[i]`, `(object)t`, `typeof(T).IsPrimitive` and `value.CompareTo(max)` compile to for a type parameter
  (`SetFactory.CreateCustomSet<T>`, `Utils.Clamp<T>`).
- Properties are functions `name(this)` / `set_name(this, v)`; auto-properties get a backing field
  `name__backing`. Static properties have no `this`.

## 3. Statics and static initialisation

One `Statics` struct per class, in a zeroed `static mut`, reached through `module::s()`:

```rust
pub unsafe fn s() -> *mut Statics { if STATE == 0 { cctor() } addr_of_mut!(S).cast() }
```

`cctor()` sets `STATE = 1` first and then runs the static field initialisers in source order followed by the
static constructor body. That is the CLR's behaviour on one thread: lazy, once, and a class that is reached
again while its own initialiser is running sees the fields assigned so far and zero for the rest. The game's
initialisation order therefore comes out of the code, not out of a hand-maintained list.
For classes with an explicit static constructor (precise initialisation in the CLR) static methods and
constructors also call `s()` on entry. `const` fields are Rust `const`s.

One struct per class rather than one global `Game`: the access path `(*main::s()).max_tiles_x` is computed from
the symbol alone, files stay independent (no 5,000-field struct that every edit recompiles), and laziness is
per class as in the CLR.

## 4. `ref` / `out`, evaluation order, assignment

- `ref T` / `out T` parameters are `*mut T`. The argument is `addr_of_mut!(place)`: a local, a field of an
  object, an array element (`a.at(i)`), a static. Inside the callee `x = 5` is `*x = 5`. Two `ref` arguments
  to the same variable alias, as in C#.
- Arguments and binary operands evaluate left to right in both languages. Named arguments written out of
  order are bound to temporaries in source order.
- Rust evaluates the right side of `=` before the place on the left; C# does the opposite. `place = value` is
  emitted as is only when that order cannot be observed: the value is a constant, or both sides are free of
  side effects, or the place depends only on locals / `this` that the value does not write. Otherwise the
  address is taken first: `{ let p = addr_of_mut!(place); let v = value; *p = v; }`. (Snippet `EvalOrder`:
  `a[i++] = i`, `a[w] = w++ * 2`, `holder.id = (holder = other).id + 40`.)
  Known deviation: C# reports a null receiver / bad index on the left only after evaluating the right side;
  the hoisted form reports it before. It is observable only if the right side has side effects and the
  exception is caught.
- `x op= y` is `x = (T)(x op y)` with `x` evaluated once and the narrowing cast C# inserts (`byte b += 1` wraps).
- `x++` as a value is `{ let o = x; x = o + 1; o }`. Assignment as a value yields the stored value.

## 5. Numbers (see `PORTING.md` "C# semantics that differ from Rust")

- `int` `+ - *` and unary `-` are `wrapping_*`. Constant expressions are folded by Roslyn and emitted as the
  folded literal. `/` and `%` go through `rt::div` / `rt::rem` (throw `DivideByZeroException`, and
  `OverflowException` for `MinValue / -1`). Shifts are `wrapping_shl/shr` (C# masks the count).
- Integer to integer casts are `as` (truncate / sign-extend like C#).
- Float to integer casts go through `rt::conv` and reproduce x86: NaN and out-of-range give `0x80000000` for
  `int` (`0x8000000000000000` for `long`); to `byte`/`short`/... is the `int` conversion followed by truncation.
- `float` is `f32` and `double` is `f64` as STORAGE. How a `float` expression is evaluated is the config option
  `floatMode` (see "Float evaluation" below): the real game is 32-bit code whose JIT computes on the x87 unit,
  and the reference dumps show the difference.
- `float.ToString()` / string concatenation use .NET Framework's "G7" (`double`: "G15"), invariant culture.

### Float evaluation (`floatMode`)

Measured against the real game (numbers in `VERIFICATION.md`): with every `float` operation rounded to 32 bits
(`"f32"`, the C# text taken literally) 301 NPC values, 57 projectile values and 14 collision families differ
from the game; all of them are of the form "the game truncated or compared a value that had not been rounded
yet". The rule the generator implements for `"x87"`:

- A tree of `float` `+ - * / %`, unary minus and `?:` is evaluated in `f64` (the x87 unit runs at 53-bit
  precision under the CLR). Leaves are widened exactly.
- The value is rounded to `f32` where it is stored or passed: assignment to any variable, field or array
  element, an argument, a `return`, an explicit `(float)` cast.
- `(double)tree` and `(int)tree` (any conversion out of `float`) take the unrounded value; comparisons of two
  `float` operands compare the unrounded values.
- `x op= tree` is one tree `x op tree`, rounded by the store.

`"x87-locals"` additionally keeps a `float` local whose address is never taken as an unrounded `f64` variable.

`"jit32"` (the whole game's mode since Proof 1) is `"x87"` plus the register allocation of the 32-bit JIT for
`float` locals, as established by probes of the real runtime in `port\crates\terraria-x87\FLOAT_RULE.md` (sections 3
and 5); `src\FloatAlloc.cs`:

- A method of 40,000 IL bytes or more (about 20,000 instructions; sizes from the reference database through
  `gen_il_sizes.py` -> `il_sizes.tsv`) is compiled without optimisation in the original: every local is in memory,
  i.e. the method behaves as `"x87"`. 20 methods, among them `NPC.AI`, `Projectile.AI`, the `SetDefaults` family and
  `Recipe.SetupRecipes`: that is why those were exact under `"x87"` already.
- Otherwise a `float` local is on the x87 stack (an `f64` variable; stores do not round) unless: its address is
  taken or a lambda captures it; it is read inside a `catch` / `finally`; its weighted reference count (x 8 per
  enclosing loop) minus twice the weight of the non-inlined calls it is live across is below 1; or it loses the
  competition for the six registers (candidates by weighted references, then declaration order; two variables
  interfere when one is written while the other is live). Liveness comes from Roslyn's control-flow graph.
- A stack variable is rounded at every non-inlined call it is live across (`x = x as f32 as f64;` in front of the
  statement that contains the call), and a stack variable already loaded when a call in the same expression runs
  (`a + F(z)`) is read rounded.
- Which calls are calls: virtual / interface / delegate calls and library methods are; XNA vector constructors and
  operators, `Math.Abs` / `Math.Sqrt`, auto-property accessors and translated methods of at most 32 IL bytes whose
  `float` result is not an arithmetic expression are inlined.

jit32: limits (each is a known difference to the rule, none is silent in the sense of a made-up value, all are
precision of the last bit): float PARAMETERS compete for registers but are never kept unrounded; a float RETURN value
is still rounded (the JIT returns the unrounded expression; fixed 2026-10-14 for direct calls, see below); the fields of a local struct (`Vector2 v`: `v.X`) are
not promoted to stack variables; a call in a loop header keeps the variables live across it in memory (counted in
`GAME_REPORT.md`); the "64 hottest locals" limit counts float variables only; `Math.Sin / Cos / ...` are still the
Rust library functions (the x87 crate is not mapped yet); ties among more than six interfering candidates are
resolved by declaration order (the JIT's order is not determined: the methods are listed in `GAME_REPORT.md`).
Read from the machine code of a run (2026-10-14, `tools\refdump_worldgen\CollisionJit.cs`: the collision reference
run repeated byte for byte, then its own code dumped): config `jitBodies` gives the x87 evaluation of an XNA member's
body (`Vector2.Length`: the sum of squares at double precision, rounded once after `fsqrt`; `Vector2.Normalize`:
`1 / length` unrounded), used only at the caller / callee pairs of `jit_bodies.tsv` where that run compiled a call
at every site; elsewhere the f32 `nativeMembers` template stays (the XNA library's own f32 arithmetic is NOT what
the original computes, but only those sites are proven). `floatSpillAtAlloc` names stack locals rounded at an array
allocation (`newarr` helper call) in one method each. Measured effect: VERIFICATION.md.
The same for the lighting reference run (2026-10-14, `tools\refdump_worldgen\LightJit.cs`, `work\jit\lightref`;
`jit_code.json` now says per method whether it was compiled before the dump began, i.e. ran): config `jitRefCalls`
(`jit_refcalls.tsv`, `gen_jit_refcalls.py`) lists, for the methods that ran there, which IL call sites were call
instructions, game callees included (one run has one answer; an inlined callee whose body still calls something,
`FastRandom.Next(int, int)` -> `Next(int)`, counts as a call). For those pairs it decides where stack values are
rounded, before `jitCalls` and the size rule; the size rule stays the register-allocation input. Two emitter rules
apply only at such proven calls: a call evaluated on some paths of its statement only (`a && F()`, a `?:` branch) gets
its rounding at the call itself (`{ v = v as f32 as f64; F() }`), not in front of the statement; and C05 holds for
`a += F()` as for `a + F()` (a is loaded before the call and spilled across it). Elsewhere the old placement stays.
Since 2026-10-14 (later) the three tables come from every reference run, not from collision and lighting alone:
`tools\refdump_worldgen\run_refjit.sh` repeats the 20 reference processes (p1, framing x2, collision, lighting, sky,
liquid x6, ticks x2, worldgen x6) with their own harness sources and an entry point that, after the run, reads the
code of every method that ran (`JitDump.cs DumpRan`: the CLR code header names the method and gives its exact size;
calls through a not-yet-compiled callee's MethodDesc slot are named). `jit_calls.call_answers` / `union` merge the
runs: a method whose answers differ between two runs (3 methods) is left out of `jitRefCalls` / `jitBodySites` and
keeps the whole-game answer in `jitCalls` (`jit_refcalls_conflicts.tsv`). `jitCalls` takes the reference runs' answer
where one ran the method (33 mscorlib pairs differ from the whole-game precompile). `CS2RS_FLOAT_REPORT=<file>` makes
the translator write its allocation per body; `gen_float_candidates.py` compares it with the machine code into
`float_mem_candidates.tsv` (a review list for `floatInMemory`, not applied).
Since 2026-10-14 (entities, `refdata/entities`): (1) config `jitFloatAlloc` (`jit_float_alloc.tsv`,
`gen_jit_float_alloc.py`): the real JIT's location of every float IL local of the methods that ran (the CLR's own debug
info, `JitDump.cs WriteDebugInfo`, `jit_debuginfo.py`), named through the portable PDB ILSpy writes for the same
decompilation (`tmeta pdblocals`); it replaces the rule for those methods, and a float local that is not an IL local
(an evaluation-stack value) is unrounded and never rounded at a call. (2) A float method called directly has an
`__x87` twin returning the unrounded f64 of its return expressions (Model.TwinF64); F64() calls the twin, a store
rounds. Virtual / interface / delegate calls still return rounded values. (3) `Math.Abs(float)` compiled as a call
(MinOpts method or proven) rounds its argument. (4) `0f - x` is emitted as `-x` (the IL is `neg`). (5) `x op= tree` is
one x87 expression for every place, properties included (the getter's twin when it has one). (6) `jitBodies` values
prefixed `f64:` return unrounded; vector and f64 bodies apply also where the run inlined the member. Limit left: the
fields of inlined XNA operator temporaries (promoted structs) are rounded per operator. `floatStackAcrossCalls` (a
config for evaluation-stack values) exists but is empty: jitFloatAlloc finds them.

## 6. Control flow

- A chain of five or more `if / else if` links is emitted flat: a labelled block in which each link ends with
  `break`. Written as nested `else { if .. }` the chains of `NPC.AI` / `Projectile.AI` (hundreds of links) were
  nested 880 levels deep, and rustc's front end is quadratic in that depth (8 minutes of `cargo check` against
  2.5 with the flat form).

- Every loop gets a label; `break` / `continue` are always labelled, so a `continue` inside a `switch` inside a
  loop means what it means in C#. `for`: `init; 'l: loop { if !cond { break } 'c: { body } incr }` with
  `continue` -> `break 'c`. `do/while` likewise.
- `switch` on integers / enums / chars / bools is a `match` inside a labelled block (`break` -> `break 'sw`).
  `switch` on strings matches on `rt::str_switch(value, &[...])`.
- `goto case`, `goto default`, `goto label` and labelled statements use one lowering: the statement list that
  owns the labels is cut into segments at each label (and at each `case` for a switch) and run as
  `let mut pc; 'b: loop { match pc { 0 => {..}, 1 => {..}, _ => break } }`; a `goto` is
  `{ pc = n; continue 'b; }`. Locals declared directly in such a list are declared (zeroed) before the loop.
  C# cannot jump into a nested block, so this covers every legal `goto`.
- Every local is initialised to zero where C# leaves it unassigned (C#'s definite-assignment analysis is not
  Rust's, and `out` arguments are passed by address).

## 7. Exceptions

`throw` is `rt::throw(e)` = `std::panic::resume_unwind(Box::new(Thrown(e)))`: an unwinding panic that does not
run the panic hook. `try { A } catch (T e) { B } finally { C }` is `catch_unwind` around a closure containing
`A`; the payload is tested against `T` (the `System.*` exception hierarchy is a table in `rt::exc`), `C` runs,
then the exception is rethrown if nothing matched. `return` / `break` / `continue` / `goto` that leave the
`try` body are returned from the closure as `Flow::Return(v)` / `Flow::Jump(n)` and performed after it.
The game uses `try { ... Main.tile[x, y] ... } catch { }` as a bounds check; that works because index and
null failures are thrown as the same typed payloads. One closure per `try`, none per statement.
`using (r) body` and `foreach` over an enumerator with `Dispose` are lowered to the same `try / finally`.

Exception classes declared by the game: the first field of such a class is the runtime's `Exception` (as a base
class would be), its `kind` is the nearest library exception type, so `catch (LibraryType)` works through the
kind table and `catch (GameType)` through the type id.

Rejected: `Result<T, Exception>` on every function. It doubles the size of every call site (`?`), slows the
hot path and rustc, to serve a path the game takes rarely.

## 8. Strings, arrays, collections, delegates, generics, LINQ, RNG

- `Str` is immutable UTF-8 with an "is ASCII" flag. `Length` and `s[i]` are defined on UTF-16 units as in C#
  (O(1) for ASCII, a scan otherwise). Lone surrogates cannot be represented. `==` is ordinal content equality.
  Literals are `static` data (`s!("...")`), no allocation.
- `Arr<T>`: `new T[n]` is zeroed; `at(i) -> *mut T` is the only accessor (read `*a.at(i)`, write
  `*a.at(i) = v`, `ref a[i]` is `a.at(i)`). `Arr2<T>` is row-major like the CLR. Array covariance is not modelled.
- `List<T>` wraps a `Vec<T>` with the version counter .NET uses to throw on modification during `foreach`.
  `Dictionary<K,V>` keeps .NET's entry array + LIFO free list, so enumeration order matches .NET (insertion
  order, removed slots reused last-freed-first). Keys hash/compare like C#: integers by value, strings by
  content, objects by address.
- `HashSet<T>` keeps .NET's slot order like `Dictionary`; `Stack<T>` enumerates from the top; `Dictionary.Keys` /
  `.Values` are sequences that throw when the dictionary changes under them. Keys of struct type compare as the
  default comparer does: the struct's `Equals(S)` + `GetHashCode()` when it has them, field-wise otherwise
  (generated `impl rt::CsKey`). A class that overrides `Equals` / `GetHashCode` or implements `IEquatable<T>` is a key by
  that equality (section 8c).
- `List<T>.Sort` / `Array.Sort` with a comparer run `ArraySortHelper<T>.DepthLimitedQuickSort`, the algorithm
  .NET Framework uses for a program that targets 4.0 (Terraria does): the sort is not stable, so the algorithm
  is part of the behaviour.
- Delegates: a lambda becomes a Rust closure behind a `Delegate` handle. Captured locals that are never
  reassigned are captured by value (exact). A captured local that is assigned after its declaration lives in a
  heap cell (`Gc<T>`, every use `*x.0`) shared by the method and the lambda, a fresh cell each time the
  declaration executes, as the C# closure class does. Invocation is `d.invoke(|f| f(args))`: arguments are
  evaluated once, every target of an invocation list runs in order, the last result is returned.
  `a + b` / `a - b` / `evt += h` / `evt -= h` are `Delegate::combine` / `remove` with .NET's rules (the last
  matching sub-list is removed; a method-group delegate has value identity: method + receiver).
  A field-like event is a delegate field `name__event`; every event also has `add_x` / `remove_x` functions (the
  custom bodies, or combine / remove on the field), which interface, virtual and custom events are subscribed
  through; library events go through their accessors on the host boundary.
- Generic classes and generic methods of the game: one Rust type / function per closed instantiation (section 8a).
- LINQ over `Seq<T>`: `Where`, `Select`, `Concat` are lazy (an enumerator factory behind the `Seq`), `Any`,
  `Count`, `First`, `Last`, `SingleOrDefault`, `FirstOrDefault`, `Contains`, `ToArray`, `ToList`, `Range`,
  `Distinct`, `OrderBy` produce their value when called (deviation for `Distinct` / `OrderBy`, which .NET defers).
  These are the operators the target classes use; any other is a host member. `yield` iterators: not supported.
- `Main.rand` is a `Gc<terraria_core::UnifiedRandom>`: the class is mapped to the verified bit-exact Rust
  type and `rand.Next(3)` is `(*rand.ptr()).next(3)` through the config's method table.

## 8a. Generic classes and generic methods of the game: monomorphised by the tool

Closed world: before anything is named or emitted, `Discovery` (`src\Mono.cs`) walks every signature and every
body of the configured types and collects each CLOSED instantiation of a generic game class and of a generic
method of a game class, to a fixed point (an instantiation found inside generic code, after substitution, is
walked too: `GWrap<string>` creates `GBox<List<string>>`). Nesting deeper than 6 type-argument levels stops with
a warning (unbounded instantiation).

- One Rust type per closed class instantiation: its own module (`ref__int`, `weighted_random__color`; two
  instantiations whose arguments spell alike get `_2`), struct, `TYPE_ID`, `Statics` and static initialiser. So
  `x is Ref<int>` is a type-id test like any other and every instantiation has its own static fields, as in C#.
- One Rust function per (generic method, type arguments): `clamp__g_int`, `try_get_at__g_te_item_frame`. No
  generated function is generic in Rust any more. A virtual / abstract / interface generic method gets every
  override and implementation in the same instantiation (the dispatcher needs them).
- Roslyn gives the SIGNATURES of a constructed symbol substituted (`Ref<int>.Value` is an `int` field). Bodies are
  bound once, in terms of the definition's type parameters; while a body is emitted for one instantiation the
  substitution is ambient (`Mono.Enter`, thread-static) and `Model.RT`, `Model.Get` and the emitter's type tests
  (`Zero`, `Box`, `Unbox`, `IsType`, `Conv`, `ToStringOf`, `Tid`) apply it. Member tables are keyed by the
  definition's member, the containing type by the constructed symbol as the use site writes it.
- What generic C# means after instantiation, as the CLR does it: `(object)t` boxes a value type and is a plain
  reference for a class; `t == null` is false for a value type; `default(T)`, `typeof(T)`, `new T()` (with or
  without initialiser) use the argument; a call through a constraint (`where T : IFoo`, `t.Foo()`) is a direct
  call on the struct's place (no boxing, the struct is mutated in place) and an interface dispatch for a class;
  `where T : Base` member access goes through the base layout.
- A generic definition nothing instantiates gets no module; its members are reported "generic type / method with
  no closed instantiation in the program". Instantiations that only reflection could create
  (`MakeGenericType`, `MakeGenericMethod`) are not found: those call sites are host members.
- Library generic members keep Rust type parameters on the host boundary, whatever the constraint
  (`ConditionalWeakTable<K, V>.GetValue` returns the caller's `V`): the earlier erasure to `Obj` for
  class-constrained parameters produced 340 type errors in the whole game.

Snippet `Generics` (tests\generics.cs): statics per instantiation, type identity, two parameters, instantiation
created by generic code only, generic struct, static generic class, generic interface on a class / a boxed
struct / a constrained call, non-generic base dispatching to generic subclasses, generic base with closed
subclasses, `new T()`, constraint calls, `default`, null test, `typeof`, `IComparable<T>`, `ref T`, generic
delegate, virtual generic method. `DoubleStack<T>` (the wire queue of `Wiring`) is the translated class now; the
hand port in `rt::sys` is no longer mapped.

## 8b. Culture

The game makes the selected language's culture the thread's culture (`LanguageManager.SetLanguage`,
LanguageManager.cs:97-98: `Thread.CurrentThread.CurrentCulture = culture.CultureInfo`), en-US at start-up
(Program.cs:197, `GameCulture.DefaultCulture`). It does not force the invariant culture. The runtime keeps the
current culture as an object (`rt::sys::CultureInfo`: the name) and implements ONE set of rules, en-US:

- number formatting: `ToString()` of the primitives (G7 / G15 for `float` / `double`), which en-US and the
  invariant culture print alike. Format strings and alignment (`string.Format`, `ToString("N2")`, interpolation with
  a format): section 14 (`rt::fmt`, en-US and invariant).
- culture-sensitive comparison (`string.Compare(a, b)`, `a.CompareTo(b)`, `Comparer<string>.Default` in `OrderBy` /
  `Sort`): `rt::sys::string_compare`, a model of the Windows en-US word sort for ASCII (letters without case,
  digits before letters, symbols before digits, hyphen and apostrophe ignored, lower case first on a tie).
  Non-ASCII characters sort after ASCII by code point: NOT the real weights. The expected orders in the unit test
  are written from the rule, not measured against .NET Framework.
- `ToLower` / `ToUpper` / `StartsWith(string)` / `IndexOf(string)`: as before (Unicode simple case mapping, ordinal
  search): equal to en-US for the game's ASCII keys, not verified beyond.

Number formatting is no longer part of that one set: since the completion stage `rt::fmt` formats with the current
culture's `NumberFormatInfo` (section 14), so a player who selects German reads "1,5" as in the original.
Comparison and case mapping remain en-US.

## 8c. Equality, hashing and the virtuals of `System.Object`

- The object header's `TypeInfo` (one per Rust payload type) carries `Equals(object)`, `GetHashCode()` and
  `ToString()` (`rt::Trace::obj_equals / obj_hash / obj_to_string`). The generator overrides them in a class's
  `Trace` impl when the class or a translated base overrides the C# method; boxed primitives, strings, XNA
  structs and translated structs implement the value rules; a native class may override them. So `o.Equals(x)`,
  `o.GetHashCode()`, `o.ToString()` and string concatenation work through any handle, whatever its static type
  (`rt::obj_equals`, `rt::obj_hash_code`, `rt::obj_to_string`), including on a native object.
- `rt::CsKey` is `EqualityComparer<T>.Default`: `IEquatable<T>.Equals(T)` when `T` implements the interface
  (`Trace::key_eq`, by the STATIC type as in .NET), else the virtual `Equals(object)`; `GetHashCode()` virtual.
  A class with neither compares by identity. Structs: `IEquatable<S>` when implemented, else the `Equals(object)`
  override, else field by field (`ValueType.Equals`). Used by `Dictionary`, `HashSet`, `List.Contains / IndexOf /
  Remove`, `Stack.Contains`, `Distinct`, `GroupBy`, `EqualityComparer<T>.Default`.
- `==` on references stays reference equality unless the type overloads the operator (that is C#, not a
  shortcut); strings compare ordinal.
- Hash codes are .NET Framework's for `int` and the other primitives, `string` (the 32-bit legacy algorithm),
  `Point`, `Rectangle`, `Color`, `Vector2/3/4`, user overrides. NOT reproduced: `Object.GetHashCode` of a class
  without an override (arbitrary in .NET too) and `ValueType.GetHashCode` of a struct without one (a fold of the
  field hashes here). `Dictionary` / `HashSet` enumeration order does not depend on hashes in this runtime
  (entry array + free list, as .NET Framework 4: tested by `Collections`, `MoreCollections`, `Equality`).
- Field-wise struct equality compares `float` fields with `Single.Equals` (NaN equals NaN, +0 equals -0); the CLR
  compares the BITS of a struct without references (so +0 and -0 differ there). Known deviation.
- Enums box with a type id of their own (one per enum the program mentions, after the class ids;
  `gen::enums::<NAME>_INFO`), so `o is E`, `o.ToString()`, `Enum.GetName / GetNames / GetValues / Parse / TryParse /
  IsDefined`, `typeof(E)` see the enum. Unboxing accepts the underlying integer and the other way round, as the
  CLR does. `char` boxes as `char`.

Snippet `Equality` (tests\equality.cs) covers all of the above against .NET 8; its expected output is the
.NET run, not hand-written.

## 8d. Iterator methods (`yield`)

The body of an iterator method is emitted unchanged inside an `async` block and rustc builds the state machine, as
the C# compiler does (`rt::iter`): `yield return v` is `__y.put(v).await`, `yield break` is `return`, `MoveNext()` is
one `poll` with a waker that does nothing. Nothing runs before the first `MoveNext`; locals keep their values between
steps; an exception leaves through `MoveNext` and ends the iterator.

- `IEnumerable<T>`: `rt::iter::enumerable` gives a `Seq<T>` whose every enumeration starts the body again with the
  arguments as they were at the call. `IEnumerator` / `IEnumerator<T>`: `rt::iter::enumerator` gives one enumerator
  object (`rt::iter::IterObj`). `IEnumerator.MoveNext / Current / Reset` and `IDisposable.Dispose` on an object held
  through the interface reach it through the `hostBodies` of `native.json` (the fall-through of the dispatchers in
  `gen::ifaces`).
- Collector: the state machine is a compiler-made `Future`, its layout is unknown to the tracer. A suspended machine
  is scanned conservatively (`Marker::mark_conservative`: every aligned word that is the address of a live object or
  of a string's data marks it). An `IterObj` scans its machine when reached; an enumerator made from an
  `IEnumerable<T>` iterator lives in a Rust box and is a root while it exists. Limits: a handle that the machine
  holds only inside a Rust-heap buffer of its own (a snapshot enumerator's `Vec` alive across a `yield`) is not seen;
  a word that only looks like a handle keeps an object for one more collection.
- Refused: `yield` inside `try` (the `try` body is a closure and cannot suspend; C# allows it only for
  `try / finally`, which includes a `foreach` over an enumerator that has `Dispose`), iterators returning the
  non-generic `IEnumerable`, `ref` / `out` parameters (C# forbids them too).

Snippet `Iterators` (tests\iterators.cs): laziness, re-enumeration, hand-stepped enumerators, nested iterators (the
shape of `Main.LoadContent_Deferred`), `yield break`, a `switch` around a `yield`, an exception through `MoveNext`.

## 8e. Anonymous types

`new { A = x, B = y }` is `rt::anon_new(&["A", "B"], &[boxed x, boxed y])`: an `rt::AnonObj` (property names in
declaration order + boxed values); the static type is `Obj`; `o.A` is `rt::anon_get(o, 0)` unboxed to the property's
type. Value equality, `GetHashCode` and `ToString()` (`{ A = 1, B = x }`) are the compiler-generated class's.
`o.GetType()` gives one `Type` per shape, and `TypeDescriptor.GetProperties` / `PropertyDescriptor.GetValue` work on
it (`rt::PropColl`, `rt::PropDesc`): that is how `Utils.FormatWith` and `LocalizedText.FormatWith` read the object
they are given. Snippet `Anonymous`.

## 8f. Resumable methods (config `"resumable"`)

Why: the browser build runs the game's threads on JSPI fibers, and a suspended JSPI stack cannot be scanned
(web/WEB_PORT.md 2.2), so no collector mark phase may end while a fiber is suspended. A thread that never ends
(the client's "TCP Client Thread", `Netplay.TcpClientLoop` -> `TcpConnectLoop`, `InnerClientLoop`, which sleeps
1 ms in every iteration while connected) would then stop every collection for the whole session.

Rule: every method listed in `"resumable": { "methods": [doc ids], "sleep": path, "try": path }` is emitted TWICE:
the plain function, unchanged, and `X__resumable`, an `async unsafe fn` with the same body, written by the same
emitter with three differences:
- `Thread.Sleep(ms)` is `<sleep>(ms).await` (terraria-host `threading::sleep_async`);
- a call of another listed method is `<that>__resumable(args).await`, so the callee's state is a field of the
  caller's machine (one heap block for the whole thread, no nested boxes);
- `try` (also `using`, `foreach` with `Dispose`, `catch` handlers) is `<try>::<R, _>(async { .. }).await` instead of
  `rt::try_(|| -> rt::Flow<R> { .. })` (terraria-host `threading::try_async`: `rt::try_` around every poll of the
  body). The body is a scope of its own exactly as the closure was: `return` / `break` / `continue` / `goto` leave
  it through `rt::Flow`, the `finally` code and `LeaveTry` are unchanged.
- a call of a library member listed in `"await"` (documentation id -> a native template ending in `.await`) uses
  that template instead of its `nativeMembers` one: a host wait that parks the machine (`TcpListener.AcceptTcpClient`
  -> `terraria_host::sys::net::TcpListener::accept_tcp_client_async`, which loops `try; threading::park_async()`).
Lambdas inside such a method stay plain (a closure cannot await). Allowed: non-generic methods without `ref` / `out`
parameters that have a translated body and are not iterators: static ones, and non-virtual instance methods of a
class (`this: *mut T` is a parameter like any other and lives in the machine; a resumable form may call only
STATIC resumable methods); anything else, and any emitter failure in the resumable form, stops the translation (a
thread that silently fell back to its plain form would suspend a stack again). The float statistics count each body
once.

The listed methods (game.json): the client's network thread (`Netplay.TcpClientLoop`, `TcpConnectLoop`,
`InnerClientLoop`) and, for the browser build's dedicated server (web/STATUS.md 2026-10-14, Host & Play), its main
loop `Main.DedServ` (an instance method the driver polls itself: terraria-host `threading::poll_main_machine` parks
it at every `Thread.Sleep`, terraria-web `server.rs`) and its threads `Netplay.ServerLoop`, `Netplay.BroadcastThread`
and `TcpSocket.ListenLoop` (instance; `AcceptTcpClient` awaited), and its console's "Server Input Thread"
(`Main.startDedInputCallBack` with `Main.ReadLineInput`; `Console.ReadLine` awaited: parked until the page's console
box has a line, web/www/HOST_ABI.md "Server console").

`gen::resumable::THREADS` lists the resumable forms that can start a thread (void, no parameters) by the method id
their `Delegate::from_method` carries (`Model.MethodId`, the hash the delegate emitter uses); each entry is a
`fn(recv: usize)` that is given the start delegate's receiver (the object of an instance method group, read from the
delegate's identity `(method id, receiver)`; 0 for a static one). A driver that
cannot scan suspended stacks hands it to the host (`threading::set_resumable_threads`; only terraria-web does).
The host then runs such a `new Thread(..)` on a fiber whose every entry is ONE poll of the machine: at a sleep of
1 ms or more the machine returns `Pending`, the fiber's body returns and its stack is gone, and the thread is
PARKED (continued by the next pump, as a sleeping thread is; `has_suspended_work` does not count it). The machine is
one Rust heap block that a collector root scans conservatively, like a suspended iterator method (8d); the roots
are traced again when a mark phase ends, so locals the machine stored without a barrier are found. Waits inside a
poll (host `Connect`, yield points, locks, sleeps under 1 ms) suspend the fiber mid-poll as before, and no mark
phase ends meanwhile. Natively nothing installs the table: the resumable forms are compiled but never called.

Evidence: host test `a_resumable_thread_parks_at_its_sleeps_and_collections_end_while_it_lives` (Windows fibers,
poison mode: four incremental cycles end while the thread lives, an array held only by the machine survives them, a
C# exception in a later slice is caught, an unreferenced control array is freed; with the root's scan disabled the
held array reads 0xDD.. instead); web/STATUS.md 2026-10-13 late night (the browser client connected for 3,000+
ticks).

## 9. Memory

`rt::gc`: a precise, non-moving mark-sweep collector that works in slices between frames. It never runs while a
generated function is on the stack of the running fiber, so handles in locals are never roots and no stack maps
exist.

- Every allocation (`rt::alloc`, strings built at run time, arrays, collections, delegates, boxes, heap cells)
  gets the 16-byte header: type id, a state word (white / gray / black / dead), a pointer to the Rust type's
  `TypeInfo` (trace, drop, size, name).
- Roots: the `Statics` struct of every class whose static initialiser has run (each `cctor` registers a
  generated tracer), what the host keeps in `rt::Rooted<T>` / `rt::add_root`, and the thread-static values of
  suspended work items (`rt::threads`).
- Tracing is exact: `rt::Trace` is implemented for every type that can be in an object (generated for each
  struct and class; closures report their captured handles through a tracer the translator emits next to them).
  `NEEDS_TRACE` is computed from the field types (`const`), so a struct of structs without handles costs nothing.
- Each thread has its own heap; generated code runs on one thread.

### The cycle (`gc_begin`, `gc_step`, `collect`)

1. **Start** (`gc_begin`): the roots are marked. Pause: the size of the roots (0.6 ms measured in the unoptimised
   build with a world loaded).
2. **Mark slices** (`gc_step(budget, may_finish)`): objects are traced from a work list until the budget is used
   up. Arrays above 2,048 elements are traced in pieces of 512 (`Marker::trace_slice`), so `Main.tile` (one
   array of 5 million handles) cannot hold a slice. The program runs between slices.
3. **End of the mark phase**, when the work list is empty and `may_finish` is true: the roots and the objects of
   `RESCAN` types (below) are traced once more with the program stopped, the work list is drained. This is the one
   pause that the budget does not bound; it is small because the barrier has already marked what the program
   moved (measured: at most 1.8 ms unoptimised, about 4,200 rescanned objects).
4. **Sweep slices**: unmarked objects are freed, a budget's worth per slice. Objects allocated during the sweep
   sit behind the sweep's end and are not looked at.

`collect()` is still there and exact: it completes a running cycle, then runs a whole one in one go. Tests and
tools use it; the app uses it only with `TERRARIA_GC_FULL=1`.

### What keeps the cycle correct while the program runs

The invariant is Dijkstra's: no black (traced) object refers to a white one unless the white one is also reachable
from a gray one or will be found at the end of the mark phase.

- **Store barrier** `rt::wb(&value)` / `rt::wbv(value)`: while a mark phase runs, the objects the stored value
  refers to are marked. Otherwise it is one load of a process-wide flag and a branch, and nothing at all for a type
  without handles (`NEEDS_TRACE` is a constant). The value, not the container, is what the barrier looks at, so it
  needs no knowledge of where a `ref` or a pointer leads. Emitted by the translator around the value side of:
  every assignment and compound assignment whose target is not a local of the method's own frame (`Emit.Wb`,
  `FrameStore`: a non-captured local or by-value parameter, or a field of such a struct variable); event
  subscriptions; property and field sets of library types; every handle-typed argument of a call into code that
  is not translated (runtime, host), which covers `list.Add(x)`, `dict[k] = v`, `SetValue(texture)` and the like;
  the reflection setters of `gen::meta`.
- **Objects allocated during a mark phase** are born gray (queued), so whatever the code that creates an object
  writes into it before the next slice needs no barrier: constructors, initialisers, `MemberwiseClone`,
  `ToArray`, a list made from a sequence.
- **`Trace::RESCAN`**, a constant per type, default `true`: "code may store a handle into a value of this type
  without the barrier". Heap objects of such types are kept in a list and traced again at the end of the mark
  phase. Translated classes and structs are `false` (unless they embed a native value that is `true`), as are
  arrays, `List`, `Dictionary`, `HashSet`, `Queue`, `Stack` (their storing methods call `wb`), delegates and
  strings. Everything else is rescanned: the host crate's objects that hold handles (`JToken`, effect passes,
  cues ...) and the state machines of iterator methods, whose locals live in the object and are written without
  a barrier. The default is the safe answer: a type nobody looked at costs time at the end of the mark phase,
  never correctness.
- **Arrays traced in pieces**: code that permutes handles inside an array without `wb` must shade them
  (`Array.Sort`, `Array.Copy` do).
- **Suspended work items** (fibers): their frames hold handles. The host hands the collector their stack memory
  (`rt::set_conservative_roots`), which is scanned conservatively with the roots, so a mark phase ends while work
  is suspended ("Changes from the completion stage" has the rules). Without that source installed (tests, tools) a
  mark phase does not END while a work item is suspended (`may_finish = false`).

What this costs: a load and a branch on every handle store outside a method's own frame; one cycle is as much
work as one collection was (about 0.1 s with a small world loaded), spread over 60 to 200 ticks; objects that
die during a cycle are freed by the next one (about 30 MB more heap at the default settings).

### Evidence

- `rt::gc` unit tests: an incremental cycle with a handle moved from an untraced part into a traced object
  (kept, with the barrier); the same moves without the barrier (the object IS freed: the test of the test);
  large arrays in pieces; `collect()` in the middle of a mark phase and of a sweep; a model-checked stress test
  with the collector running a few objects at a time between random mutations (nothing reachable is ever freed,
  a final collection leaves exactly the reachable objects).
- **The verifier** (`rt::set_verify`, `TERRARIA_GC_VERIFY=1`, `CLIENT_GC_VERIFY=1`): at the end of every mark
  phase the heap is walked a second time, independently, and the run panics, naming the type, if that walk reaches
  an object the incremental marking left white. A unit test shows it reports a store without barrier. The scripted
  play sessions ran with it on (27 cycles, no report).
- Poison mode and `gc-check` as before (`rt::set_poison`, the cargo feature): swept objects are filled with
  `0xDD` and never reused; every `Gc::ptr()` checks the state word.
- `bench_pauses` (ignored test, release): a heap shaped like the game's, full collection against slices.

Decision record (measured on the synthetic heap of `bench_pauses` and in the client): a whole collection is
95 - 150 ms (optimised) because 5.04 of the 5.81 million objects are `Tile`s, each one cache miss to mark and one
to sweep. Allocating the tiles in an arena that is live as long as its array (option b of the brief) would leave
0.77 million other objects: about 15 ms per collection, still four frames' worth, and `Main.tile[i, j] = new
Tile()` plus `Tile` handles held in locals, lists and entities make "the array holds the only reference" false.
A generational scheme needs the same barrier as an incremental one plus a remembered set per container, and its
major collections would still have to be incremental. So: incremental only, with the barrier on the value.

### Threads of the original (`rt::threads`, host `sys/threading.rs`)

Every thread of the original is a fiber of the one game thread. `[ThreadStatic]` fields (`Main.rand`,
`Main._rngs`, `WorldGen.generatingWorldOnThisThread`, `Item._DefaultAssignItemsToNewPlayer`) are registered by
their class's static initialiser and have one value per work item: a new one starts with the defaults, as a new
.NET thread, and the values are swapped at every fiber switch. The translator puts `rt::yield_point()` at the
head of every loop body (one load of a flag): while a work item runs, the host arms it after a time slice
(normal runs) or a fixed number of loop iterations (scripted runs: repeatable), and the work item gives the frame
back, so the loading screen is drawn while a world loads, saves or generates. A static initialiser is never
interrupted (`rt::CctorGuard`); monitors (`Monitor.Enter` / `TryEnter` / `Exit`; a C# `lock` statement is still
only its body) have owners, and the main thread entering a lock that a suspended work item holds continues that
work item until it leaves the lock. Work that a work item runs inline (`Task.Wait()` on a task that has not
started) is a thread of its own in the original and has its own context here too; the fiber remembers which
context was running when it gave the frame back. Actions a work item hands to the main thread and waits for
(`Main.RunOnMainThread(..).Wait()`) run at the wait, under the MAIN thread's context. One rule exists because of
the original's own code: a lock the main thread still holds after a work item has waited three frames for it is
entered anyway (`CaptureCamera.DrawTick` enters `_captureLock` every frame and returns without leaving it when
no capture is running: the main thread never waits, so such a lock was never going to be released).

## 10. Interop and gaps

The config (`pilot.json`) maps C# symbols, identified by Roslyn documentation ids, to Rust:

- The tables are `native.json`, `native_xna.json` and every `native.d\*.json` (one file per area and owner; a
  key defined twice is an error naming both files). `NATIVE_SCHEMA.md` is the precise schema with two worked
  examples; the summary here is only the outline.
- `nativeTypes`: `T:Microsoft.Xna.Framework.Color` -> `terraria_core::xna::Color` (`struct`), or `class`
  (then values are `Gc<rust type>`).
- `nativeMembers`: `M:...UnifiedRandom.Next(System.Int32)` -> `{this}.next({0})`. Templates use `{this}`
  (a place: the struct itself or `(*ptr)`), `{0}`.., and `{value}` for property setters (`get` / `set`).
- `nativeSlices`: static array fields served by const tables (`ItemID.Sets.IsFood` ->
  `terraria_data::sets::...::IS_FOOD`); only indexing and `.Length` are allowed on them.
- `stubs`: a callee or field that is not ported, with the value to continue with, chosen by a human and stated in
  the config with the reason: `rt::stub("Lang.GetTooltip", value)` records the use and returns it. A write to a
  stubbed field is a recorded gap.
- The host boundary (`Host.cs`). A member of a LIBRARY type (.NET, XNA, ReLogic, Steamworks) that the config does
  not map is called through one generated function per distinct member, `host::api::<namespace>::<type>__<member>__<parameter types>`,
  whose body goes to the single registry `rt::host`: `unimplemented` (a `void` member: recorded, logged once,
  execution continues), `unimplemented_value` (a value is needed: recorded, then a plain Rust panic that a
  translated `catch` cannot swallow), `configured` (the value from `stubs`), `new_opaque` (the constructor of a
  library class: a fresh opaque object, which translated code can only compare with null and hand back to other
  host members). `HOST_API.md` is the generated list: member, call sites, users, by namespace. It is the
  specification of the shim; a member leaves the list when `native.json` maps it to hand-written Rust.
- Game classes outside the translated set (the frontier): references are `Obj`, `new X(..)` is an opaque object
  (recorded), a `void` call is `rt::gap`, a value is `rt::unported` (panics).
- `cs2rs --reach <rust::path>` lists what a call of a function can reach that is missing (over-approximation
  by the static call graph); `SCALE_REPORT.md` lists the host values that static initialisers need.
- Anything else that is not translated: `rt::unported("Type.Member")`, which records the name and **panics**
  (a plain Rust panic, so a translated `catch { }` cannot swallow it). A `void` call that is not translated is
  `rt::gap("Type.Member")`: recorded, logged once, execution continues ("did nothing", the statement form of
  `unported!` in `PORTING.md`). The tool never picks a value on its own. `gen\UNPORTED.txt` lists every site.
- Partial types (`"members": [...]` in the config) exist so a pilot can read a few fields of `Main` or
  `Player` without translating them. Their other fields are not in the struct, the initialisers of those
  fields do not run, constructor bodies are replaced, and each of these is an `rt::gap` naming the type.
- A method the emitter cannot translate keeps its signature and gets the body
  `rt::unported_method("Type.Method", "reason")`; the report groups the reasons.

## 11. Names

`snake()` of `tools\gen_tables.py`, applied to types (module names), methods, properties and fields.
Rust keywords get a trailing `_` (`type` -> `type_`). Locals and parameters keep the decompiler's spelling.

- Namespace -> module path, class -> module: `Terraria.GameContent.Items.ItemVariants` ->
  `gen::terraria::game_content::items::item_variants`; nested class `A.B` -> `...::a::b`. The struct keeps the
  C# name (`Item`), statics are `Statics`.
- Overloads: if a class has more than one method with the same snake name, **every** member of that group gets
  `__` + its parameter types (`prefix__int`, `prefix__int_ref_bool`, no parameters: `__void`). All of them, so
  adding an overload upstream never renames silently, and the name is a function of the signature alone.
  Type spelling: C# keyword for primitives, snake of the type name otherwise, `ref_`/`out_` prefix, `_arr`
  suffix per array rank, generic arguments appended with `_`.
- Constructors: `ctor` (same overload rule) is the body; `new` / `new__...` allocates and calls it.
- Property `Name`: `name(this)`, `set_name(this, v)`. Virtual dispatcher: `name__v`.
- Remaining collisions after snake (e.g. `netDefaults` and `NetDefaults`): the later member in source order
  gets `_2`, `_3`; the report lists them.

## 12. Determinism and speed

Types, members and files are emitted in a fixed order (config order, then source order); no timestamps, no
hash-map iteration. Files are only rewritten when their bytes change, so cargo recompiles nothing after a
no-op run.

Stack depth is part of determinism. Roslyn's parser and binder, and the emitter, recurse once per nesting level
of the source, and `Projectile.AI` nests `else if` 880 levels deep. Roslyn guards its recursion with
`EnsureSufficientExecutionStack`; when the thread has too little stack left it does not crash: it reports
CS8078 ("An expression is too long or complex to compile") and replaces the whole compilation unit by skipped
text, so the file declares no types at all. The previous version parsed and emitted on thread-pool threads
(`Parallel.For`), where the stack already in use depends on how the pool nests work items, so now and then
`Projectile.cs` lost its types: the intermittent "type not found: Terraria.Projectile". The mechanism is
reproducible on demand: `cs2rs --parse-stack-kb 256 <Projectile.cs>` reports "0 type declaration(s), CS8078",
with 512 KB the file parses. Fix: every thread that parses, binds or emits is a dedicated thread with a 512 MB
stack (`Program.WorkerStack`, `Program.ParallelBig`; the main thread too), and a CS8078 on any syntax tree stops
the tool. The failure itself was not observed again (14 consecutive runs before the change, all identical), so
"this was the cause" rests on the mechanism matching the symptom exactly, not on a before / after rate.

## 13. Reflection (`gen::meta`)

The game reflects over its own types in few places. Every site outside `Terraria.Testing.Cloning`, and what serves it:

| site | what it asks | served by |
|---|---|---|
| `IdDictionary.Create` (ReLogic), `Lang.cs:459 / 544`, `SoundID.cs:1245` | `typeof(IdClass).GetFields(Static \| Public)`, `FieldInfo.Name / FieldType / IsLiteral / GetValue(null)` | static-field data of every type whose `typeof` the program takes (as before) |
| `AttributeUtilities.GetAttribute<T>(this Type)` / `GetCacheableAttribute<T, A>` (`ChatCommandProcessor.AddCommand<T>`, `ChatCommandId.FromType<T>`) | `type.GetCustomAttributes(false).OfType<A>()` | `TypeMeta.attrs`: constructor arguments and named field / property arguments of each attribute application as constants; a fresh attribute object per call, as the CLR makes them |
| `AttributeUtilities.GetAttribute<T>(this Enum)` | `type.GetField(name).GetCustomAttributes(false)` | `rt::type_get_field`; a field's attributes are empty (no translated attribute is applied to a field of the game) |
| `AttributeUtilities.GetAttribute<T>(this MethodBase)` (`DebugCommandProcessor`), `ChromaShader` (`GetMethods` + `RgbProcessorAttribute` + `Delegate.CreateDelegate`), `Program.ForceJITOnAssembly / ForceStaticInitializers` (`Assembly.GetTypes`, `RunClassConstructor`), `NewRuntimeMethods` (`typeof(GC).GetMethod`), `CrashWatcher` (`Exception.HResult` by reflection), `GraphicsUtils` / `Main.cs:3927` (private fields of XNA classes), `LeashedEntity.cs:534` (`Activator.CreateInstance(GetType(), true)`), `LocalizedText` / `Utils` (`TypeDescriptor.GetProperties` of an anonymous object), `WorldGenSnapshot` (`GenVars` members, debug) | methods, constructors, properties of arbitrary objects | NOT served: host members that stop loudly. None is on the dedicated-server initialisation; `MethodInfo`, `Activator` and `TypeDescriptor` have no runtime type yet |
| `Testing.Cloning` (deep clone by `Reflection.Emit`) | everything | not ported. It is NOT only test tooling: `Projectile.Damage` calls `DeepCloning.Clone<Projectile>` (reference database, `port_calls`), and `StateSnapshot` uses it for record / replay. To be replaced by clone functions generated from the semantic model when the simulation proofs reach it |

Served since the client stage:

| site | served by |
|---|---|
| `ChromaShader.BindProcessors` (`GetType().GetMethods(..)`, `Attribute.GetCustomAttribute(method, typeof(RgbProcessorAttribute))`, `Delegate.CreateDelegate(typeof(Processor), this, method, false)`) | config `"reflectMethods"`: `TypeMeta.methods` for the named types and every class derived from them (name, flags, attribute thunk, a thunk that binds a delegate of the method's own signature); `rt::MethodInfo`, `rt::obj_get_type`, `rt::type_get_methods`, `rt::attribute_get_custom_attribute`, `rt::delegate_create`. A type without method data is loud, never an empty list |
| `Program.ForceLoadAssembly` (`Assembly.GetTypes`, `GetMethods`, `PrepareMethod`, `RunClassConstructor`) | replaced (`nativeMembers`) by `terraria-gen-game\src\host\mod.rs force_load_assembly`: the generated `gen::types::STATIC_INITS` (every translated non-generic type with a static initialiser, in the metadata order of the executable, from `type_order.tsv`) and the progress counters; `"forceInitSkip"` leaves types out with a reason (they still initialise on first use) |
| `LocalizedText` / `Utils` (`TypeDescriptor.GetProperties` of an anonymous object) | section 8e |
| `(Exception)o`, `o is SomeLibraryException`, `o as ..` | `rt::as_exception` / `cast_exception` with `gen::types::is_exception` (translated exception classes) and the kind table of `rt::exc` |
| `x is LibraryBase` / `as` / casts where the library class has no Rust type of its own (`GraphicsResource`, `IGraphicsDeviceService`) | config `"typeTests"`; without an entry the test of a non-null object stops in the host. (Before this stage a type PATTERN `x is T v` and `x as T` between two C# types with the same Rust type were emitted as null tests: wrong, fixed) |

Data per type (`rt::TypeMeta`, one `static` per type in `gen::meta`, registered by `typeof` or `meta::register_all()`):
name (C# and CLR spelling), flags, boxing id, instance type id, base type's data, `fields` (static fields and constants
always; instance fields when `members`), `props` (when `members`), `attrs`. A field has `get(obj) -> object` and
`set(obj, value)` thunks (boxing / unboxing as a C# cast would), a property a `get` thunk that calls the translated
getter through its dispatcher. `members` is true for the types the config's `"reflect"` names and every translated
type their fields and properties mention (closure by declared type; a runtime type that is only reachable through an
interface or base-class typed member must be listed, and reading it without data is loud). `"reflectShallow"` forces
the static data of a type without following its fields (`Terraria.Main`).

`rt::obj_reflect(o)` is the other half: what a runtime container held as `object` contains (array, list, set,
dictionary: the elements boxed, in enumeration order; a delegate: the method ids of its targets). `gen::meta::
METHOD_NAMES` maps the id of a method-group delegate to `Declaring.Type+Nested.Method`.

Who uses instance members today: the proof (`tests/proof1.rs` reads the translated objects exactly as `tools/refdump`
read the real ones) and nothing in the game. Snippet `Reflection`.

## 14. Format strings

Every format the game contains (scan of both decompiles and the JSON resources):

- `ToString(format)` on numbers: `X2 X8 P P0 P1 F2 F4 D D2 D4 ##`; in composite strings and interpolations:
  `0.0% 0 0.0 00 X2 X8 P2 F2 F3 F4 G`, alignments `2 -2 3 5 7 10`; in the localisation files: `+0;-#` (24 uses per language).
- on `DateTime`: `d MMMM yyyy`, `MM-dd-yy_HH-mm-ss-ffff`, `yyyy-MM-ddTHH:mm:ss`, `MMMM d, yyy`, `yyyyMMddHHmmssfff`
  (the host crate's `DateTime`, currently switched off: see game.json `ignoreNative`); on `TimeSpan`: `\:mm\:ss`.

`rt::fmt` implements the Framework's algorithms (`NumberToString` for `D F N P X` and `G` without a precision,
`NumberToStringFormat` for custom patterns with sections, `RoundNumber`: decimal digits first, 15 significant for a
`double` and 7 for a `float`, then half-away-from-zero on those digits; `StringBuilder.AppendFormat` for composite
formats) and is checked against the real runtime: `tests\framework\FmtProbe.cs`, built with the .NET Framework 4
compiler and run as 32-bit code, wrote `fmt_probe.out` (5,212 lines: 23 integers x 27 formats x 5 integer types, 58
doubles and 36 floats x 30 formats, composite strings, `TimeSpan`); the unit test replays every line for en-US and
the invariant culture. `E`, `C`, `R` and `G` with a precision are not implemented (not used by the game) and stop loudly.

Culture: en-US and invariant differ in two things the probe showed: `P` prints `12.5%` in en-US and `12.5 %`
invariant; infinity prints `∞` in en-US and `Infinity` invariant (this is the culture data of Windows 10 and later;
older Windows printed `12.5 %` for en-US). The other eleven cultures of the game have their own data since the
completion stage (`rt::fmt::NumberFormat`, `tests/framework/culture_numbers.tsv` and `culture_fmt_probe.out`,
11,167 expected texts from the real runtime): separators, percent pattern, the words for NaN and infinity.

An interpolated string with a format or an alignment is `string.Format(composite, args)`, as the C# compiler emits.

## 15. Things that are deliberately nothing: `"inert"`

A `void` library member without an implementation is recorded and execution continues (`host::unimplemented`). On a
path under test that is a silent hole, so the proofs fail when one is reached. The config can declare a member
inert, with the reason next to it (`game.json`): the call is then counted (`host::inert`), not logged. Current
entries: the empty constructors of `System.Attribute` and `System.EventArgs`; `Game..ctor` (the host crate's `Game`
state is made by `terraria_host::game::init`); `TcpClient.NoDelay.set` (a limit, not a fact: there is no socket
host, the client object is never connected).

## Status

Implemented and tested / emitted but unproven / refused: the table "What is tested" at the end of
`COVERAGE.md`, and the refusal counts above it.

Not supported (refused per body, counted in `GAME_REPORT.md`): `yield`; local functions; `ref` reassignment;
switch expressions and patterns other than type / constant; tuples; anonymous types; `decimal`; pointers and
`fixed`; named arguments out of order with side effects in an indexer call. Counts and the reachable ones: `GAME_REPORT.md`.

## Changes from the pilot design, and why

| change | reason |
|---|---|
| `Gc` objects are freed (section 9) | the pilot leaked by design; a running game must not |
| `rt::Trace` on every value type; `alloc` needs it | precise tracing; it also carries the boxing rule for generic code |
| type ids start at 64 and structs have one | boxing and interface dispatch need an id for every value that can be an `object` |
| `Delegate<F>` is an invocation list, invoked with `invoke` | multicast delegates and events; the pilot's `get()` could only hold one closure |
| `Seq<T>` has a third kind (enumerator factory) | lazy LINQ, `HashSet` / `Dictionary` as `IEnumerable<T>` |
| library members without a mapping are host functions, not refusals | the pilot refused 4,094 call sites in 512 methods; a `SpriteBatch.Draw` call must not make a method untranslatable |
| untranslated classes construct opaque objects instead of panicking | `Main`'s static initialiser creates 40 UI / renderer objects it never looks into |
| `else if` chains are emitted flat | rustc front-end time (section 6) |
| `floatMode` | measured against the real game (section 5) |
| the runtime is its own crate | its tests and the snippets build in seconds instead of with the generated crate |
| `pilot.json` is superseded by `core.json` (whole classes) | partial types were the pilot's device; the target classes are translated whole |

## Changes from the scale-up design, and why

| change | reason |
|---|---|
| native tables are split: `native.json` + `native.d\*.json`, duplicates are errors (`includeDirs`) | a second worker maps the host API in parallel; with the old `include` rule (first definition wins, silently) two owners could shadow each other |
| a library override without an entry uses the entry of the member it overrides | `MemoryStream.Position` binds to the override; one Rust type stands for a C# class hierarchy, so one entry on the declaring class has to cover it |
| `is` / `as` / casts from `object` to ANY natively mapped class go through `rt::as_rt` / `cast_rt` (Rust `TypeId` of the heap object); optional `"is"` template when several C# classes share a Rust type | the previous rule was a hard-coded list of five collection types, so a cast to a host class was refused |
| a member of a native class called on a derived library class that is not mapped stays a host function | `CryptoStream.Dispose()` binds to `Stream.Dispose`; applying the template dereferenced an opaque object as an `rt::io::Stream` (caught by rustc, not at run time) |
| natively mapped structs box with `const { rt::box_id::<T>() }` (their `Trace::BOX`) | the previous rule knew the seven XNA structs by name; a host struct could not be boxed |
| type ids from `0x4000_0000` are reserved for host crates (`rt::tid::HOST_FIRST`) | host structs need ids that the generator can never assign |
| event accessors of library classes consult `nativeMembers` | they went straight to the host boundary, so a native class could not have an event |
| `System.IO.Stream` / `MemoryStream` / `FileStream` / `BinaryReader` / `BinaryWriter` and `TimeSpan` are native in `terraria-gen-rt` (`rt::io`, `rt::time`; `native.d
t_io.json`, `rt_time.json`) | needed by the headless proofs (world file) and as the worked examples of `NATIVE_SCHEMA.md`; to be reconciled with `terraria-host` if it brings its own |
| parse / bind / emit run on dedicated 512 MB-stack threads; CS8078 is fatal | nondeterministic loss of `Projectile.cs` (section 12) |
| `HOST_API.md` shows, per member, the `nativeMembers` key and the Rust signature | the host worker needs the exact key and types, not only the C# display name |

Differences from the hand-ported crates (`PORTING.md`), on purpose: no `g: &mut Game` (statics are reached
through `module::s()`), entities are `Gc` handles instead of indices. A hand-ported function is called from
generated code through `nativeMembers`; the other direction needs an adapter because the two sides keep
state in different places. That adapter is not designed yet and is the main integration risk.

## Changes from the native-mapping stage, and why

| change | reason |
|---|---|
| generic game classes and generic methods are monomorphised by the tool (8a); generated functions are never generic | 167 of the 312 errors of the whole-game probe; statics per instantiation, type ids, `new T()`, `typeof(T)`, constrained calls need the closed type |
| `DoubleStack<T>` hand port unmapped | the translated class replaces the stopgap |
| type parameters of LIBRARY members stay Rust type parameters whatever the constraint | `ConditionalWeakTable<K, V>` with a delegate `V`: the erased `Obj` did not type-check (340 errors) |
| `Equals` / `GetHashCode` / `ToString` in the object header; `CsKey` is the default comparer (8c) | a class that overrides `Equals` was a dictionary key by identity: wrong results, silently |
| enums box with their own id; `char` boxes as `char` | `ToString` / `Enum.GetName` / `is E` on a boxed enum printed and tested the integer |
| `ToString()` of any value in a concatenation or call (`ToStringOf`) | 26 refused bodies; class overrides were not called through `object` |
| event accessors are functions; `x.E += h` on an interface / virtual / custom event calls them | 196 members had no Rust function; interface events could not be subscribed |
| a translated class may derive from a native class: the native value is embedded as the first field (`rt::NativeSlot`), NATIVE_SCHEMA.md section 3a | `ClusterGroup : List<Cluster>`, `DitherSnake`, `Main : Game`; the earlier rule was a recorded gap and member calls that did not compile |
| a game class listed in `nativeTypes` is not translated as well | `UnifiedRandom` was emitted and mapped at once |
| `ignoreNative` (config) removes entries of another owner's table; templates whose signature mentions a removed type are not used | host `DateTime` has no `CsKey` / `CsOrd` yet; the table must not break the build |
| `--skip-native <prefix>` | snippets and the core set build without the host crate |
| reflection over STATIC fields of translated types (`gen::meta`, `rt::TypeMeta`); `Type.GetFields`, `FieldInfo` | `IdDictionary.Create`, `Lang`, `SoundID` build their tables that way on the start-up path |
| lambdas rebind by-value captures inside the closure; parameters of nested lambdas are not captures; an implicit receiver is not a capture of `this` | rustc errors of the whole game (unknown locals in tracers, `&mut` of a capture in an `Fn`) |
| `self::s()` instead of `s()` | a parameter named `s` shadowed the statics accessor |
| the dry run (`--all`, `coverage.sh`) is retired | the whole game is generated for real; `GAME_REPORT.md` has the numbers |
| `CS2RS_SURVEY=1` (runtime, diagnostic only) | lists every missing host value on a path in one run by continuing with zero; never used for a comparison |

Honest limits of this stage (also in VERIFICATION.md): `yield`, anonymous types, tuples, local functions, format
strings / alignment, unsafe code and attribute reflection are still refused or host members; a translated override
of a native virtual is not yet registered for calls from native code (the registry exists in `rt`, the generator
does not fill it); `cargo check` does not evaluate `const` blocks, so "check is clean" is weaker than "builds":
the test build of the whole game is the real gate and it passes.

## Changes from the whole-game stage (Proof 1), and why

| change | reason |
|---|---|
| reflection data carries instance fields, properties (get / set thunks), custom attributes and the base type; `"reflect"` / `"reflectShallow"` in the config; `rt::obj_reflect` for containers held as `object` (section 13) | `ChatCommandProcessor` reads attributes (the stop of the previous stage); the proof reads the translated state by reflection exactly as `tools\refdump` read the real one |
| `rt::fmt`: number formats and composite formatting of .NET Framework 4, verified against the real runtime; interpolation with a format / alignment is `string.Format` (section 14) | item tooltips and recipes use `string.Format` / `AppendFormat`; 13 bodies were refused |
| `rt::lq`: 27 more `System.Linq.Enumerable` members | host values on the start-up path (`SingleOrDefault()` ...) and elsewhere |
| `ConcurrentDictionary<K, V>` is the runtime's `Dictionary` | a `void` setter that logged and continued: a silent hole. Enumeration is in insertion order; the original's order depends on the machine's processor count (default concurrency level), so there is none to reproduce |
| `"inert"` in the config; `host::unimplemented_reached()`; the proofs fail on any other unimplemented `void` member or gap (section 15) | "logs once and continues" must not be possible on a path under test |
| float mode `"jit32"` (section 5) | 10 values of `ItemID.Sets.BannerStrength` differed in the last bit under `"x87"`: `float num = 1f / (strength + 1f); float output3 = (1f + num) / 2f;` in a 174-byte constructor, where the JIT keeps `num` on the x87 stack |
| a member of a mapped base class called on a mapped derived class with another Rust type uses the derived class's own entry (`MP3Stream.Dispose`) | the base template was expanded on the wrong Rust type (rustc error) |
| compound assignment on a property with an impure receiver (`writer.BaseStream.Position += 2`) evaluates the receiver once | `NetMessage.SendData` was a refused body |
| `"reachAssume"` (`--reach` only): branches a config fact proves dead are left out of the call graph | the static reach of `Main.Initialize` is 12,658 functions with the client branches, 11,806 without: still far too coarse to drive the work (the dynamic survey, `p1.sh --survey`, is what found the stops) |
| the host crate's tables are installed as byte copies again; `host_zz_cs2rs.json` holds this tool's templates over host types; JSON goes through the host crate (`rt::json` is no longer mapped) | the installed copies were about 320 entries behind; one JSON implementation |
| game classes that flow into the host's Newtonsoft members get `JsonObject` / `FromJToken` impls that stop loudly | the crate has to build; the member tables are not generated yet |

## Changes from the client stage (start-up to the menu), and why

| change | reason |
|---|---|
| iterator methods (8d), anonymous types (8e) | `Main.LoadContent_Deferred` / `LoadMusic_InSteps`, `ChatManager.LayoutSnippets`; `Language.GetTextValueWith(key, new { .. })` |
| method reflection, the force-load list, exception casts, `typeTests` (section 13) | `ChromaShader`, `Program.StartForceLoad`, `CrashWatcher`, `AssetRepository`, `AssetInitializer.TagAsset` |
| `"hostBodies"`: the body of a generated host-boundary function | members of library interfaces that runtime objects implement (`IEnumerator`, `IDisposable`, `IServiceProvider`, `IGraphicsDeviceService`): the dispatcher of `gen::ifaces` falls through to the host function, where a `nativeMembers` template cannot be |
| `"bodies"`: the body of a translated game method replaced by hand-written Rust | `XnbReader.FromStream<T>` (the platform edge of ReLogic's asset system): reached through interface dispatch, so rewriting call sites is not enough |
| `rt::Trace::obj_dispose` + `rt::dispose_obj` | `Dispose()` through `IDisposable` on a runtime / host object did nothing and said so only once; now the type's own `Dispose` runs (streams, readers, enumerators, `ThreadLocal`, the loader dispatcher) and a type without one is recorded by name |
| Newtonsoft member tables are generated (`Gen.JsonImpl`) with Newtonsoft's default contract, to a fixed point over member types | `TileMaterials` (`MaterialData.Materials.json`) at start-up; the 27 stubs "stop loudly" are gone. Not expressed, and loud when met: `[JsonConverter]` on a member, member types the host converter has no rule for (`Rectangle?`, XNA structs), a class without a parameterless constructor, `TypeNameHandling`, custom contract resolvers |
| interface dispatchers call the object's own override when the implementing method is virtual / abstract in a base class (`Gen.MostDerived`) | `SortingSteps.ByAttack : ByStat` (abstract `GetDisplayNameKey`) was "interface implementation not translated"; a virtual implementation overridden further down was dispatched to the BASE body (wrong, silently) |
| `List<T>.Capacity` follows .NET's growth rule; `ReadOnlyCollection<T>` is the list it wraps | `ChromaShader` loops over `_processors.Capacity` |
| Roslyn compilation uses `DesktopAssemblyIdentityComparer` | 52 binding errors: Ionic.Zip.CF references the Compact Framework's mscorlib, which Roslyn did not unify (`MapHelper..cctor` and every Ionic call site were refused bodies) |
| `rt::io` write guard (`add_read_only_root`) | terraria-host REQUESTS.md 2: a `FileStream` could write inside the game's install folder |
| diagnostics: `CS2RS_TRACE_THROW=<kind>\|all`, `CS2RS_TRACE_HOST=1` | a thrown C# exception / a missing host value carries no stack here; these print the generated functions on the Rust stack |

## Changes from the in-game stage (entering a world, scripted play, Proof 3), and why

- **Pointer code is translated** (it was refused: 15 bodies, among them `LiquidRenderer.InternalPrepareDraw` and
  `TileBatch`). `T*` is `*mut T`; `fixed (T* p = &place)` takes the address (arrays of the runtime never move: one
  allocation per array, freed only when unreachable), `fixed (T* p = array)` is null for a null or empty array;
  `*p`, `p->f`, `p[i]` are places; pointer `+ - ++ += -=` use `wrapping_offset` (an address outside the array is
  not undefined behaviour until it is dereferenced, as in C#); pointer difference, comparison and casts are the
  addresses; `sizeof(T)` is `size_of` of the `repr(C)` struct. Array buffers are aligned to 16 so that a `byte[]`
  read through a struct pointer (`MapHelper`) is aligned. Roslyn has no public operation for these forms: they are
  `OperationKind.None` nodes recognised by their children (`Emit.cs`, "pointers"). Snippet `PointerBasics`.
- **`ref` locals can be re-pointed** (`r = ref a[i]`), `_ = ref a[i]` evaluates the place (bounds check).
- **A call the decompiler printed as `M(out var x, out x)`** (one IL local for two `out` parameters; invalid C#,
  CS8196) is recovered from Roslyn's invalid node: the call gets the address of the one local twice
  (`InvalidOutVarCall`). `TileDrawing.DrawTiles_EmitParticles`.
- **`List<T>.Enumerator` is a value** (`rt::ListEnum<T>`, `Copy`, traced); boxed it becomes an enumerator object.
  **`ArraySegment<T>`** is `rt::sys::ArraySegment<T>`. **Array covariance**: an array of class handles under
  another element type is the same array; `string[]` as `object[]` is a boxed copy (strings are not object
  handles here), which the game's uses (format arguments) cannot observe.
- **`x.P++` on a receiver with side effects** evaluates the receiver once (as the compound assignment did).
  **`list.Sort()`** of a class with `IComparable<T>` calls its `CompareTo` (null first).
- **`MemoryStream(byte[])` wraps the array**, as .NET does (`CachedBuffer`: a writer builds packets in a shared
  array). **Streams have a close hook**: `CryptoStream` and a compressing `DeflateStream` finish on `Dispose` and
  close the stream they wrap.
- **The collector finds handles in two kinds of Rust heap blocks owned by a suspended iterator method** (the
  buffer of a snapshot enumerator, the boxed enumerator of a lazy sequence): they register themselves
  (`gc::register_scan_block`) and are scanned when a conservative scan finds a pointer to them. Before, such
  handles were missed (a use-after-free waiting to happen; the test has a negative control). The conservative scan
  no longer builds a set of every object address per collection: candidate words are resolved in one pass over
  the object list.
- **Float rule**: `Math.Abs(float)` inside an x87 expression is not rounded (FLOAT_RULE.md section 2);
  `floatInMemory` in the config lists locals the real JIT keeps in memory against the allocation rule, each with
  its evidence (one so far, found by Proof 3; FLOAT_RULE.md section 4.8).
- **Survey mode covers refused bodies**: with `CS2RS_SURVEY=1` a refused body or an unported value continues with
  zero and is listed, so one run shows every stop of a path (diagnostic only).

## Changes from the play-blocker stage (incremental collector, loading, Proof 2), and why

- **Section 9 is rewritten**: the collector works in slices with a store barrier on the value side of stores
  (`rt::wb`), `Trace::RESCAN` for types whose stores are not behind it, and a verifier. Reason: a whole collection
  walked 5.8 million objects (0.13 - 0.3 s every 6 seconds of play).
- **`NEEDS_TRACE` of a translated type is a `const` expression over its fields' types**, no longer "has a field
  that is not a primitive": arrays and stores of structs without handles are free.
- **`[ThreadStatic]`, yield points, `CctorGuard`** (`rt::threads`): section 9, "Threads of the original". Reason:
  world load / save / generation froze the frame; giving the frame back in the middle of such work is only
  correct when the work has its own `Main.rand`, as the original's worker thread has.
- **`[StructLayout(LayoutKind.Explicit)]`** (section 1's table says a struct is `#[repr(C)]` with its fields in
  order): for a struct whose fields are all primitives at naturally aligned offsets, the fields that own their
  bytes are laid out at their offsets (`#[repr(C, align(n))]`, padding fields, a compile-time size assertion) and
  every field that overlaps an earlier one (a C# union member) is a place expression of its own: the bytes at its
  offset, cast to its type (`Emit.FieldPlace`, `Gen.InstPlace`). Any other explicit struct keeps the sequential
  layout and is named on stderr when the tool runs. Found by Proof 2: `WorldGenerator.HashWorld` hashes
  `TileSnapshot.TileStruct` through its overlaying `int` fields.
- **Newtonsoft member tables** (section 13 / `Gen.JsonImpl`): member order as the game's dll writes it (all
  fields, then all properties; a type's own members before its base's; measured by
  `terraria-host\tests\dotnet\achievements_probe.ps1`), `[JsonObject]` inherited, an object written by its
  run-time type, get-only public properties written, `StringEnumConverter` on enum members, a class without a
  parameterless constructor made through its one public constructor, and the config key `jsonTypes` for types
  that only ever reach the serialiser as `object` (`Achievement`). An `int` member is BSON Int32.
- **`{VD<n>}` in native templates**: the `VertexDeclaration` of type argument n, taken from a default instance of
  the translated vertex struct through its `IVertexType.VertexDeclaration` getter. XNA's generic
  `DrawUserIndexedPrimitives<T>` does the same; a host function cannot ask a translated struct for anything.

## Changes from the completion stage (menus, more worlds, sockets, cultures), and why

Translator:

- **A library collection interface seen as `IEnumerable<T>`** (`ICollection<T>`, `IList<T>`, `IReadOnly...`: no
  Rust type of their own, the value is an `object`): `rt::sys::obj_seq::<T>(o)` finds the runtime collection behind
  it by its Rust type (list, array, queue, stack; anything else is an `InvalidCastException` naming the type).
  `ICollection<T>.Clear()` through such a handle is a virtual of the object (`Trace::obj_clear`,
  `rt::collection_clear`). Reason: five refused bodies, among them `SecretSeedsTracker.SetstringsFromConfig`
  (`Main.LoadSettings`) and `ZipContentSource.Refresh`.
- **`{rawN}` arguments are rendered without their implicit reference conversion at all** (before: converted,
  then thrown away, and a conversion without a rule refused the body): `token.Value<T>()` of Newtonsoft's
  `Extensions` (`JToken -> IEnumerable<JToken>`), `ResourcePack.LoadManifest`.
- **IntPtr conversions** (Roslyn's built-in `IsIntPtr` kind: `(int)handle`, `(IntPtr)n`): the same number under
  the other type. The Windows message hooks of ReLogic translate now (they stay unreachable: the platform layer is
  the host's).
- **`System.Decimal`** is `rt::decimal::Decimal` (96-bit magnitude, sign, scale; `GetBits` layout): constants
  (`from_bits`), the numeric conversions .NET has built in (`(decimal)int`, `(decimal)float` with 7 significant
  digits half-to-even, `(int)`, `(float)` through double), `==` / `!=` / `<` ... and `/` (as many decimals as fit
  96 bits, the last one half-to-even). Roslyn shows no operator METHOD for these; `Emit.Binary` and `Emit.Conv`
  write the calls. Checked against the real .NET Framework (`tests/framework/decimal_probe.out`). Addition,
  multiplication and formatting with a format string do not exist: a use is a refused body, not a wrong result.
  Reason: `UIAchievementListItem` (the achievements menu) prints tracker progress through `Tuple<decimal, decimal>`.
- **`IEnumerable<T>.GetEnumerator()` called by hand** on a runtime sequence is an enumerator object
  (`rt::iter::enumerator_of_seq`); a host member of `IEnumerable<T>` called on a runtime collection converts the
  receiver first (`ParticleOrchestrator.MagnetFakeFish`: `query.OrderBy(..).GetEnumerator()`); the struct
  enumerator of `Dictionary<K, V>.Values` is such an object too (`HotkeyCollection.GetEnumerator`).
- **Explicit layout with struct members** (`[FieldOffset]` on fields that are themselves structs: the Win32 `INPUT`
  union): size and alignment of a member are computed from the symbols (`Model.LayoutOf`: primitives,
  pointer-sized values, sequential structs of those), so the order in which types are built does not matter. A
  compile-time assertion checks the size as before.
- Refused bodies 62 -> 48: the 36 of the Reflection.Emit deep cloner and 12 that cannot run in this build
  (VERIFICATION.md has the evidence per body). `CreativeUnlocksTracker.ValidateWorld` "calling itself" is the
  ORIGINAL's code, not a translation defect: its IL is one `call` of its own token (`port_calls`: src 100673970 ->
  dst 100673970), nothing else calls it, and the translation says the same.

Runtime:

- **Number formatting follows the thread's culture** (section 8b, 14): `rt::fmt::NumberFormat`, one per culture of
  the game, data read from the real .NET Framework (`tests/framework/culture_numbers.tsv`); `float.ToString()` of
  translated code (`rt::to_str`) goes through it. The earlier statement "every other culture is formatted as
  en-US" is no longer true. String comparison and case mapping are still the en-US model.
- **The collector can finish while generated code is suspended** (section 9, "Suspended work items" is replaced):
  `rt::set_conservative_roots` takes the host's source of suspended stack memory (fibers that gave the frame
  back; the main loop of a dedicated server asleep). Those ranges are scanned with the roots (cycle start, end
  of the mark phase, verifier): EVERY word that could be an address is a candidate
  (`Marker::mark_conservative_stack`), and `resolve_candidates` marks an object when a candidate points anywhere
  into its payload (a handle, `this`, a pointer to a field, the bytes of a string) or into the buffer it owns
  (`Trace::obj_buffer`: an element of an array, an item of a list: compiled code may keep only that pointer). One
  pass over the ADDRESSES in the object list: a bit filter over the candidates' pages rejects almost every object
  without reading its header (2026-10-12: reading every header was a cache miss per object, 46 ms for 1.6 million
  objects in a multiplayer session); then the heap's list of wide objects (payload above 4096 bytes or an owned
  buffer: arrays, lists). A wide entry caches the range a word can hit: a large payload's at allocation, an
  array's buffer when `alloc_init` made it (`Trace::BUFFER_FIXED`; arrays are made by the hundred thousand per cycle
  in a session); such entries are tested against the filter without reading the header. A list's buffer moves:
  its header is read at every resolution. Words already resolved in the cycle are only tested against the moving
  buffers (what they pointed into was marked then; objects made since are born marked). The unmarked entries of the
  wide list are dropped by the first sweep slices, not in the pause. Unit tests with negative controls:
  `conservative_words_keep_objects_by_any_inner_address`, `an_old_word_under_a_moved_list_buffer_keeps_the_list`.
  Rejected after measuring: an address-ordered index built in sorted runs during the mark slices (60 - 110 ms of
  CPU per cycle to build against two passes of about 5 ms).
  The host records the range at every fiber switch, with the callee-saved registers spilled into the frame that
  switches (`threading::switch_to_main`). Unit test with a negative control:
  `a_collection_keeps_what_only_a_suspended_fiber_holds`. Not covered: a handle that exists only in an XMM
  register across the switch, and a pointer into a `Dictionary` / `HashSet` buffer with no pointer to the
  collection itself (their accessors are calls on the collection, which keep it on the stack).
  Reason: a hosted or joined game has suspended fibers all the time (the network threads); without this no
  collection could ever finish in multiplayer, and a long generation kept its garbage.
  **Measured cost, not solved:** that pass over the object list happens inside the one pause that ends a mark
  phase. With 2.5 million objects (a small world on a client) the pause was 23 - 44 ms on average and 72.5 ms at
  most in the multiplayer sessions (1.0 - 1.5 passes per cycle), against 1 - 3 ms when nothing is suspended. An
  address-ordered index of the heap (or resolving in slices with the barrier keeping the result valid) would remove
  it; neither was built. What the scan bought in a world generation from the menu is small, because almost all
  of that heap is live: 12 collections freeing 89.7 MB with it, 11 freeing 73.0 MB without
  (`work\item5\yield_on.txt`, `noscan_g15.txt`).
- **Large array buffers are freed on a helper thread** (`rt::arr::free_buffer`, 256 KB and more): giving tens of
  megabytes back to the allocator takes milliseconds, and the sweep frees inside a time slice of the game thread.
  The thread runs no code of the program.

Host:

- **Sockets** (`terraria-host/src/sys/net.rs`, module docs are the reference): `TcpClient`, `Socket`,
  `NetworkStream`, `TcpListener`, `UdpClient`, `IPAddress`, `IPEndPoint`, `Dns` on non-blocking OS sockets.
  `BeginRead` / `BeginWrite` register an operation; `net::pump` moves them without blocking; the callbacks of the
  finished ones run as work items (queued by `run_pending_work`, or inline when a thread of the game sleeps: the
  I/O threads of the original deliver while that thread sleeps). `AcceptTcpClient` and `Connect` give the frame
  back while they wait.
- **The game's endless threads are ordinary fibers** (threading.rs, "Threads that never end"): a fiber of a
  `Thread` object that sleeps a millisecond or more is suspended at once and is idle; idle fibers do not stop a
  script's clock; a sleep of 16 ms or more is honoured in real time; `Thread.Abort` deletes a suspended fiber.
- **The dedicated server is this executable with `-server`** (`terraria-app/src/server.rs`): "Host & Play" of the
  original starts `TerrariaServer.exe` and joins it, so `Process.Start()` of that name starts the port's own
  server. Its main loop is the game's (`Main.DedServ`); `threading::set_blocking_main` makes `Thread.Sleep` on the
  main thread pump the work, run the collector and really sleep.

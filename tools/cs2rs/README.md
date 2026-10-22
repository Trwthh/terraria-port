# cs2rs

Translates the decompiled C# of Terraria 1.4.5.8 into Rust, one method at a time, with Roslyn's semantic
model. The runtime model and every translation rule are in [DESIGN.md](DESIGN.md).

| file | what it tells you |
|---|---|
| [VERIFICATION.md](VERIFICATION.md) | the translated code against the real game: Proof 1 (the translated `Main.Initialize()` as one call, every reference dump compared: 3,911,380 values, all exact), collision, tile framing, lighting |
| [GAME_REPORT.md](GAME_REPORT.md) (generated) | the WHOLE game (`game.json`): per type clean / unported inside / failed / not emitted, refusal reasons |
| [HOST_API_GAME.md](HOST_API_GAME.md) (generated) | library members the whole game still calls through the host boundary, with the host crate's tables applied |
| [SCALE_REPORT.md](SCALE_REPORT.md) (generated) | what `core.json` generates: per class clean / unported inside / failed, the frontier, host values initialisers need |
| [HOST_API.md](HOST_API.md) (generated) | every library member the translated code calls: the specification of the XNA / ReLogic / System shim |
| [COVERAGE.md](COVERAGE.md) | "What is tested" (hand-maintained) and the last dry-run tables, kept for the record (the dry run is retired) |

Needs the .NET 8 SDK (Roslyn is taken from the SDK folder, nothing is downloaded), Python 3 and the Rust
toolchain pinned in `port\rust-toolchain.toml`.

## Run

From `tools\cs2rs` (Git Bash; the scripts are a few lines each, read them for the raw commands):

| command | does | time here |
|---|---|---|
| `sh run.sh` | builds the tool; regenerates `native_xna.json`, the snippets (`port\crates\terraria-gen-rt\src\snip`), the core set (`port\crates\terraria-gen\src\gen`), the whole game (`port\crates\terraria-gen-game\src\gen`), the host boundaries, the four reports | 90 s (whole game alone: 25 s) |
| `sh p1.sh` | Proof 1: regenerates the whole game, builds `tests\proof1.rs` of `terraria-gen-game` without optimisation and runs it; prints the steps, the table per reference dump and what was reached that is not game code. `--nogen` skips the regeneration (a change of the test alone relinks in 3 s) | 4 - 5 min |
| `sh p1.sh --survey` | DIAGNOSTIC, proves nothing: `tests\survey.rs` runs every statement of `Main.Initialize` / `Initialize_AlmostEverything` caught on its own, so one build lists the first stop of every sub-initialiser | same build |
| `sh p1.sh --client` | DIAGNOSTIC survey of the CLIENT start-up (`tests\client.rs`, `Main.dedServ == false`, host with no GPU): `Program.LaunchGame` / `RunGame`, `Main.Initialize` + `ClientInitialize` + `LoadContent` statement by statement (the statement list `tests\client\steps.rs` is written from the generated code by `mk_client_survey.py`), 600 splash ticks of `Update` / `Draw`, `Initialize_AlmostEverything` + `PostContentLoadInitialize` statement by statement, `CLIENT_FRAMES` (default 30) menu ticks, then every static initialiser of the force-load list. Prints one `STOP` line per distinct stop. `CS2RS_TRACE_THROW=<exception kind>` / `CS2RS_TRACE_HOST=1` print where an exception was thrown / a host value was missing | same build; run 30 s |
| `sh p1.sh --client-whole` | the same sequence as the app performs it (single calls, `src\host\launch.rs`); fails on any stop, unimplemented `void` host member or gap | |
| `CLIENT_SCRIPT=<abs path of an app input script> sh p1.sh --client --nogen` | DIAGNOSTIC: a scripted session without a GPU (menus, entering a world, play), every `Update` / `Draw` caught, the script's `dump` lines printed as `state ...`. `CLIENT_WORLD=<file.wld>` puts a world into the scratch save root, `CLIENT_SAVE_FROM=<folder>` a whole earlier save root (second-start paths), `CS2RS_SURVEY=1` continues past refused bodies and missing host values with zero (lists every stop of a path in one run; proves nothing), `CLIENT_POISON=1` runs the session with the collector's poison mode (with `--features gc-check`: the gc-check session) | run 40 - 100 s |
| `cargo test -p terraria-gen-game --test ticks` (from `port`) | Proof 3: the per-tick simulation against `refdata\ticks` (`TICKS_SCENARIO=world` for the 3,600-tick scenario); VERIFICATION.md | 15 s / 55 s |
| `cargo test -p terraria-gen-game --test saved_world` | a world saved by a client session, read by the verified reader and diffed against the original (`SAVED_WORLD`, `ORIGINAL_WORLD`, `EXPECT_TILES`) | 1 s |
| `sh run.sh --reach terraria::main::initialize` | same, and prints what a call of that function can reach that is missing | |
| `sh test.sh` | `run.sh`, the runtime + snippet tests, the comparison with the real game | 8 min |

Output is deterministic: files are written only when their bytes change, so a second `sh run.sh` reports
`(0 changed)` and cargo rebuilds nothing.

Then, from `port`:

```
cargo test -p terraria-gen-rt -j 4                                 # runtime unit tests (76; one of them runs all 41 snippets): 6 s
CARGO_TARGET_DIR=target-gen/fast CARGO_PROFILE_DEV_OPT_LEVEL=0 CARGO_PROFILE_DEV_DEBUG=0 \
  cargo test -p terraria-gen-game -j 4 --test sim -- --nocapture    # collision / lighting / framing on the Proof 1 initialisation: run 87 s
cargo check -p terraria-gen-game -j 4                              # the whole game: 3 min 15 s, 0 errors
cargo test -p terraria-gen-rt -j 4 --features gc-check              # the same with liveness checks on every dereference
cargo test -p terraria-gen -j 4 --test reference -- --nocapture     # against the real game: build 5.5 - 7 min, run 38 s
CS2RS_ONLY=collision cargo test -p terraria-gen --test reference    # one section (items, npcs, projectiles, collision, lighting, framing, gc)
```

## Crates

| crate | content | build |
|---|---|---|
| `port\crates\terraria-gen-rt` | hand-written: `rt` (handles, collector, strings, arrays, collections, exceptions, `System` helpers; 5,900 lines), the host registry, the generated snippet tests | seconds |
| `port\crates\terraria-gen` | generated: 857 modules, 684,000 lines of Rust (`src\gen`), the host boundary (`src\host\api`, 2,900 lines); hand-written: `src\host\mod.rs` (localisation providers), `tests\reference.rs` + `tests\suite\*.rs` | `cargo check` 2 min 40 s; test build at opt-level 1 with debug info 5.5 - 7 min; largest rustc process 5.9 GB (sampled) |

| `port\crates\terraria-gen-game` | generated from `game.json`: the WHOLE game + ReLogic, 2,763 files; depends on `terraria-host` (natively implemented XNA / System classes); `tests\proof1.rs` (Proof 1), `tests\sim.rs` (collision, lighting, framing), `tests\survey.rs` (diagnostic) | test build at opt-level 0 without debug info 3.5 - 4.5 min, about 5.5 GB |

`terraria-gen` (the core set) is kept as a regression gate only: Proof 1 passes on `terraria-gen-game` and every
comparison of the core set runs there too (`tests\sim.rs`); VERIFICATION.md recommends retiring it. Each generated crate is one crate on purpose for now: its classes call each other in cycles (`Player` -> `NPC` ->
`Main` -> `Player`), so a split needs `extern` declarations and cross-crate symbols; with the front end at under
3 minutes it was not needed. The one thing that was pathological for rustc is fixed in the emitter (DESIGN.md
section 6: flat `else if` chains).

## Files

| file | what |
|---|---|
| `src\Program.cs` | command line, Roslyn compilation, file tree, reports, `--reach` |
| `src\Model.cs` | config, the name rule (`snake()` of `tools\gen_tables.py`), C# type -> Rust type, member tables |
| `src\Emit.cs` | one method body (`IOperation` tree) -> Rust |
| `src\Gen.cs` | one module per C# type: struct, statics, constructors, dispatchers, `Trace` / key impls, reflection for the tests |
| `src\Host.cs` | the host boundary: one Rust function per unmapped library member, `HOST_API.md` |
| `src\DryRun.cs` | coverage measurement |
| `src\Snippets.cs` | snippet tests (C# run for real, then translated) |
| `src\Mono.cs` | closed-world monomorphisation: discovery of generic instantiations, the ambient substitution |
| `src\FloatAlloc.cs` | float mode `jit32`: which float locals the 32-bit JIT keeps on the x87 stack (liveness on Roslyn's control-flow graph) |
| `gen_il_sizes.py` -> `il_sizes.tsv` | IL size of every original method, from the reference database (for `jit32`) |
| `tests\framework\FmtProbe.cs`, `fmt_probe.out` | number formats as the real .NET Framework 4.8 x86 prints them (expected values of `rt::fmt`'s unit test); `build.cmd` rebuilds it |
| `core.json` | the core set: what to translate (whole classes, namespaces, every struct), host mappings, stubs with their reasons, `floatMode` |
| `game.json` | the whole game: both decompiles, every namespace; `ignoreNative` and `stubs` with their reasons |
| `native.json` | interop table: C# symbols provided by hand-written Rust (`rt`, `terraria-core`) |
| `native.d\*.json` | more interop tables, one per area and owner (`rt_*.json`: this tool's runtime, `rt_json.json` holds JSON / resources / reflection / culture / LINQ additions; `host_*.json`: installed copies of the `terraria-host` crate's tables). Schema, requirements on native Rust types and two worked examples: [NATIVE_SCHEMA.md](NATIVE_SCHEMA.md) |
| `gen_native_xna.py` -> `native_xna.json` | the interop table of the XNA value types, generated from `terraria-core\src\xna` |
| `tests\*.cs`, `tests\snippets.json` | the snippets (41) |
| `coverage_tail.md` | hand-written "what is tested" table (also the tail of `COVERAGE.md`) |
| `pilot.json`, `PILOT_REPORT.md` | the pilot's configuration and report (superseded by `core.json`; kept for the record, not used by the scripts) |

## Config

```jsonc
{
  "include": ["native.json"],
  "floatMode": "x87",                                          // "f32" | "x87" | "x87-locals" | "jit32" (DESIGN.md section 5)
  "floatInMemory": { "Type.Method:local": "evidence" },        // jit32: measured exceptions to the allocation rule (VERIFICATION.md, Proof 3)
  "reflect": ["Terraria.Item", "Terraria.ID.*"],               // reflection data with instance members (section 13)
  "inert": { "M:System.Attribute.#ctor": "reason" },           // library members that do nothing observable (section 15)
  "types": [ { "type": "Terraria.NPC", "reflect": true } ],    // whole class; reflect = emit reflect_fields for tests
  "namespaces": ["Terraria.ID", "Terraria.DataStructures"],    // every type of the namespace
  "autoStructs": true,                                         // every struct and interface of the sources
  "nativeMembers": { "M:Terraria.Netplay.GetSectionX(System.Int32)": "rt::div({0}, 200)" },
  "stubs": { "F:Terraria.Netplay.ListenPort": "7777" }         // "M:Type.Name(*)" = every overload
}
```

Keys are Roslyn documentation ids. What a translated body reaches that is none of these:

- a library member: a generated host function (`HOST_API.md`): `void` logs once and continues, a value panics,
  a constructor of a library class gives an opaque object;
- a game class outside the translated set: `new` gives an opaque object, a `void` call is a recorded gap, a value
  panics (`rt::unported`). `src\gen\UNPORTED.txt` lists every site.

## Adding a class

1. Add it to `types` (or its namespace to `namespaces`) in `core.json`, `sh run.sh`.
2. Read its rows in `SCALE_REPORT.md`: "failed" bodies name the construct the emitter refused; "unported
   inside" lists callees to translate, map or stub. `--reach <function>` shows what a call can run into.
3. `cargo check -p terraria-gen`. A rustc error in generated code is a bug in the tool or a wrong template in
   `native.json`: fix it there. Never edit files under `src\gen`, `src\host\api` or `terraria-gen-rt\src\snip`.
4. If the emitter learned something, add a snippet to `tests\` that exercises it, and a row to `coverage_tail.md`.

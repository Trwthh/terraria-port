# Terraria port in the browser: design and spike results

Written 2026-10-13. Scope: a browser build (WebAssembly + WebGPU / WebGL2) of the Rust port in `port\`. This file holds
the plan and the evidence for it. It is not a port. Nothing under `port\` or `tools\` was changed; build output went
only to `port\target-wasm\`; spikes are in `web\spikes\`, a patched copy of the runtime is in `web\gen-rt-patched\`,
and every command result quoted below is in `web\logs\`.

Line numbers are for the tree as of this date. `gc.rs` is being changed by another session, so its numbers may shift.

## 0. Summary

| Risky part | Verdict | Evidence |
|---|---|---|
| Fibers (world load / generation, network threads) | **GO with JSPI.** Per-fiber shadow stacks are needed and they work. Chrome and Edge only for now | `fiber-jspi` spike: three interleaved work items, results equal to a run without suspension; the negative control (one shared shadow stack) gives wrong results; the scheduler also works from inside wasm (section 1) |
| C# exceptions (`try` = `catch_unwind`) | **GO, but needs nightly.** Unwinding on wasm32 needs `-Zbuild-std` plus `+exception-handling`. Stable traps | `unwind-wasm` spike: stable traps on every `catch_unwind`; nightly with build-std and EH passes 3 of 3 (section 1.5) |
| Collector | **GO** after 2 one-line fixes in `gc.rs`. A suspended JSPI fiber's wasm locals cannot be scanned, so no mark phase may end while a fiber is suspended (`may_finish = false`), and the network threads need a precise design | 102 of 104 runtime tests pass on wasm32-wasip1 with the fixes (the 2 others need a temp dir). Linear-memory scan while suspended: frame local found 0 times (section 2) |
| x87 maths | **GO.** Pure Rust, results bit-identical to native, 3 to 5 times slower | The x87 reports are identical native and wasm on 158 normalised lines, including 1M-record dense tables (section 3) |
| Graphics | **GO** for WebGL2 and WebGPU. All 136 passes translate to GLSL ES 3.00 and fit the WebGL2 limits. Recommendation: do not put the frame inside Bevy on the web | `shader-webgl2`: 136 of 136 pass. `wgpu-wasm`: Bevy 0.19.1 plus wgpu 29.0.4 with the app's exact features checks for wasm32, both webgl2 and webgpu (section 4) |
| Content | **GO.** The user picks their own install, it is copied into OPFS, and synchronous reads happen in a Worker. No asset ships | 759 MB `Content` (section 5) |
| Memory | **GO**, measured for all sizes: medium 2.0 GB, large 2.8 GB of wasm memory (STATUS.md 2026-10-14) | Port heap about 168 to 258 MB with a small world (section 7) |
| Size | Estimated 70 to 110 MB raw wasm, roughly 20 to 30 MB brotli | wasm/x86-64 code ratio measured on `terraria-gen-rt`: 0.62 (section 7) |

**Recommended route.** Run the game (translated code, runtime, host) in one dedicated Web Worker:
- It runs on JSPI fibers, with one shadow stack per fiber.
- Its build is nightly `-Zbuild-std` with wasm exception handling.
- It renders directly with wgpu on an `OffscreenCanvas`: WebGPU first, WebGL2 as the fallback.
- Content and saves go through OPFS synchronous access handles.
- The main page is a thin layer: the content picker, input forwarding, an AudioWorklet and fullscreen.

Bevy stays the native shell. On the web it is optional (it compiles) and it gets in the way of JSPI (section 4.3).

## 1. Fibers

### 1.1 What exists natively
- Each "thread" of the original is a Windows fiber of the game thread (`terraria-host/src/sys/threading.rs` module docs, lines 1-92).
  - Fibers are created with `CreateFiberEx` and a **256 MB** stack reserve (`threading.rs:215`, "flood fills").
  - The switch is `SwitchToFiber`. It spills the callee-saved registers with inline asm (`spill_registers`, `threading.rs:271-287`) and records the stack range used by the conservative scan (`switch_to_main`, `threading.rs:297-311`).
- Everything fiber-related is `#[cfg(all(windows, target_arch = "x86_64"))]` (`threading.rs:195-216, 321, 340, 356, 415, 431, 490, 586, 1122`).
- Without fibers, the code already falls back to running work inline and to completion (`threading.rs:446-485`). In that mode a wait loop panics after `SLEEP_LIMIT` sleeps (`threading.rs:1178`).

### 1.2 Which work items actually need to suspend
Thread starts in the decompiled source (grep `new Thread(` / `QueueUserWorkItem` / `Task.Factory.StartNew`):

| Work item | Needs suspension? | Why (source) |
|---|---|---|
| `WorldGen.playWorldCallBack` (load a world) | **yes** | Waits for the main thread to draw the map: `while (Main.loadMapLock) { ... Thread.Sleep(0); }` (`decompile/Terraria/Terraria/WorldGen.cs:6800-6809`). `loadMapLock` is cleared only by `MapRenderer` during `Draw` (`MapRenderer.cs:200`). Then `Main.RunOnMainThread(..).Wait()` (`WorldGen.cs:6814`), which the host runs at the wait under the main context (DESIGN.md section 9, "Threads"), so no suspension is needed for that part |
| `CreateNewWorld` / generation (`WorldGen.cs:6596`) | for the loading screen only | No waits on the main thread, but it runs 20 s to 8 min (VERIFICATION.md: `ref_large` 479 s in the test build). Without yields the page freezes and the browser offers to kill it. The drunk-world branch sleeps 800 times x 10 ms (`WorldGen.cs:18872`) |
| `saveAndPlayCallBack`, `SaveAndQuitCallBack`, `RenameWorldCallBack`, `saveToonWhilePlayingCallBack`, `serverLoadWorldCallBack` (`WorldGen.cs:6693-6905`) | for the loading screen only | They finish on their own |
| `Netplay.ServerLoop`, `TcpClientLoop` / `InnerClientLoop`, `TcpSocket.ListenLoop`, `BroadcastThread` (`Netplay.cs:238, 626, 847`; `TcpSocket.cs:179`) | **yes, forever** | Endless loops that sleep every iteration (`threading.rs:77-95`, `net.rs:5-20`). `AcceptTcpClient` / `Connect` wait through `wait_a_frame` (`threading.rs:320`, `net.rs:471, 816`) |
| `Main.startDedInputCallBack` (`Main.cs:6027`), `SocialClientLoop`, Steam / WeGame threads | not on the web | Console input of the dedicated server; `SocialMode.None` |
| `Program.ForceLoadThread` (`Program.cs:56`) | no | Runs to completion |

### 1.3 The yield points (where a switch happens)
These are every call site of `switch_to_main`:
1. `rt::yield_point()` at the head of every translated loop body, when armed (`on_yield_point`, `threading.rs:585-611`).
   - It is armed by a time slice (`YieldMode::Millis`) or a loop count (`YieldMode::Loops`).
   - The `Millis` timer is a `std::thread` (`threading.rs:548`). On wasm32-unknown-unknown `spawn` returns `Err` and the code ignores it (`let _ =`), so the timer would never fire. Measured: `std-probe` `thread_spawn` returned 0.
   - Web replacement: compare `performance.now()` against the deadline, through an import, every N passes of the armed check.
2. `Thread.Sleep` on a fiber (`thread_sleep`, `threading.rs:1145-1176`). A thread fiber sleeping 1 ms or more suspends at once; pool work suspends after `SLEEPS_PER_SLICE` = 100 sleeps.
3. `wait_a_frame` (`threading.rs:320`): socket accept / connect.
4. `monitor_enter` on a lock held by a suspended fiber (`threading.rs:640-690`).
5. `run_to_completion` on a fiber that waits for another fiber (`threading.rs:488-520`).

The main fiber enters fibers in `resume` (`threading.rs:357-399`), called from `run_pending_work` / `Task.Wait` / `Thread.Join` / `monitor_enter`.

### 1.4 Options for wasm32, judged

| Option | Verdict | Evidence / reason |
|---|---|---|
| **JSPI** (`WebAssembly.Suspending` / `WebAssembly.promising`) | **chosen** | Spike passes (below). Costs nothing in code size. Only the switch points change. Ships by default in Chrome and Edge from 137. Firefox: behind a flag, default-on planned. Safari: unconfirmed ([caniuse](https://caniuse.com/wf-wasm-jspi), [OpenReplay overview](https://blog.openreplay.com/jspi-javascript-wasm-bridge/), searched 2026-10-13). Node 24.19 needs `--experimental-wasm-jspi` (`node --v8-options`) |
| Asyncify (`wasm-opt --asyncify`) | fallback only, **not measured** | binaryen is not installed here and it was not built. Asyncify instruments every function that can reach an unwind point. With a yield point in every loop body that is almost all 1.13M lines, so expect a large growth in code size and a slowdown, plus a very long `wasm-opt` run on a module of about 100 MB. Its one advantage: the unwound locals land in a linear-memory buffer, which the conservative scan could read |
| Wasm stack switching (`cont.new` / `resume` / `suspend`) | not available | V8 13.6 (Node 24) has only `--experimental-wasm-wasmfx` / `--experimental-wasm-stack-switching` flags (`node --v8-options`). There is no rustc support |
| Run work items to completion (the existing non-fiber path) | not enough | `playWorldCallBack`'s wait for `Draw` (`WorldGen.cs:6800`) can never finish on one stack. The existing fallback panics there by design (`threading.rs:1178`). Generation would freeze the tab for 20 s to minutes |
| Web Workers + SharedArrayBuffer (real threads) | no | The runtime is single-threaded by design: one heap per thread (DESIGN.md section 9), `rt::enter` checks the thread (`rt/mod.rs:85-94`), thread-local statics. Real threads would also need the `+atomics` build-std target and COOP/COEP headers. It would be a new runtime |
| Restructure the endless network loops as state machines | **yes, for multiplayer** | See section 2.3. The translator already turns iterator methods into heap state machines (DESIGN.md 8d). The same treatment for the four loop methods (config-listed, `Thread.Sleep` = `yield return`) removes the only fibers that never end |

### 1.5 Spikes that settle it

**`web\spikes\fiber-jspi`** (`src\lib.rs`, `run.mjs`, `sched.mjs`, `depth.mjs`; built with
`RUSTFLAGS="-C link-arg=--export=__stack_pointer" cargo build --release --target wasm32-unknown-unknown`, 22 KB).
- The setup:
  - A work item recurses 300 deep.
  - Each frame holds an address-taken array (shadow stack) and a value kept only in a wasm local.
  - Yield points are armed every 37 iterations and call a `Suspending` import.
  - A frame loop calls `frame()`, which uses the shadow stack itself, between resumptions.
  - Three work items are interleaved.
- `run.mjs`, scheduler in JS (`web\logs\fiber-jspi.txt`):
  - 3 of 3 results are EQUAL to a run without suspension.
  - 34 frames and 97 suspensions, `PASS (one shadow stack per fiber)`.
- **Negative control** `run.mjs noswap`, where every fiber shares the main shadow stack:
  - 3 of 3 results are DIFFERENT, `FAIL`.
  - So the per-fiber shadow stack (swapping `__stack_pointer` at every switch) is required, not optional.
- `sched.mjs`, scheduler inside wasm, the real code's shape (`web\logs\fiber-jspi-sched.txt`):
  - `main_frame` is a `promising` export and calls the `Suspending` import `host.resume(fiber)` for each work item, as `run_pending_work` -> `resume` does.
  - The main stack is suspended while a work item runs, and the work item's own suspension resumes it.
  - 3 of 3 EQUAL, and the main frame's own shadow-stack data is intact in 34 of 34 frames: `PASS`.
- `depth.mjs`, engine stack depth (`web\logs\fiber-jspi-depth.txt`):
  - The probe recurses with 8 live `u64` locals per frame.
  - Main stack: 9,727 frames before tier-up (Liftoff), 32,924 after.
  - JSPI stack: 33,134.
  - So a JSPI stack is about as deep as the main stack, about 1 MB, and baseline-compiled frames are about 3.4 times larger.
- A first version of `depth.mjs` kept the 8 values in an array: it overflowed the 1 MB **shadow** stack and trapped with "memory access out of bounds". The finding: shadow-stack overflow is not detected on wasm. On the main shadow stack it only traps because `--stack-first` places that stack at address 0. A fiber shadow stack allocated on the heap would silently overwrite heap memory below it, so fiber shadow stacks need a guard check.

**What the port must change (threading.rs, about 300 lines behind `cfg(target_family = "wasm")`):**
- `FiberState.fiber` becomes a fiber id plus a shadow-stack allocation:
  - 8 MB per work item, matching the native 256 MB reserve's intent. Shadow frames are small, since only address-taken locals live there.
  - Add a canary word at the bottom, checked at each switch.
- `resume` / `switch_to_main` / `fiber_main` call two `Suspending` imports.
- The frame entry (`HostedGame` update + draw) is a `promising` export driven by a JS loop.
- `spill_registers` is not needed. `suspended_stacks` reports the fibers' shadow-stack ranges only (section 2).
- Integration risk, not tested: `wasm-bindgen` generates the import object itself.
  - The two fiber imports and the frame export must be wrapped with `Suspending` / `promising` on the raw module.
  - That means a small post-processing step of its JS glue, or raw `#[link(wasm_import_module)]` imports added to the import object.
  - The spikes did this without wasm-bindgen.

**Native stack depth risk (not measurable without the game):**
- Native fibers reserve 256 MB because world generation recurses (`threading.rs:213-215`). V8 gives each stack about 1 MB (measured above).
- The original survives in a 1 MB .NET thread stack with x86 JIT frames. Optimised wasm frames should be comparable, but frames at opt-level 0 or in Liftoff are several times larger.
- Stage 2 must measure the deepest recursion of world generation under V8. `--experimental-wasm-growable-stacks` exists in V8 13.6, but only as a flag.
- Measured (STATUS.md 2026-10-14): the large world's generation on its JSPI stack traps at 128 and 256 KB
  (`--js-flags=--wasm-stack-switching-stack-size`) and completes at 512 KB; Chrome's default is about 1 MB.

### 1.6 Exceptions need unwinding (found during the spikes, blocking)
- C# `throw` is `resume_unwind` and `try` is `catch_unwind` (DESIGN.md section 7; `terraria-gen-rt/src/rt/exc.rs:1-5, 99-100`).
- The generated game uses `try_(` at **909** sites in 98 files (`grep -o "try_(" terraria-gen-game/src`). The game also relies on it as a bounds check (`try { Main.tile[x, y] } catch { }`).
- **Spike `web\spikes\unwind-wasm`** (`web\logs\unwind-wasm-*.txt`):

| Build | `try_catch` (throw 10 frames deep, typed payload, 10 destructors) | `bounds_check(1)` | `bounds_check(5)` (index panic caught) |
|---|---|---|---|
| stable 1.99, wasm32-unknown-unknown | TRAP: unreachable | TRAP | TRAP |
| stable + `-C target-feature=+exception-handling` | TRAP | TRAP | TRAP |
| nightly 1.101 (2026-10-12) `-Zbuild-std=std,panic_unwind` `-C panic=unwind -C target-feature=+exception-handling` | **1 OK** | **2 OK** | **99 OK** |

- Reason: the stable `std` for wasm32 is prebuilt with `panic=abort`.
- Consequence: the web build needs a pinned nightly toolchain and `build-std`, or a later stable release that ships an unwinding wasm `std`.
- Wasm exception handling itself runs in Node 24 / V8 13.6, as above.
- The runtime's own test harness confirms the same: on stable wasm32-wasip1 the first test that catches a panic aborts the whole test binary (`web\logs\gen-rt-wasi-tests.log`).

## 2. The collector

### 2.1 What the native code assumes
- The conservative scan of suspended stacks is the source registered with `rt::set_conservative_roots` (`gc.rs:1329`). It is installed by the shell (`terraria-app/src/shell/mod.rs:1029`) and the server (`server.rs:117`), and runs from `gc.rs:1308`.
- `mark_conservative_stack` (`gc.rs:721-747`) reads every word of the recorded machine-stack range, from the spilled registers up to the fiber's stack base.
- With no source installed, `gc_step(.., may_finish = false)` keeps a mark phase from ending while work is suspended (`gc.rs:1220-1240`; the shell decides at `shell/mod.rs:1292`).

### 2.2 On wasm
- The engine's value stack is not in linear memory.
  - Measured: while fiber 3 is suspended, a full scan of linear memory for the value its outermost frame holds in a wasm local finds it **0 times** (`fiber-jspi.txt`, first line).
  - Only address-taken locals are in memory, on the fiber's shadow stack.
  - So a scan of shadow stacks alone would miss handles and is **unsafe**.
- What replaces it:
  1. **Single player and stage 2: `may_finish = false` while a fiber is suspended.** This is the existing mode, chosen when no conservative source is installed (`shell/mod.rs:1292`). Mark and sweep slices still run; only the end of a mark phase waits. What it costs, measured natively (VERIFICATION.md "Collector: suspended work"):
     - Generation from the menu had no collection that ended during the generation.
     - Peak 11.0 million objects / 243 MB, against 9.3 million / 258 MB with the scan: not more bytes.
     - It also removes the 255 ms pause the scan caused once.
  2. **Multiplayer (stage 4): no endless fibers.** The four network loops become translator-made state machines (section 1.4). Their locals then live in a heap object and are traced through the existing `register_scan_block` / iterator path (`gc.rs:864-880`; `rt/iter.rs:81-88`). The remaining fibers are short (accept / connect waits). An alternative that needs no translator change: a mark phase may end only in frames where every suspended fiber is a network loop parked in `Thread.Sleep` at the top of its loop body. This is unproven: those frames still hold the loop's locals, so it needs a per-method argument.
  3. Asyncify's unwind buffer would be scannable, but Asyncify is not the chosen route (section 1.4).
- **Iterator methods and enumerators still use conservative scanning of heap blocks.** That works on wasm because those blocks are in linear memory. It needs two fixes.

### 2.3 32-bit assumptions in `gc.rs`, found and proven

| # | Where | Problem on wasm32 | Fix (proposed, NOT applied to `port\`) | Proof |
|---|---|---|---|---|
| 1 | `gc.rs:730` `if !(0x1_0000..0x8000_0000_0000).contains(&w)` | `cargo check` error `literal out of range for usize` (deny lint): **the only compile error of the four crates** | Choose the range by `#[cfg(target_pointer_width)]`; on wasm32 use `0x1_0000..usize::MAX` | With `-A overflowing_literals` the literal truncates to 0, the filter rejects every word, and the tests `conservative_words_keep_objects_by_any_inner_address` and `an_old_word_under_a_moved_list_buffer_keeps_the_list` FAIL (`web\logs\gen-rt-wasi-eh-each.log`). With the fix they pass (`gen-rt-patched-each.log`) |
| 2 | `gc.rs:698` `if w < 4096 \|\| w % 8 != 0` in `mark_conservative` (heap blocks of suspended iterator methods) | A `Str` inside the state machine points 4 bytes past an 8-byte boundary on wasm32. It is skipped and freed: the test `rt::iter::tests::suspended_machine_keeps_its_locals_alive` reads poisoned bytes and aborts | `w % size_of::<usize>()` | Fails before the fix, passes after (`gen-rt-wasi-eh-each.log` vs `gen-rt-patched-each.log`) |
| 3 | `gc.rs` test `poison_mode_makes_stale_handles_visible` (`0xDDDD_DDDD_DDDD_DDDDusize`) | Test code only: overflow lint | `usize::from_ne_bytes([0xDD; size_of::<usize>()])` | Compiles and passes |
| 4 | `gc.rs:1172, 1181` `GcBudget::Micros` uses `std::time::Instant::now()`; also `gc.rs:757, 1309, 1395-1407` | `Instant::now()` **traps** on wasm32-unknown-unknown (`std-probe`: `instant_now TRAP: unreachable`) | A clock function behind `cfg(target_family = "wasm")` (`performance.now()` import), or the `web-time` crate (1.1.0 is in the cargo cache) | `std-probe.txt` |

**The runtime on wasm, with the nightly EH build** (`web\gen-rt-patched` = a copy of `terraria-gen-rt` with fixes 1-3, see its `PATCH.md`).
Tests ran on wasm32-wasip1 under Node's WASI, **one fresh instance per test**:
- The runtime keeps one heap per thread. libtest runs every test on the main thread when there are no threads, so tests in one instance share heap state and fail together; run in a single instance, it aborted after 43 passes (`gen-rt-wasi-eh-tests.log`).
- Result: **102 of 104 pass**, and `bench_pauses` is ignored, as natively.
- The 2 failures need `std::env::temp_dir()`, which WASI does not have ("not supported by WASI yet", `rt/io.rs:842, 1056`, test code only).
- This covers the collector (incremental cycles, store barrier, verifier, poison mode, large arrays in pieces, conservative words), strings, collections, formatting, decimal, cultures and exceptions.

## 3. x87 maths

- `terraria-x87` is **pure Rust with no dependencies**: "Std only, no dependencies" (`terraria-x87/README.md:3-4`), and no `asm!` in `src/`.
  - It reproduces `fsin` / `fpatan` and friends with wide integer arithmetic, and `pow` / `log` with a transliterated register machine (`crt_cpu.rs`, `crt_sse2_gen.rs`).
- Translated code calls it for `Math.Sin/Cos/Tan/Atan/Atan2/Pow/Log/Log10/Acos` (`tools/cs2rs/native.json:76-79, 125-126, 241-242`). Counts in the generated game: 235 `atan2`, 206 `sin`, 175 `cos`, 38 `pow`.
- `cargo check --target wasm32-unknown-unknown`: OK (`web\logs\check-terraria-x87.log`).
- **Run on wasm32-wasip1 under Node 24's WASI** (`web\spikes\x87-wasi`).
  - The crate's own integration tests were included unchanged by `#[path]`. A local `env!` macro redirects their `CARGO_MANIFEST_DIR` to the project preopened as `/w`.
  - The unit tests were built from the port workspace (`cargo test -p terraria-x87 --lib --target wasm32-wasip1 --no-run`).
  - Results: unit tests 4 of 4 ok, `probes` 1 of 1, `tables` 9 of 9. The tables include the 1,000,000-record dense Sin / Cos tables and the 244,000-record raw register tables.
- **Native vs wasm reports are identical.**
  - Normalised (timings removed, sorted): 158 lines each, and `diff` is empty (`web\logs\x87-native.normalized.txt` vs `x87-wasi.normalized.txt`).
  - Every exact / mismatch count equals the README's table, for example `Sin_m10_10` 999,991 exact.
- **Speed** (`tables::speed_report`; native release vs wasm opt-level 3 in V8):

| Function | Native ns/call | Wasm ns/call |
|---|---|---|
| sin | 107 | 441 |
| tan | 3,044 | 16,399 |
| atan2 | 1,527 | 5,831 |
| pow | 184 | 183 |
| log | 117 | 71 |

  Trig is 3 to 5 times slower: wasm has no 64x64->128 multiply. At about 600 sin/cos/atan2 call sites, a frame that makes a few thousand calls spends roughly 1 to 3 ms. Watch this in stage 2 profiles.
- **Note for the host:** `f64::sin` from `std` is NOT the same function on both targets.
  - The checksum of the same 1M `std` sin calls differs: native `5.171478496083823e2`, wasm `5.171478496083819e2` (`x87-native.log`, `x87-wasi-tables*.log`).
  - Generated code does not use it.
  - The host and app do: `.sin()` / `.cos()` in `terraria-host/src/gfx/batcher.rs`, `input.rs`, `audio.rs`. Sprite rotation or audio may differ in the last bit between native and web. If a pixel gate ever disagrees, route these through `terraria-x87` or the `libm` crate on both targets.

## 4. Graphics

### 4.1 What the app does
- `terraria-app/src/gfx/render.rs` replays the host's draw list with raw wgpu inside Bevy's render world.
  - It gets `RenderDevice` / `RenderQueue` / `GpuImage` (`render.rs:597-614`).
  - Pipelines are cached by blend, topology and cull (`render.rs:213-224`).
  - Every target is `Rgba8Unorm` (`render.rs:57`); the back buffer is `Rgba8UnormSrgb` (`render.rs:62`).
  - No depth / stencil, MSAA 1, no storage buffers, no compute.
  - Blend: all XNA factors plus Min / Max and the blend constant (`render.rs:258-290, 840`).
  - Dynamic-offset uniform buffers (`render.rs:391`).
- `replay_frames` calls `std::time::Instant::now()` (`render.rs:598`), which traps on the web (section 6.5).
- Readback: `GetData` on render targets must be synchronous (`terraria-host/docs/readback.md` section 2: screenshots read a target they drew two statements earlier). WebGPU and WebGL in browsers only map buffers asynchronously.
  - With JSPI the readback hook can `await` `mapAsync` through a `Suspending` import, because `Draw` runs inside the `promising` frame export.
  - Without JSPI, the world-capture feature cannot work.
  - The wgpu backend does not serve readbacks natively either yet (`readback.md` header).

### 4.2 Bevy 0.19.1 + wgpu 29.0.4 on wasm32: compiles
- Spike `web\spikes\wgpu-wasm`:
  - It has the app's exact dependency set (`bevy = { 0.19.1, default-features = false, features = ["2d", "ui"] }`, `wgpu = 29.0.4`, `raw-window-handle`) and the port's `Cargo.lock`.
  - Its main is a reduced copy of `GfxPlugin`: a render-world system that builds a WGSL pipeline with raw wgpu and draws into a Bevy image target.
- `cargo check --target wasm32-unknown-unknown -j 1`:
  - **exit 0, 0 errors**, 27 min 00 s cold (`web\logs\wgpu-wasm-webgl2.log`).
  - Bevy's `2d` / `ui` features already include `default_platform` and with it `webgl2` (`bevy-0.19.1/Cargo.toml:2586-2592, 2762-2774`), so the app's feature set is already the WebGL2 one.
- `--features webgpu`: **exit 0**, 5 min 58 s (`wgpu-wasm-webgpu.log`).
- Not done: running it in a browser (no browser was launched, per the brief), `wasm-bindgen` output, and a real draw.

### 4.3 Why the web shell should not run the frame inside Bevy
- JSPI can suspend only a wasm stack that was entered through a `promising` export, with no JS frames between it and the `Suspending` import.
- Bevy's web loop runs `App::update` from winit's `requestAnimationFrame` closure, a wasm-bindgen closure the browser calls. The game frame runs inside it (the `ShellState` system, `shell/mod.rs:1-40`), so `resume` from `run_pending_work` could not suspend it.
- The host was built for this split: it has "no Bevy; output is a draw list and an audio command queue" (`terraria-host/Cargo.toml` description).
- So the web shell is a new crate, `terraria-web`:
  - A JS loop `await`s `tick()` (promising): input in, `Update` / `Draw` out.
  - It then calls `render()`, which replays the frame with wgpu on an `OffscreenCanvas` in the same Worker.
  - The replay core of `render.rs` moves into a Bevy-free module that both shells use.

### 4.4 Shaders under WebGL2
- **Spike `web\spikes\shader-webgl2`**: naga 29.0.4, the version wgpu 29.0.4 uses (`port/Cargo.lock`).
  - All 136 committed WGSL passes (`terraria-host/docs/effects/wgsl/*/*.wgsl`) were parsed and validated with `Capabilities::empty()`.
  - Each entry point was then written as GLSL ES 3.00 with `is_webgl: true`, which is wgpu-hal's GLES path on the web.
- Result: **136 of 136, 0 failures** (`web\logs\shader-webgl2.txt`). Against the wgpu 29 `downlevel_webgl2_defaults` (`wgpu-types-29.0.4/src/limits.rs:574-590`):
  - At most 4 textures in a pass, against a limit of 16 textures and 16 samplers per stage.
  - Largest uniform block 336 bytes, against 16,384.
  - Highest inter-stage location 8, against 15 variables.
- Why it is this easy: the passes are ps_2_0 / vs_2_0, straight-line code with 231 `textureSample` and 312 `select`, no loops, no derivatives in branches, one sampler per texture.
- Remaining WebGL2 limits to plan for:
  - `max_texture_dimension_2d` is 2048 in the downlevel defaults. Bevy and wgpu raise it with `using_resolution(adapter.limits())`. Large render targets (screen size, the map) need the adapter's real limit.
  - No `mapAsync` without a frame of latency, which affects readback (4.1).

## 5. Content (the player's own files) and saves

- **Nothing of the game ships.** The native host reads the install from `TERRARIA_DIR`, by default `C:\Users\user\Documents\Games\Terraria` (`terraria-host/src/content.rs:3, 32-37`), with `std::fs`.
- On wasm32-unknown-unknown `std::fs` simply fails: `std-probe` `fs_read` returned 0 (Err). Requirements:
  - `Content.Load` is synchronous in translated code, so reads must be too.
  - The install's `Content` is **759 MB** (`du -sh`, read-only).
  - XACT music is read from `Wave Bank.xwb` with `File::open` + seek (`terraria-host/src/audio.rs:783, 800`).
- **Design:**
  1. **Pick.** The first-run page asks for the user's Terraria folder. Three ways in:
     - `showDirectoryPicker()` (File System Access API: Chrome / Edge),
     - `<input type="file" webkitdirectory>` (all browsers),
     - drag and drop of the folder (`DataTransferItem.getAsFileSystemHandle` / `webkitGetAsEntry`).

     The page checks for `Content/Images/Item_1.xnb`, `Content/Fonts`, `Wave Bank.xwb` and `Terraria.exe`'s version resource, so the wrong folder is refused with a message. Executables and DLLs are never read or uploaded: only `Content\**`. `OnlineFix.dll` / `steam_api.dll` are skipped by name and never opened.
  2. **Copy into OPFS** (`navigator.storage.getDirectory()`): stream the files into `/content/...` with a progress bar, then call `navigator.storage.persist()` and check `estimate()` for the space (about 760 MB).
     - IndexedDB is the fallback where OPFS sync handles are missing. It needs an async preload of each file before the game asks for it, which only works for a known file list, so it is the second choice.
  3. **Read synchronously in the game Worker** through `FileSystemSyncAccessHandle.read(buffer, { at })`. This exists only in dedicated workers, which is one more reason the game lives in a Worker.
     - The host's file layer gets a `vfs` backend behind `cfg(target_family = "wasm")`. It maps `install_dir()` to `/content` and the save root to `/saves`, offers `read_all`, `open` + `read_at` for the wave bank, `exists`, `list_dir`, `metadata`, and is read-only for `/content` (the native write guard, `rt/io.rs` `write_guard_tests`, keeps the same rule).
  4. **Saves**: `Main.SavePath` -> OPFS `/saves`, already a separate root natively (`terraria-host/src/os.rs:8`). The settings page offers export / import of `.plr` / `.wld` as a download or upload, so the user owns their saves.
- Never: a server-side copy, a CDN of assets, or the folder being sent anywhere. Everything stays in the user's browser origin.

## 6. Audio, input, window, multiplayer, host threads

### 6.1 Audio
- Native: a mixer thread writes to `waveOut` (`terraria-app/src/host/audio.rs:1-3, 126, 314-409`). It is fed by the host's `AudioCommand` queue.
- Decoding is pure Rust: `lewton`, `symphonia` mp3, MS ADPCM in `terraria-host/src/audio/codec.rs`. The host checked clean for wasm32.
- Web:
  - The mixer runs in the game Worker once per frame and produces blocks.
  - An `AudioWorklet` on the main page plays them. The Worker posts transferable `Float32Array` blocks, about 2 frames of latency; a SharedArrayBuffer ring would need COOP/COEP headers.
  - The AudioContext starts on the first user gesture (browser autoplay rule). The pre-gesture title music is simply silent.
- Effort: move `mixer_thread`'s loop body into a function that both the thread and the web shell call.

### 6.2 Input
- The host takes an `InputSnapshot` per tick. The app fills it from Bevy events or the Win32 message hook (`shell/native.rs`).
- Web: the main page forwards keyboard (`KeyboardEvent.code` -> XNA `Keys`), text (`beforeinput` / composition events for IME), mouse, wheel and pointer lock to the Worker by `postMessage`.
- Gamepads: the Gamepad API is polled on the main page per frame. The host already applies XNA's dead zones to raw XInput-style values (plan.md, input stage, item 3), so standard-mapping gamepads map directly.
- `available_parallelism` returns `Err` (`std-probe`: 0), so `Environment.ProcessorCount` (`sys/misc.rs:95`) needs a constant, for example `navigator.hardwareConcurrency`.

### 6.3 Window / fullscreen
- `ApplyChanges` / borderless / fullscreen requests become the canvas size and `element.requestFullscreen()` (it needs a user gesture; Alt+Enter counts).
- `IsActive` follows `visibilitychange` / `blur` / `focus`, which matches XNA's `WM_ACTIVATEAPP` rule (plan.md, input stage, item 5).
- Clipboard: `navigator.clipboard`. Reading is async and needs permission; with JSPI it can be awaited. `os.rs:136-160` is Win32-only.

### 6.4 Multiplayer
- `std::net` exists for wasm32 but always fails (`std-probe`: `tcp_connect` 0). The original server speaks raw TCP on port 7777, and a browser cannot open TCP.
  - **Port client to a server:** a small native bridge is needed that relays WebSocket to TCP, one connection per socket. The `net.rs` types (`TcpClient`, `NetworkStream`) get a WebSocket backend; `BeginRead` / `pump` / `deliver` keep their shape (`net.rs:5-20`).
  - The bridge is a separate executable the user runs next to a server, or a hosted relay. Hosted is out of scope: it would carry other people's traffic.
- **Host & Play in the browser:**
  - Natively it starts the port's own server as a process (`sys/misc.rs:314`, `Process.Start` of `TerrariaServer.exe` -> `-server`, DESIGN.md "Changes from the completion stage").
  - On the web: a second instance of the same wasm module in another Worker, running `Main.DedServ`, with the "socket" a `MessageChannel`. Other browsers join through WebRTC data channels; signalling needs a server, and that is out of scope for stage 4a.
- `UdpClient` (LAN broadcast) does nothing on the web.
- Gates: the existing multiplayer gate (port to port, port client on the ORIGINAL dedicated server) is replayed through the bridge.

### 6.5 Host threads and other std calls that trap
Each row was measured by `web\spikes\std-probe` on wasm32-unknown-unknown (`web\logs\std-probe.txt`):

| API | Result on wasm32-unknown-unknown | Port call sites | Replacement |
|---|---|---|---|
| `Instant::now()` | **trap** | `gc.rs:757, 1172, 1181, 1309, 1395-1407`; `threading.rs:537`; `sys/time.rs:510` (Stopwatch); `audio.rs:948`; `render.rs:598`; `shell/mod.rs:795, 929, 969` | A `clock` module: `performance.now()` (or `web-time`) |
| `SystemTime::now()` | **trap** | `sys/time.rs:156` (`DateTime.UtcNow`), `os.rs:102` | `Date.now()`; local offset from `Date.getTimezoneOffset()` instead of `GetLocalTime` (`sys/time.rs:81-113`, which returns 0 off Windows today) |
| `thread::sleep` | **trap** | `os.rs:159`, `sys/time.rs:689-696` (tests), `threading.rs:1134` (blocking main, server only) | Never sleep on the web: give the frame back |
| `thread::spawn` | returns `Err` | `rt/arr.rs:74` (large-buffer free thread: on `Err` the channel is dropped, the send fails and the caller frees inline, `arr.rs:66-96`, so the fallback is already correct); `threading.rs:548` (yield timer: silently never fires, see 1.3); `net.rs:464` (connect helper); `app/host/audio.rs:126` (`.expect` would panic) | Inline / per-frame |
| `std::fs::*` | `Err` | `content.rs`, `sys/io.rs`, `rt/io.rs`, `audio.rs` | OPFS VFS (section 5) |
| `std::env::var` | `None` | `content.rs:34`, `sys/net.rs`, `rt/exc.rs` | Fine: defaults apply |
| `std::net` | `Err` | `sys/net.rs` | WebSocket / MessageChannel (6.4) |
| random bytes | **panic**: `/dev/urandom` `expect` | `sys/crypto.rs:283-286` | `crypto.getRandomValues` |
| `std::process` | `Err` | `sys/misc.rs:252-357` | Worker server (6.4) |
| `thread::current().id()`, `Mutex`, `mpsc` | work | `rt/mod.rs:91`; `arr.rs` | none needed |

## 7. Size, load time, memory

- **Size, measured ratio:**
  - `terraria-gen-rt`'s test binary, release: wasm32 code section 1,373,537 bytes (`web\spikes\wasm-sections.mjs`); x86_64-pc-windows-gnu `.text` 2,225,440 bytes (`objdump -h`). Ratio **0.62** (`web\logs\size-ratio.txt`).
  - Native `client-opt` (generated crate at opt-level 2): `.text` 114 MB (0x6CF0560), `.rdata` 29 MB, exe 273 MB (`objdump -h target-app/client-opt/terraria.exe`).
  - Estimate for the web module without Bevy: **about 70 MB of code plus 20 to 30 MB of data, so 80 to 110 MB raw, about 20 to 30 MB brotli**. The compression figure is typical for wasm and was not measured.
  - A real number needs one build of `terraria-gen-game` for wasm32, which the brief did not allow.
- **Load:**
  - Use `WebAssembly.compileStreaming` with a cached `Response`. V8 compiles with Liftoff first and optimises hot functions with TurboFan in the background.
  - Expect a few seconds to the title on a desktop with a warm cache, plus the 20 to 30 MB download the first time. Not measured.
  - Code caching is automatic for streamed modules above V8's size threshold.
- **Opt-level:** the generated crate must NOT be built at opt-level 0 for wasm.
  - `NPC.ai` is one 24,657-line Rust function (`gen/terraria/npc.rs`). `Projectile.ai` has 16,773 lines, `WorldGen.add_passes` 16,619, `Item.set_defaults5` 15,448.
  - V8 refuses a function with more than 50,000 locals or a body over 7,654,321 bytes (V8's `wasm-limits.h` values; not re-checked here).
  - At -O0 the wasm backend does not colour registers, so the number of locals grows with the function's size.
  - Use `client-opt` settings (opt-level 2, 256 codegen units). The first wasm build of the game must be checked with a validator or a `WebAssembly.compile` before anything else.
- **Memory:**
  - wasm32 linear memory can reach 4 GB (65,536 pages) in current Chrome and Firefox; mobile browsers allow less.
  - Measured port heaps: 5.81 million objects / 168 MB live with `ref_small` loaded (plan.md:267), 6.0 million / 180 MB after a cycle (plan.md:303), peak 243-258 MB during a small world generation from the menu (VERIFICATION.md:550-555).
  - On wasm32 the pointer fields halve. The 16-byte object header stays (allocations are aligned to the header, `gc.rs:1754`).
  - **No "3 GB peak" figure was found** in plan.md, VERIFICATION.md or the READMEs.
  - The original's own peak working set for a large world is 735 MB, and 503 MB for a medium one (`refdata/worldgen/README.md:418-419`).
  - Scaling the port's small-world heap by tile count (large = 4 times small) gives roughly 0.7 to 1.0 GB for a large world. That fits 4 GB, with the copy of `Content` living in OPFS, not in memory. Stage 2 must measure the peak of `ref_large` on wasm.
  - **Measured on wasm (2026-10-14, `test/actions/world_{medium,large}.txt`):** wasm memory 1.25 GB on the title,
    2.0 GB with a medium world, 2.8 GB with a large one (live peaks 1.14 / 1.70 / 2.49 GB). The estimate above missed
    the module's own data: every tile is a collector object (20 million in a large world, 32 bytes each in the
    small-block pool), the collector's object list and mark stack (256 + 128 MB blocks), the game's sound effects as
    PCM (~177 MB), the textures' decode buffers. Two fixes were needed for that (terraria-web allocator): growing the
    memory 64 MB at a time (V8 charges ~1.3 ms per `memory.grow`) and a pool for blocks of up to 64 bytes (dlmalloc
    made a 30-byte tile object 48 bytes). Before them: 2.24 / 3.30 GB.

## 8. The compile results of the four crates (wasm32-unknown-unknown)

Commands were run from `port\` with `CARGO_TARGET_DIR=target-wasm`, `-j 2`, toolchain `stable-x86_64-pc-windows-gnu` (rustc 1.99.0), after `rustup target add wasm32-unknown-unknown wasm32-wasip1`.

| Crate | `cargo check -p <crate> --target wasm32-unknown-unknown -j 2` | Error classes |
|---|---|---|
| `terraria-x87` | **ok**, 1.0 s (`web\logs\check-terraria-x87.log`) | none |
| `terraria-shaders` | **ok**, 0.6 s (no dependencies outside dev) | none |
| `terraria-gen-rt` | **fail**, 1 error (`check-terraria-gen-rt.log`) | `literal out of range for usize` at `gc.rs:730` (section 2.3 #1). Warning: `fmt.rs:723` unused assignment (also native) |
| `terraria-host` | **fail**, only because of `terraria-gen-rt` (`check-terraria-host.log`) | same |
| `terraria-gen-rt` with `RUSTFLAGS="-A overflowing_literals"` | **ok** (`check2-terraria-gen-rt.log`) | none |
| `terraria-host` with the same | **ok**, 0 errors (`check2-terraria-host.log`) | Warnings only: `threading.rs:145, 174` fields never read, `threading.rs:565, 578` `arm_yield` / `disarm_yield` unused (they are the fiber path, which is cfg'd out) |

Everything else in the host compiles as it is, including its dependencies (`aes`, `png`, `lewton`, `symphonia`, `miniz_oxide`). It is `#[cfg(windows)]`-clean: the Win32 imports are cfg-gated, with fallbacks. The problems are at RUN time (section 6.5), plus unwinding (1.6).

Other builds:

| Build | Result | Log |
|---|---|---|
| `cargo test -p terraria-x87 --lib --target wasm32-wasip1 --no-run`, then under Node WASI | 4 of 4 | `x87-wasi-terraria_x87-*.log` |
| `cargo test -p terraria-x87 --release` (native, the same tests, for the diff) | 14 of 14 | `x87-native.log` |
| `cargo test -p terraria-gen-rt --lib --release --target wasm32-wasip1` (stable, `-A overflowing_literals`) | aborts at the first caught panic | `gen-rt-wasi-tests.log` |
| the same on nightly `-Zbuild-std=std,panic_unwind,test` + EH | 99 of 105 in fresh instances; failures: 2 literal (#1), 1 alignment (#2), 2 temp dir, 1 ignored | `gen-rt-wasi-eh-each.log` |
| the patched copy (`web\gen-rt-patched`), the same build | **102 of 105**; 2 temp dir, 1 ignored | `gen-rt-patched-each.log` |

## 9. Staged plan

Every stage keeps the native gates green (`sh tools/cs2rs/gates.sh`). Web-only code goes behind `cfg(target_family = "wasm")` or into new crates, so the native build does not change.

### Stage 0: everything but the game compiles and runs its tests on wasm (about 1 to 1.5 weeks)

| Crate | Change | Effort |
|---|---|---|
| toolchain | A pinned nightly (`web/rust-toolchain.toml`), `-Zbuild-std=std,panic_unwind`, `-C panic=unwind -C target-feature=+exception-handling`. The native build keeps stable | 0.5 d |
| `terraria-gen-rt` | `gc.rs` fixes #1-#3; a `clock` module replacing `Instant` (#4); a CI step that runs every test in its own wasip1 instance (the runner from `web\logs\gen-rt-tests.list`) | 1 d |
| `terraria-host` | `clock`, `SystemTime`, random bytes, `ProcessorCount`, and `temp_dir` -> VFS behind `cfg(target_family = "wasm")`; the host tests on wasip1 (those that read the install need the VFS or a preopen of `Content`) | 2 to 3 d |
| `terraria-x87`, `terraria-shaders` | none (both proven) | 0 |

Risks: few. Nightly drift is handled by pinning.
Verification: the rt / host / x87 / shaders test suites on wasm32-wasip1 under Node equal native, test by test; x87's report diff is empty, as already shown.

### Stage 1: title screen in a browser (about 3 to 5 weeks)

| Crate | Change | Effort |
|---|---|---|
| `terraria-host` | VFS backend for `content.rs` / `sys/io.rs` / `rt/io.rs` (OPFS sync handles via a small JS shim imported by raw wasm imports); `threading.rs` wasm fibers (section 1.5); yield clock through `performance.now()` | 1.5 to 2 wk |
| `terraria-app` | Split `gfx/render.rs` into a Bevy-free `replay` module (wgpu only, readback via an awaitable hook) plus the existing Bevy plugin; move the mixer loop body out of the thread | 3 to 5 d |
| new `terraria-web` (bin, wasm only) | The Worker entry: `init` (async: wgpu `requestAdapter` / `requestDevice` on an `OffscreenCanvas`, WebGPU then WebGL2), `tick` (promising: input -> `game::begin_frame` / update / draw, as `shell/mod.rs`'s frame sequence), `render`, `audio_blocks` | 1 to 1.5 wk |
| `web/www` (JS, no framework) | The first-run content picker + OPFS import (section 5), the Worker, input forwarding, AudioWorklet, fullscreen, JSPI feature check with a clear message for unsupported browsers | 1 wk |
| `tools/cs2rs` | Nothing for stage 1 | 0 |
| `terraria-gen-game` | First wasm build at `client-opt` settings; check function locals / size limits (section 7); measure `.wasm` size | 2 d of build time |

Risks:
- Build memory for the 1.13M-line crate under the wasm backend. Natively the largest rustc was 5.4 GB at opt-level 2 (app README); plan for a machine or time slot without the gates running.
- The V8 function limits.
- The wasm-bindgen + JSPI glue.
- The x87 trig speed.

Verification:
- **Proof 1 values**: the `p1` dump harness (`client-whole` gate) built as a wasip1 binary. It needs no graphics: run under Node with `Content` preopened, compare the 3,911,380 values with the same `refdata` files.
- Title screenshot: the web build's back buffer read back through the readback hook, against the native `g1_title` script's capture. Same input script (the script format is the shell's), pixel diff with the tolerance the native gates use.
- A headless Chrome check is allowed only if it needs no new software beyond what is installed; otherwise the user opens the page.

### Stage 2: single-player world (about 3 to 4 weeks)

| Crate | Change | Effort |
|---|---|---|
| `terraria-host` | Fiber shadow-stack guard; the collector policy `may_finish = false` while fibers are suspended (`shell/mod.rs:1292` logic in the web shell); large-buffer frees inline | 1 wk |
| `terraria-web` | Loading-screen animation through yields; character / world creation UI paths; the frame pacing of `Main.EndDraw` on `requestAnimationFrame` (no busy-wait: the game thread must not spin as natively, plan.md "Pacing under load") | 1 wk |
| `terraria-gen-game` / `tools/cs2rs` | Only if the stack-depth measurement fails: a translator rule that turns the deep recursions found into explicit stacks, or growable JSPI stacks once available | 0 to 2 wk |

Risks: the native stack depth of world generation under V8 (section 1.5), the heap peak for large worlds, and trig speed in generation.
Verification:
- **The worldgen rows**: `tests/worldgen.rs` as a wasip1 binary (no JSPI needed: there is no main-thread wait in pure generation), 108 of 108 rows for the six reference worlds (`refdata/worldgen/matrix`).
- `ticks-player` / `ticks-world`: the 600-tick and 3,600-tick server comparisons, float bits included, on wasip1.
- Screenshots of the scripted play scripts (`g7_play`, `g12_stability`) against native captures.
- Heap peak and collector pauses from `gc_profile` lines.

### Stage 3: audio and saves (about 1.5 to 2 weeks)
- `terraria-app`'s mixer into the AudioWorklet.
- XACT wave-bank streaming from OPFS (`read_at`).
- Saves in OPFS `/saves` with export / import.
- `navigator.storage.persist()`.

Verification:
- Save on the web, load natively, and the other way round. The world files' data sections must be byte-identical, as the native gate already checks.
- Mixer statistics (`AudioStats`: blocks, peak, RMS) equal to native for the same script with `TERRARIA_MUTE`-style mixing.
- Listening by a person.

### Stage 4: multiplayer (about 3 to 5 weeks)

| Part | Change | Effort |
|---|---|---|
| `tools/cs2rs` + config | The four endless loop methods as state machines (section 1.4 / 2.2), so no fiber lives forever and the collector finishes cycles | 1 to 1.5 wk |
| `terraria-host` `sys/net.rs` | A WebSocket backend for `TcpClient` / `NetworkStream`, and a MessageChannel backend for the in-browser server | 1 wk |
| new `tools/ws-bridge` (native, small) | WebSocket <-> TCP relay to a real or port server | 2 to 3 d |
| `terraria-web` | Host & Play as a second Worker running `-server` | 1 wk |

Risks: GC with long-lived network state and browser-to-browser play (WebRTC signalling, out of scope).
Verification: the `multiplayer` slow gate scenario (join, walk, dig, chat, leave), with the web client against the original dedicated server through the bridge and against the native port's server.

### Effort for stage 1, by crate (summary)

| Crate | Stage 0 + 1 |
|---|---|
| `terraria-gen-rt` | 1.5 to 2 d |
| `terraria-host` | 2 to 2.5 wk |
| `terraria-x87` | 0 |
| `terraria-shaders` | 0 |
| `terraria-app` | 3 to 5 d |
| `terraria-web` (new) + `web/www` | 2 to 2.5 wk |
| `terraria-gen-game` build | 2 d |
| **Total** | **about 5 to 7 weeks of one person** to a title screen, gated on JSPI (Chromium) and nightly unwinding |

## 10. Open questions and things that were not checked

- No browser was run. JSPI was checked in Node 24 / V8 13.6 behind a flag. Chrome 137+ ships it by default according to the sources above, which also say Firefox and Safari differ.
- Asyncify was not measured (binaryen not installed).
- Neither the game crate nor the app was built for wasm (forbidden by the brief). Its wasm size, rustc memory and the V8 function limits are open.
- The deepest native recursion of world generation under V8 is unknown.
- Not done: wasm-bindgen + JSPI integration, OPFS sync-handle throughput for 759 MB, and AudioWorklet latency.
- The "3 GB peak for a large world generation" in the brief was not found in the documents. The documents give 243-258 MB for a small world in the port, and 735 MB for the original's large world.
- `terraria-x87` contains a transliteration of Microsoft / Intel C runtime routines (handoff). Publishing a web build distributes it, which the user should decide on before any public deployment.
- The nightly toolchain installed for the spike (`nightly-x86_64-pc-windows-gnu`, minimal + `rust-src` + the two wasm targets, about 1 GB under `%USERPROFILE%\.rustup`) stays installed. Remove it with `rustup toolchain uninstall nightly-x86_64-pc-windows-gnu` if it is not wanted.

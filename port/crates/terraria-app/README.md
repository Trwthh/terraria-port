# terraria-app

The Bevy application around the Terraria port. Two things live here:

1. the **shell** (`src/shell/`): a reusable frame driver that runs a `HostedGame` on top of `terraria-host` the way
   XNA's `Game.Run` runs a `Game` subclass. The machine-translated `Main` will plug in here.
2. the **prototype viewer** (`src/main.rs` and the `main_*.rs`, `game_content/`, `graphics/` modules): the hand-ported
   drawing of a loaded world, driven by `terraria_sim`. It does not use the shell and stays as the regression
   reference for the renderer.

## Binaries

| binary | what it is | needs |
|---|---|---|
| `terraria-app` (default) | prototype viewer: opens `TERRARIA_WORLD`, draws it with the hand-ported code, arrow keys pan | game install, a `.wld` |
| `host_demo` | host object model end to end under the shell (render target, sort modes, scissor, blend states, `SetData`, user primitives, text, a sound, input echo) | game install |
| `effect_demo` | the game's compiled effects (armor / hair dyes, trails, screen filters) through the host and the wgpu replay | game install |
| `shell_demo` | interactive stand-in game, host API only: WASD sprite at 60 Hz, mouse / keys / typed text, click plays a sound, F11 borderless, F12 game-side screenshot, Esc exits; scriptable | game install |
| `readback_demo` | `Texture2D.GetData` on render targets mid-frame, exact pixel checks, exit code 1 on a mismatch | game install, a GPU adapter |

Build and test (Git Bash, from `port\`; keep `-j 3`). With debug info a debug exe is 1.8 GB and `cargo test` makes
eleven of them (24 GB in `target-app\debug`); without it (`CARGO_PROFILE_DEV_DEBUG=0`, what `gates.sh` uses) the folder
stays at about 5 GB. `target-app\debug` is a cache: delete it when the disk is short, the next build makes it again.

```
CARGO_PROFILE_DEV_DEBUG=0 CARGO_TARGET_DIR=target-app cargo build -p terraria-app -j 3
CARGO_PROFILE_DEV_DEBUG=0 CARGO_TARGET_DIR=target-app cargo test  -p terraria-app -j 3
```

`cargo test` runs the unit tests, the script parser test, and `tests/shell_runs.rs`: two hidden runs of `readback_demo`
(with and without the reads; the screenshots must be byte-identical) and the scripted `shell_demo` (checks the log lines,
the game-side PNG and the window screenshot).

The unit tests of the shell (compiled into every binary that includes it) need no window, no device and no focus:

| test | what it proves |
|---|---|
| `host_input::tests::keyboard_events_become_host_text_and_keys` | synthetic Bevy `KeyboardInput` / `Ime` events (as winit writes them for Shift+a, Shift+1, AltGr+q on a German layout, a dead key and a vowel, a Cyrillic letter, QWERTZ / AZERTY letters, the numpad with NumLock on and off, both Enter keys, an IME commit, focus lost) through `host_input::collect` in a headless `App` -> the host's `InputSnapshot` text and `Keyboard.GetState` keys |
| `native::tests::text_is_every_wm_char_once_and_keys_are_virtual_keys`, `characters_without_a_key_press` | the same cases as window messages through the hook's state (`MessageState::on_message`), plus posted characters, an input method's result string, surrogate pairs |
| `native::tests::game_is_active_follows_wm_activateapp_only` | XNA's `Game.IsActive` rule and the game's own `FocusHelper` rule over message sequences: start-up, Alt+Tab away / back, a click on another window, a window of the same program, minimise from the task bar, Win+D, a UAC prompt (with and without `WM_KILLFOCUS`), the lock screen, close. The sequences are written from the documentation of Windows, not recorded |
| `native::tests::pointer_rules` | where `Mouse.GetState` takes the position from and what `Mouse.SetPosition` does, over made-up window facts (no pointer is read or moved) |
| `pad::tests::bevy_gamepad_events_reach_the_host_state` | synthetic `RawGamepadEvent`s (connection, every button, sticks, triggers) in a headless `App` -> the host's `GamePadState` with XNA's dead zones, packet numbers, two pads, vibration requests |
| `window::tests::*` | the game's window commands against a fake monitor (borderless, moves, maximise / restore, fullscreen and back, the desktop never touched), and that hidden / passive runs only record |
| `script::tests::parses_and_steps` | the script language, `pad` lines included |

## Environment variables

Prototype viewer only: `TERRARIA_WORLD=<file.wld>` (required), `TERRARIA_VIEW_TILE=<x>,<y>`, `TERRARIA_UNLIT=1`,
`TERRARIA_TIME`, `TERRARIA_DAYTIME=0|1`, `TERRARIA_MOON_PHASE=0..7`, `TERRARIA_UNPORTED_REPORT=1`.

Shell binaries (`ShellConfig::from_env`):

| variable | effect |
|---|---|
| `TERRARIA_HIDDEN=1` | window not shown; counted as focused; decoration, maximise, minimise, fullscreen and focus requests are recorded and reported back to the game but not applied to the real window (resize and move are applied) |
| `TERRARIA_FIXED_DT=1` | every tick advances exactly `TargetElapsedTime`: one `Update` per Bevy frame, repeatable, faster than real time |
| `TERRARIA_INPUT_SCRIPT=<file>` | scripted input (below); implies fixed step and a fixed 1920x1080 display (`TERRARIA_DISPLAY`); nothing is read from the real keyboard or mouse |
| `TERRARIA_SCREENSHOT=<png>` | window screenshot; `TERRARIA_SCREENSHOT_FRAME=<n>` is the Bevy frame (default 120), the app exits 30 frames later. With a script, a `shot` line takes it |
| `TERRARIA_MUTE=1`, `TERRARIA_VOLUME=<0..1>` | mixer without a device / extra gain (`host/audio.rs`; use at most 0.03 on the real device) |
| `TERRARIA_VOLUME_FILE=<file>` | the output gain is read from this file five times a second (no file: 0): the device is opened silent and a test lets sound out for a bounded time |
| `TERRARIA_PRESENT_MODE=vsync \| novsync` | the present mode whatever the game asks for (measuring aid) |
| `TERRARIA_DIR`, `TERRARIA_SAVE_DIR` | game install (read-only), save root (default `work\port-save`) |
| `TERRARIA_GC_MB`, `TERRARIA_GC_TICKS`, `TERRARIA_GC_SLICE_US` | the collector: a cycle starts after that many MB of allocation (48) or ticks (3600); while it runs it gets that many microseconds per tick (1500) |
| `TERRARIA_GC_FULL=1`, `TERRARIA_GC_VERIFY=1` | one whole collection in one tick, as before the incremental collector (comparison) / every mark phase checked by a second walk of the heap (slow; test sessions) |
| `TERRARIA_LOAD_SLICE=<n>ms \| <n>loops \| off` | how world load / save / generation give the frame back so the loading screen is drawn: 8 ms per tick in a normal run, 1,000,000 loop iterations per tick in a fixed-step run (repeatable), `off`: never |
| `TERRARIA_AUDIO_PCM16=1` | open the audio device as 16-bit PCM instead of 32-bit float (the mixer then limits the sum itself) |
| `TERRARIA_HEAP_HISTOGRAM=1` | with `TERRARIA_FRAME_TIMES`: the 40 most frequent object types at exit |
| `TERRARIA_GC_FIBER_SCAN=0` | the collector does not scan the stacks of suspended work; a mark phase then cannot end while a world load / generation / network thread is suspended (the behaviour before the networking stage; comparison only) |
| `TERRARIA_NET_LOOPBACK_ONLY=1` | every listener the game opens on "any address" is bound to 127.0.0.1, the LAN announcement of a server is not sent, the server of Host & Play gets `-ip 127.0.0.1`. Set by every automated run; NOT set by `run-terraria.cmd` (a player's Host & Play listens on every interface, as the original) |
| `TERRARIA_HOSTPLAY_PORT=<n>` | the server Host & Play starts listens on port n instead of the game's fixed 7777 (test runs side by side) |
| `TERRARIA_NOACTIVATE=1` | a VISIBLE window that never takes the focus or a monitor (test runs driven by window messages): created without activation (`SW_SHOWNOACTIVATE`), no task bar button, never asks for the foreground; borderless / maximise / minimise / fullscreen / move / bring-to-front requests of the game are logged (`[shell] window: the game asked for ...`), reported back as done and not applied; the game's message boxes are answered by the host instead of shown. `Game.IsActive` is what a posted `WM_ACTIVATEAPP` says (XNA's rule; the window starts inactive). The real pointer is never read and never moved |
| `TERRARIA_FAKE_MONITOR=<x>,<y>,<w>,<h> \| auto` | with `TERRARIA_NOACTIVATE`: the game is told it has ONE monitor of that size (at 0,0, as a primary monitor is) and its window requests are APPLIED for real inside that rectangle of the virtual screen: borderless, moves, sizes, maximise, fullscreen (`shell/window.rs`, `Apply::Fake`). The rectangle must lie outside the real desktop (the shell refuses to start otherwise; every later placement is checked too). `auto`: 1920x1080 at 240,160 beyond the lower right corner of the desktop. Minimise and bring-to-front stay unapplied |
| `TERRARIA_COARSE_TIMER=1` | a measuring aid: no `timeBeginPeriod(1)` (sleeps of the process then end on the system's 15.6 ms grid) |
| `TERRARIA_WINDOW_POS=<x>,<y>` | position of the window at creation, physical pixels of the virtual screen (may be outside every monitor) |
| `TERRARIA_FRAME_LOG=<file.tsv>` | one line per shell tick: tick, time, interval since the last tick (ms), `Update` calls, drawn, active, minimised, client size, ms in the game's `EndDraw`, ms of the whole tick |
| `TERRARIA_COMMAND_FILE=<file>` | the shell polls this file (every 4th tick), runs its `dump ...` / `aim ...` lines through the game adapter (answers in `TERRARIA_STATE_LOG`) and deletes it: state reads for a test driver in an unscripted run. Nothing is injected; `aim` only reports where the cursor would have to go |
| `TERRARIA_NO_GAMEPAD=1` | connected game pads are not reported to the game |

Common to both: `TERRARIA_HIDDEN`, `TERRARIA_SCREENSHOT`, `TERRARIA_SCREENSHOT_FRAME`. A hidden run must always set
`TERRARIA_HIDDEN=1`.

### Input scripts

One command per line, `#` comments. `N` is the shell tick (1 = first tick after start-up).

```
frame N key W down | up | press      frame N mouse 400 300        frame N click left|right|middle|x1|x2
frame N button left down | up        frame N wheel 120            frame N text hello world
frame N resize 1024 600              frame N shot                 frame N quit
frame N focus lost | gained          frame N minimise             frame N restore
frame N pad connect | disconnect [K] frame N pad button A down | up | press [K]
frame N pad stick left|right X Y [K] frame N pad trigger left|right V [K]
```

`pad`: a scripted game pad (K = 1..4, default 1). Buttons: `A B X Y Back Start BigButton LeftShoulder RightShoulder
LeftStick RightStick DPadUp DPadDown DPadLeft DPadRight`; sticks -1..1 with Y up; triggers 0..1. The lines are the
same `PadEvent`s the shell makes from Bevy's gamepad events, applied to the same `terraria_host::input::PadAdapter`
(`shell/pad.rs`); XNA's dead zones apply (a stick at 0.2 is inside the left stick's zone and moves nothing).

```
frame N wait SECONDS CONDITION       the script's clock stands still until CONDITION holds (another process decides
                                     when): ingame | menu | netmode K | players K | remote-moved PIXELS |
                                     tile [rel] X Y inactive | chat TEXT | status TEXT. After SECONDS of real time
                                     the script goes on and the run ends with exit code 1.
frame N loadshots FILE{}.png EVERY COUNT   while queued work is suspended (a loading screen): a screenshot every
                                     EVERY stalled ticks, COUNT of them, `{}` is the number
```

`focus lost` makes the (hidden) window inactive until `focus gained` (`Game.IsActive`, `Deactivated` / `Activated`);
`minimise` makes it inactive and stops drawing until `restore`.

A script's clock stands still while queued work (a world load, save or generation) is suspended at a yield point:
`frame N` counts the ticks in which no work was suspended, so a script does not have to know how long a load takes.

The script fills the same `InputSnapshot` as the real input; `resize` really resizes the (hidden) window, so the game
sees it through `WindowState` on the next tick; `quit` is a window close (`Exiting`, `UnloadContent`, exit).
`Mouse.SetPosition` from the game moves the scripted cursor. `scripts/shell_demo.input` is the scripted `shell_demo` run.

### Input and focus in a real window

A run with a real window and no script puts a message hook on its own window (`shell/native.rs`; the original does
the same: `WindowsMessageHook`, `WindowsIme`). One source per kind of input, so nothing arrives twice:

| what | source | the original |
|---|---|---|
| typed text | every `WM_CHAR` the window receives, in order, once: key presses (the system's `TranslateMessage`: layout, Shift, AltGr, Caps Lock, dead keys), characters posted by other programs, and an input method's result string (`WM_IME_COMPOSITION` / `GCS_RESULTSTR`). Not `WM_SYSCHAR`, not `WM_UNICHAR`. Bevy's `KeyboardInput::text` is not used here | `WindowsIme.PreFilterMessage`: message 258 |
| keys of `Keyboard.GetState` | the virtual keys of `WM_KEYDOWN` / `WM_KEYUP` / `WM_SYSKEY*`, Shift / Control / Alt by side; all released on `WM_KILLFOCUS` | `GetKeyboardState` (the same keys, kept by the system from the same messages) |
| `Game.IsActive` | `WM_ACTIVATEAPP` alone (seeded from the foreground window when the hook is installed) | `WindowsGameForm.WndProc`: message 28 |
| `Form.WindowState == Minimized`, `Form.ActiveForm` | `WM_SIZE`; `GetForegroundWindow() == hwnd` (normal run) | the game's `FocusHelper.UpdateFocus` reads both |
| mouse position | normal run, window in front or holding the capture: `GetCursorPos` + `ScreenToClient` (follows the pointer outside the client area: negative / beyond the size). Otherwise, and in every test run: the window's last mouse message | `Mouse.GetState`: `GetCursorPos` at every call, focus or not; without the focus the game keeps the old position itself (PlayerInput.cs:1400-1405) |
| mouse buttons, wheel | the window's messages (Bevy) | `GetAsyncKeyState` (the physical buttons, also outside the window) / a message hook |
| `Mouse.SetPosition` | normal run and window in front: the real pointer moves (`SetCursorPos`). Otherwise logged once and only the game's own cursor goes there. Never in a test run | `SetCursorPos`, always |
| game pads | Bevy's raw gamepad events (gilrs) -> XInput-like state -> XNA's dead zones in the host | XInput |

Without the hook (other systems, tests) Bevy's events are the source (`host/input.rs`, `collect`): text from
`KeyboardInput::text` and `Ime::Commit`, keys per press from the physical key and what it types (numpad by NumLock,
Latin letters by the layout, `, . - +`; other punctuation keys by their US position), focus from `WindowFocused`,
and a minimised window counts as deactivated.

Measured in the off-screen window (`work\logs\in_text_focus.txt`): bare `WM_CHAR` for `A ! ß я 漢` arrive and are
drawn in the name prompt and in the chat; characters typed by key messages after them arrive once each;
`WM_UNICHAR` gives nothing (as `DefWindowProcW` of the original's window); `WM_NCACTIVATE` + `WM_SETFOCUS` alone do
not activate the game, `WM_ACTIVATEAPP` does; a window minimised while active keeps `Game.IsActive`, stops drawing,
and the game's `IsSelectedApplication` turns false (its own rule).

## The shell

`src/shell/mod.rs` (docs there are the reference), `script.rs`, `window.rs`. A binary includes it with
`#[path = "../shell/mod.rs"] mod shell;` (the crate has no library target) and calls `shell::run(title, (w, h), make)`, or adds
`ShellPlugin::<G>` to its own `App` after `DefaultPlugins` and `GfxPlugin`.

```rust
pub trait HostedGame {
    fn initialize(&mut self);                 // Game.Initialize override
    fn load_content(&mut self);               // Game.LoadContent override
    fn begin_run(&mut self) {}                // Game.BeginRun
    fn update(&mut self, t: Gc<GameTime>);    // Game.Update
    fn draw(&mut self, t: Gc<GameTime>);      // Game.Draw
    fn end_draw(&mut self) {}                 // Game.EndDraw override (not in a fixed-step run)
    fn end_run(&mut self) {}                  // Game.EndRun
    fn unload(&mut self);                     // Game.UnloadContent (run by Dispose)
    fn exit_code(&self) -> u8 { 0 }
}
```

Threading: the host is single-threaded by design. The game and all state live in one `NonSend` resource and the whole
frame is ONE system that takes it, so Bevy pins every game call (including the `GetData` hook that runs inside `draw`) to
the main thread, in a fixed order. Reason in the comment at the top of `shell/mod.rs`.

### Frame sequence

Mirrors `reference\xna\Game.cs`. First tick, `RunGame` (247-263): `game::init` (`CreateDevice`) -> `initialize` ->
`load_content` (XNA runs it at the end of the base `Initialize`, 503-514) -> `begin_run` -> one `update` with zero elapsed
time. The tick after start-up runs with zero elapsed time (`ResetElapsedTime`, 485-492).

Every tick (`Tick`, 312-410; `DrawFrame`, 526-545):

1. gather input and window state: keys, mouse position / buttons / wheel, typed text, game pads (Bevy's `Gamepad`
   entities), client size, focus, monitor size, monitor list and display modes (`winforms::set_monitors`,
   `set_display_modes`); an inactive window sleeps `InactiveSleepTime` (318-321). Where each kind of input comes
   from: "Input and focus in a real window" above;
2. `game::begin_frame` (clock clamp, fixed-step accumulator, catch-up count, `IsRunningSlowly`, device reset on a size change,
   `Activated` / `Deactivated`), then `game::dispatch_events` (`ClientSizeChanged`, typed characters to the IME listeners);
3. `while let Some(t) = begin_update() { update(t); end_update() }` (354-371);
4. no draw when no update ran, all updates suppressed it, the game exited, or the window is minimised
   (`!ShouldExit && doneFirstUpdate && !Window.IsMinimized`, 529); else `begin_draw` -> `draw` -> the game's
   `EndDraw` override (`HostedGame::end_draw`) -> `end_draw` (`Present`). **Terraria paces itself in its `EndDraw`**
   (Main.cs `EndDraw`, `FrameSkipMode.Subtle`: it waits on the real clock until 1/60 s has passed since the last
   frame, then presents). With the window active the game runs with `IsFixedTimeStep = false`, so this wait is the
   only thing that holds it at 60 updates per second: without the call the game ran one `Update` per presented
   frame (measured in the first real-window run: 87 updates per second while a key repeated, 143 while the mouse
   moved; a 144 Hz display would have run it at 144). A fixed-step run (scripts) does not call it: every tick is
   one update there and the run is meant to be faster than real time;
5. outputs of `end_draw`: the frame goes to `PublishedFrames` (the render world replays it), audio commands to the mixer,
   title, `IsMouseVisible`, the `WindowRequest` (`ApplyChanges`: size, fullscreen, vsync), the `WindowCommand`s queued by
   `System.Windows.Forms` calls (`window.rs`), exit;
6. the collector, in slices (`tools\cs2rs\DESIGN.md` section 9): a cycle starts (`rt::gc_begin`) when 48 MB were
   allocated since the last one started or 3,600 ticks passed (`TERRARIA_GC_MB`, `TERRARIA_GC_TICKS`); every tick
   after that gives it 1.5 ms (`rt::gc_step`, `TERRARIA_GC_SLICE_US`) until the cycle is done. A mark phase does not
   end while a queued work item is suspended on its fiber (`threading::has_suspended_work()`). Whatever a game keeps
   across frames must be rooted (`rt::Rooted`).

Shutdown (`Game.Exit`, window closed, or script `quit`): `end_run` -> `raise_exiting` -> `unload`. The process exit code is
`exit_code()` (a test returns 1 on failure).

Start-up waits (up to 600 Bevy frames) until the render world holds the back-buffer image, so the first frame's draws and a
`GetData` in `load_content` have a target.

### Readback (`gfx/render.rs`)

`Texture2D.GetData` on a render target is synchronous, as in XNA. `GfxGpu` sits behind `Arc<Mutex<..>>` (`SharedGpu`) in both
worlds and the unreplayed frames are one shared queue (`PublishedFrames`); whoever holds the lock replays the queue in order.
The host's `readback_hook` (`WgpuReadback`, installed by the shell right after `game::init`) replays the queued frames, then
its partial frame, copies the target into a mapped buffer and waits for the GPU. State a partial frame leaves (bound target,
viewport, scissor, pending discard) carries into the next frame; the back-buffer view of the last render is cached so a
partial frame can draw into the back buffer. `docs\readback.md` in the host crate has the contract. Proof:
`readback_demo` (draws known patterns, reads them mid-frame, including a second draw into the same target and a discard
target bound twice, then draws on the back buffer; the screenshot equals the one of a run without reads).

## The translated client: `terraria`

`src/bin/terraria.rs` runs the machine-translated game (`terraria-gen-game`) under the shell. It is behind the cargo
feature `client` and has its own profile, so the other binaries and `cargo test -p terraria-app` do not compile the
1.13M-line crate:

```
CARGO_TARGET_DIR=target-app cargo build -p terraria-app --bin terraria --features client --profile client -j 4
TERRARIA_SAVE_DIR=<folder> TERRARIA_HIDDEN=1 TERRARIA_MUTE=1 TERRARIA_INPUT_SCRIPT=crates/terraria-app/scripts/g1_title.input \
  ./target-app/client/terraria.exe
```

Start it normally with `run-terraria.cmd` in the project folder (visible window, the game's own volume, saves in
`work\port-save`; it uses the `client-opt` build when there is one).

Two profiles (`port/Cargo.toml`; dependencies at opt-level 3, the app at 1, no debug info, no incremental cache).
Measured on this machine (Ryzen 5 7600X, 15 GB, `-j 4`):

| | `client` (generated crate at opt-level 0) | `client-opt` (generated crate at opt-level 2, 256 codegen units, no LTO) |
|---|---|---|
| first build (Bevy and every dependency for the profile + the generated crate) | 19 min 37 s | 23 min 57 s |
| after a change in `terraria-host` / `terraria-gen-rt` / the generated code | 4.5 - 6.5 min | 13 min |
| after a change in the app only (compile the binary + link) | 31 s | about the same |
| largest `rustc` process | 2.6 GB | 5.4 GB (sampled every 10 s) |
| `terraria.exe` | 366 MB | 265 MB |
| build folder | 3.4 GB | 2.4 GB |

Frame cost in play (hidden, scripted, 10,030 in-game ticks of walking, jumping, digging and fighting at 1280 x 720:
`scripts/g12_stability.input`; times in ms; "replay" is the CPU side of the GPU replay on the render thread: uploads,
pass encoding, submit; what the GPU needs afterwards is not measured):

| | `Main.Update` mean / p50 / p99 | `Main.Draw` (recording) mean / p50 / p99 | replay mean / p50 / p99 | collector pause mean / p99 / max | before the incremental collector: mean / max |
|---|---|---|---|---|---|
| `client` | 1.37 / 1.47 / 2.41 | 4.80 / 3.87 / 12.2 | 1.02 / 1.01 / 1.83 | 1.50 / 1.71 / 5.15 | 286 / 457, one every 6 s |
| `client-opt` | 0.64 / 0.66 / 1.22 | 1.95 / 1.36 / 5.45 | 0.89 / 0.78 / 2.26 | 1.49 / 1.73 / 5.59 | 127 / 163, one every 6 s |

The collector column is every pause of the incremental collector (a slice per tick while a cycle runs: 2,769
pauses in the `client-opt` session, 86 per cycle, 32 cycles). The target was "no pause above 4 ms": 6 of the
2,769 are between 4.0 and 5.6 ms, all sweep slices (mark slices: at most 2.1 ms; start of a cycle 0.7 ms; end of
a mark phase 2.2 ms). The shell prints these numbers per run, in wall time and in CPU time of the game thread
(`QueryThreadCycleTime`: a pause in which the thread was not running is not the collector's), with
`TERRARIA_FRAME_TIMES=1`. `TERRARIA_GC_FULL=1` brings the old behaviour back for comparison.

Both leave most of a 16.7 ms frame free. One frame of 2 s remains at the end of the splash
(`Initialize_AlmostEverything`, as in the original, which also stalls there), and entering a world blocks one frame
for the load.

Heap and collector: 5.81 million objects / 167.9 MB are live once `ref_small.wld` is loaded (5.04 million of
them are the `Tile` objects); the game allocates about 139 KB per tick in play. The collector works in slices
(`tools\cs2rs\DESIGN.md` section 9): a cycle starts when 48 MB were allocated since the last one started
(`TERRARIA_GC_MB`) or after 3,600 ticks, then every tick gives it 1.5 ms (`TERRARIA_GC_SLICE_US`) until it is done:
about 86 ticks in `client-opt`, 208 in `client`, one cycle every 6 seconds of play. After a cycle the heap is 6.0
million objects / 180 MB and stays there (tick 2,334: 6,013,538 objects; tick 10,822: 6,039,728): what died while
the cycle ran is freed by the next one. About 4,100 objects are traced a second time at the end of each mark phase
(host objects whose stores are not behind the barrier: textures, JSON tokens, effect passes, cues). The end of a
mark phase waits while a queued work item is suspended.

Other measurements of that session: no stop in 10,030 in-game ticks (four deaths and respawns included), 0 gaps, 0
unimplemented host members apart from the unused `TcpClient` object. A session with the collector's poison mode and
the `gc-check` feature (every dereference of a freed object panics; `tests/client.rs` of `terraria-gen-game` with
`CLIENT_POISON=1`, the whole `g7_play.input` session: 4,370 in-game ticks, 53 collections, 11.4 million objects
freed) ran clean. Audio: muted runs mix without a device and print the mix's statistics (10,000 ticks: 4,468 blocks, 550 voices
started, at most 10 at once, RMS -21.8 dBFS, peak +5.2 dBFS, 1,123 of 9.15 million samples above full scale: the
sum is not limited by the mixer any more, the system's audio engine does that as for the original); one run on the
real output device at 3 % volume (`AUDIO_DEVICE=1`, `g6_enter_world.input`: title music, menu sounds, entering the
world): the device opened as 32-bit float, 1,029 blocks written, 26 voices, peak 438. Nobody listened to it.

The adapter is thin: `terraria_gen_game::host::launch` is `Program.LaunchGame` / `RunGame` up to `main.Run()`
(translated statements; `SocialMode.None`), `Main.Initialize` runs with `LoadContent` inside it (the host calls the
registered override from `base.Initialize()`), then the shell calls `Main.Update` / `Main.Draw`. The working directory
becomes the save root at start-up: the few files the original writes by relative path (`client-crashlog.txt`) land
there. Environment variables of the binary (`TERRARIA_KEEP_GOING`, `TERRARIA_HOST_REPORT`, `TERRARIA_TRACE`,
`TERRARIA_FRAME_TIMES`, `TERRARIA_STATE_LOG`, `TERRARIA_LOADER_ACTIONS`) are in its module docs; the shell adds
`TERRARIA_DISPLAY=<W>x<H>` (the one monitor the game is told about; a scripted run defaults to 1920x1080 so that the
game's default UI scale, which it derives from the display height, is the same everywhere: 1.1), `TERRARIA_GC_MB`
and `TERRARIA_GC_TICKS`.

A stop in translated code (something the port does not have yet) or an uncaught C# exception ends the run with exit
code 1 and its text on stderr; `TERRARIA_KEEP_GOING=1` logs it once and continues (survey aid).

### Scripted sessions

`scripts/run_hidden.sh <script.input> [name] [seconds]` runs one hidden, muted session on a fresh save root
`work\port-save-runs\<name>` (with a copy of `work\refsave\Worlds\ref_small.wld`; `KEEP_SAVE=1` continues on the
existing one), writes `work\logs\<name>.log` and `work\logs\<name>.state.jsonl`, and kills the process by PID if it
overruns. Besides the input commands above, a script can talk to the game adapter
(`terraria-gen-game/src/host/debug_state.rs`; the headless survey `tools/cs2rs/p1.sh --client` with `CLIENT_SCRIPT`
runs the same commands without a GPU):

```
frame N dump player | world | npcs | items | combat | recipes       JSON lines read from the translated statics
frame N dump tile X Y | tile rel DX DY | tiles X0 Y0 X1 Y1 | item SLOT
frame N dump ui | menu | players | net | chat      the UI element tree with its snap points / the title menu's entries /
                                     every active player / Netplay and the connection state / the chat lines
frame N aim tile X Y | rel DX DY | enemy | slot N | recipe K | ui X Y | settings      the scripted cursor moves there
frame N aim snap NAME [ID] | aim menu K            onto a UI snap point (as `dump ui` names it) / the K-th title menu entry
frame N game moondial | sundial        the game's own Main.Moondialing() / Sundialing() (time runs 60x until dusk / dawn)
frame N game give ITEM [STACK]         the game's own Player.QuickSpawnItem for the local player (item id)
frame N set time T day|night           writes Main.time / Main.dayTime directly (the only command that writes state)
```

| script | what it does | evidence |
|---|---|---|
| `g1_title` .. `g5_world_select`, `g6_enter_world.input` | first start: language, title, new player "Porter", the world list, into `ref_small` | `work\shots\g0..g4*.png`, `g5_ingame_spawn.png` |
| `g7_play.input` (written by `make_play_script.py`) | walk, jump, dig, place, chop a tree, inventory, crafting, night, sword fight, map, settings, Save & Exit | `g6_walk`, `g6b_jump`, `g7_dig`, `g7b_place`, `g7c_tree`, `g8_inventory`, `g8b_craft_list`, `g8b_craft`, `g9_combat_00..04`, `g10_map`, `g10b_settings`, `g10c_title_after_exit` |
| `g11_reload.input` (`KEEP_SAVE=1`, same name as the play session) | second start on the saved config, player and world | `g11a_player_list`, `g11b_world_list`, `g11_reloaded` |
| `g12_stability.input` (`make_play_script.py stability`) | 10,000 in-game ticks of moving, digging and fighting | `g12_stability_5000`, `_10000`, `_end` |
| `g13_window.input` | in the world: focus lost / gained, minimise / restore, resizes (1024x600, 1600x900, an odd size), Alt+Enter twice, the cursor outside the window, focus lost with a key held | `g13a_focus_lost` .. `g13f_end` |
| `g14_trails.input` | Zenith, Light Disc and Magic Missile from `game give`, used: projectiles drawn as vertex strips | `g14a_zenith` .. `g14d_end` |
| `g15_create_world.input` (`NO_WORLD=1`) | Single Player -> player -> New -> Small / Classic / random evil -> name `ref_small`, seed `12345` -> Create; the loading screen; back in the world list; into the new world | `g15a_world_creation_default`, `g15b_.._filled`, `g15c_generating_00..03` (the loading screen at four moments), `g15d_world_list_after`, `g15e_ingame`; the file: `work\item1\wld_diff.txt` |
| `g16_language.input` | the language list from the title: German, Russian, Japanese, Chinese, Korean (CJK glyphs), settings and title in German and Japanese | `g16a_language_list_english` .. `g16k_player_select_japanese` |
| `g17_gamepad.input` | a scripted game pad from the title into the world: D-pad through the title menu (the game switches to its gamepad interface: button glyphs), A on Single Player / New / Create, the on-screen keyboard (one key with A, submit with RT), A on the Play buttons, then in the world the left stick (walk), LT (jump: the default profile's binding; A is QuickMount), RT with the right stick (use item), Y (inventory, D-pad between slots), a stick inside the dead zone, disconnect | `work\shots\in_pad_00..16*.png`, `work\logs\in_pad.state.jsonl` (`dump input`: `currentInputMode` 2 -> 4 -> 3 -> 4 -> 3 -> 2) |
| `mp_host.input` + `mp_join.input` (`scripts/run_multiplayer.sh`) | two processes: A Host & Play (the game's server is this executable with `-server`), B Join via IP 127.0.0.1:47800; B digs and walks, A chats, B leaves, A saves and exits | `mp_a*.png`, `mp_b*.png`, `work\logs\mp_a.*`, `mp_b.*` |
| `mp_join_original.input` (`scripts/run_original_server.sh`) | the port's client against the ORIGINAL `TerrariaServer.exe` on 127.0.0.1:47801 | `mpo_*.png`, `work\logs\mpo_server.log`, `mpo_client.*` |

Real-window sessions (a visible window driven by window messages; nothing touches the real mouse, the keyboard
focus or the screen): `scripts/realwin/rw.py` (`launch`, `do "<commands>"`, `kill`; its docstring lists the commands),
`stability.py`, `pacing.py` + `pacing_report.py`, `audio_check.py` + `audio_analyze.py`, `save_and_exit.py`,
`enter_world.py`. They use the save root `work\port-save-realwin` (`RW_SAVE` picks another one below `work\`).
The guard of `rw.py` now watches for the whole run, not only the first 40 s: a window of the client inside the
virtual screen, or the client as the foreground window, ends the process (`rw_<name>.foreground.log` says how often
it fired).

- `window_modes.py title | maximized`: the game's window modes applied for real against a fake monitor
  (`TERRARIA_FAKE_MONITOR=auto`, save root `work\port-save-in-window`): the default borderless start, Alt+Enter,
  the video menu's borderless / resolution / fullscreen switches, the HUD in the world, a maximised start and the
  switch from it to borderless. Every step asks the window (`GetWindowRect`, `GetClientRect`, `GetWindowLong`) and
  the game (`dump input`), takes a screenshot (`work\shots\in_win_*.png`) and writes a line to
  `work\logs\in_window_title.jsonl` / `in_window_maximized.jsonl`. Slow gate `window-modes` of `gates.sh`.
- `pace_load.py [seconds] [tag]` (one client, in a world): frame pacing for `seconds` as the machine is and for
  `seconds` with N-1 busy-loop processes (started and killed by the script); `pace_ab.sh` runs it for several
  variants of one executable.

All of them as one command, without a window or a GPU (the headless survey, every call caught on its own; fails
on any stop): `sh tools/cs2rs/scripts.sh`. Every gate of the port with one summary table: `sh tools/cs2rs/gates.sh`.

The saved world of a session is checked with `cargo test -p terraria-gen-game --test saved_world`
(`SAVED_WORLD`, `ORIGINAL_WORLD`, `EXPECT_TILES`): it must parse with the verified reader and differ from the
original only where the session worked and where a world changes by itself.

### When something goes wrong in a normal run

`run-terraria.cmd` starts a visible, unscripted run. In such a run:

- **A stop** (translated code reached something the port does not have) abandons that `Update` / `Draw` call; the
  game goes on with the next frame. **A host member that is not implemented** did nothing; the game went on. Each
  distinct one is written once to `port-crashlog.txt` in the save root (time, tick, the member or the stop's text,
  the last 200 lines of the host log) and named in the window title.
- **An uncaught C# exception** ends the game as the original does: `Program.DisplayException` writes
  `client-crashlog.txt` and shows a message box. In a visible run the game's message boxes are real dialogs
  (`MessageBoxW`); hidden and scripted runs answer them with the default button and log them.
- `last-run.log` in the save root has everything the client printed.

A scripted run stops at the first stop with exit code 1 (`TERRARIA_STRICT=1` makes an unscripted run do the same;
`TERRARIA_KEEP_GOING=1` is the survey mode that goes on after everything).

### Known issues

Blunt list; "not verified" means nobody compared it with the real game.

Playing:
- **A visible window was run by programs, never by a person** (2026-10-12, two stages; what was done, the
  evidence and the numbers are in the two status snapshots of that date in `plan.md`). The window is shown without
  activation outside the desktop (`TERRARIA_NOACTIVATE`, `TERRARIA_WINDOW_POS`, `TERRARIA_FAKE_MONITOR`) and driven
  with window messages (`scripts/realwin/rw.py`): menus, character and world creation, typed text, play, inventory,
  crafting, chat, resize, focus and activation by messages, minimise / restore, the window modes against a fake
  monitor, Save & Exit, Exit with code 0, ten minutes of mixed play without a stop.
  **What only a person in front of the window can still check** (none of it can be done without taking the
  focus, the pointer or the screen):
  1. the real focus: Alt+Tab away and back, a click on another window, Win+D, a UAC prompt, the lock screen (the
     rule is unit-tested over written message sequences, and `WM_ACTIVATEAPP` was posted; the system never sent one);
  2. the real pointer: the position following it outside the client area and during a drag out of the window
     (`GetCursorPos` path), the system cursor hidden over the client area, mouse buttons pressed outside;
  3. real key strokes: capitals, shifted symbols, AltGr, dead keys, another layout than US, Caps Lock, the numpad
     with NumLock on and off (`TranslateMessage` needs the real key state; posted key messages type lower case);
  4. an input method (IME): composition, the composition string in the text field, the commit;
  5. a real game pad: gilrs's numbers for a real device (stick range, trigger range, button names), hot-plug,
     which pad becomes player one, vibration;
  6. the default borderless window and Alt+Enter on the REAL monitor (task bar covered or not, other windows,
     a second monitor, a DPI scale other than 100 %); fullscreen at a resolution below the monitor's;
     maximise / restore by the title bar; exclusive fullscreen does not exist in the port;
  7. the real message box of a crash; how 60 frames per second look on the 143 Hz display; the sound by ear.
  `run-terraria.cmd` is two steps further than a first try, not more.
- **Frame pacing.** The game holds 60 updates per second itself, in `Main.EndDraw` (a busy wait, one core at
  100 %, as the original). Measured in a presenting window at 1280 x 720 on a 143 Hz display: frame interval mean
  16.640 ms, p99 17.25, max 29.9 over 60 s, 60.2 updates per second.
**On a busy machine it gets worse, and nothing of the port's own was found that changes it.** The client needs
  about 1.16 cores (the game thread spins through the rest of every frame, as the original's; the render thread
  takes the rest). With a synthetic load (`scripts/realwin/pace_load.py`: 5 busy-loop processes at normal
  priority on the 6 logical processors, 60 s) the update rate follows the CPU share the client still gets
  (the `client` build, whose frame costs 7 ms of work): 116 % of a core: 60 updates per second; 81 - 87 %:
  52 - 56; 69 - 75 %: 44 - 51; 49 %: 31; 32 - 36 %: 20 - 25. The `client-opt` build (2.7 ms of work) held 59
  updates per second at 74 % of a core under the same load, with 115 of 3,600 frames above 33.4 ms
  (`work\logs\in_pacing_*.txt`; frame intervals then p99 43 - 53 ms, 110 - 390 of about 3,000 frames above
  33.4 ms). Runs of one and the same configuration differed by more than the configurations did (43.7 to 55.9
  updates per second under the same synthetic load), because other programs were compiling on the machine during
  every measurement; no run was made on an idle machine. Looked at and measured:
  - `timeBeginPeriod(1)` is set now (sleeps of the helper threads end on a 1 ms grid: the mixer's 2 ms sleeps,
    `InactiveSleepTime`). No effect on the frame intervals beyond the spread between runs;
    `TERRARIA_COARSE_TIMER=1` takes it out for a comparison.
  - Bevy's main schedules on one thread (no hand-over to its task pool): no effect beyond the spread; not kept.
  - Sleeping through most of the game's busy wait (the shell sleeps until 2 ms before the frame is due): worse.
    Ten-second blocks of each in turn in ONE run: 57.5 against 60.1 updates per second without the synthetic
    load, 47.1 against 54.9 with it (`work\logs\in_pacing_alt.txt`). Not kept: the game's own spin is the better
    pacer here.
  - The collector slices of 12 ms wall time for 1.6 ms of CPU are slices in which the thread was taken off the
    processor: their budget is wall time, so such a slice ends early and does less; nothing to fix.
  - Thread priorities are all normal, as the original's one thread.
  What the original does under the same load was not measured (it may not be started). How it looks was not seen.
- Text input is the window's own `WM_CHAR` messages now (a hook on the shell's window, as the original's): text
  posted by tools arrives, exactly once, also mixed with key presses (`in_text_*.png`: `A!ßя漢` in the name
  prompt, the player file `A!ßя漢b.plr`, the chat line). An input method's result string is read from
  `WM_IME_COMPOSITION` and the IME is switched on while the game edits text; **no IME was ever used on it**, there
  is no candidate list, and whether the composition string shows in the field is unknown. A character outside
  the Basic Multilingual Plane (an emoji) arrives as two UTF-16 units, as in the original, but the runtime's
  strings cannot hold half a pair: each half becomes U+FFFD (seen in the chat: `**`). `WM_UNICHAR` is not text
  (the same in the original). A held key repeats characters as the original; Backspace repeats by the game's own
  timer.
- Keys are the layout's virtual keys in a real window (the key labelled Z on a German keyboard is `Keys.Z`), read
  from the key messages. Never typed on by a person; see point 3 above.
- The mouse position follows the real pointer outside the client area in a normal run while the window is in
  front (XNA: always; the game ignores the mouse without the focus anyway). In test runs and in the background it
  stays at the last mouse message. Mouse BUTTONS come from window messages, not from the physical state: a button
  pressed outside the window is not seen (XNA's `GetAsyncKeyState` sees it).
- `Mouse.SetPosition` moves the real pointer when the window is the foreground window of a normal run, and only
  the game's own cursor otherwise. **Nothing in this build of the game calls it** (1.4.5.8 moves its gamepad /
  keyboard cursor through `PlayerInput.MouseX / MouseY`), so the path has never run outside the unit test of its rule.
- Game pad: works in a scripted session from the title menu into play (`g17_gamepad.input`, `in_pad_*.png`: the
  game switches to its gamepad interface with button glyphs, the on-screen keyboard, stick / trigger / button
  play), with XNA's dead zones on values rebuilt from gilrs's numbers. **No button was ever pressed on a real
  pad**; whether gilrs reports a real pad's full stick and trigger range is unknown (XNA's zones are applied to
  `round(v * 32767)` / `round(v * 255)`). `GamePad.SetVibration` is handed to Bevy's gamepad rumble (gilrs force
  feedback); the game never calls it in this build, and a scripted pad logs it as inert. Jump is on the LEFT
  TRIGGER in the game's default gamepad profile ("Redigit's Pick"); A is QuickMount.
- Window modes: applied for real against a fake monitor outside the desktop (`window_modes.py`,
  `in_win_*.png`): the default start is a borderless window with the monitor's size (**new: before this stage a
  normal run would have started as a 1280 x 720 borderless window in the monitor's corner**, because the game
  took its own resize request back; `game::set_window_follows_apply_changes`), Alt+Enter, the video menu's
  switches, a maximised start, the HUD at 800 x 600, 1280 x 720 and 1920 x 1080. Differences to the original:
  fullscreen is a borderless window over the monitor at the monitor's resolution (no exclusive mode, no mode
  switch, a lower resolution is not scaled up); borderless keeps the window's style bits (winit removes the
  frame through `WM_NCCALCSIZE`); leaving fullscreen goes back to the window size from before; in the fake
  monitor a maximised window is placed by the shell and has no `WS_MAXIMIZE`. Hidden and scripted runs still keep
  their 1280 x 720 window whatever the game asks for.
- Focus: `Game.IsActive` follows `WM_ACTIVATEAPP` alone, as XNA; a minimised window is NOT inactive by itself any
  more (it stops drawing, and the game's own `FocusHelper` stops taking input: its rule). A scripted `minimise`
  still deactivates, as a person's minimise does.
- World creation from the menu works in a scripted run (`g15_create_world.input`: Small / Classic, seed
  `12345`): the file's ten data sections are byte-identical to the real game's world of that seed
  (`VERIFICATION.md`). Medium and large worlds and the special seeds were generated only through the server's path
  (five more worlds identical pass by pass), not from the menu; Journey / Expert / Master from the menu were not tried.
- **Loading screens**: world load, save and generation give the frame back (8 ms of work per frame in a normal
  run); the progress text and bar are drawn while they run (`g15c_generating_00..03.png`: four different progress
  texts, looked at; in the real window `rw_17_gen_strip.png`: a small world from the menu in 20 s with changing
  text and bars).
- **Multiplayer: works in two scripted scenarios, nothing else was tried** (see "Multiplayer" below). No Steam
  lobbies / invites / friends (`SocialMode.None`): only "Join via IP" and "Host & Play".
- A lost graphics device ends the process (seen once: `wgpu` "Out of Memory" while four compilers were running on
  the machine): there is no recovery.
- No Steam / Workshop / cloud, no RGB peripherals, no resource packs from zip files (`ZipContentSource.Refresh` is a
  refused body), no clipboard, no IME candidate list.
- Sound was never listened to. The mixer sums in float and hands the sum to the system's audio engine as the
  original does (32-bit float output; in the scripted sessions at most 8 voices at once, 739 of 11 million samples
  above full scale, peak +4.2 dBFS). One loopback recording of the output device at 3 % volume (19 s): the title
  music is tonal and has a beat every 0.60 s, and with the music off every one of six inputs (hotbar change,
  inventory open / close, two digs) is followed by a sound 50 - 140 ms later out of digital silence. Whether the
  output sounds right is unknown.
- Weapon and projectile trails (`VertexStrip`: Zenith, Light Disc, Magic Missile, Flamelash, Rainbow Rod, the
  Empress blades ...) are mapped now (`DrawUserIndexedPrimitives<VertexStrip.CustomVertexInfo>` through the host's
  raw-vertex path). `g14_trails.input` gives three of them through the game's own item spawn and uses them; see
  the status snapshot in `plan.md` for whether its screenshots were taken and looked at.
- The first click of a first start lands on the sun behind "English" and awards "Going Oldschool", as it would in
  the original for a click there.
- At 1280 x 720 with the default UI scale (1.1 on a 1080p display) the equipment column of the inventory overlaps
  the minimap (`g8_inventory.png`). The layout code is the game's own; whether the original looks the same at this
  size was not verified.

Fixed in the input / window stage (were in this list): bare `WM_CHAR` dropped; the mouse frozen at the window's
edge in a normal run; `Mouse.SetPosition` never on the real pointer; `GamePad.SetVibration` a logged "not
implemented"; a pad button stopping `Main.Draw` (`GetType()` of a glyph snippet: the snippet classes have
reflection data now, `tools\cs2rs\game.json`); window modes never applied.

Fixed in the stage before (were in this list): the collector's stalls (below), achievements not saved
(`achievements.dat` is written with the structure the game writes and a second session does not award again),
loading screens frozen, the mixer's hard clipping, a window resize not reaching the game.

Fidelity:
- Rendering was not compared with captures of the real game (none exist for these scenes). Against the hand-ported
  viewer at the spawn: terrain, sky colour and the black underground are identical; 55 - 88 % of the dirt pixels are
  bit-identical and patches differ by up to 36 of 255 (not diagnosed).
- The simulation is bit-identical to the real server in the two recorded scenarios (`VERIFICATION.md`, Proof 3), and
  world generation reproduces the reference world (Proof 2), with one measured exception to the float-allocation
  rule listed in `tools\cs2rs\game.json` (`floatInMemory`). Other locals of that kind will only show up when a
  comparison reaches them: another seed or world size can. `Math.Sin` and friends are mapped to the x87
  implementations in their plain double form; the register (`_ext`) forms are not used, and nothing compared so
  far needed them.
- **Walking speed 3.6 against the server recording's 2.9 - 3.1: explained, the client is right.** The client's
  player has the Happy! buff (146) of the sunflowers at the spawn: `Player.UpdateBuffs` (Player.cs:11223-11227)
  does `moveSpeed += 0.1f; moveSpeed *= 1.1f`, and `maxRunSpeed *= moveSpeed` (26350) makes 3 x 1.21 = 3.63 (the
  state dump has `maxRunSpeed` 3.63, buff 146). The buff is given by `if (SceneMetrics.HasSunflower) AddBuff(146,
  2)` (25889-25892), inside `if (whoAmI == Main.myPlayer)` (25859): only a client gives it to its own player. The
  server recording simulates a remote player with no client behind it, so `moveSpeed` stays 1 and the velocity
  swings around `maxRunSpeed` 3.0 in steps of `runAcceleration` 0.08 (2.84 .. 3.08). Cheapest check on the real
  client, should one be wanted: stand at the spawn of `ref_small`, hover the buff icon (it reads "Happy!"), walk
  right for 5 seconds and read the position before and after from the map coordinates (about 1,090 pixels = 68
  tiles with the buff, 900 pixels = 56 tiles after walking out of sunflower range).
- The single-player path (`netMode 0`) has no reference recording: the comparisons run the server path.

Port internals:
- 48 method bodies are still refused by the translator (`tools\cs2rs\GAME_REPORT.md`; were 62): 36 of them the
  Reflection.Emit deep cloner (`Testing.Cloning`), 12 others. Each is listed with its callers and the reason it
  cannot be reached in this build in `tools\cs2rs\VERIFICATION.md` ("Refused bodies"). **The deep cloner was not
  generated**: its users are the debug overlay (`Projectile.Damage` under `DebugOptions.DrawHitboxes`) and the
  debug tools `Testing.StateSnapshot*`, switched on only by `DebugOptions.enableDebugCommands`, which nothing but
  its own static constructor writes.
- `CreativeUnlocksTracker.ValidateWorld` calls itself in the ORIGINAL too (the IL of the method is a call to its
  own token; no other caller exists): the translation is faithful, the function is dead code.
- `[StructLayout(LayoutKind.Explicit)]` is honoured for structs of primitive fields and of struct fields
  (`ReLogic...IME.Windows.NativeMethods.INPUTUNION`: 32 bytes).
- Numbers and dates follow the game's language: the number formats of the 11 other cultures the game can set are
  tables taken from the real .NET Framework (`tools\cs2rs\tests\framework\culture_numbers.ps1`, 11,167 expected
  strings in the test `culture_formats_match_dotnet`). String comparison, case mapping and number parsing still
  follow the en-US model.
- The collector may now run while work is suspended (it scans the stacks of suspended fibers conservatively).
  Price: with a network thread suspended in every frame of a multiplayer session, the pause that ends a mark
  phase was 23 - 44 ms on average and up to 72 ms in the scripted sessions (single player: 1 - 3 ms).
- `ConditionalWeakTable` entries are never dropped. `string[]` passed as `object[]` is a copy.
- The asynchronous readback, depth / stencil and device-lost handling of the replay are missing (see below).


### Multiplayer

What exists: "Join via IP" and "Host & Play" from the Multiplayer menu, over TCP, with the game's own protocol code
(translated) on the host's sockets (`terraria-host/src/sys/net.rs`; README there, "Networking").

- **Host & Play** starts a dedicated server as the original does (`Main.HostAndPlay` starts `TerrariaServer.exe`):
  here the same executable with `-server` in front of the game's arguments (`src/server.rs`), no window, its console
  in `server-last-run.log` of the save root. It listens on port 7777 on EVERY interface (the game's
  `IPAddress.Any`) and announces itself on the LAN once per second (UDP 255.255.255.255:8888), exactly as the
  original; Windows may ask for a firewall permission. It ends when the hosting player leaves (`-autoshutdown`).
  What it does when the hosting client is killed was not tried: if a second `terraria.exe` stays in the task
  manager, end it there.
- `terraria.exe -server -world <file.wld> [-port n] [-ip a] [-password p] [-savedirectory d] ...` is the dedicated
  server on its own (the launch parameters of the original server). Only the `-autoshutdown` form was run; the
  interactive console (world menu, typed commands) was never tried.
- Automated runs set `TERRARIA_NET_LOOPBACK_ONLY=1` (127.0.0.1 only, no announcement) and
  `TERRARIA_HOSTPLAY_PORT`; `run-terraria.cmd` does not.

Verified (`tools\cs2rs\VERIFICATION.md`, "Multiplayer over real sockets"): two processes of the port (join, a tile
dug by one seen by the other, movement, chat, leave, save and exit, all exit code 0), and the port's client against
the original `TerrariaServer.exe` 1.4.5.8 (join, dig, walk, chat, leave). Everything else is untried: more than two
players, the original client against the port's server, passwords, kicks and bans, PvP, bosses and events, medium
and large worlds, long sessions, a connection that drops or lags (loopback only), IPv6, a host name instead of an
address (`Dns` is implemented, not exercised in a session).

Known costs: longer collector pauses in a session (see "Port internals"); the server process collects without a
frame budget (slices of up to 0.7 s seen).

## Connecting the translated `Main`

What exists on the translator side (read `tools\cs2rs\NATIVE_SCHEMA.md` "Class hierarchies"): a translated game class cannot
derive from a native class, so `Main : Game` has no base object and the `Game` members are free functions in `terraria_host::game`
(`game::exit()`, `game::set_is_mouse_visible`, `game::window()`, `game::content()`, `game::graphics_device()`, `game::suppress_draw()`,
`game::base_noop()` for `base.Initialize / Update / Draw`, `game::add_exiting` ...). So `Main` does not implement `HostedGame` itself; a
thin adapter does, holding what the translated code needs (usually nothing: `Main`'s state is statics and a `Gc<Main>`):

```rust
struct TerrariaGame { main: Gc<Main> /* rooted */ }
impl HostedGame for TerrariaGame {
    fn initialize(&mut self)   { /* Program.Main's work: SocialAPI.Initialize(Some(SocialMode::None)), LaunchInitializer ...;
                                    main = Main::new(); Main::Initialize(main) */ }
    fn load_content(&mut self) { Main::LoadContent(self.main) }
    fn update(&mut self, t: Gc<GameTime>) { Main::Update(self.main, t) }     // translated Main.Update(gameTime)
    fn draw(&mut self, t: Gc<GameTime>)   { Main::Draw(self.main, t) }
    fn unload(&mut self)       { Main::UnloadContent(self.main) }
}
fn main() { shell::run("Terraria", (1280, 720), || TerrariaGame { /* Rooted */ }); }
```

Things to settle when it is connected (the shell cannot know them):

- **`LoadContent` order.** The shell calls `load_content` after `initialize` returns. In the C# `Main.Initialize`, `base.Initialize()` (which
  runs `LoadContent`) is in the middle, so code after it expects loaded content. Either the adapter calls `Main::LoadContent` itself from
  inside the translated `Initialize` at the mapped `base.Initialize()` call and its `load_content` does nothing, or split `Main.Initialize`
  there. The mapped `base.Initialize()` is `game::base_noop()`, which does not call back.
- **`new Main()`** runs after `game::init`, inside the shell (the adapter's constructor closure is called then), because it creates
  `GraphicsDeviceManager`, services and window through the host. `Program.Main`'s start-up (command line, `Platform.InitializeClientServices`, `SocialAPI`)
  has to run in `initialize` or before `shell::run` (host state needs `game::init` first for anything touching the window).
- **Rooting.** `rt::collect()` runs between frames. Everything translated code keeps in statics is the generated code's responsibility to root;
  a hand-written adapter must root its own handles.
- **Thread work.** `Thread.Start` / `ThreadPool` work runs at `end_update`, on the game thread (host README "Threading model").
- **Back buffer usage.** `Main`'s `PreparingDeviceSettings` handler sets `PreserveContents`; the host reads it at `ApplyChanges`.
- **Window requests.** `Main.SetDisplayMode` goes through `Form` (`winforms`) and `GraphicsDeviceManager`; the shell serves both.
  Monitor and display-mode lists come from Bevy's `Monitor` entities.
- **Text input.** Characters arrive through the IME service listeners (`dispatch_events`), not `Keyboard`.
- **Message boxes** are auto-answered by the host (`winforms::set_interactive` is not used by the shell).

## What is missing on the app side

- Visible-window behaviour was run only on a window without the OS focus, outside the desktop: activation by posted
  messages, the minimised path (no draw), `InactiveSleepTime`, resize, and the game's window modes against a fake
  monitor work there. On the real monitor nothing was ever applied. Fullscreen is Bevy's borderless fullscreen on the
  current monitor, not exclusive.
- `WindowCommand::SendToBack`, `FlashIcon` and `KeepAwake` are not applied (no portable winit API / not wired). No window
  icon. No cursor image change.
- DPI: the window is created with scale factor 1.0, so the game sees client pixels and `IWindowService.GetScaling` is 1.0. Per-monitor DPI is not served.
- The asynchronous readback (`Command::Readback`, one frame of latency) is not served by the replay (logged once); nothing in the game uses it.
  Reading the back buffer is not supported (XNA cannot either).
- Game pads: Bevy's raw gamepad events (gilrs) through `shell/pad.rs`; the standard layout, untested with a physical pad.
  The player index is the order of connection, not XInput's user index.
- IME: the result string and the composition string; no candidate list; never used with a real input method.
- No device-lost recovery, no `PresentationParameters.MultiSampleCount`, no depth / stencil in the replay (host README "Graphics").
- Pacing: the game's own wait in `Main.EndDraw` (called by the shell in every run that is not fixed-step) holds 60 updates per
  second; the present mode (vsync as the game asks) only decides when the frame is shown. A hidden run without
  `TERRARIA_FIXED_DT` is real-time dependent.
- Audio device selection and the 3 % rule for test runs are in `host/audio.rs`; no volume UI.
- The effect shaders are replayed by `gfx/render.rs` (translated D3D9 passes); what is not translated is listed in its module docs and the host README.

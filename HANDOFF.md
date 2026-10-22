# VAEL - TERRARIA: handoff (written 2026-10-12)

For a new chat taking over. Read this, then `plan.md` (status snapshots at the end; the newest is the truth),
then the document for whatever you are about to touch.

## What this is

A Rust / Bevy 0.19.1 port of Terraria 1.4.5.8. User goal (set 2026-10-10): "Finish the game almost 1:1".
The port reads the player's own game files from `C:\Users\user\Documents\Games\Terraria` (read-only) and ships none.

## How it works (the one idea to hold on to)

The game is NOT hand-ported. A tool translates the decompiled C# into Rust mechanically:

1. `decompile\` : ILSpy C# of `Terraria.exe` and ReLogic (1,716 files). `db\terraria.sqlite` indexes it (gamedb + `port_*` tables from IL metadata).
2. `tools\cs2rs\` : the translator (C#, Roslyn with the semantic model). Config `game.json`. One command regenerates everything deterministically.
3. `port\crates\terraria-gen-game` : the generated game, about 1.13 million lines of Rust. Never edit it by hand.
4. `port\crates\terraria-gen-rt` : the hand-written runtime the generated code runs on (object model, incremental mark-sweep
   collector with a write barrier, collections, strings, .NET formatting, reflection data, fibers for "threads").
5. `port\crates\terraria-host` : native stand-in for XNA / .NET / third-party libraries (graphics command list, content, input,
   audio mixer + XACT music, file IO, JSON, regex, zip, sockets). No Bevy dependency.
6. `port\crates\terraria-shaders` : the game's 136 compiled shader passes translated from D3D9 bytecode to WGSL.
7. `port\crates\terraria-x87` : maths that reproduces the original 32-bit runtime's results (x87), plus `FLOAT_RULE.md`.
8. `port\crates\terraria-app` : the Bevy shell. Runs XNA's frame loop, replays the draw list with wgpu, feeds input, plays audio.
   Binary `terraria` (feature `client`, profiles `client` and `client-opt`).

Older, kept as oracles only: `terraria-sim` (hand-ported framing, lighting, collision, liquids), the viewer in
`terraria-app\src\main.rs`, `terraria-core`, `terraria-data`, `terraria-formats` (verified XNB and WLD readers).

## How to run

- Player: `run-terraria.cmd` in the project root. Saves go to `work\port-save`. Logs: `port-crashlog.txt`, `last-run.log` there.
- Everything automated: Git Bash, from `port\`. Toolchain is pinned to `stable-x86_64-pc-windows-gnu` (the MSVC linker is not installed; do not change it).
- All checks: `sh tools/cs2rs/gates.sh` (fast), `sh tools/cs2rs/gates.sh --slow` (adds the worldgen matrix and multiplayer).
- Build dirs: `target-gen` (generated crates and tests, `CARGO_INCREMENTAL=0`), `target-app` (app). `-j 4` or less; 15 GB RAM.
- Client build: `CARGO_TARGET_DIR=target-app cargo build -p terraria-app --bin terraria --features client --profile client -j 4`
  (4.5-8 min after a generated-code change; `client-opt` 10-23 min).
- Scripted hidden runs: `TERRARIA_HIDDEN=1` + `TERRARIA_INPUT_SCRIPT=<file>`; scripts in `port\crates\terraria-app\scripts\`.
- Real-window runs without taking the user's screen: `port\crates\terraria-app\scripts\realwin\rw.py` (window off the desktop, never activated, real messages via `bgwin.py`).

## What is proven against the real game

Reference data comes from harnesses that run the real game's own code headless (`tools\refdump*` -> `refdata\`).

| Check | Result |
|---|---|
| Startup data (items, NPCs, projectiles, recipes, tile data, ID sets, text) | 3,911,380 values, 0 different |
| World generation, 6 worlds (small / medium / large, notthebees, drunk, fortheworthy) | every pass identical; saved file data sections byte-identical |
| Server ticks: 600-tick scripted player, 3,600-tick world | identical, float bits included |
| Tile framing 5,040,000 tiles; lighting colours 175,900 tiles | 0 different |
| Collision, 62 method families x 20,000 calls | 57 families bit-exact; 5 differ on a small share by float rounding |
| Multiplayer | port to port, and the port's client on the ORIGINAL dedicated server (join, walk, dig, chat, leave) |
| Number / date formats, 12 languages | 11,167 strings equal to .NET |

Scripted client play that works: menus, character creation, world creation with animated loading, walk, jump, dig, place, chop,
inventory, craft, combat, trails, map, settings, save, reload, language switch. 10,000-tick and 10-minute real-window sessions without a crash.

## What is NOT done or not verified

- No person has played it in a normal focused window. Human-only checks: Alt+Tab focus, exclusive fullscreen, sound by ear,
  look on the 143 Hz display, a physical game pad, non-US keyboard layouts.
- The last worker (input and window gaps: IME / shifted text, mouse outside the window, game pad through the real adapter,
  window modes on a fake monitor, focus state machine, pacing under load) was told to wrap up early. Its final state is the
  newest snapshot in `plan.md`. Check that first.
- Multiplayer pauses 23-72 ms per collector cycle (conservative fiber-stack scan). Single player is 1-3 ms.
- 48 of 23,065 bodies are refused by the translator (36 are a debug-only deep cloner). List with reasons: `tools\cs2rs\GAME_REPORT.md`.
- Original client joining the port's server: never tried. More than two players, passwords: untested.
- Floats: the original runs on the legacy x86 JIT (x87). The `jit32` float mode is partial; `floatInMemory` in `game.json` holds
  measured exceptions. Trig differs from the original in about 1 call in 100,000 by one last bit, CPU-dependent.

## Rules that must carry over

- Never hand-edit generated files. Fix the tool, runtime, host or config, then regenerate.
- No silent defaults. Anything unimplemented goes through the host registry and fails tests unless marked inert in config with a reason.
- Game install is read-only (a write guard enforces it). Do not read, load or reference `OnlineFix.dll` / `steam_api.dll`. The port runs in `SocialMode.None`.
- Never run a windowed binary on the user's desktop from automation: hidden mode, or the off-desktop no-activate method. Never real
  mouse / keyboard automation, never take focus. Kill every process you start.
- The original server (`work\refserver\`, private copy) broadcasts on the LAN while it runs. Run it only time-boxed.
- Workers: one integrator at a time on `tools\cs2rs` + generated crates (two whole-game builds do not fit in RAM). Opus for
  translator / runtime work, sonnet for bounded pieces. Briefs must name owned paths and the gates to keep green.
- Disk on C: is tight and other projects (bo2, sn2) build here. Check `df -h /c` before big builds. Only regenerable build output
  under `port\target*` may be deleted, and say what was deleted.

## Things the user was told and should not be surprised by

- `terraria-x87` contains a machine transliteration of Microsoft / Intel C runtime maths routines. Fine privately; matters if distributed.
- One worker hard-deleted a stray folder it had created by mistake next to the project (`VAEL - TERRARIAefdata`).
- During the first real-window test the game window showed on the monitor for about 20 seconds before the guard existed.
- Build caches were deleted several times to free disk, the last time on the lead's authority without the user's answer.

## Where to look

| Need | File |
|---|---|
| Newest state, next stops | `plan.md` (end) |
| Translator design, float modes, reflection, formats | `tools\cs2rs\DESIGN.md` |
| Every proof and its numbers | `tools\cs2rs\VERIFICATION.md` |
| What is untranslated and why | `tools\cs2rs\GAME_REPORT.md` |
| Host layer, driver protocol, open requests | `port\crates\terraria-host\README.md`, `REQUESTS.md`, `COVERAGE.md` |
| App shell, env switches, scripts, known issues | `port\crates\terraria-app\README.md` |
| Float behaviour of the original | `port\crates\terraria-x87\FLOAT_RULE.md`, `README.md` |
| Reference data formats | `refdata\worldgen\README.md`, `refdata\ticks\README.md`, `refdata\MANIFEST.json` |
| Reference DB schema | `README.md` (project root) |

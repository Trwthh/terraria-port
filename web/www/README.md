# web/www: the browser page shell

## Play in the browser

Double-click **`play-in-browser.cmd`** in the `VAEL - TERRARIA` folder. It starts the game's server and the
multiplayer bridge without windows and opens your default browser at **http://terraria.localhost:7790/** (Chrome or
Edge 137+). The page shows only the game, full window; Alt+Enter toggles fullscreen. (`http://127.0.0.1:7790/` works
too, but it is another origin, with its own saves.)

- **Stop:** `play-in-browser.cmd stop` ends the two processes it started (their ids are in
  `work\play-in-browser.pids`). `play-in-browser.cmd status` says whether they run.
- **Game files:** read from `C:\Users\user\Documents\Games\Terraria` (set `TERRARIA_DIR` for another folder),
  read-only, only `Terraria.exe` and `Content`. There is no import step: the first start copies them into the
  browser in the background (about 260 MB, plus the 470 MB music bank, from this PC); later starts use that copy.
- **Saves:** characters, worlds and `config.json` live in the browser's private storage (OPFS) for
  `http://terraria.localhost:7790`, folder `saves`. They stay when the server stops. To download or add .plr / .wld
  files, use the developer page `http://terraria.localhost:7790/dev.html?httpinstall` and its "Saves" button.
- **Multiplayer:** Join via IP goes through the bridge (127.0.0.1:7780). Host & Play runs the server inside the
  page, and other games on this PC join it at 127.0.0.1:7777. Nothing listens beyond this PC.
- **Logs:** `work\logs\terraria-host.log` and `work\logs\ws-bridge-play.log`. The page's own log is in the browser's
  console (F12).

By hand: `python web\host\terraria_host.py` (port 7790; `--help`), and for multiplayer
`python tools\ws-bridge\ws_bridge.py --origin http://terraria.localhost:7790 --listen 7777 --to-page`.


The page that will run the Terraria port in a browser. It is plain JavaScript, with no framework and no build step.

The game is a `wasm32-wasip1` module (`game.wasm`). It imports `wasi_snapshot_preview1` and `tw`, and it draws by handing frames to `tw.gfx_frame`. The Worker replays those frames with WebGPU.

Until `game.wasm` exists, the page runs a JS mock module with the same exports. The mock:
- draws a test pattern through the same frame format and replayer,
- echoes input to the log panel,
- lists and reads `/install/Content` through the file imports,
- plays a 440 Hz tone when you click.

The contracts:
- **[HOST_ABI.md](HOST_ABI.md)** (version 2): the imports, the exports, WASI, input records and errno values.
- **[FRAME_FORMAT.md](FRAME_FORMAT.md)**: the bytes of one frame.

## Serve and open

```
cd web/www
python serve.py            # http://127.0.0.1:8137/  (127.0.0.1 only; Ctrl+C stops it)
python serve.py --port 9000
```

Open `http://127.0.0.1:8137/` in **Chrome or Edge 137+**. Both JSPI and WebGPU are required; other browsers get a message saying so.

`serve.py` sends `Cross-Origin-Opener-Policy: same-origin` and `Cross-Origin-Embedder-Policy: require-corp`. With
those headers the page is cross-origin isolated and audio uses a SharedArrayBuffer ring. Without them, for example
with any other static server, everything still works, and audio goes through `postMessage` instead.

First run:
1. Click **Choose Terraria folder** and pick your own install, the folder that holds `Terraria.exe` and `Content`.
2. The page checks the folder.
3. It copies `Content` (to OPFS `/install/Content`) and `Terraria.exe` (to `/install/Terraria.exe`, for the game's embedded resources; it must start with `MZ`) into the browser's private storage, showing bytes and files copied. No other `.exe` / `.dll` is ever opened or copied.

Nothing is uploaded. The copy is resumable: files already there with the same size are skipped.

Header controls:
- **Saves**: download a character (`.plr`) or world (`.wld`) from `/saves`, or import one.
- **Re-import game files**: stop the game and show the picker again.
- **Fullscreen**: also Alt+Enter.
- **Log**: show or hide the log panel.

To run the real game, put `game.wasm` next to `index.html`.

## Files

| File | What it is |
|---|---|
| `index.html`, `main.js` | The page: feature checks (JSPI, WebGPU, OPFS), first-run import, input capture, gamepad polling, clipboard, save export / import, fullscreen, AudioContext / AudioWorklet setup, log panel |
| `worker.js` | The game Worker: WebGPU set-up, then `game.wasm` (both import modules, JSPI wrapping, import check, `_initialize`) or the mock; frame driving; save file work |
| `vfs.js` | The file tree shared by `tw` and WASI: OPFS index, case-insensitive lookup, write guard, shared sync access handles |
| `host.js` | The `tw` imports |
| `wasi.js` | `wasi_snapshot_preview1` over `vfs.js` (preopens 3 = `/install`, 4 = `/saves`) |
| `gfx.js` | The WebGPU replayer of FRAME_FORMAT.md (rules from `render.rs`), presentation, `snapshot()` |
| `audio-worklet.js` | The AudioWorklet processor (44,100 Hz stereo) |
| `lib/paths.js` | Path parsing, the `/install` write guard, WASI errno values |
| `lib/input-codec.js` | The 32-byte input records, key code table, input queue |
| `lib/import-plan.js` | Install validation and the resumable-copy decision |
| `lib/audio-ring.js` | The SharedArrayBuffer ring and the chunk-queue fallback |
| `lib/frame-codec.js` | Frame parser and builder (FRAME_FORMAT.md) |
| `lib/gfx-rules.js` | `render.rs`'s blend / sampler / primitive / clamp rules as WebGPU values |
| `serve.py` | Local test server |
| `test/*.test.mjs` | Node unit tests (`fake-opfs.mjs` is the in-memory OPFS they use) |
| `test/gfx-replay.html`, `test/gfx-replay.js` | WebGPU replay test: a JS-built frame against expected pixels computed in JS |
| `test/run-gfx-headless.mjs` | Runs the replay test, and with `--page` the page itself, in a hidden headless Chromium |

## Tests

```
cd web/www
for f in *.js lib/*.js test/*.mjs test/*.js; do node --check "$f"; done
node --test test/*.test.mjs                 # or: npm test   (nothing to install)
node test/run-gfx-headless.mjs --page       # WebGPU replay + the page; hidden headless Chromium
```

`run-gfx-headless.mjs` uses software that is already on this machine, and downloads nothing:
- `playwright-core` from the npx cache;
- Playwright's full Chromium (`ms-playwright/chromium-*/chrome-win64/chrome.exe`) in new headless mode, first with `--use-webgpu-adapter=swiftshader` and then with the default adapter. The headless-shell build has no WebGPU adapter.

Override the paths with `PLAYWRIGHT_CORE=` and `CHROME=`. `SCREENSHOT=<file.png>` saves the page.

### Developer tools (not for players)

**`serve.py --install <Terraria folder>` with `?devimport`.** This replaces the folder picker so automated runs can import without a person.
- **Off by default.** Without the flag, the `/__devinstall/` URLs return 404.
- **What the server serves.** It binds to 127.0.0.1 and serves `Terraria.exe` from the top of the folder and the files under `Content/`, read-only. No other program file is ever opened, at the top or inside `Content`: not `OnlineFix.dll`, not `steam_api.dll`.
- **What the page does with it.** It validates the install as it would a picked folder, then copies through the same code into OPFS. The copy is resumable. The game then starts.

**`?saves=<name>`.** `/saves` becomes the OPFS folder `saves-<name>`, so a scripted run starts with no players or worlds. `run-game-headless.mjs --actions` uses a new one each run.

**`?fixedstep`.** The time passed to `tw_frame` advances exactly 1/60 s per frame, like the native shell's fixed step. Scripted runs need it: pausing between frames would otherwise turn into catch-up updates, and a click pressed and checked across two updates is lost. `--actions` sets it.

**`?mock`.** Runs the JS mock module even when `game.wasm` is present (the `?selftest` check uses it).

**`?trace[=N]`.** Records the last N import calls (`tw` and WASI, with their arguments and results). When the module fails, the error report adds them, along with the calls still in progress.

**`window.__twLog` and `window.__twDev`.** These are hooks for automation:
- `window.__twLog` holds every log line.
- `window.__twDev.events` records the import, compile, `_initialize` and `tw_init` timings, plus errors and exit.
- `window.__twDev.request('stats' | 'snapshot')` returns the frame count, or the back buffer as RGBA.

**`node test/run-game-headless.mjs [--frames 600] [--trace 64] [--install <dir>]`.** Runs the real `game.wasm` in a hidden headless Chromium.
- **What it does.**
  - Starts `serve.py --install`.
  - Opens `?devimport&autostart` in a browser profile of its own at `<project>/work/web-profile`, never the user's Chrome profile.
  - Saves the back buffer as `<project>/work/shots/web_title_<n>.png` every 60 frames, plus `web_title_final.png`.
  - Writes every log line to `<project>/work/logs/web_game_run.txt`.
  - Reports the timings and the first error.
- **Resumable import.** The OPFS copy of the install lives only in that profile. A second run copies nothing.
- **GPU.** `GPU=swiftshader` uses the software WebGPU adapter instead of the machine's own.
- **Scripted input.** `--actions <file>` replays input at exact frames, using `?fixedstep` and a fresh `?saves`. The format is documented at the top of the runner.
  - Lines are `frame N move|click|key|type|shot|diag`.
  - `diag <name>` records the next frame in the replayer: every texture it draws into goes to `work/shots/diag_target_<id>.png`, and the draw statistics go to `work/logs/<name>.json`.
  - Scripts: `test/actions/single_player.txt`, and `test/actions/single_player_fast.txt` (the same sequence with short waits).

The `?selftest` URL builds a tiny fake install inside OPFS, imports it twice (the second time must skip every file), and starts the mock. It refuses to run when the browser already holds a real import.

## Notes for the Rust side

- **Graphics.** Frames go out through `tw.gfx_frame` in the FRAME_FORMAT.md encoding.
  - The replayer follows `render.rs`: pass grouping, discard on bind, the upload split, clamped viewport / scissor, blend and sampler mapping, cull rule, null texture, pipeline cache.
  - The headless test checks those rules pixel by pixel on SwiftShader and on an NVIDIA adapter.
- **Shader errors.** A DefineShader whose WGSL does not compile is logged once with line and column. Pipelines are created synchronously, so a broken shader can invalidate that frame's command buffer: the frame is lost, and the error is logged. Translate and validate shaders on the Rust side (naga) before emitting them.
- **Audio before the first click** is accepted and dropped at the real-time rate (HOST_ABI.md section 2, "Audio").
- **Exit.** Prefer `proc_exit` to `tw.exit`.
- **`sched_yield` is not a frame yield.** Fibers and loading screens use `tw.yield_frame`.
- **`poll_oneoff` never sleeps.** A `thread::sleep` returns at once.

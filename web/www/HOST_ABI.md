# Host ABI: the contract between `game.wasm` and the web shell

Version 2 (2026-10-13), plus the `net_*` imports (section 2, "Network"; added 2026-10-13, stage 4), and
(2026-10-16) `/install` over HTTP (section 2, "Install over HTTP") and the `window_title` / `frame_info` imports
(section 2, "Window").

This file is the contract the Rust side builds against. Each part has a JS implementation and tests:

| Part | Implementation | Tests |
|---|---|---|
| `tw` imports | `host.js` | `test/host-fs.test.mjs` |
| `wasi_snapshot_preview1` | `wasi.js` | `test/wasi.test.mjs` |
| The file tree both use | `vfs.js` | the two above |
| Loading and frame driving | `worker.js` | the page run in `test/run-gfx-headless.mjs --page` |
| Frame replay | `gfx.js` | `test/frame-codec.test.mjs`, `test/gfx-replay.js` |
| Input records | `lib/input-codec.js` | `test/input-codec.test.mjs` |
| Network (`net_*`) | `lib/net-bridge.js` | `test/net-bridge.test.mjs`; the bridge: `tools/ws-bridge/selftest.mjs` |
| `/install` over HTTP | `lib/remote-install.js`, `install-worker.js`, `web/host/terraria_host.py` | `test/remote-install.test.mjs`; the play page run in `test/run-game-headless.mjs --host --page play` |
| Window (`window_title`, `frame_info`) | `host.js` | `test/host-fs.test.mjs` |

The frame bytes have their own contract, [FRAME_FORMAT.md](FRAME_FORMAT.md).

**Changes from version 1:**
- **The module.** It is a `wasm32-wasip1` reactor. It imports `wasi_snapshot_preview1` as well as `tw`, and it may export `_initialize`. wasm-bindgen is not used.
- **Graphics.** The module no longer renders. It hands each frame to `tw.gfx_frame`, and the Worker replays it with WebGPU. `self.twCanvas` is gone.
  - WebGPU is required. A page without it gets a message.
  - The device and the canvas context are set up before `tw_init`.
- **New `tw` imports:** `gfx_frame`, `gfx_frame_chunk` and `gfx_texture` (FRAME_FORMAT v2), `cpu_count`, `clipboard_get`, `clipboard_set`.
- **Audio before the first click.** `audio_free` / `audio_push` now accept frames and drop them at the real-time rate. In version 1 they returned 0.
- **Wheel deltas** stay in CSS pixels. The Rust side converts them to XNA's 120 per notch.
- **Folder rename** stays `ENOTSUP` in both `tw` and WASI.
- **Page controls.** The page can export and import saves (section 7).

Rules for every function:
- **Numbers only.** `i32`, `i64` (a JS `BigInt`), `f64`, and byte ranges `(ptr: i32, len: i32)` in the module's exported `memory`.
- **Unsigned pointers.** Pointers and lengths are read as unsigned 32-bit values, so addresses above 2 GiB work.
- **Strings** are UTF-8 and not NUL-terminated.
- **Errors.** The `tw` imports return a negative errno. The WASI functions return the errno itself, as preview1 does. Both use the **WASI preview1 numbering** (section 6).
- **Bad ranges.** A range outside `memory` gives `EFAULT` (21) and touches nothing.
- **Memory growth.** The host re-reads `memory.buffer` on every call, so `memory.grow` between calls is safe.

## 1. Module shape and start-up

The module is a raw WebAssembly module built for `wasm32-wasip1` as a **reactor**, not a command:
- Imports come only from `tw` and `wasi_snapshot_preview1`. Before instantiating, the Worker checks `WebAssembly.Module.imports`. Any other module, or a name not listed here, is refused with an error that names it.
- **Exports:**

| Export | Signature | JSPI | Meaning |
|---|---|---|---|
| `memory` | `WebAssembly.Memory` | | All `(ptr, len)` ranges refer to this memory. It may be shared (threads) or not. |
| `_initialize` | `() -> ()` | promising | Optional. The WASI reactor entry, which runs static constructors. Called once, before `tw_init`. It may suspend. A `_start` export is ignored, with a warning in the log. |
| `tw_init` | `(width: i32, height: i32) -> i32` | promising | Called once. `width` / `height` are the canvas size in device pixels. Return `0` for success. Anything else is shown to the player as `tw_init returned N`, and the game stops. It may suspend, for example for a loading screen. |
| `tw_frame` | `(now_ms: f64) -> ()` | promising | One display frame. `now_ms` is the Worker's `performance.now()`. Returns when the frame is done, and it may suspend. The return value, if any, is ignored. |
| `tw_fiber_entry` | `(handle: i32) -> ()` | promising | Optional, with `__stack_pointer`. The body of a work fiber (section "Fibers"). Called by `tw.fiber_resume` only. |
| `__stack_pointer` | `WebAssembly.Global` (mutable i32) | | Optional, with `tw_fiber_entry`. The shadow-stack pointer; the Worker swaps it at every fiber switch. |
| `tw_server_init` | `() -> i32` | promising | The dedicated server (section "Server"), instead of `tw_init`, in the server Worker's own instance. |
| `tw_server_tick` | `(now_ms: f64) -> f64` | promising | One poll of the server's main loop; returns the milliseconds to wait before the next call, or `-1 - exit code` once it ended. |

**Start-up order in the Worker:**
1. **WebGPU.** Request the adapter and device, configure the `OffscreenCanvas` context and build the frame replayer. Without WebGPU the page shows "This build needs Chrome or Edge 137+" and stops.
2. **OPFS.** Index the OPFS tree under `/install` and `/saves`.
3. **Instantiate.** Compile and instantiate `game.wasm` with both import objects. The suspending imports are wrapped in `WebAssembly.Suspending`. If `game.wasm` is absent, the JS mock is used instead.
4. **`_initialize`.** Called if exported.
5. **Ticks start.** Before `tw_init` returns, a tick only resumes a suspended `yield_frame`.
6. **`tw_init`.**
7. **`tw_frame`** on every tick.

**Exit:**
- `proc_exit(code)` throws a `WasiExit`. The Worker catches it from `_initialize`, `tw_init` or `tw_frame`, flushes stdout / stderr, and shows "The game has closed (exit code N)".
- `tw.exit(code)` followed by a trap is the other way out, and is treated the same way.

### Frame driving

The Worker's own `requestAnimationFrame` runs the tick, because the canvas is an `OffscreenCanvas` owned by the Worker.
If the Worker has no `requestAnimationFrame`, the page relays one `tick` message per display frame. On each tick:
1. If the module is suspended in `yield_frame`, that call returns with the tick's `now_ms`. Nothing else happens on this tick.
2. Else, if no `tw_frame` is still running, the Worker calls `tw_frame(now_ms)`.
3. Else the tick is skipped, for example while a frame waits for a file open.

Rule: there is never more than one `tw_init` / `tw_frame` running. The module can still run its own fibers inside a
frame, as in `web/spikes/fiber-jspi/sched.mjs`.

### JSPI

Suspending imports:

| Import module | Suspending functions |
|---|---|
| `tw` | `fs_open`, `fs_mkdir`, `fs_remove`, `fs_rename`, `yield_frame`, `fiber_resume`, `fiber_suspend` |
| `wasi_snapshot_preview1` | `path_open`, `path_filestat_get`, `path_create_directory`, `path_remove_directory`, `path_unlink_file`, `path_rename` |

Rules:
- The Worker wraps each of these in `new WebAssembly.Suspending(fn)`. To the module they look synchronous.
- They may only be called while a promising export (`_initialize`, `tw_init`, `tw_frame`, `tw_fiber_entry`) is on the stack. Called from anywhere else, JSPI traps.
- One returns without suspending when its answer is ready. Examples: a file that another descriptor already holds open, a cached file size, or an error found before any OPFS call.
- When one suspends, it suspends only the stack that entered through the innermost promising export. A work fiber that opens a file therefore suspends only that fiber.
- Every other import is synchronous.

### Fibers

Queued work of the game (world generation, world load, save: `Thread` / `ThreadPool` / `Task` work) runs on a fiber:
an engine stack of its own plus a shadow stack of its own in linear memory (`terraria-host` `threading::wasm_fibers`;
JS side `lib/fibers.js`; design: `web/WEB_PORT.md` section 1).

| Import | Signature | Suspends | Meaning |
|---|---|---|---|
| `fiber_resume` | `(handle: i32, stack_top: i32) -> i32` | yes | Main stack only. `stack_top` non-zero: start fiber `handle`: `__stack_pointer` = `stack_top`, then `WebAssembly.promising(tw_fiber_entry)(handle)`. `stack_top` 0: continue the fiber where it called `fiber_suspend`, with its saved `__stack_pointer`. Returns when the fiber suspends (`0`), ends (`1`) or traps (`2`, logged; its stack is gone), with the main stack's `__stack_pointer` back. |
| `fiber_suspend` | `(handle: i32) -> ()` | yes | Work fiber only. Saves its `__stack_pointer` and hands control back to the main stack's `fiber_resume`. Returns when resumed. |
| `fiber_forget` | `(handle: i32) -> ()` | no | The fiber will never be resumed (`Thread.Abort`). |

- All hand-overs happen inside one `tw_frame` (microtasks); one wasm stack runs at a time. A fiber may also suspend in
  a file import; the main stack then stays suspended in `fiber_resume` until the fiber suspends or ends.
- The module decides when a fiber gives the frame back (a time slice: `YieldMode::Millis(8)` in terraria-web).
- While a fiber is suspended its frames are not in linear memory (they cannot be scanned): the collector does not end
  a mark phase then (`web/WEB_PORT.md` section 2.2).

## 2. `tw` imports

### Files

The host has one virtual tree (`vfs.js`) with two roots, both in the browser's Origin Private File System (OPFS):

| Root | Contents | Access |
|---|---|---|
| `/install/...` | Copied from the player's own install at first run. The game sees `/install/Content/...` and `/install/Terraria.exe` (see below this table). | Read-only |
| `/saves/...` | The save root (`Main.SavePath`). | Read-write |

What `/install` holds:
- `/install/Content/...`: the install's `Content` folder.
- `/install/Terraria.exe`: the player's own executable. The game reads its embedded resources from it with `rt::resources::install_from_assembly("/install/Terraria.exe")`.
  - The importer requires it in the picked folder, checks that it starts with `MZ`, and copies it resumably like the rest.
- Nothing else. No other `.exe` / `.dll` is ever opened or copied: not `OnlineFix.dll`, not `steam_api.dll`, not anything inside `Content`.
- Imports made before the exe was copied are completed by choosing the folder again.

`tw` and WASI share this tree:
- The same index serves both, with the same lookup, the same write guard and the same OPFS sync access handles.
- A file opened through WASI and through `tw` at the same time shares one handle.

How `tw` paths are parsed:
- Paths are absolute.
- `/` and `\` are both separators. Repeated and trailing separators are ignored.
- `.` and `..` components are refused with `EINVAL`, and so is NUL.
- A component longer than 255 bytes, or a path longer than 4096, gets `ENAMETOOLONG`.
- A first component other than `install` / `saves` gets `ENOENT`. The root names are exact (lower case).

Lookup is case-insensitive below the roots. An exact match wins. Otherwise the first name that matches
case-insensitively is used, as on Windows. A file created through a case-different path reuses the existing entry.

**Write guard.** Every create, truncate, append, mkdir, remove or rename whose path, source or destination is under
`/install`, or is `/`, `/install` or `/saves` itself, returns `EACCES` (2). The guard runs before any OPFS call.

**The index:**
- `fs_stat` and `fs_list` (and WASI `fd_readdir`) answer from an in-memory index of names and kinds.
- The Worker builds it once before `_initialize`.
- Every successful mkdir, create, remove and rename updates it.
- File sizes and modification times are cached on first use, and updated by every write through a handle that is then released.
- Changes made to OPFS by anything else are not seen. While the game runs, only the page's save import writes there, and it goes through the same tree.

| Import | Signature | Suspends | Behaviour |
|---|---|---|---|
| `fs_open` | `(path_ptr, path_len, mode: i32) -> i32` | yes | Open a file and return a handle (>= 1) or `-errno`. See the open modes below this table. |
| `fs_size` | `(h: i32) -> f64` | no | Current size in bytes, or `-EBADF`. |
| `fs_read` | `(h: i32, offset: f64, ptr, len) -> i32` | no | Read up to `len` bytes at `offset`. Returns bytes read: `0` at or past the end, short at the end. `-EINVAL` if `offset` is negative or not finite. `len` is capped at 2^31-1. |
| `fs_write` | `(h: i32, offset: f64, ptr, len) -> i32` | no | Write `len` bytes at `offset`, growing the file if needed. In append mode `offset` is ignored and the write goes to the end. Returns bytes written. `-EBADF` on a read handle, `-ENOSPC` when the quota is full. |
| `fs_close` | `(h: i32) -> i32` | no | Flush (if written) and close. `0` or `-EBADF`. |
| `fs_stat` | `(path_ptr, path_len) -> i32` | no | `-1` missing (or an invalid path), `0` file, `1` directory. |
| `fs_list` | `(path_ptr, path_len, out_ptr, out_cap) -> i32` | no | Entries, sorted by code unit, joined by `\n`, no trailing newline. Directory names end with `/`. See the return values below this table. |
| `fs_mkdir` | `(path_ptr, path_len) -> i32` | yes | Create the directory and any missing parents. `0` if it now exists, including when it already did. `-ENOTDIR` if a component is a file. |
| `fs_remove` | `(path_ptr, path_len) -> i32` | yes | Remove a file or an empty directory. `-ENOTEMPTY`, `-EBUSY` (open file), `-ENOENT`. |
| `fs_rename` | `(from_ptr, from_len, to_ptr, to_len) -> i32` | yes | Move or rename a **file**, replacing an existing destination file. See the errors below this table. |

`fs_open` modes:

| `mode` | Meaning | When the file is missing |
|---|---|---|
| `0` | read | `-ENOENT` |
| `1` | write: create or truncate | Created. The parent directory must exist (`-ENOENT`). |
| `2` | append: create if missing, every write goes to the end | Created. The parent directory must exist (`-ENOENT`). |

Other `fs_open` results:
- Any other mode: `-EINVAL`.
- The path is a directory: `-EISDIR`.
- More than 1024 handles are open: `-EMFILE`.
- Another tab holds the file: `-EBUSY`.

`fs_list` return values, for directory names and errors:

| Result | Meaning |
|---|---|
| `0` or more | Bytes written. `0` is an empty directory. |
| `-1` to `-255` | `-errno`: `-ENOENT` (missing), `-ENOTDIR` (a file). |
| `-256` or less | The buffer is too small and nothing was written. Call again with `out_cap >= -result`. A needed size under 256 is reported as 256, so this can never be mistaken for an errno. |

`fs_rename` errors:
- Directories: `-ENOTSUP`.
- The source or the destination is open: `-EBUSY`.
- The destination is a directory: `-EISDIR`.

### Install over HTTP

The play page (`play.html`, served at `/` by `web/host/terraria_host.py`) has no import step: the game Worker is
started with `install: 'http'` and `/install` comes from that server, read-only, as before. The developer page uses it
with `?httpinstall` (served at `/dev.html` by the same server); otherwise it keeps the OPFS copy of the import.

What the server offers (all on 127.0.0.1; cross-origin isolation headers on every response):

| URL | What | Caching |
|---|---|---|
| `install/manifest.json` | `{ version: 1, pack: { id, size, url, files }, files: [{ p: path, s: size, m: mtime ms, h: id, o: offset in the pack or -1 }] }`. `p` is relative to `/install` ("Terraria.exe", "Content/..."). The id is BLAKE2b-128 of path, size and mtime (`--content-hash`: of the bytes). | ETag, `no-cache` |
| `install/pack/<pack id>` | Every file of at most 32 MB, concatenated: first in the order the game opens them at start-up (`web/host/pack-order.txt`), then by path. Byte ranges. The server keeps it as one file (`web/host/cache/pack-<id>.bin`, made once in the background): one open instead of 16,000 (about 7 ms each the first time, the on-access scan). | immutable |
| `install/file/<id>` | One file by id: the big ones (`Content/Wave Bank.xwb`, 496 MB). Byte ranges. | immutable |
| `game.wasm` and the page's files | gzip-compressed once for game.wasm (cached by size + mtime; `br` when the optional `brotli` module exists). | ETag, `no-cache` (so the HTTP cache and V8's compiled-code cache are reused) |

Only `Terraria.exe` at the top of the install and the files under `Content/` are listed or opened; no other program
file (`.exe`, `.dll`, ...) is ever opened.

In the Worker (`lib/remote-install.js`, used by `vfs.js` through `createVfs(root, { install })`):
- **The tree.** `/install` is built from the manifest: names, kinds, sizes and mtimes. `fs_stat`, `fs_list`,
  `path_filestat_get`, `fd_readdir` never touch the network. Lookup and the write guard are unchanged.
- **The cache.** OPFS `tw-cache/`: `pack-<id>.bin` (the pack), `pack-<id>.json` (`{ done }`: how far it is written),
  `big-<id>.bin` + `big-<id>.map` (a big file and one byte per 1 MB block it holds). Files of another pack or big id are
  removed at start (a game update).
- **The download.** A Worker of its own (`install-worker.js`; the game Worker's event loop turns only between the
  game's slices, so a download there got one network chunk per slice: 258 MB in 114 s). It writes the pack from
  `done` on with its own `readwrite-unsafe` access handle and publishes the bytes written in a `BigInt64Array` on a
  `SharedArrayBuffer` (index 0; index 1 = 1 once it ended; `Atomics.notify` on every chunk), saves `done` every 16 MB,
  then fills the big files' missing blocks.
- **An open** (`fs_open`, `path_open`: they may suspend):
  - a packed file the download has passed: synchronous, a view of the one pack access handle;
  - a packed file the download reaches within about a second (by its speed so far), or within 2 MB: waits for it
    synchronously (`Atomics.wait`, at most 1.5 s), then as above;
  - otherwise: that file's bytes alone, `fetch` with a Range header (the open suspends), kept in memory until closed;
  - a big file: an access handle whose reads (synchronous, `fd_read`) take missing 1 MB blocks with a synchronous
    `XMLHttpRequest` (allowed in Workers; a few ms per block on the loopback), write them to `big-<id>.bin` and mark
    them in the map.
- **The server Worker** (Host & Play) reads the same cache through its own handles (`role: 'server'`); it never
  downloads, re-reads `pack-<id>.json` before fetching a file alone.
- **A second start**: the manifest is a 304, the pack is complete, every open is synchronous.

### Window

| Import | Signature | Behaviour |
|---|---|---|
| `window_title` | `(ptr, len) -> ()` | The game window's title changed (`Game.Window.Title`, set by the game). The play page makes it the document title. |
| `frame_info` | `(mouse_x: i32, mouse_y: i32, flags: i32) -> ()` | After every presented frame: `PlayerInput.MouseX`, `PlayerInput.MouseY` (the game's mouse in back-buffer pixels; `Main.mouseX` is that divided by the UI scale while the UI is drawn) and flags: bit 0 `IsMouseVisible` (the page hides the system cursor over the canvas when 0), bit 1 `Main.gameMenu`, bit 2 `Main.showSplash`. A change of the flags is posted to the page (`{ type: 'cursor', visible, menu, splash }`); the end of the splash is the page's `title-screen` event. |

Both are optional for the module (an older game.wasm does not import them).

**The canvas and the game's resolution.** The canvas backing size is the device pixels it covers
(`ResizeObserver` `device-pixel-content-box`), sent as `RESIZE` records; the game takes it as its window's client
size through its own `ClientSizeChanged` path (`terraria-host` `game.rs` `begin_frame`). The Worker applies the new
canvas size at the next presentation (never shown cleared). When the game keeps a resolution of its own (its 800x600
minimum, one chosen in its settings, or the frame right after a resize), the back buffer is drawn fitted, aspect
ratio kept, centred (`gfx.js` `fitRect`). Pointer positions arrive in canvas pixels; the Worker maps them into
back-buffer pixels through that same fit before the module sees them, so `PlayerInput.MouseX` / `MouseY` is the pixel
under the pointer (test: `test/actions/play_dpr.txt`, `play_world.txt`, runner action `cursorcheck`).

**Input before the game exists.** `input` messages that reach the Worker before the host and the replayer exist
(the page's first RESIZE while game.wasm compiles) are kept and applied then; `tw_init` starts from the newest
measured size. The page measures the canvas at once, again every display frame for the first 10 s and every 30th
after (ResizeObserver device-pixel box, else the layout box or client size x devicePixelRatio).

**Start-up size.** `tw_init` gets the canvas size with the height 2 px smaller, and the first frame's input carries
a `RESIZE` to the real size (worker.js), as a native window opens and is then made borderless or maximised. Started
at its final size, the game's borderless start-up (`screenBorderlessPendingResizes`, `Main.SetDisplayMode`) left part
of its state at another size: the language menu missed the first click and the title's background stopped about
75 px short of the right edge. Only 2 px, because the default UI scale is picked from the start-up height.

### Time, randomness, logging, exit, system

| Import | Signature | Behaviour |
|---|---|---|
| `now_ms` | `() -> f64` | `performance.now()` in the Worker. Monotonic, sub-millisecond. Same clock as `tw_frame`'s argument, the input records' `time`, and WASI's monotonic clock (which is in ns). |
| `wall_ms` | `() -> f64` | `Date.now()`. |
| `random` | `(ptr, len) -> i32` | Fill the range with `crypto.getRandomValues` bytes. `0` or `-EFAULT`. |
| `log` | `(level: i32, ptr, len) -> ()` | Write a line to the console and the page's log panel. Levels: 0 trace, 1 debug, 2 info, 3 warn, 4 error (clamped). WASI stdout lands here at level 2, stderr at level 3. |
| `exit` | `(code: i32) -> ()` | Records the exit code and tells the page. It **returns**: the module should trap right after (`unreachable`). Prefer WASI `proc_exit`, which does not return. |
| `cpu_count` | `() -> i32` | `navigator.hardwareConcurrency`, at least 1. For `Environment.ProcessorCount`. Threads are not available; only the number is. |

### Clipboard

| Import | Signature | Behaviour |
|---|---|---|
| `clipboard_get` | `(out_ptr, cap) -> i32` | The text of the last paste the page saw, as UTF-8 (may be empty). Returns the bytes written. `-max(needed, 256)` if `cap` is too small, as with `fs_list`. |
| `clipboard_set` | `(ptr, len) -> i32` | Remember `text` as the clipboard. It is what `clipboard_get` returns from then on. Returns `0` or `-EFAULT`. |

`clipboard_get`:
- The page listens for the browser's `paste` event (Ctrl+V / Cmd+V, which it does not block) and forwards the text.
- So the usual pattern works: the game sees the Ctrl+V `KEY` record and then reads `clipboard_get`. The paste event arrives in the same input batch or just before it.

`clipboard_set`:
- The page writes the text to the system clipboard with `navigator.clipboard.writeText` on the player's **next click or key press**. Browsers allow it only during a user gesture.
- It is best effort: a refusal is only logged.

### Input

| Import | Signature | Behaviour |
|---|---|---|
| `input_next` | `(out_ptr, out_cap) -> i32` | Move as many whole 32-byte records as fit into `out_cap` bytes. Returns bytes written (a multiple of 32), `0` when empty, `-EINVAL` when records wait but `out_cap < 32`. Call it until it returns 0. |

The queue holds up to 4096 records:
- When it is full, the oldest records are dropped and the next drain starts with an `OVERFLOW` record.
- Consecutive `MOUSE_MOVE` records with the same buttons and mods are coalesced, and the newest wins.

### Audio

| Import | Signature | Behaviour |
|---|---|---|
| `audio_free` | `() -> i32` | How many frames the module should push now, keeping about 4096 frames (93 ms) buffered. |
| `audio_push` | `(ptr, frames) -> i32` | Queue `frames` stereo frames: `frames * 2` little-endian `f32`, interleaved L R, at **44,100 Hz**, range -1..1. `ptr` must be 4-byte aligned (`-EINVAL`). Returns the frames accepted, fewer when the buffer is full. |

**Before the first click or key press** (the browser autoplay rule):
- No `AudioContext` exists yet.
- `audio_push` accepts frames and drops them at the real-time rate: a virtual 4096-frame buffer drains at 44,100 frames per second.
- `audio_free` reports its free space.
- So the mixer's clock, and the position in the music, keep running, and nothing is heard.
- This is "accept and drop" paced to real time. Reporting the full capacity on every call would let the mixer run about 5.5 times faster than real time at 60 fps.

**Transport:**
- **Cross-origin isolated page:** a SharedArrayBuffer ring of 16384 frames, read by `audio-worklet.js`.
- **Otherwise:** transferable chunks posted to the worklet.

The module sees no difference.

### Graphics

| Import | Signature | Behaviour |
|---|---|---|
| `gfx_frame_chunk` | `(ptr, len) -> ()` | FRAME_FORMAT v2: append a part of the frame being received. The bytes are copied out of wasm memory at once. Cut only at command boundaries. |
| `gfx_texture` | `(id, width, height, ptr, len) -> ()` | Create texture `id` **now**, outside any frame: `rgba8unorm`, usage TEXTURE_BINDING, COPY_DST and COPY_SRC, `width` x `height`. Write the RGBA rows from `(ptr, len)`, where `len` must be `width * height * 4` (`-EINVAL`). The bytes are copied at once. The texture is marked preloaded. See the rule below this table. |
| `gfx_frame` | `(ptr, len) -> ()` | Append the last part (all of a one-part frame) and replay the frame in the [FRAME_FORMAT.md](FRAME_FORMAT.md) encoding. Once per presented frame (after `Game.EndDraw`). See the replay rules below this table. |

The preload rule for `gfx_texture`:
- When a frame's CreateTexture arrives for a preloaded id with the same size, and not as a render target, the texture and its contents are kept, and the mark is cleared.
- Otherwise the texture is recreated as usual (transparent black), and the mismatch is logged once.
- The module only preloads textures that the same frame creates and no draw has used yet, so applying them early keeps the order.
- **Why.** The title's content load no longer carries hundreds of MB of Upload commands in one frame.

How `gfx_frame` replays a frame:
- The Worker replays it synchronously with WebGPU (`gfx.js`) and presents texture 0 on the canvas.
- The bytes are only read during the call; a frame in shared memory is copied first.
- A frame that does not parse is logged once and skipped.
- A frame whose `serial` is not larger than the last one replayed is ignored.
- WebGPU validation errors are logged once each.
- If no WebGPU replayer exists, the call returns `-ENOSYS`. That cannot happen in the real start-up, where WebGPU comes first.

Presentation:
- Texture 0 is drawn over the whole canvas, nearest texel, with alpha forced to 1.
- The canvas size follows the `RESIZE` records: the host sets `width` / `height` before it queues the record. Texture 0 keeps the size the frame header gives it, so the game decides its back buffer size.
- The canvas format is `navigator.gpu.getPreferredCanvasFormat()`, `bgra8unorm` on Windows. Presentation is a render pass, which writes logical RGBA in any format, so the colours shown equal the bytes in texture 0.
- `test/gfx-replay.js` checks this: it compares the canvas pixels with the back buffer.

### Network

The game's `TcpClient` connections (Join via IP; terraria-host `sys::net::wasm_net`). A page cannot open TCP, so each
connection is a WebSocket to a bridge on the player's own computer, `tools/ws-bridge` (`python
tools/ws-bridge/ws_bridge.py`, 127.0.0.1 only), which opens the TCP connection the game asked for. The game still
types an address and a port; where the bridge listens is the page's business: `?bridge=ws://127.0.0.1:7780`, else the
header's "Server bridge" field (remembered), else `ws://127.0.0.1:7780`. A change of the field reaches the Worker as a
`bridge` message and applies to the next connection.

| Import | Signature | Behaviour |
|---|---|---|
| `net_connect` | `(host_ptr, host_len, port: i32) -> i32` | Start a connection to `host` (UTF-8 IP address text) : `port` through the bridge. A handle (>= 1) at once, or `-EINVAL` (port, empty host), `-EFAULT`, `-ENOSYS` (no bridge set). Never blocks. |
| `net_status` | `(h) -> i32` | `0` connecting, `1` connected, `2` the server closed (received bytes can still be read), `-ECONNREFUSED` (14: the bridge refused, could not connect, or is not running), `-ECONNRESET` (15: the connection broke), `-EBADF`. |
| `net_send` | `(h, ptr, len) -> i32` | Queue `len` bytes (copied at once): returns `len`. `-EAGAIN` (6) while connecting, `-EPIPE` (64) after the server closed, the failure's errno after a failure. |
| `net_recv` | `(h, ptr, cap, peek: i32) -> i32` | Copy up to `cap` received bytes, across WebSocket messages, and remove them unless `peek` is 1. `-EAGAIN` when nothing has arrived and the connection is open or connecting; `0` at the end of the stream (closed and drained); the failure's errno after a failure. |
| `net_close` | `(h) -> i32` | Close the WebSocket (code 1000) and forget the handle. `0` or `-EBADF`. |

- None of them suspends. The module polls and gives the frame back in between (the game's network threads are work
  fibers that sleep between packets; `TcpClient.Connect` waits with a fiber suspend), so the WebSocket's events arrive
  between frames.
- The bridge's contract: `<bridge>/?host=<address>&port=<n>`; the TCP connection is made before the WebSocket
  handshake is answered, so "open" means "connected" and a refused connection is a failed handshake (HTTP 502). Binary
  messages both ways; a WebSocket close from the bridge is the end of the TCP stream. Targets: loopback addresses only
  (`--allow-lan`: private LAN addresses too; public addresses never); page origins: loopback pages only (`--origin`).
- Not on the web: the LAN broadcast (the server's `UdpClient.Send` sends nothing).

Listening and the page's own server (Host & Play, section "Server"):

| Import | Signature | Behaviour |
|---|---|---|
| `net_listen` | `(port: i32) -> i32` | Server Worker: a listener of the page on `port` (`TcpListener.Start`). A listener handle, `-EADDRINUSE` (3), or `-ENOSYS` in the game Worker. The server Worker then registers with the bridge in reverse mode (below). |
| `net_accept` | `(l) -> i32` | A connection that arrived for listener `l`: a connection handle, already connected (`net_status` 1), used with `net_send` / `net_recv` / `net_close`. `-EAGAIN` when none is waiting. Never blocks. |
| `net_unlisten` | `(l) -> i32` | Stop the listener (connections not yet accepted are closed). |
| `net_peer` | `(h, out_ptr, cap) -> i32` | The remote end of a connection as "address:port" text (the bridge's TCP peer); length `0` for the hosting client of the page. |
| `server_start` | `(args_ptr, args_len) -> i32` | Game Worker (`Process.Start` of `TerrariaServer.exe` in Host & Play): start the page's dedicated server Worker with these launch parameters (UTF-8, as the game wrote them). An id. From now on a connection of the game to a loopback address and the server's port (`-port`, else 7777) goes to that server through the page, not the bridge. |
| `server_status` | `(id) -> i32` | `0` the server runs (or is starting), `1` it ended (`Process.HasExited`), `-EBADF`. |
| `server_kill` | `(id) -> i32` | End that server at once (`Process.Kill`). |

- In-page connections are a `MessageChannel`: the game Worker keeps one end, the page hands the other to the server
  Worker once it listens (until then the connect waits, `net_status` 0). Binary messages are bytes; `{t: 'open'}`
  (accepted), `{t: 'refused'}`, `{t: 'close'}` (end of stream).
- Reverse mode of the bridge (`ws_bridge.py --listen <tcp port> --to-page`): the server Worker opens
  `<bridge>/?listen=<port>` (a control connection, retried every 5 s while listening); for each TCP connection to the
  bridge's TCP port the bridge sends a text message `{"conn": <id>, "peer": "<address>:<port>"}`, the server Worker
  opens `<bridge>/?accept=<id>` and hands that WebSocket to the listener (`net_peer` = the TCP peer).

### Server

Host & Play runs the game's dedicated server in the page (terraria-web `server.rs`; the native one is
`terraria.exe -server`): `server_start` -> the page starts `server-worker.js`, a module Worker with its own instance
of game.wasm (the game Worker's compiled `WebAssembly.Module` is passed along, no second compile) over the same OPFS
files, and no canvas, input or sound (those imports exist and are never called).
1. The launch parameters are the WASI arguments: `argv = ["terraria-server", "<the game's argument string>"]`.
2. `tw_server_init()`: what the native server does up to its main loop (`Main.dedServ = true`,
   `Program.LaunchGame` / `RunGame`). `0` started; `1` no readable Terraria.exe; `2` the game could not be created;
   `3` called twice or in a client instance.
3. `tw_server_tick(now)` in a loop: one poll of `Main.DedServ`'s resumable form (a state machine made by the
   translator, tools/cs2rs DESIGN.md 8f) up to its next `Thread.Sleep(ms)`; the Worker waits `ms` (a timer) and calls
   again. Inside a long poll (the world load) the main stack gives the page its turn at its yield points: the server
   Worker's `yield_frame` resolves on a timer of 0 ms. A negative result is the end (`-1 - exit code`): the Worker
   reports `exit` and the page ends it. Host & Play passes `-autoshutdown`: the server ends when its host leaves.
4. The server's threads (`Netplay.ServerLoop`, `BroadcastThread`, `TcpSocket.ListenLoop`) are resumable too and park
   between their sleeps / accepts, so the collector's mark phases end while the server runs.
5. Both Workers use the same `/saves` (OPFS), each with its own index (vfs.js). Every change under `/saves` is announced
   on the BroadcastChannel `tw-vfs:<saves folder>` (`{type: 'vfs-changed', dirs, written}`): the other index marks those
   directories stale, drops cached sizes of written files and re-reads the directories at once; `path_open`,
   `path_filestat_get` and `fs_open` (suspending) wait for such a re-read and re-read the parent on a miss. Stat and
   listing stay synchronous (they answer from the index). The server's world save is in the game's world list
   afterwards without a reload.
6. A long poll (the world load: `while (!task.IsCompleted)` on the main stack, the load on a work fiber) gives the
   page its turn every 50 ms: after a fiber slice the main stack's next yield point reads the clock at once.

Messages (page <-> server Worker): `start {id, args, module, saves, bridge}`, `incoming {port, channel, peer}`,
`stop`, `console {line}`; back: `log` (lines start with `[server]`), `console-out {id, text}`, `listening {id, port}`,
`unlisten`, `exit {id, code}`. Page <->
game Worker: `server-start {id, args, module}`, `server-kill {id}`, `local-connect {id, port, channel}`; back:
`server-listening {id, port}`, `server-exit {id, code}`.

### Server console

The dedicated server's console is the page's console box (index.html `#server-console`, shown while a server of the
page runs; main.js "Server console"). It uses the standard streams of the server Worker, no `tw` import:
- **Input = WASI fd 0.** The page posts `console {line}`; server-worker.js appends the line and a LF (CR / LF inside the
  line become blanks) to a byte queue. `fd_read(0)` returns queued bytes (up to the iovecs' size) or `EAGAIN` while
  the queue is empty; it never reports the end of input while the Worker runs.
- **Reading = the game's own input thread.** terraria-host `misc::console_read_line` (`Console.ReadLine`) reads fd 0
  raw (not `std::io::stdin()`'s buffer) and returns a line once its LF is there. The original's "Server Input
  Thread" (`Main.startDedInputCallBack`, which runs `help`, `say`, `save`, `exit` ...) and its `Main.ReadLineInput`
  have resumable forms (tools/cs2rs game.json "resumable"; `Console.ReadLine` is awaited there as
  `misc::console_read_line_async`): while no line is queued the thread's machine PARKS (no stack kept, polled again at
  the next pump of DedServ's sleeps), so the tick never blocks and the collector's mark phases still end. A command
  runs on that thread's fiber between DedServ's polls, as the original's thread runs next to the main loop.
- **Starting it.** Host & Play passes `-autoshutdown`, with which the original never starts the input thread. Here
  the page is the console, so terraria-web `server.rs` calls the translated `Main.startDedInput()` once after the
  poll in which DedServ reached its main loop (`Main.gameMenu` false, `Main.autoShutdown` set, not disconnecting);
  without `-autoshutdown` DedServ starts it itself, as in the original.
- **Output = WASI fd 1.** Every piece written to fd 1 goes to the page as it comes (`console-out {id, text}`,
  including the `: ` prompt without a line end) and is appended to `#sc-out`; the page echoes the typed line after the
  prompt as a terminal does. fd 1 also goes to the line log as before (`[server] ...`).
- `exit` ends DedServ's loop (`Netplay.Disconnect`): it saves (`Saving before exit...`), the Worker reports
  `exit {code: 0}` and the box shows `server stopped (exit code 0)` and takes no more input.

### Frames

| Import | Signature | Suspends | Behaviour |
|---|---|---|---|
| `yield_frame` | `() -> f64` | yes | Give the display frame back. Returns on the next tick with that tick's `now_ms`. Use it for multi-frame work inside one promising call, such as a loading screen. |

`game.wasm` calls `yield_frame` from the main stack when game code inside `tw_init` / `tw_frame` has run longer than
its 50 ms slice (terraria-host threading `main_yield`; the title's content loading, the first frame in a world). For
the game the frame goes on: under `?fixedstep` the Worker does NOT advance the virtual clock for such a tick, so a
scripted run does not depend on where a frame was cut. The Worker's `timing` counts the stretches between two ticks
(`sliceMax`, `slicesOver100`, `mainYields`).

## 3. `wasi_snapshot_preview1`

`wasi.js` implements preview1 over the same tree. The numbering of every flag, struct and errno is preview1's.

**Preopens:**

| fd | Preopen | Notes |
|---|---|---|
| 3 | `/install` | Read-only. Its inheriting rights have no write rights. |
| 4 | `/saves` | Read-write. |
| 5 | `/` | The whole tree. Since libc picks the longest matching preopen, it only catches paths that 3 and 4 cannot, such as Windows-style `/saves\config.json` or `\saves`. `\` becomes `/`, and a leading `\` means the root of the tree, as a rooted path does on Windows: `\saves\Worlds` relative to any fd is `/saves/Worlds`. The same guard applies: nothing directly under `/` can be created, removed or renamed. |

`fd_prestat_get(6)` returns `EBADF`, which ends libc's preopen scan.
Rust's `std::fs` finds the preopens itself, so it can use absolute paths such as `/install/Content/Images/Item_1.xnb` or `/saves/Players/x.plr`.

**Standard streams:**
- fd 0 reads as empty (at its end) in the game Worker. In the server Worker it is the server console's input (see
  "Server console" in section 2): `EAGAIN` while nothing was typed, else the typed bytes.
- fd 1 goes to the log at level 2 and fd 2 at level 3. Both are line-buffered, and a UTF-8 character split across writes is kept intact.

**Arguments and environment:**
- `args`: `["terraria"]`.
- `environ`: `TERRARIA_DIR=/install`, `TERRARIA_SAVE_DIR=/saves`, `TERRARIA_PROJECT_DIR=/saves`.

**Paths** are relative to a directory fd:
- `.` components are skipped. An existing entry gives `EEXIST` to `path_create_directory` before the write guard is checked, so `create_dir_all("/saves")` works.
- `..` may climb up to the preopen and not above it (`ENOTCAPABLE`, 76).
- An absolute path is `ENOTCAPABLE`.
- `\` is treated as `/`, because the game was written for Windows.
- Lookup is case-insensitive, as for `tw`.
- The write guard is the same: any write, create, truncate, mkdir, unlink, rmdir or rename under `/install` gives `EACCES` (2).

**`path_open`:**
- `oflags`:
  - `CREAT` (1) creates a missing file.
  - `EXCL` (4) with `CREAT` on an existing file gives `EEXIST`.
  - `TRUNC` (8) truncates.
  - `DIRECTORY` (2) on a file gives `ENOTDIR`.
- The file is opened writable exactly when `fs_rights_base` contains `FD_WRITE`.
- The rights granted are the requested ones, limited to file rights. A read-only open loses the write rights, so `fd_write` on it returns `ENOTCAPABLE`.
- Opening a directory returns a directory fd. It is `EISDIR` if `FD_WRITE` or `TRUNC` is asked.
- `fdflags` `APPEND` (1) makes every `fd_write` go to the end.
- It suspends only when an OPFS handle has to be created.

**Implemented functions:**

| Function | Notes |
|---|---|
| `args_get`, `args_sizes_get`, `environ_get`, `environ_sizes_get` | As above. |
| `clock_time_get` | Realtime (0) is `Date.now()` in ns. Monotonic (1) and the CPU-time clocks (2, 3) are `performance.now()` in ns. Any other id is `EINVAL`. |
| `clock_res_get` | 1 ms for realtime, 1 µs for the others. |
| `random_get` | `crypto.getRandomValues`. |
| `fd_read`, `fd_write`, `fd_pread`, `fd_pwrite`, `fd_seek`, `fd_tell` | On files, through the shared sync access handle. `pread` / `pwrite` do not move the position. Seeking before 0 is `EINVAL`. The standard streams give `ESPIPE` for seek / tell / pread / pwrite. |
| `fd_close` | Releases the handle; the last release flushes. |
| `fd_fdstat_get` | Filetype: 2 for the standard streams, 3 for directories, 4 for files. Also the flags and the granted rights. |
| `fd_fdstat_set_flags` | Toggles `APPEND`. |
| `fd_filestat_get` | `dev` 1, `ino` (per node, stable while the page runs), `nlink` 1, size; mtime = atime = ctime from `File.lastModified` or the last write. |
| `fd_filestat_set_size` | Truncates or extends; needs `FD_FILESTAT_SET_SIZE`. |
| `fd_sync`, `fd_datasync` | Flush. |
| `fd_prestat_get`, `fd_prestat_dir_name` | The two preopens. |
| `fd_readdir` | See below this table. |
| `path_open` | As above. |
| `path_filestat_get` | Suspends only the first time the size of a closed file is needed (`File.size`). |
| `path_create_directory` | `EEXIST`; the parent must exist (`ENOENT`). |
| `path_remove_directory` | `ENOTDIR`, `ENOTEMPTY`. |
| `path_unlink_file` | `EISDIR`; `EBUSY` while the file is open. |
| `path_rename` | Files only, replacing the destination. Directories are `ENOTSUP`. `EBUSY` while open. |
| `proc_exit` | Flushes stdout / stderr and throws `WasiExit(code)` (never returns). |
| `sched_yield` | Returns 0 at once. It is not a frame yield; use `tw.yield_frame` for that. |
| `poll_oneoff` | Never blocks. Every clock subscription fires at once (error 0). Every fd subscription gets an event with error `ENOTSUP`. `nevents = nsubscriptions`. |

`fd_readdir`:
- Entries come from the index: `.` and `..` first, then the directory's entries sorted by name.
- `d_next` is the cookie of the following entry; the first call uses cookie 0.
- `d_type`: 3 for a directory, 4 for a file. `d_ino` is the node's inode number.
- The last entry may be cut off when the buffer fills, as preview1 specifies. `bufused == buf_len` means "maybe more": call again from the `d_next` of the last whole entry.

**Stubs.** These return `ENOSYS` (52) and log their name once:
- `fd_advise`, `fd_allocate`, `fd_fdstat_set_rights`, `fd_filestat_set_times`, `fd_renumber`
- `path_filestat_set_times`, `path_link`, `path_readlink`, `path_symlink`
- `proc_raise`
- `sock_accept`, `sock_recv`, `sock_send`, `sock_shutdown`

A browser exception inside any function becomes its errno (for example `EIO`), never a trap.

## 4. Page controls (no ABI, for completeness)

**Saves**:
- The header's **Saves** button lists `/saves/**/*.plr` and `*.wld`, with a **Download** button for each.
- It also imports `.plr` files into `/saves/Players` and `.wld` files into `/saves/Worlds`, asking before replacing.
- The Worker does the file work through the same tree, so it respects the game's open handles: a file the game holds open gives "the game has this file open".
- Nothing goes over the network.

**Clipboard:** see section 2.

**Fullscreen and re-import:** as in version 1.

## 5. Input records

Unchanged from version 1. Fixed **32 bytes**, little-endian. The module may read them with any alignment. Source of truth: `lib/input-codec.js`.

```
offset size type  field
 0     1    u8    kind
 1     1    u8    state     1 = down / pressed / focused / connected, 0 = up / released / blurred / disconnected
 2     2    u16   mods      bit 0 Shift, 1 Ctrl, 2 Alt, 3 Meta, 4 CapsLock on, 5 NumLock on, 6 Repeat (KEY only)
 4     4    u32   a         per kind
 8     8    f64   time_ms   when the event happened, on the Worker's performance.now() clock
16     4          s0        per kind (f32 or u32)
20     4          s1
24     4          s2
28     4          s3
```

Unused fields are 0.

| kind | name | state | a | s0 @16 | s1 @20 | s2 @24 | s3 @28 |
|---|---|---|---|---|---|---|---|
| 1 | KEY | 1 down, 0 up | key code id (table below) | u32 `KeyboardEvent.key` as one Unicode scalar, 0 for named keys (`Enter`, `Shift`, ...) | u32 `KeyboardEvent.location` (0 standard, 1 left, 2 right, 3 numpad) | | |
| 2 | TEXT | 0 | u32 byte length 1..16 | UTF-8 bytes 16..31 (`a` of them) | | | |
| 3 | MOUSE_MOVE | 0 | 0 | f32 x | f32 y | u32 buttons | |
| 4 | MOUSE_BUTTON | 1 down, 0 up | button | f32 x | f32 y | u32 buttons (after the change) | |
| 5 | WHEEL | 0 | 0 | f32 x | f32 y | f32 dy | f32 dx |
| 6 | FOCUS | 1 focused, 0 blurred / hidden | 0 | | | | |
| 7 | RESIZE | 0 | u32 pixel width | f32 css width | f32 css height | f32 devicePixelRatio | u32 pixel height |
| 8 | GAMEPAD_CONNECT | 1 connected, 0 disconnected | pad index | u32 1 = standard mapping | u32 axis count | u32 button count | |
| 9 | GAMEPAD_AXES | 0 | pad index | f32 axis 0 (left x) | f32 axis 1 (left y) | f32 axis 2 (right x) | f32 axis 3 (right y) |
| 10 | GAMEPAD_BUTTONS | 0 | pad index | u32 pressed bitmask, bit i = button i (0..31) | f32 button 6 value (left trigger) | f32 button 7 value (right trigger) | |
| 15 | OVERFLOW | 0 | records dropped | | | | |

Details:
- **Coordinates** (x, y): the page sends canvas device pixels (origin top-left); the Worker turns them into the game's back-buffer pixels (section 2, "Window") before the module reads them. They can fall outside the canvas while a button is held, because the pointer is captured.
- **Mouse buttons** use DOM numbering. In `a`: 0 left, 1 middle, 2 right, 3 back, 4 forward. In the `buttons` mask: bit 0 left, bit 1 right, bit 2 middle, bit 3 back, bit 4 forward.
- **Wheel** deltas are in CSS pixels. Positive `dy` means scrolling down (content moves up). Line mode is converted at 40 px per line, page mode at the canvas height. XNA's `ScrollWheelValue` counts 120 per notch and has the opposite sign; a typical notch is 100 px in Chrome on Windows. The mapping is the Rust side's.
- **Text**:
  - Typed characters arrive as `TEXT` records, separately from the `KEY` records of the same key presses. IME results arrive as one `TEXT` per composition end.
  - Long text is split across several `TEXT` records at code-point boundaries. Concatenate consecutive records.
  - Enter, Backspace, Tab and similar keys never produce `TEXT`. Use `KEY`.
- **Focus** sends 0 on window blur and when the tab is hidden, and 1 on focus. This matches XNA `IsActive`.
- **Gamepads** are polled once per display frame on the page. A `GAMEPAD_CONNECT` is sent when a pad appears or disappears. `GAMEPAD_AXES` and `GAMEPAD_BUTTONS` are sent only when the pad's `timestamp` changed. Values are raw: no dead zone is applied, so the host applies XNA's.
- **Key repeat**: auto-repeat sends extra `KEY` down records with mods bit 6 set.
- **Browser keys**: F5, F12 and Ctrl/Cmd + R/W/T/N/Tab keep their browser meaning; their records are still sent. Alt+Enter also toggles fullscreen.

### Key code ids (`KeyboardEvent.code`; append-only)

| | | | | | |
|---|---|---|---|---|---|
| 0 Unidentified | 1 KeyA | 2 KeyB | 3 KeyC | 4 KeyD | 5 KeyE |
| 6 KeyF | 7 KeyG | 8 KeyH | 9 KeyI | 10 KeyJ | 11 KeyK |
| 12 KeyL | 13 KeyM | 14 KeyN | 15 KeyO | 16 KeyP | 17 KeyQ |
| 18 KeyR | 19 KeyS | 20 KeyT | 21 KeyU | 22 KeyV | 23 KeyW |
| 24 KeyX | 25 KeyY | 26 KeyZ | 27 Digit0 | 28 Digit1 | 29 Digit2 |
| 30 Digit3 | 31 Digit4 | 32 Digit5 | 33 Digit6 | 34 Digit7 | 35 Digit8 |
| 36 Digit9 | 37 Enter | 38 Escape | 39 Backspace | 40 Tab | 41 Space |
| 42 Minus | 43 Equal | 44 BracketLeft | 45 BracketRight | 46 Backslash | 47 Semicolon |
| 48 Quote | 49 Backquote | 50 Comma | 51 Period | 52 Slash | 53 CapsLock |
| 54 F1 | 55 F2 | 56 F3 | 57 F4 | 58 F5 | 59 F6 |
| 60 F7 | 61 F8 | 62 F9 | 63 F10 | 64 F11 | 65 F12 |
| 66 PrintScreen | 67 ScrollLock | 68 Pause | 69 Insert | 70 Home | 71 PageUp |
| 72 Delete | 73 End | 74 PageDown | 75 ArrowRight | 76 ArrowLeft | 77 ArrowDown |
| 78 ArrowUp | 79 NumLock | 80 NumpadDivide | 81 NumpadMultiply | 82 NumpadSubtract | 83 NumpadAdd |
| 84 NumpadEnter | 85 Numpad1 | 86 Numpad2 | 87 Numpad3 | 88 Numpad4 | 89 Numpad5 |
| 90 Numpad6 | 91 Numpad7 | 92 Numpad8 | 93 Numpad9 | 94 Numpad0 | 95 NumpadDecimal |
| 96 IntlBackslash | 97 ContextMenu | 98 NumpadEqual | 99 F13 | 100 F14 | 101 F15 |
| 102 F16 | 103 F17 | 104 F18 | 105 F19 | 106 F20 | 107 F21 |
| 108 F22 | 109 F23 | 110 F24 | 111 ControlLeft | 112 ShiftLeft | 113 AltLeft |
| 114 MetaLeft | 115 ControlRight | 116 ShiftRight | 117 AltRight | 118 MetaRight | 119 IntlRo |
| 120 IntlYen | 121 KanaMode | 122 Lang1 | 123 Lang2 | 124 Convert | 125 NonConvert |
| 126 AudioVolumeMute | 127 AudioVolumeDown | 128 AudioVolumeUp | 129 MediaTrackNext | 130 MediaTrackPrevious | 131 MediaStop |
| 132 MediaPlayPause | 133 BrowserBack | 134 BrowserForward | 135 NumpadComma | | |

`code` is the physical key: `KeyZ` is the key in the QWERTY Z position on every layout. That matches XNA `Keys` (virtual keys on a US layout) closely enough for game controls. Use the `key` code point or `TEXT` for characters.

## 6. Errno values (WASI preview1 numbering)

The `tw` imports return these negated; WASI functions return them as they are.

| errno | value | used for |
|---|---|---|
| EACCES | 2 | write / create / remove / rename under `/install`, or of a root |
| EAGAIN | 6 | `net_send` while connecting, `net_recv` with nothing received; WASI `fd_read(0)` of the server Worker with no console line typed |
| EBADF | 8 | unknown handle or fd; `tw` write on a read handle |
| EBUSY | 10 | file open (remove / rename); OPFS lock held by another tab |
| ECONNREFUSED | 14 | `net_status` of a connection the bridge refused or could not make |
| ECONNRESET | 15 | a connection that broke |
| EEXIST | 20 | `path_open` with `CREAT` + `EXCL` on an existing file; `path_create_directory` on an existing entry |
| EFAULT | 21 | range outside `memory` |
| EINVAL | 28 | bad mode, offset, path syntax (`tw`: `.`, `..`, relative, NUL), unaligned audio pointer, `input_next` buffer < 32, unknown clock, seek before 0 |
| EIO | 29 | any other browser error |
| EISDIR | 31 | open of a directory for writing / `tw` open of a directory; rename onto a directory; unlink of a directory |
| EMFILE | 33 | more than 1024 open handles / fds |
| ENAMETOOLONG | 37 | component > 255 bytes or `tw` path > 4096 |
| ENOENT | 44 | missing file / parent; unknown root |
| ENOSPC | 51 | storage quota exceeded |
| ENOSYS | 52 | a WASI stub; `gfx_frame` without a replayer; `net_connect` without a bridge |
| ENOTDIR | 54 | a path component is a file; `O_DIRECTORY` / `fd_readdir` / rmdir on a file |
| ENOTEMPTY | 55 | remove of a non-empty directory |
| ENOTSUP | 58 | rename of a directory; `poll_oneoff` fd subscriptions |
| EPIPE | 64 | `net_send` after the server closed |
| ESPIPE | 70 | seek / tell / pread / pwrite on a standard stream |
| ENOTCAPABLE | 76 | WASI: a right the fd was not granted; a path that leaves its preopen or is absolute |

# Browser port: status

Design and evidence: `WEB_PORT.md`. Newest entry first.

## 2026-10-16: the game alone in the browser, served from this PC (no import step)

- **Start:** `play-in-browser.cmd` (VAEL - TERRARIA root) starts `web/host/terraria_host.py` (127.0.0.1:7790) and
  `tools/ws-bridge/ws_bridge.py` (127.0.0.1:7780; Host & Play joiners on 127.0.0.1:7777) with pythonw, no windows,
  and opens the default browser at **http://terraria.localhost:7790/**. `play-in-browser.cmd stop` ends exactly the
  processes it started (ids in `work/play-in-browser.pids`); `status` reports. User section: `web/www/README.md`
  "Play in the browser". Tested with `web/host/play.ps1 start -NoBrowser` / `status` / `stop` (no browser opened).
- **Host server** (`web/host/terraria_host.py`, stdlib only, 127.0.0.1 only): `/` = `play.html`, `/dev.html` = the
  old page, the rest of `web/www` with ETag + `no-cache`; `game.wasm` gzip-compressed once (55.9 -> 10.6 MB, cached
  by size + mtime in `web/host/cache/`); `/install/manifest.json` (15,998 files: Terraria.exe + Content/**, no other
  program file; ids from path + size + mtime, `--content-hash` for byte hashes); `/install/pack/<id>` (every file up
  to 32 MB, 258 MB, in the order the game opens them at start-up, `web/host/pack-order.txt`, recorded with
  `--record-order --write-order`, then by path; immutable; ranges) kept as one file `web/host/cache/pack-<id>.bin`
  (made in the background with 8 reader threads, 17 s; reading 16,000 files one by one cost about 7 ms per open the
  first time, the on-access scan: a cold pack took 114 s); `/install/file/<id>` (the 496 MB wave bank; ranges).
  Path traversal (`..`, `%2e%2e`) 404; HEAD; 304s.
- **No import:** `lib/remote-install.js` (vfs.js `createVfs(root, { install })`): /install is built from the manifest
  (stat / listing need nothing), files open as views of one OPFS file (`tw-cache/pack-<id>.bin`), downloaded by a
  Worker of its own (`install-worker.js`; in the game Worker the download got one network chunk per game slice);
  progress in a SharedArrayBuffer, an open the download reaches within ~1 s waits synchronously (Atomics.wait),
  else that file is fetched alone (Range); the wave bank by 1 MB blocks with synchronous XHR (`big-<id>.bin/.map`).
  The server Worker of Host & Play reads the same cache. HOST_ABI.md "Install over HTTP".
- **Play page** (`play.html`; main.js `PLAY`): the canvas fills the window, a text-free bar until the game's first
  frame, document title = the game window's title (new import `tw.window_title`: "Terraria: Legend of Maxx"), system
  cursor hidden while the game hides it, no log DOM (window.__twLog only), errors to the console; only an unsupported
  browser gets a message. `navigator.storage.persist()` asked. Requests made by the page and its Workers in a run:
  `/`, main.js, worker.js, vfs.js, host.js, wasi.js, gfx.js, install-worker.js, lib/*.js, game.wasm, /install/*
  (nothing else from the repo). Shaders: from the install's Content xnb (terraria-host content.rs `XnbAsset::Effect`).
- **Cursor offset (root cause):** two things. (1) The game's back buffer is not always the canvas size (its 800x600
  minimum: a 1024x768 window with the dev page's toolbar and log gave a ~578 px canvas and a 600 px back buffer,
  stretched, so the pointer drifted up to ~20 px); the picture is now fitted with its aspect kept (gfx.js `fitRect`)
  and the Worker maps pointer positions into back-buffer pixels through the same fit. (2) Started at its final size,
  the game's borderless start-up kept part of its state at another size (first click lost, the title's background
  ~75 px short of the right edge, PlayerInput's mouse scaled by ~0.94): `tw_init` now gets the height 2 px smaller and
  the first frame resizes to the canvas, as a native window opens and is then made borderless (the dev page had this
  by accident: its layout settled 605 -> 604). Canvas size from ResizeObserver `device-pixel-content-box`.
- **Resolution follows the window** through the game's own path (ClientSizeChanged -> SetDisplayMode); the canvas
  size is applied at the next presentation (never shown cleared); the game's limits are honoured (2049x1152 ->
  1920x1078 with the Reach profile, 640x480 -> 800x600, fitted).
- **Rust (terraria-web only, one build at a time, no other cargo running):** imports `window_title` and
  `frame_info(PlayerInput.MouseX, MouseY, flags: IsMouseVisible | gameMenu << 1 | showSplash << 2)` (lib.rs
  `report_window`). game.wasm 58.6 MB.

Measured (headless Chromium, NVIDIA, `node web/www/test/run-game-headless.mjs --host --page play --trace 0`):

| | manifest | game.wasm fetch + compile | tw_init done | title screen |
|---|---|---|---|---|
| cold (browser storage + HTTP cache cleared, host pack cache made) | 0.4-0.6 s | 0.16-0.2 s | 9.7-12.8 s | **17.4-20.6 s** |
| warm (second start) | 0.35-0.4 s | 0.15-0.2 s | 1.2-1.5 s | **8.3-9.1 s** |
| before: dev page, OPFS import already there (import check 9.5 s) | - | 0.2 s | 15.1 s | 56.2 s |

Cold: the pack download takes 0.9-5 s; most of the cold tw_init is the wave bank (496 MB) read in full by
`terraria-host` audio.rs `xact_file` (`std::fs::read` of the whole file to check its 4-byte signature, also natively)
and fetched synchronously. Warm title time is the game's content loading (CPU). A first start with a cold OS file
cache and no host pack cache: download 114 s (the per-file opens above; fixed by the pack cache).
Frame budget (`budget` action, 1280x800 / 1280x604): title 1.6-2.0 ms of module time per frame, world 1.9-5.8 ms;
the display frame 6.9-8.1 ms (124-144 fps, headless rAF). Memory after the title: wasm 1,270 MB.

Tests:
- `test/actions/play_title.txt` (cold and warm, above), `play_world.txt` (title -> new character -> new world ->
  in the world -> window 1280x604, 1024x768, 1600x900@1.25, 900x700@1.5, 700x500@2, 640x480, back, three times;
  shots `work/shots/f_*`, `rs_*`): 107 of 107 actions; `cursorcheck` 0 px worst at every size (6 points each);
  render targets 30 / 163 MB before and after every cycle; collector live after each cycle 171.9-176.0 MB (172.6
  before the resizes, 172.0 at the end), peak 1,220 MB unchanged; a run without resizes for comparison:
  `play_world_control.txt`.
- `play_dpr.txt` (title; `--params "&nodpbox"`: headless Chromium's emulated devicePixelRatio leaves the
  device-pixel box at CSS size, so the canvas is CSS x DPR there): 1280x800@1, 1024x768@1.25 (1280x960),
  1366x768@1.5 (2049x1152 -> 1920x1078), 800x600@2 (1600x1200), 1000x560@1 (-> 1000x600), 700x450@1.5: 0 px worst.
- `play_hostplay.txt` (play page, bridge running): Host & Play -> "Server started", "Hoster has joined.", chat,
  Save & Exit -> "Saving before exit...", server exit 0, the saved world seen and loaded: 84 of 84.
- JS 87 pass (+9: remote-install 8, window imports 1; the host import list test updated); bridge selftest ok;
  `run-host-console.sh` exit 0; `run_hostplay_web.sh` exit 0 (a first run failed once at the native joiner's
  "wait chat hello from A": it reached its wait after the host's four lines; the rerun passed, twice).

Follow-up (same day, from a check in the app's browser pane: 1154x592, resized while loading):
- **A flat colour over the whole canvas at the title** (canvas 1x1 until a later resize): the page's first RESIZE
  reached the game Worker while game.wasm was still compiling and was dropped (`input` was only handled once the
  host existed), so a page whose first measurement was wrong (laid out at 0x0, a pane still opening) kept a 1x1
  canvas. Now: input that comes early is kept and applied once the host and the replayer exist, `tw_init` starts
  from the newest measured size; the page measures at once after wiring, falls back to the client size when the
  layout box is 0 and to CSS x DPR without a device-pixel box, and measures again every display frame for 10 s, then
  every 30th. Test `test/actions/play_start_size.txt` (checkcanvas: canvas = CSS x DPR, back buffer = that or the
  game's minimum, not one flat colour; checktitle; cursorcheck) in five starts: 1154x592 resized to 1100x700 during
  load (warm, and cold at 300 ms), 1154x592 with a forced first measurement of 1x1 (`?devstartsize=1x1`), 800x500,
  1154x592: all pass, 0 px; with the old Worker the 1x1 start fails the check (canvas 1x1, the reported symptom).
- **"Terraria: Terraria: Terraria:"** is not an appended title: it is GameTitle 31 of the original's own
  `Terraria.Localization.Content.en-US.json`, one of the 68 random titles `Main.SetTitle` picks
  (`Lang.GetRandomGameTitle`); `tw.window_title` passes the whole title and the page replaces document.title.
  checktitle (document.title = the game window's title) passed in every run above with other picks.
- Regressions: JS 87 pass; play_dpr 21/21, play_world 107/107 (cursor 0 px); run-host-console.sh exit 0.

Open: the wave bank's full read at start-up (terraria-host `xact_file`: reading only the signature would save
496 MB of reading and memory at every start; a terraria-host change, not made here); Join via IP from another
program to the play page's Host & Play was not run on the play page (the same server Worker code passes in
`run_hostplay_web.sh` on the dev page); terraria-web never takes the WinForms window commands nor reports the window
(`winforms::take_commands` / `report_window`), so the queue grows by a few entries per resolution change.

## 2026-10-14 evening: server console, every command, compared with the original

- All 23 commands of `Main.startDedInputCallBack` plus `fps`, the usage lines and the unknown-command path are now
  tested in the browser's Host & Play, with joiners: `sh web/www/test/run-host-console-mp.sh`
  (`test/actions/host_console_mp.txt`; tools/ws-bridge reverse mode on 127.0.0.1:47800; the native port client is
  started at each page mark with `work/console-joiners/web/console_*.input`, written by
  `test/tools/make_console_joiners.py`). Final run exit 0, 138 of 138 actions, 4 joiners exit 0
  (`work/logs/hpcm_run_final.txt`, `hpcm_host_page.txt`, shots `hpcm_*`).
- The same commands went into the ORIGINAL `work/refserver/TerrariaServer.exe` (`python web/www/test/tools/orig_console.py
  web/www/test/actions/orig_console.txt`: no window, 127.0.0.1:47801, no -autoshutdown, about 140 s, killed by PID
  at the end), with the same native joiners on 47801. Its stdin must be UTF-16LE: `Program.InitializeConsoleOutput`
  sets `Console.InputEncoding = Encoding.Unicode` on Windows (UTF-8 input arrives as one line at end of file).
  `python web/www/test/tools/console_diff.py` compares the two transcripts command by command (ports, peer ports and
  fps numbers normalized): **0 differences** (`work/logs/console_diff.txt`). Identical: the help list (tabs too),
  version, port, maxplayers (255), seed (`World Seed: 1.1.2.0.12345`), dusk / midnight / dawn / noon + time
  (7:30 PM, 12:00 AM, 4:30 AM, 12:00 PM), motd (`Welcome to ref_small!`, set, `MOTD: ...`), password (none, set,
  shown, `password ` = `Password disabled.`), say / kick / ban usages, kick / ban of nobody (nothing), `bogus`
  (`Invalid command.`), settle (`Forcing water to settle.`, `Water is already settling`, later `Water has been
  settled.`), fps (`61  (61)` per second), kick / ban (`<addr> was booted: Kicked from server.` / `Banned from
  server.`), the refusals (`was booted: Incorrect password`, `You are banned from this server.`), `playing` (same
  lines; the page's server also lists its hosting player). Only in the original: the autosave when a joiner leaves
  (it runs without -autoshutdown; Host & Play passes it, so the port's server does not save then, as the original's
  Host & Play would not).
- Effects checked: wrong password -> the joiner's status "Incorrect password", right password -> in; the MOTD in the
  joiner's chat on join; kick -> the joiner back in the menu with "Kicked from server.", `Joiner has left.`, playing
  back to 1; ban -> `/saves/banlist.txt` = `//Joiner` + `127.0.0.1` (the server's working directory is /saves, the
  original writes it to its working directory: same content), the next join refused; clear -> the panel emptied;
  exit-nosave -> server exit 0, no "Saving before exit...", ref_small.wld unchanged.
- Fixed (test infrastructure only; no game, translator, host or Rust change, so no gates and no wasm rebuild):
  tools/ws-bridge closed the TCP side at once when the page closed a connection; with the joiner's bytes still unread
  that is a reset on Windows and the joiner lost the Kick message just before it (1 of 3 runs: no "Banned from
  server." status). It now half-closes (FIN) and lets that end finish (2 s). Runner actions `consolecleared`,
  `consoledump`, `savefile`, `same`, `waitmark`, `<sp>` in `console`; worker.js `saves read` takes `all` (automation)
  for files other than .plr / .wld. The page waits for the joiner's "MOTD seen" mark before kick / ban (a kick
  between "in the world" and the MOTD check failed the joiner's script once).
- Regressions: `run-host-console.sh` exit 0; `run_hostplay_web.sh` exit 0 (host, native joiner, second page, server
  exit 0, 0 unimplemented); bridge selftest ok; JS 78 pass.
- Open: the shared /saves index reports a file's mtime a few ms apart after it re-read a directory another Worker
  changed (the cached `Date.now()` of the write vs OPFS `lastModified`); `same` allows 1 s.

## 2026-10-14 later: the dedicated server's console in Host & Play

- Page: while a server of the page runs, a "Server console" panel sits beside the log, at the log's height, so the canvas
  keeps its size: output area (`#sc-out`), a command box and Send (index.html `#server-console`, main.js "Server
  console"). The typed line is echoed after the `: ` prompt; `Console.Clear` (ANSI 2J) empties the area; at the end it
  shows `server stopped (exit code N)` and takes no more input.
- Path (HOST_ABI.md "Server console"): page `console {line}` -> server-worker.js queues it as WASI **fd 0** (EAGAIN
  while empty) -> terraria-host `misc::console_read_line` reads fd 0 raw on wasm. stdout (fd 1) goes back as it is
  written (`console-out`; wasi.js `onStd`); the line log no longer shows the `: ` prompt in front of the next line.
- The game's own input thread runs the commands: `Main.startDedInputCallBack` + `Main.ReadLineInput` are resumable
  (tools/cs2rs game.json; `Console.ReadLine` awaited as `misc::console_read_line_async`). While no line is queued the
  machine parks with no stack, so the tick never blocks and mark phases still end. Host & Play passes `-autoshutdown`,
  and with it the original never starts that thread. So terraria-web `server.rs` calls the translated
  `Main.startDedInput()` once DedServ reaches its loop (`gameMenu` false). Natively nothing changes.
- Test `sh web/www/test/run-host-console.sh` (`test/actions/host_console.txt`; new runner actions `console`,
  `waitconsole`, `newer`, `pageshot`): exit 0, 75 of 75 actions (`work/logs/hpc_host_page.txt`, shots `hpc_*`).
  It checked: `help` (the list, "Display game time."), `say hello from the console` (`<Server> ...` on the console
  and `[chat] <Server> hello from the console` in the hosting client), `time` / `noon` / `time` = 12:00 PM / `dawn` /
  `time` = 4:30 AM, `playing` = `Hoster (127.0.0.1:50002)`, "1 player connected.", `save` (ref_small.wld
  2,987,873 -> 2,988,286 bytes, newer mtime, .bak written), and `exit` ("Saving before exit...", server exit 0, panel
  "server stopped (exit code 0)").
- Regressions: JS 78 pass (+1 wasi stdin/onStd). `run_hostplay_web.sh` exit 0: host 111/111, native joiner 0, second
  page 0, server exit 0, "0 unimplemented". A first run failed at Save & Exit because the panel had been put below the
  stage, which shrank the canvas and moved the click target; the panel now sits beside the log. `run_mp_web.sh port`
  exit 0. Gates `sh tools/cs2rs/gates.sh` (regenerated) all green: p1 3,911,380 exact, client-whole, rt 106, sim,
  ticks-player/world, host 109, app 102, viewer, scripts 14/14. wasm 58.2 MB.
- Open: a command runs on the input thread's fiber inside DedServ's pump, so a `save` holds that poll (about 0.5 s on
  ref_small, with page turns). Not tested here: kick / ban / password / motd / settle / seed / clear (same thread, same
  path). The native `-server` console is unchanged.

## 2026-10-14: Host & Play in the browser (the dedicated server in a second Worker)

- Follow-up (same day): one /saves tree for both Workers, and no long server polls.
  - vfs.js: each change under /saves is announced on BroadcastChannel `tw-vfs:<saves folder>`; the other index marks
    the directories stale, drops cached sizes, re-reads them at once (stat / listing stay synchronous); `path_open`,
    `path_filestat_get`, `fs_open` wait for a pending re-read and re-read the parent on a miss (`vfs.ready`); on the
    server's exit the game Worker re-reads all of /saves (`refreshSaves`), and the page ends the server Worker 2 s
    later. Tests `test/vfs-shared.test.mjs` (two indexes over one fake OPFS; JS 77 pass).
  - Long polls: (1) the world load waited in `while (!task.IsCompleted)` on the main stack, which read the clock only
    every 1,024 passes, each running an 8 ms fiber slice (8.7 s): after a fiber slice the next main yield point now
    reads the clock; the pass count between clock reads adapts (4..1,024, aiming at 1 ms). (2) Joins: 2-6 s stretches,
    CPU profile (`PROFILE_WORKER=server-worker`, `work/logs/hpw_server_join_profile.txt`): 24 % in
    `wasm_fibers::free_stack` (a 32 MB zeroed shadow stack and a full scan per network callback): shadow stacks are
    pooled (4), the high-water scan stops at 64 KB of zeros.
  - Evidence `work/logs/hpw_run6.txt` / `hpw_host_page.txt` (exit 0): server Worker "12,991 stretches without a page
    turn, longest 111 ms, 1 over 100 ms (connected)" (before: 2,031 / 3,463 / 5,827 ms); world load not among them.
    After hosting the game Worker's index, no reload: `Worlds/ref_small.wld 2,990,029 bytes` (the server's save; the
    import was 2,987,873) and `Worlds/ref_small.wld.bak`; Single Player -> world list -> that world loaded (shots
    `hpw_18`, `hpw_19`). Gates: fast (regenerated) + multiplayer green after the vfs / first threading change;
    `--only host,app` green after the last (wasm-only) threading change.

- Design: the game's own Host & Play path. `Main.HostAndPlay` -> `Process.Start("TerrariaServer.exe")` -> terraria-host
  `misc.rs` (wasm branch) -> `tw.server_start` -> the page starts `www/server-worker.js`: a module Worker with its own
  instance of game.wasm (the game Worker's compiled `WebAssembly.Module` is passed along) over the same OPFS files.
  terraria-web `server.rs`: `tw_server_init()` (launch parameters = WASI argv[1]; what terraria-app `server.rs`
  does: `Main.dedServ`, `LaunchGame`) and `tw_server_tick(now) -> ms`: one poll of `Main.DedServ`'s resumable form
  up to its next `Thread.Sleep(ms)` (host `threading::poll_main_machine`: every sleep runs the queued work, then
  parks), collector slice between polls, the Worker waits `ms`. Natively nothing changes.
- Translator (tools/cs2rs, DESIGN.md 8f): resumable instance methods (non-virtual, `this` in the machine), the
  thread table gets the delegate's receiver (`fn(recv)`), config `"await"` (a library member awaited inside a
  resumable form). game.json: `Main.DedServ`, `Netplay.ServerLoop`, `BroadcastThread`, `TcpSocket.ListenLoop`
  (`AcceptTcpClient` -> `TcpListener::accept_tcp_client_async`, which parks via `threading::park_async`).
- Listener (host `net.rs` wasm): `TcpListener.Start/AcceptTcpClient` = `tw.net_listen/net_accept/net_peer`. In-page:
  the hosting client's connection to 127.0.0.1:<server port> is a MessageChannel the page relays (held until the
  server listens). Outside: `ws_bridge.py --listen <tcp port> --to-page` (reverse mode: control `?listen=`, one
  `?accept=<id>` WebSocket per TCP connection); the server Worker registers when it listens (retry every 5 s).
  `UdpClient.Send` sends nothing on wasm. HOST_ABI.md "Network" / "Server". Outside the owned files: terraria-host
  `misc.rs` (the wasm branch of `Process.Start/HasExited/Kill`), `web/ci/build_game_wasm.sh` (two exports).
- Evidence: gates `sh tools/cs2rs/gates.sh` (regenerated) green: p1 3,911,380 exact, client-whole, rt 106, sim,
  ticks-player/world, host 109 (+1: `the_main_machine_parks_at_every_sleep_and_instance_threads_get_their_receiver`),
  app 102, viewer, scripts 14/14; `--nogen --only multiplayer` ok (A 0, B 0). JS 74 pass (+4 `net-local.test.mjs`);
  `node tools/ws-bridge/selftest.mjs` ok incl. reverse mode. wasm 58.2 MB.
- `sh tools/ws-bridge/run_hostplay_web.sh` (all hidden; exit 0 three times, `work/logs/hpw_run{1,2,3}.txt`; the last with the final game.wasm, whose "work fiber ended" line is logged only for work of 1 KB+ shadow stack: 14 lines instead of 8,888): page 1
  (`test/actions/mp_host.txt`) Host & Play with ref_small -> `[server] Server started`, `Hoster has joined.` -> the
  native port client (mp_join.input, 127.0.0.1:47800 through the bridge): `Joiner has joined.`, bridge trace
  `TileChange (2097, 232)`, `(2095, 231)`, four `<Hoster> hello from A` (the client's `wait chat hello from A` held),
  `Joiner has left.` -> page 2 (`mp_join_page.txt`, own profile copy work/web-profile-2): `WebJoiner has joined.`,
  `TileChange (2098, 231)`, `<WebJoiner> hello from web`, left -> host Save & Exit -> `Local player left.
  Autoshutdown starting.`, `Saving before exit...`, server exit 0, "0 unimplemented host members". Shots
  `work/shots/hpw_01..17`, `mpw_*` (page 2); logs `hpw_host_page.txt`, `hpw_join_page.txt`, `hpw_native.log`, `hpw_bridge.log`.
- Memory (`HOST_ACTIONS=test/actions/mp_host_long.txt`, `work/logs/hpl_host_page.txt`): 9,645 game updates hosted
  (update count 66,294 -> 75,939), 5 collector cycles (every 3,600 polls), after each: 568.0 / 567.0 / 567.0 /
  566.9 / 567.1 MB allocated, collector heap 145.1 / 144.8 / 144.7 / 144.7 / 144.8 MB. Server instance peak 718.5 MB;
  the hosting page's wasm memory 1,316 MB.
- Open: `Sleep(0)` polls wait on a 0 ms timer
  (about 4 ms in a Worker, the 60 Hz pacing is kept by DedServ's own accumulator); one page server at a time; console
  commands of the server (no stdin).

## 2026-10-14 early: medium and large worlds, and a title load that gives the frame back

- Measured first (new `test/actions/world_medium.txt`, `world_large.txt`: the single_player_fast flow with the size,
  seed and options of the original's `ref_medium` / `ref_large`, then 600 frames in the world). Before any change both
  sizes already worked, no trap: medium wasm memory 2,239 MB (live peak 1,850 MB), large 3,295 MB (live peak 2,671 MB
  in the world, still rising), generation 93 s / 236 s, title frames 110 = 4.5 s and 48 = 1.1 s, 93 frames over
  100 ms (frames 4-43 about 160 ms each), large world load one 1.56 s work frame. Logs `work/logs/web_{med,large}_before*.txt`.
- Where the time went (new: V8 CPU profiles of the Worker through CDP, runner action `profile start|stop <name>` /
  `--profile-start`, `test/profile-slices.mjs`): 11.6 of 34.9 s of the title load in wasi-libc's `sbrk`. V8 charges
  about 1.3 ms per `memory.grow` whatever its size, and malloc grows by exactly what one allocation needs
  (`web/spikes/memory-grow.mjs`: 4,576 grows of 256 KB 6.1 s, 18 grows of 64 MB 24 ms). The rest of the long title
  stretches: the fonts (Death_Text: 10.6 MB of LZX in one `read_xnb` call), a music track decoded whole on first play.
- Where the memory went (new: live blocks by size class in the 300-tick log line): every `SoundEffect` kept its
  samples twice (`pcm`, unused: 177 MB at the title); every collector object is one malloc block, and dlmalloc makes a
  16 + 14 byte `Tile` object 48 bytes (20 million tiles in a large world).
- Fixes (browser build only, natively nothing runs differently):
  - terraria-web `lib.rs` allocator: after an allocation that grew the memory, it grows 64 MB more at once
    (`GROW_STEP`); blocks of up to 64 bytes come from a pool of 16/32/48/64-byte classes (`small`); live blocks by
    size class and the pool's slabs are logged. Medium generation 93 -> 52 s, large 236 -> 119 s.
  - terraria-host `audio.rs`: `SoundEffect.pcm` field removed (a method on the one copy; only a test read it).
  - Main-stack yields: terraria-host threading `main_yield` (wasm): inside a window the driver opens around game
    code (tw_init, each tw_frame up to Present), a translated yield point on the MAIN stack past a 50 ms slice calls
    the driver's hook: the sound is pumped and the whole stack waits in `tw.yield_frame` for the page's next tick.
    Nothing else runs meanwhile (no Update, Draw, work or collector), so for the game it is one slow frame, as
    natively. Host loops call `main_yield::check()`: between dispatcher actions, before each XNB parse, between a
    font's page textures, every 1,024 MS-ADPCM blocks. worker.js does not advance the `?fixedstep` clock for such a
    tick (scripted runs stay frame-exact) and measures the stretches between ticks (`sliceMax`, `slicesOver100`,
    `mainYields`, `slowImports`).
  - terraria-host `content.rs` `decompress_in_slices` / `parse_sliced` (used on wasm for compressed XNB files of
    512 KB or more): LZX frame by frame with a check every 16 frames; test `sliced_lzx_equals_the_formats_decoder`
    (the 5 fonts and Tiles_0: payload and parsed `Xnb` equal to terraria-formats').
  - `web/ci/build_game_wasm.sh` stops when cargo fails (it copied the old module as new before).
- After (`work/logs/web_med_final*.txt`, `web_large_final2*.txt`): medium wasm memory 2,023 MB (live peak 1,700 MB),
  large 2,743-2,780 MB (live peak 2,375-2,493 MB), title 916 MB live / 1,142 MB peak (was 1,095 / 1,330). Generation
  3,000 frames / 51.8 s (medium), 6,600-7,200 frames / 118-125 s (large); 600 frames in each world; no trap.
  Stretches without a page tick over 100 ms after tw_init: 0 to 5 per run, all of them one wait: `wasi.path_open`
  (an OPFS `createSyncAccessHandle`) answering in 81-181 ms during the title's content load (large run: frames 5, 7,
  40, 43, 49 = opens of 81, 147, 100, 181, 60 ms; the new `slowImports` list), plus once a loading-screen Draw of
  140 ms and once the work fiber writing the 11.8 MB world file in one write (108 ms). The Worker is idle in those
  waits (no CPU), but no frame can be presented until the open answers. tw_init itself: 109-233 ms (language files,
  static constructors: no yields inside those). The title's frame 110 still takes 2.0 s, now in 39 pieces.
- Native gates: `sh tools/cs2rs/gates.sh` green (regenerated: p1 3,911,380 values exact, client-whole, rt 106, sim,
  ticks-player, ticks-world, host 108 (+1), app 102, viewer, scripts 14 of 14); `--nogen --only worldgen`: all six
  worlds 108 of 108 rows (`work/logs/gates_{fast,worldgen}_final.txt`). JS unit tests 70 pass.
- The worlds: exported from the browser's OPFS (`test/export-opfs.mjs`) and compared with the ORIGINAL game's
  `ref_medium.wld` / `ref_large.wld` (`web/ci/compare_wld.py`): tiles, chests, signs, NPCs, tile entities,
  pressure plates, town manager, bestiary, creative powers byte-identical; header and footer differ by design (name,
  ids, time). Shots `work/shots/med_*.png`, `large_*.png` (title, creation, generation, entering, 600 frames).
- Stacks: shadow-stack high-water 157 KB of 32 MB (generation), 1 KB (load). JSPI engine stack, by Chrome's
  `--js-flags=--wasm-stack-switching-stack-size` (runner `EXTRA_CHROME_ARGS`): the large generation traps with
  "RangeError: Maximum call stack size exceeded" at 128 KB and 256 KB (reported as a trapped fiber, the game goes
  on), completes at 512 KB; the default is about 1 MB, so at least twice the need.
- Open: OPFS file opens of up to 181 ms during the title load (page side: e.g. open handles ahead of the game, or
  present the last frame while an open waits); a world save writes its file in one call (108 ms slice); the collector's object list and mark
  stack grow by doubling (256 + 128 MB blocks in a large world, gen-rt, not changed); the static constructors in
  tw_init cannot yield (`CctorGuard`).

## 2026-10-13 late night: collector cycles end while connected (the network thread as a state machine)

- Cause: the client has one thread that never ends, the "TCP Client Thread" (`Netplay.TcpClientLoop` ->
  `TcpConnectLoop`, `InnerClientLoop`, `Netplay.cs:390-545`; it sleeps 1 ms in every iteration while connected). Its
  fiber was always suspended in that `Thread.Sleep`, so `has_suspended_work()` was always true and no mark phase
  could end. The 834 "work fiber ended" lines were the read callbacks (`ClientReadCallBack`), which end in their pump.
- Fix (WEB_PORT.md 2.2 option 2, the planned translator rule): config `"resumable"` in `tools/cs2rs/game.json`
  (DESIGN.md 8f). Those three methods ALSO get an `async fn X__resumable`: `Thread.Sleep` = `sleep_async(ms).await`,
  calls between them are awaited, `try` = `try_async` (`rt::try_` around each poll). `gen::resumable::THREADS` maps the
  start delegate's method id to the machine. terraria-web installs it (`set_resumable_threads`); the native shell
  does not, so natively the plain functions run, unchanged. Host (`threading.rs` "Resumable threads"): such a thread's
  fiber does ONE poll per entry; at a sleep of 1 ms or more the machine returns `Pending`, the fiber's body returns
  (no stack left), the thread is parked (the pump continues it; `has_suspended_work` ignores it). The machine is one
  heap block scanned conservatively by a collector root, which is traced again when a mark phase ends. Waits inside a
  poll (`Connect`, yield points, locks) still suspend the fiber, and no mark phase ends meanwhile. The parked
  machine keeps its 32 MB shadow stack, so a poll costs no allocation.
- Evidence: host test `a_resumable_thread_parks_at_its_sleeps_and_collections_end_while_it_lives` (Windows fibers,
  poison mode: 4 incremental cycles end while the thread lives, an array only the machine holds survives them, a
  control array is freed, an exception in a later slice is caught). Negative control: with the root's scan off, the
  held array reads 0xDD.. (freed). Gates `sh tools/cs2rs/gates.sh` (regenerated, deterministic): p1 3,911,380 values
  exact, client-whole, rt 106, sim, ticks-player, ticks-world, host 107, app 102, viewer, scripts 14 of 14;
  `--nogen --only multiplayer` ok (A exit 0, B exit 0). `sh web/ci/build_game_wasm.sh`: 57.8 MB.
- Browser, `MP_ACTIONS=test/actions/mp_join_long.txt sh tools/ws-bridge/run_mp_web.sh port` (new actions file: the
  mp_join scenario plus ~3,300 frames in the world and a `waitlog collector cycle`; the env hook is new in
  run_mp_web.sh): 97 of 97 actions, runner exit 0. Connected from tick 1843 to 5767 (3,924 ticks). 8 cycles ended
  while connected (ticks 2096, 2647, 3124, 3626, 4132, 4640, 5128, 5622), each freeing ~48 MB / 1.13 M objects. The
  live heap after each cycle was flat at 106.6-108.7 MB. Module allocation was 1,062-1,139 MB live, and the peak
  stayed at 1,329.7 MB (the title's). Before the fix: no cycle, and 17.6 -> 136 MB allocated since the last cycle
  over 1,200 ticks. Server: joined, `<Joiner> hello from web`, left, autoshutdown. The ORIGINAL TerrariaServer.exe
  (once, killed by PID): the same, 8 cycles while connected, live 107.0-110.3 MB. Logs
  `work/logs/mpw_{port,original}_long_{page,run}.txt`; shots `work/shots/mpw_long_*.png`. No process left over.
- Open: the server-side endless loops (`ServerLoop`, `ListenLoop`, `BroadcastThread`) are not resumable (no server
  in the browser). The "work fiber ended" warn line for every read callback is unchanged (1,110 lines in the long
  run). A `using` / `foreach` over an enumerator inside a resumable method is allowed by the rule but not exercised.

## 2026-10-13 night: multiplayer from the browser (stage 4a: Join via IP through a WebSocket bridge)

- Design: the game's `TcpClient` connection is a WebSocket to a bridge on the player's computer, which opens the TCP
  connection the game asked for. Everything above the byte stream is net.rs as it was (polled reads / writes, queued
  callbacks, the network threads as fibers that sleep). terraria-host `sys::net::wasm_net` (cfg wasm): `Stream` is a
  `WsStream` there (`TcpStream` natively, unchanged); `Connect` polls `net_status` and gives the frame back
  (`wait_a_frame`, a fiber suspend) until the bridge connected or refused (21 s bound). The imports are installed by
  the driver as a `Backend` (as the fibers), so the host's wasm unit tests need no `tw` imports. No name lookups on
  the web (addresses and "localhost"); no listening, no LAN broadcast.
- `tw.net_connect / net_status / net_send / net_recv(peek) / net_close` (HOST_ABI.md "Network"; `www/lib/net-bridge.js`,
  never suspend). The bridge is the page's business: `?bridge=`, else the header's "Server bridge" field (remembered),
  else `ws://127.0.0.1:7780`. terraria-web logs `now in the world / the menu (netMode)` changes.
- `tools/ws-bridge/ws_bridge.py` (Python stdlib): 127.0.0.1 only; connects the TCP side before answering the
  handshake (open = connected, refused = HTTP 502); targets loopback only (`--allow-lan`: private LAN; public never);
  page origins loopback only (`--origin`); logs connections; `--trace-terraria` logs connect / player / tile change /
  chat messages and per-type counts. `selftest.mjs`: 1 MB round trip, server close = close 1000, refused port,
  public / LAN / name targets and a foreign origin refused.
- `tools/ws-bridge/run_mp_web.sh port|original`: server on 127.0.0.1:47800 (fresh ref_small.wld, -autoshutdown), the
  bridge, the headless runner with `test/actions/mp_join.txt` (new runner actions `waitlog <max> <text>`,
  `mouse Down|Up`, `--bridge`); kills everything by PID.
- Evidence (both servers, 85 of 85 actions, runner exit 0): Multiplayer -> Join -> new player "Joiner" -> 127.0.0.1 /
  47800 -> connected in 0-15 ms -> in the world 35-40 frames later -> dig -> walk -> chat -> Settings -> Save & Exit
  -> title. Server console, port and ORIGINAL TerrariaServer.exe alike: `Joiner has joined.`, `<Joiner> hello from
  web`, `Joiner has left.`, `Local player left. Autoshutdown starting.` Bridge trace: `page->server TileChange action
  0 at tile (2097, 232)` (twice: hit, break) and `(2095, 231)`, `chat command 'Say' text 'hello from web'`; about 415
  messages / 4.8 KB up, 512 / 82 KB down in 9.3 s. Shots `work/shots/mpw_port/` and `work/shots/mpw_original/`
  (`mpw_10_in_world`, `mpw_13_dug`, `mpw_14_walked`, `mpw_15_chat`, `mpw_18_title_after_leaving`); logs
  `work/logs/mpw_{port,original}_{run,page,server,bridge}*`.
- Native gates `--nogen --only rt,host,app,scripts` green (106 / 106 / 102 / 14 of 14). Unchanged: wasm unit tests of terraria-host (97 passed, the known `files_and_directories` failure, 3 socket tests
  ignored); JS unit tests 70 pass (+4 `test/net-bridge.test.mjs`).
- Open (fixed in the late-night entry): while connected the network threads are always suspended, so no collector mark phase ends (allocated since
  the last cycle grew 17.6 -> 136 MB over 1,200 ticks; WEB_PORT.md 2.2 option 2 / translator state machines). Entering
  the world costs one 2.8 s frame (main thread). threading.rs logs "work fiber ended" at warn level for every network
  callback (834 lines in one run): it should log only for real work. The server does not log tile edits; the bridge
  trace is the server-side evidence. Joining a server on the LAN needs `--allow-lan`; browsers other than the
  player's own machine (WebRTC, in-browser server) are not done.

## 2026-10-13 evening: loading screen during generation and load (stage 2: work fibers on JSPI)

- Design: the native one, on JSPI. Each queued work item (world generation, load, save) runs on an engine stack of its
  own (promising export `tw_fiber_entry`) with a 32 MB shadow stack of its own, which has a guard checked at every
  switch. The main stack, inside `tw_frame` -> `run_pending_work` -> `resume`, waits in the suspending import
  `tw.fiber_resume`; the work gives the frame back with `tw.fiber_suspend` after an 8 ms slice (`YieldMode::Millis`;
  wasm has no timer thread, so an armed yield point reads the clock every 1,024 passes). The page swaps
  `__stack_pointer` at every hand-over (`web/www/lib/fibers.js`, the spike `sched.mjs` shape).
  Why this design: it is the same structure as the native fibers (threading.rs `resume` / `switch_to_main` / `run`,
  only the stack switch differs), so the game's waits (`playWorldCallBack`'s `loadMapLock` loop, locks, `Task.Wait`)
  behave as natively. A fiber that suspends in a file import needs nothing special.
- threading.rs: the fiber paths are compiled for `windows x86_64` or `wasm`. The wasm stack switch is a `Backend`
  the driver installs (`wasm_fibers::install`). Without one (the wasm unit tests under Node's WASI), work runs inline
  as before. Native code paths are unchanged.
- Collector: while work is suspended, `gc_step(.., may_finish = false)`. In the run the mark phase ran on during
  generation and ended after it (phase Mark from tick 2100 to 3000, Sweep at 3300).
- Evidence, headless `--actions test/actions/single_player_fast.txt` (the new `waitwork` action waits for the work
  and takes shots meanwhile): generation took 1,440 frames (31.6 s) instead of one 27 s frame. Work frames: 1,457,
  mean 22.0 ms. None over 100 ms during generation. During load, frames 3471 and 3472 took 370 and 291 ms, but the
  fiber's share was only 16 and 10 ms; the rest is main-thread work on entering the world. Shots
  `work/shots/f_gen_01..11.png` show the progress text changing ("Adding mushroom patches", "Generating
  structures", ...) with moving bars. `f_load_01.png`, then `f_14_in_world_600.png` (the world drawn correctly).
  Shadow-stack high-water mark: 110 KB (generation). Memory: wasm 1,466 -> 1,636 MB during generation, 1,713 MB in
  the world. Audio during generation (run 1): the module pushed 19.4 s of sound in 19.3 s wall (tick 2100 -> 3000).
  The whole run had 719,744 underrun frames, most of them in the two title-load frames (1.2 s, 4.3 s) and the
  runner's pauses between actions.
- Gates `--nogen --only rt,host,app,scripts` green (106 / 106 / 102 / 14 of 14). JS unit tests: 66 pass
  (+5 `test/fibers.test.mjs`).
- Open: the main-thread cost of entering the world (370 ms). The title-load frames still run in `tw_init` / the first
  frames without yields. The engine stack depth of a large world's generation on a JSPI stack has not been measured
  (small world only).

## 2026-10-13 afternoon: sound (stage 3: the native mixer into the AudioWorklet)

- The mixer moved out of terraria-app into `terraria-host/src/audio/mixer.rs` (`Mixer`, `AudioStats`, `soft_limit`,
  `BlockReader`). It has no thread, no device and no Bevy. terraria-host was the smallest move: terraria-web already
  depends on it and `AudioCommand` / `Sound` live there. terraria-app `host/audio.rs` keeps the thread, waveOut and
  the env variables. One change: voices are summed in voice-id order (`BTreeMap`). The `HashMap` summed them in a
  per-process random order, so with 3+ voices the low bits differed between runs and could never match across builds.
- terraria-web: every `FrameOutput.audio` (tw_init and each tw_frame) goes to the mixer. Each tw_frame then pushes
  what `tw.audio_free()` asks for through `tw.audio_push` (a `Vec<f32>`, 4-byte aligned; refused frames are kept for
  the next frame). It mixes in the native 1,024-frame blocks (`BlockReader`), so the samples match for the same
  commands. The mixer stats are logged every 300 ticks.
- Page: fixed a race in `worker.js`. An `audio` message that arrived before the host existed was dropped, so the page
  stayed silent for good (a click during loading, or `?devaudio`). The AudioWorklet now reports frames / peak / RMS /
  underruns (`LevelMeter`, lib/audio-ring.js, +1 unit test: 61 pass). `?devaudio` starts audio without a gesture.
  The headless runner uses it with `--autoplay-policy=no-user-gesture-required --mute-audio`.
- Evidence: gates `--nogen --only host,app` green (106 / 102). New test
  `the_moved_mixer_is_bit_equal_to_the_native_one`: 48 blocks of a fixed sequence (every command kind, up to 6
  voices, 3 source rates, a stream). The test compares the pre-move code, kept as a reference copy, the moved mixer
  and a `BlockReader` read in 735/128/1/160-frame pieces: bit-equal, same stats.
- Headless `--frames 1800` on the title: the module pushed 692,864 frames and the AudioWorklet played 691,200
  (15.7 s). Peak 10,615 of 32,767, RMS 1,558 (-26.5 dBFS). The module's own mixer stats agree: peak 10,614, RMS
  1,558.2, 3 voices. Native `run_hidden.sh g1_title.input` (muted; this run used the client exe built before the move):
  883 blocks, 10 voices (the script also clicks through the language menu), peak 14,879, RMS 1,738.6 (-25.5 dBFS).
  The web RMS at tick 900, music only, was 1,736.6.
- Open: 649,728 underrun frames (14.7 s) in that run. Most come from the long loading frames: one tw_frame lasts
  seconds and `audio_free` gives at most 4,096 frames per call (stage 2 fibers). Sound is not yet verified by ear in
  a real browser.

## 2026-10-13 late morning: playable in the browser (headless-verified)

Scripted run in hidden headless Chrome with WebGPU (`node web/www/test/run-game-headless.mjs --actions
test/actions/single_player_fast.txt`; synthetic page input, never the real mouse / keyboard): title -> new character
-> new small world (generated in the browser: one 27 s frame) -> in the world, 600 frames, drawn correctly
(`work/shots/f_14_in_world_600.png`: sky, trees, grass, player, Guide, HUD, minimap). Memory: 267 MB after tw_init,
~920 MB live on the title, 1.5-1.6 GB live in the world, wasm memory 1.86 GB, stable.

Fixed on the way (each found by measurement):
- the frame as one buffer (862 MB of uploads in one title-load frame): streamed in 8 MB parts (`gfx_frame_chunk`)
  and new textures' pixels sent at once (`gfx_texture`, host `draw_list::set_upload_sink`);
- content textures kept a CPU copy of every pixel: they are re-read from their file for GetData now (host
  `content.rs`, `Stream::source_file`); closed file streams free their buffer at Close (`rt/io.rs`);
- the game rebuilt its render targets every frame (the module reported 1280x605, the game applied 1280x604), which
  also restarted the lighting engine (the world was drawn black) and leaked ~1 MB per tick: the window follows the
  game's ApplyChanges size, as natively (`set_window_follows_apply_changes(true)`);
- delegate method ids were cut to 32 bits on wasm32 (u64 now), WASI has no file attributes (`file_set_attributes`).

Open: sound (stage 3: the mixer into the AudioWorklet), the loading screen during world load / generation (the page
freezes for that frame: needs fibers on JSPI, stage 2), multiplayer (stage 4: WebSocket bridge), a person opening
the page in Chrome / Edge (serve.py, then the folder picker), save export / import by hand, wasm memory for medium /
large worlds (4 GB address space).

## 2026-10-13 morning: the game runs in a browser (stage 1, first light)

- `game.wasm` (57.7 MB, `port/crates/terraria-web`, wasm32-wasip1 reactor; build `sh web/ci/build_game_wasm.sh`):
  driver = the native shell's frame sequence (lib.rs), frame serializer with effect resolution (frame.rs, streamed
  in 8 MB parts: one title-load frame carries 862 MB of texture uploads), input records -> XNA input (input.rs).
- Page (`web/www`, by the worker): player picks their own Terraria folder; Terraria.exe + Content copied into the
  browser's private storage (731 MB, 16,000 files); WASI over OPFS (wasi.js / vfs.js); WebGPU replayer (gfx.js,
  0 pixel mismatches against JS-computed expectations, headless on SwiftShader and the NVIDIA adapter); 59 unit tests.
- Proof 1 as wasm (`sh web/ci/wasm_game_test.sh proof1`): 3,911,380 of 3,911,380 values exact (after delegate method
  ids became u64).
- Embedded resources read from the player's Terraria.exe (`rt/clr_resources.rs`; 100 resources, manifest order,
  90 byte-identical to the decompile's copies).
- Headless run (`node web/www/test/run-game-headless.mjs`): tw_init 1.2 s, 908 frames, no trap; the Re-Logic splash
  renders (`work/shots/web_title_final.png`). Open: the title menu, config.json save (WASI errno 52), wasm memory
  2.1 GB (texture data kept twice: draw list + replayer), input not yet exercised, sound (stage 3).

## 2026-10-13: stage 0 (everything but the game runs its tests on wasm)

Command: `sh web/ci/wasm_tests.sh <crate>` (nightly `-Zbuild-std`, wasm exception handling, wasm32-wasip1, Node 24's
WASI, one instance per test; runner `web/ci/wasi-run.mjs` preopens the project root as `/w` and the player's install
as `/install`). Logs: `web/logs/stage0-<crate>.txt`.

| crate | wasm32-wasip1 | native | not run on wasm, and why |
|---|---|---|---|
| `terraria-gen-rt` | 105 passed | 105 | 1 ignored (as natively) |
| `terraria-x87` | 14 passed | 14 | none; the report equals native (worker, `logs/x87-*.normalized.txt`) |
| `terraria-shaders` | 14 passed | 16 | 2 need a GPU adapter (WASI has none): run in the browser stage |
| `terraria-host` | 97 passed (1 partly) | 106 | 3 socket tests (stage 4: WebSocket backend); `sys::io::tests::files_and_directories` fails after `FileInfo` and the listing part of `content::tests::loads_from_the_install_and_caches` is skipped: Node's WASI on Windows implements neither folder listing (`fd_readdir`: "Function not implemented") nor a metadata call FileInfo needs. Stage 1 replaces this file layer with the browser's own (OPFS), where these are tested |

Source changes (native behaviour unchanged, every native gate green after them):
- `gc.rs`: the three wasm32 fixes from the worker's `gen-rt-patched` (address range by pointer width; pointer
  alignment `w % word`, which keeps a `Str` in a suspended iterator alive on wasm32; a test constant).
- `terraria-gen-rt`: `test_temp_dir`; a test closes its stream before removing its folder.
- `terraria-host`: `manifest_dir()` (fixtures under `/w` on WASI), `scratch_dir()`, `install_dir()` = `/install` on
  WASI, `os_random` through WASI `random_get`, `get_files_recursive` maps `\` to the host's separator (results keep
  `\`), `host_lists_directories()`; socket tests and two GPU tests ignored on wasm with the reason.

Next: the whole game crate (`terraria-gen-game`, 1.13M lines) built for wasm32 at `client-opt` settings: size,
compiler memory, V8 function limits (WEB_PORT.md section 7). Then stage 1 (title screen).

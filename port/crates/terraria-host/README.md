# terraria-host

The native layer under the machine-translated Terraria 1.4.5.8 client: the parts of XNA 4.0, ReLogic's platform
edge and `System.*` that the game calls, in Rust, without Bevy. "FNA, but only the members this game uses."

It depends on `terraria-gen-rt` (the translated code's object model: `Gc`, `Str`, `Arr`, `Trace`, exceptions),
`terraria-core` (XNA value types) and `terraria-formats` (XNB). `terraria-gen` names it through the mapping
tables; the app (`terraria-app`) drives it and replays its output.

| read | for |
|---|---|
| this file | architecture, the driver protocol, what does not work |
| `COVERAGE.md` (generated) | every member of `tools\cs2rs\HOST_API.md`: implemented + tested / untested / object model only / unimplemented, with the reason |
| `REQUESTS.md` | what this layer needs from the runtime / generator |
| `native.d\host_*.json` (generated) | the mapping tables; **staged here, not installed** (see "Mapping tables") |
| `docs\readback.md` | `GetData` on render targets: call sites, the decision, the exact change the wgpu backend needs |

## Architecture

```
translated game (terraria-gen)            terraria-host                          terraria-app (Bevy)
  Main.Update / Main.Draw      ->  object model with XNA semantics   ->  gfx::Frame     -> gfx/render.rs (wgpu replay)
  spriteBatch.Draw(...)             gfx: GraphicsDevice, SpriteBatch,     audio commands -> host/audio.rs (mixer, waveOut)
  Keyboard.GetState()               Texture2D, states, Effect...      <-  InputSnapshot  <- host/input.rs (Bevy input)
  File.Exists(...)                  input, audio, content, game, os, sys
```

- **Objects** are plain Rust structs allocated with `rt::alloc_init`, used through `Gc<T>` by translated code
  (`{this}.method(args)` templates). One Rust type per C# hierarchy: `Texture2D` is also `RenderTarget2D`,
  `VertexBuffer` also `DynamicVertexBuffer`, `sys::threading::Task` also `Thread`.
- **Graphics output** is a backend-neutral draw list (`gfx::draw_list`): ordered commands (create / upload /
  destroy texture, set target, clear, viewport, scissor, draw range with texture + blend + sampler + cull +
  effect snapshot, readback request) and one vertex buffer per frame, vertices in clip space. The app keeps only
  the replay. The hand-ported prototype in `terraria-app` records into the same list (`gfx::SpriteBatcher`,
  `DrawList::draw_colored_quads`).
- **Audio output** is PCM buffers plus an ordered command queue (`audio::AudioCommand`).
- **`Game`** is one per process: its state is thread-local (`game::HostState`) and the `Game.*` members are free
  functions. The translated `Main : Game` does not derive from a native class.
- **Not implemented** always goes through `terraria_gen_rt::host::unimplemented` / `unimplemented_value` with the
  member's name, so it shows up in `host::reached()`. Nothing returns a made-up value.

## Driver protocol

The host never calls `Main.Update` / `Main.Draw`: the app does, when the host says so. All of it on ONE thread
(the thread that called `game::init`; objects are `!Send`). Per window frame:

```rust
game::init(window_state);                       // once: device, GraphicsDeviceManager, window, ContentManager, services
// translated: new Main(), Main.Initialize(), Main.LoadContent()

let n = game::begin_frame(input_snapshot, window_state, elapsed_seconds);   // 1. input, window, Game.Tick timing
game::dispatch_events();                        // 2. ClientSizeChanged / Activated / Deactivated, typed characters
while let Some(game_time) = game::begin_update() {                          // 3. n times (0..)
    main_update(game_time);                     //    translated Main.Update(gameTime)
    game::end_update();                         // 4. advances game time, runs queued "thread" work
}
let draw_time = game::begin_draw();             // 5. None: this tick draws nothing
if let Some(game_time) = draw_time { main_draw(game_time); }               //    translated Main.Draw(gameTime)
let out = game::end_draw(draw_time.is_some());  // 6. Present; ALWAYS call it
//   out.frame          -> replay (gfx::Frame; serial increases by one per call)
//   out.audio          -> mixer
//   out.title, out.is_mouse_visible, out.window_request, out.mouse_position_request, out.exit
// between frames, never under translated code: rt::collect()
// at shutdown: game::raise_exiting()
```

- `begin_frame` is `Game.Tick` (reference/xna/Game.cs:312): elapsed time clamped to 0..500 ms; fixed step: within
  1/64 of `TargetElapsedTime` counts as exactly the target, the accumulator gives the number of updates (0 means
  no draw either), `IsRunningSlowly` after two slow ticks within 20 updates; variable step: one update with the
  real elapsed time. A window whose size differs from the back buffer resets the device to the new size.
- `InputSnapshot` (`input.rs`): `Keys` values that are down, mouse position in client pixels, cumulative wheel,
  five buttons, typed characters, up to four game pads. `WindowState` (`game.rs`): client size and position,
  focus, native handle, monitor size, DPI scale.
- `out.window_request` is what `GraphicsDeviceManager.ApplyChanges` asked for (size, full screen, vsync). The
  device already has that size; the app should make the window match and report the real size next frame.
  `game::set_window_follows_apply_changes(true)` makes `Form.ClientSize` the new size at once, as XNA resizes
  its form inside `ApplyChanges` (without it the game takes its own request back in the next `Update`: see the
  function's docs; off by default, which is what the scripted runs were recorded with).
- `winforms::report_window(maximized, borderless)`, `report_minimized(bool)` (`Form.WindowState`),
  `set_foreground(Option<bool>)` (`Form.ActiveForm`: the system's foreground window; `None`: `Game.IsActive`
  stands in): what the game's own focus code reads (`FocusHelper.UpdateFocus`).
- `terraria-app\src\bin\host_demo.rs` is a complete driver (without translated code).

## Graphics

Rules are taken from the decompiled XNA assemblies (`reference\xna\*.cs`); each module names its source.

- `SpriteBatch`: every `Begin` and `Draw` overload, vertex arithmetic of `PlatformRenderBatch`, texture runs,
  custom effect passes per run, the nesting rules of `Immediate`. Sort modes use `Array.Sort`'s .NET 4.0
  quicksort (not stable; checked against real .NET). `SpriteSortMode.Texture` orders by descending texture id
  where XNA orders by native pointer value (undefined for a game either way).
- Render targets: binding the bound target is a no-op; a `DiscardContents` target (and the back buffer after
  every `Present`) is cleared to the discard colour (68, 34, 136) before the first draw, on a target switch or at
  `Present` unless `Clear` was called; `PreserveContents` keeps pixels across switches; binding resets the viewport.
- State objects: XNA's defaults and presets; a bound state object is read-only.
- Direct3D 9's half-pixel offset is deliberately absent (wgpu's pixel centres make the same texel mapping without it).
- `Texture2D.GetData` is synchronous in XNA. It is served from a CPU copy (textures the game creates, `FromStream`,
  textures from ReLogic's XNB hook), by re-reading the content file (content textures), or, for a render target,
  by `GraphicsDevice::readback_hook` (replay the partial frame now and read the pixels). `readback.rs` fills the hook
  from a `ReadbackBackend` (`install_sync`) and has the request / response variant (`Command::Readback`, answer one
  frame later) with a CPU fake backend; **no real backend installs the hook yet** (`render.rs` is another worker's),
  so `GetData` on a render target stays `unimplemented_value` in the app. Every real call site (the three in
  `CaptureCamera`, screenshots) needs the pixels just drawn, so the synchronous path is the one that matters;
  `HorizonHelper:103` is debug-only. Call-site table, the contract of the hook and the wgpu change:
  `docs/readback.md`. Screenshots are written by `System.Drawing.Bitmap.Save(path, ImageFormat.Png)` (`sys/drawing.rs`,
  PNG via the `png` crate); `Texture2D.SaveAsPng` exists in `image.rs`, `SaveAsJpeg` and JPEG in general do not (never called).
  `SetData` callers: `LiquidRenderer` wave mask, `MapRenderer` map sections, `WaterShaderData`, `BasicDebugDrawer`, `PngReader`.

### Shader gap (the one thing drawn wrong on purpose)

**Effects are not executed.** `EffectPass.Apply()` records an `EffectSnapshot` (effect, technique, pass name and
index, every parameter value, `Textures[1..3]` with their samplers) into each draw; the replay draws with the
plain sprite shader (texture x vertex colour). So today: dyes, hair dyes, misc trail shaders, water / heat /
screen filters, the tile shader and anything else that sets an effect render as plain sprites.

What exists: the three compiled effects load (`PixelShader.xnb`, `TileShader.xnb`, `ScreenShader.xnb`); their
techniques, passes and parameters are read out of the D3DX effect binary (`gfx::fx_binary`, tested on the real
files): `PixelShader` one technique with the armor / hair / misc passes ("Default", "ColorOnly", "ArmorColored",
... ; the test prints the list), `TileShader` `Technique1` with `Pass1`..`Pass7`, `ScreenShader` one technique
with 27 passes ("FilterMoonLord", "FilterHeatDistortion", "WaterDebugDraw", ...) and 26 parameters. The game
selects passes by name in 157 places (`GameShaders.Armor / Hair / Misc`, `Filters.Scene`).

What is needed: translate each pass's HLSL bytecode (ps_2_0 / ps_3_0, a few vs_2_0) to WGSL, one pipeline per
(effect, pass), fed from the snapshot; the vertex stage of passes with a vertex shader takes `MatrixTransform`.
Until then user primitives under such an effect are positioned by this crate's reading of that parameter
(`gfx::device` module docs), which is not an XNA rule.

Also not replayed: depth / stencil (no depth buffer: only observable with overlapping sprites of different
`layerDepth` under a depth-testing state), `FillMode`, MSAA, `Clear` under a partial viewport, more than one
vertex stream, effect-less draws with `Textures[0] == null`.

## Content

`ContentManager.Load<T>` for `Texture2D`, `Effect`, `SoundEffect` and font data, from
`<TERRARIA_DIR>\<RootDirectory>\<name>.xnb` (default install `C:\Users\user\Documents\Games\Terraria`), cached by
name. The install folder is read-only: nothing here writes, and `sys::io` refuses writes below it. (The runtime's
own `rt::io::Stream::open_file` has no such guard: see REQUESTS.md.)

ReLogic's asset system (`Asset<T>`, `AssetRepository`, sources, `DynamicSpriteFont`) is translated code. Its
platform edge is a **direct native hook**, not XNA's `ContentReader` / `ContentTypeReader` plumbing (which is
reflection over reader type names and a second object model for no gain): `XnbReader.FromStream<T>` ->
`content::xnb_from_stream_{texture,effect,sound,font}`, `TitleContainer.OpenStream`, recursive `GetFiles`,
`Texture2D.FromStream` (PNG, not premultiplied, as XNA) for `PngReader`. The font comes back as `FontData`
(REQUESTS.md: the translated side has to build its `DynamicSpriteFont` from it).

## Input

`input.rs`: `Keyboard.GetState`, `KeyboardState` (256 bits, `GetPressedKeys` ascending; a value that is not a
member of `Keys` is never down, as XNA's `stateMask`), `Mouse.GetState`, `Mouse.SetPosition` (recorded for the
app, which decides whether the real pointer moves). `Keys` is generated from the decompiled enum.

Game pads follow the decompiled XNA (`reference\xna\GamePad*.cs`, `XINPUT_*.cs`): the snapshot carries the raw
XInput state per player index (`GamePadInput`: button mask, trigger bytes, 16-bit stick values, packet number) and
`GamePad.GetState(index[, deadZoneMode])` builds the `GamePadState` from it with `GamePadDeadZoneUtils`' numbers
(`input::dead_zone`: left stick 7849, right stick 8689, triggers 30; `IndependentAxes` / `Circular` / `None`).
`IsButtonDown` derives the thumbstick-direction and trigger "buttons" from the raw values, as XNA. Also
`GamePad.GetCapabilities` (a connected pad counts as a full XInput pad) and `GamePad.SetVibration` (true when
connected; the request is recorded for the app, `take_vibration_requests`). `PadAdapter` / `PadEvent` build the
raw state from single changes (a named button, a stick axis in -1..1, a trigger in 0..1): the app's gamepad
events and an input script's `pad` lines both go through it. The game itself calls only `GamePad.GetState(index)`,
`IsConnected`, `IsButtonDown`, `ThumbSticks` and `Triggers`; the other members have no mapping line in
`tools/coverage.py` yet because nothing translated reaches them.

Text input: `IImeService` key listeners get the snapshot's characters as UTF-16 units. `os::ime_enabled()` tells
the app when the game wants the input method on (`IImeService.Enable`), `os::set_ime_composition` feeds
`CompositionString`; there is no candidate list. Bevy adapter: `terraria-app\src\host\input.rs` and
`src\shell\native.rs` (the window's own `WM_CHAR` / key messages where there is a window).

## Audio

`SoundEffect` (XNB PCM 8/16 bit and MS ADPCM, `FromStream` for RIFF WAVE), `SoundEffectInstance` (state machine
and argument rules from the decompiled class), `DynamicSoundEffectInstance` (`SubmitBuffer`, stream voices).
`State` is decided by the host from elapsed time, not by the backend.

Backend (`terraria-app\src\host\audio.rs`): a small mixer thread writing to Windows `waveOut` through winmm,
declared by hand. No audio crate, no Bevy audio feature (it would have pulled rodio / cpal and a second asset
path for sounds the host already holds). Volume, pitch (resampling), pan, looping, stream voices. The pan law is
FNA's, not verified against XNA's native mixer. `TERRARIA_MUTE=1` mixes without a device.

XACT (the legacy music path, `LegacyAudioSystem`: `Music_<n>` cues played through `CueAudioTrack`):

- `Sound Bank.xsb` (104 cues, all "simple": cue -> sound -> one wave of the one wave bank), `Wave Bank.xwb` (104 MS ADPCM
  stereo entries, 1.8 GB of PCM, 171 minutes) and `TerrariaMusic.xgs` (3 categories `Global` / `Default` / `Music`, 8 variables,
  one RPC curve) are parsed (`audio/xact.rs`; formats read from the real files and FAudio's `read_volbyte`). A sound's volume
  byte is -11.94 dB, the `Volume` variable (0..1) runs the RPC curve -25 dB .. +6 dB: exactly the constants
  `ASoundEffectBasedAudioTrack.ReMapVolumeToMatchXact` hard-codes. Complex sounds, DSP presets and interactive cues
  are not parsed (a cue needing them throws `InvalidOperationException` with the reason; Terraria's bank has none).
- `Cue`: `SetVariable("Volume", v)` before `Play()` is remembered (the game's order) and applied at the start;
  `Play`/`Pause`/`Resume`/`Stop`/`IsPlaying`/`IsPaused`/`IsStopped`/`IsPrepared` follow the voice state; a stopped or finished cue cannot be
  played again (`Reuse()` gets a new one); `AudioEngine.GetCategory(name).SetVolume(v)` scales the cues of the category and its children.
  Pitch / pan / 3D and `AsAuthored` release are not modelled (the bank has none of them). Looping is the game's
  restart-when-stopped logic (simple sounds have no loop event).
- **Decode when a cue first plays, not streaming.** Measured on the real bank: 104 tracks, 34 s to 212 s each, largest track 37.5 MB
  of PCM, all 104 decoded in 6.2 s (slowest 152 ms), 1.8 GB if all were kept. So a track is decoded in full at `Play()` (20 - 150 ms
  on the game thread: one dropped frame at most), shared with the mixer as an `Arc<Sound>`, and only the last 2 decoded waves stay cached.
  Streaming would bound memory at a few seconds of PCM but makes the cue depend on the frame cadence of the driver; with 40 MB per track it is not needed.
- `examples/host_audio_demo.rs` (no Bevy: it includes the app's mixer file by path) plays a cue for up to 5 s through the host's XACT path:
  `TERRARIA_MUTE=1 host_audio_demo Music_1 5` mixes without a device and prints peak and RMS (`Music_1`: peak 14751 = 45 %, RMS 2424 = 7.4 % of full scale,
  `Play()` 38 - 60 ms); `TERRARIA_VOLUME=0.03` opens the default device at 3 % (peak 442, RMS 73.1, as 3 % of the muted run). It refuses to open a device above 3 %.

The newer path (`Terraria.Audio`: `OGGAudioTrack`, `MP3AudioTrack`, resource-pack music) needs `NVorbis.VorbisReader` and
`MP3Sharp.MP3Stream`: `audio/codec.rs`. **Crates added:** `lewton` 0.10 (pure Rust Vorbis; with `ogg` 0.8) and `symphonia` 0.5.5 with only the
`mp3` feature (`symphonia-core`, `-bundle-mp3`, `-metadata`, plus `encoding_rs`, `core_detect`, `multiversion_no_op`); `miniz_oxide` 0.8 was already in the lock file
(through `png`). A first MP3 choice, `puremp3`, decoded these test files with different waveforms and was dropped. Streams are read into memory, PCM is produced
packet by packet. `VorbisReader`: `Channels`, `SampleRate`, `TotalSamples`, `TotalTime`, `ReadSamples(float[], offset, count)`, `DecodedPosition` get/set (exact,
by decoding forward from the start: tens of ms for a loop point seconds in), `SeekTo`, `Tags.All` (upper-case keys: `LOOPSTART` / `LOOPEND`).
`Mp3Stream`: `Frequency`, `Read(byte[], ...)` (16-bit stereo; a mono file is duplicated; MP3Sharp's quirk of decoding the Xing / Info frame as silence is reproduced so
byte counts match), `Position` (file bytes consumed; set 0 rewinds), `Length`. Tests (`audio/xact_tests.rs`) compare with the game's NVorbis.dll / MP3Sharp.dll
on ffmpeg-made fixtures (`tests/data`): channels, rate, total samples, tags, float counts, positions after seeks identical; Vorbis samples within 2e-3 and RMS within 1e-4 of NVorbis;
MP3 byte counts identical and samples within 9 of 32768 over the whole file (not bit-exact: different floating-point code). `DynamicSoundEffectInstance`
(`SubmitBuffer`, stream voices) was already in the mixer; `PendingBufferCount` and the `BufferNeeded` event are not provided (`Update()` of the game's track polls `PendingBufferCount < 8`: use `pending_seconds`).

## System.*

- `sys::io`: `Path` (checked line by line against .NET), `File`, `Directory`, `FileInfo`, `StreamReader`,
  `StreamWriter`. Streams and binary readers / writers are the runtime's (`rt::io`).
- Save root: `Main.SavePath` comes from `IPathService.GetStoragePath("Terraria")` -> `TERRARIA_SAVE_DIR`, default
  `<project>\work\port-save`. `Environment.GetFolderPath` answers inside `<save root>\_folders`. The real
  `Documents\My Games\Terraria` is never touched. `MoveToRecycleBin` moves into `<save root>\_recycled`.
- `sys::time`: `DateTime` (.NET layout, `ToBinary`, formats `D`, `d` and custom patterns, `ToShortDateString` / `ToLongDateString`, culture-aware: the 12 `GameCulture`s + invariant, data generated from the real
  .NET Framework by `tools/gen_cultures.ps1` into `sys/cultures.rs`, genitive month names as .NET picks them; the runtime's current culture, `rt::sys::CultureInfo`, selects one), `Stopwatch`,
  `Environment.TickCount`. `TimeSpan` is the runtime's. 1,235 formatted values (13 cultures x 5 dates x 19 formats) are checked against .NET (`culture_probe.ps1`). The game uses `ToString("D")`, `ToShortDateString()`
  (non-English `UIWorldListItem`, `Player.cs:39596`) and fixed invariant patterns for file names.
- `sys::crypto`: AES-CBC as `RijndaelManaged` + `CryptoStream` do it for player files, checked against
  ciphertexts made by .NET. A write-mode `CryptoStream` encrypts at `FlushFinalBlock` only (the game calls it for
  player files; `AchievementManager` relies on `Dispose`: REQUESTS.md).
- `sys::regex` over `sys::regex_engine`, a backtracking engine with .NET's semantics (look-behind and look-ahead, lazy quantifiers, named groups numbered after the unnamed ones, inline options,
  back references; see its module docs for the unsupported rest: `RightToLeft`, conditionals, balancing groups, class subtraction, `\G`), replacement strings with .NET's rules, `Split` with captures. The `regex` crate is no longer used for it.
  `tests/dotnet/regex_probe.ps1` runs 30 patterns (the game's 12 including `ChatManager.Regexes.Format`, 18 more) x 87 inputs through real .NET: 2,610 cases compared match by match, group by group (index, length, value), plus `Replace` and `Split`: all identical.
- `sys::text`: encodings, `StringBuilder`, base64, `Uri` escaping, the `X2` / `F2` / `P0` formats,
  `Math.Round(double, int)`, `Math.Log(a, base)`.
- `sys::misc`: console, environment, `Guid`, `Process` (never starts anything), the registry (holds nothing),
  `System.Random` = `terraria_core::UnifiedRandom` (checked against .NET).
- `json` (Newtonsoft.Json 10, the game's own dll as reference): `reader.rs` (the `JsonTextReader` rules: comments, trailing and missing commas, single-quoted strings, unquoted names, hex integers,
  `BigInteger`, ISO and `/Date()/` dates, the exact messages with path, line and position), `writer.rs` (indent / escaping / `FloatFormatHandling`), `number.rs` (`double.ToString("R")` of the .NET Framework,
  `-0.0` -> `0.0`, `EnsureDecimalPlace`), `token.rs` (the `JToken` family as one heap type; the two load paths of Newtonsoft differ for comments and duplicate keys), `convert.rs` (`ToObject<T>`, explicit
  conversions, `JsonConvert`, `JsonSerializerSettings`, BSON serializer), `bson.rs`. Tests: 52 edge cases (input, indented, compact and serializer-path output, errors included), **all 87 embedded JSON
  resources** byte-identical (SHA-256 of the indented and compact output against the dll), 162 `ToObject` conversions (18 JSON values x 9 target types incl. exceptions), BSON bytes of 5 documents.
  REQUESTS.md section 10: what the generator emits for reflection over game classes (member tables) and where the game uses them.
- `sys::csv` (CsvHelper 2.0 `CsvReader`: the one use is `LanguageManager.LoadLanguageFromFileTextCsv`; 18 inputs identical to the dll, including its padding of short records with nulls),
  `sys::zip` (Ionic.Zip.CF read-only: `ZipFile.Read`, entries, `Extract`, CRC check; `Ionic.Zlib.DeflateStream` over `miniz_oxide`, `ZlibCodec.Inflate`; tested against a zip read by the dll and deflate / zlib
  streams made by it, and Ionic's `DeflateStream` inflates what ours deflates), `sys::drawing` (`Point` / `Size` / `Rectangle`, `Bitmap` + `LockBits` + `Marshal.Copy` + PNG `Save`: the screenshot path),
  `halfvec` (`HalfVector2`, XNA's half conversion; unchecked against the XNA binary, which is not installed), `winforms` (Form / Control / Screen / MessageBox over the host window state; the app takes
  `WindowCommand`s each frame and feeds monitors: `winforms::set_monitors`, `take_commands`; **the Bevy-side application of the commands is not written yet**), `pinvoke` (the game's user32 / kernel32 /
  nfd P/Invokes, Steamworks and the RGB vendors' DLLs: answers and the source line that handles each; the port starts in `SocialMode.None`, REQUESTS.md section 11), `os::set_os_clipboard` (Windows clipboard,
  text only, opt-in; `TERRARIA_TEST_OS_CLIPBOARD=1` runs its round-trip test and restores the text).
- **Not here**: `ReLogic.Content` (the asset system is translated ReLogic code), DPI / monitor discovery in the Bevy adapter, any process launching (`Process.Start`, `OpenURL`), JPEG, a real Steam / Rail / peripheral SDK.

### Threading model

One thread. `Thread.Start`, `ThreadPool.QueueUserWorkItem`, `Task.Factory.StartNew` queue the work; it runs on
the game thread in `game::end_update` or on demand (`Task.Wait`, `Task.IsCompleted`, `Thread.Join`).
`sys\threading.rs` lists the affected code paths: world load / save / generation give correct results but block
the frame (no animated progress screen). On Windows every queued work item runs on a FIBER of the game thread:
a wait loop on `Thread.Sleep` (a worker waiting for the main thread: `WorldGen.playWorldCallBack` waits for
`Main.DoDraw` to draw the loaded map) suspends the work after 100 sleeps, the frame goes on, and the work
continues at the next pump. The driver must not collect garbage while work is suspended
(`threading::has_suspended_work()`): the suspended frames hold handles the collector cannot see.
`RunOnMainThread(..).Wait()` from queued work is still served by running the main thread's action queue at the
wait. Since the networking stage the collector MAY run while work is suspended: `threading::suspended_stacks`
gives it the stack memory of every suspended fiber (recorded at each switch, callee-saved registers included) and
the driver installs it with `rt::set_conservative_roots`.

The game's endless threads (`Netplay.ServerLoop`, `InnerClientLoop`, `TcpSocket.ListenLoop`,
`Netplay.BroadcastThread`) are fibers like any other work (threading.rs, "Threads that never end"): each sleeps in
every iteration; a fiber of a `Thread` object that sleeps a millisecond or more is suspended at once and counts as
idle (it does not hold a script's clock: `has_suspended_foreground_work`), a sleep of 16 ms or more is honoured in
real time, `Thread.Abort()` deletes a suspended fiber. Steam callback threads do not exist (`SocialMode.None`).

### Networking (`sys/net.rs`)

`TcpClient`, `Socket`, `NetworkStream`, `TcpListener`, `UdpClient`, `IPAddress`, `IPEndPoint`, `IPHostEntry`, `Dns`:
the members `Terraria.Net.Sockets.TcpSocket`, `DebugNetworkStream`, `Netplay` and `TcpAddress` call, on
non-blocking OS sockets. `BeginRead` / `BeginWrite` register an operation; `net::pump()` moves every pending one
as far as its socket allows; the `AsyncCallback` of a finished one is handed out by `net::deliver` and runs as a
work item: queued by `threading::run_pending_work` (once per `Update`), or inline when a thread of the game sleeps
(`Thread.Sleep` on a fiber: the client loop then reads as fast as data arrives, as with the I/O threads of the
original). `AcceptTcpClient` and `Connect` give the frame back while they wait (`threading::wait_a_frame`); the OS
connect runs on a helper thread. `Socket.Available` answers 0 on a reset connection (the game's "is it still
connected" test depends on that).

What listens where: exactly what the game asks for: `IPAddress.Any` unless the launch parameter `-ip <address>`
is given (TcpSocket.cs `StartListening`), port 7777 unless `-port`. A real "Host & Play" therefore listens on
every interface, as the original does, and `Netplay.BroadcastThread` announces the server on the LAN (one UDP
datagram per second to 255.255.255.255:8888). **`TERRARIA_NET_LOOPBACK_ONLY=1`** (set by every automated test):
a listener on an unspecified address is bound to 127.0.0.1 and the announcement is not sent; the server started by
Host & Play gets `-ip 127.0.0.1`. `TERRARIA_HOSTPLAY_PORT=<n>`: that server gets `-port n` and the client's
connection to 127.0.0.1:7777 goes to n (the game has no port choice for Host & Play).

"Host & Play" starts `TerrariaServer.exe` in the original (`Main.HostAndPlay`). `Process.Start()` of that name
starts the running executable with `-server` in front of the game's arguments, without a console window, output
in `server-last-run.log` of the save root (`sys/misc.rs`); the app implements `-server` (`terraria-app/src/server.rs`)
with `threading::set_blocking_main`: `Thread.Sleep` on the main thread of `Main.DedServ` runs the queued work, the
collector, and really sleeps. `Process.PriorityClass` (the server asks for `High`) is deliberately not applied.

Tests: a loopback round trip with a 4 MB write, ordered writes, end of stream, errors after `Close`
(`loopback_round_trip_with_queued_callbacks`), addresses / names / a refused connection, the loopback-only mode.
Not here: IPv6 beyond what `std::net` gives, `IPAddress.TryParse` of the shortened IPv4 forms, socket timeouts
(the game sets none), `HttpWebRequest` (SteelSeries GameSense), UPnP (`Netplay.OpenPort` is empty in this version).

## Mapping tables

`tools\coverage.py` holds ONE table (member key -> Rust template + status). It writes `COVERAGE.md` and
`native.d\host_<area>.json` in this crate. `python tools/coverage.py --install` copies the tables to
`tools\cs2rs\native.d`.

They are **not installed**: a template that does not compile stops `terraria-gen` from building, and that crate
was being regenerated by its owner while this was written. What was checked: a private cs2rs run (config copied
to a scratch folder, output there) accepted all seven tables with no duplicate or unknown key, and HOST_API went
from 411 members / 3398 call sites to 140 / 1449. What was not: `cargo check -p terraria-gen` with them. Known
before trying: `Asset<T>.Value` still returns `Obj` where the mapped `SpriteBatch.Draw` takes `Gc<Texture2D>`
(REQUESTS.md).

## Tests

`cargo test -p terraria-host` (`CARGO_TARGET_DIR=target-host2`, `-j 3`): 87 tests. Expected values come from the decompiled
XNA sources or from the real libraries (`tests\dotnet\*.ps1`, run with Windows PowerShell 5.1 and `-ExecutionPolicy Bypass`; the game's own
Newtonsoft.Json, NVorbis, MP3Sharp, CsvHelper and Ionic.Zip.CF dlls from `meta\refs` plus .NET Framework itself; outputs are checked in next to them:
`json_*.expected`, `bson_cases.expected`, `regex_*.expected`, `codec.expected`, `csv.expected`, `zip.expected`, `deflate.expected`, `datetime.expected`). Fixtures in `tests\data`.
Tests that need the player's install skip with a note when it is missing.

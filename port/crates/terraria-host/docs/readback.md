# `Texture2D.GetData` on render targets and screenshots

Status: the host half is written and tested with a fake backend (`src/readback.rs`, `src/image.rs`,
`src/sys/drawing.rs`). The wgpu backend (`terraria-app/src/gfx/render.rs`) does **not** serve readbacks yet: it logs
once when it meets `Command::Readback` and `GetData` on a render target stays `unimplemented_value`. This file is the
exact change for whoever owns `render.rs`.

## 1. Every `GetData` / image call site of the game

| site | what it reads | when | latency it can take |
|---|---|---|---|
| `Graphics/Capture/CaptureCamera.cs:202` `_scaledFrameBuffer.GetData(_scaledFrameData, 0, w*h*4)` | the `RenderTarget2D` it drew into **two statements earlier** (`SetRenderTarget(_scaledFrameBuffer)`, `Clear`, sprite batch draw, `SetRenderTarget(null)`) | once per 128x126-tile chunk of a world / area screenshot (user-triggered, seconds long, `Main.GlobalTimerPaused`) | **none**: the bytes are copied into the output image in the next statement |
| `CaptureCamera.cs:329` and `:352` (`SaveImage(Texture2D ...)`, FNA and Bitmap branches) `texture.GetData(_outputData, 0, w*h*4)` | `_frameBuffer`, just drawn by `Main.DrawCapture` | once per chunk when `UseScaling` is off | **none** |
| `GameContent/Drawing/HorizonHelper.cs:103` `_pixelTarget.GetData(array)` | the 1x1 sun-visibility target | only in `Test_DrawSmallTarget`, behind the constant `DebugSunVisibility` (false) | never runs |
| `ReLogic/Content/Readers/PngReader.cs:36` `texture2D.GetData(value, 0, num)` | a texture from `Texture2D.FromStream` | asset load | none; **served from the CPU copy** (`content::texture_from_stream`) |
| `Terraria.GameContent.UI.States.AWorkshopPublishInfoState.cs:733` `Texture2D.FromStream` | a PNG preview | workshop UI | none; `content::texture_from_stream` |

`TilePaintSystemV2`, `Main.DoDraw` map / mini map and the other render-target users **only draw into or from** targets;
none of them reads pixels back (decompile grep, 2026-10: `GetData` has exactly the calls above). `SetData` callers are
unrelated to this file (`LiquidRenderer`, `MapRenderer`, `WaterShaderData`, `BasicDebugDrawer`, `PngReader`).

Screenshot files are written by `System.Drawing.Bitmap.Save(path, ImageFormat.Png)` (`CaptureCamera.SaveImage`, the
non-FNA branch the XNA build takes; `PlatformUtilities.SavePng` throws `NotSupportedException` and is only reached
when `Program.IsFna`). `Texture2D.SaveAsPng` / `SaveAsJpeg` are never called; JPEG is never requested (`ImageFormat.Png`
is the only value passed).

## 2. Decision

**The call sites that matter (screenshots) cannot take one frame of latency**: a "frame N requests, frame N+1 has the
data, return the previous frame's contents" policy would put the *previous chunk's* pixels into the current chunk of
the output image. So the primary path is **synchronous**: `GetData` ends the frame recorded so far, the backend
replays it immediately and reads the target back before `GetData` returns.

- Host side, done: `GraphicsDevice::pixels()` already calls `readback_hook(frame, target_id)` after `take_frame()`;
  `readback::install_sync(device, backend)` fills that hook from a `ReadbackBackend`.
- The asynchronous protocol (`Command::Readback` / `DrawList::deliver_readback`, `readback::request / latest /
  stale_or_request / serve`) is implemented and tested too, for a caller that tolerates a frame of latency. Nothing in
  the game is such a caller today, so the backend may leave it out; it is the better fit if a future mod-like consumer
  polls a target every frame.

Costs of the synchronous path: one `device.poll(Wait)` per `GetData` plus a texture-to-buffer copy (a
2048x2048 RGBA8 target is 16 MB); a screenshot of a large world is a few hundred chunks, each one GetData. The game
already pauses its timers for the capture.

## 3. Contract of the hook

```
readback_hook: Option<Box<dyn FnMut(Frame, TextureId) -> Vec<u8>>>      // terraria_host::gfx::GraphicsDevice
```

- `frame` is a *partial* frame: it holds everything recorded since the last `take_frame()` (serial N, N+1, ...: one
  per `GetData` plus one per `present()`), it is **not presented**, and the commands continue in the next frame
  (render targets keep their contents; `DiscardContents` clearing happens at `SetTarget`, as recorded).
- Return value: the RGBA8 rows of `TextureId`, **top row first**, `width * height * 4` bytes, premultiplied exactly as
  stored (no conversion; `CaptureCamera` copies them into a `Format32bppPArgb` bitmap, which wants BGRA bytes: the game
  swaps R and B itself in its copy loop).
- The size check (`want != p.len()` -> `ArgumentException`) is done by `get_data_range`.

## 4. The backend change (terraria-app, `src/gfx/render.rs` + the host adapter)

`render.rs` today: `publish_draw_list` (main world, `Last`) pushes `list.take_frame()` into `PublishedFrames`;
`extract_frames` moves them into the render world; `replay_frames` replays them in `RenderSystems::Queue` (render
thread, pipelined). A hook called from the game system (`Main.Update/Draw`) runs on the main thread, in another
world. Needed:

1. **Share the GPU state.** Make `GfxGpu` reachable from both sides: `Arc<Mutex<GfxGpu>>` held by a resource of
   each world (created in `GfxPlugin::build` before `RenderApp` exists), plus the wgpu `Device` / `Queue` (clone them
   out of `RenderDevice` / `RenderQueue` once; both are `Send + Sync`). `replay_frames` locks the mutex for the frames
   it replays.
2. **One ordered queue.** The hook must replay *after* every frame already published and *before* the frames to come.
   So the hook (a) drains `PublishedFrames` itself (the same `Mutex<Vec<Arc<Frame>>>` the extract system drains; whoever
   takes a frame replays it, so nothing is replayed twice), (b) replays those frames in order, (c) replays its own
   partial frame, (d) reads the target. Replaying means calling `replay_frame(&frame, &mut gpu, back_view, device,
   queue)` with `back_view = None` (a partial frame never needs the swap-chain image; `Option` is already the
   parameter type).
3. **The read.**

```rust
// render.rs, next to replay_frame
pub fn read_target(gpu: &GfxGpu, device: &wgpu::Device, queue: &wgpu::Queue, id: TextureId) -> Vec<u8> {
    let t = gpu.textures.get(&id).expect("a target is created by its CreateTexture command before it is read");
    let bpr = (t.width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"), size: (bpr * t.height) as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
    let mut enc = device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        t.texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(bpr), rows_per_image: Some(t.height) } },
        wgpu::Extent3d { width: t.width, height: t.height, depth_or_array_layers: 1 });
    queue.submit([enc.finish()]);
    buf.slice(..).map_async(wgpu::MapMode::Read, |r| r.expect("map"));
    device.poll(wgpu::PollType::Wait).expect("poll");          // the exact variant name depends on the wgpu version (29.x: check)
    let data = buf.slice(..).get_mapped_range();
    let mut out = Vec::with_capacity((t.width * t.height * 4) as usize);
    for row in data.chunks(bpr as usize).take(t.height as usize) { out.extend_from_slice(&row[..(t.width * 4) as usize]); }
    out
}
```

   The target texture must have `COPY_SRC` in its usage (`new_target_image` / `CreateTexture` for `target: true`).
   The back buffer (`TextureId(0)`) is the `Image` of `BackBufferImage`; reading it needs the same `COPY_SRC` and its
   `GpuImage`; the game never reads it.
4. **Install the hook** where the host device is created (the driver, after `game::init`):

```rust
let rd = Rc::new(RefCell::new(WgpuReadback { shared: shared_gpu.clone(), published: published.clone(), device, queue }));
terraria_host::readback::install_sync(terraria_host::game::graphics_device(), rd);   // impl ReadbackBackend for WgpuReadback
```

   `impl ReadbackBackend for WgpuReadback { fn read_target(&mut self, frame: Frame, id: TextureId) -> Vec<u8> { drain; replay; read_target(...) } }`.
5. **Async path (optional).** In `replay_frame`, the `Command::Readback { id, request }` arm currently warns. Replace it
   with: after the frame's last command, `read_target(id)`, send `(request, id, pixels)` through an mpsc channel; a
   main-world system in `First` / `Last` drains the channel into `DrawListRes.0.deliver_readback(request, id, pixels)`.
   That is `readback::serve` split across the thread boundary; the data is then visible to the game at the next frame.

## 5. Tests

- `src/readback.rs`: the plumbing against `CpuBackend` (executes `CreateTexture`, `Upload`, `SetTarget`, `Clear`;
  counts draws): a `GetData` after `SetRenderTarget(rt); Clear(c); SetRenderTarget(null)` returns `c`, the backend saw
  one partial frame per call with consecutive serials, the later `present()` frame does not repeat the handed-over
  commands; the request / response path returns frame N's contents during frame N+1.
- `src/sys/drawing.rs`: `CaptureCamera.SaveImage(data, w, h, ImageFormat.Png, path)` end to end (BGRA premultiplied ->
  `Bitmap` -> PNG with straight alpha), below the save root only.
- `src/image.rs`: `SaveAsPng` -> `FromStream` round trip.
- Backend side, once implemented: render a known sprite into a target in a hidden window (`TERRARIA_HIDDEN=1`) and
  compare the returned bytes with `CpuBackend`'s expectation for the same commands (solid fills and clears are exactly
  comparable; filtered sprites need a tolerance).

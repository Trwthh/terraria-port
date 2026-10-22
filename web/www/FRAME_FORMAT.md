# Frame format: what `game.wasm` hands the WebGPU replayer each frame

Version 2 (2026-10-13). Version 2 adds frames sent in parts (`gfx_frame_chunk`), `n_commands = 0xFFFFFFFF` and op 0 End. Part of the host ABI (`HOST_ABI.md`). Producer: `port/crates/terraria-web/src/frame.rs`.
Consumer: `web/www/gfx.js`. The reference for every rendering rule is the native replayer
`port/crates/terraria-app/src/gfx/render.rs` (and the sprite pipeline in it): the web replayer must produce the
same pixels. Everything Direct3D-specific is resolved on the Rust side (effect files, shader translation to WGSL,
parameters -> registers, which texture goes on which sampler unit), so the replayer only creates WebGPU objects and
draws.

## Import

Two imports, both synchronous. The bytes are only valid during each call.

| Import | Meaning |
|---|---|
| `tw.gfx_frame_chunk(ptr: i32, len: i32) -> ()` | Appends the bytes to the frame being received. The host copies them out of wasm memory at once, so the module may reuse its buffer. |
| `tw.gfx_frame(ptr: i32, len: i32) -> ()` | Appends the bytes as the last part, then replays the whole frame. Once per presented frame (after `Game.EndDraw`). |

A frame may be a single `gfx_frame` call, as in version 1, or any number of `gfx_frame_chunk` calls followed by one `gfx_frame`.

How the parts are read:
- They are read as one byte stream and never joined into one buffer.
- The producer cuts only at command boundaries; the first part holds the header and the vertices.
- The reader nevertheless works for any cut, including inside a large Upload.

**Why.** A frame can carry hundreds of MB of texture uploads during the content load. Building it as one `Vec` in wasm memory runs out of memory: at 1.18 GB of heap, the `Vec` doubling asked for 1 GiB more.

The replayer executes the commands in order: an upload between two draws is seen by the second one only. It then presents the back buffer (texture id 0) on the canvas.

## Encoding

Little-endian. `u8/u16/u32/i32/f32/f64` as named. `str` = `u32` byte length + UTF-8 bytes. No padding anywhere.

```
header:
  magic       4 bytes  "TWF1"
  serial      f64      frame serial (integer valued); a serial already replayed is ignored
  back_w      u32      back buffer size in pixels (texture 0 is (re)created at this size when it changes)
  back_h      u32
  n_vertices  u32      sprite vertices, 20 bytes each: pos f32x2 (clip space), uv f32x2, color u8x4 (RGBA, Unorm)
  n_raw       u32      raw vertices, 36 bytes each: pos f32x4, tex f32x4, color u8x4 (only for passes with a vertex shader)
  n_commands  u32      the number of commands, or 0xFFFFFFFF: commands until op 0 (End). The module always sends 0xFFFFFFFF
vertices      n_vertices * 20 bytes
raw_vertices  n_raw * 36 bytes
commands      each: op u8, then its fields; with n_commands = 0xFFFFFFFF the last one is End, and no byte may follow it
```

All targets and textures are `rgba8unorm` (no sRGB anywhere, as XNA). Texture id 0 is the back buffer.

| op | command | fields | rule (render.rs) |
|---|---|---|---|
| 0 | End | (none) | the end of the commands (required with `n_commands = 0xFFFFFFFF`; an End inside an exact count also stops) |
| 1 | CreateTexture | id u32, w u32, h u32, target u8 | contents transparent black until uploaded / drawn; `target` = can be rendered to (TEXTURE_BINDING, COPY_DST, RENDER_ATTACHMENT, COPY_SRC); an id preloaded by `tw.gfx_texture` with the same size (and not a target) keeps its texture and contents (HOST_ABI.md, `gfx_texture`) |
| 2 | Upload | id u32, x i32, y i32, w i32, h i32, then w*h*4 bytes RGBA rows | `Texture2D.SetData`; ends the current render pass |
| 3 | DestroyTexture | id u32 | |
| 4 | SetTarget | id u32, preserve u8 | `preserve == 0`: the target is cleared to transparent black when bound unless a Clear follows before the first draw (RenderTargetUsage.DiscardContents). The viewport resets to the whole target and the scissor test turns off (XNA's SetRenderTarget) |
| 5 | Clear | r u8, g u8, b u8, a u8 | the whole current target |
| 6 | Viewport | x i32, y i32, w i32, h i32 | |
| 7 | Scissor | on u8, x i32, y i32, w i32, h i32 | `on == 0`: scissor test off (the rect is ignored); on: clamp to the target |
| 8 | Draw | see below | |
| 9 | DefineShader | id u32, label str, wgsl str, units u8 + units * u8 (sampler units the shader uses), ps_regs u32, vs_regs i32 (-1: no vertex shader) | emitted once, before the first Draw that uses it. Bindings: group 0: unit `u` -> texture at binding `2u`, sampler at `2u+1` (fragment); group 1: binding 0 = fragment uniforms (ps_regs * 16 bytes) when ps_regs > 0, binding 1 = vertex uniforms ((vs_regs + 1) * 16 bytes) when vs_regs >= 0. Entry points `vs` and `fs`. Vertex buffer: raw vertices (stride 36: Float32x4, Float32x4, Unorm8x4 at locations 0, 1, 2) when vs_regs >= 0, else sprite vertices (stride 20: Float32x2, Float32x2, Unorm8x4) |
| 10 | Readback | id u32, request f64 | not served yet (logged once), as natively |

Draw (op 8):
```
texture   u32       the draw's texture (sampler unit 0 of an effect, or the sprite texture)
blend     color_src u8, color_dst u8, color_fn u8, alpha_src u8, alpha_dst u8, alpha_fn u8, factor u8x4, write_mask u8
          (Blend and BlendFunction numbering of terraria-host/src/gfx/draw_list.rs; write_mask: R1 G2 B4 A8)
sampler   filter u8, address_u u8, address_v u8   (TextureFilter / TextureAddressMode numbering of draw_list.rs)
cull      u8        0 none, 1 CullClockwiseFace (wgpu Face::Front with front_face Cw), 2 CullCounterClockwiseFace (Face::Back)
topology  u8        0 triangle list, 1 line list
first     u32       first vertex (of the sprite or the raw array)
count     u32
shader    u32       0: the plain sprite shader (render.rs's sprite pipeline); else a DefineShader id
if shader != 0:
  ps_bytes   u32 + bytes   fragment uniform block (exactly ps_regs * 16 bytes)
  vs_bytes   u32 + bytes   vertex uniform block ((vs_regs + 1) * 16; row 0 = (1/vp_w, -1/vp_h, 0, 0) already filled in)
  n_units    u8
  per unit   unit u8, texture u32 (0xFFFFFFFF: "no texture": samples opaque black (0,0,0,255) as Direct3D 9), sampler 3 bytes
```

Rules the producer already applied (the replayer does not re-check them): a draw whose texture is the bound target
is not emitted; a pass that cannot be translated or whose parameters do not fit is not emitted (reported once in the
log); an `EffectSnapshot` without an effect file is drawn with shader 0.

Blend mapping, sampler mapping (filter -> min/mag/mipmap, address modes), the sprite shader, the winding rule and the
render-pass grouping are exactly `render.rs`'s (`blend_component`, `build_sampler`, `build_pipeline`).

# The game's compiled effects: listing, translation, verification

Terraria 1.4.5.8 ships three compiled XNA effects (`Content/PixelShader.xnb`, `TileShader.xnb`, `ScreenShader.xnb`):
D3DX "fx_2_0" binaries holding Direct3D 9 `ps_2_0` / `vs_2_0` programs. The port executes them by translating the
bytecode to WGSL.

| file | what |
|---|---|
| `PixelShader.md`, `TileShader.md`, `ScreenShader.md` | generated: every parameter, technique, pass, state assignment, constant, sampler, preshader |
| `asm/<Effect>/<index>_<pass>.asm` | generated: assembly listing of every pass (constant table and preshader as comments) |
| `wgsl/<Effect>/<index>_<pass>.wgsl` | generated: the WGSL module the renderer uses for the pass |
| `COVERAGE.md` | generated: opcode histogram, the table of all passes with the verification results |
| `verification.tsv` | written by the test `gpu_vs_interpreter`: the measured errors per pass |

Code: the effect container is parsed in `terraria-host/src/gfx/fx_binary.rs`; everything about the shaders
(decoder, constant table, preshader, disassembler, translator, interpreter) is the crate `terraria-shaders`; the
renderer side is `terraria-app/src/gfx/render.rs`; `terraria-app/src/bin/effect_demo.rs` draws a sample of passes.

## Regenerate

From `port/` (Git Bash). The player's install is read (never written) from `C:\Users\user\Documents\Games\Terraria`
or `TERRARIA_DIR`; without it the tests and the generator say "SKIPPED" and do nothing.

```
CARGO_TARGET_DIR=target-host cargo test -p terraria-host -p terraria-shaders -j 3      # writes verification.tsv
CARGO_TARGET_DIR=target-host cargo run -p terraria-shaders --example effects_docs -j 3  # writes everything else
```

Both are deterministic: a second run writes identical files (`verification.tsv` depends on the GPU and driver it
ran on; the committed one is from an NVIDIA GeForce RTX 4070 Ti, Vulkan, driver 617.14).

## What the files contain

| effect | parameters | passes | with a vertex shader | with a preshader | state assignments besides shaders |
|---|---|---|---|---|---|
| PixelShader | 21 | 64 | 6 (`MagicMissile`, `FinalFractalVertex`, `HorizonClouds`, `LitNature`, `Aurora`, `StormLightning`) | 50 | none |
| TileShader | 8 | 45 | 0 | 0 | none |
| ScreenShader | 26 | 27 | 0 | 21 | none |

- Every program is shader model 2.0. Each effect has one technique.
- No pass assigns a render state, a sampler state or a texture; no sampler parameter carries sampler states. So an
  applied pass changes the pixel shader (and in six passes the vertex shader) and nothing else: blend, cull,
  scissor, `Textures[n]` and `SamplerStates[n]` are whatever the game set. The host test
  `the_games_effects_are_internally_consistent` asserts this; the renderer refuses a pass that assigns anything else.
- All parameter defaults are zero except `TileShader.Epsilon` (1e-10).
- `PixelShader` has two passes named `ArmorPlaid` (indices 19 and 21, identical bytecode). `Passes["ArmorPlaid"]`
  finds the first, as XNA does.
- Most pixel shaders have a **preshader**: a small program the HLSL compiler split off, which D3DX runs on the CPU
  when the pass is applied and whose results land in float registers of the shader. It is not optional: without it
  those registers are empty. The files use nine of its operations (`add`, `cmp`, `cos`, `frc`, `mov`, `mul`, `neg`,
  `rcp`, `sin`), all with the scalar-first-operand form too.

## How a pass is executed

1. **Load.** `Effect` creation records `Command::CreateEffect` with the file's bytes; parameters start with the
   defaults stored in the file.
2. **Apply.** `EffectPass.Apply()` snapshots every parameter value; each draw carries the snapshot plus
   `Textures[1..3]` with their sampler states. The game's two patterns both end as such draws:
   `Begin(SpriteSortMode.Immediate)` followed by `Apply()` before each `Draw` (what `effect_demo` does for dyes and
   filters, and what the game does) and `SpriteBatch.Begin(..., effect)`, where the batch applies every pass of the
   current technique per texture run (recorded the same way; covered by the host's SpriteBatch test, not drawn by
   the demo).
3. **Translate** (first draw of a pass): `terraria_shaders::wgsl::translate` gives one WGSL module with `vs` and `fs`.
   Pipelines are cached by (effect, technique, pass, blend state, topology, cull mode); all targets are `Rgba8Unorm`.
4. **Uniforms** (every draw): `terraria_shaders::uniforms::fill` packs the snapshot's values into the float
   registers of the constant table, then runs the preshader. The registers are one `array<vec4<f32>, N>` indexed by
   register number (group 1, binding 0); a vertex shader has its own block (binding 1) that starts with the
   half-pixel vector.
   - scalar / vector: one register per array element, from component x; unused components 0;
   - `float4x4` (column-major, the only matrix: `MatrixTransform`): one register per column, so
     `dp4 oPos.x, v0, c16` is the row vector times XNA's matrix;
   - `bool` parameters (`invertSpecialGroupResult`, `uMultiChunkScene`) live in float registers as 1.0 / 0.0;
     `SetValue(int)` on a float parameter (`uDirection`) converts.
   The constant-table test checks that all 641 constants of all shaders and preshaders name an effect parameter of
   the same shape.
5. **Samplers.** Sampler register n is texture binding 2n and sampler binding 2n+1 of group 0. Register 0 is the
   draw's texture with `SamplerStates[0]`; registers 1..3 are `Textures[1..3]` with `SamplerStates[1..3]` as they
   were at the draw. A register with no texture samples opaque black (0, 0, 0, 1), Direct3D 9's result for a null
   texture.
6. **Vertex stage.** A pass without a vertex shader keeps SpriteBatch's (in Direct3D the vertex shader of XNA's
   `SpriteEffect` stays bound): the host transforms the vertices as before and the generated module contains the
   pass-through `vs` for the sprite vertex. That stage provides `t0.xy` and `v0`; all 130 such pixel shaders read
   nothing else (the translator refuses otherwise). A pass with a vertex shader gets the vertices as the game
   supplied them (`RawVertex`: position, texcoord 0, colour as vec4 / vec4 / bytes) and runs the translated program.

## Pixel centres (the Direct3D 9 half pixel)

- Sprite path: XNA's SpriteBatch subtracts half a pixel and Direct3D 9 rasterises with pixel centres at integers;
  the two cancel. The host leaves both out (unchanged from before this work).
- Passes with a vertex shader: the game builds `MatrixTransform` itself and subtracts the half pixel there
  (`SpriteViewMatrix.Rebuild`, SpriteViewMatrix.cs:159-160: `... * CreateTranslation(-0.5, -0.5, 0) * ortho`). The
  generated vertex function therefore ends with Direct3D 9's rule, `position.xy += half_pixel * position.w` with
  `half_pixel = (1 / viewport width, -1 / viewport height)`, which moves the geometry back by (+0.5, +0.5) pixels.
  Without it these passes would sit half a pixel up and left.
- Pixel shaders: no half-texel correction was found. What was checked: the preshaders, which is where a
  `0.5 / size` would be computed. Nine of them combine a size parameter with a 0.5 literal; in each the 0.5 is half
  a size (a centre: `0.5 * uSourceRect.zw` in `ArmorHades` / `ArmorPhase` / `ArmorGel` / `ArmorLoki` / `ForceField`,
  `0.5 * uScreenResolution - 200` in `OverlayGlow`), a threshold (`0.5 - uIntensity` in `FilterNoir`) or part of an
  angle reduction (`ArmorSolar`, `ArmorVoid`). The shader bodies themselves were not analysed for this, and
  nothing was compared with a Direct3D 9 capture.

## Verification

There is no Direct3D 9 reference renderer on the development machine, so the proof has two halves.

**The decoder and the semantics** are written from the public descriptions (MSDN "Direct3D Shader Codes" and the
per-instruction pages, Wine's `d3dx9_36`, MojoShader) and can be checked by eye: each WGSL line carries its source
instruction, and `asm/` is the listing.

**The WGSL path against a CPU interpreter** (`cargo test -p terraria-shaders --test gpu_vs_interpreter`): every pass
is rendered on the GPU through its generated WGSL (headless wgpu device, no window) and by the interpreter, which
executes the same bytecode on `f32` registers with a software sampler (point / bilinear, wrap / clamp / mirror).
Set-up: a 64 x 64 quad; RGBA8 textures of 64 / 32 / 16 / 32 pixels on samplers 0..3 (sampler 0 clamps, the others
wrap); vertex colour (230, 204, 179, 217); uniform values from the table `TEST_VALUES` in
`terraria-shaders/tests/support/mod.rs` (for example `uColor` = (0.9, 0.5, 0.2), `uTime` = 1.37, `uOpacity` = 0.8,
image sizes = the texture sizes, `MatrixTransform` = pixels to clip space); two runs per pass, bilinear filtering of
smooth textures and point sampling of per-texel noise.

The target is `Rgba32Float` and the error is measured before 8-bit rounding, in 8-bit steps (1.0 = 1/255), after
clamping both results to 0..1. Reason: this GPU's float to `Rgba8Unorm` conversion is not exact round-to-nearest
(it rounds down for fractions between .50 and about .55), so an 8-bit read-back differs by one step in roughly
10 % of the pixels even for `texture * colour`. That one step is a property of the output stage, not of a shader.

Results (details per pass in COVERAGE.md):

| | passes | largest error |
|---|---|---|
| translated to WGSL | 136 of 136 | |
| WGSL accepted by naga's validator | 136 of 136 | |
| verified against the interpreter on the GPU | 136 of 136 | below one step in every pass |
| of these: no pixel above one step in either run | 133 | arithmetic passes: 0.57 steps (`FilterBlizzardBackground`, bilinear); with transcendental instructions: 0.99 steps (`ArmorShiftingSands`, bilinear) |
| of these: some pixels on a discontinuity | 3 | see below |

Three passes have pixels that differ by more than a step, each explained by a discontinuity of the shader where the
last bit of the GPU's arithmetic picks the side (the test re-runs the interpreter with small perturbations and
requires the GPU's value to be reachable; anything unexplained fails the test):
- `ArmorShiftingSands`: 9 of 4096 pixels in the bilinear run, on a `frc` wrap of a computed texture coordinate;
- `ArmorPolarized`: 7 pixels, on its `cmp` threshold "average of r, g, b >= 0.6" where texels sum to exactly 459;
- `ArmorDev`: 280 pixels in the point-sampled run. The shader hashes with `frac(sin(x) * 43758.5)`, which turns the
  last bits of `sin` into noise; any two implementations of `sin` (CPU, this GPU, a Direct3D 9 driver, the
  reference polynomial) give different noise. Its bilinear run agrees within 0.5 steps.

Weak spots of the test, stated plainly:
- two passes give a constant image with the test inputs (`MonoFix`, whose output IS one preshader register, and
  `FinalFractalVertex`, where the chosen inputs saturate); they agree, but on one colour;
- the interpreter and the WGSL are two implementations of the same reading of the instruction set. A misreading
  shared by both (a wrong formula for an instruction) would not show; that is what the per-line comments and the
  reference documents are for;
- the preshader runs on the CPU in both paths, so it is tested by its unit tests and by review, not by this
  comparison.

Instructions the translator implements but the files never use (`sub`, `slt`, `sge`, `texkill`, `texld` projected /
biased, the `abs` source modifiers) are covered by a hand-assembled shader in the same test.

## Opcode coverage

Used by the three files (vertex and pixel shaders, 4639 instructions including declarations): `abs`, `add`,
`cmp`, `dcl`, `def`, `dp2add`, `dp3`, `dp4`, `exp`, `frc`, `log`, `lrp`, `mad`, `max`, `min`, `mov`, `mul`, `nrm`,
`pow`, `rcp`, `rsq`, `sincos`, `texld`. Source modifiers: negate only. Result modifiers: `_sat` only. No relative
addressing, no flow control, no `texkill`, no derivatives. All of it is translated and interpreted.

Everything else the decoder knows (`lit`, `dst`, `m3x3`.., `crs`, `sgn`, flow control, `mova`, `dsx` / `dsy`,
`texldd`, `texldl`, `setp`, `cnd`, `bem`, int / bool constants, shader model 3 registers, the ps_1_x source
modifiers, predication) is disassembled but **refused** by the translator and the interpreter with an error naming
the instruction; nothing is skipped silently. Shader model 1 is refused at decode. The exact counts are in
COVERAGE.md.

## Known deviations

- `sincos`: Direct3D 9 defines it as a polynomial (the two constant registers are its coefficients) valid on
  -pi..pi; drivers are free to do better. Here it is the GPU's `cos` / `sin`. 21 of the 27 `sincos` in the files
  come directly after the compiler's range reduction (`frc`, then `mad` with 2 pi and -pi); the other six
  (`ArmorPlaid` twice, `ArmorAcid`, `ForceField`, `WaterDistortionObject`, `FilterCrystalDestructionVortex`) take a
  value the preceding code bounds another way (a clamp, a `cmp` selection, a polynomial of a saturated value).
  Whether each of those stays inside -pi..pi was not analysed; outside it the polynomial and the real functions
  diverge. `ArmorDev` (above) is the pass where even the last bits are visible, as different noise.
- Division by zero and friends: Direct3D 9 gives `rcp(0)` = +infinity, `rsq(0)` = +infinity (the reference uses
  FLT_MAX), `log(0)` = -infinity (FLT_MAX), `nrm` of a zero vector undefined. WGSL leaves these to the
  implementation; the interpreter uses IEEE results.
- `texkill` (not used by the files): the four-component rule of shader model 2 as Wine implements and tests it;
  MojoShader tests xyz only.
- 8-bit output: one step of latitude in the GPU's float to byte conversion (see Verification). Direct3D 9 hardware
  had the same latitude; byte-exact agreement with a particular Direct3D 9 driver is not claimed.
- Texture filtering: GPUs filter with fixed-point weights; textures here have one mip level, so `texldb`'s bias and
  the mip part of a filter mode do nothing. The interpreter does not model different minification and
  magnification filters (the renderer does).
- Preshader arithmetic follows Wine where Wine and MojoShader disagree (`exp` = 2^x, `log` = log2|x|, `rsq` of |x|,
  `dot` writes one component, `div` yields 0). None of those five operations occurs in the files, so the choice
  does not affect the game. Array-indexed preshader operands are refused (none occur).
- `_pp` (partial precision) and `_centroid` are ignored. No shader writes depth.
- Vertex shader passes: `oD0` is clamped to 0..1 as shader model 2 does; `oFog` / `oPts` are refused (not written
  by the files); vertices keep their z, but there is no depth buffer in the port's renderer.
- Not checked against the original: the pictures. `work/shots/s1_effects.png` shows plausible dyes, trails and
  filters; whether each matches the game pixel for pixel needs captures from the real game.

## What the translated game still needs from this path

- `terraria-app/Cargo.toml` should list `terraria-shaders`; until then `gfx/render.rs` compiles the crate as a
  module by path (it has no dependencies and uses no `crate::` paths for that reason).
- `EffectParameter.SetValue(Quaternion)` and `SetValue(string)`: the game's `ShaderData.EffectParameter<T>` builds
  delegates for them (ShaderData.cs:48); no shader of the three files has such a parameter, the host has no such
  setter.
- `Effect.Clone()`, assigning `CurrentTechnique`, `EffectParameter.GetValue*`: not used by the game's code
  (searched in the decompile). `CurrentTechnique` assignment and `GetValueSingle / Vector2 / Vector3` exist in the
  host; `Clone` does not.
- A custom effect whose passes mix vertex-shader and fixed-pipeline passes inside one SpriteBatch is reported as
  not implemented (no effect of the game does that).
- Uniforms are recomputed and uploaded per draw and `Apply` clones every parameter value; correct, not fast.

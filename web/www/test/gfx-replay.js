// Headless replay test of gfx.js (run by test/run-gfx-headless.mjs, or open test/gfx-replay.html by hand).
// A JS-built frame goes through the real replayer; snapshot() of the back buffer is compared with pixels computed
// here from the blend rules. Result: window.__gfxResult = { ok, ... }.

import { requestDevice, createGfx } from '../gfx.js';
import { FrameBuilder, BLEND_STATES, SAMPLERS, regs, NO_TEXTURE } from '../lib/frame-codec.js';

const W = 16;
const H = 16;
const TOL = 2; // unorm8 rounding of blend results differs by implementation by at most 1

// ---------------------------------------------------------------- the frame
const P = (tx, ty) => [tx * 80, ty * 80, (tx + ty) * 30, [255, 200, 128, 64][(tx + ty) % 4]];
const pattern = [];
for (let ty = 0; ty < 4; ty++) for (let tx = 0; tx < 4; tx++) pattern.push(...P(tx, ty));
const NEW00 = [0, 255, 0, 255]; // texel (0,0) after the mid-frame upload
const C0 = [1, 0.5, 1, 1]; // the effect's colour register
const CLEAR = [20, 40, 60, 255];
const B_COLOR = [255, 128, 64, 128];

// tiny effect following the DefineShader binding rules: unit 0 -> bindings 0 / 1, fragment uniforms at group 1 binding 0
const INVERT_WGSL = `
struct VOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) color: vec4<f32> };
@vertex fn vs(@location(0) pos: vec2<f32>, @location(1) uv: vec2<f32>, @location(2) color: vec4<f32>) -> VOut {
  var o: VOut; o.pos = vec4<f32>(pos, 0.0, 1.0); o.uv = uv; o.color = color; return o;
}
@group(0) @binding(0) var t0: texture_2d<f32>;
@group(0) @binding(1) var s0: sampler;
struct Ps { c0: vec4<f32> };
@group(1) @binding(0) var<uniform> ps: Ps;
@fragment fn fs(i: VOut) -> @location(0) vec4<f32> {
  let t = textureSample(t0, s0, i.uv);
  return vec4<f32>(1.0 - t.rgb, 1.0) * ps.c0;
}`;

function frame1() {
  const f = new FrameBuilder(1, W, H);
  f.createTexture(1, 4, 4).upload(1, 0, 0, 4, 4, pattern);
  f.createTexture(2, 1, 1).upload(2, 0, 0, 1, 1, [255, 255, 255, 255]);
  f.createTexture(3, 4, 4, true);
  // render target 3: discard-on-bind (no Clear) -> transparent, left half red
  f.setTarget(3, false);
  f.draw({ texture: 2, blend: BLEND_STATES.Opaque, ...f.quad(4, 4, 0, 0, 2, 4, [255, 0, 0, 255]) });
  // the back buffer
  f.setTarget(0, false).clear(...CLEAR);
  f.draw({ texture: 1, blend: BLEND_STATES.AlphaBlend, sampler: SAMPLERS.PointClamp, ...f.quad(W, H, 0, 0, 8, 8) }); // A
  f.upload(1, 0, 0, 1, 1, NEW00); // splits the pass: A saw the old texel, everything after sees the new one
  f.draw({ texture: 1, blend: BLEND_STATES.Additive, ...f.quad(W, H, 4, 4, 12, 12, B_COLOR) }); // B
  f.defineShader({ id: 1, label: 'test invert', wgsl: INVERT_WGSL, units: [0], psRegs: 1, vsRegs: -1 });
  f.draw({ texture: 1, shader: 1, ps: regs(C0), units: [{ unit: 0, texture: 1, sampler: SAMPLERS.PointClamp }], blend: BLEND_STATES.Opaque, ...f.quad(W, H, 8, 0, 16, 8) }); // Q2
  f.draw({ texture: NO_TEXTURE, shader: 1, ps: regs(C0), units: [{ unit: 0, texture: NO_TEXTURE, sampler: SAMPLERS.PointClamp }], blend: BLEND_STATES.Opaque, ...f.quad(W, H, 8, 8, 16, 16) }); // Q4: null texture
  f.draw({ texture: 3, blend: BLEND_STATES.AlphaBlend, ...f.quad(W, H, 0, 8, 8, 16) }); // Q3: the render target
  f.scissor({ x: 12, y: 12, w: 100, h: 100 }); // clamped to the target
  f.draw({ texture: 2, blend: BLEND_STATES.Opaque, ...f.quad(W, H, 0, 0, W, H, [0, 255, 0, 255]) });
  f.scissor(null);
  return f; // the builder: sent as parts (v2 chunks) and as one buffer
}

function frame3() {
  const f = new FrameBuilder(3, W, H);
  f.createTexture(5, 2, 2).createTexture(6, 4, 4);
  f.setTarget(0, true);
  f.draw({ texture: 5, blend: BLEND_STATES.Opaque, ...f.quad(W, H, 8, 8, 12, 12) });
  f.draw({ texture: 6, blend: BLEND_STATES.AlphaBlend, ...f.quad(W, H, 12, 0, 16, 4) });
  return f.build({ untilEnd: true });
}

function frame2() {
  // persistence across frames: SetTarget 0 with preserve, no Clear, one blue pixel; texture 2 still exists
  const f = new FrameBuilder(2, W, H);
  f.setTarget(0, true);
  f.viewport(0, 0, 8, 8); // half the target: clip space maps onto pixels 0..8, so this 0..4 quad lands on 0..2
  f.draw({ texture: 2, blend: BLEND_STATES.Opaque, ...f.quad(W, H, 0, 0, 4, 4, [0, 0, 255, 255]) });
  return f.build({ untilEnd: true }); // FRAME_FORMAT v2: one part, commands until End
}

// ---------------------------------------------------------------- expected pixels
const n = (v) => v / 255;
const sat = (v) => Math.min(1, Math.max(0, v));
function expected() {
  const img = [];
  for (let i = 0; i < W * H; i++) img.push(CLEAR.map(n));
  const at = (x, y) => img[y * W + x];
  const set = (x, y, v) => (img[y * W + x] = v.map(sat));
  const texel = (tx, ty, after) => (after && tx === 0 && ty === 0 ? NEW00 : P(tx, ty)).map(n);
  // A: AlphaBlend (One, InverseSourceAlpha), old texels
  for (let y = 0; y < 8; y++) for (let x = 0; x < 8; x++) {
    const s = texel(x >> 1, y >> 1, false);
    const d = at(x, y);
    set(x, y, [0, 1, 2].map((k) => s[k] + d[k] * (1 - s[3])).concat(s[3] + d[3] * (1 - s[3])));
  }
  // B: Additive (SourceAlpha, One), texel * vertex colour, new texel
  const bc = B_COLOR.map(n);
  for (let y = 4; y < 12; y++) for (let x = 4; x < 12; x++) {
    const t = texel((x - 4) >> 1, (y - 4) >> 1, true);
    const s = t.map((v, k) => v * bc[k]);
    const d = at(x, y);
    set(x, y, [0, 1, 2, 3].map((k) => (k < 3 ? s[k] * s[3] + d[k] : s[3] * s[3] + d[3])));
  }
  // Q2: the effect, opaque: (1 - t.rgb, 1) * c0
  for (let y = 0; y < 8; y++) for (let x = 8; x < 16; x++) {
    const t = texel((x - 8) >> 1, y >> 1, true);
    set(x, y, [(1 - t[0]) * C0[0], (1 - t[1]) * C0[1], (1 - t[2]) * C0[2], C0[3]]);
  }
  // Q4: null texture samples opaque black -> c0
  for (let y = 8; y < 16; y++) for (let x = 8; x < 16; x++) set(x, y, C0);
  // Q3: render target 3 (left half red, right half transparent) with AlphaBlend
  for (let y = 8; y < 16; y++) for (let x = 0; x < 8; x++) {
    const s = (x >> 1) < 2 ? [1, 0, 0, 1] : [0, 0, 0, 0];
    const d = at(x, y);
    set(x, y, [0, 1, 2].map((k) => s[k] + d[k] * (1 - s[3])).concat(s[3] + d[3] * (1 - s[3])));
  }
  // scissored green
  for (let y = 12; y < 16; y++) for (let x = 12; x < 16; x++) set(x, y, [0, 1, 0, 1]);
  const f1 = img.map((p) => p.map((v) => Math.round(v * 255)));
  const f2 = f1.map((p) => p.slice());
  for (let y = 0; y < 2; y++) for (let x = 0; x < 2; x++) f2[y * W + x] = [0, 0, 255, 255];
  return [f1, f2];
}

function compare(snap, want, label) {
  const bad = [];
  for (let i = 0; i < W * H; i++) {
    const got = [...snap.data.subarray(i * 4, i * 4 + 4)];
    if (got.some((v, k) => Math.abs(v - want[i][k]) > TOL)) bad.push({ x: i % W, y: Math.floor(i / W), got, want: want[i] });
  }
  return { label, mismatches: bad.length, first: bad.slice(0, 8) };
}

async function run() {
  const out = { ok: false, steps: [] };
  try {
    const { adapter, device } = await requestDevice();
    out.adapter = adapter.info ? `${adapter.info.vendor} ${adapter.info.architecture} ${adapter.info.description}`.trim() : 'unknown';
    const errors = [];
    device.addEventListener('uncapturederror', (e) => errors.push(e.error.message));
    const canvas = new OffscreenCanvas(W * 2, H * 2); // presented at 2x: nearest texel
    const notes = [];
    const gfx = createGfx({ device, canvas, log: (level, text) => (level >= 4 ? errors : notes).push(`[log ${level}] ${text}`) });
    out.canvasFormat = gfx.canvasFormat;
    const [want1, want2] = expected();

    const f1 = frame1();
    const parts = f1.buildChunks(1); // v2: one command per gfx_frame_chunk part, the End in the last part
    out.steps.push({ replayed: gfx.frame(parts), parts: parts.length });
    const snap1 = await gfx.snapshot(0);
    out.frame1 = compare(snap1, want1, 'frame 1');
    // presentation: what the canvas shows equals the back buffer (alpha forced to 255), whatever the canvas format
    const bmp = canvas.transferToImageBitmap();
    const c2 = new OffscreenCanvas(W * 2, H * 2).getContext('2d');
    c2.drawImage(bmp, 0, 0);
    const shown = c2.getImageData(0, 0, W * 2, H * 2).data;
    let presentBad = 0;
    for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
      const s = snap1.data.subarray((y * W + x) * 4, (y * W + x) * 4 + 3);
      const o = ((y * 2 + 1) * W * 2 + (x * 2 + 1)) * 4;
      if ([0, 1, 2].some((k) => Math.abs(shown[o + k] - s[k]) > TOL) || shown[o + 3] !== 255) presentBad++;
    }
    out.present = { mismatches: presentBad };

    out.steps.push({ sameSerialIgnored: gfx.frame(f1.build()) === false });
    out.steps.push({ replayed2: gfx.frame(frame2()) });
    const snap2 = await gfx.snapshot(0);
    out.frame2 = compare(snap2, want2, 'frame 2');
    // gfx_texture: texture 5 preloaded (2 x 2) and kept by the frame's CreateTexture of the same size; texture 6
    // preloaded 2 x 2 but created 4 x 4 by the frame: recreated, so transparent
    gfx.preload(5, 2, 2, new Uint8Array([255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255]));
    gfx.preload(6, 2, 2, new Uint8Array(16).fill(200));
    out.steps.push({ replayed3: gfx.frame(frame3()) });
    const snap3 = await gfx.snapshot(0);
    const want3 = want2.map((p) => p.slice());
    out.steps.push({ mismatchNoted: notes.some((n) => n.includes('texture 6: preloaded 2x2')) });
    // texture 5 over pixels 8..12 x 8..12 (each texel 2 x 2), opaque
    const t5 = [[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255], [255, 255, 255, 255]];
    for (let y = 8; y < 12; y++) for (let x = 8; x < 12; x++) want3[y * W + x] = t5[((y - 8) >> 1) * 2 + ((x - 8) >> 1)];
    // texture 6 (recreated: transparent black) with AlphaBlend over 12..16 x 0..4: unchanged
    out.frame3 = compare(snap3, want3, 'frame 3 (gfx_texture)');
    out.errors = errors;
    out.notes = notes;
    out.sample = { q1_00: [...snap1.data.subarray(0, 4)], q4: [...snap1.data.subarray((9 * W + 9) * 4, (9 * W + 9) * 4 + 4)] };
    out.ok = out.frame1.mismatches === 0 && out.frame2.mismatches === 0 && out.frame3.mismatches === 0 && presentBad === 0 && errors.length === 0 &&
      out.steps[1].sameSerialIgnored === true;
  } catch (e) {
    out.error = String(e && e.stack ? e.stack : e);
  }
  window.__gfxResult = out;
  document.getElementById('out').textContent = JSON.stringify(out, null, 2);
}

run();

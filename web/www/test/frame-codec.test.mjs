import { test } from 'node:test';
import assert from 'node:assert/strict';
import { FrameBuilder, parseFrame, OP, BLEND_STATES, SAMPLERS, regs, NO_TEXTURE, BLEND, BLEND_FN, UNTIL_END } from '../lib/frame-codec.js';
import { blendComponent, blendState, samplerDescriptor, minMagLinear, primitiveState, clampRect, passEnd, BLEND_FACTOR, createTextureAction } from '../lib/gfx-rules.js';

function sample() {
  const f = new FrameBuilder(42, 16, 8);
  const q = f.quad(16, 8, 0, 0, 8, 8, [255, 128, 0, 200]);
  f.rawVertex([1, 2, 3, 4], [5, 6, 7, 8], [9, 10, 11, 12]);
  f.createTexture(1, 4, 4, false)
    .upload(1, 0, 0, 2, 1, [1, 2, 3, 4, 5, 6, 7, 8])
    .createTexture(3, 8, 8, true)
    .setTarget(3, false)
    .clear(10, 20, 30, 40)
    .viewport(-5, 1, 100, 2)
    .scissor({ x: 1, y: 2, w: 3, h: 4 })
    .scissor(null)
    .defineShader({ id: 7, label: 'fx.Tech[0] P', wgsl: '// ünïcode', units: [0, 2], psRegs: 2, vsRegs: 3 });
  f.draw({ texture: 1, blend: BLEND_STATES.Additive, sampler: SAMPLERS.LinearWrap, cull: 2, topology: 1, ...q });
  f.draw({ texture: 1, shader: 7, ps: regs([1, 2, 3, 4], [5, 6, 7, 8]), vs: regs([0.5, -0.25, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0]), units: [{ unit: 0, texture: 1, sampler: SAMPLERS.PointClamp }, { unit: 2, texture: NO_TEXTURE, sampler: SAMPLERS.PointWrap }], first: 0, count: 3 });
  f.destroyTexture(1).readback(3, 99);
  return f.build();
}

test('frame round trip: header, vertices, every command', () => {
  const bytes = sample();
  const f = parseFrame(bytes);
  assert.equal(f.serial, 42);
  assert.deepEqual([f.backW, f.backH, f.nVertices, f.nRaw], [16, 8, 6, 1]);
  assert.equal(f.vertices.length, 6 * 20);
  assert.equal(f.raw.length, 36);
  const v = new DataView(f.vertices.buffer, f.vertices.byteOffset);
  assert.deepEqual([v.getFloat32(0, true), v.getFloat32(4, true), v.getFloat32(8, true), v.getFloat32(12, true)], [-1, 1, 0, 0]);
  assert.deepEqual([...f.vertices.subarray(16, 20)], [255, 128, 0, 200]);
  assert.deepEqual([...f.raw.subarray(32, 36)], [9, 10, 11, 12]);
  const ops = f.commands.map((c) => c.op);
  assert.deepEqual(ops, [1, 2, 1, 4, 5, 6, 7, 7, 9, 8, 8, 3, 10]);
  const [ct, up, , st, cl, vp, sc1, sc2, ds, d1, d2, dt, rb] = f.commands;
  assert.deepEqual(ct, { op: OP.CREATE_TEXTURE, id: 1, w: 4, h: 4, target: false });
  assert.deepEqual([up.id, up.x, up.y, up.w, up.h, [...up.rgba]], [1, 0, 0, 2, 1, [1, 2, 3, 4, 5, 6, 7, 8]]);
  assert.deepEqual(st, { op: OP.SET_TARGET, id: 3, preserve: false });
  assert.deepEqual(cl.rgba, [10, 20, 30, 40]);
  assert.deepEqual(vp.rect, { x: -5, y: 1, w: 100, h: 2 });
  assert.deepEqual(sc1.rect, { x: 1, y: 2, w: 3, h: 4 });
  assert.equal(sc2.rect, null);
  assert.deepEqual([ds.id, ds.label, ds.wgsl, ds.units, ds.psRegs, ds.vsRegs], [7, 'fx.Tech[0] P', '// ünïcode', [0, 2], 2, 3]);
  assert.deepEqual(d1.blend, { ...BLEND_STATES.Additive });
  assert.deepEqual(d1.sampler, SAMPLERS.LinearWrap);
  assert.deepEqual([d1.cull, d1.topology, d1.first, d1.count, d1.shader, d1.units.length], [2, 1, 0, 6, 0, 0]);
  assert.equal(d2.shader, 7);
  assert.equal(d2.ps.length, 32);
  assert.equal(d2.vs.length, 64);
  assert.equal(new Float32Array(d2.vs.slice().buffer)[1], -0.25);
  assert.deepEqual(d2.units, [{ unit: 0, texture: 1, sampler: SAMPLERS.PointClamp }, { unit: 2, texture: NO_TEXTURE, sampler: SAMPLERS.PointWrap }]);
  assert.deepEqual(dt, { op: OP.DESTROY_TEXTURE, id: 1 });
  assert.deepEqual(rb, { op: OP.READBACK, id: 3, request: 99 });
});

test('a frame inside a larger buffer (wasm memory) parses from its view', () => {
  const bytes = sample();
  const mem = new Uint8Array(bytes.length + 1000);
  mem.set(bytes, 500);
  const f = parseFrame(mem.subarray(500, 500 + bytes.length));
  assert.equal(f.commands.length, 13);
  assert.deepEqual([...f.commands[1].rgba], [1, 2, 3, 4, 5, 6, 7, 8]);
});

test('malformed frames fail with the offset', () => {
  const bytes = sample();
  assert.throws(() => parseFrame(bytes.subarray(0, bytes.length - 3)), /truncated at byte/);
  const bad = bytes.slice();
  bad[0] = 88;
  assert.throws(() => parseFrame(bad), /bad magic/);
  const extra = new Uint8Array(bytes.length + 2);
  extra.set(bytes);
  assert.throws(() => parseFrame(extra), /trailing bytes/);
  const f = new FrameBuilder(1, 1, 1).build();
  const op = new Uint8Array(f.length + 1);
  op.set(f);
  new DataView(op.buffer).setUint32(28, 1, true); // n_commands = 1 (header: 4 + 8 + 5 * 4 bytes)
  op[f.length] = 99;
  assert.throws(() => parseFrame(op), /unknown op 99/);
});

test('blend mapping (render.rs blend_factor / blend_component)', () => {
  assert.equal(BLEND_FACTOR.length, 13);
  assert.deepEqual(blendComponent(BLEND.One, BLEND.InverseSourceAlpha, BLEND_FN.Add), { srcFactor: 'one', dstFactor: 'one-minus-src-alpha', operation: 'add' });
  assert.deepEqual(blendComponent(BLEND.SourceAlpha, BLEND.One, BLEND_FN.ReverseSubtract), { srcFactor: 'src-alpha', dstFactor: 'one', operation: 'reverse-subtract' });
  assert.deepEqual(blendComponent(BLEND.Zero, BLEND.SourceColor, BLEND_FN.Max), { srcFactor: 'one', dstFactor: 'one', operation: 'max' }, 'Min / Max force One');
  assert.deepEqual(blendComponent(BLEND.BlendFactor, BLEND.InverseBlendFactor, BLEND_FN.Subtract), { srcFactor: 'constant', dstFactor: 'one-minus-constant', operation: 'subtract' });
  assert.equal(blendComponent(BLEND.SourceAlphaSaturation, BLEND.DestinationColor, 0).srcFactor, 'src-alpha-saturated');
  assert.deepEqual(blendState(BLEND_STATES.Additive).alpha, { srcFactor: 'src-alpha', dstFactor: 'one', operation: 'add' });
  assert.throws(() => blendComponent(13, 0, 0), RangeError);
});

test('sampler mapping (render.rs build_sampler, TextureFilter::min_mag_linear)', () => {
  assert.deepEqual(samplerDescriptor(SAMPLERS.PointClamp), { addressModeU: 'clamp-to-edge', addressModeV: 'clamp-to-edge', addressModeW: 'clamp-to-edge', magFilter: 'nearest', minFilter: 'nearest', mipmapFilter: 'nearest' });
  assert.deepEqual(samplerDescriptor({ filter: 0, u: 0, v: 2 }), { addressModeU: 'repeat', addressModeV: 'mirror-repeat', addressModeW: 'clamp-to-edge', magFilter: 'linear', minFilter: 'linear', mipmapFilter: 'nearest' });
  assert.deepEqual([0, 1, 2, 3, 4, 5, 6, 7, 8].map(minMagLinear), [[true, true], [false, false], [true, true], [true, true], [false, false], [true, false], [true, false], [false, true], [false, true]]);
});

test('primitive state: front face Cw, CullClockwiseFace = front', () => {
  assert.deepEqual(primitiveState(0, 0), { topology: 'triangle-list', frontFace: 'cw', cullMode: 'none' });
  assert.deepEqual(primitiveState(1, 1), { topology: 'line-list', frontFace: 'cw', cullMode: 'front' });
  assert.equal(primitiveState(0, 2).cullMode, 'back');
});

test('clampRect (render.rs clamp_rect)', () => {
  assert.deepEqual(clampRect({ x: 0, y: 0, w: 16, h: 8 }, 16, 8), [0, 0, 16, 8]);
  assert.deepEqual(clampRect({ x: -5, y: 1, w: 100, h: 2 }, 16, 8), [0, 1, 16, 2]);
  assert.deepEqual(clampRect({ x: 20, y: 20, w: 5, h: 5 }, 16, 8), [16, 8, 0, 0]);
  assert.deepEqual(clampRect({ x: 4, y: 4, w: -3, h: 2 }, 16, 8), [4, 4, 0, 2]);
  assert.deepEqual(clampRect({ x: 2147483647, y: 0, w: 2147483647, h: 1 }, 16, 8), [16, 0, 0, 1]);
});

test('render-pass span: draws, viewports and scissors only', () => {
  const cmds = [{ op: OP.CLEAR }, { op: OP.DRAW }, { op: OP.VIEWPORT }, { op: OP.DRAW }, { op: OP.SCISSOR }, { op: OP.UPLOAD }, { op: OP.DRAW }];
  assert.equal(passEnd(cmds, 1, OP), 5);
  assert.equal(passEnd(cmds, 5, OP), 5);
  assert.equal(passEnd(cmds, 6, OP), 7);
});

// ---------------------------------------------------------------- FRAME_FORMAT v2
function big() {
  // a frame with a large upload (4 MB) between small commands
  const f = new FrameBuilder(7, 64, 64);
  const q = f.quad(64, 64, 0, 0, 32, 32);
  const px = new Uint8Array(1024 * 1024 * 4);
  for (let i = 0; i < px.length; i++) px[i] = (i * 7) & 255;
  f.createTexture(1, 1024, 1024).upload(1, 0, 0, 1024, 1024, px).setTarget(0, false).clear(1, 2, 3, 4);
  f.draw({ texture: 1, ...q }).readback(0, 5);
  return { f, px };
}

test('v2: n_commands = 0xFFFFFFFF reads until op 0 (End)', () => {
  const { f, px } = big();
  const bytes = f.build({ untilEnd: true });
  assert.equal(new DataView(bytes.buffer).getUint32(28, true), UNTIL_END);
  assert.equal(bytes.at(-1), OP.END);
  const p = parseFrame(bytes);
  assert.equal(p.untilEnd, true);
  assert.deepEqual(p.commands.map((c) => c.op), [1, 2, 4, 5, 8, 10]);
  assert.equal(p.commands[1].rgba.length, px.length);
  assert.equal(p.commands[1].rgba[12345], px[12345]);
  // the exact count still works, and End is not required there
  assert.deepEqual(parseFrame(f.build()).commands.map((c) => c.op), [1, 2, 4, 5, 8, 10]);
  // missing End in until-End mode: truncated
  assert.throws(() => parseFrame(bytes.subarray(0, bytes.length - 1)), /truncated/);
  // bytes after End: refused
  const extra = new Uint8Array(bytes.length + 1);
  extra.set(bytes);
  assert.throws(() => parseFrame(extra), /trailing bytes/);
});

test('v2: a frame in parts (cut at command boundaries) parses like the whole', () => {
  const { f, px } = big();
  const whole = parseFrame(f.build({ untilEnd: true }));
  for (const every of [1, 2, 3, 100]) {
    const parts = f.buildChunks(every);
    if (every === 1) assert.ok(parts.length >= 6, `one command per part (${parts.length} parts)`);
    const p = parseFrame(parts);
    assert.deepEqual(p.commands.map((c) => c.op), whole.commands.map((c) => c.op));
    assert.deepEqual(p.commands[3], whole.commands[3]);
    const up = p.commands[1];
    assert.equal(up.rgba.length, px.length);
    assert.equal(up.rgba[4000000], px[4000000]);
    // a part-local range is a view, not a copy
    assert.ok(parts.some((x) => x.buffer === up.rgba.buffer), 'the upload pixels are a view into their part');
  }
});

test('v2: the stream reader does not depend on where the parts are cut', () => {
  const { f } = big();
  const bytes = f.build({ untilEnd: true });
  const want = parseFrame(bytes);
  for (const cutsAt of [[1], [3, 4, 5, 6, 7], [31, 33, 129, 200], [bytes.length - 2], [50, 1000, 2000000, 4000000]]) {
    const parts = [];
    let prev = 0;
    for (const c of cutsAt) { parts.push(bytes.subarray(prev, c)); prev = c; }
    parts.push(bytes.subarray(prev));
    parts.splice(1, 0, new Uint8Array(0)); // an empty part is harmless
    const p = parseFrame(parts);
    assert.equal(p.serial, 7);
    assert.deepEqual([...p.vertices], [...want.vertices]);
    assert.deepEqual(p.commands.map((c) => c.op), want.commands.map((c) => c.op));
    assert.deepEqual([...p.commands[1].rgba.subarray(0, 64)], [...want.commands[1].rgba.subarray(0, 64)]);
    assert.equal(p.commands[1].rgba[3999999], want.commands[1].rgba[3999999]);
  }
});

test('gfx_texture preload rule: same size keeps the texture, anything else recreates it', () => {
  const pre = new Map([[7, { width: 64, height: 32 }], [8, { width: 1, height: 1 }]]);
  assert.equal(createTextureAction(pre, 7, 64, 32, false), 'keep');
  assert.equal(createTextureAction(pre, 7, 32, 64, false), 'create', 'different size');
  assert.equal(createTextureAction(pre, 7, 64, 32, true), 'create', 'a render target needs RENDER_ATTACHMENT');
  assert.equal(createTextureAction(pre, 9, 64, 32, false), 'create', 'not preloaded');
  assert.equal(createTextureAction(pre, 8, 0, 0, false), 'keep', 'sizes are at least 1, as created');
  assert.equal(createTextureAction(new Map(), 7, 64, 32, false), 'create');
});

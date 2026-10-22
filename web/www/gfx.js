// The WebGPU replayer of FRAME_FORMAT.md. Every rule follows the native replayer,
// port/crates/terraria-app/src/gfx/render.rs (`replay_frame`, `effect_draw`, `build_pipeline`, `build_sampler`,
// `blend_component`, `clamp_rect`); comments name the render.rs piece each part mirrors.
//
// Works in a Worker (OffscreenCanvas) or on a page. All targets are rgba8unorm, no sRGB anywhere.
// Texture id 0 is the back buffer; after each frame it is drawn onto the canvas (presentation).

import { parseFrame, OP, NO_TEXTURE, VERTEX_BYTES } from './lib/frame-codec.js';
import { blendState, blendKey, samplerDescriptor, samplerKey, primitiveState, clampRect, passEnd, createTextureAction } from './lib/gfx-rules.js';

const TARGET_FORMAT = 'rgba8unorm';

// render.rs SHADER, unchanged
export const SPRITE_WGSL = `
struct VOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) color: vec4<f32> };
@vertex fn vs(@location(0) pos: vec2<f32>, @location(1) uv: vec2<f32>, @location(2) color: vec4<f32>) -> VOut {
    var o: VOut;
    o.pos = vec4<f32>(pos, 0.0, 1.0);
    o.uv = uv;
    o.color = color;
    return o;
}
@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
// SpriteBatch's effect: texture * vertex colour
@fragment fn fs(i: VOut) -> @location(0) vec4<f32> {
    return textureSample(tex, samp, i.uv) * i.color;
}
`;

// Presentation: the back buffer drawn onto the canvas, nearest texel, alpha forced to 1 (XNA presents opaque).
// Same size: 1:1. Another size (the game keeps a resolution of its own: at least 800x600, or one chosen in its
// settings; or the window was just resized and the game applies the new size at its next frame): scaled to fit with
// the aspect ratio kept, centred, black around it (presentRect(); the game Worker maps the pointer through it).
// A render pass writes logical RGBA whatever the canvas format is, so a bgra8unorm canvas shows the same colours.
const PRESENT_WGSL = `
@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var<uniform> rect: vec4<f32>; // x, y, width, height of the picture on the canvas
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    var p = array<vec2<f32>, 3>(vec2<f32>(-1.0, -3.0), vec2<f32>(-1.0, 1.0), vec2<f32>(3.0, 1.0));
    return vec4<f32>(p[i], 0.0, 1.0);
}
@fragment fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let size = vec2<f32>(textureDimensions(src));
    let q = pos.xy - rect.xy;
    if (q.x < 0.0 || q.y < 0.0 || q.x >= rect.z || q.y >= rect.w) { return vec4<f32>(0.0, 0.0, 0.0, 1.0); }
    let t = vec2<i32>(clamp(floor(q * size / rect.zw), vec2<f32>(0.0), size - 1.0));
    return vec4<f32>(textureLoad(src, t, 0).rgb, 1.0);
}
`;

/** Where a back buffer of bw x bh lands on a canvas of cw x ch: 1:1 when equal, else fitted and centred. */
export function fitRect(bw, bh, cw, ch) {
  if (bw === cw && bh === ch) return { x: 0, y: 0, w: cw, h: ch };
  const s = Math.min(cw / bw, ch / bh);
  const w = Math.max(1, Math.round(bw * s));
  const h = Math.max(1, Math.round(bh * s));
  return { x: Math.floor((cw - w) / 2), y: Math.floor((ch - h) / 2), w, h };
}

const SPRITE_ATTRIBUTES = [
  { shaderLocation: 0, offset: 0, format: 'float32x2' },
  { shaderLocation: 1, offset: 8, format: 'float32x2' },
  { shaderLocation: 2, offset: 16, format: 'unorm8x4' },
];
const RAW_ATTRIBUTES = [
  { shaderLocation: 0, offset: 0, format: 'float32x4' },
  { shaderLocation: 1, offset: 16, format: 'float32x4' },
  { shaderLocation: 2, offset: 32, format: 'unorm8x4' },
];

/** Request an adapter / device. Throws a player-facing Error when WebGPU is missing. */
export async function requestDevice() {
  if (!globalThis.navigator?.gpu) throw new Error('This browser has no WebGPU. This build needs Chrome or Edge 137+.');
  const adapter = await navigator.gpu.requestAdapter();
  if (!adapter) throw new Error('WebGPU is present but no graphics adapter is available (it may be disabled or blocklisted).');
  const device = await adapter.requestDevice();
  return { adapter, device };
}

/**
 * @param {object} o
 * @param {GPUDevice} o.device
 * @param {OffscreenCanvas|HTMLCanvasElement|null} [o.canvas]  presentation target (none: replay only, for tests)
 * @param {(level: number, text: string) => void} [o.log]
 */
export function createGfx({ device, canvas = null, log = () => {} }) {
  const once = new Set();
  const messages = []; // every warning / error the replayer logged (diagnostics)
  const logOnce = (key, level, text) => {
    if (!once.has(key)) {
      once.add(key);
      messages.push({ level, text });
      log(level, text);
    }
  };
  // diagnostics: armed by captureNext(); the next frame with commands records what it draws into what
  let captureArmed = false;
  let capture = null;
  device.addEventListener?.('uncapturederror', (e) => logOnce('gpu:' + e.error.message, 4, `WebGPU: ${e.error.message}`));

  let context = null;
  let canvasFormat = null;
  if (canvas) {
    context = canvas.getContext('webgpu');
    canvasFormat = navigator.gpu.getPreferredCanvasFormat();
    context.configure({ device, format: canvasFormat, alphaMode: 'opaque' });
  }

  // ---------------------------------------------------------------- shared objects (render.rs build_shared)
  const spriteModule = device.createShaderModule({ label: 'spritebatch', code: SPRITE_WGSL });
  const textureEntry = (binding) => ({ binding, visibility: GPUShaderStage.FRAGMENT, texture: { sampleType: 'float', viewDimension: '2d', multisampled: false } });
  const samplerEntry = (binding) => ({ binding, visibility: GPUShaderStage.FRAGMENT, sampler: { type: 'filtering' } });
  const spriteLayout = device.createBindGroupLayout({ label: 'spritebatch texture', entries: [textureEntry(0), samplerEntry(1)] });
  const spritePipelineLayout = device.createPipelineLayout({ label: 'spritebatch', bindGroupLayouts: [spriteLayout] });

  const textures = new Map(); // id -> { texture, view, width, height, gen }
  let texGen = 1;
  const samplers = new Map(); // samplerKey -> GPUSampler
  const spriteGroups = new Map(); // `${id}:${gen}|${samplerKey}` -> GPUBindGroup
  const pipelines = new Map(); // `${blendKey}|${topology}|${cull}` -> sprite pipeline
  const shaders = new Map(); // id -> { def, module, texturesLayout, uniformsLayout, pipelineLayout }
  const effectPipelines = new Map(); // `${shader}|${blendKey}|${topology}|${cull}`
  const effectTextureGroups = new Map();
  const effectUniformGroups = new Map(); // `${shader}|${generation}`
  let nullView = null;
  let lastSerial = -Infinity;
  let pendingDestroy = [];

  function sampler(s) {
    const k = samplerKey(s);
    let smp = samplers.get(k);
    if (!smp) {
      smp = device.createSampler({ label: 'spritebatch', ...samplerDescriptor(s) });
      samplers.set(k, smp);
    }
    return smp;
  }

  function getNullView() {
    // what Direct3D 9 samples from a sampler with no texture: opaque black
    if (!nullView) {
      const t = device.createTexture({ label: 'null texture', size: [1, 1], format: TARGET_FORMAT, usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST });
      device.queue.writeTexture({ texture: t }, new Uint8Array([0, 0, 0, 255]), { bytesPerRow: 4, rowsPerImage: 1 }, [1, 1]);
      nullView = t.createView();
    }
    return nullView;
  }

  const stats = { last: null, frames: 0 };
  // tw.gfx_texture: textures created and filled outside any frame, before the frame that creates them arrives
  const preloaded = new Map(); // id -> { width, height }

  /**
   * tw.gfx_texture(id, width, height, rgba): create texture `id` now (TEXTURE_BINDING | COPY_DST | COPY_SRC) and
   * write its RGBA rows; the frame's CreateTexture of the same size then keeps it (createTextureAction).
   */
  function preload(id, w, h, rgba) {
    const width = Math.max(1, w);
    const height = Math.max(1, h);
    dropTexture(id);
    const texture = device.createTexture({ label: `texture ${id} (preloaded)`, size: [width, height], format: TARGET_FORMAT, usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST | GPUTextureUsage.COPY_SRC });
    if (w > 0 && h > 0) device.queue.writeTexture({ texture }, rgba, { bytesPerRow: w * 4, rowsPerImage: h }, [w, h]);
    textures.set(id, { texture, view: texture.createView(), width, height, gen: texGen++, target: false });
    preloaded.set(id, { width, height });
    stats.preloaded = (stats.preloaded ?? 0) + 1;
    stats.preloadedBytes = (stats.preloadedBytes ?? 0) + w * h * 4;
  }

  function createTexture(id, w, h, target) {
    if (id > (stats.maxTextureId ?? 0)) stats.maxTextureId = id;
    stats.texturesCreated = (stats.texturesCreated ?? 0) + 1;
    if (createTextureAction(preloaded, id, w, h, target) === 'keep') {
      preloaded.delete(id);
      return;
    }
    if (preloaded.has(id)) {
      logOnce(`preload-mismatch:${id}`, 3, `texture ${id}: preloaded ${preloaded.get(id).width}x${preloaded.get(id).height}, the frame creates ${w}x${h}${target ? ' (target)' : ''}: recreated`);
      preloaded.delete(id);
    }
    let usage = GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST;
    if (target) usage |= GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC;
    dropTexture(id);
    // WebGPU textures start zeroed: transparent black, like a new XNA texture
    const texture = device.createTexture({ label: `texture ${id}`, size: [Math.max(1, w), Math.max(1, h)], format: TARGET_FORMAT, usage });
    textures.set(id, { texture, view: texture.createView(), width: Math.max(1, w), height: Math.max(1, h), gen: texGen++, target });
  }
  function dropTexture(id) {
    const t = textures.get(id);
    if (!t) return;
    textures.delete(id);
    const token = `${id}:${t.gen}`;
    for (const k of spriteGroups.keys()) if (k.startsWith(token + '|')) spriteGroups.delete(k);
    for (const k of effectTextureGroups.keys()) if (k.includes(`#${token}#`)) effectTextureGroups.delete(k);
    pendingDestroy.push(t.texture); // destroyed after the frame's last submit: draws recorded earlier may still use it
  }

  // ---------------------------------------------------------------- pipelines (render.rs build_pipeline / build_effect_pipeline)
  function spritePipeline(blend, topology, cull) {
    const k = `${blendKey(blend)}|${topology}|${cull}`;
    let p = pipelines.get(k);
    if (!p) {
      p = device.createRenderPipeline({
        label: 'spritebatch',
        layout: spritePipelineLayout,
        vertex: { module: spriteModule, entryPoint: 'vs', buffers: [{ arrayStride: VERTEX_BYTES, stepMode: 'vertex', attributes: SPRITE_ATTRIBUTES }] },
        fragment: { module: spriteModule, entryPoint: 'fs', targets: [{ format: TARGET_FORMAT, blend: blendState(blend), writeMask: blend.writeMask & 15 }] },
        primitive: primitiveState(topology, cull),
      });
      pipelines.set(k, p);
    }
    return p;
  }

  function defineShader(c) {
    // render.rs build_pass: group 0 = textures (unit u -> binding 2u, sampler 2u+1), group 1 = uniforms
    const module = device.createShaderModule({ label: c.label || `shader ${c.id}`, code: c.wgsl });
    module.getCompilationInfo?.().then((info) => {
      for (const m of info.messages) if (m.type === 'error') logOnce(`shader:${c.id}:${m.message}`, 4, `shader ${c.id} (${c.label}): ${m.lineNum}:${m.linePos} ${m.message}`);
    });
    const entries = [];
    for (const u of c.units) entries.push(textureEntry(u * 2), samplerEntry(u * 2 + 1));
    const texturesLayout = device.createBindGroupLayout({ label: 'effect textures', entries });
    const uniform = (binding, visibility) => ({ binding, visibility, buffer: { type: 'uniform', hasDynamicOffset: true } });
    const uEntries = [];
    if (c.psRegs > 0) uEntries.push(uniform(0, GPUShaderStage.FRAGMENT));
    if (c.vsRegs >= 0) uEntries.push(uniform(1, GPUShaderStage.VERTEX));
    const uniformsLayout = device.createBindGroupLayout({ label: 'effect uniforms', entries: uEntries });
    const pipelineLayout = device.createPipelineLayout({ label: c.label, bindGroupLayouts: [texturesLayout, uniformsLayout] });
    // a redefinition replaces the old one and its cached objects
    for (const map of [effectPipelines, effectTextureGroups, effectUniformGroups]) {
      for (const k of map.keys()) if (k.startsWith(`${c.id}|`)) map.delete(k);
    }
    shaders.set(c.id, { def: c, module, texturesLayout, uniformsLayout, pipelineLayout });
  }

  function effectPipeline(sh, blend, topology, cull) {
    const k = `${sh.def.id}|${blendKey(blend)}|${topology}|${cull}`;
    let p = effectPipelines.get(k);
    if (!p) {
      const raw = sh.def.vsRegs >= 0;
      p = device.createRenderPipeline({
        label: 'effect pass',
        layout: sh.pipelineLayout,
        vertex: { module: sh.module, entryPoint: 'vs', buffers: [{ arrayStride: raw ? 36 : VERTEX_BYTES, stepMode: 'vertex', attributes: raw ? RAW_ATTRIBUTES : SPRITE_ATTRIBUTES }] },
        fragment: { module: sh.module, entryPoint: 'fs', targets: [{ format: TARGET_FORMAT, blend: blendState(blend), writeMask: blend.writeMask & 15 }] },
        primitive: primitiveState(topology, cull),
      });
      effectPipelines.set(k, p);
    }
    return p;
  }

  // ---------------------------------------------------------------- per-frame buffers (render.rs UniformArena, vertex buffers)
  const align = device.limits.minUniformBufferOffsetAlignment;
  const arena = { buffer: null, capacity: 0, used: 0, generation: 0 };
  function pushUniforms(data) {
    const start = Math.ceil(arena.used / align) * align;
    if (!arena.buffer || start + data.length + 4096 > arena.capacity) {
      arena.capacity = Math.max(arena.capacity * 2, 1 << 18);
      arena.buffer = device.createBuffer({ label: 'effect uniforms', size: arena.capacity, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
      arena.generation++;
      arena.used = 0;
      return pushUniforms(data);
    }
    if (data.length) device.queue.writeBuffer(arena.buffer, start, data);
    arena.used = start + data.length;
    return start;
  }
  const vb = { sprite: null, spriteCap: 0, raw: null, rawCap: 0 };
  function vertexBuffer(kind, bytes, minCap) {
    const cap = kind === 'sprite' ? 'spriteCap' : 'rawCap';
    if (!vb[kind] || vb[cap] < bytes.length) {
      vb[cap] = Math.max(2 ** Math.ceil(Math.log2(Math.max(1, bytes.length))), minCap);
      vb[kind] = device.createBuffer({ label: kind + ' vertices', size: vb[cap], usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST });
    }
    if (bytes.length) device.queue.writeBuffer(vb[kind], 0, bytes);
    return vb[kind];
  }

  // ---------------------------------------------------------------- effect draw (render.rs effect_draw)
  function effectDraw(d, target) {
    const sh = shaders.get(d.shader);
    if (!sh) {
      logOnce(`noshader:${d.shader}`, 4, `draw with shader ${d.shader}, which was never defined: skipped`);
      return null;
    }
    const def = sh.def;
    if (d.ps.length !== def.psRegs * 16 || (def.vsRegs >= 0 ? d.vs.length !== (def.vsRegs + 1) * 16 : d.vs.length !== 0)) {
      logOnce(`regs:${d.shader}`, 4, `shader ${d.shader} (${def.label}): uniform blocks of ${d.ps.length} / ${d.vs.length} bytes do not match ps_regs ${def.psRegs} / vs_regs ${def.vsRegs}: skipped`);
      return null;
    }
    const offsets = [];
    if (def.psRegs > 0) offsets.push(pushUniforms(d.ps));
    if (def.vsRegs >= 0) offsets.push(pushUniforms(d.vs));
    const uKey = `${def.id}|${arena.generation}`;
    let uniforms = effectUniformGroups.get(uKey);
    if (!uniforms) {
      if (!arena.buffer) pushUniforms(new Uint8Array(0));
      const entries = [];
      if (def.psRegs > 0) entries.push({ binding: 0, resource: { buffer: arena.buffer, offset: 0, size: def.psRegs * 16 } });
      if (def.vsRegs >= 0) entries.push({ binding: 1, resource: { buffer: arena.buffer, offset: 0, size: (def.vsRegs + 1) * 16 } });
      uniforms = device.createBindGroup({ label: 'effect uniforms', layout: sh.uniformsLayout, entries });
      effectUniformGroups.set(uKey, uniforms);
    }
    // textures: each unit the shader uses; a missing or "no texture" unit samples opaque black
    const bound = [];
    for (const unit of def.units) {
      const e = d.units.find((u) => u.unit === unit) ?? (unit === 0 ? { unit: 0, texture: d.texture, sampler: d.sampler } : null);
      if (e && e.texture === target) return null; // a target cannot be sampled while it is bound (XNA throws)
      const t = e && e.texture !== NO_TEXTURE ? textures.get(e.texture) : null;
      bound.push(t ? { t, token: `${e.texture}:${t.gen}`, s: e.sampler } : { t: null, token: 'null', s: { filter: 1, u: 1, v: 1 } });
    }
    const gKey = `${def.id}|` + bound.map((b) => `#${b.token}#${samplerKey(b.s)}`).join('');
    let group = effectTextureGroups.get(gKey);
    if (!group) {
      const entries = [];
      def.units.forEach((unit, i) => {
        entries.push({ binding: unit * 2, resource: bound[i].t ? bound[i].t.view : getNullView() });
        entries.push({ binding: unit * 2 + 1, resource: sampler(bound[i].s) });
      });
      group = device.createBindGroup({ label: 'effect textures', layout: sh.texturesLayout, entries });
      effectTextureGroups.set(gKey, group);
    }
    return { sh, textures: group, uniforms, offsets, raw: def.vsRegs >= 0 };
  }

  // ---------------------------------------------------------------- the frame (render.rs replay_frame)
  // what the last non-empty frame did (declared before preload() uses it) (for diagnostics: window.__twDev.request('stats').gfxReplay)
  let cur = null;
  const count = (k, n = 1) => { if (cur) cur[k] = (cur[k] ?? 0) + n; };

  function replay(f) {
    cur = { serial: f.serial, back: `${f.backW}x${f.backH}`, commands: f.commands.length, vertices: f.nVertices, raw: f.nRaw, draws: {}, skipped: {} };
    const cap = captureArmed && f.commands.length ? (capture = { serial: f.serial, back: cur.back, commands: f.commands.length, ops: {}, targets: {}, order: [] }) : null;
    if (cap) {
      captureArmed = false;
      for (const c of f.commands) cap.ops[c.op] = (cap.ops[c.op] ?? 0) + 1;
    }
    const capTarget = (id) => {
      let t = cap.targets[id];
      if (!t) {
        const tex = textures.get(id);
        t = cap.targets[id] = { id, size: tex ? `${tex.width}x${tex.height}` : '?', passes: 0, clears: [], draws: 0, skipped: 0, shaders: {}, blends: {}, raw: 0,
          spriteColour: { draws: 0, vertices: 0, min: [255, 255, 255, 255], max: [0, 0, 0, 0], sum: [0, 0, 0, 0], allZeroRgbDraws: 0 }, textures: {} };
        cap.order.push(id);
      }
      return t;
    };
    // texture 0 is the back buffer, (re)created at the frame's size
    const back = textures.get(0);
    if (!back || back.width !== Math.max(1, f.backW) || back.height !== Math.max(1, f.backH)) createTexture(0, f.backW, f.backH, true);

    const spriteVb = vertexBuffer('sprite', f.vertices, 1 << 16);
    const rawVb = f.raw.length ? vertexBuffer('raw', f.raw, 1 << 14) : vb.raw;
    arena.used = 0;

    let encoder = null;
    let target = 0;
    let discard = false; // the target was bound with DiscardContents and nothing has been drawn into it yet
    let viewport = null;
    let scissor = null;
    const cmds = f.commands;
    let i = 0;
    while (i < cmds.length) {
      const c = cmds[i];
      switch (c.op) {
        case OP.CREATE_TEXTURE:
          if (c.w > 4096 || c.h > 4096) log(3, `CreateTexture ${c.id}: ${c.w}x${c.h}${c.target ? ' (target)' : ''}, ${(c.w * c.h * 4 / 1048576).toFixed(0)} MB`);
          if (c.w > device.limits.maxTextureDimension2D || c.h > device.limits.maxTextureDimension2D) {
            logOnce(`toolarge:${c.id}`, 4, `texture ${c.id} is ${c.w}x${c.h}, larger than this GPU's limit ${device.limits.maxTextureDimension2D}`);
          }
          createTexture(c.id, c.w, c.h, c.target);
          i++;
          continue;
        case OP.UPLOAD: {
          count('uploads');
          // draws recorded before the upload must see the old contents
          if (encoder) {
            device.queue.submit([encoder.finish()]);
            encoder = null;
          }
          const t = textures.get(c.id);
          if (t && c.w > 0 && c.h > 0) {
            if (c.x < 0 || c.y < 0 || c.x + c.w > t.width || c.y + c.h > t.height) {
              logOnce(`upload:${c.id}`, 4, `upload outside texture ${c.id} (${c.x},${c.y} ${c.w}x${c.h} into ${t.width}x${t.height}): skipped`);
            } else {
              device.queue.writeTexture({ texture: t.texture, origin: [c.x, c.y] }, c.rgba, { bytesPerRow: c.w * 4, rowsPerImage: c.h }, [c.w, c.h]);
            }
          }
          i++;
          continue;
        }
        case OP.DESTROY_TEXTURE:
          if (c.id !== 0) dropTexture(c.id);
          i++;
          continue;
        case OP.DEFINE_SHADER:
          defineShader(c);
          i++;
          continue;
        case OP.SET_TARGET:
          target = c.id;
          discard = !c.preserve;
          viewport = null;
          scissor = null;
          i++;
          continue;
        case OP.VIEWPORT:
          viewport = c.rect;
          i++;
          continue;
        case OP.SCISSOR:
          scissor = c.rect;
          i++;
          continue;
        case OP.READBACK:
          logOnce('readback', 3, 'render target readback (Texture2D.GetData on a target) is not implemented in the WebGPU replay');
          i++;
          continue;
        case OP.CLEAR:
        case OP.DRAW:
          break;
        default:
          i++;
          continue;
      }
      // ---- a render pass: Clear / Draw, then every following Draw / Viewport / Scissor
      let clearValue = discard ? [0, 0, 0, 0] : null;
      discard = false;
      if (c.op === OP.CLEAR) {
        clearValue = c.rgba.map((v) => v / 255);
        i++;
      }
      const end = passEnd(cmds, i, OP);
      const tgt = textures.get(target);
      count('passes');
      if (!tgt || !tgt.target) {
        count('skipPassNoTarget');
        if (tgt && !tgt.target) logOnce(`nottarget:${target}`, 4, `texture ${target} is bound as a render target but was not created as one: its draws are skipped`);
        i = end;
        continue;
      }
      const tw = tgt.width;
      const th = tgt.height;
      if (cap) {
        const ct = capTarget(target);
        ct.passes++;
        if (clearValue) ct.clears.push(clearValue.map((v) => Math.round(v * 255)).join(','));
      }
      encoder ??= device.createCommandEncoder({ label: 'spritebatch' });
      const pass = encoder.beginRenderPass({
        label: 'spritebatch',
        colorAttachments: [{ view: tgt.view, loadOp: clearValue ? 'clear' : 'load', storeOp: 'store', clearValue: clearValue ?? [0, 0, 0, 0] }],
      });
      pass.setVertexBuffer(0, spriteVb);
      const full = { x: 0, y: 0, w: tw, h: th };
      const applyViewport = (v) => {
        const [x, y, w, h] = clampRect(v ?? full, tw, th);
        if (w > 0 && h > 0) {
          pass.setViewport(x, y, w, h, 0, 1);
          return true;
        }
        return false;
      };
      const applyScissor = (s) => {
        const [x, y, w, h] = clampRect(s ?? full, tw, th);
        pass.setScissorRect(x, y, w, h);
        return w > 0 && h > 0;
      };
      let viewportOk = applyViewport(viewport);
      let scissorOk = scissor ? applyScissor(scissor) : true;
      let bound = null; // key of the pipeline that is set
      let rawBound = false;
      for (const d of cmds.slice(i, end)) {
        if (d.op === OP.VIEWPORT) {
          viewport = d.rect;
          viewportOk = applyViewport(viewport);
          continue;
        }
        if (d.op === OP.SCISSOR) {
          scissor = d.rect;
          scissorOk = applyScissor(scissor);
          continue;
        }
        if (!(viewportOk && scissorOk)) { count('skipViewportOrScissor'); if (cap) capTarget(target).skipped++; continue; }
        if (d.texture === target) { count('skipTextureIsTarget'); continue; } // a target cannot be sampled while it is bound
        cur.draws[target] = (cur.draws[target] ?? 0) + 1;
        if (cap) {
          const ct = capTarget(target);
          ct.draws++;
          const label = d.shader === 0 ? '0 sprite' : `${d.shader} ${shaders.get(d.shader)?.def.label ?? '(undefined)'}`;
          ct.shaders[label] = (ct.shaders[label] ?? 0) + 1;
          const bk = blendKey(d.blend);
          ct.blends[bk] = (ct.blends[bk] ?? 0) + 1;
          if (d.shader !== 0 && shaders.get(d.shader)?.def.vsRegs >= 0) ct.raw++;
          const texKey = d.texture === NO_TEXTURE ? 'none' : `${d.texture}${textures.get(d.texture) ? ' ' + textures.get(d.texture).width + 'x' + textures.get(d.texture).height + (textures.get(d.texture).target ? ' target' : '') : ' (missing)'}`;
          ct.textures[texKey] = (ct.textures[texKey] ?? 0) + 1;
          if (d.shader === 0 || !(shaders.get(d.shader)?.def.vsRegs >= 0)) {
            // sprite vertices: colour statistics
            const sc = ct.spriteColour;
            sc.draws++;
            let allZero = true;
            for (let v = d.first; v < d.first + d.count; v++) {
              const o = v * VERTEX_BYTES + 16;
              if (o + 4 > f.vertices.length) break;
              sc.vertices++;
              for (let k = 0; k < 4; k++) {
                const c = f.vertices[o + k];
                if (c < sc.min[k]) sc.min[k] = c;
                if (c > sc.max[k]) sc.max[k] = c;
                sc.sum[k] += c;
              }
              if (f.vertices[o] || f.vertices[o + 1] || f.vertices[o + 2]) allZero = false;
            }
            if (allZero) sc.allZeroRgbDraws++;
          }
        }
        if ((cur.sample ??= []).length < 6) {
          const o = d.first * VERTEX_BYTES + 16; // first vertex's colour
          const tex = textures.get(d.texture);
          cur.sample.push(`tex ${d.texture}${tex ? ` ${tex.width}x${tex.height}` : ' (none)'} shader ${d.shader} colour ${d.shader ? '-' : [...f.vertices.subarray(o, o + 4)].join(',')}`);
        }
        const setBlendConstant = () => pass.setBlendConstant(d.blend.factor.map((v) => v / 255));
        if (d.shader !== 0) {
          const e = effectDraw(d, target);
          if (!e) { count('skipEffect'); continue; }
          count('effectDraws');
          if (e.raw && !rawVb) continue;
          const key = `e${d.shader}|${blendKey(d.blend)}|${d.topology}|${d.cull}`;
          if (bound !== key) {
            pass.setPipeline(effectPipeline(e.sh, d.blend, d.topology, d.cull));
            setBlendConstant();
            bound = key;
          }
          if (e.raw !== rawBound) {
            pass.setVertexBuffer(0, e.raw ? rawVb : spriteVb);
            rawBound = e.raw;
          }
          pass.setBindGroup(0, e.textures);
          pass.setBindGroup(1, e.uniforms, e.offsets);
          pass.draw(d.count, 1, d.first, 0);
          continue;
        }
        if (rawBound) {
          pass.setVertexBuffer(0, spriteVb);
          rawBound = false;
        }
        const t = textures.get(d.texture);
        if (!t) { count('skipMissingTexture'); continue; }
        const gk = `${d.texture}:${t.gen}|${samplerKey(d.sampler)}`;
        let group = spriteGroups.get(gk);
        if (!group) {
          group = device.createBindGroup({ label: 'spritebatch texture', layout: spriteLayout, entries: [{ binding: 0, resource: t.view }, { binding: 1, resource: sampler(d.sampler) }] });
          spriteGroups.set(gk, group);
        }
        const key = `s|${blendKey(d.blend)}|${d.topology}|${d.cull}`;
        if (bound !== key) {
          pass.setPipeline(spritePipeline(d.blend, d.topology, d.cull));
          setBlendConstant();
          bound = key;
        }
        pass.setBindGroup(0, group);
        pass.draw(d.count, 1, d.first, 0);
      }
      pass.end();
      i = end;
    }
    return encoder;
  }

  // ---------------------------------------------------------------- presentation
  let present = null;
  let pendingSize = null; // a canvas size to apply at the next presentation (so the canvas is never shown cleared)
  // the last presentation: back buffer size, canvas size and where the picture went (pointer mapping)
  const presented = { backW: 0, backH: 0, canvasW: 0, canvasH: 0, x: 0, y: 0, w: 0, h: 0 };
  function presentInto(encoder) {
    if (!context) return;
    if (pendingSize) {
      if (canvas.width !== pendingSize.w) canvas.width = pendingSize.w;
      if (canvas.height !== pendingSize.h) canvas.height = pendingSize.h;
      pendingSize = null;
    }
    const back = textures.get(0);
    if (!back) return;
    if (!present) {
      const module = device.createShaderModule({ label: 'present', code: PRESENT_WGSL });
      const pipeline = device.createRenderPipeline({
        label: 'present',
        layout: 'auto',
        vertex: { module, entryPoint: 'vs' },
        fragment: { module, entryPoint: 'fs', targets: [{ format: canvasFormat }] },
        primitive: { topology: 'triangle-list' },
      });
      const size = device.createBuffer({ label: 'present size', size: 16, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
      present = { pipeline, size, group: null, view: null };
    }
    if (present.view !== back.view) {
      present.view = back.view;
      present.group = device.createBindGroup({ layout: present.pipeline.getBindGroupLayout(0), entries: [{ binding: 0, resource: back.view }, { binding: 1, resource: { buffer: present.size } }] });
    }
    const out = context.getCurrentTexture();
    const r = fitRect(back.width, back.height, out.width, out.height);
    Object.assign(presented, { backW: back.width, backH: back.height, canvasW: out.width, canvasH: out.height, ...r });
    device.queue.writeBuffer(present.size, 0, new Float32Array([r.x, r.y, r.w, r.h]));
    const pass = encoder.beginRenderPass({ label: 'present', colorAttachments: [{ view: out.createView(), loadOp: 'clear', storeOp: 'store', clearValue: [0, 0, 0, 1] }] });
    pass.setPipeline(present.pipeline);
    pass.setBindGroup(0, present.group);
    pass.draw(3);
    pass.end();
  }

  /**
   * Replay one frame and present it. Synchronous. `input`: the frame's bytes (tw.gfx_frame alone), or the list of its
   * parts in order (gfx_frame_chunk ... gfx_frame), read as one stream.
   */
  function frame(input) {
    // bytes in shared wasm memory are copied first (they are only valid during the call anyway)
    const own1 = (b) => (typeof SharedArrayBuffer !== 'undefined' && b.buffer instanceof SharedArrayBuffer ? b.slice() : b);
    const own = Array.isArray(input) ? input.map(own1) : own1(input);
    let f;
    try {
      f = parseFrame(own);
    } catch (e) {
      logOnce('parse:' + e.message, 4, `gfx_frame: ${e.message}`);
      return false;
    }
    if (f.serial <= lastSerial) return false; // already replayed
    lastSerial = f.serial;
    let encoder = replay(f);
    stats.frames++;
    if (f.commands.length) stats.last = cur;
    if (capture && capture.serial === f.serial) capture.done = true;
    encoder ??= device.createCommandEncoder({ label: 'present' });
    presentInto(encoder);
    device.queue.submit([encoder.finish()]);
    for (const t of pendingDestroy) t.destroy();
    pendingDestroy = [];
    return true;
  }

  /** The RGBA8 rows of texture `id` (default: the back buffer), top row first. Waits for the GPU. */
  async function snapshot(id = 0) {
    const t = textures.get(id);
    if (!t) throw new Error(`snapshot: no texture ${id}`);
    const bpr = Math.ceil((t.width * 4) / 256) * 256;
    const buf = device.createBuffer({ label: 'snapshot', size: bpr * t.height, usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST });
    const enc = device.createCommandEncoder({ label: 'snapshot' });
    enc.copyTextureToBuffer({ texture: t.texture }, { buffer: buf, bytesPerRow: bpr, rowsPerImage: t.height }, [t.width, t.height]);
    device.queue.submit([enc.finish()]);
    await buf.mapAsync(GPUMapMode.READ);
    const src = new Uint8Array(buf.getMappedRange());
    const out = new Uint8Array(t.width * t.height * 4);
    for (let y = 0; y < t.height; y++) out.set(src.subarray(y * bpr, y * bpr + t.width * 4), y * t.width * 4);
    buf.unmap();
    buf.destroy();
    return { width: t.width, height: t.height, data: out };
  }

  /** Diagnostics: record the next frame that has commands; captured() returns it (with the shader labels known). */
  function captureNext() {
    captureArmed = true;
    capture = null;
  }
  function captured() {
    if (!capture) return null;
    for (const t of Object.values(capture.targets)) {
      const sc = t.spriteColour;
      sc.mean = sc.vertices ? sc.sum.map((v) => +(v / sc.vertices).toFixed(1)) : null;
      delete sc.sum;
      if (!sc.vertices) { sc.min = null; sc.max = null; }
    }
    return { ...capture, shaderLabels: [...shaders.values()].map((s) => `${s.def.id} ${s.def.label} (units ${s.def.units.join('/')}, ps ${s.def.psRegs}, vs ${s.def.vsRegs})`), messages: messages.slice() };
  }

  /** Resize the canvas at the next presentation (w x h device pixels). */
  function setCanvasSize(w, h) {
    if (!canvas || w <= 0 || h <= 0) return;
    pendingSize = { w, h };
  }
  /** Canvas pixels -> back-buffer pixels through the last presentation (identity before the first). */
  function toBack(x, y) {
    const p = presented;
    if (!p.w || !p.h) return { x, y };
    if (p.x === 0 && p.y === 0 && p.w === p.backW && p.h === p.backH) return { x, y };
    return { x: ((x - p.x) * p.backW) / p.w, y: ((y - p.y) * p.backH) / p.h };
  }

  /** Textures alive in the replayer now: count, and their pixels in MB (render targets separately). */
  function liveTextures() {
    let n = 0, mb = 0, targets = 0, targetMb = 0;
    for (const t of textures.values()) {
      const m = (t.width * t.height * 4) / 1048576;
      n++;
      mb += m;
      if (t.target) {
        targets++;
        targetMb += m;
      }
    }
    return { count: n, mb: +mb.toFixed(1), targets, targetMb: +targetMb.toFixed(1) };
  }

  return { frame, preload, snapshot, setCanvasSize, toBack, presented, liveTextures, captureNext, captured, device, stats, get canvasFormat() { return canvasFormat; }, _debug: { textures, shaders, pipelines, effectPipelines } };
}

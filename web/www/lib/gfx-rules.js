// The state mappings of render.rs (port/crates/terraria-app/src/gfx/render.rs) as WebGPU descriptor values.
// Pure: used by gfx.js, tested by test/gfx-rules.test.mjs.

import { BLEND_FN } from './frame-codec.js';

/** render.rs blend_factor, indexed by draw_list.rs Blend. */
export const BLEND_FACTOR = Object.freeze([
  'one', // One
  'zero', // Zero
  'src', // SourceColor
  'one-minus-src', // InverseSourceColor
  'src-alpha', // SourceAlpha
  'one-minus-src-alpha', // InverseSourceAlpha
  'dst', // DestinationColor
  'one-minus-dst', // InverseDestinationColor
  'dst-alpha', // DestinationAlpha
  'one-minus-dst-alpha', // InverseDestinationAlpha
  'constant', // BlendFactor
  'one-minus-constant', // InverseBlendFactor
  'src-alpha-saturated', // SourceAlphaSaturation
]);

/** render.rs blend_component's operation, indexed by BlendFunction. */
export const BLEND_OP = Object.freeze(['add', 'subtract', 'reverse-subtract', 'min', 'max']);

/** render.rs blend_component: Min / Max ignore the factors (Direct3D); WebGPU requires them to be One. */
export function blendComponent(src, dst, fn) {
  const operation = BLEND_OP[fn];
  const srcFactor = BLEND_FACTOR[src];
  const dstFactor = BLEND_FACTOR[dst];
  if (!operation || !srcFactor || !dstFactor) throw new RangeError(`bad blend (${src}, ${dst}, ${fn})`);
  if (fn === BLEND_FN.Min || fn === BLEND_FN.Max) return { srcFactor: 'one', dstFactor: 'one', operation };
  return { srcFactor, dstFactor, operation };
}

export function blendState(b) {
  return { color: blendComponent(b.colorSrc, b.colorDst, b.colorFn), alpha: blendComponent(b.alphaSrc, b.alphaDst, b.alphaFn) };
}

export const blendKey = (b) => `${b.colorSrc},${b.colorDst},${b.colorFn},${b.alphaSrc},${b.alphaDst},${b.alphaFn},${b.factor.join(',')},${b.writeMask}`;

/** draw_list.rs TextureFilter::min_mag_linear: (min linear, mag linear). One mip level, so the mip part does nothing. */
export function minMagLinear(filter) {
  switch (filter) {
    case 0: case 2: case 3: return [true, true]; // Linear, Anisotropic, LinearMipPoint
    case 1: case 4: return [false, false]; // Point, PointMipLinear
    case 5: case 6: return [true, false]; // MinLinearMagPoint*
    case 7: case 8: return [false, true]; // MinPointMagLinear*
    default: throw new RangeError(`bad filter ${filter}`);
  }
}

const ADDRESS_MODE = Object.freeze(['repeat', 'clamp-to-edge', 'mirror-repeat']); // Wrap, Clamp, Mirror

/** render.rs build_sampler. */
export function samplerDescriptor(s) {
  const [minLinear, magLinear] = minMagLinear(s.filter);
  const u = ADDRESS_MODE[s.u];
  const v = ADDRESS_MODE[s.v];
  if (!u || !v) throw new RangeError(`bad address mode (${s.u}, ${s.v})`);
  return {
    addressModeU: u,
    addressModeV: v,
    addressModeW: 'clamp-to-edge',
    magFilter: magLinear ? 'linear' : 'nearest',
    minFilter: minLinear ? 'linear' : 'nearest',
    mipmapFilter: 'nearest',
  };
}
export const samplerKey = (s) => `${s.filter},${s.u},${s.v}`;

/** render.rs build_pipeline's primitive state: XNA judges winding on screen; front_face Cw. */
export function primitiveState(topology, cull) {
  return {
    topology: topology === 1 ? 'line-list' : 'triangle-list',
    frontFace: 'cw',
    cullMode: cull === 1 ? 'front' : cull === 2 ? 'back' : 'none',
  };
}

/** render.rs clamp_rect: [x, y, w, h] of `r` clamped to a w x h target (never negative). */
export function clampRect(r, w, h) {
  const clamp = (v, lo, hi) => Math.min(Math.max(v, lo), hi);
  const x0 = clamp(r.x, 0, w);
  const y0 = clamp(r.y, 0, h);
  const x1 = clamp(r.x + r.w, x0, w);
  const y1 = clamp(r.y + r.h, y0, h);
  return [x0, y0, x1 - x0, y1 - y0];
}

/**
 * A frame's CreateTexture for an id that tw.gfx_texture preloaded (HOST_ABI.md, FRAME_FORMAT.md v2):
 * 'keep' when the preloaded texture has the same size and the command does not ask for a render target (the texture
 * and its contents stay, the preload mark is cleared); otherwise 'create' (recreated as usual: transparent black).
 * `preloaded` = Map(id -> { width, height }).
 */
export function createTextureAction(preloaded, id, width, height, target) {
  const p = preloaded.get(id);
  if (!p) return 'create';
  if (!target && p.width === Math.max(1, width) && p.height === Math.max(1, height)) return 'keep';
  return 'create';
}

/** The commands a render pass spans: from `i` up to the first that is not a draw / viewport / scissor. */
export function passEnd(commands, i, OP) {
  let j = i;
  while (j < commands.length && (commands[j].op === OP.DRAW || commands[j].op === OP.VIEWPORT || commands[j].op === OP.SCISSOR)) j++;
  return j;
}

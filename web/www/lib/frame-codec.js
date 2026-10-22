// FRAME_FORMAT.md version 2: the parser the replayer uses (parseFrame) and a builder (FrameBuilder) for tests and
// the JS mock module. Pure: tested by test/frame-codec.test.mjs.
//
// Version 2: a frame may arrive in several parts (tw.gfx_frame_chunk ... tw.gfx_frame); parseFrame reads a list of
// parts as one byte stream without joining them. n_commands = 0xFFFFFFFF means "commands until op 0 (End)".

export const MAGIC = 'TWF1';
export const VERTEX_BYTES = 20;
export const RAW_VERTEX_BYTES = 36;
export const NO_TEXTURE = 0xffffffff;
export const UNTIL_END = 0xffffffff;

export const OP = Object.freeze({
  END: 0, CREATE_TEXTURE: 1, UPLOAD: 2, DESTROY_TEXTURE: 3, SET_TARGET: 4, CLEAR: 5, VIEWPORT: 6, SCISSOR: 7, DRAW: 8,
  DEFINE_SHADER: 9, READBACK: 10,
});

// draw_list.rs numbering
export const BLEND = Object.freeze({
  One: 0, Zero: 1, SourceColor: 2, InverseSourceColor: 3, SourceAlpha: 4, InverseSourceAlpha: 5, DestinationColor: 6,
  InverseDestinationColor: 7, DestinationAlpha: 8, InverseDestinationAlpha: 9, BlendFactor: 10, InverseBlendFactor: 11,
  SourceAlphaSaturation: 12,
});
export const BLEND_FN = Object.freeze({ Add: 0, Subtract: 1, ReverseSubtract: 2, Min: 3, Max: 4 });
export const FILTER = Object.freeze({
  Linear: 0, Point: 1, Anisotropic: 2, LinearMipPoint: 3, PointMipLinear: 4, MinLinearMagPointMipLinear: 5,
  MinLinearMagPointMipPoint: 6, MinPointMagLinearMipLinear: 7, MinPointMagLinearMipPoint: 8,
});
export const ADDRESS = Object.freeze({ Wrap: 0, Clamp: 1, Mirror: 2 });
export const CULL = Object.freeze({ None: 0, CullClockwiseFace: 1, CullCounterClockwiseFace: 2 });
export const TOPOLOGY = Object.freeze({ Triangles: 0, Lines: 1 });

const blendState = (cs, cd, cf, as, ad, af) => Object.freeze({
  colorSrc: cs, colorDst: cd, colorFn: cf, alphaSrc: as, alphaDst: ad, alphaFn: af, factor: [255, 255, 255, 255], writeMask: 15,
});
/** XNA's BlendState presets. */
export const BLEND_STATES = Object.freeze({
  Opaque: blendState(BLEND.One, BLEND.Zero, BLEND_FN.Add, BLEND.One, BLEND.Zero, BLEND_FN.Add),
  AlphaBlend: blendState(BLEND.One, BLEND.InverseSourceAlpha, BLEND_FN.Add, BLEND.One, BLEND.InverseSourceAlpha, BLEND_FN.Add),
  Additive: blendState(BLEND.SourceAlpha, BLEND.One, BLEND_FN.Add, BLEND.SourceAlpha, BLEND.One, BLEND_FN.Add),
  NonPremultiplied: blendState(BLEND.SourceAlpha, BLEND.InverseSourceAlpha, BLEND_FN.Add, BLEND.SourceAlpha, BLEND.InverseSourceAlpha, BLEND_FN.Add),
});
export const SAMPLERS = Object.freeze({
  PointClamp: Object.freeze({ filter: FILTER.Point, u: ADDRESS.Clamp, v: ADDRESS.Clamp }),
  PointWrap: Object.freeze({ filter: FILTER.Point, u: ADDRESS.Wrap, v: ADDRESS.Wrap }),
  LinearClamp: Object.freeze({ filter: FILTER.Linear, u: ADDRESS.Clamp, v: ADDRESS.Clamp }),
  LinearWrap: Object.freeze({ filter: FILTER.Linear, u: ADDRESS.Wrap, v: ADDRESS.Wrap }),
});

const dec = new TextDecoder();
const enc = new TextEncoder();

/**
 * A little-endian reader over a list of byte arrays treated as one stream. A read that fits in one part is a view
 * into it; one that crosses a part boundary is copied (only small fields or an odd large range ever do).
 */
function streamReader(parts) {
  parts = parts.filter((p) => p.length > 0);
  const total = parts.reduce((n, p) => n + p.length, 0);
  const views = parts.map((p) => new DataView(p.buffer, p.byteOffset, p.byteLength));
  let i = 0; // current part
  let off = 0; // offset in it
  let pos = 0; // offset in the stream
  const settle = () => {
    while (i < parts.length && off === parts[i].length) {
      i++;
      off = 0;
    }
  };
  const need = (n, what) => {
    if (pos + n > total) throw new Error(`frame truncated at byte ${pos} reading ${what} (${n} bytes, ${total - pos} left)`);
  };
  function bytes(n, what) {
    need(n, what);
    settle();
    if (n === 0) return new Uint8Array(0);
    if (off + n <= parts[i].length) {
      const v = parts[i].subarray(off, off + n);
      off += n;
      pos += n;
      return v;
    }
    const out = new Uint8Array(n);
    let w = 0;
    while (w < n) {
      settle();
      const take = Math.min(n - w, parts[i].length - off);
      out.set(parts[i].subarray(off, off + take), w);
      w += take;
      off += take;
    }
    pos += n;
    return out;
  }
  function num(n, what, get) {
    need(n, what);
    settle();
    if (off + n <= parts[i].length) {
      const v = get(views[i], off);
      off += n;
      pos += n;
      return v;
    }
    const b = bytes(n, what);
    return get(new DataView(b.buffer, b.byteOffset, n), 0);
  }
  return {
    get pos() { return pos; },
    get left() { return total - pos; },
    bytes,
    u8: (w) => num(1, w, (d, o) => d.getUint8(o)),
    u32: (w) => num(4, w, (d, o) => d.getUint32(o, true)),
    i32: (w) => num(4, w, (d, o) => d.getInt32(o, true)),
    f64: (w) => num(8, w, (d, o) => d.getFloat64(o, true)),
  };
}

/**
 * Parse a frame: one Uint8Array, or a list of them (the parts of a chunked frame, in order). Byte ranges (vertices,
 * upload pixels, uniform blocks) are views into the parts where possible, valid as long as they are.
 * Throws an Error naming the stream offset on malformed input.
 */
export function parseFrame(input) {
  const r = streamReader(Array.isArray(input) ? input : [input]);
  const u8r = r.u8;
  const u32 = r.u32;
  const i32 = r.i32;
  const f64 = r.f64;
  const bytes = r.bytes;
  const str = (w) => dec.decode(bytes(u32(w + ' length'), w));
  const sampler = () => ({ filter: u8r('filter'), u: u8r('address_u'), v: u8r('address_v') });
  const rect = () => ({ x: i32('x'), y: i32('y'), w: i32('w'), h: i32('h') });

  const m = bytes(4, 'magic');
  const magic = String.fromCharCode(m[0], m[1], m[2], m[3]);
  if (magic !== MAGIC) throw new Error(`bad magic "${magic}" (want ${MAGIC})`);
  const serial = f64('serial');
  const backW = u32('back_w');
  const backH = u32('back_h');
  const nVertices = u32('n_vertices');
  const nRaw = u32('n_raw');
  const nCommands = u32('n_commands');
  const vertices = bytes(nVertices * VERTEX_BYTES, 'vertices');
  const raw = bytes(nRaw * RAW_VERTEX_BYTES, 'raw vertices');
  const commands = [];
  const untilEnd = nCommands === UNTIL_END;
  let ended = false;
  for (let c = 0; !ended && (untilEnd || c < nCommands); c++) {
    const at = r.pos;
    const op = u8r('op');
    switch (op) {
      case OP.END:
        ended = true;
        break;
      case OP.CREATE_TEXTURE:
        commands.push({ op, id: u32('id'), w: u32('w'), h: u32('h'), target: u8r('target') !== 0 });
        break;
      case OP.UPLOAD: {
        const id = u32('id');
        const r = rect();
        if (r.w < 0 || r.h < 0) throw new Error(`upload at byte ${at}: negative size`);
        commands.push({ op, id, ...r, rgba: bytes(r.w * r.h * 4, 'upload pixels') });
        break;
      }
      case OP.DESTROY_TEXTURE:
        commands.push({ op, id: u32('id') });
        break;
      case OP.SET_TARGET:
        commands.push({ op, id: u32('id'), preserve: u8r('preserve') !== 0 });
        break;
      case OP.CLEAR:
        commands.push({ op, rgba: [u8r('r'), u8r('g'), u8r('b'), u8r('a')] });
        break;
      case OP.VIEWPORT:
        commands.push({ op, rect: rect() });
        break;
      case OP.SCISSOR: {
        const on = u8r('on') !== 0;
        const r = rect();
        commands.push({ op, rect: on ? r : null });
        break;
      }
      case OP.DRAW: {
        const texture = u32('texture');
        const blend = {
          colorSrc: u8r('color_src'), colorDst: u8r('color_dst'), colorFn: u8r('color_fn'),
          alphaSrc: u8r('alpha_src'), alphaDst: u8r('alpha_dst'), alphaFn: u8r('alpha_fn'),
          factor: [u8r('factor'), u8r('factor'), u8r('factor'), u8r('factor')], writeMask: u8r('write_mask'),
        };
        const samp = sampler();
        const cull = u8r('cull');
        const topology = u8r('topology');
        const first = u32('first');
        const count = u32('count');
        const shader = u32('shader');
        const d = { op, texture, blend, sampler: samp, cull, topology, first, count, shader, ps: null, vs: null, units: [] };
        if (shader !== 0) {
          d.ps = bytes(u32('ps_bytes length'), 'ps_bytes');
          d.vs = bytes(u32('vs_bytes length'), 'vs_bytes');
          const n = u8r('n_units');
          for (let k = 0; k < n; k++) d.units.push({ unit: u8r('unit'), texture: u32('unit texture'), sampler: sampler() });
        }
        commands.push(d);
        break;
      }
      case OP.DEFINE_SHADER: {
        const id = u32('id');
        const label = str('label');
        const wgsl = str('wgsl');
        const n = u8r('units');
        const units = [];
        for (let k = 0; k < n; k++) units.push(u8r('unit'));
        commands.push({ op, id, label, wgsl, units, psRegs: u32('ps_regs'), vsRegs: i32('vs_regs') });
        break;
      }
      case OP.READBACK:
        commands.push({ op, id: u32('id'), request: f64('request') });
        break;
      default:
        throw new Error(`unknown op ${op} at byte ${at} (command ${c})`);
    }
  }
  if (r.left !== 0) throw new Error(`${r.left} trailing bytes after ${commands.length} commands${ended ? ' and End' : ''}`);
  return { serial, backW, backH, nVertices, nRaw, vertices, raw, commands, untilEnd };
}

/** Builds frames in the same encoding. Coordinates for quads are in target pixels; vertices go out in clip space. */
export class FrameBuilder {
  constructor(serial, backW, backH) {
    this.serial = serial;
    this.backW = backW;
    this.backH = backH;
    this.verts = [];
    this.raws = [];
    this.cmds = [];
  }
  /** One sprite vertex; returns its index. pos in clip space, color [r,g,b,a] bytes. */
  vertex(x, y, u, v, color) {
    this.verts.push([x, y, u, v, color]);
    return this.verts.length - 1;
  }
  rawVertex(pos4, tex4, color) {
    this.raws.push([pos4, tex4, color]);
    return this.raws.length - 1;
  }
  /** Two triangles covering pixels [x0,x1) x [y0,y1) of a w x h target, UV rect [u0,u1] x [v0,v1]. Returns { first, count }. */
  quad(w, h, x0, y0, x1, y1, color = [255, 255, 255, 255], u0 = 0, v0 = 0, u1 = 1, v1 = 1) {
    const cx = (x) => (x / w) * 2 - 1;
    const cy = (y) => 1 - (y / h) * 2;
    const first = this.verts.length;
    // clockwise on screen (y down), as SpriteBatch emits
    const tl = [cx(x0), cy(y0), u0, v0];
    const tr = [cx(x1), cy(y0), u1, v0];
    const bl = [cx(x0), cy(y1), u0, v1];
    const br = [cx(x1), cy(y1), u1, v1];
    for (const p of [tl, tr, bl, tr, br, bl]) this.vertex(p[0], p[1], p[2], p[3], color);
    return { first, count: 6 };
  }
  createTexture(id, w, h, target = false) { this.cmds.push({ op: OP.CREATE_TEXTURE, id, w, h, target }); return this; }
  upload(id, x, y, w, h, rgba) { this.cmds.push({ op: OP.UPLOAD, id, x, y, w, h, rgba }); return this; }
  destroyTexture(id) { this.cmds.push({ op: OP.DESTROY_TEXTURE, id }); return this; }
  setTarget(id, preserve = false) { this.cmds.push({ op: OP.SET_TARGET, id, preserve }); return this; }
  clear(r, g, b, a) { this.cmds.push({ op: OP.CLEAR, rgba: [r, g, b, a] }); return this; }
  viewport(x, y, w, h) { this.cmds.push({ op: OP.VIEWPORT, rect: { x, y, w, h } }); return this; }
  scissor(rect) { this.cmds.push({ op: OP.SCISSOR, rect }); return this; }
  readback(id, request) { this.cmds.push({ op: OP.READBACK, id, request }); return this; }
  defineShader({ id, label = '', wgsl, units = [0], psRegs = 0, vsRegs = -1 }) {
    this.cmds.push({ op: OP.DEFINE_SHADER, id, label, wgsl, units, psRegs, vsRegs });
    return this;
  }
  /** d: { texture, blend, sampler, cull, topology, first, count, shader, ps, vs, units: [{unit, texture, sampler}] } */
  draw(d) {
    this.cmds.push({
      op: OP.DRAW, texture: d.texture ?? NO_TEXTURE, blend: d.blend ?? BLEND_STATES.AlphaBlend, sampler: d.sampler ?? SAMPLERS.PointClamp,
      cull: d.cull ?? CULL.None, topology: d.topology ?? TOPOLOGY.Triangles, first: d.first, count: d.count, shader: d.shader ?? 0,
      ps: d.ps ?? new Uint8Array(0), vs: d.vs ?? new Uint8Array(0), units: d.units ?? [],
    });
    return this;
  }
  /**
   * The frame's bytes. untilEnd: n_commands = 0xFFFFFFFF and an End op after the last command (what the module sends
   * from version 2 on). Returns one Uint8Array, or with `chunks: true` { bytes, cuts } where cuts are the byte offsets
   * of every command boundary (for splitting into gfx_frame_chunk parts).
   */
  build({ untilEnd = false, chunks = false } = {}) {
    const parts = [];
    let size = 0;
    const cuts = [];
    const push = (b) => { parts.push(b); size += b.length; };
    const u8 = (v) => push(Uint8Array.of(v));
    const num = (setter, n, v) => { const b = new Uint8Array(n); new DataView(b.buffer)[setter](0, v, true); push(b); };
    const u32 = (v) => num('setUint32', 4, v >>> 0);
    const i32 = (v) => num('setInt32', 4, v | 0);
    const f32 = (v) => num('setFloat32', 4, v);
    const f64 = (v) => num('setFloat64', 8, v);
    const str = (s) => { const b = enc.encode(s); u32(b.length); push(b); };
    const samp = (s) => { u8(s.filter); u8(s.u); u8(s.v); };
    const rect = (r) => { i32(r.x); i32(r.y); i32(r.w); i32(r.h); };

    push(enc.encode(MAGIC));
    f64(this.serial);
    u32(this.backW);
    u32(this.backH);
    u32(this.verts.length);
    u32(this.raws.length);
    u32(untilEnd ? UNTIL_END : this.cmds.length);
    for (const [x, y, u, v, c] of this.verts) { f32(x); f32(y); f32(u); f32(v); push(Uint8Array.from(c)); }
    for (const [p, t, c] of this.raws) { p.forEach(f32); t.forEach(f32); push(Uint8Array.from(c)); }
    for (const c of [...this.cmds, ...(untilEnd ? [{ op: OP.END }] : [])]) {
      cuts.push(size);
      u8(c.op);
      switch (c.op) {
        case OP.END: break;
        case OP.CREATE_TEXTURE: u32(c.id); u32(c.w); u32(c.h); u8(c.target ? 1 : 0); break;
        case OP.UPLOAD: u32(c.id); rect(c); push(Uint8Array.from(c.rgba)); break;
        case OP.DESTROY_TEXTURE: u32(c.id); break;
        case OP.SET_TARGET: u32(c.id); u8(c.preserve ? 1 : 0); break;
        case OP.CLEAR: c.rgba.forEach(u8); break;
        case OP.VIEWPORT: rect(c.rect); break;
        case OP.SCISSOR: u8(c.rect ? 1 : 0); rect(c.rect ?? { x: 0, y: 0, w: 0, h: 0 }); break;
        case OP.DRAW: {
          const b = c.blend;
          u32(c.texture);
          [b.colorSrc, b.colorDst, b.colorFn, b.alphaSrc, b.alphaDst, b.alphaFn].forEach(u8);
          b.factor.forEach(u8);
          u8(b.writeMask);
          samp(c.sampler);
          u8(c.cull); u8(c.topology); u32(c.first); u32(c.count); u32(c.shader);
          if (c.shader !== 0) {
            u32(c.ps.length); push(c.ps);
            u32(c.vs.length); push(c.vs);
            u8(c.units.length);
            for (const un of c.units) { u8(un.unit); u32(un.texture); samp(un.sampler ?? SAMPLERS.PointClamp); }
          }
          break;
        }
        case OP.DEFINE_SHADER: u32(c.id); str(c.label); str(c.wgsl); u8(c.units.length); c.units.forEach(u8); u32(c.psRegs); i32(c.vsRegs); break;
        case OP.READBACK: u32(c.id); f64(c.request); break;
        default: throw new Error(`unknown op ${c.op}`);
      }
    }
    const out = new Uint8Array(size);
    let o = 0;
    for (const p of parts) { out.set(p, o); o += p.length; }
    return chunks ? { bytes: out, cuts } : out;
  }

  /**
   * The frame as gfx_frame_chunk parts plus the final gfx_frame part, cut at command boundaries: every `every`
   * commands (the first part also holds the header and vertices). untilEnd is implied.
   */
  buildChunks(every = 1) {
    const { bytes, cuts } = this.build({ untilEnd: true, chunks: true });
    const at = cuts.filter((_, k) => k > 0 && k % every === 0);
    const out = [];
    let prev = 0;
    for (const c of at) {
      out.push(bytes.slice(prev, c));
      prev = c;
    }
    out.push(bytes.slice(prev));
    return out;
  }
}

/** Float registers (vec4 rows) as the bytes of a uniform block. */
export function regs(...rows) {
  const f = new Float32Array(rows.length * 4);
  rows.forEach((r, i) => f.set(r, i * 4));
  return new Uint8Array(f.buffer);
}

// The input record format (HOST_ABI.md, "Input records"): fixed 32-byte little-endian records.
// Pure: used by main.js (encode), host.js (queue + drain) and the mock (decode); tested by test/input-codec.test.mjs.
//
// Common header, every record:
//   0  u8   kind
//   1  u8   state     (1 = down / pressed / focused / connected, 0 = up / released / blurred / disconnected)
//   2  u16  mods      (MOD bits)
//   4  u32  a         (per kind)
//   8  f64  time_ms   (the Worker's performance.now() clock)
//   16..31  four 32-bit slots s0..s3 (per kind: f32 or u32)

export const REC = 32;

export const KIND = Object.freeze({
  KEY: 1,
  TEXT: 2,
  MOUSE_MOVE: 3,
  MOUSE_BUTTON: 4,
  WHEEL: 5,
  FOCUS: 6,
  RESIZE: 7,
  GAMEPAD_CONNECT: 8,
  GAMEPAD_AXES: 9,
  GAMEPAD_BUTTONS: 10,
  OVERFLOW: 15,
});
export const KIND_NAME = Object.freeze(Object.fromEntries(Object.entries(KIND).map(([k, v]) => [v, k])));

export const MOD = Object.freeze({ SHIFT: 1, CTRL: 2, ALT: 4, META: 8, CAPS: 16, NUM: 32, REPEAT: 64 });

// KeyboardEvent.code -> id (the array index). APPEND ONLY: ids are part of the ABI.
const range = (pre, a, b) => Array.from({ length: b - a + 1 }, (_, i) => `${pre}${a + i}`);
export const KEY_CODES = Object.freeze([
  'Unidentified', // 0
  ...Array.from({ length: 26 }, (_, i) => 'Key' + String.fromCharCode(65 + i)), // 1..26 KeyA..KeyZ
  ...range('Digit', 0, 9), // 27..36
  'Enter', 'Escape', 'Backspace', 'Tab', 'Space', // 37..41
  'Minus', 'Equal', 'BracketLeft', 'BracketRight', 'Backslash', // 42..46
  'Semicolon', 'Quote', 'Backquote', 'Comma', 'Period', 'Slash', // 47..52
  'CapsLock', // 53
  ...range('F', 1, 12), // 54..65
  'PrintScreen', 'ScrollLock', 'Pause', // 66..68
  'Insert', 'Home', 'PageUp', 'Delete', 'End', 'PageDown', // 69..74
  'ArrowRight', 'ArrowLeft', 'ArrowDown', 'ArrowUp', // 75..78
  'NumLock', 'NumpadDivide', 'NumpadMultiply', 'NumpadSubtract', 'NumpadAdd', 'NumpadEnter', // 79..84
  ...range('Numpad', 1, 9), 'Numpad0', 'NumpadDecimal', // 85..95
  'IntlBackslash', 'ContextMenu', 'NumpadEqual', // 96..98
  ...range('F', 13, 24), // 99..110
  'ControlLeft', 'ShiftLeft', 'AltLeft', 'MetaLeft', // 111..114
  'ControlRight', 'ShiftRight', 'AltRight', 'MetaRight', // 115..118
  'IntlRo', 'IntlYen', 'KanaMode', 'Lang1', 'Lang2', 'Convert', 'NonConvert', // 119..125
  'AudioVolumeMute', 'AudioVolumeDown', 'AudioVolumeUp', // 126..128
  'MediaTrackNext', 'MediaTrackPrevious', 'MediaStop', 'MediaPlayPause', // 129..132
  'BrowserBack', 'BrowserForward', 'NumpadComma', // 133..135
]);
const CODE_ID = new Map(KEY_CODES.map((c, i) => [c, i]));
export const codeId = (code) => CODE_ID.get(code) ?? 0;
export const codeName = (id) => KEY_CODES[id] ?? 'Unidentified';

const enc = new TextEncoder();
const dec = new TextDecoder();

function header(kind, state, mods, a, time) {
  const u8 = new Uint8Array(REC);
  const dv = new DataView(u8.buffer);
  dv.setUint8(0, kind);
  dv.setUint8(1, state ? 1 : 0);
  dv.setUint16(2, (mods ?? 0) & 0xffff, true);
  dv.setUint32(4, (a ?? 0) >>> 0, true);
  dv.setFloat64(8, time ?? 0, true);
  return { u8, dv };
}

/** Mods from a DOM KeyboardEvent / MouseEvent / WheelEvent. */
export function modsOf(e, repeat = false) {
  let m = 0;
  if (e.shiftKey) m |= MOD.SHIFT;
  if (e.ctrlKey) m |= MOD.CTRL;
  if (e.altKey) m |= MOD.ALT;
  if (e.metaKey) m |= MOD.META;
  if (typeof e.getModifierState === 'function') {
    if (e.getModifierState('CapsLock')) m |= MOD.CAPS;
    if (e.getModifierState('NumLock')) m |= MOD.NUM;
  }
  if (repeat) m |= MOD.REPEAT;
  return m;
}

/** The key's character as one Unicode scalar value, or 0 when `key` is a named key ("Enter", "Shift", ...). */
export function keyCodepoint(key) {
  if (typeof key !== 'string' || key.length === 0) return 0;
  const cp = key.codePointAt(0);
  return String.fromCodePoint(cp).length === key.length ? cp : 0;
}

export function encodeKey({ down, code, key, mods = 0, repeat = false, location = 0, time = 0 }) {
  const { u8, dv } = header(KIND.KEY, down, mods | (repeat ? MOD.REPEAT : 0), codeId(code), time);
  dv.setUint32(16, keyCodepoint(key), true);
  dv.setUint32(20, location >>> 0, true);
  return u8;
}

/** Text is split at code point boundaries into records of at most 16 UTF-8 bytes. */
export function encodeText(text, time = 0) {
  const chunks = [];
  let cur = [];
  let curLen = 0;
  for (const ch of text) {
    const b = enc.encode(ch);
    if (curLen + b.length > 16) {
      chunks.push(cur);
      cur = [];
      curLen = 0;
    }
    cur.push(b);
    curLen += b.length;
  }
  if (curLen > 0) chunks.push(cur);
  const out = new Uint8Array(chunks.length * REC);
  chunks.forEach((parts, i) => {
    const len = parts.reduce((n, b) => n + b.length, 0);
    const { u8 } = header(KIND.TEXT, 0, 0, len, time);
    let o = 16;
    for (const b of parts) {
      u8.set(b, o);
      o += b.length;
    }
    out.set(u8, i * REC);
  });
  return out;
}

export function encodeMouseMove({ x, y, buttons = 0, mods = 0, time = 0 }) {
  const { u8, dv } = header(KIND.MOUSE_MOVE, 0, mods, 0, time);
  dv.setFloat32(16, x, true);
  dv.setFloat32(20, y, true);
  dv.setUint32(24, buttons >>> 0, true);
  return u8;
}

export function encodeMouseButton({ down, button, x, y, buttons = 0, mods = 0, time = 0 }) {
  const { u8, dv } = header(KIND.MOUSE_BUTTON, down, mods, button, time);
  dv.setFloat32(16, x, true);
  dv.setFloat32(20, y, true);
  dv.setUint32(24, buttons >>> 0, true);
  return u8;
}

export function encodeWheel({ x, y, dx = 0, dy = 0, mods = 0, time = 0 }) {
  const { u8, dv } = header(KIND.WHEEL, 0, mods, 0, time);
  dv.setFloat32(16, x, true);
  dv.setFloat32(20, y, true);
  dv.setFloat32(24, dy, true);
  dv.setFloat32(28, dx, true);
  return u8;
}

export function encodeFocus({ focused, time = 0 }) {
  return header(KIND.FOCUS, focused, 0, 0, time).u8;
}

export function encodeResize({ pixelWidth, pixelHeight, cssWidth, cssHeight, dpr, time = 0 }) {
  const { u8, dv } = header(KIND.RESIZE, 0, 0, pixelWidth, time);
  dv.setFloat32(16, cssWidth, true);
  dv.setFloat32(20, cssHeight, true);
  dv.setFloat32(24, dpr, true);
  dv.setUint32(28, pixelHeight >>> 0, true);
  return u8;
}

export function encodeGamepadConnect({ index, connected, standard = false, axes = 0, buttons = 0, time = 0 }) {
  const { u8, dv } = header(KIND.GAMEPAD_CONNECT, connected, 0, index, time);
  dv.setUint32(16, standard ? 1 : 0, true);
  dv.setUint32(20, axes >>> 0, true);
  dv.setUint32(24, buttons >>> 0, true);
  return u8;
}

export function encodeGamepadAxes({ index, axes, time = 0 }) {
  const { u8, dv } = header(KIND.GAMEPAD_AXES, 0, 0, index, time);
  for (let i = 0; i < 4; i++) dv.setFloat32(16 + 4 * i, Number(axes?.[i] ?? 0), true);
  return u8;
}

/** pressed: bit i = button i pressed (buttons 0..31). lt / rt: analog values of buttons 6 and 7 (standard mapping). */
export function encodeGamepadButtons({ index, pressed, lt = 0, rt = 0, time = 0 }) {
  const { u8, dv } = header(KIND.GAMEPAD_BUTTONS, 0, 0, index, time);
  dv.setUint32(16, pressed >>> 0, true);
  dv.setFloat32(20, lt, true);
  dv.setFloat32(24, rt, true);
  return u8;
}

export function encodeOverflow({ dropped, time = 0 }) {
  return header(KIND.OVERFLOW, 0, 0, dropped, time).u8;
}

/** Concatenate records. */
export function concat(list) {
  const n = list.reduce((s, r) => s + r.length, 0);
  const out = new Uint8Array(n);
  let o = 0;
  for (const r of list) {
    out.set(r, o);
    o += r.length;
  }
  return out;
}

/** Decode one record at `off` of a Uint8Array (any backing buffer). */
export function decode(u8, off = 0) {
  const dv = new DataView(u8.buffer, u8.byteOffset + off, REC);
  const kind = dv.getUint8(0);
  const r = {
    kind,
    name: KIND_NAME[kind] ?? 'UNKNOWN',
    state: dv.getUint8(1),
    mods: dv.getUint16(2, true),
    a: dv.getUint32(4, true),
    time: dv.getFloat64(8, true),
  };
  const f = (o) => dv.getFloat32(o, true);
  const u = (o) => dv.getUint32(o, true);
  switch (kind) {
    case KIND.KEY:
      return { ...r, down: r.state === 1, code: codeName(r.a), repeat: (r.mods & MOD.REPEAT) !== 0, key: u(16), location: u(20) };
    case KIND.TEXT: {
      const len = Math.min(r.a, 16);
      const bytes = new Uint8Array(len);
      for (let i = 0; i < len; i++) bytes[i] = dv.getUint8(16 + i);
      return { ...r, text: dec.decode(bytes) };
    }
    case KIND.MOUSE_MOVE:
      return { ...r, x: f(16), y: f(20), buttons: u(24) };
    case KIND.MOUSE_BUTTON:
      return { ...r, down: r.state === 1, button: r.a, x: f(16), y: f(20), buttons: u(24) };
    case KIND.WHEEL:
      return { ...r, x: f(16), y: f(20), dy: f(24), dx: f(28) };
    case KIND.FOCUS:
      return { ...r, focused: r.state === 1 };
    case KIND.RESIZE:
      return { ...r, pixelWidth: r.a, pixelHeight: u(28), cssWidth: f(16), cssHeight: f(20), dpr: f(24) };
    case KIND.GAMEPAD_CONNECT:
      return { ...r, index: r.a, connected: r.state === 1, standard: u(16) === 1, axes: u(20), buttons: u(24) };
    case KIND.GAMEPAD_AXES:
      return { ...r, index: r.a, axes: [f(16), f(20), f(24), f(28)] };
    case KIND.GAMEPAD_BUTTONS:
      return { ...r, index: r.a, pressed: u(16), lt: f(20), rt: f(24) };
    case KIND.OVERFLOW:
      return { ...r, dropped: r.a };
    default:
      return r;
  }
}

export function decodeAll(u8) {
  const out = [];
  for (let o = 0; o + REC <= u8.length; o += REC) out.push(decode(u8, o));
  return out;
}

/** Add `delta` ms to every record's time (main-thread epoch time -> Worker performance.now()). In place. */
export function rebaseTimes(u8, delta) {
  const dv = new DataView(u8.buffer, u8.byteOffset, u8.byteLength);
  for (let o = 0; o + REC <= u8.length; o += REC) dv.setFloat64(o + 8, dv.getFloat64(o + 8, true) + delta, true);
  return u8;
}

/**
 * The Worker's input queue. Records go in as they arrive; input_next drains them into wasm memory.
 * - Consecutive MOUSE_MOVE records with the same buttons and mods are coalesced (the newest wins).
 * - When more than `cap` records are waiting, the oldest are dropped and the next drain starts with one OVERFLOW
 *   record whose `a` is the number dropped.
 */
export class InputQueue {
  constructor(cap = 4096) {
    this.cap = cap;
    this.q = [];
    this.head = 0;
    this.dropped = 0;
  }
  get length() {
    return this.q.length - this.head + (this.dropped > 0 ? 1 : 0);
  }
  push(rec) {
    if (rec.length !== REC) throw new RangeError('record must be 32 bytes');
    const last = this.q.length > this.head ? this.q[this.q.length - 1] : null;
    if (last && rec[0] === KIND.MOUSE_MOVE && last[0] === KIND.MOUSE_MOVE &&
        last[2] === rec[2] && last[3] === rec[3] &&
        last[24] === rec[24] && last[25] === rec[25] && last[26] === rec[26] && last[27] === rec[27]) {
      this.q[this.q.length - 1] = rec;
      return;
    }
    this.q.push(rec);
    while (this.q.length - this.head > this.cap) {
      this.head++;
      this.dropped++;
    }
    if (this.head > 1024 && this.head * 2 > this.q.length) {
      this.q = this.q.slice(this.head);
      this.head = 0;
    }
  }
  /** Push every record of a buffer (length a multiple of 32). */
  pushAll(u8) {
    for (let o = 0; o + REC <= u8.length; o += REC) this.push(u8.slice(o, o + REC));
  }
  /** Write as many whole records as fit into `out` (a Uint8Array view). Returns the number of bytes written. */
  drain(out, now = 0) {
    let o = 0;
    if (this.dropped > 0 && out.length >= REC) {
      out.set(encodeOverflow({ dropped: this.dropped, time: now }), 0);
      this.dropped = 0;
      o = REC;
    }
    while (this.head < this.q.length && o + REC <= out.length) {
      out.set(this.q[this.head++], o);
      o += REC;
    }
    if (this.head === this.q.length) {
      this.q = [];
      this.head = 0;
    }
    return o;
  }
}

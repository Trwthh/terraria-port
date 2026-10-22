// The `tw` imports of HOST_ABI.md (version 2), as plain JS functions. Runs in the game Worker.
//
// Functions listed in SUSPENDING may return a Promise (OPFS directory operations and createSyncAccessHandle are
// async). For a real wasm module the worker wraps exactly those in `new WebAssembly.Suspending(fn)`; to the module
// they look synchronous. Every other import is synchronous and never returns a Promise.
// The file tree itself (index, shared access handles, write guard) is vfs.js, shared with wasi.js.

import { ERRNO, MODE, parsePath, errnoFromDomError } from './lib/paths.js';
import { REC, InputQueue, rebaseTimes } from './lib/input-codec.js';
import { ringViews, ringWrite, ringFree, ringAvailable, TARGET_FRAMES, RING_FRAMES, CHANNELS, SAMPLE_RATE } from './lib/audio-ring.js';
import { then } from './vfs.js';

export const SUSPENDING = new Set(['fs_open', 'fs_mkdir', 'fs_remove', 'fs_rename', 'yield_frame']);

const MAX_OPEN = 1024;
const I32_MAX = 0x7fffffff;
const enc = new TextEncoder();
const dec = new TextDecoder();

/**
 * @param {object} o
 * @param {ReturnType<import('./vfs.js').createVfs>} o.vfs  the shared file tree
 * @param {(msg: object, transfer?: Transferable[]) => void} o.post  message to the page
 */
export function createHost({ vfs, post }) {
  let memory = null;
  const buf = () => memory.buffer;
  const inBounds = (ptr, len) => memory !== null && ptr + len <= memory.buffer.byteLength;
  /** Copy a UTF-8 string out of wasm memory (TextDecoder does not accept views of a SharedArrayBuffer). */
  function readStr(ptr, len) {
    ptr >>>= 0;
    len >>>= 0;
    if (!inBounds(ptr, len)) return null;
    return dec.decode(new Uint8Array(buf(), ptr, len).slice());
  }
  function parseAt(ptr, len) {
    const s = readStr(ptr, len);
    if (s === null) return { err: ERRNO.EFAULT };
    return parsePath(s);
  }

  // ---------------------------------------------------------------- files
  const fds = new Map(); // handle -> { node, s, write, append }
  let nextFd = 1;
  function newFd(node, s, mode) {
    const fd = nextFd;
    nextFd = nextFd >= I32_MAX ? 1 : nextFd + 1;
    fds.set(fd, { node, s, write: mode !== MODE.READ, append: mode === MODE.APPEND });
    return fd;
  }

  function fs_open(ptr, len, mode) {
    const p = parseAt(ptr, len);
    if (p.err) return -p.err;
    if (mode !== MODE.READ && mode !== MODE.WRITE && mode !== MODE.APPEND) return -ERRNO.EINVAL;
    if (fds.size >= MAX_OPEN) return -ERRNO.EMFILE;
    const opts = mode === MODE.READ ? {} : mode === MODE.WRITE ? { write: true, create: true, trunc: true } : { write: true, create: true };
    // another Worker's changes under /saves first (vfs.js: the shared tree)
    return then(vfs.ready(p.parts), () => then(vfs.openFile(p.parts, opts), (r) => (typeof r === 'number' ? r : newFd(r.node, r.s, mode))));
  }

  function fs_size(h) {
    const f = fds.get(h);
    if (!f) return -ERRNO.EBADF;
    try {
      return f.s.access.getSize();
    } catch (e) {
      return -errnoFromDomError(e);
    }
  }

  function fs_read(h, offset, ptr, len) {
    const f = fds.get(h);
    if (!f) return -ERRNO.EBADF;
    ptr >>>= 0;
    len = Math.min(len >>> 0, I32_MAX);
    if (!(offset >= 0) || !Number.isFinite(offset)) return -ERRNO.EINVAL;
    if (!inBounds(ptr, len)) return -ERRNO.EFAULT;
    try {
      return f.s.access.read(new Uint8Array(buf(), ptr, len), { at: offset });
    } catch (e) {
      return -errnoFromDomError(e);
    }
  }

  function fs_write(h, offset, ptr, len) {
    const f = fds.get(h);
    if (!f) return -ERRNO.EBADF;
    if (!f.write) return -ERRNO.EBADF;
    ptr >>>= 0;
    len = Math.min(len >>> 0, I32_MAX);
    if (!f.append && (!(offset >= 0) || !Number.isFinite(offset))) return -ERRNO.EINVAL;
    if (!inBounds(ptr, len)) return -ERRNO.EFAULT;
    try {
      const at = f.append ? f.s.access.getSize() : offset;
      return f.s.access.write(new Uint8Array(buf(), ptr, len), { at });
    } catch (e) {
      return -errnoFromDomError(e);
    }
  }

  function fs_close(h) {
    const f = fds.get(h);
    if (!f) return -ERRNO.EBADF;
    fds.delete(h);
    return vfs.release(f.node, f.s, f.write);
  }

  function fs_stat(ptr, len) {
    const p = parseAt(ptr, len);
    if (p.err) return -1;
    const node = vfs.resolve(p.parts);
    if (!node) return -1;
    return node.kind === 'dir' ? 1 : 0;
  }

  function fs_list(ptr, len, outPtr, outCap) {
    const p = parseAt(ptr, len);
    if (p.err) return -p.err;
    const node = vfs.resolve(p.parts);
    if (!node) return -ERRNO.ENOENT;
    if (node.kind !== 'dir') return -ERRNO.ENOTDIR;
    const names = vfs.list(node).map((e) => (e.kind === 'dir' ? e.name + '/' : e.name)).sort();
    const bytes = enc.encode(names.join('\n'));
    outPtr >>>= 0;
    outCap >>>= 0;
    if (bytes.length > outCap) return -Math.max(bytes.length, 256);
    if (!inBounds(outPtr, bytes.length)) return -ERRNO.EFAULT;
    new Uint8Array(buf(), outPtr, bytes.length).set(bytes);
    return bytes.length;
  }

  function fs_mkdir(ptr, len) {
    const p = parseAt(ptr, len);
    if (p.err) return -p.err;
    return vfs.mkdir(p.parts, { recursive: true });
  }

  function fs_remove(ptr, len) {
    const p = parseAt(ptr, len);
    if (p.err) return -p.err;
    return vfs.remove(p.parts, 'any');
  }

  function fs_rename(fromPtr, fromLen, toPtr, toLen) {
    const a = parseAt(fromPtr, fromLen);
    if (a.err) return -a.err;
    const b = parseAt(toPtr, toLen);
    if (b.err) return -b.err;
    return vfs.rename(a.parts, b.parts);
  }

  // ---------------------------------------------------------------- time, randomness, log, exit, system
  const now_ms = () => performance.now();
  const wall_ms = () => Date.now();

  function random(ptr, len) {
    ptr >>>= 0;
    len >>>= 0;
    if (!inBounds(ptr, len)) return -ERRNO.EFAULT;
    fillRandom(new Uint8Array(buf(), ptr, len));
    return 0;
  }

  const LEVELS = ['trace', 'debug', 'info', 'warn', 'error'];
  function logText(level, text) {
    const lv = Math.max(0, Math.min(4, level | 0));
    const fn = lv >= 4 ? console.error : lv === 3 ? console.warn : lv <= 1 ? console.debug : console.info;
    fn.call(console, `[game ${LEVELS[lv]}] ${text}`);
    post({ type: 'log', level: lv, text });
  }
  function log(level, ptr, len) {
    logText(level, readStr(ptr, len) ?? '<log: bad range>');
  }

  const state = { exitCode: null };
  function exit(code) {
    if (state.exitCode !== null) return;
    state.exitCode = code | 0;
    post({ type: 'exit', code: state.exitCode });
  }

  function cpu_count() {
    const n = Number(globalThis.navigator?.hardwareConcurrency);
    return Number.isFinite(n) && n >= 1 ? Math.floor(n) : 1;
  }

  // ---------------------------------------------------------------- clipboard
  // The page forwards the text of every paste event (setClipboardText); clipboard_set asks the page to write the
  // system clipboard on the player's next click or key press (browsers allow it only then).
  let clipboard = '';
  function clipboard_get(outPtr, cap) {
    outPtr >>>= 0;
    cap >>>= 0;
    const bytes = enc.encode(clipboard);
    if (bytes.length > cap) return -Math.max(bytes.length, 256);
    if (!inBounds(outPtr, bytes.length)) return -ERRNO.EFAULT;
    new Uint8Array(buf(), outPtr, bytes.length).set(bytes);
    return bytes.length;
  }
  function clipboard_set(ptr, len) {
    const text = readStr(ptr, len);
    if (text === null) return -ERRNO.EFAULT;
    clipboard = text;
    post({ type: 'clipboard-set', text });
    return 0;
  }

  // ---------------------------------------------------------------- window (HOST_ABI.md "Window")
  // the game window's title (the page's document title) and, after every presented frame, the game's own mouse
  // position (Main.mouseX / mouseY, back-buffer pixels), IsMouseVisible (bit 0), Main.gameMenu (bit 1) and
  // Main.showSplash (bit 2)
  const windowInfo = { title: null, mouseX: 0, mouseY: 0, flags: -1, frames: 0 };
  function window_title(ptr, len) {
    const text = readStr(ptr, len);
    if (text === null) return;
    windowInfo.title = text;
    post({ type: 'title', text });
  }
  function frame_info(x, y, flags) {
    windowInfo.mouseX = x;
    windowInfo.mouseY = y;
    windowInfo.frames++;
    if (flags !== windowInfo.flags) {
      windowInfo.flags = flags;
      post({ type: 'cursor', visible: (flags & 1) !== 0, menu: (flags & 2) !== 0, splash: (flags & 4) !== 0 });
    }
  }

  // ---------------------------------------------------------------- input
  const input = new InputQueue(4096);
  function input_next(outPtr, outCap) {
    outPtr >>>= 0;
    outCap >>>= 0;
    const cap = outCap - (outCap % REC);
    if (cap === 0) return input.length > 0 ? -ERRNO.EINVAL : 0;
    if (!inBounds(outPtr, cap)) return -ERRNO.EFAULT;
    return input.drain(new Uint8Array(buf(), outPtr, cap), performance.now());
  }
  /** From the page: records stamped with epoch ms (performance.timeOrigin + now on the main thread). */
  const inputSeen = {}; // records received, by kind (diagnostics)
  const inputLast = [];
  function pushInput(arrayBuffer) {
    const u8 = rebaseTimes(new Uint8Array(arrayBuffer), -performance.timeOrigin);
    for (let o = 0; o + REC <= u8.length; o += REC) {
      inputSeen[u8[o]] = (inputSeen[u8[o]] ?? 0) + 1;
      if (u8[o] !== 3) {
        inputLast.push([...u8.subarray(o, o + 8)].join(','));
        if (inputLast.length > 12) inputLast.shift();
      }
    }
    input.pushAll(u8);
    return u8;
  }

  // ---------------------------------------------------------------- audio
  // Before the page has an AudioContext (first click / key press) frames are accepted and dropped at the real-time
  // rate: a virtual buffer drains at 44,100 frames per second, so the mixer's clock keeps running.
  const audio = { mode: 'off', ring: null, port: null, pushed: 0, consumed: 0, virt: 0, virtAt: performance.now() };
  function attachAudio({ port, sab }) {
    audio.port = port;
    if (sab) {
      audio.ring = ringViews(sab);
      audio.mode = 'sab';
    } else {
      audio.mode = 'port';
      port.onmessage = (e) => {
        if (e.data && e.data.type === 'consumed') audio.consumed = e.data.frames;
      };
    }
  }
  function drainVirtual() {
    const now = performance.now();
    audio.virt = Math.max(0, audio.virt - ((now - audio.virtAt) * SAMPLE_RATE) / 1000);
    audio.virtAt = now;
  }
  function buffered() {
    if (audio.mode === 'sab') return ringAvailable(audio.ring);
    if (audio.mode === 'port') return Math.max(0, audio.pushed - audio.consumed);
    drainVirtual();
    return Math.ceil(audio.virt);
  }
  function audio_free() {
    if (audio.mode === 'off') return Math.max(0, TARGET_FRAMES - buffered());
    const room = audio.mode === 'sab' ? ringFree(audio.ring) : RING_FRAMES - 1 - buffered();
    return Math.max(0, Math.min(room, TARGET_FRAMES - buffered()));
  }
  function audio_push(ptr, frames) {
    ptr >>>= 0;
    frames >>>= 0;
    if (ptr % 4 !== 0) return -ERRNO.EINVAL;
    if (!inBounds(ptr, frames * CHANNELS * 4)) return -ERRNO.EFAULT;
    if (frames === 0) return 0;
    if (audio.mode === 'off') {
      const n = Math.min(frames, Math.max(0, RING_FRAMES - 1 - buffered()));
      audio.virt += n;
      return n;
    }
    const src = new Float32Array(buf(), ptr, frames * CHANNELS);
    if (audio.mode === 'sab') return ringWrite(audio.ring, src, frames);
    const n = Math.min(frames, Math.max(0, RING_FRAMES - 1 - buffered()));
    if (n === 0) return 0;
    const chunk = src.slice(0, n * CHANNELS);
    audio.port.postMessage({ type: 'chunk', data: chunk }, [chunk.buffer]);
    audio.pushed += n;
    return n;
  }

  // ---------------------------------------------------------------- graphics
  let gfx = null; // { frame(Uint8Array) }
  const gfxStats = { frames: 0, lastBytes: 0, maxBytes: 0, totalBytes: 0, lastChunks: 0, maxChunks: 0 };
  // the parts of the frame being received (FRAME_FORMAT.md v2): copies, outside wasm memory
  let pending = [];
  let pendingBytes = 0;
  function gfx_frame_chunk(ptr, len) {
    ptr >>>= 0;
    len >>>= 0;
    if (!inBounds(ptr, len)) return -ERRNO.EFAULT;
    if (len) {
      pending.push(new Uint8Array(buf(), ptr, len).slice());
      pendingBytes += len;
    }
    return 0;
  }
  /** tw.gfx_texture(id, width, height, ptr, len): a texture created and filled now, outside any frame (v2). */
  function gfx_texture(id, width, height, ptr, len) {
    id >>>= 0;
    width >>>= 0;
    height >>>= 0;
    ptr >>>= 0;
    len >>>= 0;
    if (len !== width * height * 4) return -ERRNO.EINVAL;
    if (!inBounds(ptr, len)) return -ERRNO.EFAULT;
    if (!gfx) return -ERRNO.ENOSYS;
    const view = new Uint8Array(buf(), ptr, len);
    // writeTexture copies at once; a view of shared memory is copied first
    gfx.preload(id, width, height, typeof SharedArrayBuffer !== 'undefined' && view.buffer instanceof SharedArrayBuffer ? view.slice() : view);
    gfxStats.preloaded = (gfxStats.preloaded ?? 0) + 1;
    gfxStats.preloadedBytes = (gfxStats.preloadedBytes ?? 0) + len;
    return 0;
  }

  function gfx_frame(ptr, len) {
    ptr >>>= 0;
    len >>>= 0;
    if (!inBounds(ptr, len)) {
      pending = [];
      pendingBytes = 0;
      return -ERRNO.EFAULT;
    }
    const chunks = pending.length;
    const total = pendingBytes + len;
    gfxStats.frames++;
    gfxStats.lastBytes = total;
    gfxStats.maxBytes = Math.max(gfxStats.maxBytes, total);
    gfxStats.totalBytes += total;
    gfxStats.lastChunks = chunks;
    gfxStats.maxChunks = Math.max(gfxStats.maxChunks, chunks);
    let parts;
    if (chunks === 0) {
      parts = new Uint8Array(buf(), ptr, len); // replayed during the call: no copy needed
    } else {
      parts = pending;
      if (len) parts.push(new Uint8Array(buf(), ptr, len));
    }
    pending = [];
    pendingBytes = 0;
    if (!gfx) return -ERRNO.ENOSYS;
    gfx.frame(parts);
    return 0;
  }

  // ---------------------------------------------------------------- frames
  let yieldResolve = null;
  let onYield = null; // the frame driver's statistics (worker.js: main-stack slices)
  function yield_frame() {
    onYield?.();
    return new Promise((res) => {
      yieldResolve = res;
    });
  }
  /** Called by the frame driver on each tick. True when a suspended yield_frame was resumed. */
  function resumeYield(now) {
    if (!yieldResolve) return false;
    const r = yieldResolve;
    yieldResolve = null;
    r(now);
    return true;
  }

  const tw = {
    fs_open, fs_size, fs_read, fs_write, fs_close, fs_stat, fs_list, fs_mkdir, fs_remove, fs_rename,
    now_ms, wall_ms, random, log, exit, cpu_count,
    clipboard_get, clipboard_set,
    input_next,
    audio_push, audio_free,
    gfx_frame, gfx_frame_chunk, gfx_texture,
    yield_frame,
    window_title, frame_info,
  };

  return {
    imports: { tw },
    setMemory(m) { memory = m; },
    setGfx(g) { gfx = g; },
    setClipboardText(t) { clipboard = String(t ?? ''); },
    pushInput,
    attachAudio,
    resumeYield,
    setOnYield(f) { onYield = f; },
    logText,
    exit,
    get exitCode() { return state.exitCode; },
    get audioMode() { return audio.mode; },
    gfxStats,
    windowInfo,
    _debug: { fds, input, inputSeen, inputLast },
  };
}

/** crypto.getRandomValues in 64 KiB pieces through a private buffer (it rejects views of a SharedArrayBuffer). */
export function fillRandom(view) {
  const tmp = new Uint8Array(Math.min(view.length, 65536));
  for (let o = 0; o < view.length; o += 65536) {
    const n = Math.min(65536, view.length - o);
    const t = tmp.subarray(0, n);
    crypto.getRandomValues(t);
    view.set(t, o);
  }
}

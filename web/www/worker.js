// The game Worker (module worker). Owns the OffscreenCanvas and its WebGPU device, the OPFS sync access handles,
// the input queue and the audio producer side. Loads game.wasm (wasm32-wasip1 reactor: imports
// `wasi_snapshot_preview1` + `tw`) when it exists, else runs the JS mock module below. Contract: HOST_ABI.md.

import { createVfs } from './vfs.js';
import { createRemoteInstall } from './lib/remote-install.js';
import { createHost, SUSPENDING } from './host.js';
import { createWasi, WasiExit, WASI_SUSPENDING, PREVIEW1 } from './wasi.js';
import { requestDevice, createGfx, fitRect } from './gfx.js';
import { parsePath } from './lib/paths.js';
import { REC, decode, KIND, codeName, encodeResize } from './lib/input-codec.js';
import { SAMPLE_RATE } from './lib/audio-ring.js';
import { FrameBuilder, BLEND_STATES, SAMPLERS, regs } from './lib/frame-codec.js';
import { createFibers, FIBER_SUSPENDING } from './lib/fibers.js';
import { createNet, normalizeBridge, isLoopback, serverPort, NET_ERRNO } from './lib/net-bridge.js';

const post = (msg, transfer) => self.postMessage(msg, transfer ?? []);
Error.stackTraceLimit = 100; // traps deep in the game: keep the whole wasm stack
const say = (text, level = 2) => post({ type: 'log', level, text });

let vfs = null;
let host = null;
let pendingAudio = null; // an 'audio' message that came before the host existed
const earlyInput = []; // 'input' messages that came before the host and the replayer existed
let lastResize = null; // { w, h } of the newest RESIZE record (the canvas size the page measured)

function attachAudio(m) {
  host.attachAudio({ port: m.port, sab: m.sab ?? null });
  say(`audio: started (${m.sab ? 'SharedArrayBuffer ring' : 'postMessage chunks'})`);
}
let wasi = null;
let gfx = null;
let mod = null; // { kind, memory, tw_init, tw_frame }
let canvas = null;
let inFrame = false;
let running = false;
let initDone = false;
let framesDone = 0;
let traceDepth = 0;
// ?fixedstep (scripted runs): the time handed to tw_frame / yield_frame advances exactly 1/60 s per frame, whatever
// the wall clock does (pauses between frames, slow frames), like the native shell's fixed step
let fixedStep = false;
let virtualNow = 0; // ?trace: keep the last N import calls (0: off)
const trace = { last: [], open: new Set(), seq: 0, failed: new Set() };
const dev = (data) => post({ type: 'dev', data });
// automation: stop starting frames after frame `pause.at` until resumed (window.__twDev.request('run-to' / 'resume'))
const pause = { at: null, paused: false, waiter: null };
// frame timings: every tw_frame's duration; frames over 1 s are listed. work*: the frames in which a work fiber ran
// (world generation, load, save: lib/fibers.js); over100: every frame over 100 ms (first 400); workSlow: the work frames over 100 ms, with the part spent on the fiber
// slowImports (?trace): import calls of 40 ms or more (frame, name, ms).
// slices: the module's stretches without giving the page a tick (a frame is cut into several when the main stack
// gives the frame back, tw.yield_frame); sliceMax / slicesOver100 (first 400; frame 0 = tw_init), mainYields.
const timing = { slow: [], max: 0, maxFrame: 0, total: 0, workFrames: 0, workMax: 0, workMaxFrame: 0, workTotal: 0, workSlow: [], over100: [], sliceMax: 0, sliceMaxFrame: 0, slicesOver100: [], mainYields: 0, slowImports: [] };
let sliceStart = null;
function sliceEnd(frame) {
  if (sliceStart === null) return;
  const ms = performance.now() - sliceStart;
  sliceStart = null;
  if (ms > timing.sliceMax) {
    timing.sliceMax = ms;
    timing.sliceMaxFrame = frame;
  }
  if (ms > 100 && timing.slicesOver100.length < 400) timing.slicesOver100.push({ frame, ms: Math.round(ms) });
}
let fibers = null; // lib/fibers.js, for game.wasm
let net = null; // lib/net-bridge.js: the game's TcpClient connections through the WebSocket bridge
let bridgeUrl = null; // ws://host:port of tools/ws-bridge (the page's ?bridge= or its header field)
let compiled = null; // the compiled game.wasm (handed to the page's server Worker: no second compile)
// Host & Play: the dedicated server this page runs in a Worker of its own (server-worker.js), started by the game
// (tw.server_start). Connections of the game to 127.0.0.1:<its port> go to it through the page.
let serverSeq = 0;
let localServer = null; // { id, port, state: 'starting' | 'listening' | 'exited', code }
let installMode = 'opfs'; // 'http': /install from the page's server (lib/remote-install.js), no import step
let remote = null; // lib/remote-install.js
let firstFramePosted = false;
let m_recordOrder = false; // ?recordorder: the order of first opens is reported at "steady" (web/host/pack-order.txt)
// "steady": the first time 30 frames in a row each took under 50 ms after tw_init (the title is up and responsive)
const steady = { run: 0, at: null, frame: null };
const titleAt = { splash: null, frame: null };
const snapAt = new Map(); // frame number -> { resolve, reject } (window.__twDev.request('snapshot-at'))
let logDumps = 0; // ?trace: how many "last import calls" dumps were written for error-looking log lines
const hasRaf = typeof self.requestAnimationFrame === 'function';

self.onmessage = (e) => {
  const m = e.data;
  switch (m.type) {
    case 'start':
      start(m).catch(fail);
      break;
    case 'input':
      // input before the host and the replayer exist (the page's first RESIZE while game.wasm still compiles) is
      // kept and applied once they do: dropping it left the canvas at its first, possibly 1x1, size
      if (host && gfx) onInput(m.buf);
      else earlyInput.push(m.buf);
      break;
    case 'tick':
      if (!hasRaf) tick(performance.now());
      break;
    case 'audio':
      // the AudioContext may be ready before the host exists (a click during loading, ?devaudio): attached then
      if (host) attachAudio(m);
      else pendingAudio = m;
      break;
    case 'paste':
      host?.setClipboardText(m.text);
      break;
    case 'bridge':
      bridgeUrl = normalizeBridge(m.url);
      say(`multiplayer: WebSocket bridge ${bridgeUrl ?? '(none)'}`);
      break;
    case 'server-listening':
      if (localServer && localServer.id === m.id && localServer.state === 'starting') localServer.state = 'listening';
      break;
    case 'server-exit':
      if (localServer && localServer.id === m.id) {
        localServer.state = 'exited';
        localServer.code = m.code;
      }
      // the server saved the world on its way out: its last announcements (vfs.js) may have been lost with its
      // Worker, so the whole /saves tree is read again
      vfs?.refreshSaves().then(() => say(`files: /saves read again after the server ended (shared index: ${JSON.stringify(vfs.stats)})`));
      break;
    case 'dev-request':
      devRequest(m).then(
        ({ reply, transfer }) => post({ type: 'dev-reply', id: m.id, ...reply }, transfer),
        (err) => post({ type: 'dev-reply', id: m.id, error: String(err.message ?? err) }),
      );
      break;
    case 'saves':
      saves(m).then((r) => post({ type: 'saves-reply', id: m.id, ...r }, r.bytes ? [r.bytes.buffer] : []), (err) => post({ type: 'saves-reply', id: m.id, error: String(err.message ?? err) }));
      break;
  }
};

/** A WasiExit (proc_exit) is a normal exit; anything else is an error shown to the player. */
function fail(err) {
  if (err instanceof WasiExit) {
    wasi?.flush();
    host?.exit(err.code);
    running = false;
    return;
  }
  running = false;
  let text = err && err.stack ? String(err.stack) : String(err);
  if (traceDepth) text += '\n\n' + traceReport();
  console.error(err);
  post({ type: 'error', text });
}

// ---------------------------------------------------------------- import-call trace (?trace[=N])
function traced(module, fns) {
  const out = {};
  for (const [name, fn] of Object.entries(fns)) {
    out[name] = (...args) => {
      // ?trace: when the game logs something that looks like an error, write the import calls just before it
      if (module === 'tw' && name === 'log' && logDumps < 8 && mod?.memory) {
        let text = '';
        try { text = new TextDecoder().decode(new Uint8Array(mod.memory.buffer, args[1] >>> 0, args[2] >>> 0).slice()); } catch { /* ignore */ }
        if (/os error|Exception|Unable to/i.test(text)) {
          logDumps++;
          const prev = trace.last.slice(-10).map((r) => `  #${r.n} ${r.call} -> ${r.result === undefined ? '(no return)' : r.result}`);
          host?.logText(3, `trace: the last ${prev.length} import calls before the log line "${text.slice(0, 120)}":\n${prev.join('\n')}`);
        }
      }
      const rec = { n: ++trace.seq, call: `${module}.${name}(${args.map((a) => (typeof a === 'bigint' ? a + 'n' : a)).join(', ')})`, result: undefined };
      trace.last.push(rec);
      if (trace.last.length > traceDepth) trace.last.shift();
      trace.open.add(rec);
      const t0 = performance.now();
      const done = (r) => {
        rec.result = typeof r === 'bigint' ? r + 'n' : r;
        trace.open.delete(rec);
        // an import call of 40 ms or more (its own time, or its wait when it suspended): timing.slowImports
        const ms = performance.now() - t0;
        if (ms >= 40 && timing.slowImports.length < 200) timing.slowImports.push({ frame: framesDone + 1, call: `${module}.${name}`, ms: Math.round(ms) });
        // a WASI call that fails: logged once per (call, errno), with the path when it has one
        if (module === 'wasi' && typeof r === 'number' && r !== 0) {
          const key = `${name}:${r}`;
          if (!trace.failed.has(key)) {
            trace.failed.add(key);
            let detail = '';
            if (/^path_/.test(name) && mod?.memory) {
              const [, , p, l] = name === 'path_open' || name === 'path_filestat_get' ? args : [0, 0, args[1], args[2]];
              try { detail = ` path "${new TextDecoder().decode(new Uint8Array(mod.memory.buffer, p >>> 0, l >>> 0).slice())}"`; } catch { /* ignore */ }
            }
            host?.logText(3, `wasi ${rec.call} -> errno ${r}${detail}`);
          }
        }
        return r;
      };
      try {
        const r = fn(...args);
        if (r && typeof r.then === 'function') {
          rec.result = '(suspended)';
          return r.then(done, (e) => {
            rec.result = 'threw ' + e;
            throw e;
          });
        }
        return done(r);
      } catch (e) {
        rec.result = 'threw ' + (e && e.name ? e.name + ': ' + e.message : e);
        trace.open.delete(rec);
        throw e;
      }
    };
  }
  return out;
}
function traceReport() {
  const open = [...trace.open].map((r) => `  #${r.n} ${r.call}`);
  const last = trace.last.map((r) => `  #${r.n} ${r.call} -> ${r.result === undefined ? '(no return)' : r.result}`);
  return `import calls still in progress (${open.length}):\n${open.join('\n') || '  none'}\nlast ${last.length} import calls:\n${last.join('\n')}`;
}

// ---------------------------------------------------------------- automation requests (window.__twDev.request)
async function devRequest(m) {
  if (m.op === 'stats') {
    return {
      reply: {
        frames: framesDone, initDone, running, exitCode: host?.exitCode ?? null, kind: mod?.kind ?? null,
        fibers: fibers ? { live: fibers.live(), ...fibers.stats } : null,
        net: net ? { live: net.live, ...net.stats } : null,
        server: localServer ? { ...localServer } : null,
        install: remote ? { mode: 'http', complete: remote.complete, ...remote.stats } : { mode: 'opfs' },
        window: host?.windowInfo ? { ...host.windowInfo } : null, presented: gfx ? { ...gfx.presented } : null, gfxLive: gfx?.liveTextures?.() ?? null, jsHeapMB: performance.memory ? Math.round(performance.memory.usedJSHeapSize / 1048576) : null, steady: { ...steady },
        memoryBytes: mod?.memory?.buffer.byteLength ?? 0, timing, inputSeen: host?._debug.inputSeen, inputLast: host?._debug.inputLast, gfx: host?.gfxStats ?? null, gfxReplay: gfx?.stats.last ?? null, maxTextureId: gfx?.stats.maxTextureId ?? 0, texturesCreated: gfx?.stats.texturesCreated ?? 0, trace: traceDepth ? traceReport() : null,
      },
    };
  }
  if (m.op === 'run-to') {
    // run until frame m.frame has completed, then hold; answers with the frame number
    if (m.frame <= framesDone) {
      pause.paused = true;
      return { reply: { frames: framesDone } };
    }
    pause.at = m.frame;
    pause.paused = false;
    const f = await new Promise((resolve) => (pause.waiter = resolve));
    return { reply: { frames: f } };
  }
  if (m.op === 'resume') {
    pause.paused = false;
    return { reply: { frames: framesDone } };
  }
  if (m.op === 'sync') {
    // answered after every message the page sent before it (input records included): they are queued now
    return { reply: { frames: framesDone, queued: host?._debug.input.length ?? 0 } };
  }
  if (m.op === 'diag-arm') {
    gfx.captureNext();
    return { reply: { frames: framesDone } };
  }
  if (m.op === 'diag-result') {
    const cap = gfx.captured();
    if (!cap) throw new Error('nothing captured (no frame with commands since diag-arm)');
    // the contents of every texture the frame drew into, after the frame (targets have COPY_SRC)
    const images = [];
    const transfer = [];
    for (const id of cap.order) {
      try {
        const s = await gfx.snapshot(id);
        images.push({ id, width: s.width, height: s.height, data: s.data });
        transfer.push(s.data.buffer);
      } catch (e) {
        images.push({ id, error: e.message });
      }
    }
    return { reply: { capture: cap, images }, transfer };
  }
  if (m.op === 'snapshot-at') {
    // the back buffer exactly after tw_frame number m.frame has finished
    if (m.frame <= framesDone) throw new Error(`frame ${m.frame} has already run (now ${framesDone})`);
    const snap = await new Promise((resolve, reject) => snapAt.set(m.frame, { resolve, reject }));
    return { reply: { width: snap.width, height: snap.height, data: snap.data, frames: m.frame }, transfer: [snap.data.buffer] };
  }
  if (m.op === 'snapshot') {
    if (!gfx) throw new Error('no graphics yet');
    const snap = await gfx.snapshot(m.texture ?? 0); // texture 0: the back buffer
    return { reply: { width: snap.width, height: snap.height, data: snap.data, frames: framesDone }, transfer: [snap.data.buffer] };
  }
  throw new Error(`unknown dev op ${m.op}`);
}

function onInput(buffer) {
  // Before the module sees the records: a resize sets the canvas backing size (applied at the next presentation,
  // so the canvas is never shown cleared); pointer positions (canvas device pixels) become back-buffer pixels
  // through the last presentation (gfx.js presentRect: the game's resolution can differ from the canvas, e.g. its
  // 800x600 minimum, and the picture is then fitted with its aspect ratio kept), so the game's mouse is exactly
  // where the pointer is.
  const u8 = new Uint8Array(buffer);
  const dv = new DataView(buffer);
  let pending = null;
  for (let o = 0; o + REC <= u8.length; o += REC) {
    const kind = u8[o];
    if (kind === KIND.RESIZE) {
      const r = decode(u8, o);
      if (r.pixelWidth > 0 && r.pixelHeight > 0) {
        gfx?.setCanvasSize(r.pixelWidth, r.pixelHeight);
        pending = { w: r.pixelWidth, h: r.pixelHeight };
        lastResize = pending;
      }
    } else if ((kind === KIND.MOUSE_MOVE || kind === KIND.MOUSE_BUTTON || kind === KIND.WHEEL) && gfx) {
      const x = dv.getFloat32(o + 16, true);
      const y = dv.getFloat32(o + 20, true);
      const p = mapPointer(x, y, pending);
      dv.setFloat32(o + 16, p.x, true);
      dv.setFloat32(o + 20, p.y, true);
    }
  }
  host.pushInput(buffer);
}

/** Canvas pixels -> the game's back-buffer pixels. `pending`: a canvas size not presented yet (a resize just now). */
function mapPointer(x, y, pending) {
  const p = gfx.presented;
  if (!p.backW) return { x, y };
  const cw = pending ? pending.w : p.canvasW;
  const ch = pending ? pending.h : p.canvasH;
  // after a resize the game takes the new window size at its next frame (unless it keeps a size of its own)
  const bw = pending && p.backW === p.canvasW && p.backH === p.canvasH ? cw : p.backW;
  const bh = pending && p.backW === p.canvasW && p.backH === p.canvasH ? ch : p.backH;
  const r = fitRect(bw, bh, cw, ch);
  return { x: ((x - r.x) * bw) / r.w, y: ((y - r.y) * bh) / r.h };
}

async function start(m) {
  canvas = m.canvas;
  traceDepth = m.trace | 0;
  fixedStep = !!m.fixedStep;
  bridgeUrl = normalizeBridge(m.bridge);
  installMode = m.install === 'http' ? 'http' : 'opfs';
  m_recordOrder = !!m.recordOrder;
  say(`multiplayer: Join via IP connects through the WebSocket bridge ${bridgeUrl ?? '(none set)'} (tools/ws-bridge)`);
  if (fixedStep) say('fixed-step clock: tw_frame time advances 1000/60 ms per frame (?fixedstep)');
  // the compile starts at once (streaming; from the HTTP cache on a second start), while WebGPU starts and the
  // files are indexed
  const wasmFetch = m.mock ? null : fetchGame();
  // WebGPU first: no point indexing 760 MB of files for a browser that cannot draw
  let device;
  try {
    ({ device } = await requestDevice());
  } catch (e) {
    post({ type: 'no-webgpu', text: e.message });
    return;
  }
  device.lost.then((info) => {
    if (info.reason !== 'destroyed') post({ type: 'error', text: `The graphics device was lost: ${info.message}` });
  });

  const root = await navigator.storage.getDirectory();
  const savesDir = m.saves && /^[A-Za-z0-9_-]{1,64}$/.test(m.saves) ? 'saves-' + m.saves : 'saves';
  if (installMode === 'http') {
    remote = createRemoteInstall({
      base: new URL('./', self.location.href).href,
      opfsRoot: root,
      role: 'game',
      log: (level, text) => say(text, level),
      onProgress: (p) => post({ type: 'loading', ...p }),
      recordOrder: !!m.recordOrder,
      // the download in a Worker of its own (install-worker.js): this Worker's event loop turns only between the
      // game's slices
      startHelper: (params) => {
        const w = new Worker(new URL('./install-worker.js', import.meta.url), { type: 'module', name: 'install' });
        w.postMessage(params);
        return w;
      },
    });
    const r = await remote.init();
    say(`files: /install from the server: ${r.files} files, ${(r.packSize / 1048576).toFixed(0)} MB packed, ${(r.cached / 1048576).toFixed(0)} MB of it cached in this browser (${r.ms.toFixed(0)} ms)`);
    dev({ event: 'install-ready', ...r, stats: { ...remote.stats } });
    remote.startDownload()?.then(() => dev({ event: 'install-cached', stats: { ...remote.stats } }));
  }
  vfs = createVfs(root, { savesDir, install: remote });
  if (savesDir !== 'saves') say(`/saves is the OPFS folder "${savesDir}" (?saves=)`);
  const idx = await vfs.init();
  say(`files: indexed ${idx.entries} entries under /install and /saves in ${idx.ms.toFixed(0)} ms`);
  host = createHost({ vfs, post });
  host.setOnYield(() => {
    timing.mainYields++;
    sliceEnd(initDone ? framesDone + 1 : 0);
  });
  if (pendingAudio) {
    attachAudio(pendingAudio);
    pendingAudio = null;
  }
  gfx = createGfx({ device, canvas, log: (level, text) => host.logText(level, text) });
  host.setGfx(gfx);
  for (const buf of earlyInput.splice(0)) onInput(buf);
  say(`graphics: WebGPU, canvas format ${gfx.canvasFormat}`);

  const tLoad = performance.now();
  mod = m.mock ? null : await loadGame(await wasmFetch); // ?mock: the JS mock even when game.wasm exists
  if (mod) dev({ event: 'loaded', ms: performance.now() - tLoad });
  if (!mod) {
    mod = createMock(host, canvas);
    post({ type: 'status', text: 'mock module (no game.wasm)' });
  } else {
    post({ type: 'status', text: 'game.wasm' });
  }
  host.setMemory(mod.memory);
  wasi?.setMemory(mod.memory);
  net?.setMemory(mod.memory);
  if (mod.initialize) {
    const t = performance.now();
    await mod.initialize(); // WASI reactor: static constructors
    dev({ event: 'initialize-done', ms: performance.now() - t });
  }

  // Ticks start before tw_init: the module may suspend in yield_frame during init (loading screens).
  running = true;
  post({ type: 'need-ticks', on: !hasRaf });
  if (hasRaf) self.requestAnimationFrame(rafLoop);
  dev({ event: 'init-start', width: canvas.width, height: canvas.height });
  // The game window opens 2 px lower than the canvas and is resized to it with the first frame's input, as the
  // native shell's window opens and is then made borderless / maximised: the game takes the real size through its
  // resize path (Main.SetDisplayMode with the window's new client size). Started at the canvas size directly, its
  // borderless start-up kept part of its state at another size (the menu missed clicks, the title's background
  // stopped short of the right edge). Only 2 px: the default UI scale is picked from the height at start-up.
  // the newest size the page measured (a RESIZE that came while loading), else the canvas's first size
  if (lastResize) {
    canvas.width = lastResize.w;
    canvas.height = lastResize.h;
  }
  const realW = canvas.width;
  const realH = canvas.height;
  const INIT_W = realW;
  const INIT_H = realH > 602 ? realH - 2 : realH + 2;
  if (mod.kind === 'wasm' && (realW !== INIT_W || realH !== INIT_H)) {
    host.pushInput(encodeResize({ pixelWidth: realW, pixelHeight: realH, cssWidth: realW, cssHeight: realH, dpr: 1, time: performance.timeOrigin + performance.now() }).buffer);
  }
  const tInit = performance.now();
  sliceStart = tInit;
  const rc = await mod.tw_init(mod.kind === 'wasm' ? INIT_W : realW, mod.kind === 'wasm' ? INIT_H : realH);
  sliceEnd(0);
  dev({ event: 'init-done', rc, ms: performance.now() - tInit });
  if (rc !== 0 && rc !== undefined) throw new Error(`tw_init returned ${rc}`);
  initDone = true;
  post({ type: 'status', text: mod.kind === 'mock' ? 'mock module running' : 'game running' });
}

function rafLoop() {
  if (!running) return;
  tick(performance.now());
  self.requestAnimationFrame(rafLoop);
}

/** One display frame: resume a module suspended in yield_frame, else start a new tw_frame if none is in flight. */
function tick(wallNow) {
  if (!running) return;
  if (!firstFramePosted && gfx && gfx.stats.frames > 0) {
    firstFramePosted = true; // the game drew: the page's loading bar goes
    post({ type: 'first-frame' });
    dev({ event: 'first-frame', frames: framesDone });
  }
  if (host.exitCode !== null) {
    running = false;
    return;
  }
  const now = fixedStep ? virtualNow : wallNow;
  // a yield of the main stack: for the game the frame goes on (a slow frame, as natively); under ?fixedstep the
  // game's clock does not move for it, so a scripted run does not depend on where the frame was cut
  if (host.resumeYield(now)) {
    sliceStart = performance.now();
    return;
  }
  if (!initDone) return; // tw_init still running: only yield_frame is resumed
  if (pause.paused) return; // automation holds the game between two frames
  if (inFrame) return; // still inside the previous frame (e.g. suspended on a file open): skip this tick
  inFrame = true;
  if (fixedStep) virtualNow += 1000 / 60;
  const tStart = performance.now();
  sliceStart = tStart;
  const resumes0 = fibers ? fibers.stats.resumes : 0;
  const fiberMs0 = fibers ? fibers.stats.ms : 0;
  Promise.resolve(mod.tw_frame(now)).then(
    () => {
      inFrame = false;
      framesDone++;
      sliceEnd(framesDone);
      const ms = performance.now() - tStart;
      timing.total += ms;
      if (ms > timing.max) {
        timing.max = ms;
        timing.maxFrame = framesDone;
      }
      if (fibers && fibers.stats.resumes !== resumes0) {
        timing.workFrames++;
        timing.workTotal += ms;
        if (ms > timing.workMax) {
          timing.workMax = ms;
          timing.workMaxFrame = framesDone;
        }
        if (ms > 100 && timing.workSlow.length < 200) {
          timing.workSlow.push({ frame: framesDone, ms: Math.round(ms), fiberMs: Math.round(fibers.stats.ms - fiberMs0) });
        }
      }
      // the title screen: the game's splash (and the content loading behind it) ended (tw.frame_info bit 2)
      const fl = host.windowInfo.flags;
      if (fl >= 0 && titleAt.splash === null && (fl & 4)) titleAt.splash = framesDone;
      if (fl >= 0 && titleAt.splash !== null && titleAt.frame === null && !(fl & 4)) {
        titleAt.frame = framesDone;
        dev({ event: 'title-reached', frame: framesDone, install: remote ? { ...remote.stats } : null });
        if (remote && m_recordOrder) dev({ event: 'open-order', paths: remote.order.slice() });
      }
      if (steady.at === null) {
        steady.run = ms < 50 ? steady.run + 1 : 0;
        if (steady.run >= 30) {
          steady.at = performance.now();
          steady.frame = framesDone;
          dev({ event: 'steady', frame: framesDone, install: remote ? { ...remote.stats } : null });
        }
      }
      if (ms > 100 && timing.over100.length < 400) timing.over100.push({ frame: framesDone, ms: Math.round(ms), fiberMs: fibers ? Math.round(fibers.stats.ms - fiberMs0) : 0 });
      if (ms > 1000) {
        timing.slow.push({ frame: framesDone, ms: Math.round(ms) });
        say(`frame ${framesDone} took ${(ms / 1000).toFixed(1)} s`, 3);
      }
      if (pause.at !== null && framesDone >= pause.at) {
        pause.paused = true;
        pause.at = null;
        const w = pause.waiter;
        pause.waiter = null;
        w?.(framesDone);
      }
      const want = snapAt.get(framesDone);
      if (want) {
        snapAt.delete(framesDone);
        gfx.snapshot(0).then(want.resolve, want.reject); // right after tw_frame #N (its gfx_frame is submitted)
      }
    },
    (err) => {
      inFrame = false;
      if (host.exitCode !== null && !(err instanceof WasiExit)) return; // exit() then a trap is the normal way out
      fail(err);
    },
  );
}

// ------------------------------------------------------------------------------------------------ game.wasm
/**
 * The compile of game.wasm, started as early as possible. "no-cache": revalidated with the server every time (a
 * rebuilt module is used at once) but taken from the HTTP cache when unchanged (a 304), which also lets V8 reuse its
 * cached machine code for the module. Resolves to { module, ms } or null (no game.wasm: the JS mock).
 */
function fetchGame() {
  const t0 = performance.now();
  return fetch('game.wasm', { cache: 'no-cache' }).then(
    async (resp) => {
      if (!resp.ok) return null;
      say(`game.wasm: ${resp.headers.get('content-length') ?? '?'} bytes${resp.headers.get('content-encoding') ? ` (${resp.headers.get('content-encoding')})` : ''}, compiling`);
      const module = await WebAssembly.compileStreaming(resp);
      return { module, ms: performance.now() - t0 };
    },
    () => null,
  );
}

async function loadGame(fetched) {
  if (!fetched) return null;
  const module = fetched.module;
  compiled = module;
  dev({ event: 'compiled', ms: fetched.ms });
  say(`game.wasm: fetched and compiled in ${(fetched.ms / 1000).toFixed(1)} s`);
  wasi = createWasi({ vfs, log: (level, text) => host.logText(level, text) });
  // work fibers (world generation / load on their own JSPI stacks): lib/fibers.js
  fibers = createFibers({ log: (level, text) => host.logText(level, text) });
  // Join via IP: the game's TcpClient connections through the WebSocket bridge (lib/net-bridge.js); Host & Play: the
  // page's own server Worker, and this client's connection to it through the page
  net = createNet({
    bridge: () => bridgeUrl,
    log: (level, text) => host.logText(level, text),
    local: (host, port) => (localServer && localServer.state !== 'exited' && port === localServer.port && isLoopback(host)
      ? (channel) => post({ type: 'local-connect', id: localServer.id, port, channel }, [channel])
      : null),
    servers: {
      start(args) {
        if (localServer && localServer.state !== 'exited') {
          // one server per page: the game starts a new one only after the last one ended (or was killed)
          post({ type: 'server-kill', id: localServer.id });
          localServer.state = 'exited';
        }
        const id = ++serverSeq;
        localServer = { id, port: serverPort(args), state: 'starting', code: null };
        say(`Host & Play: starting the dedicated server ${id} in a Worker of this page (port ${localServer.port})`);
        post({ type: 'server-start', id, args, module: compiled });
        return id;
      },
      status: (id) => (id >= 1 && id <= serverSeq ? (localServer && localServer.id === id && localServer.state !== 'exited' ? 0 : 1) : -NET_ERRNO.EBADF),
      kill(id) {
        if (!(id >= 1 && id <= serverSeq)) return -NET_ERRNO.EBADF;
        post({ type: 'server-kill', id });
        if (localServer && localServer.id === id) localServer.state = 'exited';
        return 0;
      },
    },
  });
  const tw = { ...host.imports.tw, ...fibers.imports, ...net.imports };
  const suspendingTw = new Set([...SUSPENDING, ...FIBER_SUSPENDING]);
  const w = wasi.imports.wasi_snapshot_preview1;
  const missing = WebAssembly.Module.imports(module).filter((i) =>
    !(i.module === 'tw' && i.name in tw) && !(i.module === 'wasi_snapshot_preview1' && PREVIEW1.includes(i.name)));
  if (missing.length) {
    throw new Error('game.wasm needs imports the host does not provide: ' +
      missing.map((i) => `${i.module}.${i.name} (${i.kind})`).join(', '));
  }
  const wrap = (fns, suspending) => Object.fromEntries(Object.entries(fns).map(([n, f]) => [n, suspending.has(n) ? new WebAssembly.Suspending(f) : f]));
  const twFns = traceDepth ? traced('tw', tw) : tw;
  const wFns = traceDepth ? traced('wasi', w) : w;
  if (traceDepth) say(`import-call trace on: the last ${traceDepth} calls are reported with any error`);
  const instance = await WebAssembly.instantiate(module, { tw: wrap(twFns, suspendingTw), wasi_snapshot_preview1: wrap(wFns, WASI_SUSPENDING) });
  const ex = instance.exports;
  if (fibers.attach(ex)) say('work fibers: on (JSPI; world generation and loading give the frame back)');
  else if (WebAssembly.Module.imports(module).some((i) => i.module === 'tw' && i.name.startsWith('fiber_'))) {
    throw new Error('game.wasm imports the fiber functions but exports no tw_fiber_entry / __stack_pointer');
  }
  for (const need of ['memory', 'tw_init', 'tw_frame']) {
    if (!(need in ex)) throw new Error(`game.wasm does not export ${need}`);
  }
  if (!(ex.memory instanceof WebAssembly.Memory)) throw new Error('game.wasm export "memory" is not a memory');
  if ('_start' in ex) say('game.wasm exports _start (a command, not a reactor); it is not called', 3);
  return {
    kind: 'wasm',
    memory: ex.memory,
    initialize: typeof ex._initialize === 'function' ? WebAssembly.promising(ex._initialize) : null,
    tw_init: WebAssembly.promising(ex.tw_init),
    tw_frame: WebAssembly.promising(ex.tw_frame),
  };
}

// ------------------------------------------------------------------------------------------------ saves (page control)
// Export / import of .plr / .wld under /saves/Players and /saves/Worlds. Nothing leaves the browser except as a
// download the player starts; uploads come from a file the player picks.
const SAVE_RE = /\.(plr|wld)$/i;
async function saves(m) {
  if (!vfs) throw new Error('the game is not running');
  if (m.op === 'list') {
    // m.all (automation): every file, not only .plr / .wld
    const out = [];
    const walk = async (node, parts) => {
      if (node.stale || node.refreshing) await vfs.refreshDir(node); // another Worker changed it (vfs.js)
      for (const e of vfs.list(node)) {
        const c = node.children.get(e.name);
        if (e.kind === 'dir') await walk(c, [...parts, e.name]);
        else if (m.all || SAVE_RE.test(e.name)) {
          const st = await vfs.fileStat(c);
          out.push({ path: '/' + [...parts, e.name].join('/'), name: e.name, size: typeof st === 'number' ? -1 : st.size, mtime: typeof st === 'number' ? 0 : st.mtime });
        }
      }
    };
    await walk(vfs.resolve(['saves']), ['saves']);
    return { files: out };
  }
  if (m.op === 'read') {
    const p = parsePath(m.path);
    // m.all (automation: test/run-game-headless.mjs "savefile"): any file under /saves, e.g. the server's banlist.txt
    if (p.err || p.root !== 'saves' || (!m.all && !SAVE_RE.test(m.path))) throw new Error('not a save file');
    const o = await vfs.openFile(p.parts, {});
    if (typeof o === 'number') throw new Error(`cannot open (errno ${-o})`);
    try {
      const bytes = new Uint8Array(o.s.access.getSize());
      o.s.access.read(bytes, { at: 0 });
      return { bytes, name: p.parts.at(-1) };
    } finally {
      vfs.release(o.node, o.s, false);
    }
  }
  if (m.op === 'write') {
    const name = String(m.name).split(/[\\/]/).pop();
    if (!SAVE_RE.test(name) || name.length > 200) throw new Error('only .plr and .wld files can be imported');
    const dir = ['saves', /\.plr$/i.test(name) ? 'Players' : 'Worlds'];
    const mk = await vfs.mkdir(dir, { recursive: true });
    if (mk < 0) throw new Error(`cannot create /${dir.join('/')} (errno ${-mk})`);
    const parts = [...dir, name];
    if (vfs.resolve(parts) && !m.overwrite) return { exists: true, path: '/' + parts.join('/') };
    const o = await vfs.openFile(parts, { write: true, create: true, trunc: true });
    if (typeof o === 'number') throw new Error(o === -10 ? 'the game has this file open; close it in the game first' : `cannot write (errno ${-o})`);
    try {
      o.s.access.write(new Uint8Array(m.bytes), { at: 0 });
    } finally {
      vfs.release(o.node, o.s, true);
    }
    return { written: '/' + parts.join('/') };
  }
  throw new Error(`unknown saves op ${m.op}`);
}

// ------------------------------------------------------------------------------------------------ mock module
// A JS stand-in with the same exports as game.wasm. It calls the `tw` imports exactly as the wasm module would:
// numbers and (ptr, len) ranges in its own WebAssembly.Memory; it draws by building FRAME_FORMAT.md frames and
// handing them to tw.gfx_frame. Imports that may suspend are awaited.

function createMock(host, canvas) {
  const tw = host.imports.tw;
  const memory = new WebAssembly.Memory({ initial: 32, maximum: 1024 }); // 2 MiB to start
  const enc = new TextEncoder();
  const dec = new TextDecoder();
  const u8 = () => new Uint8Array(memory.buffer);
  const STR = 0; // 0..4095: path / log strings
  const STR2 = 4096; // second path (rename)
  const IN = 8192; // input records, 64 records
  const IN_CAP = 64 * REC;
  const IO = 16384; // file I/O scratch, 16 KiB
  const LIST = 32768; // fs_list output, 32 KiB
  const LIST_CAP = 32768;
  const TONE = 65536; // 1 s of stereo f32 = 352,800 bytes
  const TONE_FRAMES = SAMPLE_RATE;
  const FRAME = 1 << 20; // frame bytes, up to 1 MiB

  const put = (s, at = STR) => {
    const b = enc.encode(s);
    u8().set(b, at);
    return b.length;
  };
  const logm = (text, level = 2) => tw.log(level, STR, put(text));
  const path = (p, at = STR) => [at, put(p, at)];
  const str = (ptr, len) => dec.decode(u8().slice(ptr, ptr + len));

  async function list(p) {
    let r = tw.fs_list(...path(p), LIST, 1024); // small on purpose: exercises the "needed size" answer
    let out = LIST;
    let retried = false;
    if (r <= -256) {
      retried = true;
      const need = -r;
      if (need > LIST_CAP) {
        out = memory.buffer.byteLength;
        memory.grow(Math.ceil(need / 65536));
      }
      r = tw.fs_list(...path(p), out, need);
    }
    if (r < 0) return { err: -r };
    return { names: r === 0 ? [] : str(out, r).split('\n'), retried };
  }

  async function fileDemo() {
    const top = await list('/install/Content');
    if (top.err) logm(`fs_list /install/Content failed: errno ${top.err}`, 3);
    else logm(`fs_list /install/Content: ${top.names.length} entries: ${top.names.slice(0, 20).join(', ')}${top.names.length > 20 ? ', ...' : ''}`);
    const imgs = await list('/install/Content/Images');
    if (!imgs.err) logm(`fs_list /install/Content/Images: ${imgs.names.length} entries${imgs.retried ? ' (needed-size retry used)' : ''}`);

    // case-insensitive lookup + read: XNB files start with "XNB" and a platform byte
    const probe = '/install/content/images/item_1.xnb';
    const h = await tw.fs_open(...path(probe), 0);
    if (h < 0) {
      logm(`fs_open ${probe}: errno ${-h}`, 3);
    } else {
      const size = tw.fs_size(h);
      const n = tw.fs_read(h, 0, IO, 4);
      const magic = str(IO, Math.max(0, n));
      logm(`fs_open ${probe} -> handle ${h}, size ${size}, first bytes "${magic}"${magic.startsWith('XNB') ? ' (XNB header ok)' : ''}`);
      tw.fs_close(h);
    }

    // the write guard
    const bad = await tw.fs_open(...path('/install/Content/mock-should-fail.txt'), 1);
    logm(`fs_open /install/... for write -> ${bad} (expected -2, EACCES)`, bad === -2 ? 2 : 4);

    // saves: mkdir, write, read back, rename, stat
    let r = await tw.fs_mkdir(...path('/saves/mock'));
    if (r !== 0) logm(`fs_mkdir /saves/mock -> ${r}`, 4);
    const text = `hello from the mock at ${new Date(tw.wall_ms()).toISOString()}`;
    const w = await tw.fs_open(...path('/saves/mock/hello.txt'), 1);
    if (w >= 0) {
      const len = put(text, IO);
      const wrote = tw.fs_write(w, 0, IO, len);
      tw.fs_close(w);
      const rd = await tw.fs_open(...path('/saves/mock/hello.txt'), 0);
      const size = tw.fs_size(rd);
      const got = tw.fs_read(rd, 0, IO + 4096, size);
      const back = str(IO + 4096, got);
      tw.fs_close(rd);
      r = await tw.fs_rename(...path('/saves/mock/hello.txt'), ...path('/saves/mock/hello-renamed.txt', STR2));
      const st = tw.fs_stat(...path('/saves/mock/hello-renamed.txt'));
      logm(`saves: wrote ${wrote} bytes, read back ${back === text ? 'equal' : 'DIFFERENT'}, rename -> ${r}, stat -> ${st}`,
        back === text && r === 0 && st === 0 ? 2 : 4);
    } else {
      logm(`fs_open /saves/mock/hello.txt for write: errno ${-w}`, 4);
    }
  }

  // ---------------------------------------------------------------- graphics: frames for tw.gfx_frame
  // Texture 1: 8 x 1 colour bars; 2: 256 x 1 grey ramp; 3: 1 x 1 white. Shader 1: a tiny effect (inverts its
  // texture, times a colour register) that exercises DefineShader and the effect bindings.
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
  let serial = 0;
  let defined = false;
  function drawFrame(now) {
    const W = canvas.width;
    const H = canvas.height;
    const f = new FrameBuilder(++serial, W, H);
    if (!defined) {
      defined = true;
      const bars = [[191, 191, 191], [191, 191, 0], [0, 191, 191], [0, 191, 0], [191, 0, 191], [191, 0, 0], [0, 0, 191], [0, 0, 0]];
      f.createTexture(1, 8, 1).upload(1, 0, 0, 8, 1, bars.flatMap((c) => [...c, 255]));
      f.createTexture(2, 256, 1).upload(2, 0, 0, 256, 1, Array.from({ length: 256 }, (_, i) => [i, i, i, 255]).flat());
      f.createTexture(3, 1, 1).upload(3, 0, 0, 1, 1, [255, 255, 255, 255]);
      f.defineShader({ id: 1, label: 'mock invert', wgsl: INVERT_WGSL, units: [0], psRegs: 1, vsRegs: -1 });
    }
    f.setTarget(0, false).clear(0, 0, 0, 255);
    const y1 = Math.round(H * 0.7);
    const q = (x0, yy0, x1, yy1, color, tex, blend = BLEND_STATES.Opaque, sampler = SAMPLERS.PointClamp) => {
      const r = f.quad(W, H, x0, yy0, x1, yy1, color);
      f.draw({ texture: tex, blend, sampler, ...r });
    };
    q(0, 0, W, y1, [255, 255, 255, 255], 1);
    q(0, y1, W, H, [255, 255, 255, 255], 2, BLEND_STATES.Opaque, SAMPLERS.LinearClamp);
    // the effect: an inverted copy of the bars in a box, through shader 1
    const bx = Math.round(W * 0.35);
    const by = Math.round(H * 0.25);
    const r = f.quad(W, H, bx, by, W - bx, H - by - Math.round(H * 0.2));
    f.draw({ texture: 1, shader: 1, ps: regs([1, 1, 1, 1]), units: [{ unit: 0, texture: 1, sampler: SAMPLERS.PointClamp }], blend: BLEND_STATES.Opaque, ...r });
    const sx = Math.round(((now / 4000) % 1) * W);
    q(sx - 2, 0, sx + 2, H, [255, 255, 255, 255], 3);
    const e = 4;
    for (const [x0, yy0, x1, yy1] of [[0, 0, W, e], [0, H - e, W, H], [0, 0, e, H], [W - e, 0, W, H]]) q(x0, yy0, x1, yy1, [255, 128, 0, 255], 3);
    // a half-transparent pointer square (AlphaBlend with premultiplied colour)
    if (pointer) q(pointer.x - 8, pointer.y - 8, pointer.x + 8, pointer.y + 8, [128, 128, 128, 128], 3, BLEND_STATES.AlphaBlend);
    const bytes = f.build();
    if (FRAME + bytes.length > memory.buffer.byteLength) memory.grow(Math.ceil((FRAME + bytes.length - memory.buffer.byteLength) / 65536));
    u8().set(bytes, FRAME);
    tw.gfx_frame(FRAME, bytes.length);
  }

  // ---------------------------------------------------------------- audio
  const tone = { playing: false, off: 0, ready: false };
  function makeTone() {
    const f = new Float32Array(memory.buffer, TONE, TONE_FRAMES * 2);
    const fade = Math.floor(SAMPLE_RATE * 0.01);
    for (let i = 0; i < TONE_FRAMES; i++) {
      const env = Math.min(1, i / fade, (TONE_FRAMES - 1 - i) / fade);
      const v = 0.2 * env * Math.sin((2 * Math.PI * 440 * i) / SAMPLE_RATE);
      f[i * 2] = v;
      f[i * 2 + 1] = v;
    }
    tone.ready = true;
  }

  let pointer = null;
  const lastPadLog = new Map();
  function echo(r) {
    switch (r.kind) {
      case KIND.MOUSE_MOVE:
        pointer = { x: r.x, y: r.y };
        return; // too many to log
      case KIND.KEY:
        return logm(`key ${r.down ? 'down' : 'up'} ${codeName(r.a)}${r.key ? ` '${String.fromCodePoint(r.key)}'` : ''}${r.repeat ? ' (repeat)' : ''} mods=${r.mods}`);
      case KIND.TEXT:
        return logm(`text "${r.text}"`);
      case KIND.MOUSE_BUTTON:
        pointer = { x: r.x, y: r.y };
        return logm(`mouse ${r.down ? 'down' : 'up'} button ${r.button} at ${r.x.toFixed(0)},${r.y.toFixed(0)}`);
      case KIND.WHEEL:
        return logm(`wheel dy=${r.dy.toFixed(1)} dx=${r.dx.toFixed(1)} at ${r.x.toFixed(0)},${r.y.toFixed(0)}`);
      case KIND.FOCUS:
        return logm(r.focused ? 'focus' : 'blur');
      case KIND.RESIZE:
        return logm(`resize ${r.pixelWidth}x${r.pixelHeight} px (css ${r.cssWidth.toFixed(0)}x${r.cssHeight.toFixed(0)} x ${r.dpr})`);
      case KIND.GAMEPAD_CONNECT:
        return logm(`gamepad ${r.index} ${r.connected ? 'connected' : 'disconnected'}${r.standard ? ' (standard mapping)' : ''}`);
      case KIND.GAMEPAD_BUTTONS:
        if (lastPadLog.get(r.index) !== r.pressed) {
          lastPadLog.set(r.index, r.pressed);
          logm(`gamepad ${r.index} buttons 0x${r.pressed.toString(16)} lt=${r.lt.toFixed(2)} rt=${r.rt.toFixed(2)}`);
        }
        return;
      case KIND.GAMEPAD_AXES:
        return;
      case KIND.OVERFLOW:
        return logm(`input overflow: ${r.dropped} records dropped`, 3);
      default:
        return logm(`input kind ${r.kind}`);
    }
  }

  return {
    kind: 'mock',
    memory,
    initialize: null,
    async tw_init(width, height) {
      logm(`mock tw_init(${width}, ${height}); cpu_count ${tw.cpu_count()}`);
      await fileDemo();
      // the yield_frame round trip: wait one display frame inside "init"
      const t0 = tw.now_ms();
      const t1 = await tw.yield_frame();
      logm(`yield_frame resumed after ${(t1 - t0).toFixed(1)} ms`);
      const rnd = tw.random(IO, 8);
      logm(`random -> ${rnd}, bytes ${[...u8().slice(IO, IO + 8)].map((b) => b.toString(16).padStart(2, '0')).join('')}`);
      makeTone();
      logm('mock ready: click the canvas for a 1 s 440 Hz tone; keys, text and wheel are echoed here; Ctrl+V pastes into the log, Ctrl+C copies the time');
      return 0;
    },
    async tw_frame(now) {
      for (;;) {
        const n = tw.input_next(IN, IN_CAP);
        if (n <= 0) break;
        const view = u8();
        for (let o = 0; o < n; o += REC) {
          const r = decode(view, IN + o);
          echo(r);
          if (r.kind === KIND.MOUSE_BUTTON && r.down && tone.ready && !tone.playing && host.audioMode !== 'off') {
            tone.playing = true;
            tone.off = 0;
          }
          if (r.kind === KIND.KEY && r.down && (r.mods & 2) && r.a === 22 /* KeyV */) {
            const len = tw.clipboard_get(IO, 4096);
            logm(len >= 0 ? `clipboard_get: "${str(IO, len)}"` : `clipboard_get: needs ${-len} bytes`);
          }
          if (r.kind === KIND.KEY && r.down && (r.mods & 2) && r.a === 3 /* KeyC */) {
            const t = `mock time ${tw.now_ms().toFixed(0)} ms`;
            tw.clipboard_set(IO, put(t, IO));
            logm(`clipboard_set: "${t}" (written on your next click or key press)`);
          }
        }
      }
      if (tone.playing) {
        const want = Math.min(tw.audio_free(), TONE_FRAMES - tone.off);
        if (want > 0) {
          const got = tw.audio_push(TONE + tone.off * 8, want);
          if (got > 0) tone.off += got;
        }
        if (tone.off >= TONE_FRAMES) tone.playing = false;
      }
      drawFrame(now);
    },
  };
}


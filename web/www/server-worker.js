// The dedicated server Worker (module worker) of "Host & Play": the page starts it when the game's Host & Play
// starts `TerrariaServer.exe` (terraria-host sys/misc.rs `Process.start` -> tw.server_start in the game Worker ->
// main.js). It runs its own instance of game.wasm as the dedicated server (port/crates/terraria-web/src/server.rs:
// tw_server_init, then tw_server_tick until the server ends), with the same files (/install, /saves in OPFS).
// Contract: HOST_ABI.md "Server".
//
// Connections to the server's listener (lib/net-bridge.js):
// - from the hosting client of this page: the page relays the far end of a MessageChannel ('incoming');
// - from outside (a native client, another browser page): through tools/ws-bridge in reverse mode
//   (`ws_bridge.py --listen <tcp port> --to-page`). When the game starts listening, this Worker registers with the
//   bridge (ws://.../?listen=<port>); for each TCP connection the bridge offers ({"conn", "peer"}) it opens
//   ws://.../?accept=<id> and hands that WebSocket to the listener.
// No graphics, input or sound: those imports exist (the module is the same) and are never called by a server.
//
// The server's console (HOST_ABI.md "Server console"): the page's console box. A typed line ({type:'console',
// line}) is queued as the server's stdin (WASI fd 0, EAGAIN while empty); the game's input thread
// (Main.startDedInputCallBack, resumable: parked until a line is there) reads it with Console.ReadLine and runs the
// command. Everything the server writes to stdout (fd 1) goes to the page as it comes ({type:'console-out', text}),
// besides the line log.

import { createVfs } from './vfs.js';
import { createRemoteInstall } from './lib/remote-install.js';
import { createHost, SUSPENDING } from './host.js';
import { createWasi, WasiExit, WASI_SUSPENDING, PREVIEW1 } from './wasi.js';
import { createFibers, FIBER_SUSPENDING } from './lib/fibers.js';
import { createNet, normalizeBridge } from './lib/net-bridge.js';

const post = (msg, transfer) => self.postMessage(msg, transfer ?? []);
Error.stackTraceLimit = 100;
const say = (text, level = 2) => post({ type: 'log', level, text: `[server] ${text}` });

let net = null;
let vfsRef = null; // the server's file index (vfs.js), for its statistics
let bridgeUrl = null;
let serverId = 0;
let ended = false;
const controls = new Map(); // port -> { ws, retry, warned }
// the console's stdin: bytes typed into the page's box, not yet read by the game
const stdinQueue = { bytes: new Uint8Array(0), lines: 0 };
const stdin = {
  read(max) {
    if (stdinQueue.bytes.length === 0) return null; // EAGAIN: the game's ReadLine parks until a line comes
    const n = Math.min(max, stdinQueue.bytes.length);
    const out = stdinQueue.bytes.slice(0, n);
    stdinQueue.bytes = stdinQueue.bytes.slice(n);
    return out;
  },
};
function consoleLine(line) {
  const add = new TextEncoder().encode(String(line).replace(/[\r\n]+/g, ' ') + '\n');
  const all = new Uint8Array(stdinQueue.bytes.length + add.length);
  all.set(stdinQueue.bytes);
  all.set(add, stdinQueue.bytes.length);
  stdinQueue.bytes = all;
  stdinQueue.lines++;
}
const early = []; // 'incoming' connections that came before the module listened (refused by net then)
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const stats = { ticks: 0, waits: 0, waitedMs: 0, yields: 0, maxTickMs: 0 };
// stretches of the module without giving the page a turn (a poll, or a piece of one between two yield_frame calls):
// the longest, those over 100 ms (first 50), and the same while a player is connected
const slices = { start: null, max: 0, over100: [], connectedMax: 0, connectedOver100: 0, count: 0 };
const t00 = performance.now();
function sliceEnd() {
  if (slices.start === null) return;
  const now = performance.now();
  const ms = now - slices.start;
  slices.start = null;
  slices.count++;
  const connected = !!net && net.stats.accepted > 0 && net.live > 0;
  if (ms > slices.max) slices.max = ms;
  if (ms > 100 && slices.over100.length < 50) slices.over100.push(`${((now - t00) / 1000).toFixed(1)}s:${Math.round(ms)}${connected ? '(connected)' : ''}`);
  if (connected) {
    if (ms > slices.connectedMax) slices.connectedMax = ms;
    if (ms > 100) slices.connectedOver100++;
  }
}
const sliceReport = () => `${slices.count} stretches without a page turn, longest ${slices.max.toFixed(0)} ms, over 100 ms: ${slices.over100.length ? slices.over100.join(' ') : 'none'}; while a player was connected: longest ${slices.connectedMax.toFixed(0)} ms, ${slices.connectedOver100} over 100 ms`;
let lastSliceReport = 0;

self.onmessage = (e) => {
  const m = e.data;
  switch (m.type) {
    case 'start':
      start(m).catch((err) => finish(1, err));
      break;
    case 'incoming':
      if (net) net.acceptPort(m.port, m.channel, m.peer ?? '');
      else early.push(m);
      break;
    case 'stop':
      finish(m.code ?? 1, null, 'stopped by the page');
      break;
    case 'console':
      if (!ended) consoleLine(m.line ?? '');
      break;
  }
};

function finish(code, err, why) {
  if (ended) return;
  ended = true;
  if (err && !(err instanceof WasiExit)) {
    console.error(err);
    say(`stopped with an error: ${err && err.stack ? err.stack : err}`, 4);
  } else if (err instanceof WasiExit) {
    code = err.code;
  }
  for (const c of controls.values()) {
    clearTimeout(c.retry);
    try {
      c.ws?.close(1000);
    } catch {}
  }
  controls.clear();
  if (vfsRef) say(`files: shared /saves index: ${JSON.stringify(vfsRef.stats)}`);
  say(`server ${serverId} ended (exit code ${code})${why ? `: ${why}` : ''}; ${stats.ticks} ticks (polls), longest poll ${stats.maxTickMs.toFixed(0)} ms (with its page turns); ${sliceReport()}`);
  post({ type: 'exit', id: serverId, code });
}

// ---------------------------------------------------------------- the bridge's reverse mode
function register(port) {
  if (!bridgeUrl || ended) return;
  const c = controls.get(port) ?? { ws: null, retry: 0, warned: false };
  controls.set(port, c);
  let ws;
  try {
    ws = new WebSocket(`${bridgeUrl}/?listen=${port}`);
  } catch (e) {
    say(`bridge ${bridgeUrl}: ${e.message ?? e}`, 3);
    return;
  }
  c.ws = ws;
  ws.onopen = () => {
    c.warned = false;
    say(`others can join through the bridge ${bridgeUrl} (reverse mode: its TCP port leads here)`);
  };
  ws.onmessage = (ev) => {
    let offer;
    try {
      offer = JSON.parse(ev.data);
    } catch {
      return;
    }
    const data = new WebSocket(`${bridgeUrl}/?accept=${offer.conn}`);
    data.binaryType = 'arraybuffer';
    data.onopen = () => net.acceptWebSocket(port, data, offer.peer ?? '');
    data.onclose = (ev2) => say(`the bridge's connection ${offer.conn} from ${offer.peer} could not be taken (close ${ev2.code})`, 3);
  };
  ws.onerror = () => {};
  ws.onclose = () => {
    if (c.ws !== ws || ended || !controls.has(port)) return;
    c.ws = null;
    if (!c.warned) {
      c.warned = true;
      say(`no bridge in reverse mode at ${bridgeUrl}: only this page can join (start tools/ws-bridge/ws_bridge.py --listen <tcp port> --to-page); retrying every 5 s`, 2);
    }
    c.retry = setTimeout(() => register(port), 5000);
  };
}
function unregister(port) {
  const c = controls.get(port);
  if (!c) return;
  controls.delete(port);
  clearTimeout(c.retry);
  try {
    c.ws?.close(1000);
  } catch {}
}

// ---------------------------------------------------------------- start and run
async function start(m) {
  serverId = m.id;
  bridgeUrl = normalizeBridge(m.bridge);
  const t0 = performance.now();
  const root = await navigator.storage.getDirectory();
  const savesDir = m.saves && /^[A-Za-z0-9_-]{1,64}$/.test(m.saves) ? 'saves-' + m.saves : 'saves';
  let remote = null;
  if (m.install === 'http') {
    // /install over HTTP, as the game Worker: the pack the game Worker downloaded, shared (lib/remote-install.js)
    remote = createRemoteInstall({ base: new URL('./', self.location.href).href, opfsRoot: root, role: 'server', log: (level, text) => say(text, level) });
    await remote.init();
  }
  const vfs = createVfs(root, { savesDir, install: remote });
  await vfs.init();
  vfsRef = vfs;
  const host = createHost({
    vfs,
    post: (msg) => {
      if (msg.type === 'log') post({ type: 'log', level: msg.level, text: msg.text.startsWith('[server]') ? msg.text : `[server] ${msg.text}` });
      else if (msg.type === 'exit') finish(msg.code);
    },
  });
  // the line log of stdout leaves out the input thread's ": " prompt (Console.Write without a line end, so it would
  // stand in front of the next line: ": Hoster has joined."); the console box shows it as it is
  const wasi = createWasi({ vfs, log: (level, text) => host.logText(level, level === 2 ? text.replace(/^(?:: )+/, '') : text), args: ['terraria-server', String(m.args ?? '')], stdin, onStd: (fd, text) => {
    if (fd === 1) post({ type: 'console-out', id: serverId, text });
  } });
  const fibers = createFibers({ log: (level, text) => host.logText(level, text) });
  net = createNet({
    bridge: () => bridgeUrl,
    log: (level, text) => host.logText(level, text),
    listeners: {
      onListen: (port) => {
        post({ type: 'listening', id: serverId, port });
        for (const x of early.splice(0)) net.acceptPort(x.port, x.channel, x.peer ?? '');
        register(port);
      },
      onUnlisten: (port) => {
        post({ type: 'unlisten', id: serverId, port });
        unregister(port);
      },
    },
  });
  let module = m.module instanceof WebAssembly.Module ? m.module : null;
  if (!module) {
    const resp = await fetch('game.wasm', { cache: 'no-cache' });
    if (!resp.ok) throw new Error(`game.wasm: HTTP ${resp.status}`);
    module = await WebAssembly.compileStreaming(resp);
  }
  // the server's main stack gives the page its turn (a long poll: the world load) and goes on at once
  const yield_frame = () => {
    stats.yields++;
    sliceEnd();
    return new Promise((res) => setTimeout(() => {
      slices.start = performance.now();
      res(performance.now());
    }, 0));
  };
  const tw = { ...host.imports.tw, ...fibers.imports, ...net.imports, yield_frame };
  const suspendingTw = new Set([...SUSPENDING, ...FIBER_SUSPENDING]);
  const w = wasi.imports.wasi_snapshot_preview1;
  const missing = WebAssembly.Module.imports(module).filter((i) => !(i.module === 'tw' && i.name in tw) && !(i.module === 'wasi_snapshot_preview1' && PREVIEW1.includes(i.name)));
  if (missing.length) throw new Error('game.wasm needs imports the server Worker does not provide: ' + missing.map((i) => `${i.module}.${i.name}`).join(', '));
  const wrap = (fns, suspending) => Object.fromEntries(Object.entries(fns).map(([n, f]) => [n, suspending.has(n) ? new WebAssembly.Suspending(f) : f]));
  const instance = await WebAssembly.instantiate(module, { tw: wrap(tw, suspendingTw), wasi_snapshot_preview1: wrap(w, WASI_SUSPENDING) });
  const ex = instance.exports;
  if (!fibers.attach(ex)) throw new Error('game.wasm has no work fibers (tw_fiber_entry / __stack_pointer)');
  for (const need of ['memory', 'tw_server_init', 'tw_server_tick']) if (!(need in ex)) throw new Error(`game.wasm does not export ${need} (rebuild it: web/ci/build_game_wasm.sh)`);
  host.setMemory(ex.memory);
  wasi.setMemory(ex.memory);
  net.setMemory(ex.memory);
  if (typeof ex._initialize === 'function') await WebAssembly.promising(ex._initialize)();
  const init = WebAssembly.promising(ex.tw_server_init);
  const tick = WebAssembly.promising(ex.tw_server_tick);
  say(`server ${serverId}: game.wasm instantiated in ${((performance.now() - t0) / 1000).toFixed(1)} s${m.module ? ' (module from the game Worker)' : ''}; launch parameters: ${m.args}`);
  slices.start = performance.now();
  const rc = await init();
  sliceEnd();
  if (rc !== 0) return finish(2, null, `tw_server_init returned ${rc}`);
  // the main loop: one poll of Main.DedServ per call, then its Thread.Sleep(ms) here
  for (;;) {
    if (ended) return;
    const t = performance.now();
    slices.start = t;
    const r = await tick(t);
    sliceEnd();
    const ms = performance.now() - t;
    if (t - lastSliceReport > 30000) {
      lastSliceReport = t;
      say(`worker: ${sliceReport()}`);
    }
    stats.ticks++;
    if (ms > stats.maxTickMs) stats.maxTickMs = ms;
    if (r < 0) {
      wasi.flush();
      return finish(Math.round(-1 - r));
    }
    stats.waits++;
    stats.waitedMs += r;
    await sleep(Math.max(0, r));
  }
}

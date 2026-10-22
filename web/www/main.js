// The page (main thread): feature checks, the first-run import of the player's own Content folder into OPFS,
// the game Worker, input forwarding, the AudioWorklet and fullscreen. Contract with the Worker: HOST_ABI.md.
//
// Two pages use it:
// - the play page (play.html, served at / by web/host/terraria_host.py; <html data-page="play">): only the game, full
//   window. /install comes from the server over HTTP (no import), the document title is the game window's title,
//   nothing of the page's own is shown except a text-free loading bar until the game's first frame (and a message
//   when the browser cannot run it at all). Logging goes to window.__twLog only (no DOM).
// - the developer page (index.html; /dev.html on the host server): toolbar, log, server console, saves, the OPFS import.

import { locateContent, validateInstall, planCopy, hasRoom, fmtBytes, isCopyAllowed, findGameExe, isMzHeader, GAME_EXE } from './lib/import-plan.js';
import {
  encodeKey, encodeText, encodeMouseMove, encodeMouseButton, encodeWheel, encodeFocus, encodeResize,
  encodeGamepadConnect, encodeGamepadAxes, encodeGamepadButtons, modsOf, concat,
} from './lib/input-codec.js';
import { createRing, RING_FRAMES, SAMPLE_RATE } from './lib/audio-ring.js';
import { DEFAULT_BRIDGE, normalizeBridge } from './lib/net-bridge.js';

const PLAY = document.documentElement.dataset.page === 'play';
// The play page has few of the developer page's elements: a missing one is a detached stand-in, so the shared code
// below needs no checks (and nothing of it is shown).
const standIns = new Map();
const $ = (id) => document.getElementById(id) ?? standIns.get(id) ?? standIns.set(id, document.createElement(id === 'bridge' || id === 'sc-input' ? 'input' : 'div')).get(id);
const MARKER = 'tw-import.json'; // OPFS root; written when an import completes
const params = new URLSearchParams(location.search);

// ------------------------------------------------------------------------------------------------ log panel
const logEl = $('log');
const LOG_MAX = 300;
// Everything the log panel ever showed, for automated runs (test/run-game-headless.mjs reads it). Capped.
const devLog = [];
window.__twLog = devLog;
function addLog(text, level = 2) {
  if (devLog.length < 500000) devLog.push({ t: performance.now(), level, text });
  if (PLAY) {
    if (level >= 4) console.error(text);
    return;
  }
  const line = document.createElement('div');
  line.className = 'l' + level;
  line.textContent = `${new Date().toLocaleTimeString()}  ${text}`;
  logEl.appendChild(line);
  while (logEl.childElementCount > LOG_MAX) logEl.firstElementChild.remove();
  logEl.scrollTop = logEl.scrollHeight;
}
const setStatus = (t) => ($('status').textContent = t);

// ------------------------------------------------------------------------------------------------ multiplayer bridge
// Join via IP goes through tools/ws-bridge on this computer (lib/net-bridge.js). Where it listens: ?bridge=, else what
// the player typed last time (localStorage), else the bridge's default.
const BRIDGE_KEY = 'tw-bridge';
function storedBridge() {
  try {
    return localStorage.getItem(BRIDGE_KEY);
  } catch {
    return null;
  }
}
let bridge = normalizeBridge(params.get('bridge')) ?? normalizeBridge(storedBridge()) ?? DEFAULT_BRIDGE;
$('bridge').value = bridge;
$('bridge').addEventListener('change', () => {
  const v = normalizeBridge($('bridge').value);
  $('bridge').classList.toggle('bad', !v);
  if (!v) return;
  bridge = v;
  $('bridge').value = v;
  try {
    localStorage.setItem(BRIDGE_KEY, v);
  } catch {}
  worker?.postMessage({ type: 'bridge', url: v });
});
$('bridge').addEventListener('keydown', (e) => {
  if (e.key === 'Enter') {
    $('bridge').dispatchEvent(new Event('change'));
    canvas?.focus();
  }
});
$('btn-log').onclick = () => document.body.classList.toggle('log-hidden');

function show(id) {
  for (const s of document.querySelectorAll('.screen')) s.hidden = s.id !== id;
}

// ------------------------------------------------------------------------------------------------ feature checks
function missingFeatures() {
  const miss = [];
  if (typeof WebAssembly !== 'object' || typeof WebAssembly.Suspending !== 'function' || typeof WebAssembly.promising !== 'function') {
    miss.push('WebAssembly JavaScript Promise Integration (WebAssembly.Suspending / WebAssembly.promising)');
  }
  if (!navigator.gpu) miss.push('WebGPU');
  if (!navigator.storage || typeof navigator.storage.getDirectory !== 'function') miss.push('Origin Private File System');
  if (typeof HTMLCanvasElement.prototype.transferControlToOffscreen !== 'function') miss.push('OffscreenCanvas');
  if (typeof Worker !== 'function') miss.push('Web Workers');
  return miss;
}

// ------------------------------------------------------------------------------------------------ OPFS helpers
async function opfs() {
  return navigator.storage.getDirectory();
}
async function readMarker() {
  try {
    const f = await (await (await opfs()).getFileHandle(MARKER)).getFile();
    return JSON.parse(await f.text());
  } catch {
    return null;
  }
}
async function writeMarker(obj) {
  const h = await (await opfs()).getFileHandle(MARKER, { create: true });
  const w = await h.createWritable();
  await w.write(JSON.stringify(obj));
  await w.close();
}
async function listEntries(dir) {
  const out = [];
  for await (const [name, h] of dir.entries()) out.push({ name, kind: h.kind, handle: h });
  return out;
}
/** Every file under `dir` (paths relative to /install) with its size; files isCopyAllowed refuses are not opened. */
async function walkFiles(dir, prefix = '', out = []) {
  for await (const [name, h] of dir.entries()) {
    if (h.kind === 'directory') await walkFiles(h, prefix + name + '/', out);
    else if (isCopyAllowed(prefix + name) && !name.endsWith('.crswap')) {
      const f = await h.getFile();
      out.push({ path: prefix + name, size: f.size, handle: h });
    }
  }
  return out;
}

// ------------------------------------------------------------------------------------------------ import
let importAbort = false;

async function pickFolder() {
  if (typeof window.showDirectoryPicker !== 'function') {
    throw new Error('This browser has no folder picker (showDirectoryPicker). Use Chrome or Edge 137+.');
  }
  return window.showDirectoryPicker({ id: 'terraria-install', mode: 'read', startIn: 'desktop' });
}

function showProblems(list) {
  const ul = $('import-problems');
  ul.replaceChildren(...list.map((t) => Object.assign(document.createElement('li'), { textContent: t })));
  ul.hidden = list.length === 0;
}

/**
 * Validate the picked folder; returns { contentDir, exe } (exe = the Terraria.exe file handle) or null (problems
 * shown). Only Terraria.exe is opened at the top level, and only to check its first two bytes.
 */
async function checkPicked(dir) {
  const rootEntries = await listEntries(dir);
  const exeName = findGameExe(rootEntries);
  let exe = null;
  let mz = null;
  if (exeName) {
    exe = await dir.getFileHandle(exeName);
    mz = isMzHeader(new Uint8Array(await (await exe.getFile()).slice(0, 2).arrayBuffer()));
  }
  const where = locateContent(rootEntries);
  let contentDir = null;
  let contentEntries = null;
  if (where === '.') {
    contentDir = dir;
    contentEntries = rootEntries;
  } else if (where) {
    contentDir = await dir.getDirectoryHandle(where);
    contentEntries = await listEntries(contentDir);
  }
  const v = validateInstall(where, contentEntries, { found: exe !== null, mz });
  showProblems(v.problems);
  return v.ok ? { contentDir, exe } : null;
}

async function ensureDir(root, parts, cache) {
  let d = root;
  let key = '';
  for (const p of parts) {
    key += '/' + p;
    let next = cache.get(key);
    if (!next) {
      next = await d.getDirectoryHandle(p, { create: true });
      cache.set(key, next);
    }
    d = next;
  }
  return d;
}

/** The picked folder as the import source: [{ path (relative to /install), size, handle: { getFile() } }]. */
async function pickedSource({ contentDir, exe }) {
  const src = await walkFiles(contentDir, 'Content/');
  src.push({ path: GAME_EXE, size: (await exe.getFile()).size, handle: exe });
  return src;
}

/**
 * Copy the picked Content folder to OPFS /install/Content and Terraria.exe to /install/Terraria.exe.
 * Resumable: files already there with the same size are skipped.
 */
async function importContent(picked, opts = {}) {
  return importFiles(() => pickedSource(picked), opts);
}

/**
 * The import itself, whatever the source: `source()` gives [{ path, size, handle }] where handle.getFile() resolves
 * to something with `.stream()`. Paths that isCopyAllowed refuses are never copied (planCopy drops them).
 */
async function importFiles(source, { selftest = false, dev = false } = {}) {
  importAbort = false;
  $('import-progress').hidden = false;
  $('btn-pick').disabled = true;
  $('btn-stop').hidden = false;
  const bar = $('progress-bar');
  const text = $('progress-text');
  text.textContent = 'Reading the folder…';

  const root = await opfs();
  await writeMarker({ complete: false, started: new Date().toISOString(), selftest, dev });
  const dest = await root.getDirectoryHandle('install', { create: true });

  const src = await source();
  const have = new Map((await walkFiles(dest)).map((f) => [f.path, f.size]));
  const plan = planCopy(src, have);
  addLog(`import: ${plan.totalFiles} files, ${fmtBytes(plan.totalBytes)}; already copied ${plan.skip.length} (${fmtBytes(plan.skipBytes)}); to copy ${plan.copy.length} (${fmtBytes(plan.copyBytes)})`);

  const est = navigator.storage.estimate ? await navigator.storage.estimate() : null;
  if (!hasRoom(est, plan.copyBytes)) {
    throw new Error(`Not enough browser storage: ${fmtBytes(plan.copyBytes)} needed, ${fmtBytes(est.quota - est.usage)} free. Free some disk space and try again.`);
  }
  if (navigator.storage.persist) {
    const persisted = await navigator.storage.persist().catch(() => false);
    addLog(`storage persistence ${persisted ? 'granted' : 'not granted (the browser may clear the files under storage pressure)'}`);
  }

  let doneBytes = plan.skipBytes;
  let doneFiles = plan.skip.length;
  let raf = 0;
  const paint = () => {
    raf = 0;
    bar.max = Math.max(1, plan.totalBytes);
    bar.value = doneBytes;
    text.textContent = `${fmtBytes(doneBytes)} of ${fmtBytes(plan.totalBytes)} · ${doneFiles} of ${plan.totalFiles} files`;
  };
  const schedule = () => { if (!raf) raf = requestAnimationFrame(paint); };
  paint();

  const dirCache = new Map();
  for (const f of plan.copy) {
    if (importAbort) throw new Error('Stopped. Choose the folder again to continue where it left off.');
    const parts = f.path.split('/');
    const name = parts.pop();
    const dir = await ensureDir(dest, parts, dirCache);
    const file = await f.handle.getFile();
    const fh = await dir.getFileHandle(name, { create: true });
    // createWritable writes to a swap file and replaces the target only on close, so an interrupted copy never
    // leaves a file of the right name with the wrong contents (the size check alone would then be enough).
    const w = await fh.createWritable();
    const counter = new TransformStream({
      transform(chunk, ctl) {
        doneBytes += chunk.byteLength;
        schedule();
        ctl.enqueue(chunk);
      },
    });
    await file.stream().pipeThrough(counter).pipeTo(w);
    doneFiles++;
    schedule();
  }
  paint();
  await writeMarker({ complete: true, exe: true, files: plan.totalFiles, bytes: plan.totalBytes, finished: new Date().toISOString(), selftest, dev });
  addLog(`import complete: ${plan.totalFiles} files, ${fmtBytes(plan.totalBytes)}`);
  return plan;
}

function resetImportUi() {
  $('btn-pick').disabled = false;
  $('btn-stop').hidden = true;
}

$('btn-stop').onclick = () => {
  importAbort = true;
};

$('btn-pick').onclick = async () => {
  let dir;
  try {
    dir = await pickFolder();
  } catch (e) {
    if (e.name !== 'AbortError') showProblems([e.message]);
    return;
  }
  try {
    const picked = await checkPicked(dir);
    if (!picked) return;
    await importContent(picked);
    resetImportUi();
    startGame();
  } catch (e) {
    resetImportUi();
    showProblems([e.message]);
    addLog(`import: ${e.message}`, 4);
  }
};

async function showImport(fromGame) {
  stopGame();
  showProblems([]);
  $('import-progress').hidden = true;
  $('btn-cancel-reimport').hidden = !fromGame;
  setStatus('Game files needed');
  show('screen-import');
}

$('btn-reimport').onclick = () => showImport(true);
$('btn-cancel-reimport').onclick = () => startGame();
$('btn-restart').onclick = () => startGame();

// Developer tool (?devimport): the install comes from `serve.py --install <dir>` instead of the folder picker, for
// automated runs. Same validation and the same copy code after the source. The server refuses unless started with
// --install, so a normal deployment never offers it.
async function devImport() {
  const resp = await fetch('__devinstall/list.json', { cache: 'no-store' });
  if (!resp.ok) throw new Error(`?devimport: the server has no dev install (start serve.py with --install <Terraria folder>): HTTP ${resp.status}`);
  const list = await resp.json();
  const fileUrl = (path) => '__devinstall/file/' + path.split('/').map(encodeURIComponent).join('/');
  // validation, as for a picked folder
  const rootEntries = [];
  const contentEntries = new Map();
  for (const f of list) {
    const parts = f.path.split('/');
    if (parts.length === 1) rootEntries.push({ name: parts[0], kind: 'file' });
    else if (parts[0] === 'Content') {
      if (!rootEntries.some((e) => e.name === 'Content')) rootEntries.push({ name: 'Content', kind: 'directory' });
      contentEntries.set(parts[1], { name: parts[1], kind: parts.length > 2 ? 'directory' : 'file' });
    }
  }
  const exeEntry = list.find((f) => f.path === GAME_EXE);
  let mz = null;
  if (exeEntry) {
    const r = await fetch(fileUrl(GAME_EXE), { cache: 'no-store' });
    const reader = r.body.getReader();
    const { value } = await reader.read();
    reader.cancel().catch(() => {});
    mz = isMzHeader(value ?? new Uint8Array(0));
  }
  const where = locateContent(rootEntries);
  const v = validateInstall(where, where ? [...contentEntries.values()] : null, { found: !!exeEntry, mz });
  showProblems(v.problems);
  if (!v.ok) throw new Error('?devimport: ' + v.problems.join(' '));
  show('screen-import');
  const t0 = performance.now();
  const plan = await importFiles(async () => list.map((f) => ({
    path: f.path,
    size: f.size,
    handle: { getFile: async () => ({ size: f.size, stream: () => streamOf(fileUrl(f.path)) }) },
  })), { dev: true });
  const ms = performance.now() - t0;
  devEvent({ event: 'import-done', ms, files: plan.totalFiles, bytes: plan.totalBytes, copied: plan.copy.length, skipped: plan.skip.length });
  addLog(`devimport: ${plan.copy.length} copied, ${plan.skip.length} already there, ${(ms / 1000).toFixed(1)} s`);
  resetImportUi();
}

/** A ReadableStream of a URL's body (fetched when the copy starts reading). */
function streamOf(url) {
  let reader = null;
  return new ReadableStream({
    async pull(ctl) {
      if (!reader) {
        const r = await fetch(url, { cache: 'no-store' });
        if (!r.ok) throw new Error(`${url}: HTTP ${r.status}`);
        reader = r.body.getReader();
      }
      const { value, done } = await reader.read();
      if (done) ctl.close();
      else ctl.enqueue(value);
    },
    cancel() {
      reader?.cancel();
    },
  });
}

// ------------------------------------------------------------------------------------------------ automation hooks
// window.__twDev: events from the page and the Worker (import done, tw_init start / end, errors), and requests to
// the Worker (frame stats, a snapshot of the back buffer). Used by test/run-game-headless.mjs.
const devEvents = [];
const devWaiting = new Map();
let devSeq = 0;
function devEvent(e) {
  devEvents.push({ t: performance.now(), ...e });
}
window.__twDev = {
  events: devEvents,
  request(op, extra = {}) {
    return new Promise((resolve, reject) => {
      if (!worker) return reject(new Error('no game worker'));
      const id = ++devSeq;
      devWaiting.set(id, (m) => (m.error ? reject(new Error(m.error)) : resolve(m)));
      worker.postMessage({ type: 'dev-request', id, op, ...extra });
    });
  },
  // automation: a save file put into /saves (test/run-game-headless.mjs action "import")
  saves: (msg, transfer = []) => savesCall(msg, transfer),
};

// Development self-test (?selftest): builds a tiny fake install inside OPFS (never the real one) and imports it
// twice, the second time checking that every file is skipped. Refuses to run over a real import.
async function selftest() {
  const marker = await readMarker();
  if (marker && marker.complete && !marker.selftest) {
    addLog('selftest refused: this browser already holds a real import', 3);
    return false;
  }
  const root = await opfs();
  const src = await root.getDirectoryHandle('selftest-src', { create: true });
  const content = await src.getDirectoryHandle('Content', { create: true });
  const put = async (dir, name, bytes) => {
    const w = await (await dir.getFileHandle(name, { create: true })).createWritable();
    await w.write(bytes);
    await w.close();
  };
  const images = await content.getDirectoryHandle('Images', { create: true });
  const fonts = await content.getDirectoryHandle('Fonts', { create: true });
  await put(images, 'Item_1.xnb', new TextEncoder().encode('XNBw\x05\x00fake item texture'));
  await put(images, 'Item_2.xnb', new Uint8Array(200000).fill(7));
  await put(fonts, 'Mouse_Text.xnb', new TextEncoder().encode('XNBw fake font'));
  await put(content, 'Wave Bank.xwb', new Uint8Array(1024));
  await put(content, 'Sound Bank.xsb', new Uint8Array(64));
  await put(src, 'Terraria.exe', new TextEncoder().encode('MZ\x90\x00 fake game executable')); // copied
  await put(src, 'Other.dll', new Uint8Array(16)); // never copied
  await put(content, 'Stray.exe', new Uint8Array(16)); // never copied, even inside Content
  const picked = await checkPicked(src);
  if (!picked) {
    addLog('selftest: validation FAILED', 4);
    return false;
  }
  show('screen-import');
  const first = await importContent(picked, { selftest: true });
  const second = await importContent(picked, { selftest: true });
  const install = await root.getDirectoryHandle('install');
  const exeCopied = await install.getFileHandle('Terraria.exe').then(() => true, () => false);
  const ok = second.copy.length === 0 && second.skip.length === first.totalFiles && first.totalFiles === 6 && exeCopied &&
    first.copy.every((f) => f.path === 'Terraria.exe' || f.path.startsWith('Content/'));
  addLog(`selftest import: first run copied ${first.copy.length}, second run copied ${second.copy.length} and skipped ${second.skip.length}: ${ok ? 'PASS' : 'FAIL'}`, ok ? 2 : 4);
  resetImportUi();
  return ok;
}

// ------------------------------------------------------------------------------------------------ the game
let worker = null;
let canvas = null;
let needTicks = false;
let pixelW = 0;
let pixelH = 0;
let rafId = 0;
const listeners = [];

function on(target, type, fn, opts) {
  target.addEventListener(type, fn, opts);
  listeners.push(() => target.removeEventListener(type, fn, opts));
}

const epoch = () => performance.timeOrigin + performance.now();
let cursorSeen = false;
let focusSent = null; // the last FOCUS record sent (null: none yet)
let sawSplash = false; // the game reported Main.showSplash (tw.frame_info): its end is the title screen
// where /install comes from: the page's server over HTTP (the play page; ?httpinstall on the developer page) or the
// OPFS copy the import made
const installMode = PLAY || params.has('httpinstall') ? 'http' : 'opfs';
function send(records) {
  if (!worker || records.length === 0) return;
  const buf = records.buffer.byteLength === records.byteLength ? records.buffer : records.slice().buffer;
  worker.postMessage({ type: 'input', buf }, [buf]);
}

// ------------------------------------------------------------------------------------------------ Host & Play
// The game's Host & Play starts its dedicated server (the game Worker asks: 'server-start'): a second Worker with
// its own instance of game.wasm (server-worker.js). The hosting client's connection to it (127.0.0.1:<its port>) is
// a MessageChannel the game Worker opens and this page hands over ('local-connect' -> 'incoming'), held until the
// server listens. Other players reach it through tools/ws-bridge in reverse mode (the server Worker registers).
const servers = new Map(); // id -> { worker, port, state: 'starting' | 'listening' | 'exited', waiting: MessagePort[] }

// Server console: while a server of this page runs, the box under the game is its console. A line typed there goes
// to the server Worker ('console'), which makes it the server's stdin (HOST_ABI.md "Server console"); the game's
// own input thread (Main.startDedInputCallBack) reads and runs it. What the server writes to stdout comes back
// ('console-out') and is shown as it comes, like the original's console window.
const SC_MAX = 200000; // characters kept in the output area
let consoleServer = null; // the id of the server the box talks to
function consoleOut(text, cls) {
  const out = $('sc-out');
  // Console.Clear (terraria-host misc.rs: ANSI "erase display, cursor home"; DedServ before its banner, the `clear`
  // command) empties the area, as it clears the original's console window
  const clear = text.lastIndexOf('\x1b[2J');
  if (clear >= 0) {
    out.textContent = '';
    text = text.slice(clear + 4);
  }
  text = text.replace(/\x1b\[H/g, '');
  if (!text) return;
  const atEnd = out.scrollTop + out.clientHeight >= out.scrollHeight - 4;
  if (cls) {
    const span = document.createElement('span');
    span.className = cls;
    span.textContent = text;
    out.appendChild(span);
  } else {
    const last = out.lastChild;
    if (last && last.nodeType === Node.TEXT_NODE) last.appendData(text);
    else out.appendChild(document.createTextNode(text));
  }
  while (out.textContent.length > SC_MAX && out.firstChild) out.firstChild.remove();
  if (atEnd) out.scrollTop = out.scrollHeight;
}
function consoleState(text, ended = false) {
  $('sc-state').textContent = text;
  $('sc-state').classList.toggle('ended', ended);
}
function consoleOpen(id) {
  consoleServer = id;
  $('sc-out').textContent = '';
  consoleState('starting…');
  $('sc-input').disabled = true;
  $('sc-send').disabled = true;
  $('server-console').hidden = false; // beside the log, same height: the canvas keeps its size
}
function consoleReady(id, port) {
  if (consoleServer !== id) return;
  consoleState(`running, port ${port}`);
  $('sc-input').disabled = false;
  $('sc-send').disabled = false;
}
function consoleEnded(id, text) {
  if (consoleServer !== id) return;
  consoleState(text, true);
  consoleOut(`\n[${text}]\n`, 'note');
  $('sc-input').disabled = true;
  $('sc-send').disabled = true;
  if (document.activeElement === $('sc-input')) $('ime').focus({ preventScroll: true });
}
$('sc-form').addEventListener('submit', (e) => {
  e.preventDefault();
  const s = servers.get(consoleServer);
  const line = $('sc-input').value;
  if (!s || s.state === 'exited') return;
  s.worker.postMessage({ type: 'console', line });
  consoleOut(line + '\n', 'echo'); // the console echoes what was typed, as a terminal does
  devEvent({ event: 'server-console-in', id: consoleServer, line });
  $('sc-input').value = '';
});
$('sc-input').addEventListener('keydown', (e) => {
  if (e.key === 'Escape') $('ime').focus({ preventScroll: true }); // back to the game
});

function startServer(m) {
  for (const [id, s] of servers) if (s.state !== 'exited') stopServer(id, 'replaced by a new server');
  const sw = new Worker(new URL('./server-worker.js', import.meta.url), { type: 'module', name: 'server' });
  const s = { worker: sw, port: null, state: 'starting', waiting: [] };
  servers.set(m.id, s);
  addLog(`Host & Play: dedicated server ${m.id} starting in a Worker of this page`);
  devEvent({ event: 'server-start', id: m.id });
  consoleOpen(m.id);
  sw.onmessage = (e) => {
    const x = e.data;
    switch (x.type) {
      case 'log': addLog(x.text, x.level); break;
      case 'console-out': if (consoleServer === m.id) consoleOut(x.text); break;
      case 'listening':
        s.state = 'listening';
        s.port = x.port;
        devEvent({ event: 'server-listening', id: m.id, port: x.port });
        consoleReady(m.id, x.port);
        for (const p of s.waiting.splice(0)) sw.postMessage({ type: 'incoming', port: x.port, channel: p.channel, peer: '' }, [p.channel]);
        worker?.postMessage({ type: 'server-listening', id: m.id, port: x.port });
        break;
      case 'exit': stopServer(m.id, null, x.code); break;
    }
  };
  sw.onerror = (e) => {
    addLog(`[server] the server Worker failed: ${e.message || e}`, 4);
    stopServer(m.id, 'the Worker failed', 1);
  };
  sw.postMessage({ type: 'start', id: m.id, args: m.args, module: m.module ?? null, saves: params.get('saves'), bridge, install: installMode });
}

function stopServer(id, why, code = null) {
  const s = servers.get(id);
  if (!s || s.state === 'exited') return;
  s.state = 'exited';
  for (const p of s.waiting.splice(0)) {
    try {
      p.channel.postMessage({ t: 'refused', why: 'the server ended' });
      p.channel.close();
    } catch {}
  }
  // a server that ended by itself had just saved the world: its Worker lives on a moment, so that its last file
  // announcements (vfs.js BroadcastChannel) are delivered
  if (code !== null && code >= 0) setTimeout(() => s.worker.terminate(), 2000);
  else s.worker.terminate();
  addLog(`Host & Play: dedicated server ${id} ended${code !== null ? ` (exit code ${code})` : ''}${why ? `: ${why}` : ''}`);
  consoleEnded(id, `server stopped${code !== null ? ` (exit code ${code})` : ''}${why ? `: ${why}` : ''}`);
  devEvent({ event: 'server-exit', id, code });
  worker?.postMessage({ type: 'server-exit', id, code: code ?? 1 });
}

function localConnect(m) {
  const s = servers.get(m.id);
  if (!s || s.state === 'exited') {
    m.channel.postMessage({ t: 'refused', why: 'no server runs in this page' });
    m.channel.close();
  } else if (s.state === 'listening') {
    s.worker.postMessage({ type: 'incoming', port: m.port, channel: m.channel, peer: '' }, [m.channel]);
  } else {
    s.waiting.push({ channel: m.channel }); // the client's connect waits, as a TCP connect to a port about to listen
  }
}

function stopGame() {
  if (rafId) cancelAnimationFrame(rafId);
  rafId = 0;
  resizedHook = null;
  frameCount = 0;
  devicePixels = null;
  for (const off of listeners.splice(0)) off();
  for (const id of servers.keys()) stopServer(id, 'the game stopped');
  if (worker) worker.terminate();
  worker = null;
  if (canvas) canvas.remove();
  canvas = null;
  closeAudio();
  for (const cb of savesWaiting.values()) cb({ error: 'The game stopped.' });
  savesWaiting.clear();
  for (const cb of devWaiting.values()) cb({ error: 'The game stopped.' });
  devWaiting.clear();
  $('btn-fullscreen').disabled = true;
  $('btn-saves').disabled = true;
}

// The canvas's backing size in device pixels: exactly the pixels it covers on screen (ResizeObserver's
// device-pixel-content-box where the browser has it, else CSS size x devicePixelRatio). It is the game window's
// client size: the game takes it as its resolution through its own ClientSizeChanged path (terraria-host
// game.rs begin_frame), unless it keeps one of its own (then the picture is fitted: gfx.js presentRect).
let devicePixels = null; // [w, h] from the last ResizeObserver entry
let resizedHook = null; // wireInput's resize check, for frameLoop
// ?devstartsize=WxH (automation): the first measurement gives this, as a page laid out at 0x0 or in a pane still
// opening would; the size is then corrected by the next measurement (the re-measuring in frameLoop).
const devStartSize = /^(\d+)x(\d+)$/.exec(params.get('devstartsize') ?? '');
let measured = 0;
function canvasSize() {
  const r = canvas.getBoundingClientRect();
  const dpr = window.devicePixelRatio || 1;
  // CSS size: the layout box, else the element's client size (a box of 0 while the page is still being laid out)
  const cssW = r.width || canvas.clientWidth || 0;
  const cssH = r.height || canvas.clientHeight || 0;
  let [pw, ph] = devicePixels && devicePixels[0] > 0 && devicePixels[1] > 0 ? devicePixels : [Math.round(cssW * dpr), Math.round(cssH * dpr)];
  if (devStartSize && measured++ === 0) [pw, ph] = [Number(devStartSize[1]), Number(devStartSize[2])];
  return { cssW, cssH, dpr, pw: Math.max(1, pw), ph: Math.max(1, ph) };
}

function startGame() {
  stopGame();
  show(null);
  setStatus('Loading…');
  canvas = document.createElement('canvas');
  canvas.tabIndex = 0;
  $('stage').prepend(canvas);
  const s = canvasSize();
  pixelW = s.pw;
  pixelH = s.ph;
  canvas.width = pixelW;
  canvas.height = pixelH;
  const off = canvas.transferControlToOffscreen();

  worker = new Worker(new URL('./worker.js', import.meta.url), { type: 'module', name: 'game' });
  worker.onmessage = onWorkerMessage;
  worker.onerror = (e) => fail('The game worker failed to start', e.message || String(e));
  worker.postMessage({ type: 'start', canvas: off, crossOriginIsolated: self.crossOriginIsolated, trace: params.has('trace') ? Number(params.get('trace')) || 64 : 0, mock: params.has('mock'), saves: params.get('saves'), fixedStep: params.has('fixedstep'), bridge, install: installMode, recordOrder: params.has('recordorder') }, [off]);
  $('btn-fullscreen').disabled = false;
  $('btn-saves').disabled = false;
  wireInput();
  canvas.focus();
  rafId = requestAnimationFrame(frameLoop);
  // ?devaudio (headless runs, launched with an autoplay policy that needs no gesture): sound from the start
  if (params.has('devaudio')) startAudio();
}

function fail(title, text) {
  if (PLAY) {
    // the play page shows nothing of its own: the reason goes to the console (and window.__twLog); the last
    // frame stays on the canvas
    devEvent({ event: 'failed', title, text });
    console.error(`${title}: ${text}`);
    $('loading').hidden = true;
    if (worker) worker.terminate();
    worker = null;
    return;
  }
  stopGame();
  $('message-title').textContent = title;
  $('message-text').textContent = text;
  show('screen-message');
  setStatus(title);
  addLog(`${title}: ${text}`, 4);
}

function onWorkerMessage(e) {
  const m = e.data;
  switch (m.type) {
    case 'log': addLog(m.text, m.level); break;
    case 'status': setStatus(m.text); break;
    case 'need-ticks': needTicks = m.on; break;
    case 'dev': devEvent(m.data); break;
    case 'dev-reply': {
      const cb = devWaiting.get(m.id);
      devWaiting.delete(m.id);
      cb?.(m);
      break;
    }
    case 'error': devEvent({ event: 'error', text: m.text }); fail('The game stopped with an error', m.text); break;
    case 'no-webgpu': fail('No WebGPU', `${m.text}\nThis build needs Chrome or Edge 137+ with WebGPU enabled (chrome://gpu shows its status).`); break;
    case 'clipboard-set': pendingClipboard = m.text; break;
    case 'title':
      // the game window's title (Game.Window.Title): the play page's document title
      devEvent({ event: 'title', text: m.text });
      if (PLAY && m.text) document.title = m.text;
      break;
    case 'cursor':
      // the game draws its own cursor and hides the system one (IsMouseVisible), as natively
      if (canvas) canvas.style.cursor = m.visible ? '' : 'none';
      if (!cursorSeen) {
        cursorSeen = true;
        devEvent({ event: 'cursor-seen', splash: m.splash });
      }
      // the Re-Logic splash (and the content loading behind it) ended: the title menu is up
      if (sawSplash && !m.splash && m.menu) devEvent({ event: 'title-screen' });
      sawSplash = m.splash;
      break;
    case 'first-frame':
      devEvent({ event: 'first-frame' });
      $('loading').hidden = true;
      break;
    case 'loading': {
      // the game files' first download into this browser (lib/remote-install.js): a plain bar until the game draws
      const el = $('loading');
      el.classList.remove('busy');
      $('loading-bar').style.width = `${Math.round((100 * m.done) / Math.max(1, m.total))}%`;
      break;
    }
    case 'server-start': startServer(m); break;
    case 'server-kill': stopServer(m.id, 'killed by the game (Process.Kill)', -1); break;
    case 'local-connect': localConnect(m); break;
    case 'saves-reply': {
      const cb = savesWaiting.get(m.id);
      savesWaiting.delete(m.id);
      cb?.(m);
      break;
    }
    case 'exit': {
      const code = m.code;
      devEvent({ event: 'exit', code });
      if (PLAY) {
        // Exit in the game's menu: as the native window closes, the game goes; the tab stays black
        stopGame();
        document.title = 'Terraria';
        break;
      }
      stopGame();
      $('message-title').textContent = 'The game has closed';
      $('message-text').textContent = `Exit code ${code}.`;
      show('screen-message');
      setStatus(`Exited (${code})`);
      break;
    }
  }
}

// Per display frame: poll gamepads (the Gamepad API is main-thread only) and relay a tick when the Worker has no
// requestAnimationFrame of its own.
const pads = new Map(); // index -> { timestamp, connected }
let frameCount = 0;
function frameLoop() {
  rafId = requestAnimationFrame(frameLoop);
  if (!worker) return;
  // the canvas size measured again: every display frame for the first 10 s (a page that starts in a viewport still
  // changing size, or laid out at 0x0, or without a ResizeObserver callback yet), then every 30th
  if (++frameCount < 600 || frameCount % 30 === 0) resizedHook?.();
  pollGamepads();
  if (needTicks) worker.postMessage({ type: 'tick' });
}

function pollGamepads() {
  if (!navigator.getGamepads) return;
  const t = epoch();
  const recs = [];
  const seen = new Set();
  for (const gp of navigator.getGamepads()) {
    if (!gp) continue;
    seen.add(gp.index);
    const prev = pads.get(gp.index);
    if (!prev) {
      recs.push(encodeGamepadConnect({ index: gp.index, connected: true, standard: gp.mapping === 'standard', axes: gp.axes.length, buttons: gp.buttons.length, time: t }));
    }
    if (!prev || prev.timestamp !== gp.timestamp) {
      let pressed = 0;
      gp.buttons.forEach((b, i) => { if (i < 32 && b.pressed) pressed |= 1 << i; });
      recs.push(encodeGamepadAxes({ index: gp.index, axes: gp.axes, time: t }));
      recs.push(encodeGamepadButtons({ index: gp.index, pressed: pressed >>> 0, lt: gp.buttons[6]?.value ?? 0, rt: gp.buttons[7]?.value ?? 0, time: t }));
    }
    pads.set(gp.index, { timestamp: gp.timestamp });
  }
  for (const idx of [...pads.keys()]) {
    if (!seen.has(idx)) {
      pads.delete(idx);
      recs.push(encodeGamepadConnect({ index: idx, connected: false, time: t }));
    }
  }
  if (recs.length) send(concat(recs));
}

function toCanvas(e) {
  const r = canvas.getBoundingClientRect();
  return { x: ((e.clientX - r.left) * pixelW) / Math.max(1, r.width), y: ((e.clientY - r.top) * pixelH) / Math.max(1, r.height) };
}

/** Keys the browser keeps: reload, devtools, close/new tab and window shortcuts. Everything else goes to the game. */
function browserKeeps(e) {
  if (e.code === 'F5' || e.code === 'F12') return true;
  // Ctrl+V must reach the browser so the paste event (clipboard_get) fires; Ctrl+C / X likewise for the page
  if ((e.ctrlKey || e.metaKey) && ['KeyR', 'KeyW', 'KeyT', 'KeyN', 'Tab', 'KeyV', 'KeyC', 'KeyX'].includes(e.code)) return true;
  return false;
}

function gameHasKeyboard() {
  const a = document.activeElement;
  return a === canvas || a === $('ime') || a === document.body;
}

function wireInput() {
  const ime = $('ime');
  const focusGame = () => ime.focus({ preventScroll: true });

  on(canvas, 'pointerdown', (e) => {
    focusGame();
    // a click into the game means the window is active, whatever focus / visibility events said before
    // (headless pages often report document.hasFocus() false): the game ignores the mouse while inactive
    if (focusSent === false) send(encodeFocus({ focused: (focusSent = true), time: epoch() }));
    startAudio();
    flushClipboard();
    canvas.setPointerCapture(e.pointerId);
    const p = toCanvas(e);
    send(encodeMouseButton({ down: true, button: e.button, x: p.x, y: p.y, buttons: e.buttons, mods: modsOf(e), time: epoch() }));
    e.preventDefault();
  });
  on(canvas, 'pointerup', (e) => {
    const p = toCanvas(e);
    send(encodeMouseButton({ down: false, button: e.button, x: p.x, y: p.y, buttons: e.buttons, mods: modsOf(e), time: epoch() }));
  });
  on(canvas, 'pointermove', (e) => {
    const p = toCanvas(e);
    window.__twPointer = { clientX: e.clientX, clientY: e.clientY, x: p.x, y: p.y, pixelW, pixelH }; // automation
    send(encodeMouseMove({ x: p.x, y: p.y, buttons: e.buttons, mods: modsOf(e), time: epoch() }));
  });
  on(canvas, 'wheel', (e) => {
    const k = e.deltaMode === 1 ? 40 : e.deltaMode === 2 ? pixelH : 1;
    const p = toCanvas(e);
    send(encodeWheel({ x: p.x, y: p.y, dx: e.deltaX * k, dy: e.deltaY * k, mods: modsOf(e), time: epoch() }));
    e.preventDefault();
  }, { passive: false });
  on(canvas, 'contextmenu', (e) => e.preventDefault());

  on(window, 'keydown', (e) => {
    if (!worker || !gameHasKeyboard()) return;
    if (focusSent === false) send(encodeFocus({ focused: (focusSent = true), time: epoch() }));
    startAudio();
    flushClipboard();
    if (e.code === 'Enter' && e.altKey && !e.repeat) toggleFullscreen();
    send(encodeKey({ down: true, code: e.code, key: e.key, mods: modsOf(e), repeat: e.repeat, location: e.location, time: epoch() }));
    // Printable keys reach the hidden textarea so text and IME input still work.
    const printable = e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey;
    if (!browserKeeps(e) && !printable && !e.isComposing) e.preventDefault();
  });
  on(window, 'keyup', (e) => {
    if (!worker || !gameHasKeyboard()) return;
    send(encodeKey({ down: false, code: e.code, key: e.key, mods: modsOf(e), location: e.location, time: epoch() }));
    if (!browserKeeps(e)) e.preventDefault();
  });

  on(ime, 'input', (e) => {
    if (!e.isComposing && e.inputType === 'insertText' && e.data) send(encodeText(e.data, epoch()));
    if (!e.isComposing) ime.value = '';
  });
  on(ime, 'compositionend', (e) => {
    if (e.data) send(encodeText(e.data, epoch()));
    ime.value = '';
  });

  on(window, 'paste', (e) => {
    if (!worker || !gameHasKeyboard()) return;
    const text = e.clipboardData?.getData('text/plain') ?? '';
    worker.postMessage({ type: 'paste', text });
    e.preventDefault();
  });

  const focus = (f) => {
    focusSent = f;
    devEvent({ event: 'focus', focused: f });
    send(encodeFocus({ focused: f, time: epoch() }));
  };
  on(window, 'focus', () => focus(true));
  on(window, 'blur', () => focus(false));
  on(document, 'visibilitychange', () => focus(document.visibilityState === 'visible' && document.hasFocus()));

  const resized = () => {
    if (!canvas) return;
    resizedHook = resized;
    const s = canvasSize();
    if (s.pw === pixelW && s.ph === pixelH) return;
    pixelW = s.pw;
    pixelH = s.ph;
    send(encodeResize({ pixelWidth: s.pw, pixelHeight: s.ph, cssWidth: s.cssW, cssHeight: s.cssH, dpr: s.dpr, time: epoch() }));
  };
  const ro = new ResizeObserver((entries) => {
    const e = entries[entries.length - 1];
    const d = e.devicePixelContentBoxSize?.[0];
    // (?nodpbox: CSS size x devicePixelRatio instead; headless Chromium's emulated devicePixelRatio leaves the
    // device-pixel box at CSS size, so the automated DPR checks use this path)
    devicePixels = d && !params.has('nodpbox') ? [Math.round(d.inlineSize), Math.round(d.blockSize)] : null;
    resized();
  });
  try {
    ro.observe(canvas, { box: 'device-pixel-content-box' });
  } catch {
    ro.observe(canvas);
  }
  listeners.push(() => ro.disconnect());
  // devicePixelRatio changes (browser zoom, moving to another monitor) do not always resize the element
  on(window, 'resize', resized);
  let dprQuery = null;
  const watchDpr = () => {
    dprQuery?.removeEventListener('change', onDpr);
    dprQuery = matchMedia(`(resolution: ${window.devicePixelRatio || 1}dppx)`);
    dprQuery.addEventListener('change', onDpr);
  };
  const onDpr = () => {
    devicePixels = null; // until the observer reports the new device-pixel size
    resized();
    watchDpr();
  };
  watchDpr();
  listeners.push(() => dprQuery?.removeEventListener('change', onDpr));
  resizedHook = resized;
  resized(); // at once: the size measured when the canvas was made may already be stale
  focusGame();
}

// ------------------------------------------------------------------------------------------------ clipboard
// clipboard_set from the game: written with navigator.clipboard on the player's next click or key press.
let pendingClipboard = null;
function flushClipboard() {
  if (pendingClipboard === null || !navigator.clipboard?.writeText) return;
  const text = pendingClipboard;
  pendingClipboard = null;
  navigator.clipboard.writeText(text).catch((e) => addLog(`clipboard: ${e.message}`, 3));
}

// ------------------------------------------------------------------------------------------------ saves
// Download a .plr / .wld from /saves, or put one there. The Worker owns the OPFS handles, so it does the file work;
// nothing goes over the network.
const savesWaiting = new Map();
let savesSeq = 0;
function savesCall(msg, transfer = []) {
  return new Promise((resolve, reject) => {
    if (!worker) return reject(new Error('Start the game first: it owns the save folder.'));
    const id = ++savesSeq;
    savesWaiting.set(id, (m) => (m.error ? reject(new Error(m.error)) : resolve(m)));
    worker.postMessage({ type: 'saves', id, ...msg }, transfer);
  });
}

const para = (text) => Object.assign(document.createElement('p'), { textContent: text });

async function showSaves() {
  const list = $('saves-list');
  list.replaceChildren(para('Reading /saves…'));
  $('saves-status').textContent = '';
  show('screen-saves');
  try {
    const { files } = await savesCall({ op: 'list' });
    if (!files.length) {
      list.replaceChildren(para('No characters (.plr) or worlds (.wld) yet.'));
      return;
    }
    list.replaceChildren(...files.map((f) => {
      const row = document.createElement('div');
      row.className = 'save-row';
      const label = document.createElement('span');
      label.textContent = `${f.path.replace(/^\/saves\//, '')} · ${f.size >= 0 ? fmtBytes(f.size) : '?'}${f.mtime ? ' · ' + new Date(f.mtime).toLocaleString() : ''}`;
      const btn = document.createElement('button');
      btn.type = 'button';
      btn.textContent = 'Download';
      btn.onclick = () => downloadSave(f.path);
      row.append(label, btn);
      return row;
    }));
  } catch (e) {
    list.replaceChildren(para(e.message));
  }
}

async function downloadSave(path) {
  try {
    const r = await savesCall({ op: 'read', path });
    const url = URL.createObjectURL(new Blob([r.bytes], { type: 'application/octet-stream' }));
    const a = Object.assign(document.createElement('a'), { href: url, download: r.name });
    document.body.append(a);
    a.click();
    a.remove();
    setTimeout(() => URL.revokeObjectURL(url), 10000);
    $('saves-status').textContent = `Downloaded ${r.name}.`;
  } catch (e) {
    $('saves-status').textContent = e.message;
  }
}

async function importSaves(files) {
  const done = [];
  for (const file of files) {
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      let r = await savesCall({ op: 'write', name: file.name, bytes: bytes.buffer, overwrite: false }, [bytes.buffer]);
      if (r.exists) {
        if (!confirm(`${r.path} already exists. Replace it?`)) continue;
        const again = new Uint8Array(await file.arrayBuffer());
        r = await savesCall({ op: 'write', name: file.name, bytes: again.buffer, overwrite: true }, [again.buffer]);
      }
      done.push(r.written);
    } catch (e) {
      done.push(`${file.name}: ${e.message}`);
    }
  }
  await showSaves();
  $('saves-status').textContent = done.length ? `Imported: ${done.join(', ')}.` : '';
}

$('btn-saves').onclick = () => ($('screen-saves').hidden ? showSaves() : show(null));
$('btn-saves-close').onclick = () => show(null);
$('saves-file').onchange = (e) => {
  const files = [...e.target.files];
  e.target.value = '';
  if (files.length) importSaves(files);
};

// ------------------------------------------------------------------------------------------------ fullscreen
function toggleFullscreen() {
  if (document.fullscreenElement) document.exitFullscreen().catch(() => {});
  else $('stage').requestFullscreen({ navigationUI: 'hide' }).catch((e) => addLog(`fullscreen: ${e.message}`, 3));
}
$('btn-fullscreen').onclick = () => {
  toggleFullscreen();
  $('ime').focus({ preventScroll: true });
};

// ------------------------------------------------------------------------------------------------ audio
// Started on the first user gesture (autoplay rule). 44,100 Hz stereo; the AudioWorklet reads either a
// SharedArrayBuffer ring (cross-origin isolated page) or chunks the Worker posts.
let audioCtx = null;
let audioStarting = false;
async function startAudio() {
  if (audioCtx || audioStarting || !worker) return;
  audioStarting = true;
  try {
    const ctx = new AudioContext({ sampleRate: SAMPLE_RATE, latencyHint: 'interactive' });
    await ctx.audioWorklet.addModule(new URL('./audio-worklet.js', import.meta.url));
    const node = new AudioWorkletNode(ctx, 'tw-audio', { numberOfInputs: 0, numberOfOutputs: 1, outputChannelCount: [2] });
    node.connect(ctx.destination);
    // what the worklet played (read by test/run-game-headless.mjs)
    node.port.onmessage = (e) => {
      if (e.data && e.data.type === 'stats') window.__twAudio = { ...e.data, contextState: ctx.state };
    };
    if (ctx.state !== 'running') await ctx.resume();
    const sab = self.crossOriginIsolated ? createRing(RING_FRAMES) : null;
    const ch = new MessageChannel();
    node.port.postMessage({ type: 'link', port: ch.port1, sab }, [ch.port1]);
    if (!worker) {
      ctx.close();
      return;
    }
    worker.postMessage({ type: 'audio', port: ch.port2, sab }, [ch.port2]);
    audioCtx = ctx;
  } catch (e) {
    addLog(`audio: ${e.message}`, 3);
  } finally {
    audioStarting = false;
  }
}
function closeAudio() {
  if (audioCtx) audioCtx.close().catch(() => {});
  audioCtx = null;
}

// ------------------------------------------------------------------------------------------------ boot
async function boot() {
  const miss = missingFeatures();
  if (miss.length) {
    $('unsupported-detail').textContent = 'Missing: ' + miss.join('; ') + '.';
    show('screen-unsupported');
    setStatus('Unsupported browser');
    $('btn-reimport').disabled = true;
    $('btn-saves').disabled = true;
    return;
  }
  addLog(`page ready; cross-origin isolated: ${self.crossOriginIsolated ? 'yes (SharedArrayBuffer audio ring)' : 'no (audio by postMessage)'}`);
  if (PLAY) {
    // saves (and the cached game files) should survive storage pressure: asked once, the browser decides
    navigator.storage.persist?.().then((ok) => addLog(`storage persistence ${ok ? 'granted' : 'not granted'}`), () => {});
    startGame();
    return;
  }
  if (params.has('selftest')) {
    const ok = await selftest().catch((e) => {
      addLog(`selftest: ${e.message}`, 4);
      return false;
    });
    document.documentElement.dataset.selftest = ok ? 'pass' : 'fail';
    if (!ok) return;
    startGame();
    return;
  }
  if (params.has('devimport')) {
    try {
      await devImport();
    } catch (e) {
      devEvent({ event: 'error', text: e.message });
      addLog(e.message, 4);
      showProblems([e.message]);
      return;
    }
    startGame();
    return;
  }
  if (installMode === 'http') {
    // ?httpinstall: the game files come from the page's server (web/host/terraria_host.py), no import
    startGame();
    return;
  }
  const marker = await readMarker();
  // imports made before Terraria.exe was copied (no `exe` in the marker) are completed by choosing the folder again
  if (marker && marker.complete && marker.exe) startGame();
  else showImport(false);
}

boot();

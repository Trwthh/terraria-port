// Runs the real game.wasm in a hidden headless Chromium and records what happens.
//
//   node test/run-game-headless.mjs [--frames 600] [--install <Terraria folder>] [--trace 64] [--port 8147] [--shot-at 900,1500]
//                                   [--actions test/actions/single_player.txt] [--timeout-init 600] [--bridge ws://127.0.0.1:7780]
//                                   [--log web_game_run.txt]   (the log's name under work/logs)
//
// --actions <file>: one action per line, fired when the module has completed frame N (the Worker holds the game
// between frames while the runner acts, so the timing is exact). Coordinates are canvas device pixels.
//   frame N move X Y            pointer to (X, Y)
//   frame N click X Y           move at N, left button down after N+1, up after N+3 (a click the game can see)
//   frame N mouse Down|Up       left button down / up where the pointer is (held: digging)
//   frame N key Down|Up <code>  KeyboardEvent.code, e.g. Enter, KeyA, Escape
//   frame N key Press <code>    down at N, up after N+2
//   frame N type <text>         typed text (characters, as the IME / keyboard would)
//   frame N shot <name>         the back buffer to work/shots/<name>.png
//   frame N diag <name>         record frame N+1 in the replayer: every texture it draws into as
//                               work/shots/diag_target_<id>.png, statistics to work/logs/<name>.json
//   frame N waitwork <name> [every K]
//                               wait for queued work on a fiber (world generation / load: lib/fibers.js) to appear
//                               (up to 600 frames) and to end; a shot work/shots/<name>_<i>.png every K frames
//                               (default 60) meanwhile. Later actions move by the frames it waited (their numbers
//                               count from the end of the work as if it had taken no frames beyond N).
//   frame N profile start       V8 CPU profile of the game's Worker (Chrome DevTools protocol) from here
//   frame N profile stop <name> ... to here: work/logs/<name>.cpuprofile and a summary <name>_profile.txt (self and
//                               inclusive time by function; wasm functions by their names)
//   (--profile-start: the profile starts before tw_init; a later "profile stop" ends it)
//   frame N waitlog <max> <text>
//                               run frames until a page / module log line containing <text> has appeared (since the
//                               line an earlier waitlog matched)
//                               (up to <max> frames; multiplayer: "now in the world", a bridge connection). Later
//                               actions move by the frames it waited, as with waitwork.
//   frame N import <file>       put a .plr / .wld file (path relative to the project) into the page's /saves (Players
//                               or Worlds), as the page's save import does (Host & Play needs a world)
//   frame N saves <label>       log the files under /saves as the game Worker's index sees them (path, size, mtime)
//   frame N console <line>      type <line> into the page's server console box (Host & Play: index.html #sc-input) and
//                               press Enter there, as a player would (the line goes to the server's stdin)
//   frame N waitconsole <max> <text>
//                               run frames until the server console's output area (#sc-out) shows <text> after what
//                               an earlier waitconsole matched (up to <max> frames; fails the run otherwise)
//                               ("<sp>" in <line> is a space: the file's lines are trimmed, "password<sp>" ends in one)
//   frame N consolecleared <text>
//                               fail unless the output area no longer shows <text> (the `clear` command); later
//                               waitconsole actions search from the start of the area again
//   frame N consoledump <name>  the output area's text to work/logs/<name>.txt (compared with the original server's
//                               transcript by test/tools/console_diff.py)
//   frame N savefile <path> <text>
//                               fail unless /saves/<path> (any file, e.g. banlist.txt) exists and contains <text>
//   frame N pageshot <name>     the whole page (HTML around the canvas too: the server console box) to work/shots/<name>.png
//   frame N newer <path>        fail unless /saves/<path> is newer (mtime) or bigger than at the last "saves" action
//   frame N same <path>         fail unless /saves/<path> has the same size and mtime as at the last "saves" action
//   frame N waitmark <max> <name>
//                               run frames until $MARKS_DIR/<name> exists (a run script writes it: what another
//                               process saw, e.g. a native joiner's chat), up to <max> frames; fails otherwise
//   frame N mark <name>         write $MARKS_DIR/<name> (default work/logs/marks; the frame number): a run script waits for it to start
//                               something at this point (tools/ws-bridge/run_hostplay_web.sh: the joining clients)
//   frame N viewport W H [DPR]  the window's size in CSS pixels and its devicePixelRatio (DevTools device metrics;
//                               the canvas follows, so does the game's resolution)
//   frame N cursorcheck <name>  move the pointer to 6 points of the canvas, run 3 frames after each and compare the
//                               game's Main.mouseX / mouseY (tw.frame_info) with the back-buffer pixel under the
//                               pointer, computed here from the canvas rectangle, the canvas size and the fit of the
//                               back buffer (gfx.js fitRect); more than 1 px off fails the run
//   frame N checkcanvas <name>  fail unless the canvas backing size is its CSS size x devicePixelRatio (+-1) and the
//                               game's back buffer is that size (the game's 800x600 minimum and even sizes allowed
//                               for), and the back buffer is not one flat colour (a shot work/shots/<name>.png)
//   frame N checktitle          fail unless document.title is the game window's title (tw.window_title), not empty
//   frame N memory <label>      log the wasm memory size and the module's last "allocated ... live" line
//   frame N waitsteady <max>    run frames until the Worker reports "steady" (30 frames in a row under 50 ms after
//                               tw_init: the title is up), up to <max> frames; later actions move by the frames waited
// --host: the page's own server web/host/terraria_host.py instead of serve.py (/install over HTTP); with
//   --page play the play page at / (no import), else /dev.html?devimport... as before
// --cold: the origin's storage (OPFS: the cached game files, saves) and the HTTP cache are cleared first
// --viewport WxH (default 1280x800), --dpr N (default 1), --record-order (the play page reports the order of its
//   first file opens; --write-order <file> writes it: web/host/pack-order.txt)
// --load-resize WxH:ms  the viewport changes to WxH this long after the page started loading (a pane opening)
// --profile-dir <dir>: the browser profile (default work/web-profile; a second runner needs a copy of its own)
// --no-serve: use the serve.py another runner started on --port (one origin, so the same OPFS import)
// EXTRA_CHROME_ARGS (environment): more Chromium switches, e.g. "--js-flags=--wasm-stack-switching-stack-size=128"
// (the JSPI stacks' size in KB: how deep world generation goes on its work fiber's engine stack).
// --bridge <url>: the page's ?bridge= (Join via IP goes through tools/ws-bridge there; web/ci/run_mp_web.sh starts it).
// '#' starts a comment. With --actions the periodic shots are off and the run ends after the last action.
//
// 1. Starts `serve.py --port 8147 --quiet --install <dir>` (a fixed port, so the OPFS origin is the same every run) (127.0.0.1; serves Terraria.exe + Content/** read-only).
// 2. Opens index.html?devimport&autostart in Playwright's Chromium (new headless, WebGPU on), with a browser profile
//    of its own under <project>/work/web-profile (never the user's Chrome profile). The OPFS copy of the install
//    lives in that profile only; a second run finds it there and copies nothing (resumable import).
// 3. Waits for the import, _initialize, tw_init, then N frames. Every 60 frames, and at the end, the back buffer
//    (gfx.snapshot) is saved as <project>/work/shots/web_title_<n>.png (alpha forced to 255, as presented).
// 4. Writes every log line (page log, module log, console, page errors) to <project>/work/logs/web_game_run.txt.
//    Sound: the page is opened with ?devaudio and Chromium with an autoplay policy that needs no gesture and with
//    its output muted (--mute-audio: nothing reaches the speakers, the AudioWorklet still runs), so the game's mix
//    goes through the real path (audio_free / audio_push -> ring -> AudioWorklet) from the first frame.
// 5. Reports import time, tw_init time and result, frames run, the first error with its stack (and, with --trace,
//    the import calls in progress and the last N calls). Closes the browser and stops the server in all cases.
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';

const WWW = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const PROJECT = path.resolve(WWW, '..', '..');
const LOCAL = process.env.LOCALAPPDATA ?? path.join(os.homedir(), 'AppData', 'Local');
const argv = process.argv.slice(2);
const opt = (name, def) => {
  const i = argv.indexOf('--' + name);
  return i >= 0 ? argv[i + 1] : def;
};
const FRAMES = Number(opt('frames', 600));
const INSTALL = opt('install', 'C:\\Users\\user\\Documents\\Games\\Terraria');
const TRACE = Number(opt('trace', 64));
const INIT_TIMEOUT_S = Number(opt('timeout-init', 600));
const IMPORT_TIMEOUT_S = Number(opt('timeout-import', 1800));
const STALL_S = Number(opt('stall', 60));
// a fixed port: OPFS belongs to the origin (127.0.0.1:<port>), so the same port finds the previous run's import
const PORT = Number(opt('port', 8147));
// --shot-at 900,1500: also save the back buffer exactly after these frame numbers (web_title_at_<n>.png)
const SHOT_AT = String(opt('shot-at', '')).split(',').map(Number).filter((n) => n > 0);
const ACTIONS = opt('actions', null);
const BRIDGE = opt('bridge', null);
const HOST = argv.includes('--host');
const PAGE = opt('page', 'dev');
const COLD = argv.includes('--cold');
const [VIEW_W, VIEW_H] = String(opt('viewport', '1280x800')).split('x').map(Number);
const DPR = Number(opt('dpr', 1));
const RECORD_ORDER = argv.includes('--record-order');
const WRITE_ORDER = opt('write-order', null);
const HOSTNAME = opt('hostname', HOST ? 'terraria.localhost' : '127.0.0.1');
const EXTRA_PARAMS = opt('params', '');
const LOAD_RESIZE = /^(\d+)x(\d+):(\d+)$/.exec(opt('load-resize', '') ?? ''); // more page parameters, e.g. "&nodpbox"

/** Parse an actions file into [{ frame, kind, ... }], clicks and key presses expanded, sorted by frame. */
function parseActions(file) {
  const out = [];
  fs.readFileSync(file, 'utf8').split(/\r?\n/).forEach((raw, i) => {
    const line = raw.replace(/#.*$/, '').trim();
    if (!line) return;
    const m = line.match(/^frame\s+(\d+)\s+(\w+)\s*(.*)$/);
    if (!m) throw new Error(`${file}:${i + 1}: cannot read "${raw}"`);
    const frame = Number(m[1]);
    const kind = m[2];
    const rest = m[3];
    const xy = () => {
      const v = rest.split(/\s+/).map(Number);
      if (v.length !== 2 || v.some((n) => !Number.isFinite(n))) throw new Error(`${file}:${i + 1}: want X Y`);
      return { x: v[0], y: v[1] };
    };
    const at = `${file}:${i + 1}`;
    switch (kind) {
      case 'move': out.push({ frame, kind: 'move', ...xy(), at }); break;
      case 'click': {
        const p = xy();
        out.push({ frame, kind: 'move', ...p, at }, { frame: frame + 1, kind: 'down', at }, { frame: frame + 3, kind: 'up', at });
        break;
      }
      case 'mouse': {
        const how = rest.trim();
        if (how === 'Down') out.push({ frame, kind: 'down', at });
        else if (how === 'Up') out.push({ frame, kind: 'up', at });
        else throw new Error(`${at}: mouse Down|Up`);
        break;
      }
      case 'key': {
        const [how, code] = rest.split(/\s+/);
        if (how === 'Press') out.push({ frame, kind: 'keydown', code, at }, { frame: frame + 2, kind: 'keyup', code, at });
        else if (how === 'Down') out.push({ frame, kind: 'keydown', code, at });
        else if (how === 'Up') out.push({ frame, kind: 'keyup', code, at });
        else throw new Error(`${at}: key Down|Up|Press <code>`);
        break;
      }
      case 'type': out.push({ frame, kind: 'type', text: rest, at }); break;
      case 'import': out.push({ frame, kind: 'import', file: rest.trim(), at }); break;
      case 'mark': out.push({ frame, kind: 'mark', name: rest.trim(), at }); break;
      case 'waitmark': {
        const w = rest.trim().match(/^(\d+)\s+(\S+)$/);
        if (!w) throw new Error(`${at}: waitmark <max frames> <name>`);
        out.push({ frame, kind: 'waitmark', max: Number(w[1]), name: w[2], at });
        break;
      }
      case 'console': out.push({ frame, kind: 'console', text: rest.replaceAll('<sp>', ' '), at }); break;
      case 'consolecleared': out.push({ frame, kind: 'consolecleared', text: rest.trim(), at }); break;
      case 'consoledump': out.push({ frame, kind: 'consoledump', name: rest.trim(), at }); break;
      case 'savefile': {
        const w = rest.trim().match(/^(\S+)\s+(.+)$/);
        if (!w) throw new Error(`${at}: savefile <path> <text>`);
        out.push({ frame, kind: 'savefile', name: w[1], text: w[2], at });
        break;
      }
      case 'newer': out.push({ frame, kind: 'newer', name: rest.trim(), at }); break;
      case 'same': out.push({ frame, kind: 'newer', same: true, name: rest.trim(), at }); break;
      case 'pageshot': out.push({ frame, kind: 'pageshot', name: rest.trim(), at }); break;
      case 'waitconsole': {
        const w = rest.trim().match(/^(\d+)\s+(.+)$/);
        if (!w) throw new Error(`${at}: waitconsole <max frames> <text>`);
        out.push({ frame, kind: 'waitconsole', max: Number(w[1]), text: w[2], at });
        break;
      }
      case 'saves': out.push({ frame, kind: 'saves', name: rest.trim(), at }); break;
      case 'shot': out.push({ frame, kind: 'shot', name: rest.trim(), at }); break;
      case 'viewport': {
        const v = rest.trim().split(/\s+/).map(Number);
        if (v.length < 2 || v.some((n) => !(n > 0))) throw new Error(`${at}: viewport W H [DPR]`);
        out.push({ frame, kind: 'viewport', w: v[0], h: v[1], dpr: v[2] ?? 1, at });
        break;
      }
      case 'cursorcheck': out.push({ frame, kind: 'cursorcheck', name: rest.trim() || 'cursor', at }); break;
      case 'memory': out.push({ frame, kind: 'memory', name: rest.trim(), at }); break;
      case 'checkcanvas': out.push({ frame, kind: 'checkcanvas', name: rest.trim() || 'checkcanvas', at }); break;
      case 'checktitle': out.push({ frame, kind: 'checktitle', at }); break;
      case 'budget': {
        const w = rest.trim().match(/^(\S+)\s+(\d+)$/);
        if (!w) throw new Error(`${at}: budget <label> <frames>`);
        out.push({ frame, kind: 'budget', name: w[1], n: Number(w[2]), at });
        break;
      }
      case 'waitsteady': out.push({ frame, kind: 'waitsteady', max: Number(rest.trim() || 3600), at }); break;
      case 'diag': out.push({ frame, kind: 'diag', name: rest.trim() || 'diag', at }); break;
      case 'profile': {
        const [how, name] = rest.trim().split(/\s+/);
        if (how !== 'start' && !(how === 'stop' && name)) throw new Error(`${at}: profile start | profile stop <name>`);
        out.push({ frame, kind: 'profile', how, name, at });
        break;
      }
      case 'waitlog': {
        const w = rest.trim().match(/^(\d+)\s+(.+)$/);
        if (!w) throw new Error(`${at}: waitlog <max frames> <text>`);
        out.push({ frame, kind: 'waitlog', max: Number(w[1]), text: w[2], at });
        break;
      }
      case 'waitwork': {
        const w = rest.trim().match(/^(\S+)(?:\s+every\s+(\d+))?$/);
        if (!w) throw new Error(`${at}: waitwork <name> [every K]`);
        out.push({ frame, kind: 'waitwork', name: w[1], every: Number(w[2] ?? 60), at });
        break;
      }
      default: throw new Error(`${at}: unknown action "${kind}"`);
    }
  });
  return out.map((a, k) => ({ ...a, k })).sort((a, b) => a.frame - b.frame || a.k - b.k);
}
const LOG_FILE = path.join(PROJECT, 'work', 'logs', opt('log', 'web_game_run.txt'));
const SHOTS = path.join(PROJECT, 'work', 'shots');
const PROFILE = opt('profile-dir', null) ? path.resolve(PROJECT, opt('profile-dir')) : path.join(PROJECT, 'work', 'web-profile');
const NO_SERVE = argv.includes('--no-serve');

function findPlaywright() {
  if (process.env.PLAYWRIGHT_CORE) return process.env.PLAYWRIGHT_CORE;
  const npx = path.join(LOCAL, 'npm-cache', '_npx');
  for (const d of fs.existsSync(npx) ? fs.readdirSync(npx) : []) {
    const p = path.join(npx, d, 'node_modules', 'playwright-core');
    if (fs.existsSync(path.join(p, 'package.json'))) return p;
  }
  throw new Error('playwright-core not found (set PLAYWRIGHT_CORE)');
}
function findChrome() {
  if (process.env.CHROME) return process.env.CHROME;
  const root = path.join(LOCAL, 'ms-playwright');
  const dirs = (fs.existsSync(root) ? fs.readdirSync(root) : []).filter((d) => /^chromium-\d+$/.test(d)).sort().reverse();
  for (const d of dirs) {
    const exe = path.join(root, d, 'chrome-win64', 'chrome.exe');
    if (fs.existsSync(exe)) return exe;
  }
  throw new Error('Playwright Chromium not found (set CHROME)');
}

const t0 = Date.now();
const stamp = () => ((Date.now() - t0) / 1000).toFixed(3).padStart(9);
const lines = []; // everything for the log file, in arrival order
const note = (src, text) => lines.push(`${stamp()} [${src}] ${text}`);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function startServer() {
  return new Promise((resolve, reject) => {
    const cmd = HOST ? [path.join(WWW, '..', 'host', 'terraria_host.py'), '--port', String(PORT), '--quiet', '--install', INSTALL] : [path.join(WWW, 'serve.py'), '--port', String(PORT), '--quiet', '--install', INSTALL];
    const proc = spawn('python', cmd, { cwd: WWW, stdio: ['ignore', 'pipe', 'pipe'], windowsHide: true });
    let buf = '';
    const timer = setTimeout(() => reject(new Error('serve.py did not start within 120 s')), 120000);
    proc.stdout.on('data', (d) => {
      buf += d;
      for (const l of String(d).split('\n').filter(Boolean)) note('serve.py', l.trim());
      const m = buf.match(/http:\/\/127\.0\.0\.1:(\d+)\//);
      if (m) {
        clearTimeout(timer);
        resolve({ proc, port: Number(m[1]) });
      }
    });
    proc.stderr.on('data', (d) => note('serve.py stderr', String(d).trim()));
    proc.on('exit', (code) => {
      clearTimeout(timer);
      reject(new Error(`serve.py exited with ${code}: ${buf}`));
    });
  });
}

async function savePng(page, file, op = 'snapshot', extra = {}, timeoutMs = 15000) {
  const r = await page.evaluate(async ([op, extra, timeoutMs]) => {
    const s = await Promise.race([window.__twDev.request(op, extra), new Promise((_, rej) => setTimeout(() => rej(new Error(`${op}: no answer in ${timeoutMs / 1000} s`)), timeoutMs))]);
    const px = new Uint8ClampedArray(s.data.buffer, s.data.byteOffset, s.data.byteLength);
    for (let i = 3; i < px.length; i += 4) px[i] = 255; // presented opaque
    const c = new OffscreenCanvas(s.width, s.height);
    c.getContext('2d').putImageData(new ImageData(px, s.width, s.height), 0, 0);
    const blob = await c.convertToBlob({ type: 'image/png' });
    const bytes = new Uint8Array(await blob.arrayBuffer());
    let bin = '';
    for (let i = 0; i < bytes.length; i += 0x8000) bin += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
    // a cheap content summary: distinct colours in a 64 x 64 sample grid
    const seen = new Set();
    for (let y = 0; y < 64; y++) for (let x = 0; x < 64; x++) {
      const o = ((Math.floor((y * s.height) / 64) * s.width) + Math.floor((x * s.width) / 64)) * 4;
      seen.add((px[o] << 16) | (px[o + 1] << 8) | px[o + 2]);
    }
    return { b64: btoa(bin), width: s.width, height: s.height, frames: s.frames, colours: seen.size };
  }, [op, extra, timeoutMs]);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, Buffer.from(r.b64, 'base64'));
  note('runner', `saved ${file} (${r.width}x${r.height}, frame ${r.frames}, ${r.colours} distinct colours in a 64x64 sample)`);
  return r;
}

const report = { frames: 0, shots: [] };

// ---------------------------------------------------------------- CPU profiles of the Worker (CDP, flatten off)
const PROFILE_START = argv.includes('--profile-start');
const prof = { cdp: null, worker: null, seq: 0, wait: new Map(), started: false };
async function profAttach() {
  prof.cdp = await context.newCDPSession(page);
  prof.cdp.on('Target.attachedToTarget', (e) => {
    // PROFILE_WORKER=<text>: the Worker whose script URL contains it (server-worker: the page's dedicated server)
    if (e.targetInfo.type === 'worker' && !prof.worker && (!process.env.PROFILE_WORKER || e.targetInfo.url.includes(process.env.PROFILE_WORKER))) {
      prof.worker = e.sessionId;
      note('runner', `profiler: attached to the Worker ${e.targetInfo.url}`);
      if (PROFILE_START) profStart().catch((x) => note('runner', `profiler: ${x.message}`));
    }
  });
  prof.cdp.on('Target.receivedMessageFromTarget', (e) => {
    const m = JSON.parse(e.message);
    const w = m.id !== undefined && prof.wait.get(m.id);
    if (w) {
      prof.wait.delete(m.id);
      m.error ? w.rej(new Error(JSON.stringify(m.error))) : w.res(m.result);
    }
  });
  await prof.cdp.send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: false, flatten: false });
}
function profSend(method, params = {}) {
  const id = ++prof.seq;
  return new Promise((res, rej) => {
    prof.wait.set(id, { res, rej });
    prof.cdp.send('Target.sendMessageToTarget', { sessionId: prof.worker, message: JSON.stringify({ id, method, params }) }).catch(rej);
  });
}
async function profStart() {
  if (!prof.worker) throw new Error('profiler: no Worker attached');
  await profSend('Profiler.enable');
  await profSend('Profiler.setSamplingInterval', { interval: 500 });
  await profSend('Profiler.start');
  prof.started = true;
  note('runner', 'profiler: started');
}
async function profStop(name) {
  const r = await profSend('Profiler.stop');
  prof.started = false;
  const p = r.profile;
  const out = path.join(PROJECT, 'work', 'logs', `${name}.cpuprofile`);
  fs.writeFileSync(out, JSON.stringify(p));
  // self and inclusive time by function name (a function counted once per sample however deep it recurses)
  const byId = new Map(p.nodes.map((n) => [n.id, n]));
  const parent = new Map();
  for (const n of p.nodes) for (const c of n.children ?? []) parent.set(c, n.id);
  const fname = (n) => n.callFrame.functionName || `(anonymous ${path.basename(n.callFrame.url || '')})`;
  const self = new Map();
  const incl = new Map();
  let total = 0;
  p.samples.forEach((id, i) => {
    const dt = (p.timeDeltas[i] ?? 0) / 1000;
    total += dt;
    const n = byId.get(id);
    self.set(fname(n), (self.get(fname(n)) ?? 0) + dt);
    const seen = new Set();
    for (let k = id; k !== undefined; k = parent.get(k)) {
      const f = fname(byId.get(k));
      if (!seen.has(f)) {
        seen.add(f);
        incl.set(f, (incl.get(f) ?? 0) + dt);
      }
    }
  });
  const top = (m, n) => [...m].sort((a, b) => b[1] - a[1]).slice(0, n).map(([f, ms]) => `${ms.toFixed(0).padStart(8)} ms  ${(100 * ms / total).toFixed(1).padStart(5)}%  ${f}`).join('\n');
  const txt = `profile ${name}: ${total.toFixed(0)} ms sampled\n\n-- self time --\n${top(self, 60)}\n\n-- inclusive time --\n${top(incl, 150)}\n`;
  const sum = path.join(PROJECT, 'work', 'logs', `${name}_profile.txt`);
  fs.writeFileSync(sum, txt);
  note('runner', `profiler: ${name}: ${total.toFixed(0)} ms -> ${out}, summary ${sum}`);
}
let server = null;
let pageCdp = null;
let context = null;
let page = null;
let logCursor = 0;
let evCursor = 0;
const events = [];

async function pull() {
  // copy new page-log lines and dev events into the log, in order
  const r = await page.evaluate(([lc, ec]) => ({
    log: (window.__twLog ?? []).slice(lc),
    ev: (window.__twDev?.events ?? []).slice(ec),
  }), [logCursor, evCursor]);
  logCursor += r.log.length;
  evCursor += r.ev.length;
  for (const l of r.log) note(`page L${l.level}`, l.text);
  for (const e of r.ev) {
    events.push(e);
    note('event', JSON.stringify(e).slice(0, 4000));
  }
  return r.ev;
}

/** A request to the Worker that cannot hang the runner (the Worker may be gone after a trap). */
const ask = (op) => page.evaluate((op) => Promise.race([
  window.__twDev.request(op),
  new Promise((_, rej) => setTimeout(() => rej(new Error(op + ': no answer in 15 s')), 15000)),
]), op);

const firstError = () => events.find((e) => e.event === 'error' || e.event === 'exit');

try {
  server = NO_SERVE ? null : await startServer();
  const base = `http://${HOSTNAME}:${server ? server.port : PORT}`;
  note('runner', `server on ${base}, install ${INSTALL}, profile ${PROFILE}, frames ${FRAMES}, trace ${TRACE}`);
  const require = createRequire(findPlaywright() + path.sep);
  const { chromium } = require('playwright-core');
  fs.mkdirSync(PROFILE, { recursive: true });
  context = await chromium.launchPersistentContext(PROFILE, {
    headless: true,
    executablePath: findChrome(),
    // the machine's own adapter by default (fast); GPU=swiftshader for the software one
    args: [...(process.env.GPU === 'swiftshader' ? ['--enable-unsafe-webgpu', '--use-webgpu-adapter=swiftshader'] : ['--enable-unsafe-webgpu']), '--autoplay-policy=no-user-gesture-required', '--mute-audio', ...(process.env.EXTRA_CHROME_ARGS ? process.env.EXTRA_CHROME_ARGS.split(' ').filter(Boolean) : [])],
    viewport: { width: VIEW_W, height: VIEW_H },
    deviceScaleFactor: DPR,
  });
  page = context.pages()[0] ?? (await context.newPage());
  page.on('console', (m) => note(`console.${m.type()}`, m.text()));
  page.on('pageerror', (e) => note('pageerror', String(e.stack ?? e)));
  page.on('crash', () => note('runner', 'PAGE CRASHED'));
  // every URL the page and its Workers request (the play page must load nothing but its runtime, game.wasm and /install)
  context.on('request', (rq) => {
    try {
      const u = new URL(rq.url());
      (report.requests ??= new Map()).set(u.origin + u.pathname.replace(/^\/install\/(pack|file)\/.*/, '/install/$1/<id>'), (report.requests.get(u.origin + u.pathname) ?? 0) + 1);
    } catch {}
  });
  if (PROFILE_START || (ACTIONS && /^\s*frame\s+\d+\s+profile\b/m.test(fs.readFileSync(ACTIONS, 'utf8')))) await profAttach();
  const savesParam = ACTIONS ? `&saves=run${Date.now()}&fixedstep` : ''; // an action run starts with no players or worlds
  const bridgeParam = BRIDGE ? `&bridge=${encodeURIComponent(BRIDGE)}` : '';
  if (COLD) {
    // a first start: no cached game files, no saves, no HTTP cache (so no cached game.wasm or compiled code either)
    const c = await context.newCDPSession(page);
    await c.send('Storage.clearDataForOrigin', { origin: base, storageTypes: 'all' });
    await c.send('Network.clearBrowserCache');
    await c.detach();
    note('runner', `cold start: storage of ${base} and the HTTP cache cleared`);
  }
  const tNav = Date.now();
  if (LOAD_RESIZE) {
    const [, w, h, ms] = LOAD_RESIZE.map(Number);
    setTimeout(() => {
      page.setViewportSize({ width: w, height: h }).then(() => note('runner', `viewport changed to ${w}x${h} while loading (${ms} ms after the start)`), (e) => note('runner', `load-resize: ${e.message}`));
    }, ms);
  }
  if (PAGE === 'play') await page.goto(`${base}/?devaudio${TRACE ? `&trace=${TRACE}` : ''}${savesParam}${bridgeParam}${RECORD_ORDER ? '&recordorder' : ''}${EXTRA_PARAMS}`);
  else await page.goto(`${base}/${HOST ? 'dev.html?httpinstall&' : '?devimport&'}autostart&devaudio${TRACE ? `&trace=${TRACE}` : ''}${savesParam}${bridgeParam}`);
  report.navMs = Date.now() - tNav;

  // import (the developer page)
  let deadline = Date.now() + IMPORT_TIMEOUT_S * 1000;
  for (; PAGE !== 'play' && !HOST;) {
    await pull();
    const done = events.find((e) => e.event === 'import-done');
    if (done) {
      report.import = done;
      break;
    }
    if (firstError()) break;
    if (Date.now() > deadline) throw new Error(`import did not finish in ${IMPORT_TIMEOUT_S} s`);
    await sleep(500);
  }

  // load, _initialize, tw_init
  deadline = Date.now() + INIT_TIMEOUT_S * 1000;
  while (!firstError()) {
    await pull();
    const done = events.find((e) => e.event === 'init-done');
    if (done) {
      report.init = done;
      break;
    }
    if (Date.now() > deadline) {
      report.initTimeout = true;
      const st = await ask('stats').catch((e) => ({ error: e.message }));
      note('runner', `tw_init did not return within ${INIT_TIMEOUT_S} s; stats ${JSON.stringify(st)}`);
      report.stats = st;
      break;
    }
    await sleep(500);
  }
  for (const k of ['loaded', 'initialize-done']) report[k] = events.find((e) => e.event === k);

  // frames
  if (report.init && report.init.rc === 0 && ACTIONS) {
    const actions = parseActions(ACTIONS);
    note('runner', `${actions.length} actions from ${ACTIONS}`);
    const tFrames = Date.now();
    const toPage = async (x, y) => page.evaluate(([x, y]) => {
      const c = document.querySelector('canvas');
      const r = c.getBoundingClientRect();
      const dpr = window.devicePixelRatio || 1;
      const pw = Math.max(1, Math.round(r.width * dpr));
      const ph = Math.max(1, Math.round(r.height * dpr));
      return { x: r.left + ((x + 0.5) * r.width) / pw, y: r.top + ((y + 0.5) * r.height) / ph };
    }, [x, y]);
    const runTo = (frame) => page.evaluate((frame) => window.__twDev.request('run-to', { frame }), frame);
    report.actions = [];
    report.work = [];
    let offset = 0; // frames added by waitwork
    let logMark = 0; // waitlog: lines seen by an earlier waitlog do not count again
    let consoleMark = 0; // waitconsole: the output area's text up to here was matched by an earlier waitconsole
    let lastSaves = new Map(); // newer: /saves path -> { size, mtime } at the last "saves" action
    try {
      for (const a of actions) {
        if (firstError()) break;
        const t = Date.now();
        const r = await runTo(a.frame + offset);
        const waited = (Date.now() - t) / 1000;
        if (waited > 5) note('runner', `reached frame ${r.frames} after ${waited.toFixed(1)} s`);
        await pull();
        switch (a.kind) {
          case 'move': {
            const p = await toPage(a.x, a.y);
            await page.mouse.move(p.x, p.y);
            break;
          }
          case 'viewport': {
            // the context's own ratio: Playwright's viewport (it re-applies its metrics, so a DevTools override of
            // the size alone does not stay); another ratio: a DevTools device-metrics override
            if (a.dpr === DPR) await page.setViewportSize({ width: a.w, height: a.h });
            else {
              pageCdp ??= await context.newCDPSession(page);
              await pageCdp.send('Emulation.setDeviceMetricsOverride', { width: a.w, height: a.h, deviceScaleFactor: a.dpr, mobile: false });
            }
            note('runner', `viewport ${a.w}x${a.h} CSS px at devicePixelRatio ${a.dpr}`);
            break;
          }
          case 'checkcanvas': {
            const g = await page.evaluate(() => {
              const c = document.querySelector('canvas');
              const rc = c.getBoundingClientRect();
              return { w: rc.width, h: rc.height, dpr: devicePixelRatio, title: document.title };
            });
            const st = await ask('stats');
            const p = st.presented;
            const ew = Math.round(g.w * g.dpr);
            const eh = Math.round(g.h * g.dpr);
            const shot = await savePng(page, path.join(SHOTS, `${a.name}.png`));
            const bw = Math.max(p.canvasW, 800) & ~1;
            const bh = Math.max(p.canvasH, 600) & ~1;
            const okCanvas = Math.abs(p.canvasW - ew) <= 1 && Math.abs(p.canvasH - eh) <= 1;
            const okBack = p.canvasW > 1920 || p.canvasH > 1200 || (Math.abs(p.backW - bw) <= 1 && Math.abs(p.backH - bh) <= 1);
            const rec = { name: a.name, css: `${g.w}x${g.h}`, dpr: g.dpr, canvas: `${p.canvasW}x${p.canvasH}`, expectedCanvas: `${ew}x${eh}`, back: `${p.backW}x${p.backH}`, expectedBack: `${bw}x${bh}`, colours: shot.colours };
            (report.canvasChecks ??= []).push(rec);
            note('runner', `checkcanvas ${JSON.stringify(rec)}`);
            if (!okCanvas || !okBack || shot.colours < 32) throw new Error(`checkcanvas ${a.name}: ${JSON.stringify(rec)}`);
            break;
          }
          case 'checktitle': {
            const t = await page.evaluate(() => document.title);
            const st = await ask('stats');
            note('runner', `checktitle: document "${t}", game window "${st.window?.title}"`);
            report.titleCheck = { document: t, game: st.window?.title };
            if (!t || t !== st.window?.title) throw new Error(`checktitle: document.title "${t}" is not the game's window title "${st.window?.title}"`);
            break;
          }
          case 'memory': {
            await pull();
            const st = await ask('stats');
            const live = [...lines].reverse().find((l) => /allocated [\d.]+ MB live/.test(l));
            const m = live?.match(/tick (\d+): allocated ([\d.]+) MB live/);
            const rec = { label: a.name, frame: st.frames, wasmMB: Math.round(st.memoryBytes / 1048576), liveMB: m ? Number(m[2]) : null, liveTick: m ? Number(m[1]) : null, textures: st.texturesCreated, back: st.presented ? `${st.presented.backW}x${st.presented.backH}` : null, canvas: st.presented ? `${st.presented.canvasW}x${st.presented.canvasH}` : null, gfxLive: st.gfxLive, jsHeapMB: st.jsHeapMB };
            (report.memoryLog ??= []).push(rec);
            note('runner', `memory ${a.name}: ${JSON.stringify(rec)}`);
            break;
          }
          case 'budget': {
            // the frame budget: the module's time per tw_frame and the wall time per display frame over n frames
            const st0 = await ask('stats');
            const t0 = Date.now();
            await runTo(st0.frames + a.n);
            const t1 = Date.now();
            const st1 = await ask('stats');
            const n = st1.frames - st0.frames;
            const rec = { label: a.name, frames: n, moduleMsPerFrame: +((st1.timing.total - st0.timing.total) / n).toFixed(2), wallMsPerFrame: +((t1 - t0) / n).toFixed(2), fps: +((1000 * n) / (t1 - t0)).toFixed(1), back: `${st1.presented.backW}x${st1.presented.backH}` };
            (report.budget ??= []).push(rec);
            offset += n;
            note('runner', `budget ${a.name}: ${JSON.stringify(rec)}`);
            break;
          }
          case 'waitsteady': {
            const from = r.frames;
            // the title screen (the game's splash ended: tw.frame_info), or with a module without frame_info "steady"
            const reached = () => events.some((e) => e.event === 'title-screen' || e.event === 'title-reached') || (!events.some((e) => e.event === 'cursor-seen') && events.some((e) => e.event === 'steady'));
            while (!reached()) {
              if (firstError()) break;
              const now = await runTo((await ask('stats')).frames + 10);
              await pull();
              if (now.frames - from > a.max) throw new Error(`waitsteady: not steady within ${a.max} frames`);
            }
            const st = await ask('stats');
            offset += st.frames - from;
            note('runner', `title reached at frame ${st.frames} (waited ${st.frames - from} frames)`);
            break;
          }
          case 'cursorcheck': {
            const pts = [[0.5, 0.5], [0.1, 0.1], [0.9, 0.12], [0.15, 0.88], [0.87, 0.9], [0.33, 0.66]];
            const res = [];
            const from = r.frames;
            for (const [fx, fy] of pts) {
              const g = await page.evaluate(([fx, fy]) => {
                const c = document.querySelector('canvas');
                const rc = c.getBoundingClientRect();
                return { x: Math.round(rc.left + fx * rc.width), y: Math.round(rc.top + fy * rc.height), left: rc.left, top: rc.top, w: rc.width, h: rc.height, dpr: devicePixelRatio, iw: innerWidth, ih: innerHeight };
              }, [fx, fy]);
              await page.mouse.move(g.x, g.y);
              await page.evaluate(() => window.__twDev.request('sync'));
              const st0 = await ask('stats');
              await runTo(st0.frames + 3);
              const st = await ask('stats');
              const p = st.presented;
              // the pointer's canvas pixel, then the back-buffer pixel under it (gfx.js fitRect, done again here)
              const cx = ((g.x - g.left) * p.canvasW) / g.w;
              const cy = ((g.y - g.top) * p.canvasH) / g.h;
              let rx = 0, ry = 0, rw = p.canvasW, rh = p.canvasH;
              if (p.backW !== p.canvasW || p.backH !== p.canvasH) {
                const k = Math.min(p.canvasW / p.backW, p.canvasH / p.backH);
                rw = Math.max(1, Math.round(p.backW * k));
                rh = Math.max(1, Math.round(p.backH * k));
                rx = Math.floor((p.canvasW - rw) / 2);
                ry = Math.floor((p.canvasH - rh) / 2);
              }
              const ex = Math.floor(((cx - rx) * p.backW) / rw);
              const ey = Math.floor(((cy - ry) * p.backH) / rh);
              const gx = st.window?.mouseX;
              const gy = st.window?.mouseY;
              const ptr = await page.evaluate(() => window.__twPointer ?? null);
              res.push({ pointer: ptr, css: [g.x, g.y], dpr: g.dpr, window: `${g.iw}x${g.ih}`, canvas: `${p.canvasW}x${p.canvasH}`, back: `${p.backW}x${p.backH}`, expected: [ex, ey], game: [gx, gy], dx: gx - ex, dy: gy - ey });
            }
            const st = await ask('stats');
            offset += st.frames - from;
            const worst = Math.max(...res.map((q) => Math.max(Math.abs(q.dx), Math.abs(q.dy))));
            (report.cursor ??= []).push({ name: a.name, worst, points: res });
            note('runner', `cursorcheck ${a.name}: worst ${worst} px; ${JSON.stringify(res)}`);
            if (!(worst <= 1)) throw new Error(`cursorcheck ${a.name}: the game's mouse is ${worst} px from the pointer (${JSON.stringify(res)})`);
            break;
          }
          case 'down': await page.mouse.down(); break;
          case 'up': await page.mouse.up(); break;
          case 'keydown': await page.keyboard.down(a.code); break;
          case 'keyup': await page.keyboard.up(a.code); break;
          case 'type': await page.keyboard.type(a.text); break;
          case 'import': {
            const file = path.resolve(PROJECT, a.file);
            const b64 = fs.readFileSync(file).toString('base64');
            const r2 = await page.evaluate(async ([name, b64]) => {
              const bin = atob(b64);
              const bytes = new Uint8Array(bin.length);
              for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
              return window.__twDev.saves({ op: 'write', name, bytes: bytes.buffer, overwrite: true }, [bytes.buffer]);
            }, [path.basename(file), b64]);
            note('runner', `import ${file}: ${JSON.stringify(r2)}`);
            break;
          }
          case 'saves': {
            const r3 = await page.evaluate(() => window.__twDev.saves({ op: 'list', all: true }));
            for (const f of r3.files) note('runner', `saves ${a.name}: ${f.path} ${f.size} bytes, modified ${f.mtime ? new Date(f.mtime).toISOString() : '?'}`);
            lastSaves = new Map(r3.files.map((f) => [f.path, f]));
            break;
          }
          case 'newer': {
            const r4 = await page.evaluate(() => window.__twDev.saves({ op: 'list', all: true }));
            const key = a.name.startsWith('/saves/') ? a.name : `/saves/${a.name.replace(/^\/+/, '')}`;
            const now = r4.files.find((f) => f.path === key);
            const before = lastSaves.get(key);
            const fmt = (f) => (f ? `${f.size} bytes, modified ${f.mtime ? new Date(f.mtime).toISOString() : '?'}` : 'missing');
            note('runner', `newer ${a.name}: before ${fmt(before)}, now ${fmt(now)}`);
            if (a.same) {
              // (the index reports an mtime a few ms apart after it re-read a directory another Worker changed; a real write
              // carries the time of the write, seconds or minutes later)
              if (!now || !before || Math.abs(now.mtime - before.mtime) > 1000 || now.size !== before.size) throw new Error(`same: /saves/${a.name} changed (or is missing) since the last "saves" action`);
              break;
            }
            if (!now || (before && !(now.mtime > before.mtime || now.size !== before.size))) throw new Error(`newer: /saves/${a.name} was not written since the last "saves" action`);
            break;
          }
          case 'pageshot': {
            await page.screenshot({ path: path.join(SHOTS, `${a.name}.png`) });
            note('runner', `page screenshot work/shots/${a.name}.png`);
            break;
          }
          case 'console': {
            const ok = await page.evaluate(() => {
              const i = document.getElementById('sc-input');
              return !!i && !i.disabled && !document.getElementById('server-console').hidden;
            });
            if (!ok) throw new Error(`console: the server console box is not open (no server runs in the page?)`);
            await page.click('#sc-input');
            await page.fill('#sc-input', a.text);
            await page.press('#sc-input', 'Enter');
            break;
          }
          case 'consolecleared': {
            const text = await page.evaluate(() => document.getElementById('sc-out')?.textContent ?? '');
            note('runner', `consolecleared "${a.text}": the output area holds ${text.length} characters${text.includes(a.text) ? ', and still shows it' : ''}: ${JSON.stringify(text.slice(0, 200))}`);
            if (text.includes(a.text)) throw new Error(`consolecleared: the server console still shows "${a.text}"`);
            consoleMark = 0;
            break;
          }
          case 'consoledump': {
            const text = await page.evaluate(() => document.getElementById('sc-out')?.textContent ?? '');
            fs.writeFileSync(path.join(PROJECT, 'work', 'logs', `${a.name}.txt`), text);
            note('runner', `consoledump: ${text.length} characters to work/logs/${a.name}.txt`);
            break;
          }
          case 'savefile': {
            const key = a.name.startsWith('/saves/') ? a.name : `/saves/${a.name.replace(/^\/+/, '')}`;
            const r5 = await page.evaluate(async (key) => {
              try {
                const f = await window.__twDev.saves({ op: 'read', path: key, all: true });
                return { text: new TextDecoder().decode(f.bytes) };
              } catch (e) {
                return { error: String(e?.message ?? e) };
              }
            }, key);
            note('runner', `savefile ${key}: ${r5.error ? `cannot read: ${r5.error}` : JSON.stringify(r5.text)}`);
            if (r5.error || !r5.text.includes(a.text)) throw new Error(`savefile: ${key} ${r5.error ? 'cannot be read' : `does not contain "${a.text}"`}`);
            break;
          }
          case 'waitconsole': {
            const start = r.frames;
            let f = start;
            let at = -1;
            const tWait = Date.now();
            for (;;) {
              if (firstError()) break;
              const text = await page.evaluate(() => document.getElementById('sc-out')?.textContent ?? '');
              at = text.indexOf(a.text, consoleMark);
              if (at >= 0 || f - start >= a.max) break;
              f = (await runTo(f + 5)).frames;
              await pull();
            }
            if (at >= 0) consoleMark = at + a.text.length;
            note('runner', `waitconsole "${a.text}": ${at >= 0 ? `shown after ${f - start} frames` : `NOT shown in ${f - start} frames`} (${((Date.now() - tWait) / 1000).toFixed(1)} s)`);
            if (at < 0) throw new Error(`waitconsole: "${a.text}" not shown within ${a.max} frames after frame ${start}`);
            offset = f - a.frame;
            break;
          }
          case 'waitmark': {
            const dir = process.env.MARKS_DIR ? path.resolve(PROJECT, process.env.MARKS_DIR) : path.join(PROJECT, 'work', 'logs', 'marks');
            const start = r.frames;
            let f = start;
            const tWait = Date.now();
            while (!fs.existsSync(path.join(dir, a.name)) && f - start < a.max && !firstError()) {
              f = (await runTo(f + 5)).frames;
              await pull();
            }
            const ok = fs.existsSync(path.join(dir, a.name));
            note('runner', `waitmark ${a.name}: ${ok ? `there after ${f - start} frames` : `NOT there in ${f - start} frames`} (${((Date.now() - tWait) / 1000).toFixed(1)} s)`);
            if (!ok) throw new Error(`waitmark: ${a.name} not written within ${a.max} frames after frame ${start}`);
            offset = f - a.frame;
            break;
          }
          case 'mark': {
            const dir = process.env.MARKS_DIR ? path.resolve(PROJECT, process.env.MARKS_DIR) : path.join(PROJECT, 'work', 'logs', 'marks');
            fs.mkdirSync(dir, { recursive: true });
            fs.writeFileSync(path.join(dir, a.name), String(r.frames));
            note('runner', `mark ${a.name} at frame ${r.frames}`);
            break;
          }
          case 'diag': {
            await page.evaluate(() => window.__twDev.request('diag-arm'));
            await page.evaluate(() => window.__twDev.request('resume'));
            await runTo(a.frame + offset + 1);
            const res = await page.evaluate(async () => {
              const r = await window.__twDev.request('diag-result');
              const pngs = [];
              for (const im of r.images) {
                if (im.error) { pngs.push({ id: im.id, error: im.error }); continue; }
                const px = new Uint8ClampedArray(im.data.buffer, im.data.byteOffset, im.data.byteLength);
                // alpha kept as it is (a target's alpha matters); a second copy forced opaque shows the colours
                const enc = async (opaque) => {
                  const p2 = opaque ? new Uint8ClampedArray(px) : px;
                  if (opaque) for (let i = 3; i < p2.length; i += 4) p2[i] = 255;
                  const c = new OffscreenCanvas(im.width, im.height);
                  c.getContext('2d').putImageData(new ImageData(p2, im.width, im.height), 0, 0);
                  const b = new Uint8Array(await (await c.convertToBlob({ type: 'image/png' })).arrayBuffer());
                  let bin = '';
                  for (let i = 0; i < b.length; i += 0x8000) bin += String.fromCharCode(...b.subarray(i, i + 0x8000));
                  return btoa(bin);
                };
                // content summary: share of fully transparent pixels and of RGB-black pixels
                let transparent = 0;
                let black = 0;
                for (let i = 0; i < px.length; i += 4) {
                  if (px[i + 3] === 0) transparent++;
                  if (!px[i] && !px[i + 1] && !px[i + 2]) black++;
                }
                const n = px.length / 4;
                pngs.push({ id: im.id, width: im.width, height: im.height, transparent: +(transparent / n).toFixed(3), rgbBlack: +(black / n).toFixed(3), png: await enc(false), opaque: await enc(true) });
              }
              return { capture: r.capture, pngs };
            });
            for (const p of res.pngs) {
              if (p.error) { note('runner', `diag target ${p.id}: ${p.error}`); continue; }
              fs.writeFileSync(path.join(SHOTS, `diag_target_${p.id}.png`), Buffer.from(p.png, 'base64'));
              fs.writeFileSync(path.join(SHOTS, `diag_target_${p.id}_opaque.png`), Buffer.from(p.opaque, 'base64'));
              res.capture.targets[p.id].image = { file: `work/shots/diag_target_${p.id}.png`, size: `${p.width}x${p.height}`, transparent: p.transparent, rgbBlack: p.rgbBlack };
              delete p.png;
              delete p.opaque;
            }
            const out = path.join(PROJECT, 'work', 'logs', `${a.name}.json`);
            fs.writeFileSync(out, JSON.stringify(res.capture, null, 2));
            note('runner', `diag: frame ${res.capture.serial} captured, ${res.capture.order.length} targets -> ${out}`);
            report.diag = out;
            break;
          }
          case 'waitwork': {
            const start = r.frames;
            let f = start;
            let appeared = false;
            let i = 0;
            const tWork = Date.now();
            let first = null;
            for (;;) {
              if (firstError()) break;
              const st = await ask('stats');
              const live = st.fibers?.live ?? 0;
              if (live > 0 && !appeared) {
                appeared = true;
                first = f;
              }
              if (appeared && live === 0) break;
              if (!appeared && f - start >= 600) {
                note('runner', `waitwork ${a.name}: no work fiber in 600 frames (fibers ${JSON.stringify(st.fibers)})`);
                break;
              }
              if (f - start > 30000) throw new Error(`waitwork ${a.name}: work still running after 30000 frames`);
              f = (await runTo(f + (appeared ? a.every : 1))).frames;
              await pull();
              const st2 = await ask('stats');
              if (appeared && (st2.fibers?.live ?? 0) > 0) {
                const name = `${a.name}_${String(++i).padStart(2, '0')}`;
                const s = await savePng(page, path.join(SHOTS, `${name}.png`));
                report.shots.push({ file: `work/shots/${name}.png`, frame: s.frames, size: `${s.width}x${s.height}`, colours: s.colours, memoryMB: Math.round(st2.memoryBytes / 1048576) });
              }
            }
            const st = await ask('stats');
            const w = { name: a.name, from: start, workFrom: first, to: f, seconds: (Date.now() - tWork) / 1000, fibers: st.fibers, timing: { workFrames: st.timing.workFrames, workMax: Math.round(st.timing.workMax), workMaxFrame: st.timing.workMaxFrame, workMean: +(st.timing.workTotal / Math.max(1, st.timing.workFrames)).toFixed(1) } };
            report.work.push(w);
            note('runner', `waitwork ${a.name}: ${JSON.stringify(w)}`);
            offset = f - a.frame;
            break;
          }
          case 'waitlog': {
            const from = logMark;
            const start = r.frames;
            let f = start;
            let hit = null;
            const tWait = Date.now();
            for (;;) {
              if (firstError()) break;
              hit = lines.slice(from).find((l) => /\[(page L\d|console\.\w+)\]/.test(l) && l.includes(a.text));
              if (hit || f - start >= a.max) break;
              f = (await runTo(f + 5)).frames;
              await pull();
            }
            if (hit) logMark = lines.indexOf(hit) + 1;
            const w = { name: `waitlog "${a.text}"`, from: start, to: f, seconds: (Date.now() - tWait) / 1000, found: !!hit };
            (report.waits ??= []).push(w);
            note('runner', `waitlog "${a.text}": ${hit ? `seen after ${f - start} frames` : `NOT seen in ${f - start} frames`} (${w.seconds.toFixed(1)} s)`);
            if (!hit) throw new Error(`waitlog: "${a.text}" not seen within ${a.max} frames after frame ${start}`);
            offset = f - a.frame;
            break;
          }
          case 'profile': {
            if (a.how === 'start') await profStart();
            else await profStop(a.name);
            break;
          }
          case 'shot': {
            const s = await savePng(page, path.join(SHOTS, `${a.name}.png`));
            const st = await ask('stats').catch(() => null);
            const mb = st ? Math.round(st.memoryBytes / 1048576) : null;
            report.shots.push({ file: `work/shots/${a.name}.png`, frame: s.frames, size: `${s.width}x${s.height}`, colours: s.colours, memoryMB: mb, maxTex: st?.maxTextureId, created: st?.texturesCreated });
            break;
          }
        }
        // every input message is in the Worker's queue before the next frame starts
        await page.evaluate(() => window.__twDev.request('sync'));
        if (process.env.INPUT_DEBUG) {
          const st = await ask('stats').catch(() => null);
          note('runner', `input seen ${JSON.stringify(st?.inputSeen)} last ${JSON.stringify(st?.inputLast?.slice(-3))}`);
        }
        note('runner', `frame ${r.frames}: ${a.kind}${a.x !== undefined ? ` ${a.x},${a.y}` : ''}${a.code ? ' ' + a.code : ''}${a.text ? ` "${a.text}"` : ''}${a.name ? ' ' + a.name : ''}`);
        report.actions.push(a.frame);
      }
      const st = await ask('stats').catch(() => null);
      if (st) {
        report.frames = st.frames;
        report.memory = st.memoryBytes;
        report.timing = st.timing;
      }
      await page.evaluate(() => window.__twDev.request('resume')).catch(() => {});
    } catch (e) {
      note('runner', `actions stopped: ${e.message}`);
      report.actionsError = e.message;
    }
    report.framesSeconds = (Date.now() - tFrames) / 1000;
    report.actionsDone = `${report.actions.length} of ${actions.length}`;
  } else if (report.init && report.init.rc === 0) {
    // exact-frame shots: requested now, answered by the Worker right after that frame
    const atShots = SHOT_AT.map((n) => savePng(page, path.join(SHOTS, `web_title_at_${n}.png`), 'snapshot-at', { frame: n }, 3600000)
      .then((r) => report.shots.push({ file: `work/shots/web_title_at_${n}.png`, frame: r.frames, size: `${r.width}x${r.height}`, colours: r.colours }))
      .catch((e) => note('runner', `shot at ${n} failed: ${e.message}`)));
    let nextShot = 60;
    let nextStat = 0;
    let last = 0;
    let lastChange = Date.now();
    const tFrames = Date.now();
    while (!firstError()) {
      await pull();
      const st = await ask('stats').catch((e) => ({ error: e.message }));
      if (st.error) {
        note('runner', `stats: ${st.error}`);
        break;
      }
      report.frames = st.frames;
      if (st.frames !== last) {
        last = st.frames;
        lastChange = Date.now();
      } else if (Date.now() - lastChange > STALL_S * 1000) {
        report.stall = { frames: st.frames, seconds: STALL_S, trace: st.trace };
        note('runner', `no frame finished for ${STALL_S} s at frame ${st.frames}\n${st.trace ?? ''}`);
        break;
      }
      if (st.frames >= nextStat) {
        nextStat += 30;
        note('runner', `frame ${st.frames}: wasm memory ${(st.memoryBytes / 1048576).toFixed(0)} MB, gfx_frame last ${st.gfx?.lastBytes} B, max ${st.gfx?.maxBytes} B, ${st.gfx?.frames} frames handed over; last replayed frame ${JSON.stringify(st.gfxReplay)}`);
        report.memory = st.memoryBytes;
      }
      if (st.frames >= nextShot) {
        const n = nextShot;
        nextShot += 60;
        const r = await savePng(page, path.join(SHOTS, `web_title_${n}.png`)).catch((e) => { note('runner', `snapshot failed: ${e.message}`); return null; });
        if (r) report.shots.push({ file: `work/shots/web_title_${n}.png`, frame: r.frames, size: `${r.width}x${r.height}`, colours: r.colours });
      }
      if (st.frames >= FRAMES) break;
      await sleep(100);
    }
    report.framesSeconds = (Date.now() - tFrames) / 1000;
    await Promise.race([Promise.all(atShots), sleep(20000)]);
    const r = await savePng(page, path.join(SHOTS, 'web_title_final.png')).catch((e) => { note('runner', `final snapshot failed: ${e.message}`); return null; });
    if (r) report.shots.push({ file: 'work/shots/web_title_final.png', frame: r.frames, size: `${r.width}x${r.height}`, colours: r.colours });
  }
  await pull().catch(() => {});
  report.audio = await page.evaluate(() => window.__twAudio ?? null).catch(() => null);
  report.error = firstError() ?? null;
} catch (e) {
  report.runnerError = String(e.stack ?? e);
  note('runner', 'ERROR ' + report.runnerError);
  if (page) await pull().catch(() => {});
} finally {
  if (context) await context.close().catch(() => {});
  if (server) server.proc.kill();
  fs.mkdirSync(path.dirname(LOG_FILE), { recursive: true });
  fs.writeFileSync(LOG_FILE, lines.join('\n') + '\n');
}

// summary
const s = (x) => (x ? (x.ms / 1000).toFixed(1) + ' s' : 'n/a');
console.log(`import: ${report.import ? `${s(report.import)} (${report.import.files} files, ${(report.import.bytes / 1048576).toFixed(0)} MB, ${report.import.copied} copied, ${report.import.skipped} already there)` : 'not finished'}`);
console.log(`compile + instantiate: ${s(report.loaded)}; _initialize: ${s(report['initialize-done'])}`);
{
  // the load timeline, from the page's navigation start (event times are the page's performance.now())
  const at = (name) => events.find((e) => e.event === name);
  const fmt = (e) => (e ? (e.t / 1000).toFixed(2) + ' s' : 'n/a');
  const ir = at('install-ready');
  const steady = at('steady');
  const comp = at('compiled');
  const titleScreen = at('title-reached') ?? at('title-screen');
  console.log(`timeline (${COLD ? 'cold' : 'warm'} start, ${PAGE} page): install manifest ${fmt(ir)}${ir ? ` (${(ir.cached / 1048576).toFixed(0)} of ${(ir.packSize / 1048576).toFixed(0)} MB cached)` : ''}; game.wasm fetched + compiled in ${comp ? (comp.ms / 1000).toFixed(2) + ' s' : 'n/a'}; tw_init done ${fmt(at('init-done'))}; first frame ${fmt(at('first-frame'))}; title screen ${fmt(titleScreen)}; steady (30 frames under 50 ms) ${fmt(steady)}${(titleScreen?.install ?? steady?.install) ? ((i) => `; files up to then: ${i.opens} opens, ${i.packOpens} from the cache, ${i.fetched} fetched alone (${(i.fetchedBytes / 1048576).toFixed(1)} MB), ${i.waited} waited ${i.waitMs.toFixed(0)} ms, wave bank ${i.bigBlocks} blocks fetched`)(titleScreen?.install ?? steady.install) : ''}`);
  const ic = at('install-cached');
  if (ic) console.log(`game files cached in this browser at ${fmt(ic)} (download ${(ic.stats.downloadMs / 1000).toFixed(1)} s)`);
  const title = [...events].reverse().find((e) => e.event === 'title');
  if (title) console.log(`window title: ${title.text}`);
  const order = at('open-order');
  if (order && WRITE_ORDER) {
    fs.writeFileSync(path.resolve(PROJECT, WRITE_ORDER), `# The order in which the game opens its files at start-up (up to the title), recorded by\n# node web/www/test/run-game-headless.mjs --host --page play --cold --record-order --write-order <this file>.\n# web/host/terraria_host.py puts these first in the pack.\n${order.paths.join('\n')}\n`);
    console.log(`open order: ${order.paths.length} paths -> ${WRITE_ORDER}`);
  }
}
if (report.requests) console.log(`requested: ${[...report.requests.keys()].map((k) => k.replace(/^https?:\/\/[^/]+/, '')).sort().join(' ')}`);
for (const c of report.canvasChecks ?? []) console.log(`checkcanvas ${c.name}: CSS ${c.css} at DPR ${c.dpr}, canvas ${c.canvas} (expected ${c.expectedCanvas}), back buffer ${c.back} (expected ${c.expectedBack}), ${c.colours} colours`);
if (report.titleCheck) console.log(`checktitle: document.title "${report.titleCheck.document}" = the game's window title "${report.titleCheck.game}"`);
for (const c of report.cursor ?? []) console.log(`cursorcheck ${c.name}: worst ${c.worst} px over ${c.points.length} points (${c.points[0].window} CSS px, DPR ${c.points[0].dpr}, canvas ${c.points[0].canvas}, back buffer ${c.points[0].back})`);
for (const b of report.budget ?? []) console.log(`frame budget ${b.label}: ${b.moduleMsPerFrame} ms of module time per frame, ${b.wallMsPerFrame} ms per display frame (${b.fps} fps) over ${b.frames} frames at ${b.back}`);
for (const m of report.memoryLog ?? []) console.log(`memory ${m.label}: frame ${m.frame}, wasm ${m.wasmMB} MB, module live ${m.liveMB} MB (tick ${m.liveTick}), ${m.textures} textures created, ${m.gfxLive ? `${m.gfxLive.count} alive (${m.gfxLive.mb} MB; ${m.gfxLive.targets} render targets, ${m.gfxLive.targetMb} MB)` : ''}, Worker JS heap ${m.jsHeapMB} MB, back ${m.back}, canvas ${m.canvas}`);
console.log(`tw_init: ${report.init ? `${s(report.init)}, returned ${report.init.rc}` : report.initTimeout ? 'did not return' : 'not reached'}`);
if (report.memory) console.log(`wasm memory at the last stats: ${(report.memory / 1048576).toFixed(0)} MB`);
if (report.diag) console.log(`diag: ${report.diag}`);
if (report.actionsDone) console.log(`actions done: ${report.actionsDone}${report.actionsError ? ` (stopped: ${report.actionsError})` : ''}`);
if (report.timing) console.log(`frame times: longest ${(report.timing.max / 1000).toFixed(1)} s (frame ${report.timing.maxFrame}); frames over 1 s: ${JSON.stringify(report.timing.slow)}`);
if (report.timing?.sliceMax !== undefined) console.log(`longest slice without a page tick: ${report.timing.sliceMax.toFixed(0)} ms (frame ${report.timing.sliceMaxFrame}; 0 = tw_init); main-stack yields ${report.timing.mainYields}; slices over 100 ms: ${report.timing.slicesOver100.length}${report.timing.slicesOver100.length ? ': ' + report.timing.slicesOver100.map((f) => `${f.frame}:${f.ms}`).join(' ') : ''}`);
if (report.timing?.slowImports?.length) console.log(`import calls of 40 ms or more: ${report.timing.slowImports.map((c) => `${c.frame}:${c.call}:${c.ms}`).join(' ')}`);
if (report.timing?.over100) console.log(`frames over 100 ms: ${report.timing.over100.length}${report.timing.over100.length ? ': ' + report.timing.over100.map((f) => `${f.frame}:${f.ms}${f.fiberMs ? `(fiber ${f.fiberMs})` : ''}`).join(' ') : ''}`);
if (report.timing?.workFrames) console.log(`frames with a work fiber running: ${report.timing.workFrames}, longest ${report.timing.workMax.toFixed(1)} ms (frame ${report.timing.workMaxFrame}), mean ${(report.timing.workTotal / report.timing.workFrames).toFixed(1)} ms; over 100 ms: ${JSON.stringify(report.timing.workSlow)}`);
for (const w of report.waits ?? []) console.log(`${w.name}: frames ${w.from} -> ${w.to} (${w.seconds.toFixed(1)} s)`);
for (const w of report.work ?? []) console.log(`waitwork ${w.name}: work from frame ${w.workFrom} to ${w.to} (${w.to - (w.workFrom ?? w.to)} frames, ${w.seconds.toFixed(1)} s); fibers ${JSON.stringify(w.fibers)}`);
console.log(`AudioWorklet played: ${report.audio ? `${report.audio.frames} frames (${(report.audio.frames / 44100).toFixed(1)} s), peak ${report.audio.peak} of 32767, RMS ${report.audio.rms}, ${report.audio.underrunFrames} underrun frames, context ${report.audio.contextState}` : 'nothing (no AudioContext)'}`);
const mixLine = [...lines].reverse().find((l) => l.includes('audio mixer:'));
if (mixLine) console.log(`module mixer (last report): ${mixLine.replace(/^.*?\[web\] /, '')}`);
console.log(`frames: ${report.frames}${report.framesSeconds ? ` in ${report.framesSeconds.toFixed(1)} s` : ''}${report.stall ? ` (stalled for ${report.stall.seconds} s)` : ''}`);
for (const sh of report.shots) console.log(`shot: ${sh.file} (frame ${sh.frame}, ${sh.size}, ${sh.colours} colours${sh.memoryMB != null ? `, wasm memory ${sh.memoryMB} MB` : ''}${sh.maxTex != null ? `, max texture id ${sh.maxTex}, ${sh.created} CreateTexture so far` : ''})`);
if (report.error) console.log(`first error / exit:\n${report.error.text ?? `exit code ${report.error.code}`}`);
if (report.runnerError) console.log('runner error: ' + report.runnerError);
console.log(`log: ${LOG_FILE} (${lines.length} lines)`);
process.exit(report.error || report.actionsError || report.runnerError || report.initTimeout || report.stall ? 1 : 0);

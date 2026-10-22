// Runs test/gfx-replay.html (and optionally the page itself) in a hidden headless Chromium with WebGPU.
//
//   node test/run-gfx-headless.mjs            replay test, SwiftShader WebGPU adapter and the default adapter
//   node test/run-gfx-headless.mjs --page     also open index.html?selftest and check the mock runs on gfx_frame
//
// Uses software that is already installed: playwright-core from the npx cache and Playwright's Chromium
// (full chrome.exe in new headless mode; the headless-shell build has no WebGPU adapter). Override with
// PLAYWRIGHT_CORE=<dir of playwright-core> and CHROME=<chrome.exe>. Nothing is downloaded. The browser and the
// local server are closed before the script exits.
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';

const WWW = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const LOCAL = process.env.LOCALAPPDATA ?? path.join(os.homedir(), 'AppData', 'Local');

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

const MIME = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript', '.mjs': 'text/javascript', '.wasm': 'application/wasm', '.json': 'application/json' };
function serve() {
  const server = http.createServer((req, res) => {
    const url = new URL(req.url, 'http://x');
    const file = path.normalize(path.join(WWW, decodeURIComponent(url.pathname === '/' ? '/index.html' : url.pathname)));
    res.setHeader('Cross-Origin-Opener-Policy', 'same-origin');
    res.setHeader('Cross-Origin-Embedder-Policy', 'require-corp');
    res.setHeader('Cross-Origin-Resource-Policy', 'same-origin');
    res.setHeader('Cache-Control', 'no-store');
    if (!file.startsWith(WWW) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) {
      res.statusCode = 404;
      return res.end('not found');
    }
    res.setHeader('Content-Type', MIME[path.extname(file)] ?? 'application/octet-stream');
    fs.createReadStream(file).pipe(res);
  });
  return new Promise((resolve) => server.listen(0, '127.0.0.1', () => resolve(server)));
}

const require = createRequire(findPlaywright() + path.sep);
const { chromium } = require('playwright-core');
const chrome = findChrome();
const wantPage = process.argv.includes('--page');
const server = await serve();
const base = `http://127.0.0.1:${server.address().port}`;
let failed = false;
const configs = [
  ['SwiftShader', ['--enable-unsafe-webgpu', '--use-webgpu-adapter=swiftshader']],
  ['default adapter', ['--enable-unsafe-webgpu']],
];
try {
  for (const [name, args] of configs) {
    const browser = await chromium.launch({ headless: true, executablePath: chrome, args });
    try {
      const page = await browser.newPage();
      const consoleErrors = [];
      page.on('console', (m) => m.type() === 'error' && consoleErrors.push(m.text()));
      page.on('pageerror', (e) => consoleErrors.push(String(e)));
      await page.goto(`${base}/test/gfx-replay.html`);
      await page.waitForFunction(() => window.__gfxResult, null, { timeout: 60000 });
      const r = await page.evaluate(() => window.__gfxResult);
      console.log(`\n== gfx replay, ${name} (${r.adapter ?? 'no adapter'}), canvas format ${r.canvasFormat}`);
      if (r.error) console.log('ERROR', r.error);
      for (const k of ['frame1', 'frame2', 'frame3']) if (r[k]) console.log(`${r[k].label}: ${r[k].mismatches} mismatching pixels of 256${r[k].mismatches ? ' ' + JSON.stringify(r[k].first) : ''}`);
      if (r.present) console.log(`presentation (canvas vs back buffer): ${r.present.mismatches} mismatching pixels`);
      console.log('steps', JSON.stringify(r.steps), 'samples', JSON.stringify(r.sample));
      if (r.errors?.length) console.log('WebGPU / replayer errors:', r.errors);
      if (consoleErrors.length) console.log('console errors:', consoleErrors);
      console.log(r.ok ? 'PASS' : 'FAIL');
      if (!r.ok) failed = true;

      if (wantPage && name === 'SwiftShader') {
        const p2 = await browser.newPage();
        const errs = [];
        p2.on('pageerror', (e) => errs.push(String(e)));
        await p2.goto(`${base}/?selftest&mock`);
        await p2.waitForFunction(() => /running|error|WebGPU|stopped/i.test(document.getElementById('status').textContent), null, { timeout: 60000 });
        await p2.waitForTimeout(1500);
        await p2.click('canvas');
        await p2.keyboard.press('a');
        await p2.waitForTimeout(800);
        const info = await p2.evaluate(() => ({ status: document.getElementById('status').textContent, selftest: document.documentElement.dataset.selftest, log: document.getElementById('log').innerText }));
        console.log(`\n== page ?selftest: status "${info.status}", selftest ${info.selftest}`);
        console.log(info.log.split('\n').map((l) => '  ' + l).join('\n'));
        if (errs.length) console.log('page errors:', errs);
        const shot = process.env.SCREENSHOT;
        if (shot) {
          await p2.screenshot({ path: shot });
          console.log('screenshot:', shot);
        }
        // saves: import a .plr through the file input, see it listed, download it back unchanged
        const plr = Buffer.from([1, 2, 3, 4, 5, 250, 251, 252]);
        await p2.click('#btn-saves');
        await p2.setInputFiles('#saves-file', { name: 'Headless Hero.plr', mimeType: 'application/octet-stream', buffer: plr });
        await p2.waitForFunction(() => /Imported/.test(document.getElementById('saves-status').textContent), null, { timeout: 15000 });
        const listed = await p2.evaluate(() => document.getElementById('saves-list').innerText);
        const [download] = await Promise.all([p2.waitForEvent('download'), p2.click('.save-row button')]);
        const got = fs.readFileSync(await download.path());
        const savesOk = /Players\/Headless Hero\.plr/.test(listed) && Buffer.compare(got, plr) === 0 && download.suggestedFilename() === 'Headless Hero.plr';
        console.log(`saves: listed "${listed.replace(/\s+/g, ' ').trim()}", download ${download.suggestedFilename()} ${got.length} bytes ${Buffer.compare(got, plr) === 0 ? 'equal' : 'DIFFERENT'}: ${savesOk ? 'ok' : 'FAIL'}`);
        await p2.click('#btn-saves-close');
        const pageOk = savesOk && info.selftest === 'pass' && /running/.test(info.status) && !/WebGPU:|error/i.test(info.log) && errs.length === 0;
        console.log(pageOk ? 'PAGE PASS' : 'PAGE FAIL');
        if (!pageOk) failed = true;
      }
    } finally {
      await browser.close();
    }
  }
} finally {
  server.close();
}
process.exit(failed ? 1 : 0);

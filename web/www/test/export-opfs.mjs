// Copies files out of the headless runner's browser profile (its OPFS, origin http://127.0.0.1:8147): a world or
// player the game saved in a scripted run. Hidden headless Chromium, no server (the origin's page is served by a
// route), the profile under <project>/work/web-profile.
//   node test/export-opfs.mjs --list [<folder>]
//   node test/export-opfs.mjs <opfs path> <out file>     e.g. saves-run1791912544244/Worlds/webworld.wld work/x.wld
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';

const WWW = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const PROJECT = path.resolve(WWW, '..', '..');
const LOCAL = process.env.LOCALAPPDATA ?? path.join(os.homedir(), 'AppData', 'Local');
const PROFILE = path.join(PROJECT, 'work', 'web-profile');
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
const [a, b] = process.argv.slice(2);
const require = createRequire(findPlaywright() + path.sep);
const { chromium } = require('playwright-core');
const context = await chromium.launchPersistentContext(PROFILE, { headless: true, executablePath: findChrome() });
try {
  const page = context.pages()[0] ?? (await context.newPage());
  await page.route('http://127.0.0.1:8147/__opfs_export', (r) => r.fulfill({ body: '<!doctype html><title>export</title>', contentType: 'text/html' }));
  await page.goto('http://127.0.0.1:8147/__opfs_export');
  if (a === '--list') {
    const names = await page.evaluate(async (p) => {
      let d = await navigator.storage.getDirectory();
      for (const part of (p ?? '').split('/').filter(Boolean)) d = await d.getDirectoryHandle(part);
      const out = [];
      for await (const [n, h] of d.entries()) out.push(h.kind === 'file' ? `${n} ${(await h.getFile()).size}` : `${n}/`);
      return out.sort();
    }, b);
    console.log(names.join('\n'));
  } else {
    const b64 = await page.evaluate(async (p) => {
      const parts = p.split('/').filter(Boolean);
      let d = await navigator.storage.getDirectory();
      for (const part of parts.slice(0, -1)) d = await d.getDirectoryHandle(part);
      const bytes = new Uint8Array(await (await (await d.getFileHandle(parts.at(-1))).getFile()).arrayBuffer());
      let bin = '';
      for (let i = 0; i < bytes.length; i += 0x8000) bin += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
      return btoa(bin);
    }, a);
    fs.mkdirSync(path.dirname(path.resolve(b)), { recursive: true });
    fs.writeFileSync(b, Buffer.from(b64, 'base64'));
    console.log(`${a} -> ${b} (${fs.statSync(b).size} bytes)`);
  }
} finally {
  await context.close();
}

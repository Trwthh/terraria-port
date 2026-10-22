// lib/remote-install.js and vfs.js over it: /install from the host server's manifest, the pack download into OPFS,
// opens before / during / after it, the big file's block cache, resume and eviction. A fake server and a fake OPFS.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { FakeDir } from './fake-opfs.mjs';
import { createRemoteInstall } from '../lib/remote-install.js';
import { createVfs } from '../vfs.js';
import { ERRNO } from '../lib/paths.js';

const BASE = 'http://terraria.localhost:7790/';
const bytesOf = (n, seed) => Uint8Array.from({ length: n }, (_, i) => (i * 7 + seed) & 255);

/** A fake terraria_host.py: three packed files and one big file (2.5 blocks). */
function fakeServer() {
  const small = [
    { p: 'Terraria.exe', b: bytesOf(5000, 1) },
    { p: 'Content/Fonts/Mouse_Text.xnb', b: bytesOf(300, 2) },
    { p: 'Content/Images/Item_1.xnb', b: bytesOf(70000, 3) },
  ];
  const big = { p: 'Content/Wave Bank.xwb', b: bytesOf((5 << 20) / 2, 4), h: 'b'.repeat(32) };
  let off = 0;
  const files = small.map((f, i) => {
    const e = { p: f.p, s: f.b.length, m: 1000 + i, h: String(i).repeat(32), o: off };
    off += f.b.length;
    return e;
  });
  files.push({ p: big.p, s: big.b.length, m: 2000, h: big.h, o: -1 });
  const pack = new Uint8Array(off);
  small.forEach((f, i) => pack.set(f.b, files[i].o));
  const id = 'c'.repeat(32);
  const log = [];
  const range = (h, size) => {
    const m = /bytes=(\d+)-(\d*)/.exec(h ?? '');
    return m ? [Number(m[1]), m[2] ? Number(m[2]) + 1 : size] : null;
  };
  const server = {
    small, big, pack, files, log, id, chunk: 1024,
    manifest: { version: 1, pack: { id, size: pack.length, url: `install/pack/${id}`, files: small.length }, files },
    async fetch(u, init = {}) {
      const path = new URL(u).pathname;
      const r = range(init.headers?.Range, path.includes('/pack/') ? pack.length : big.b.length);
      log.push({ path, range: r });
      if (path === '/install/manifest.json') return new Response(JSON.stringify(server.manifest), { status: 200 });
      const src = path === `/install/pack/${id}` ? pack : path === `/install/file/${big.h}` ? big.b : null;
      if (!src) return new Response('no', { status: 404 });
      const [a, b] = r ?? [0, src.length];
      const body = src.slice(a, b);
      // streamed in small chunks, as a real response body is
      const stream = new ReadableStream({
        start(ctl) {
          for (let i = 0; i < body.length; i += server.chunk) ctl.enqueue(body.slice(i, i + server.chunk));
          ctl.close();
        },
      });
      return new Response(stream, { status: r ? 206 : 200 });
    },
    syncCalls: [],
    syncRange(u, a, b) {
      server.syncCalls.push([a, b]);
      assert.equal(new URL(u).pathname, `/install/file/${big.h}`);
      return big.b.slice(a, b);
    },
  };
  return server;
}

const make = (server, opfs, role = 'game') => createRemoteInstall({ base: BASE, opfsRoot: opfs, role, fetch: server.fetch, syncRange: server.syncRange, recordOrder: true });

async function readAll(vfs, path) {
  let o = vfs.openFile(path.split('/').filter(Boolean), {});
  const sync = !(o && typeof o.then === 'function');
  o = await o;
  assert.notEqual(typeof o, 'number', `open ${path}: errno ${-o}`);
  const out = new Uint8Array(o.s.access.getSize());
  // read in odd pieces, as the game does
  for (let at = 0; at < out.length; ) at += o.s.access.read(out.subarray(at, at + 777), { at });
  vfs.release(o.node, o.s, false);
  return { bytes: out, sync };
}

test('the /install tree comes from the manifest: stat and listing without any open', async () => {
  const server = fakeServer();
  const opfs = new FakeDir('root');
  const inst = make(server, opfs);
  await inst.init();
  const vfs = createVfs(opfs, { install: inst, channel: null });
  const idx = await vfs.init();
  assert.ok(idx.entries >= 7);
  const content = vfs.resolve(['install', 'content']); // case-insensitive, as before
  assert.equal(content.kind, 'dir');
  assert.deepEqual(vfs.list(content).map((e) => e.name), ['Fonts', 'Images', 'Wave Bank.xwb']);
  const st = vfs.fileStat(vfs.resolve(['install', 'Content', 'Images', 'Item_1.xnb']));
  assert.deepEqual(st, { size: 70000, mtime: 1002 });
  assert.equal(server.log.filter((l) => l.path !== '/install/manifest.json').length, 0);
  // the OPFS "install" folder of the import is neither read nor created
  assert.equal(opfs.map.has('install'), false);
  // the write guard still refuses /install
  assert.equal(vfs.openFile(['install', 'Terraria.exe'], { write: true }), -ERRNO.EACCES);
});

test('before the download a file is fetched by itself; after it opens are synchronous views of the pack', async () => {
  const server = fakeServer();
  const opfs = new FakeDir('root');
  const inst = make(server, opfs);
  await inst.init();
  const vfs = createVfs(opfs, { install: inst, channel: null });
  await vfs.init();
  const a = await readAll(vfs, '/install/Content/Images/Item_1.xnb');
  assert.deepEqual(a.bytes, server.small[2].b);
  assert.equal(a.sync, false);
  assert.equal(inst.stats.fetched, 1);
  assert.deepEqual(server.log.at(-1).range, [5300, 75300]);

  await inst.startDownload();
  assert.equal(inst.complete, true);
  for (const [i, f] of server.small.entries()) {
    const r = await readAll(vfs, '/install/' + f.p);
    assert.equal(r.sync, true, f.p);
    assert.deepEqual(r.bytes, server.small[i].b, f.p);
  }
  assert.equal(inst.stats.fetched, 1); // no more single fetches
  // progress saved; the big file was filled in the background too (blocks 0..2)
  const mark = JSON.parse(new TextDecoder().decode(opfs.map.get('tw-cache').map.get(`pack-${server.id}.json`).bytes));
  assert.equal(mark.done, server.pack.length);
  assert.equal(inst.stats.bigBlocks, 3);
  assert.deepEqual(inst.order.slice(0, 1), ['Content/Images/Item_1.xnb']);
});

test('an open just beyond the download waits for it instead of fetching', async () => {
  const server = fakeServer();
  server.chunk = 512;
  const opfs = new FakeDir('root');
  const inst = make(server, opfs);
  await inst.init();
  const vfs = createVfs(opfs, { install: inst, channel: null });
  await vfs.init();
  const dl = inst.startDownload();
  const r = await readAll(vfs, '/install/Content/Fonts/Mouse_Text.xnb');
  assert.deepEqual(r.bytes, server.small[1].b);
  assert.equal(inst.stats.fetched, 0);
  await dl;
});

test('second start: the pack is complete, nothing is downloaded, the big file needs no request', async () => {
  const server = fakeServer();
  const opfs = new FakeDir('root');
  const first = make(server, opfs);
  await first.init();
  await first.startDownload();
  const n = server.log.length;
  server.syncCalls.length = 0;

  const inst = make(server, opfs);
  const r = await inst.init();
  assert.equal(r.cached, server.pack.length);
  const vfs = createVfs(opfs, { install: inst, channel: null });
  await vfs.init();
  await inst.startDownload();
  assert.equal(server.log.length, n + 1); // only the manifest
  const exe = await readAll(vfs, '/install/Terraria.exe');
  assert.equal(exe.sync, true);
  assert.deepEqual(exe.bytes, server.small[0].b);
  const wb = await readAll(vfs, '/install/Content/Wave Bank.xwb');
  assert.deepEqual(wb.bytes, server.big.b);
  assert.equal(server.syncCalls.length, 0);
});

test('the big file: missing blocks are read synchronously, once, and kept', async () => {
  const server = fakeServer();
  const opfs = new FakeDir('root');
  const inst = make(server, opfs);
  await inst.init();
  const vfs = createVfs(opfs, { install: inst, channel: null });
  await vfs.init();
  const o = await vfs.openFile(['install', 'Content', 'Wave Bank.xwb'], {});
  const B = 1 << 20;
  const buf = new Uint8Array(B);
  assert.equal(o.s.access.read(buf, { at: B - 100 }), B); // blocks 0 and 1: one request
  assert.deepEqual(buf, server.big.b.slice(B - 100, 2 * B - 100));
  assert.deepEqual(server.syncCalls, [[0, 2 * B]]);
  o.s.access.read(buf.subarray(0, 10), { at: 5 });
  assert.equal(server.syncCalls.length, 1);
  assert.equal(o.s.access.read(buf, { at: server.big.b.length - 10 }), 10); // block 2 (short, at the end)
  assert.deepEqual(server.syncCalls[1], [2 * B, server.big.b.length]);
  assert.equal(o.s.access.read(buf, { at: server.big.b.length }), 0);
  vfs.release(o.node, o.s, false);

  // a new instance finds the blocks in OPFS
  server.syncCalls.length = 0;
  const again = make(server, opfs);
  await again.init();
  const v2 = createVfs(opfs, { install: again, channel: null });
  await v2.init();
  const all = await readAll(v2, '/install/Content/Wave Bank.xwb');
  assert.deepEqual(all.bytes, server.big.b);
  assert.equal(server.syncCalls.length, 0);
});

test('an interrupted download resumes where it stopped; files of an older install are removed', async () => {
  const server = fakeServer();
  const opfs = new FakeDir('root');
  const cache = await opfs.getDirectoryHandle('tw-cache', { create: true });
  // 4,000 bytes of the pack were written and recorded; an old pack is lying around
  const f = await cache.getFileHandle(`pack-${server.id}.bin`, { create: true });
  f.bytes = server.pack.slice(0, 4000);
  cache.addFile(`pack-${server.id}.json`, JSON.stringify({ done: 4000 }));
  cache.addFile(`pack-${'d'.repeat(32)}.bin`, 'old');
  const inst = make(server, opfs);
  const r = await inst.init();
  assert.equal(r.cached, 4000);
  await inst.startDownload();
  const packReq = server.log.find((l) => l.path.includes('/pack/'));
  assert.deepEqual(packReq.range, [4000, server.pack.length]);
  assert.deepEqual(f.bytes, server.pack);
  assert.equal(cache.map.has(`pack-${'d'.repeat(32)}.bin`), false);
});

test('the server Worker reads what the game Worker downloaded (shared handles), without downloading', async () => {
  const server = fakeServer();
  const opfs = new FakeDir('root');
  const game = make(server, opfs, 'game');
  await game.init();
  const srv = make(server, opfs, 'server');
  await srv.init(); // both hold the pack open ('readwrite-unsafe')
  await game.startDownload();
  assert.equal(srv.startDownload(), null);
  const v = createVfs(opfs, { install: srv, channel: null });
  await v.init();
  const r = await readAll(v, '/install/Content/Images/Item_1.xnb');
  assert.deepEqual(r.bytes, server.small[2].b);
  assert.equal(srv.stats.fetched, 0); // it re-read the progress mark and found the file in the pack
});

test('with a download Worker: progress through the shared counter, opens become synchronous views', async () => {
  const server = fakeServer();
  const opfs = new FakeDir('root');
  let posted = null;
  const inst = createRemoteInstall({
    base: BASE, opfsRoot: opfs, role: 'game', fetch: server.fetch, syncRange: server.syncRange,
    // a stand-in for install-worker.js: writes the whole pack through its own handle, then reports
    startHelper: (params) => {
      posted = params;
      const w = { onmessage: null, terminate() {} };
      (async () => {
        const cache = await opfs.getDirectoryHandle('tw-cache');
        const a = await (await cache.getFileHandle(`pack-${params.packId}.bin`)).createSyncAccessHandle({ mode: 'readwrite-unsafe' });
        a.write(server.pack, { at: 0 });
        a.close();
        Atomics.store(params.progress, 0, BigInt(server.pack.length));
        Atomics.store(params.progress, 1, 1n);
        w.onmessage({ data: { type: 'progress', done: server.pack.length, total: server.pack.length } });
        w.onmessage({ data: { type: 'done', ms: 1 } });
      })();
      return w;
    },
  });
  await inst.init();
  const vfs = createVfs(opfs, { install: inst, channel: null });
  await vfs.init();
  await inst.startDownload();
  assert.equal(posted.packSize, server.pack.length);
  assert.equal(posted.bigs.length, 1);
  const r = await readAll(vfs, '/install/Content/Images/Item_1.xnb');
  assert.equal(r.sync, true);
  assert.deepEqual(r.bytes, server.small[2].b);
  assert.equal(inst.stats.fetched, 0);
});

// host.js (`tw` imports) over vfs.js against an in-memory stand-in for OPFS (test/fake-opfs.mjs).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createHost, SUSPENDING } from '../host.js';
import { createVfs } from '../vfs.js';
import { FakeDir } from './fake-opfs.mjs';
import { ERRNO } from '../lib/paths.js';
import { encodeKey, encodeMouseMove, decodeAll, REC, KIND } from '../lib/input-codec.js';

async function setup() {
  const root = new FakeDir('');
  const install = await root.getDirectoryHandle('install', { create: true });
  const content = await install.getDirectoryHandle('Content', { create: true });
  const images = await content.getDirectoryHandle('Images', { create: true });
  await content.getDirectoryHandle('Fonts', { create: true });
  images.addFile('Item_1.xnb', 'XNBw\x05fake');
  content.addFile('Wave Bank.xwb', 'WBND' + 'x'.repeat(100));
  const posted = [];
  const vfs = createVfs(root);
  const idx = await vfs.init();
  const host = createHost({ vfs, post: (m) => posted.push(m) });
  const memory = new WebAssembly.Memory({ initial: 2 });
  host.setMemory(memory);
  const tw = host.imports.tw;
  const u8 = () => new Uint8Array(memory.buffer);
  let strTop = 0;
  // put a string into memory, return [ptr, len]
  const s = (str) => {
    const b = new TextEncoder().encode(str);
    const ptr = 65536 + (strTop % 16) * 512; // second page: clear of the data areas below 64 KiB
    strTop++;
    u8().set(b, ptr);
    return [ptr, b.length];
  };
  const text = (ptr, len) => new TextDecoder().decode(u8().slice(ptr, ptr + len));
  return { root, install, content, images, host, tw, memory, u8, s, text, posted, idx, vfs };
}

test('every tw import exists; the suspending set is as documented', async () => {
  const { tw } = await setup();
  const names = ['fs_open', 'fs_size', 'fs_read', 'fs_write', 'fs_close', 'fs_stat', 'fs_list', 'fs_mkdir', 'fs_remove',
    'fs_rename', 'now_ms', 'wall_ms', 'random', 'log', 'exit', 'cpu_count', 'clipboard_get', 'clipboard_set', 'input_next',
    'audio_push', 'audio_free', 'gfx_frame', 'gfx_frame_chunk', 'gfx_texture', 'yield_frame', 'window_title', 'frame_info'];
  assert.deepEqual(Object.keys(tw).sort(), [...names].sort());
  assert.deepEqual([...SUSPENDING].sort(), ['fs_mkdir', 'fs_open', 'fs_remove', 'fs_rename', 'yield_frame']);
});

test('window_title and frame_info: the title goes to the page, the mouse is kept, flag changes are posted once', async () => {
  const { tw, host, s, posted } = await setup();
  tw.window_title(...s('Terraria: Dig Peon, Dig!'));
  assert.deepEqual(posted.at(-1), { type: 'title', text: 'Terraria: Dig Peon, Dig!' });
  const n = posted.length;
  tw.frame_info(10, 20, 2 | 4);
  tw.frame_info(11, 21, 2 | 4);
  assert.equal(posted.length, n + 1);
  assert.deepEqual(posted.at(-1), { type: 'cursor', visible: false, menu: true, splash: true });
  assert.equal(host.windowInfo.mouseX, 11);
  assert.equal(host.windowInfo.mouseY, 21);
  tw.frame_info(11, 21, 2);
  assert.deepEqual(posted.at(-1), { type: 'cursor', visible: false, menu: true, splash: false });
});

test('index, stat and list (sorted, dirs with a slash, needed-size answer)', async () => {
  const { tw, s, text, idx } = await setup();
  assert.equal(idx.entries, 5);
  assert.equal(tw.fs_stat(...s('/install/Content')), 1);
  assert.equal(tw.fs_stat(...s('/install/Content/Images/Item_1.xnb')), 0);
  assert.equal(tw.fs_stat(...s('/install/content/IMAGES/item_1.XNB')), 0, 'case-insensitive fallback');
  assert.equal(tw.fs_stat(...s('/install/nope')), -1);
  assert.equal(tw.fs_stat(...s('/elsewhere')), -1);
  assert.equal(tw.fs_stat(...s('/')), 1);

  const out = 8192;
  let n = tw.fs_list(...s('/install/Content'), out, 4096);
  assert.equal(text(out, n), 'Fonts/\nImages/\nWave Bank.xwb');
  n = tw.fs_list(...s('/install/Content'), out, 5);
  assert.equal(n, -256, 'too small: -max(needed, 256)');
  n = tw.fs_list(...s('/'), out, 4096);
  assert.equal(text(out, n), 'install/\nsaves/');
  n = tw.fs_list(...s('/install/Content/Fonts'), out, 4096);
  assert.equal(n, 0, 'empty directory');
  assert.equal(tw.fs_list(...s('/install/missing'), out, 4096), -ERRNO.ENOENT);
  assert.equal(tw.fs_list(...s('/install/Content/Wave Bank.xwb'), out, 4096), -ERRNO.ENOTDIR);
});

test('read: open (suspends the first time, not the second), size, read at offset, close', async () => {
  const { tw, s, text } = await setup();
  const r1 = tw.fs_open(...s('/install/Content/Images/Item_1.xnb'), 0);
  assert.ok(r1 instanceof Promise, 'first open of a file needs createSyncAccessHandle: a Promise');
  const h = await r1;
  assert.ok(h > 0);
  const r2 = tw.fs_open(...s('/install/Content/Images/Item_1.xnb'), 0);
  assert.equal(typeof r2, 'number', 'second open shares the handle: no suspension');
  assert.equal(tw.fs_size(h), 9);
  assert.equal(tw.fs_read(h, 0, 4096, 4), 4);
  assert.equal(text(4096, 4), 'XNBw');
  assert.equal(tw.fs_read(h, 8, 4096, 100), 1, 'short read at the end');
  assert.equal(tw.fs_read(h, 100, 4096, 10), 0, 'read past the end');
  assert.equal(tw.fs_read(h, -1, 4096, 10), -ERRNO.EINVAL);
  assert.equal(tw.fs_read(h, 0, 0x7fff0000, 10), -ERRNO.EFAULT, 'out of wasm memory');
  assert.equal(tw.fs_write(h, 0, 4096, 1), -ERRNO.EBADF, 'read handle');
  assert.equal(tw.fs_close(h), 0);
  assert.equal(tw.fs_close(r2), 0);
  assert.equal(tw.fs_close(h), -ERRNO.EBADF);
  assert.equal(tw.fs_size(h), -ERRNO.EBADF);
  assert.equal(await tw.fs_open(...s('/install/Content/nope.xnb'), 0), -ERRNO.ENOENT);
  assert.equal(await tw.fs_open(...s('/install/Content'), 0), -ERRNO.EISDIR);
  assert.equal(await tw.fs_open(...s('/install/Content/Images/Item_1.xnb'), 7), -ERRNO.EINVAL);
});

test('/install is read-only for every write path', async () => {
  const { tw, s, root } = await setup();
  assert.equal(await tw.fs_open(...s('/install/Content/Images/Item_1.xnb'), 1), -ERRNO.EACCES);
  assert.equal(await tw.fs_open(...s('/install/Content/new.txt'), 2), -ERRNO.EACCES);
  assert.equal(await tw.fs_mkdir(...s('/install/Content/NewDir')), -ERRNO.EACCES);
  assert.equal(await tw.fs_remove(...s('/install/Content/Wave Bank.xwb')), -ERRNO.EACCES);
  assert.equal(await tw.fs_rename(...s('/install/Content/Wave Bank.xwb'), ...s('/saves/x.xwb')), -ERRNO.EACCES);
  assert.equal(await tw.fs_rename(...s('/saves/a'), ...s('/install/a')), -ERRNO.EACCES);
  assert.equal(await tw.fs_remove(...s('/saves')), -ERRNO.EACCES);
  assert.equal(await tw.fs_open(...s('/saves/../install/Content/x'), 1), -ERRNO.EINVAL);
  const content = root.map.get('install').map.get('Content');
  assert.deepEqual([...content.map.keys()].sort(), ['Fonts', 'Images', 'Wave Bank.xwb'], 'nothing changed');
});

test('saves: mkdir, write (truncate), append, read back, rename over, remove', async () => {
  const { tw, s, text, u8, root } = await setup();
  assert.equal(await tw.fs_mkdir(...s('/saves/Players/Backups')), 0, 'recursive');
  assert.equal(await tw.fs_mkdir(...s('/saves/Players')), 0, 'existing is fine');
  assert.equal(tw.fs_stat(...s('/saves/Players/Backups')), 1);
  assert.equal(await tw.fs_open(...s('/saves/NoDir/a.plr'), 1), -ERRNO.ENOENT, 'parent must exist');

  const w = await tw.fs_open(...s('/saves/Players/a.plr'), 1);
  assert.ok(w > 0);
  u8().set(new TextEncoder().encode('hello world'), 4096);
  assert.equal(tw.fs_write(w, 0, 4096, 11), 11);
  assert.equal(tw.fs_write(w, 6, 4096, 5), 5); // "hello hello"
  assert.equal(tw.fs_close(w), 0);

  const a = await tw.fs_open(...s('/saves/Players/a.plr'), 2);
  u8().set(new TextEncoder().encode('!!'), 4096);
  assert.equal(tw.fs_write(a, 0, 4096, 2), 2, 'append ignores the offset');
  tw.fs_close(a);

  const r = await tw.fs_open(...s('/saves/Players/a.plr'), 0);
  const size = tw.fs_size(r);
  assert.equal(size, 13);
  tw.fs_read(r, 0, 8192, size);
  assert.equal(text(8192, size), 'hello hello!!');
  assert.equal(await tw.fs_remove(...s('/saves/Players/a.plr')), -ERRNO.EBUSY, 'open file');
  assert.equal(await tw.fs_rename(...s('/saves/Players/a.plr'), ...s('/saves/Players/b.plr')), -ERRNO.EBUSY);
  tw.fs_close(r);

  const t = await tw.fs_open(...s('/saves/Players/a.plr'), 1); // truncates
  assert.equal(tw.fs_size(t), 0);
  tw.fs_close(t);

  // rename over an existing file
  const b = await tw.fs_open(...s('/saves/Players/b.plr'), 1);
  tw.fs_write(b, 0, 4096, 2);
  tw.fs_close(b);
  assert.equal(await tw.fs_rename(...s('/saves/Players/b.plr'), ...s('/saves/Players/a.plr')), 0);
  assert.equal(tw.fs_stat(...s('/saves/Players/b.plr')), -1);
  assert.equal(tw.fs_stat(...s('/saves/Players/a.plr')), 0);
  const players = root.map.get('saves').map.get('Players');
  assert.deepEqual([...players.map.keys()].sort(), ['Backups', 'a.plr']);
  assert.equal(new TextDecoder().decode(players.map.get('a.plr').bytes), '!!');
  assert.equal(await tw.fs_rename(...s('/saves/Players/Backups'), ...s('/saves/B')), -ERRNO.ENOTSUP);
  assert.equal(await tw.fs_rename(...s('/saves/Players/a.plr'), ...s('/saves/Players/Backups')), -ERRNO.EISDIR);

  assert.equal(await tw.fs_remove(...s('/saves/Players')), -ERRNO.ENOTEMPTY);
  assert.equal(await tw.fs_remove(...s('/saves/Players/Backups')), 0);
  assert.equal(await tw.fs_remove(...s('/saves/Players/a.plr')), 0);
  assert.equal(await tw.fs_remove(...s('/saves/Players/a.plr')), -ERRNO.ENOENT);
  assert.equal(players.map.size, 0);
});

test('input_next drains whole records; log, random, exit, audio before start', async () => {
  const { tw, host, s, u8, posted } = await setup();
  const t0 = performance.timeOrigin + performance.now();
  const buf = new Uint8Array(REC * 3);
  buf.set(encodeKey({ down: true, code: 'KeyA', key: 'a', time: t0 }), 0);
  buf.set(encodeMouseMove({ x: 1, y: 2, time: t0 }), REC);
  buf.set(encodeMouseMove({ x: 3, y: 4, time: t0 }), REC * 2);
  host.pushInput(buf.buffer);
  assert.equal(tw.input_next(16384, 40), REC, 'one record fits in 40 bytes');
  assert.equal(tw.input_next(16384, 10), -ERRNO.EINVAL, 'records waiting, no room for one');
  const n = tw.input_next(16384, 1024);
  assert.equal(n, REC, 'the two moves were coalesced');
  const [mv] = decodeAll(u8().slice(16384, 16384 + n));
  assert.equal(mv.kind, KIND.MOUSE_MOVE);
  assert.equal(mv.x, 3);
  assert.ok(Math.abs(mv.time - performance.now()) < 5000, 'time rebased to the worker clock');
  assert.equal(tw.input_next(16384, 1024), 0);

  tw.log(3, ...s('a warning'));
  assert.deepEqual(posted.at(-1), { type: 'log', level: 3, text: 'a warning' });
  u8().fill(0, 20000, 20064);
  assert.equal(tw.random(20000, 64), 0);
  assert.ok(u8().slice(20000, 20064).some((b) => b !== 0));
  // before audio starts: accepted and dropped, paced at the real-time rate
  const { TARGET_FRAMES } = await import('../lib/audio-ring.js');
  assert.equal(tw.audio_free(), TARGET_FRAMES, 'audio not started: the virtual buffer is empty');
  assert.equal(tw.audio_push(20000, 100), 100);
  const free = tw.audio_free();
  assert.ok(free <= TARGET_FRAMES - 90 && free >= TARGET_FRAMES - 100, `virtual buffer holds the pushed frames (free ${free})`);
  assert.equal(tw.audio_push(20001, 4), -ERRNO.EINVAL, 'unaligned');
  assert.equal(typeof tw.now_ms(), 'number');
  assert.ok(Math.abs(tw.wall_ms() - Date.now()) < 1000);
  tw.exit(3);
  assert.equal(host.exitCode, 3);
  assert.deepEqual(posted.at(-1), { type: 'exit', code: 3 });
});

test('audio through a ring, and yield_frame resumes on the next tick', async () => {
  const { tw, host, u8 } = await setup();
  const { createRing, ringViews, ringAvailable, TARGET_FRAMES } = await import('../lib/audio-ring.js');
  const sab = createRing(8192, ArrayBuffer);
  host.attachAudio({ port: { postMessage() {} }, sab });
  assert.equal(tw.audio_free(), TARGET_FRAMES);
  const f = new Float32Array(u8().buffer, 32768, 200);
  f.fill(0.5);
  assert.equal(tw.audio_push(32768, 100), 100);
  assert.equal(ringAvailable(ringViews(sab)), 100);
  assert.equal(tw.audio_free(), TARGET_FRAMES - 100);

  let resumed = null;
  tw.yield_frame().then((t) => (resumed = t));
  assert.equal(host.resumeYield(123.5), true);
  await Promise.resolve();
  assert.equal(resumed, 123.5);
  assert.equal(host.resumeYield(1), false);
});

test('cpu_count, clipboard_get / clipboard_set, gfx_frame', async () => {
  const { tw, host, s, text, posted, u8 } = await setup();
  const n = tw.cpu_count();
  assert.ok(Number.isInteger(n) && n >= 1);
  assert.equal(tw.clipboard_get(4096, 100), 0, 'empty at first');
  host.setClipboardText('pasted text');
  assert.equal(tw.clipboard_get(4096, 4), -256, 'too small: -max(needed, 256)');
  const len = tw.clipboard_get(4096, 100);
  assert.equal(text(4096, len), 'pasted text');
  assert.equal(tw.clipboard_set(...s('copied')), 0);
  assert.deepEqual(posted.at(-1), { type: 'clipboard-set', text: 'copied' });
  assert.equal(text(4096, tw.clipboard_get(4096, 100)), 'copied');

  assert.equal(tw.gfx_frame(4096, 8), -ERRNO.ENOSYS, 'no replayer yet');
  let got = null;
  host.setGfx({ frame: (bytes) => (got = bytes.slice()) });
  u8().set([84, 87, 70, 49], 4096);
  assert.equal(tw.gfx_frame(4096, 4), 0);
  assert.deepEqual([...got], [84, 87, 70, 49], 'a frame of one part arrives as one Uint8Array');

  // v2: chunks are copied out of wasm memory, then gfx_frame appends the last part and hands over the list
  let parts = null;
  host.setGfx({ frame: (p) => (parts = Array.isArray(p) ? p.map((x) => [...x]) : null) });
  u8().set([1, 2, 3], 4096);
  assert.equal(tw.gfx_frame_chunk(4096, 3), 0);
  u8().set([9, 9, 9], 4096); // the module reuses its buffer: the copy must not change
  assert.equal(tw.gfx_frame_chunk(4096, 2), 0);
  u8().set([7], 4096);
  assert.equal(tw.gfx_frame(4096, 1), 0);
  assert.deepEqual(parts, [[1, 2, 3], [9, 9], [7]]);
  assert.equal(host.gfxStats.lastChunks, 2);
  assert.equal(host.gfxStats.lastBytes, 6);
  assert.equal(tw.gfx_frame_chunk(0x7fffff00, 1000), -ERRNO.EFAULT);

  // gfx_texture: handed to the replayer at once, with the bytes as they are now
  let pre = null;
  host.setGfx({ frame() {}, preload: (id, w, h, rgba) => (pre = { id, w, h, rgba: [...rgba] }) });
  u8().set([1, 2, 3, 4, 5, 6, 7, 8], 4096);
  assert.equal(tw.gfx_texture(12, 2, 1, 4096, 8), 0);
  assert.deepEqual(pre, { id: 12, w: 2, h: 1, rgba: [1, 2, 3, 4, 5, 6, 7, 8] });
  assert.equal(tw.gfx_texture(12, 2, 2, 4096, 8), -ERRNO.EINVAL, 'len must be w * h * 4');
  assert.equal(tw.gfx_texture(12, 2, 1, 0x7ffffff0, 8 ), -ERRNO.EFAULT);
  assert.equal(host.gfxStats.preloaded, 1);
});

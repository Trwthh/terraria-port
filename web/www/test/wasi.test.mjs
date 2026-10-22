// wasi.js over vfs.js against the in-memory OPFS stand-in. Calls the imports exactly as a wasm32-wasip1 module
// would: numbers, BigInt for 64-bit values, pointers into a WebAssembly.Memory.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createVfs } from '../vfs.js';
import { createWasi, WasiExit, PREVIEW1, WASI_SUSPENDING, RIGHTS as R, OFLAGS, FDFLAGS, WHENCE, FILETYPE } from '../wasi.js';
import { createHost } from '../host.js';
import { FakeDir } from './fake-opfs.mjs';

const E = { ACCES: 2, BADF: 8, EXIST: 20, INVAL: 28, ISDIR: 31, NOENT: 44, NOSYS: 52, NOTDIR: 54, NOTEMPTY: 55, NOTSUP: 58, ESPIPE: 70, NOTCAPABLE: 76 };
const READ_RIGHTS = R.FD_READ | R.FD_SEEK | R.FD_TELL | R.FD_FILESTAT_GET | R.FD_READDIR;
const WRITE_RIGHTS = READ_RIGHTS | R.FD_WRITE | R.FD_FILESTAT_SET_SIZE | R.FD_DATASYNC | R.FD_SYNC;

async function setup({ manyFiles = 0 } = {}) {
  const root = new FakeDir('');
  const install = await root.getDirectoryHandle('install', { create: true });
  const content = await install.getDirectoryHandle('Content', { create: true });
  const images = await content.getDirectoryHandle('Images', { create: true });
  images.addFile('Item_1.xnb', 'XNBw\x05fake');
  for (let i = 0; i < manyFiles; i++) images.addFile(`Tile_${String(i).padStart(4, '0')}.xnb`, 'x');
  content.addFile('Wave Bank.xwb', 'WBND0123456789');
  const vfs = createVfs(root);
  await vfs.init();
  const logs = [];
  const wasi = createWasi({ vfs, log: (level, text) => logs.push([level, text]) });
  const memory = new WebAssembly.Memory({ initial: 4 });
  wasi.setMemory(memory);
  const w = wasi.imports.wasi_snapshot_preview1;
  const u8 = () => new Uint8Array(memory.buffer);
  const dv = () => new DataView(memory.buffer);
  let top = 0;
  const str = (s) => {
    const bytes = new TextEncoder().encode(s);
    const p = 200000 + (top++ % 32) * 512;
    u8().set(bytes, p);
    return [p, bytes.length];
  };
  const text = (p, n) => new TextDecoder().decode(u8().slice(p, p + n));
  const OUT = 1000; // scratch: out-params
  // path_open helper: returns [errno, fd]
  const open = async (dirfd, path, { oflags = 0, rights = READ_RIGHTS, fdflags = 0 } = {}) => {
    const e = await w.path_open(dirfd, 1, ...str(path), oflags, rights, 0n, fdflags, OUT);
    return [e, dv().getUint32(OUT, true)];
  };
  // one iovec at 2000 -> buffer at `at`
  const iov = (at, len) => {
    dv().setUint32(2000, at, true);
    dv().setUint32(2004, len, true);
    return 2000;
  };
  return { root, vfs, wasi, w, memory, u8, dv, str, text, open, iov, logs, OUT, images };
}

test('import object: every preview1 function, the documented suspending set', async () => {
  const { w } = await setup();
  assert.deepEqual(Object.keys(w).sort(), [...PREVIEW1].sort());
  assert.equal(PREVIEW1.length, 46);
  assert.deepEqual([...WASI_SUSPENDING].sort(), ['path_create_directory', 'path_filestat_get', 'path_open', 'path_remove_directory', 'path_rename', 'path_unlink_file']);
});

test('args, environ, preopens', async () => {
  const { w, dv, text, u8 } = await setup();
  assert.equal(w.args_sizes_get(0, 4), 0);
  assert.deepEqual([dv().getUint32(0, true), dv().getUint32(4, true)], [1, 9]);
  assert.equal(w.args_get(100, 200), 0);
  assert.equal(text(dv().getUint32(100, true), 9), 'terraria\0');
  assert.equal(w.environ_sizes_get(0, 4), 0);
  const n = dv().getUint32(0, true);
  const size = dv().getUint32(4, true);
  assert.equal(n, 3);
  assert.equal(w.environ_get(100, 300), 0);
  const env = text(300, size).split('\0').filter(Boolean);
  assert.deepEqual(env, ['TERRARIA_DIR=/install', 'TERRARIA_SAVE_DIR=/saves', 'TERRARIA_PROJECT_DIR=/saves']);

  for (const [fd, name] of [[3, '/install'], [4, '/saves'], [5, '/']]) {
    assert.equal(w.fd_prestat_get(fd, 0), 0);
    assert.equal(dv().getUint8(0), 0);
    const len = dv().getUint32(4, true);
    assert.equal(w.fd_prestat_dir_name(fd, 500, len), 0);
    assert.equal(text(500, len), name);
  }
  assert.equal(w.fd_prestat_get(6, 0), E.BADF, 'libc stops scanning preopens here');
  assert.equal(w.fd_prestat_get(1, 0), E.BADF);
  assert.equal(w.fd_fdstat_get(3, 0), 0);
  assert.equal(u8()[0], FILETYPE.DIRECTORY);
  assert.equal(w.fd_fdstat_get(1, 0), 0);
  assert.equal(u8()[0], FILETYPE.CHARACTER_DEVICE);
});

test('read: path_open (case-insensitive), fd_read, fd_seek, fd_tell, fd_pread, filestat', async () => {
  const { w, open, iov, text, dv, OUT } = await setup();
  const [e, fd] = await open(3, 'content/images/ITEM_1.xnb');
  assert.equal(e, 0);
  assert.ok(fd >= 5);
  assert.equal(w.fd_read(fd, iov(3000, 4), 1, OUT), 0);
  assert.equal(dv().getUint32(OUT, true), 4);
  assert.equal(text(3000, 4), 'XNBw');
  assert.equal(w.fd_tell(fd, OUT), 0);
  assert.equal(dv().getBigUint64(OUT, true), 4n);
  assert.equal(w.fd_seek(fd, -4n, WHENCE.END, OUT), 0);
  assert.equal(dv().getBigUint64(OUT, true), 5n);
  assert.equal(w.fd_read(fd, iov(3000, 100), 1, OUT), 0);
  assert.equal(dv().getUint32(OUT, true), 4);
  assert.equal(text(3000, 4), 'fake');
  assert.equal(w.fd_seek(fd, -100n, WHENCE.CUR, OUT), E.INVAL);
  assert.equal(w.fd_pread(fd, iov(3000, 3), 1, 1n, OUT), 0);
  assert.equal(text(3000, 3), 'NBw');
  assert.equal(w.fd_tell(fd, OUT), 0);
  assert.equal(dv().getBigUint64(OUT, true), 9n, 'pread does not move the position');
  assert.equal(w.fd_filestat_get(fd, 4000), 0);
  assert.equal(dv().getUint8(4016), FILETYPE.REGULAR_FILE);
  assert.equal(dv().getBigUint64(4032, true), 9n);
  assert.equal(w.fd_write(fd, iov(3000, 1), 1, OUT), E.NOTCAPABLE, 'opened without FD_WRITE');
  assert.equal(w.fd_close(fd), 0);
  assert.equal(w.fd_close(fd), E.BADF);
  assert.equal((await open(3, 'Content/missing.xnb'))[0], E.NOENT);
  assert.equal((await open(3, '../saves/x'))[0], E.NOTCAPABLE, 'no escape above the preopen');
  assert.equal((await open(3, '/install/Content'))[0], E.NOTCAPABLE, 'absolute paths are refused');
  assert.equal((await open(3, 'Content/Wave Bank.xwb', { oflags: OFLAGS.DIRECTORY }))[0], E.NOTDIR);
});

test('the write guard: nothing under /install can be written, created or removed', async () => {
  const { w, open, str, root } = await setup();
  assert.equal((await open(3, 'Content/Wave Bank.xwb', { rights: WRITE_RIGHTS }))[0], E.ACCES);
  assert.equal((await open(3, 'Content/new.txt', { oflags: OFLAGS.CREAT, rights: WRITE_RIGHTS }))[0], E.ACCES);
  assert.equal((await open(3, 'Content/Wave Bank.xwb', { oflags: OFLAGS.TRUNC }))[0], E.ACCES);
  assert.equal(await w.path_create_directory(3, ...str('Content/New')), E.ACCES);
  assert.equal(await w.path_unlink_file(3, ...str('Content/Wave Bank.xwb')), E.ACCES);
  assert.equal(await w.path_remove_directory(3, ...str('Content/Images')), E.ACCES);
  assert.equal(await w.path_rename(3, ...str('Content/Wave Bank.xwb'), 4, ...str('stolen.xwb')), E.ACCES);
  assert.deepEqual([...root.map.get('install').map.get('Content').map.keys()].sort(), ['Images', 'Wave Bank.xwb']);
});

test('saves: create, O_EXCL, write, seek + pwrite / pread, truncate, append, rename, unlink, rmdir', async () => {
  const { w, open, iov, str, u8, dv, text, OUT, root } = await setup();
  assert.equal(await w.path_create_directory(4, ...str('Players')), 0);
  assert.equal(await w.path_create_directory(4, ...str('Players')), E.EXIST);
  assert.equal(await w.path_create_directory(4, ...str('No/Such')), E.NOENT);
  assert.equal(await w.path_create_directory(4, ...str('.')), E.EXIST, 'the preopen itself exists (create_dir_all("/saves"))');
  assert.equal(await w.path_create_directory(3, ...str('Content')), E.EXIST, 'existing beats the guard');

  const [e1, fd] = await open(4, 'Players/a.plr', { oflags: OFLAGS.CREAT | OFLAGS.EXCL, rights: WRITE_RIGHTS });
  assert.equal(e1, 0);
  assert.equal((await open(4, 'Players/a.plr', { oflags: OFLAGS.CREAT | OFLAGS.EXCL, rights: WRITE_RIGHTS }))[0], E.EXIST, 'O_EXCL on an existing file');
  u8().set(new TextEncoder().encode('0123456789'), 3000);
  assert.equal(w.fd_write(fd, iov(3000, 10), 1, OUT), 0);
  assert.equal(dv().getUint32(OUT, true), 10);
  // seek, then pwrite in the middle (position unchanged), then pread
  assert.equal(w.fd_seek(fd, 2n, WHENCE.SET, OUT), 0);
  u8().set(new TextEncoder().encode('AB'), 3100);
  assert.equal(w.fd_pwrite(fd, iov(3100, 2), 1, 5n, OUT), 0);
  assert.equal(w.fd_tell(fd, OUT), 0);
  assert.equal(dv().getBigUint64(OUT, true), 2n);
  assert.equal(w.fd_pread(fd, iov(3200, 10), 1, 0n, OUT), 0);
  assert.equal(text(3200, dv().getUint32(OUT, true)), '01234AB789');
  // fd_read continues at the seek position
  assert.equal(w.fd_read(fd, iov(3300, 3), 1, OUT), 0);
  assert.equal(text(3300, 3), '234');
  // write past the end leaves a hole of zeros
  assert.equal(w.fd_pwrite(fd, iov(3100, 2), 1, 12n, OUT), 0);
  assert.equal(w.fd_filestat_get(fd, 4000), 0);
  assert.equal(dv().getBigUint64(4032, true), 14n);
  assert.equal(w.fd_filestat_set_size(fd, 4n), 0);
  assert.equal(w.fd_filestat_get(fd, 4000), 0);
  assert.equal(dv().getBigUint64(4032, true), 4n);
  assert.equal(w.fd_sync(fd), 0);
  assert.equal(await w.path_unlink_file(4, ...str('Players/a.plr')), 10, 'EBUSY while open');
  assert.equal(w.fd_close(fd), 0);

  // append mode
  const [e2, fa] = await open(4, 'Players/a.plr', { rights: WRITE_RIGHTS, fdflags: FDFLAGS.APPEND });
  assert.equal(e2, 0);
  assert.equal(w.fd_write(fa, iov(3100, 2), 1, OUT), 0);
  w.fd_close(fa);
  // O_TRUNC
  const [e3, ft] = await open(4, 'Players/b.plr', { oflags: OFLAGS.CREAT | OFLAGS.TRUNC, rights: WRITE_RIGHTS });
  assert.equal(e3, 0);
  assert.equal(w.fd_write(ft, iov(3000, 3), 1, OUT), 0);
  w.fd_close(ft);

  // path_filestat_get on a closed file (async the first time is fine)
  assert.equal(await w.path_filestat_get(4, 1, ...str('Players/A.PLR'), 4000), 0);
  assert.equal(dv().getUint8(4016), FILETYPE.REGULAR_FILE);
  assert.equal(dv().getBigUint64(4032, true), 6n);
  assert.ok(dv().getBigUint64(4048, true) > 0n, 'mtime set by the write');
  assert.equal(await w.path_filestat_get(4, 1, ...str('Players'), 4000), 0);
  assert.equal(dv().getUint8(4016), FILETYPE.DIRECTORY);
  assert.equal(await w.path_filestat_get(4, 1, ...str('nope'), 4000), E.NOENT);

  // rename over an existing file; directories are refused
  assert.equal(await w.path_rename(4, ...str('Players/b.plr'), 4, ...str('Players/a.plr')), 0);
  const players = root.map.get('saves').map.get('Players');
  assert.deepEqual([...players.map.keys()], ['a.plr']);
  assert.equal(new TextDecoder().decode(players.map.get('a.plr').bytes), '012');
  assert.equal(await w.path_rename(4, ...str('Players'), 4, ...str('P2')), E.NOTSUP);

  assert.equal(await w.path_remove_directory(4, ...str('Players')), E.NOTEMPTY);
  assert.equal(await w.path_unlink_file(4, ...str('Players')), E.ISDIR);
  assert.equal(await w.path_remove_directory(4, ...str('Players/a.plr')), E.NOTDIR);
  assert.equal(await w.path_unlink_file(4, ...str('Players/a.plr')), 0);
  assert.equal(await w.path_remove_directory(4, ...str('Players')), 0);
  assert.equal(root.map.get('saves').map.size, 0);
});

function parseDirents(u8, start, used) {
  const out = [];
  const dv = new DataView(u8.buffer);
  let o = start;
  while (o + 24 <= start + used) {
    const next = dv.getBigUint64(o, true);
    const len = dv.getUint32(o + 16, true);
    const type = dv.getUint8(o + 20);
    if (o + 24 + len > start + used) break; // cut entry
    out.push({ next, type, name: new TextDecoder().decode(u8.slice(o + 24, o + 24 + len)) });
    o += 24 + len;
  }
  return out;
}

test('fd_readdir: d_type, d_next cookies, a listing larger than the buffer resumes from the cookie', async () => {
  const { w, open, u8, dv, OUT } = await setup({ manyFiles: 40 });
  const [e, dfd] = await open(3, 'Content/Images', { oflags: OFLAGS.DIRECTORY, rights: R.FD_READDIR | R.PATH_OPEN });
  assert.equal(e, 0);
  // small buffer: 200 bytes holds a few entries and a cut one
  const names = [];
  let cookie = 0n;
  let calls = 0;
  for (;;) {
    calls++;
    assert.equal(w.fd_readdir(dfd, 10000, 200, cookie, OUT), 0);
    const used = dv().getUint32(OUT, true);
    const ents = parseDirents(u8(), 10000, used);
    for (const en of ents) {
      names.push(en.name);
      assert.equal(en.type, en.name === '.' || en.name === '..' ? FILETYPE.DIRECTORY : FILETYPE.REGULAR_FILE);
    }
    if (ents.length) cookie = ents[ents.length - 1].next;
    if (used < 200) break; // a short buffer means the end
  }
  assert.ok(calls > 5, `needed several calls (${calls})`);
  assert.equal(names.length, 2 + 41);
  assert.deepEqual(names.slice(0, 3), ['.', '..', 'Item_1.xnb']);
  assert.equal(names.at(-1), 'Tile_0039.xnb');
  assert.equal(new Set(names).size, names.length, 'no entry twice');
  // cookie past the end: nothing
  assert.equal(w.fd_readdir(dfd, 10000, 200, 1000n, OUT), 0);
  assert.equal(dv().getUint32(OUT, true), 0);
  assert.equal(w.fd_readdir(3, 10000, 4096, 0n, OUT), 0, 'a preopen can be listed');
  assert.deepEqual(parseDirents(u8(), 10000, dv().getUint32(OUT, true)).map((d) => d.name), ['.', '..', 'Content']);
  const [, ffd] = await open(3, 'Content/Wave Bank.xwb');
  assert.equal(w.fd_readdir(ffd, 10000, 200, 0n, OUT), E.NOTDIR);
});

test('stdout / stderr to the log (line-buffered, UTF-8 split across writes), stdin empty', async () => {
  const { w, iov, u8, dv, logs, OUT } = await setup();
  const bytes = new TextEncoder().encode('héllo\nwor');
  u8().set(bytes, 3000);
  assert.equal(w.fd_write(1, iov(3000, 2), 1, OUT), 0); // "h" + first byte of "é"
  assert.equal(w.fd_write(1, iov(3002, bytes.length - 2), 1, OUT), 0);
  assert.deepEqual(logs, [[2, 'héllo']]);
  u8().set(new TextEncoder().encode('ld\npanic!\n'), 3000);
  w.fd_write(1, iov(3000, 3), 1, OUT);
  w.fd_write(2, iov(3003, 7), 1, OUT);
  assert.deepEqual(logs.slice(1), [[2, 'world'], [3, 'panic!']]);
  assert.equal(w.fd_read(0, iov(3000, 10), 1, OUT), 0);
  assert.equal(dv().getUint32(OUT, true), 0);
  assert.equal(w.fd_seek(1, 0n, 0, OUT), E.ESPIPE);
});

test('clocks, random, poll_oneoff, sched_yield, proc_exit, stubs', async () => {
  const { w, dv, u8, logs } = await setup();
  assert.equal(w.clock_time_get(0, 0n, 0), 0);
  const real = dv().getBigUint64(0, true);
  assert.ok(Math.abs(Number(real / 1_000_000n) - Date.now()) < 1000);
  assert.equal(w.clock_time_get(1, 0n, 0), 0);
  const mono = dv().getBigUint64(0, true);
  assert.ok(Math.abs(Number(mono) / 1e6 - performance.now()) < 1000);
  assert.equal(w.clock_res_get(1, 0), 0);
  assert.equal(w.clock_time_get(9, 0n, 0), E.INVAL);
  u8().fill(0, 100, 132);
  assert.equal(w.random_get(100, 32), 0);
  assert.ok(u8().slice(100, 132).some((x) => x));
  assert.equal(w.sched_yield(), 0);

  // two subscriptions: a clock (fires at once) and an fd read (not supported)
  const v = dv();
  v.setBigUint64(1000, 77n, true);
  v.setUint8(1008, 0);
  v.setUint32(1016, 1, true);
  v.setBigUint64(1024, 5_000_000_000n, true); // 5 s: must NOT block
  v.setBigUint64(1048, 88n, true);
  v.setUint8(1056, 1);
  const t0 = performance.now();
  assert.equal(w.poll_oneoff(1000, 2000, 2, 3000), 0);
  assert.ok(performance.now() - t0 < 100);
  assert.equal(v.getUint32(3000, true), 2);
  assert.equal(v.getBigUint64(2000, true), 77n);
  assert.equal(v.getUint16(2008, true), 0);
  assert.equal(v.getBigUint64(2032, true), 88n);
  assert.equal(v.getUint16(2040, true), 58);

  assert.throws(() => w.proc_exit(7), (e) => e instanceof WasiExit && e.code === 7);
  assert.equal(w.sock_accept(0, 0, 0), E.NOSYS);
  assert.equal(w.sock_accept(0, 0, 0), E.NOSYS);
  assert.equal(w.path_symlink(0, 0, 3, 0, 0), E.NOSYS);
  assert.deepEqual(logs.filter(([, t]) => t.includes('not implemented')).map(([, t]) => t), [
    'wasi: sock_accept is not implemented (ENOSYS)', 'wasi: path_symlink is not implemented (ENOSYS)',
  ], 'each stub logs its name once');
});

test('wasi and tw share one tree and one set of OPFS handles', async () => {
  const root = new FakeDir('');
  (await root.getDirectoryHandle('saves', { create: true })).addFile('shared.txt', 'abc');
  const vfs = createVfs(root);
  await vfs.init();
  const memory = new WebAssembly.Memory({ initial: 4 });
  const wasi = createWasi({ vfs, log() {} });
  const host = createHost({ vfs, post() {} });
  wasi.setMemory(memory);
  host.setMemory(memory);
  const enc = new TextEncoder();
  const put = (s, at) => (new Uint8Array(memory.buffer).set(enc.encode(s), at), [at, enc.encode(s).length]);
  const w = wasi.imports.wasi_snapshot_preview1;
  const tw = host.imports.tw;
  assert.equal(await w.path_open(4, 0, ...put('shared.txt', 0), 0, R.FD_READ, 0n, 0, 500), 0);
  const fd = new DataView(memory.buffer).getUint32(500, true);
  const h = tw.fs_open(...put('/saves/shared.txt', 100), 0);
  assert.equal(typeof h, 'number', 'tw reuses the handle wasi opened: no suspension');
  assert.equal(tw.fs_size(h), 3);
  assert.equal(await tw.fs_remove(...put('/saves/shared.txt', 100)), -10, 'busy while either side holds it');
  tw.fs_close(h);
  w.fd_close(fd);
  assert.equal(await tw.fs_remove(...put('/saves/shared.txt', 100)), 0);
});

test('preopen "/" (fd 5): Windows-style paths reach the same tree and the same guard', async () => {
  const { w, open, str, dv, u8, iov, OUT, root } = await setup();
  // what libc hands over for "/saves\config.json" (no "/saves" prefix match): "saves\config.json" relative to "/"
  const [e, fd] = await open(5, 'saves\\config.json', { oflags: OFLAGS.CREAT, rights: WRITE_RIGHTS });
  assert.equal(e, 0);
  assert.ok(fd >= 6, 'new fds start after the preopens');
  u8().set(new TextEncoder().encode('{}'), 3000);
  assert.equal(w.fd_write(fd, iov(3000, 2), 1, OUT), 0);
  assert.equal(w.fd_close(fd), 0);
  assert.deepEqual([...root.map.get('saves').map.keys()], ['config.json']);
  assert.equal((await open(5, 'install\\Content\\IMAGES\\item_1.xnb'))[0], 0, 'reads through "/" with case-insensitive lookup');
  assert.equal((await open(5, 'install\\Content\\x.txt', { oflags: OFLAGS.CREAT, rights: WRITE_RIGHTS }))[0], E.ACCES, 'the guard still applies');
  assert.equal(await w.path_create_directory(5, ...str('newroot')), E.ACCES, 'nothing directly under "/"');
  assert.equal(await w.path_create_directory(5, ...str('saves\\Players')), 0);
  assert.equal((await open(5, '..'))[0], E.NOTCAPABLE);
  // what the game really does: create_dir_all("\saves") checks the directory first, then writes "/saves\config.json"
  assert.equal(await w.path_filestat_get(5, 1, ...str('\\saves'), 4000), 0, 'a leading backslash is the same directory');
  assert.equal(dv().getUint8(4016), FILETYPE.DIRECTORY);
  assert.equal(await w.path_create_directory(5, ...str('\\saves\\Worlds')), 0);
  // a rooted Windows path relative to /saves (the game's current directory) is from the root of the tree, as on
  // Windows: "\saves\Worlds" under fd 4 is /saves/Worlds, not /saves/saves/Worlds
  const [ew, fw] = await open(4, '\\saves\\Worlds', { oflags: OFLAGS.DIRECTORY, rights: R.FD_READDIR | R.PATH_OPEN });
  assert.equal(ew, 0, 'opened /saves/Worlds');
  assert.equal(w.fd_close(fw), 0);
  assert.equal((await open(4, '\\install\\Content\\x.txt', { oflags: OFLAGS.CREAT, rights: WRITE_RIGHTS }))[0], E.ACCES, 'the guard still applies');
  assert.equal(w.fd_readdir(5, 10000, 4096, 0n, OUT), 0);
  const used = dv().getUint32(OUT, true);
  const names = [];
  for (let o = 10000; o < 10000 + used; ) { const n = dv().getUint32(o + 16, true); names.push(new TextDecoder().decode(u8().slice(o + 24, o + 24 + n))); o += 24 + n; }
  assert.deepEqual(names, ['.', '..', 'install', 'saves']);
});

test('server console: fd 0 from a stdin queue (EAGAIN while empty, split reads), fd 1 text to onStd as it comes', async () => {
  const vfs = createVfs(new FakeDir(''));
  await vfs.init();
  let queue = new TextEncoder().encode('');
  const shown = [];
  const wasi = createWasi({
    vfs,
    log: () => {},
    stdin: {
      read(max) {
        if (queue.length === 0) return null;
        const out = queue.slice(0, max);
        queue = queue.slice(max);
        return out;
      },
    },
    onStd: (fd, text) => shown.push([fd, text]),
  });
  const memory = new WebAssembly.Memory({ initial: 1 });
  wasi.setMemory(memory);
  const w = wasi.imports.wasi_snapshot_preview1;
  const dv = new DataView(memory.buffer);
  const u8 = new Uint8Array(memory.buffer);
  // two iovecs: 4 bytes at 3000, 100 at 4000
  dv.setUint32(2000, 3000, true);
  dv.setUint32(2004, 4, true);
  dv.setUint32(2008, 4000, true);
  dv.setUint32(2012, 100, true);
  assert.equal(w.fd_read(0, 2000, 2, 1000), 6, 'EAGAIN while nothing was typed');
  queue = new TextEncoder().encode('say hi\n');
  assert.equal(w.fd_read(0, 2000, 2, 1000), 0);
  assert.equal(dv.getUint32(1000, true), 7);
  assert.equal(new TextDecoder().decode(u8.slice(3000, 3004)) + new TextDecoder().decode(u8.slice(4000, 4003)), 'say hi\n');
  assert.equal(w.fd_read(0, 2000, 2, 1000), 6, 'empty again');
  u8.set(new TextEncoder().encode(': Time: 4:30 AM\r\n'), 5000);
  dv.setUint32(2020, 5000, true);
  dv.setUint32(2024, 2, true);
  w.fd_write(1, 2020, 1, 1000);
  assert.deepEqual(shown, [[1, ': ']], 'the prompt without a line end is shown at once');
});

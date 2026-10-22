// /install over HTTP (HOST_ABI.md "Install over HTTP"): the game's files come from the page's own server
// (web/host/terraria_host.py) instead of a copy imported into OPFS first. Used by vfs.js when the Worker is started
// with `install: 'http'`.
//
// - The manifest (install/manifest.json) lists every file: path, size, mtime, id, and its offset in the pack. vfs.js
//   builds the /install tree from it, so stat and listing never touch the network.
// - The pack (install/pack/<id>): every file but the big wave bank, concatenated in the order the game opens them at
//   start-up. The game Worker downloads it once, in the background, into one OPFS file (tw-cache/pack-<id>.bin; how
//   far it got: tw-cache/pack-<id>.json, so an interrupted download resumes). An open of a file the download has
//   passed is synchronous: a view of that one OPFS access handle. An open of a file not there yet waits for the
//   download when it is close, else fetches just that file's bytes (a Range request; the open suspends, JSPI).
// - Big files (Content/Wave Bank.xwb, 496 MB, the music): read in 1 MB blocks on demand, cached in
//   tw-cache/big-<id>.bin with a block map (big-<id>.map). The game reads them synchronously (fd_read), so a missing
//   block is fetched with a synchronous XMLHttpRequest (allowed in Workers; on this PC's loopback a 1 MB block
//   takes a few ms). After the pack, the game Worker fills the rest of the wave bank in the background.
// - Second start: the manifest is revalidated (a 304), the pack is complete, every open is synchronous.
//
// Pure apart from the injected platform (fetch, OPFS root, the synchronous range reader): tested by
// test/remote-install.test.mjs.

const BLOCK = 1 << 20;
const NEAR = 2 << 20; // a packed file at most this far beyond the download's end waits for it instead of a fetch
const MARK_EVERY = 16 << 20; // the download's progress is saved after every 16 MB
const errorNamed = (name, message) => Object.assign(new Error(message), { name });

/** A read-only access handle (the vfs.js shape) over bytes [off, off + size) of another access handle. */
export function sliceAccess(access, off, size) {
  return {
    read(view, { at = 0 } = {}) {
      if (at >= size) return 0;
      const n = Math.min(view.length, size - at);
      return access.read(n === view.length ? view : view.subarray(0, n), { at: off + at });
    },
    getSize: () => size,
    write() { throw errorNamed('NoModificationAllowedError', 'read-only'); },
    truncate() { throw errorNamed('NoModificationAllowedError', 'read-only'); },
    flush() {},
    close() {},
  };
}

/** A read-only access handle over bytes in memory. */
export function memoryAccess(bytes) {
  return {
    read(view, { at = 0 } = {}) {
      if (at >= bytes.length) return 0;
      const n = Math.min(view.length, bytes.length - at);
      view.set(bytes.subarray(at, at + n));
      return n;
    },
    getSize: () => bytes.length,
    write() { throw errorNamed('NoModificationAllowedError', 'read-only'); },
    truncate() { throw errorNamed('NoModificationAllowedError', 'read-only'); },
    flush() {},
    close() {},
  };
}

/** The browser's synchronous range read: bytes [a, b) of `url` (Workers only). */
export function xhrRange(url, a, b) {
  const x = new XMLHttpRequest();
  x.open('GET', url, false);
  x.responseType = 'arraybuffer';
  x.setRequestHeader('Range', `bytes=${a}-${b - 1}`);
  x.send();
  if (x.status === 206) return new Uint8Array(x.response);
  if (x.status === 200) return new Uint8Array(x.response).slice(a, b);
  throw errorNamed('NetworkError', `${url}: HTTP ${x.status}`);
}

/** A sync access handle that other Workers of the page may share ('readwrite-unsafe'), else an exclusive one. */
async function sharedHandle(fileHandle) {
  try {
    return { access: await fileHandle.createSyncAccessHandle({ mode: 'readwrite-unsafe' }), shared: true };
  } catch (e) {
    if (e && e.name === 'NoModificationAllowedError') throw e; // held exclusively elsewhere
    return { access: await fileHandle.createSyncAccessHandle(), shared: false };
  }
}

async function readJson(dir, name) {
  try {
    return JSON.parse(await (await (await dir.getFileHandle(name)).getFile()).text());
  } catch {
    return null;
  }
}
async function writeJson(dir, name, obj) {
  const h = await dir.getFileHandle(name, { create: true });
  if (typeof h.createWritable === 'function') {
    const w = await h.createWritable();
    await w.write(JSON.stringify(obj));
    await w.close();
  } else {
    // (test double) through a sync handle
    const a = await h.createSyncAccessHandle();
    try {
      const b = new TextEncoder().encode(JSON.stringify(obj));
      a.truncate(0);
      a.write(b, { at: 0 });
      a.flush();
    } finally {
      a.close();
    }
  }
}

/**
 * @param {object} o
 * @param {string} o.base  the server's root URL ("http://terraria.localhost:7790/")
 * @param {FileSystemDirectoryHandle} o.opfsRoot
 * @param {'game' | 'server'} [o.role]  the game Worker downloads the pack; the server Worker only reads
 * @param {(level: number, text: string) => void} [o.log]
 * @param {(state: object) => void} [o.onProgress]  the pack download's progress (game role)
 * @param {typeof fetch} [o.fetch]
 * @param {(url: string, a: number, b: number) => Uint8Array} [o.syncRange]
 * @param {boolean} [o.recordOrder]  remember the order of first opens (web/host/pack-order.txt is made from it)
 * @param {(params: object) => { onmessage: any }} [o.startHelper]  start the download in a Worker of its own
 *   (install-worker.js; given { packUrl, packId, packSize, done, progress, bigs }); without it the download runs
 *   in this Worker's event loop (tests)
 */
export function createRemoteInstall({ base, opfsRoot, role = 'game', log = () => {}, onProgress = () => {}, fetch: fetchFn = globalThis.fetch?.bind(globalThis), syncRange = xhrRange, recordOrder = false, startHelper = null }) {
  const url = (p) => new URL(p, base).href;
  const stats = { opens: 0, packOpens: 0, waited: 0, waitMs: 0, fetched: 0, fetchedBytes: 0, bigOpens: 0, bigBlocks: 0, bigBytes: 0, bigReads: 0, packDone: 0, packSize: 0, downloadMs: 0, cachedAtStart: 0, manifestMs: 0 };
  const order = [];
  const seen = new Set();
  let manifest = null;
  let files = [];
  let cache = null; // tw-cache directory
  let pack = null; // { id, size, url, access, shared, done }
  const waiters = []; // { until, resolve }
  const bigs = new Map(); // id -> { access, map: Uint8Array, mapAccess, url, size, blocks }
  let downloading = null;
  // the download Worker's progress: [0] bytes of the pack written, [1] 1 once it ended (SharedArrayBuffer)
  let progress = null;
  const SYNC_WAIT_MS = 1500; // an open waits at most this long, synchronously, for the download to reach its file
  const helperStart = { t: 0, done: 0 }; // for the download's measured speed
  /** How far the pack is in OPFS now. */
  const packDone = () => (progress ? Math.max(pack.done, Number(Atomics.load(progress, 0))) : pack.done);

  async function init() {
    const t0 = performance.now();
    const r = await fetchFn(url('install/manifest.json'), { cache: 'no-cache' });
    if (!r.ok) throw new Error(`install/manifest.json: HTTP ${r.status} (is web/host/terraria_host.py serving this page?)`);
    manifest = await r.json();
    stats.manifestMs = performance.now() - t0;
    files = manifest.files.map((f) => ({ path: f.p, size: f.s, mtime: f.m, id: f.h, off: f.o }));
    cache = await opfsRoot.getDirectoryHandle('tw-cache', { create: true });
    const p = manifest.pack;
    pack = { id: p.id, size: p.size, url: url(p.url), access: null, shared: false, done: 0 };
    stats.packSize = p.size;
    try {
      const h = await cache.getFileHandle(`pack-${p.id}.bin`, { create: true });
      Object.assign(pack, await sharedHandle(h));
      const mark = await readJson(cache, `pack-${p.id}.json`);
      pack.done = Math.min(Math.max(0, mark?.done | 0), pack.access.getSize(), pack.size);
    } catch (e) {
      log(3, `install cache: the pack's OPFS file is not available here (${e.name}: ${e.message}); files are fetched one by one`);
      pack.access = null;
    }
    stats.cachedAtStart = pack.done;
    stats.packDone = pack.done;
    if (role === 'game') evict().catch(() => {});
    return { files: files.length, packSize: pack.size, cached: pack.done, ms: performance.now() - t0 };
  }

  /** Cache files of other installs / builds are removed (tw-cache holds only the current pack and big files). */
  async function evict() {
    const keep = new Set([`pack-${pack.id}.bin`, `pack-${pack.id}.json`]);
    for (const f of files) if (f.off < 0) keep.add(`big-${f.id}.bin`).add(`big-${f.id}.map`);
    const gone = [];
    for await (const [name] of cache.entries()) if (!keep.has(name) && /^(pack|big)-/.test(name)) gone.push(name);
    for (const name of gone) await cache.removeEntry(name).catch(() => {});
    if (gone.length) log(2, `install cache: ${gone.length} files of an older install removed`);
  }

  function advance(done) {
    pack.done = done;
    stats.packDone = done;
    for (let i = waiters.length - 1; i >= 0; i--) {
      if (waiters[i].until <= done) waiters.splice(i, 1)[0].resolve();
    }
  }

  /** The background download of the pack (game role), resumable; then the big files' missing blocks. */
  function startDownload() {
    if (downloading || role !== 'game') return downloading;
    if (startHelper && pack.access && typeof SharedArrayBuffer === 'function' && (globalThis.crossOriginIsolated ?? true)) {
      progress = new BigInt64Array(new SharedArrayBuffer(16));
      progress[0] = BigInt(pack.done);
      const bigsList = files.filter((f) => f.off < 0).map((f) => ({ id: f.id, url: url(`install/file/${f.id}`), size: f.size, path: f.path }));
      if (pack.done >= pack.size) progress[1] = 1n;
      helperStart.t = performance.now();
      helperStart.done = pack.done;
      downloading = new Promise((resolve) => {
        const w = startHelper({ type: 'start', packUrl: pack.url, packId: pack.id, packSize: pack.size, done: pack.done, progress, bigs: bigsList });
        w.onmessage = (e) => {
          const x = e.data;
          if (x.type === 'log') log(x.level, x.text);
          else if (x.type === 'progress') {
            advance(Math.max(pack.done, x.done));
            onProgress({ phase: 'pack', done: x.done, total: x.total });
          } else if (x.type === 'done') {
            stats.downloadMs = x.ms;
            advance(packDone());
            for (const wt of waiters.splice(0)) wt.resolve();
            w.terminate?.();
            resolve();
          }
        };
      });
      return downloading;
    }
    downloading = (async () => {
      if (pack.access && pack.done < pack.size) {
        const t0 = performance.now();
        const from = pack.done;
        let markAt = pack.done + MARK_EVERY;
        try {
          const r = await fetchFn(pack.url, { headers: { Range: `bytes=${pack.done}-` } });
          if (r.status !== 206 && !(r.status === 200 && pack.done === 0)) throw new Error(`HTTP ${r.status}`);
          const reader = r.body.getReader();
          let pos = pack.done;
          for (;;) {
            const { value, done } = await reader.read();
            if (done) break;
            pack.access.write(value, { at: pos });
            pos += value.length;
            advance(pos);
            if (pos >= markAt || pos >= pack.size) {
              markAt = pos + MARK_EVERY;
              pack.access.flush();
              await writeJson(cache, `pack-${pack.id}.json`, { done: pos, size: pack.size });
              onProgress({ phase: 'pack', done: pos, total: pack.size });
            }
          }
          pack.access.flush();
          await writeJson(cache, `pack-${pack.id}.json`, { done: pack.done, size: pack.size });
          stats.downloadMs = performance.now() - t0;
          log(2, `install cache: ${((pack.done - from) / 1048576).toFixed(0)} MB of game files downloaded in ${(stats.downloadMs / 1000).toFixed(1)} s (${pack.done === pack.size ? 'complete' : 'incomplete'})`);
          onProgress({ phase: 'pack', done: pack.done, total: pack.size });
        } catch (e) {
          log(3, `install cache: the download stopped at ${(pack.done / 1048576).toFixed(0)} MB (${e.message}); missing files are fetched one by one`);
        }
      }
      for (const w of waiters.splice(0)) w.resolve(); // whatever still waits fetches its bytes itself
      // then the music: the big files' missing blocks, a few MB at a time, between frames
      for (const f of files) {
        if (f.off >= 0) continue;
        try {
          const b = await openBig(f);
          for (let i = 0; i < b.blocks; i++) {
            if (b.map[i]) continue;
            let j = i;
            while (j < b.blocks && !b.map[j] && j - i < 8) j++;
            const a = i * BLOCK;
            const e = Math.min(f.size, j * BLOCK);
            const r = await fetchFn(b.url, { headers: { Range: `bytes=${a}-${e - 1}` } });
            if (r.status !== 206) throw new Error(`HTTP ${r.status}`);
            putBlocks(b, i, new Uint8Array(await r.arrayBuffer()));
            i = j - 1;
          }
          log(2, `install cache: ${f.path} complete (${(f.size / 1048576).toFixed(0)} MB)`);
        } catch (e) {
          log(3, `install cache: ${f.path}: background fill stopped (${e.message})`);
        }
      }
    })();
    return downloading;
  }

  async function fetchRange(u, a, b) {
    const r = await fetchFn(u, { headers: { Range: `bytes=${a}-${b - 1}` } });
    if (r.status !== 206 && r.status !== 200) throw errorNamed('NetworkError', `HTTP ${r.status}`);
    const bytes = new Uint8Array(await r.arrayBuffer());
    return r.status === 200 ? bytes.slice(a, b) : bytes;
  }

  // ---------------------------------------------------------------- big files (block cache)
  async function openBig(f) {
    let b = bigs.get(f.id);
    if (b) return b;
    const blocks = Math.ceil(f.size / BLOCK);
    b = { url: url(`install/file/${f.id}`), size: f.size, blocks, access: null, mapAccess: null, map: new Uint8Array(blocks) };
    try {
      b.access = (await sharedHandle(await cache.getFileHandle(`big-${f.id}.bin`, { create: true }))).access;
      b.mapAccess = (await sharedHandle(await cache.getFileHandle(`big-${f.id}.map`, { create: true }))).access;
      b.mapAccess.read(b.map, { at: 0 });
    } catch (e) {
      log(3, `install cache: ${f.path} is read without a cache (${e.name})`);
      b.access = null;
      b.mapAccess = null;
      b.map.fill(0);
    }
    bigs.set(f.id, b);
    return b;
  }
  function putBlocks(b, first, bytes) {
    if (!b.access) return;
    b.access.write(bytes, { at: first * BLOCK });
    b.access.flush();
    const n = Math.ceil(bytes.length / BLOCK);
    for (let k = 0; k < n; k++) b.map[first + k] = 1;
    b.mapAccess.write(b.map.subarray(first, first + n), { at: first });
    b.mapAccess.flush();
    stats.bigBlocks += n;
    stats.bigBytes += bytes.length;
  }
  function bigAccess(b) {
    return {
      read(view, { at = 0 } = {}) {
        if (at >= b.size) return 0;
        const end = Math.min(b.size, at + view.length);
        stats.bigReads++;
        const i0 = Math.floor(at / BLOCK);
        const i1 = Math.ceil(end / BLOCK);
        // missing blocks: one synchronous request for each run of them
        let mem = null; // without a cache file: the fetched bytes, read directly
        for (let i = i0; i < i1; i++) {
          // the download Worker fills blocks too: its map entry first
          if (!b.map[i] && b.mapAccess && progress) b.mapAccess.read(b.map.subarray(i, i + 1), { at: i });
          if (b.map[i]) continue;
          let j = i;
          // at most 32 MB per request when there is a cache file to collect them in (a read of the whole bank: 496 MB)
          while (j < i1 && !b.map[j] && (!b.access || j - i < 32) && !(b.mapAccess && progress && (b.mapAccess.read(b.map.subarray(j, j + 1), { at: j }), b.map[j]))) j++;
          const bytes = syncRange(b.url, i * BLOCK, Math.min(b.size, j * BLOCK));
          if (b.access) putBlocks(b, i, bytes);
          else mem = { first: i, bytes };
          i = j - 1;
        }
        const n = end - at;
        const out = n === view.length ? view : view.subarray(0, n);
        if (mem) {
          out.set(mem.bytes.subarray(at - mem.first * BLOCK, at - mem.first * BLOCK + n));
          return n;
        }
        return b.access.read(out, { at });
      },
      getSize: () => b.size,
      write() { throw errorNamed('NoModificationAllowedError', 'read-only'); },
      truncate() { throw errorNamed('NoModificationAllowedError', 'read-only'); },
      flush() {},
      close() {},
    };
  }

  // ---------------------------------------------------------------- open
  /** An access handle (vfs.js shape) for manifest entry `f`, or a Promise of one. */
  function open(f) {
    stats.opens++;
    if (recordOrder && !seen.has(f.path)) {
      seen.add(f.path);
      order.push(f.path);
    }
    if (f.off < 0) {
      stats.bigOpens++;
      return openBig(f).then(bigAccess);
    }
    const end = f.off + f.size;
    if (pack.access && end <= packDone()) {
      stats.packOpens++;
      return sliceAccess(pack.access, f.off, f.size);
    }
    // the download Worker reaches this file soon (by its speed so far, within a second): wait for it right here,
    // synchronously (Atomics.wait in a Worker); else the file's bytes are fetched alone below
    const rate = progress ? (packDone() - helperStart.done) / Math.max(1, performance.now() - helperStart.t) : 0; // bytes per ms
    if (progress && pack.access && progress[1] === 0n && (end - packDone() <= NEAR || (rate > 0 && (end - packDone()) / rate < 1000))) {
      // the download Worker is close to this file: wait for it right here (synchronously: Atomics.wait in a Worker)
      const t0 = performance.now();
      for (;;) {
        const now = Atomics.load(progress, 0);
        if (Number(now) >= end || Atomics.load(progress, 1) !== 0n || performance.now() - t0 > SYNC_WAIT_MS) break;
        Atomics.wait(progress, 0, now, 50);
      }
      stats.waited++;
      stats.waitMs += performance.now() - t0;
      if (end <= packDone()) {
        stats.packOpens++;
        return sliceAccess(pack.access, f.off, f.size);
      }
    }
    return (async () => {
      if (pack.access && role !== 'game') {
        // the server Worker: the game Worker may have got further since this Worker started
        const mark = await readJson(cache, `pack-${pack.id}.json`);
        if (mark && mark.done >= end) {
          advance(Math.min(mark.done, pack.access.getSize()));
          stats.packOpens++;
          return sliceAccess(pack.access, f.off, f.size);
        }
      }
      if (pack.access && downloading && !progress && f.off <= pack.done + NEAR && pack.done < pack.size) {
        const t0 = performance.now();
        await new Promise((resolve) => waiters.push({ until: end, resolve }));
        stats.waited++;
        stats.waitMs += performance.now() - t0;
        if (end <= pack.done) {
          stats.packOpens++;
          return sliceAccess(pack.access, f.off, f.size);
        }
      }
      const bytes = await fetchRange(pack.url, f.off, end);
      stats.fetched++;
      stats.fetchedBytes += bytes.length;
      return memoryAccess(bytes);
    })();
  }

  return {
    init,
    open,
    startDownload,
    get files() { return files; },
    get manifest() { return manifest; },
    get complete() { return !!pack && pack.done >= pack.size; },
    stats,
    order,
  };
}

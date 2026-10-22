// The game files' download into this browser (lib/remote-install.js, HOST_ABI.md "Install over HTTP"), in a Worker
// of its own: the game Worker's event loop only turns between the game's slices, so a download there would get one
// network chunk per slice (measured: 258 MB in 114 s while the title loaded). Here it runs at the loopback's speed.
//
// Started by the game Worker with { packUrl, packId, packSize, done, progress, bigs: [{ id, url, size, path }] }:
// - writes the pack into OPFS tw-cache/pack-<id>.bin from `done` on (its own 'readwrite-unsafe' handle; the game
//   Worker reads through its own), publishes how far it got in `progress` (a BigInt64Array on a SharedArrayBuffer,
//   index 0, Atomics.notify on every chunk: the game Worker reads it, or waits on it, synchronously), and saves it
//   to tw-cache/pack-<id>.json every 16 MB (an interrupted download resumes there);
// - then the big files' missing 1 MB blocks (tw-cache/big-<id>.bin + .map), a few MB per request.
// Messages to the game Worker: { type: 'progress', done, total }, { type: 'log', level, text }, { type: 'done', ms }.

const BLOCK = 1 << 20;
const MARK_EVERY = 16 << 20;
const post = (m) => self.postMessage(m);

async function handle(dir, name) {
  const h = await dir.getFileHandle(name, { create: true });
  return h.createSyncAccessHandle({ mode: 'readwrite-unsafe' });
}
async function mark(dir, name, obj) {
  const w = await (await dir.getFileHandle(name, { create: true })).createWritable();
  await w.write(JSON.stringify(obj));
  await w.close();
}

self.onmessage = async (e) => {
  const m = e.data;
  if (m.type !== 'start') return;
  const t0 = performance.now();
  const cache = await (await navigator.storage.getDirectory()).getDirectoryHandle('tw-cache', { create: true });
  let done = m.done;
  try {
    if (done < m.packSize) {
      const access = await handle(cache, `pack-${m.packId}.bin`);
      const from = done;
      let markAt = done + MARK_EVERY;
      const r = await fetch(m.packUrl, { headers: { Range: `bytes=${done}-` } });
      if (r.status !== 206 && !(r.status === 200 && done === 0)) throw new Error(`HTTP ${r.status}`);
      const reader = r.body.getReader();
      for (;;) {
        const { value, done: end } = await reader.read();
        if (end) break;
        access.write(value, { at: done });
        done += value.length;
        if (done >= markAt || done >= m.packSize) {
          markAt = done + MARK_EVERY;
          access.flush();
          await mark(cache, `pack-${m.packId}.json`, { done, size: m.packSize });
          post({ type: 'progress', done, total: m.packSize });
        }
        // published after the bytes are written: whoever reads `progress` may read them at once
        Atomics.store(m.progress, 0, BigInt(done));
        Atomics.notify(m.progress, 0);
      }
      access.flush();
      access.close();
      await mark(cache, `pack-${m.packId}.json`, { done, size: m.packSize });
      post({ type: 'log', level: 2, text: `install cache: ${((done - from) / 1048576).toFixed(0)} MB of game files downloaded in ${((performance.now() - t0) / 1000).toFixed(1)} s (${done >= m.packSize ? 'complete' : 'incomplete'})` });
    }
  } catch (err) {
    post({ type: 'log', level: 3, text: `install cache: the download stopped at ${(done / 1048576).toFixed(0)} MB (${err.message}); missing files are fetched one by one` });
  }
  // a waiter in the game Worker gives up at once once it is clear nothing more comes (-1 = the download ended)
  Atomics.store(m.progress, 1, 1n);
  Atomics.notify(m.progress, 0);
  const packMs = performance.now() - t0;
  post({ type: 'progress', done, total: m.packSize, end: true });
  // the music: big files' missing blocks
  for (const b of m.bigs) {
    try {
      const data = await handle(cache, `big-${b.id}.bin`);
      const mapA = await handle(cache, `big-${b.id}.map`);
      const blocks = Math.ceil(b.size / BLOCK);
      const map = new Uint8Array(blocks);
      mapA.read(map, { at: 0 });
      let got = 0;
      for (let i = 0; i < blocks; i++) {
        mapA.read(map.subarray(i, i + 1), { at: i }); // the game Worker may have read this block meanwhile
        if (map[i]) continue;
        let j = i;
        while (j < blocks && !map[j] && j - i < 8) j++;
        const a = i * BLOCK;
        const z = Math.min(b.size, j * BLOCK);
        const r = await fetch(b.url, { headers: { Range: `bytes=${a}-${z - 1}` } });
        if (r.status !== 206) throw new Error(`HTTP ${r.status}`);
        data.write(new Uint8Array(await r.arrayBuffer()), { at: a });
        data.flush();
        map.fill(1, i, j);
        mapA.write(map.subarray(i, j), { at: i });
        mapA.flush();
        got += z - a;
        i = j - 1;
      }
      data.close();
      mapA.close();
      post({ type: 'log', level: 2, text: `install cache: ${b.path} complete (${(got / 1048576).toFixed(0)} MB fetched now)` });
    } catch (err) {
      post({ type: 'log', level: 3, text: `install cache: ${b.path}: background fill stopped (${err.message})` });
    }
  }
  post({ type: 'done', ms: packMs, total: performance.now() - t0 });
};

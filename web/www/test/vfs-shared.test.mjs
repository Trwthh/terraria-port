// vfs.js, the /saves tree shared by two Workers (the game and the dedicated server of Host & Play): two indexes over
// the same OPFS (one fake OPFS root, as two Workers of one origin see one storage), each with its own channel end.
// A file one creates, writes, renames or removes is seen by the other at its next lookup; stat and listing stay
// synchronous.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { FakeDir } from './fake-opfs.mjs';
import { createVfs } from '../vfs.js';

const tick = () => new Promise((r) => setTimeout(r, 5));
const until = async (f) => {
  for (let i = 0; i < 200; i++) {
    if (f()) return true;
    await tick();
  }
  return false;
};

async function pair() {
  const opfs = new FakeDir('root');
  const name = `tw-vfs-test-${Math.random()}`;
  const ca = new BroadcastChannel(name);
  const cb = new BroadcastChannel(name);
  ca.unref?.();
  cb.unref?.();
  const a = createVfs(opfs, { channel: ca });
  const b = createVfs(opfs, { channel: cb });
  await a.init();
  await b.init();
  return { opfs, a, b, close: () => (ca.close(), cb.close()) };
}
const names = (vfs, parts) => vfs.list(vfs.resolve(parts)).map((e) => e.name);
async function writeFile(vfs, parts, text) {
  const o = await vfs.openFile(parts, { write: true, create: true, trunc: true });
  assert.equal(typeof o, 'object');
  o.s.access.write(new TextEncoder().encode(text), { at: 0 });
  vfs.release(o.node, o.s, true);
}

test('a file the other index created, wrote, renamed, removed: seen at the next lookup', async () => {
  const { a, b, close } = await pair();
  await a.mkdir(['saves', 'Worlds'], { recursive: true });
  // the new directory: the announcement re-reads /saves in b (listing stays synchronous)
  assert.ok(await until(() => !!b.resolve(['saves', 'Worlds'])), 'b sees the directory a made');
  await writeFile(a, ['saves', 'Worlds', 'w.wld'], 'world v1');
  assert.ok(await until(() => names(b, ['saves', 'Worlds']).includes('w.wld')), 'b lists the file a created');
  // a lookup through the async entry point waits for a re-read that is still running, and re-reads on a miss
  await writeFile(a, ['saves', 'Worlds', 'x.wld'], 'x');
  await b.ready(['saves', 'Worlds', 'x.wld']);
  assert.ok(b.resolve(['saves', 'Worlds', 'x.wld']), 'ready() made the brand-new file visible');
  // size: b stat'ed (cached) v1; a writes a longer save; b's cached size goes, the next stat is the new one
  const n = b.resolve(['saves', 'Worlds', 'w.wld']);
  assert.equal((await b.fileStat(n)).size, 8);
  await writeFile(a, ['saves', 'Worlds', 'w.wld'], 'world v2, longer');
  assert.ok(await until(() => n.size === undefined || n.size === 16), 'the cached size of a written file is dropped');
  assert.equal((await b.fileStat(b.resolve(['saves', 'Worlds', 'w.wld']))).size, 16);
  // a backup made by renaming, then a removal
  assert.equal(await a.rename(['saves', 'Worlds', 'x.wld'], ['saves', 'Worlds', 'w.wld.bak']), 0);
  assert.ok(await until(() => { const l = names(b, ['saves', 'Worlds']); return l.includes('w.wld.bak') && !l.includes('x.wld'); }), 'b sees the rename');
  assert.equal(await a.remove(['saves', 'Worlds', 'w.wld.bak']), 0);
  assert.ok(await until(() => !names(b, ['saves', 'Worlds']).includes('w.wld.bak')), 'b sees the removal');
  // and the other way round: b's write is a's to see
  await writeFile(b, ['saves', 'Worlds', 'from-b.wld'], 'b');
  await a.ready(['saves', 'Worlds', 'from-b.wld']);
  assert.ok(a.resolve(['saves', 'Worlds', 'from-b.wld']));
  assert.ok(a.stats.announced > 0 && b.stats.heard > 0 && b.stats.refreshed > 0);
  close();
});

test('without a message (a missed one): a miss under /saves re-reads the directory; /install is never re-read', async () => {
  const opfs = new FakeDir('root');
  const a = createVfs(opfs, { channel: null });
  const b = createVfs(opfs, { channel: null });
  await a.init();
  await b.init();
  await a.mkdir(['saves', 'Players'], { recursive: true });
  await writeFile(a, ['saves', 'Players', 'p.plr'], 'p');
  assert.equal(b.resolve(['saves', 'Players']), null, 'no channel: b has not heard');
  const r1 = b.ready(['saves', 'Players', 'p.plr']);
  assert.ok(r1 && typeof r1.then === 'function', 'a miss: a re-read');
  await r1;
  assert.ok(b.resolve(['saves', 'Players', 'p.plr']), 'found after the re-read (a new directory, then its file)');
  assert.equal(b.ready(['saves', 'Players', 'p.plr']), undefined, 'a hit answers at once');
  assert.equal(b.ready(['install', 'Content', 'nothing.xnb']), undefined, '/install is not shared and never re-read');
  assert.equal(await b.ready(['saves', 'Players', 'none.plr']), 0, 'a real miss: one re-read, then ENOENT for the caller');
});

test('refreshSaves: every /saves directory read again (the other Worker ended before its announcements arrived)', async () => {
  const opfs = new FakeDir('root');
  const a = createVfs(opfs, { channel: null });
  const b = createVfs(opfs, { channel: null });
  await a.init();
  await a.mkdir(['saves', 'Worlds'], { recursive: true });
  await b.init();
  await writeFile(a, ['saves', 'Worlds', 'w.wld'], 'w');
  await writeFile(a, ['saves', 'Worlds', 'w.wld.bak'], 'old');
  assert.deepEqual(names(b, ['saves', 'Worlds']), []);
  await b.refreshSaves();
  assert.deepEqual(names(b, ['saves', 'Worlds']), ['w.wld', 'w.wld.bak']);
});

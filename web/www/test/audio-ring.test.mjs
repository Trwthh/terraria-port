import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createRing, ringViews, ringWrite, ringRead, ringFree, ringAvailable, ChunkQueue } from '../lib/audio-ring.js';

const stereo = (n, start = 0) => {
  const a = new Float32Array(n * 2);
  for (let i = 0; i < n; i++) {
    a[i * 2] = start + i;
    a[i * 2 + 1] = -(start + i);
  }
  return a;
};

test('ring: write/read with wrap-around, one slot kept empty', () => {
  const v = ringViews(createRing(8));
  assert.equal(v.cap, 8);
  assert.equal(ringFree(v), 7);
  assert.equal(ringWrite(v, stereo(10), 10), 7, 'only cap-1 frames fit');
  assert.equal(ringAvailable(v), 7);
  const L = new Float32Array(5);
  const R = new Float32Array(5);
  assert.equal(ringRead(v, L, R, 5), 5);
  assert.deepEqual([...L], [0, 1, 2, 3, 4]);
  assert.deepEqual([...R], [-0, -1, -2, -3, -4]);
  assert.equal(ringWrite(v, stereo(4, 100), 4), 4); // wraps
  const L2 = new Float32Array(8);
  const R2 = new Float32Array(8);
  assert.equal(ringRead(v, L2, R2, 8), 6);
  assert.deepEqual([...L2.subarray(0, 6)], [5, 6, 100, 101, 102, 103]);
  assert.equal(ringAvailable(v), 0);
});

test('ring works on a plain ArrayBuffer too (no isolation in tests)', () => {
  const v = ringViews(createRing(4, ArrayBuffer));
  assert.equal(ringWrite(v, stereo(2), 2), 2);
  assert.equal(ringAvailable(v), 2);
});

test('chunk queue: reads across chunk boundaries', () => {
  const q = new ChunkQueue();
  q.push(stereo(3, 0));
  q.push(stereo(4, 10));
  const L = new Float32Array(5);
  const R = new Float32Array(5);
  assert.equal(q.read(L, R, 5), 5);
  assert.deepEqual([...L], [0, 1, 2, 10, 11]);
  assert.equal(q.frames, 2);
  assert.equal(q.read(L, R, 5), 2);
  assert.deepEqual([...L.subarray(0, 2)], [12, 13]);
  assert.equal(q.read(L, R, 5), 0);
});

test('LevelMeter: frames, peak and RMS in 16-bit units; underruns only after the first frame', async () => {
  const { LevelMeter } = await import('../lib/audio-ring.js');
  const m = new LevelMeter();
  const z = new Float32Array(4);
  m.add(z, z, 0, 4); // nothing arrived yet: not an underrun
  m.add(Float32Array.of(0.5, -0.5, 0, 0), Float32Array.of(-0.5, 0.5, 0, 0), 2, 4); // the first frames: no underrun yet
  m.add(z, z, 0, 2); // then 2 frames of silence because nothing arrived
  const r = m.report();
  assert.equal(r.frames, 2);
  assert.equal(r.peak, Math.round(0.5 * 32767));
  assert.equal(r.rms, +(0.5 * 32767).toFixed(1));
  assert.equal(r.underrunFrames, 2);
});

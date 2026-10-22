// Audio transport between the game Worker and the AudioWorklet (HOST_ABI.md, "Audio").
// Pure: used by host.js and audio-worklet.js, tested by test/audio-ring.test.mjs.
//
// Ring layout (SharedArrayBuffer, used when the page is cross-origin isolated):
//   bytes 0..3  i32 write index (frames, 0..cap-1), written by the Worker only
//   bytes 4..7  i32 read index  (frames, 0..cap-1), written by the worklet only
//   bytes 8..   f32 stereo interleaved samples, cap frames
// One frame slot is always left empty, so at most cap-1 frames are buffered.

export const SAMPLE_RATE = 44100;
export const CHANNELS = 2;
export const HEADER_BYTES = 8;
export const RING_FRAMES = 16384; // about 370 ms
export const TARGET_FRAMES = 4096; // audio_free keeps about 93 ms buffered

export function createRing(frames = RING_FRAMES, SAB = globalThis.SharedArrayBuffer) {
  return new SAB(HEADER_BYTES + frames * CHANNELS * 4);
}

export function ringViews(sab) {
  const data = new Float32Array(sab, HEADER_BYTES);
  return { hdr: new Int32Array(sab, 0, 2), data, cap: data.length / CHANNELS };
}

export function ringAvailable(v) {
  const w = Atomics.load(v.hdr, 0);
  const r = Atomics.load(v.hdr, 1);
  return (w - r + v.cap) % v.cap;
}

export function ringFree(v) {
  return v.cap - 1 - ringAvailable(v);
}

/** Write up to `frames` interleaved stereo frames from `src` (Float32Array). Returns frames written. */
export function ringWrite(v, src, frames) {
  const n = Math.min(frames, ringFree(v), Math.floor(src.length / CHANNELS));
  let w = Atomics.load(v.hdr, 0);
  let done = 0;
  while (done < n) {
    const run = Math.min(n - done, v.cap - w);
    v.data.set(src.subarray(done * CHANNELS, (done + run) * CHANNELS), w * CHANNELS);
    done += run;
    w = (w + run) % v.cap;
  }
  Atomics.store(v.hdr, 0, w);
  return n;
}

/** Read up to `frames` frames into planar outputs L and R. Returns frames read (the caller zero-fills the rest). */
export function ringRead(v, L, R, frames) {
  const n = Math.min(frames, ringAvailable(v));
  let r = Atomics.load(v.hdr, 1);
  for (let i = 0; i < n; i++) {
    L[i] = v.data[r * 2];
    R[i] = v.data[r * 2 + 1];
    r = r + 1 === v.cap ? 0 : r + 1;
  }
  Atomics.store(v.hdr, 1, r);
  return n;
}

/** The postMessage fallback on the worklet side: a queue of interleaved Float32Array chunks. */
export class ChunkQueue {
  constructor() {
    this.chunks = [];
    this.off = 0; // frame offset into chunks[0]
    this.frames = 0;
  }
  push(chunk) {
    if (chunk.length >= CHANNELS) {
      this.chunks.push(chunk);
      this.frames += Math.floor(chunk.length / CHANNELS);
    }
  }
  read(L, R, frames) {
    let i = 0;
    while (i < frames && this.chunks.length) {
      const c = this.chunks[0];
      const have = Math.floor(c.length / CHANNELS) - this.off;
      const run = Math.min(have, frames - i);
      for (let k = 0; k < run; k++) {
        L[i + k] = c[(this.off + k) * 2];
        R[i + k] = c[(this.off + k) * 2 + 1];
      }
      i += run;
      this.off += run;
      if (this.off >= Math.floor(c.length / CHANNELS)) {
        this.chunks.shift();
        this.off = 0;
      }
    }
    this.frames -= i;
    return i;
  }
}

/** Level statistics of what the worklet played (headless runs report them): frames received, peak and RMS in
 *  16-bit units (32,767 = full scale, as the native mixer's `AudioStats`), frames played as silence because nothing
 *  had arrived (counted only after the first frame arrived). */
export class LevelMeter {
  constructor() {
    this.frames = 0;
    this.peak = 0;
    this.sumSq = 0;
    this.underrun = 0;
  }
  add(L, R, got, wanted) {
    for (let i = 0; i < got; i++) {
      const l = Math.abs(L[i]);
      const r = Math.abs(R[i]);
      if (l > this.peak) this.peak = l;
      if (r > this.peak) this.peak = r;
      this.sumSq += l * l + r * r;
    }
    if (this.frames > 0) this.underrun += wanted - got;
    this.frames += got;
  }
  report() {
    const rms = this.frames ? Math.sqrt(this.sumSq / (this.frames * CHANNELS)) : 0;
    return { frames: this.frames, peak: Math.round(this.peak * 32767), rms: +(rms * 32767).toFixed(1), underrunFrames: this.underrun };
  }
}

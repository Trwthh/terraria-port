// AudioWorklet processor: plays the game's stereo f32 frames at 44,100 Hz (HOST_ABI.md, "Audio").
// The page sends { type: 'link', port, sab } once; `port` is a MessagePort whose other end is in the game Worker.
// With `sab` (cross-origin isolated page) samples come through the shared ring; without it the Worker posts
// { type: 'chunk', data: Float32Array } messages and this side reports { type: 'consumed', frames } back.
// Every 64 render quanta the page gets { type: 'stats', frames, peak, rms, underrunFrames } on `this.port` (what was
// played: LevelMeter in lib/audio-ring.js).

import { ringViews, ringRead, ChunkQueue, LevelMeter } from './lib/audio-ring.js';

class TwAudio extends AudioWorkletProcessor {
  constructor() {
    super();
    this.ring = null;
    this.queue = new ChunkQueue();
    this.link = null;
    this.consumed = 0;
    this.blocks = 0;
    this.meter = new LevelMeter();
    this.quanta = 0;
    this.port.onmessage = (e) => {
      const m = e.data;
      if (m && m.type === 'link') {
        this.link = m.port;
        if (m.sab) this.ring = ringViews(m.sab);
        this.link.onmessage = (ev) => {
          if (ev.data && ev.data.type === 'chunk') this.queue.push(ev.data.data);
        };
      }
    };
  }

  process(_inputs, outputs) {
    const out = outputs[0];
    if (!out || out.length === 0) return true;
    const L = out[0];
    const R = out[1] ?? out[0];
    const frames = L.length;
    const got = this.ring ? ringRead(this.ring, L, R, frames) : this.queue.read(L, R, frames);
    for (let i = got; i < frames; i++) {
      L[i] = 0;
      R[i] = 0;
    }
    this.meter.add(L, R, got, frames);
    if (++this.quanta % 64 === 0) this.port.postMessage({ type: 'stats', ...this.meter.report() });
    if (!this.ring && this.link) {
      this.consumed += got;
      if (++this.blocks % 8 === 0) this.link.postMessage({ type: 'consumed', frames: this.consumed });
    }
    return true;
  }
}

registerProcessor('tw-audio', TwAudio);

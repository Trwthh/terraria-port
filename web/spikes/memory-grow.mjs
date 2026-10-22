// V8 cost of WebAssembly memory.grow by step size (node web/spikes/memory-grow.mjs). 2026-10-13, Node 24:
// 4,576 grows of 256 KB to 1.4 GB: 6,084 ms; 18 grows of 64 MB: 24 ms. terraria-web lib.rs GROW_STEP.
const m = new WebAssembly.Memory({ initial: 4096 });
let t = performance.now(); let n = 0;
while (m.buffer.byteLength < 1400 * 1048576) { m.grow(4); n++; }
console.log(`grow by 4 pages (256 KB) x ${n}: ${(performance.now() - t).toFixed(0)} ms`);
const m2 = new WebAssembly.Memory({ initial: 4096 });
t = performance.now(); n = 0;
while (m2.buffer.byteLength < 1400 * 1048576) { m2.grow(1024); n++; }
console.log(`grow by 64 MB x ${n}: ${(performance.now() - t).toFixed(0)} ms`);

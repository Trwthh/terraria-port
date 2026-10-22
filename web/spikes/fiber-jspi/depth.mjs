// node --experimental-wasm-jspi depth.mjs <fiber_jspi.wasm>
// Deepest recursion of probe_depth that fits: on the main stack, and on a JSPI (secondary) stack.
import { readFileSync } from 'node:fs';
const suspend = new WebAssembly.Suspending(async () => {});
const { instance } = await WebAssembly.instantiate(readFileSync(process.argv[2]), { host: { suspend } });
const ex = instance.exports;
const onFiber = WebAssembly.promising(ex.probe_depth_on_fiber);
async function fits(f, n) { try { await f(n); return true; } catch (e) { if (e instanceof RangeError) return false; throw e; } }
async function search(f) { let lo = 1, hi = 1 << 24; while (lo + 1 < hi) { const m = (lo + hi) >>> 1; if (await fits(f, m)) lo = m; else hi = m; } return lo; }
for (let round = 0; round < 2; round++) {
  const main = await search(async (n) => ex.probe_depth(n));
  const fiber = await search(onFiber);
  console.log(`round ${round}: deepest probe_depth on the main stack ${main}, on a JSPI stack ${fiber}`);
}

// node --experimental-wasm-jspi sched.mjs <fiber_jspi.wasm>
// The scheduler inside wasm: main_frame (promising) calls host.resume (Suspending) for each unfinished work item.
import { readFileSync } from 'node:fs';
let ex;
const savedSp = new Map(); // fiber -> its shadow-stack pointer while suspended
const pending = new Map(); // fiber -> resolve of its current suspension
const running = new Map(); // fiber -> promise of its work_entry_stored
let back = null;           // resolves the main fiber's `resume` when the work item gives the frame back / ends
let mainSp = 0;            // the main fiber's stack pointer at the moment it called resume

const suspend = new WebAssembly.Suspending(async (fiber) => {
  const sp = ex.__stack_pointer;
  savedSp.set(fiber, sp.value);
  sp.value = mainSp;
  const p = new Promise((res) => pending.set(fiber, res));
  back(0);
  await p;
});
const DEPTH = 300;
const resume = new WebAssembly.Suspending(async (fiber) => {
  const sp = ex.__stack_pointer;
  mainSp = sp.value;
  return await new Promise((res) => {
    back = res;
    if (!running.has(fiber)) {
      sp.value = ex.alloc_stack(256 * 1024);
      const p = work(fiber, DEPTH);
      running.set(fiber, p);
      p.then(() => { sp.value = mainSp; back(1); });
    } else {
      sp.value = savedSp.get(fiber);
      pending.get(fiber)();
    }
  });
});
const { instance } = await WebAssembly.instantiate(readFileSync(process.argv[2]), { host: { suspend, resume } });
ex = instance.exports;
const work = WebAssembly.promising(ex.work_entry_stored);
const mainFrame = WebAssembly.promising(ex.main_frame);
const expect = [1, 2, 3].map((f) => ex.reference(f, DEPTH));
const sp0 = ex.__stack_pointer.value;
ex.set_yield_every(37);
let frames = 0, alive = 3, bad = 0;
const t0 = performance.now();
while (alive > 0 && frames < 10000) {
  frames++;
  const r = await mainFrame(frames);
  if (r >= 1000) bad++;
  alive = r % 1000;
}
const ms = performance.now() - t0;
let ok = bad === 0 && ex.__stack_pointer.value === sp0;
[1, 2, 3].forEach((f, i) => {
  const same = ex.result_of(f) === expect[i];
  ok &&= same;
  console.log(`fiber ${f}: ${same ? 'EQUAL' : 'DIFFERENT'} to the reference`);
});
console.log(`frames ${frames}, main-frame checks failed ${bad}, suspensions ${ex.yields()}, ${ms.toFixed(1)} ms`);
console.log(ok ? 'PASS (scheduler inside wasm: main fiber suspended in resume, work fibers on their own stacks)' : 'FAIL');

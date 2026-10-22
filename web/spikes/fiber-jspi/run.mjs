// node --experimental-wasm-jspi run.mjs <fiber_jspi.wasm> [noswap]
// Interleaves three work items (fibers) with a frame loop, the way terraria-host's run_pending_work does:
// every frame calls frame() on the main stack, then continues each unfinished work item until it gives the frame back.
// "noswap" is the negative control: every fiber shares the main shadow stack.
import { readFileSync } from 'node:fs';

const [file, mode] = process.argv.slice(2);
const swap = mode !== 'noswap';
let ex;
const pending = new Map(); // fiber -> resolve function of its current suspension
const savedSp = new Map(); // fiber -> its shadow-stack pointer while suspended
let mainSp = 0;
let notify = null; // the scheduler's "the running fiber gave the frame back / finished"
let scanned = false;
const SEARCH = 3n;

const suspend = new WebAssembly.Suspending(async (fiber) => {
  const sp = ex.__stack_pointer;
  if (swap) {
    savedSp.set(fiber, sp.value);
    sp.value = mainSp;
  }
  if (BigInt(fiber) === SEARCH && !scanned) {
    // The collector question: is a value that a suspended frame holds in a wasm local anywhere in linear memory?
    scanned = true;
    const mem = new BigUint64Array(ex.memory.buffer, 0, ex.memory.buffer.byteLength >> 3);
    const want = (SEARCH * 1000n) ^ (SEARCH << 17n) ^ 0x5151000000000000n; // `local` of the outermost deep() frame
    let hits = 0;
    for (let i = 0; i < mem.length; i++) if (mem[i] === want) hits++;
    console.log(`scan of all ${ex.memory.buffer.byteLength >> 20} MB of linear memory while fiber 3 is suspended: the outermost frame's wasm local found ${hits} time(s)`);
  }
  const p = new Promise((res) => pending.set(fiber, res));
  notify({ fiber, done: false });
  await p;
});

const { instance } = await WebAssembly.instantiate(readFileSync(file), { host: { suspend } });
ex = instance.exports;
const work = WebAssembly.promising(ex.work_entry);
const sp = ex.__stack_pointer;
mainSp = sp.value;

const DEPTH = 300;
const fibers = [1, 2, 3];
const expect = new Map(fibers.map((f) => [f, ex.reference(f, DEPTH)]));
ex.set_yield_every(37);
const results = new Map();
const running = new Map();

// `start` runs wasm synchronously up to the first suspension (that is how a promising export behaves), so the
// notification target must be in place before it is called.
function until(fiber, start) {
  return new Promise((res) => {
    notify = res;
    const fresh = !running.has(fiber);
    start();
    if (fresh) {
      // once per work item: when it returns, tell whoever waits for it now
      running.get(fiber).then((v) => {
        results.set(fiber, v);
        if (swap) sp.value = mainSp;
        notify({ fiber, done: true });
      });
    }
  });
}

let frames = 0;
let framesOk = 0;
let resumptions = 0;
const t0 = performance.now();
while (results.size < fibers.length && frames < 100000) {
  frames++;
  framesOk += ex.frame(frames); // Update / Draw on the main stack
  for (const f of fibers) {
    // run_pending_work: continue every unfinished work item once
    if (results.has(f)) continue;
    if (!running.has(f)) {
      if (swap) sp.value = ex.alloc_stack(256 * 1024);
      else sp.value = mainSp;
      await until(f, () => running.set(f, work(f, DEPTH)));
    } else {
      if (swap) sp.value = savedSp.get(f);
      await until(f, () => pending.get(f)());
    }
    resumptions++;
  }
}
const ms = performance.now() - t0;
let ok = framesOk === frames;
for (const f of fibers) {
  const same = results.get(f) === expect.get(f);
  ok &&= same;
  console.log(`fiber ${f}: result ${results.get(f)?.toString(16)} reference ${expect.get(f).toString(16)} ${same ? 'EQUAL' : 'DIFFERENT'}`);
}
const y = ex.yields();
console.log(`frames ${frames} (main-stack self-check ok in ${framesOk}), suspensions ${y}, resumptions ${resumptions}, ${ms.toFixed(1)} ms, ${((ms * 1000) / y).toFixed(2)} us per suspend + resume (including the frame calls)`);
console.log(`${ok && sp.value === mainSp ? 'PASS' : 'FAIL'} (${swap ? 'one shadow stack per fiber' : 'NEGATIVE CONTROL: fibers share the main shadow stack'})`);

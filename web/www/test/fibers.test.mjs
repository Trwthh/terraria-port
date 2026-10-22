// lib/fibers.js: the hand-over protocol between the main stack and work fibers, with async functions standing in for
// the JSPI stacks (a suspending import is an awaited promise, a promising export an async function). The real
// engine stacks run in the headless game run (test/run-game-headless.mjs).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createFibers, FIBER_SUSPENDING } from '../lib/fibers.js';

function setup() {
  const sp = new WebAssembly.Global({ value: 'i32', mutable: true }, 1000);
  const fib = createFibers();
  const seen = []; // [who, stack pointer seen]
  const work = new Map(); // handle -> async body(handle)
  const exports = {
    __stack_pointer: sp,
    tw_fiber_entry: (handle) => work.get(handle)(handle),
  };
  assert.equal(fib.attach(exports, (f) => f), true);
  return { sp, fib, seen, work, t: fib.imports };
}

test('the suspending set', () => {
  assert.deepEqual([...FIBER_SUSPENDING].sort(), ['fiber_resume', 'fiber_suspend']);
});

test('fibers run on their own stack pointer and the main stack gets its own back', async () => {
  const { sp, fib, seen, work, t } = setup();
  // a fiber that suspends n times, checking its stack pointer each time it runs
  const body = (n, top) => async (h) => {
    for (let i = 0; i < n; i++) {
      seen.push([h, sp.value]);
      sp.value = top - 16 * (i + 1); // the fiber's frames move its pointer
      await t.fiber_suspend(h);
      assert.equal(sp.value, top - 16 * (i + 1), `fiber ${h} resumed with its own pointer`);
    }
    return h * 10;
  };
  work.set(7, body(3, 50000));
  work.set(9, body(1, 90000));
  const codes = [];
  // the main stack: one "frame" resumes every live fiber once, like run_pending_work
  const started = new Set();
  for (let frame = 0; frame < 6; frame++) {
    sp.value = 1000 - frame; // the main stack's own pointer this frame
    for (const [h, top] of [[7, 50000], [9, 90000]]) {
      if (codes.includes(`${h}:1`)) continue;
      const first = !started.has(h);
      started.add(h);
      const r = await t.fiber_resume(h, first ? top : 0);
      codes.push(`${h}:${r}`);
      assert.equal(sp.value, 1000 - frame, 'the main pointer is back after every hand-over');
    }
  }
  assert.deepEqual(codes, ['7:0', '9:0', '7:0', '9:1', '7:0', '7:1']);
  assert.deepEqual(seen.filter(([h]) => h === 7).map(([, v]) => v), [50000, 49984, 49968]);
  assert.equal(fib.live(), 0);
  const { ms, ...counts } = fib.stats;
  assert.deepEqual(counts, { started: 2, ended: 2, trapped: 0, resumes: 6, suspends: 4 });
  assert.ok(ms >= 0);
});

test('a fiber that traps ends with 2 and leaves the main stack running', async () => {
  const { sp, fib, work, t } = setup();
  const logs = [];
  const fib2 = createFibers({ log: (l, s) => logs.push([l, s]) });
  fib2.attach({ __stack_pointer: sp, tw_fiber_entry: (h) => work.get(h)(h) }, (f) => f);
  work.set(3, async (h) => {
    await fib2.imports.fiber_suspend(h);
    throw new WebAssembly.RuntimeError('unreachable');
  });
  sp.value = 500;
  assert.equal(await fib2.imports.fiber_resume(3, 8000), 0);
  assert.equal(await fib2.imports.fiber_resume(3, 0), 2);
  assert.equal(sp.value, 500);
  assert.equal(fib2.live(), 0);
  assert.match(logs[0][1], /fiber 3 trapped: RuntimeError: unreachable/);
  assert.equal(fib.live(), 0);
});

test('forget drops a suspended fiber; resuming it is an error', async () => {
  const { work, t, fib } = setup();
  work.set(4, async (h) => {
    await t.fiber_suspend(h);
  });
  assert.equal(await t.fiber_resume(4, 4096), 0);
  assert.equal(fib.live(), 1);
  t.fiber_forget(4);
  assert.equal(fib.live(), 0);
  await assert.rejects(t.fiber_resume(4, 0), /not suspended/);
});

test('a module without fibers attaches as none; half of it is refused', () => {
  assert.equal(createFibers().attach({}), false);
  assert.throws(() => createFibers().attach({ tw_fiber_entry() {} }), /only one of/);
});

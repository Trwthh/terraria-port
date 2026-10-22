// Work fibers of game.wasm on JSPI (HOST_ABI.md "Fibers"; port/crates/terraria-host/src/sys/threading.rs
// `wasm_fibers`; the design and its spike: web/WEB_PORT.md section 1, web/spikes/fiber-jspi/sched.mjs).
//
// A queued work item of the game (world generation, world load, save) runs on an engine stack of its own: the
// module's main stack, inside tw_frame, calls the suspending import `fiber_resume`; the first call enters the
// promising export `tw_fiber_entry` (a new stack), later calls wake the fiber where it suspended itself with
// `fiber_suspend`. The main stack waits, suspended, until the fiber gives control back or ends. Only one wasm stack
// runs at a time, all inside one tw_frame (the hand-overs are microtasks).
//
// Each fiber has its own shadow stack in linear memory (allocated by the module, `stack_top`); this file swaps the
// module's `__stack_pointer` global at every hand-over. Without the swap the main stack's frames overwrite the
// suspended fiber's (the spike's negative control).

/** The fiber imports that suspend the calling wasm stack (wrapped in `WebAssembly.Suspending`). */
export const FIBER_SUSPENDING = new Set(['fiber_resume', 'fiber_suspend']);

/**
 * @param {{ log?: (level: number, text: string) => void }} [opts]
 * @returns the `tw` imports (`fiber_resume`, `fiber_suspend`, `fiber_forget`), `attach(exports)` once the module
 *   is instantiated, `live()` (fibers started and not ended) and `stats`.
 */
export function createFibers({ log = () => {} } = {}) {
  let sp = null; // the module's __stack_pointer (WebAssembly.Global)
  let entry = null; // promising tw_fiber_entry
  /** handle -> { sp: its stack pointer while suspended, wake: resolves its fiber_suspend } */
  const fibers = new Map();
  let back = null; // resolves the main stack's pending fiber_resume
  let mainSp = 0; // the main stack's pointer when it called fiber_resume
  // ms: wall time spent inside fibers (from fiber_resume to the hand-back), all fibers together
  const stats = { started: 0, ended: 0, trapped: 0, resumes: 0, suspends: 0, ms: 0 };
  let t0 = 0;

  /** Control goes back to the main stack: 0 the fiber suspended, 1 it ended, 2 it trapped. */
  function giveBack(code) {
    const b = back;
    back = null;
    stats.ms += performance.now() - t0;
    sp.value = mainSp;
    b(code);
  }

  const imports = {
    /** Main stack: start (stack_top != 0) or continue fiber `handle`. Resolves 0 / 1 / 2 (see giveBack). */
    fiber_resume(handle, stackTop) {
      handle >>>= 0;
      stackTop >>>= 0;
      if (!entry) throw new Error('fiber_resume: game.wasm does not export tw_fiber_entry / __stack_pointer');
      if (back) throw new Error('fiber_resume: a fiber is already running');
      stats.resumes++;
      t0 = performance.now();
      mainSp = sp.value;
      return new Promise((resolve, reject) => {
        if (stackTop !== 0) {
          if (fibers.has(handle)) return reject(new Error(`fiber_resume: fiber ${handle} started twice`));
          fibers.set(handle, { sp: 0, wake: null });
          stats.started++;
          back = resolve;
          sp.value = stackTop;
          let p;
          try {
            p = entry(handle); // runs at once on the new stack, until the fiber suspends or ends
          } catch (e) {
            p = Promise.reject(e);
          }
          p.then(
            () => {
              fibers.delete(handle);
              stats.ended++;
              giveBack(1);
            },
            (e) => {
              fibers.delete(handle);
              stats.trapped++;
              log(4, `work fiber ${handle} trapped: ${(e && e.stack) || e}`);
              giveBack(2);
            },
          );
        } else {
          const f = fibers.get(handle);
          if (!f || !f.wake) return reject(new Error(`fiber_resume: fiber ${handle} is not suspended`));
          back = resolve;
          sp.value = f.sp;
          const wake = f.wake;
          f.wake = null;
          wake();
        }
      });
    },
    /** Work fiber: give control back to the main stack; resolves when resumed. */
    fiber_suspend(handle) {
      handle >>>= 0;
      const f = fibers.get(handle);
      if (!f || !back) throw new Error(`fiber_suspend: fiber ${handle} is not running`);
      stats.suspends++;
      f.sp = sp.value;
      const p = new Promise((resolve) => (f.wake = resolve));
      giveBack(0);
      return p;
    },
    /** Fiber `handle` will never run again (Thread.Abort): its suspended stack is dropped with the last reference. */
    fiber_forget(handle) {
      fibers.delete(handle >>> 0);
    },
  };

  return {
    imports,
    stats,
    live: () => fibers.size,
    /** Connects the instance. A module without fibers (no tw_fiber_entry) is fine; one with half of it is not. */
    attach(exports, promising = WebAssembly.promising) {
      const hasEntry = typeof exports.tw_fiber_entry === 'function';
      const hasSp = typeof WebAssembly !== 'undefined' && exports.__stack_pointer instanceof WebAssembly.Global;
      if (hasEntry !== hasSp) throw new Error('game.wasm exports only one of tw_fiber_entry / __stack_pointer (both are needed for work fibers)');
      if (!hasEntry) return false;
      sp = exports.__stack_pointer;
      entry = promising(exports.tw_fiber_entry);
      return true;
    },
  };
}

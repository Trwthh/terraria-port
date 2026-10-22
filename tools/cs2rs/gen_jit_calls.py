"""jit_calls.tsv: for every method of the game with a float or double IL local, which of the mscorlib methods it calls the real
32-bit JIT compiled as call instructions and which it inlined. Ground truth for the float mode "jit32" (FloatAlloc.cs:
a float value on the x87 stack is rounded at every call instruction it is live across, and only there).

Two inputs, the first one wins:
1. the dumps of the reference runs (tools/refdump_worldgen/run_refjit.sh -> work/jit/ref/<scenario>, the processes
   that wrote the reference data; jit_calls.call_answers / union): every (method, callee) pair that ran in one of
   them and got the same answer in all of them that ran it. A pair with different answers in two runs keeps the
   whole-game answer (2.) (it is listed in jit_refcalls_conflicts.tsv by gen_jit_refcalls.py).
2. one run of `tools/refdump_worldgen/bin/RefDumpWorldgenX.exe jitdump <dir> <list with *::*>` (default
   work/jit/all: every game method compiled callees first), for the methods no reference run ran.
Only mscorlib callees are kept: the JIT's inlining of a GAME method depends on the order methods were compiled in
(measured 2026-10-13: CanHitLine inlines Tile.active() when Tile.active is not compiled yet and calls it when it is),
which only the reference runs' own code answers (jit_refcalls.tsv). Even an mscorlib callee can be inlined in one order
and called in another (the caller's inlining budget); the count of such pairs is printed.
Columns: caller, callee (both `Full.Type::Name(ParamType,...)`, CLR names, nested types with '+'), IL call sites of
that callee in the caller, call instructions to it in the caller's machine code. Virtual dispatches are left out (they
are always calls). Deterministic: sorted, no timestamp.

    python gen_jit_calls.py [whole-game jitdump dir] [reference dump base dir]
"""
import os
import sys

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(here, '..', 'refdump_worldgen'))
import jit_calls  # noqa: E402

d = sys.argv[1] if len(sys.argv) > 1 else os.path.join(here, '..', '..', 'work', 'jit', 'all')
base = sys.argv[2] if len(sys.argv) > 2 else os.path.join(here, '..', '..', 'work', 'jit', 'ref')
sysonly = lambda c: c.startswith('System.')  # noqa: E731
want = lambda name, info: jit_calls.has_float_local(info)  # noqa: E731
whole, _ = jit_calls.call_answers(d, want)
dirs = jit_calls.ref_dirs(base)
ref, conflicts, _ = jit_calls.union(dirs, want) if dirs else ({}, {}, {})
rows = []
from_ref = from_all = differ = 0
for m in sorted(set(whole) | set(ref)):
    if m in ref:
        a = {c: v for c, v in ref[m].items() if sysonly(c)}
        # the pairs the runs disagree on: the whole-game answer
        a.update({c: v for c, v in whole.get(m, {}).items() if sysonly(c) and c in conflicts.get(m, {})})
        w = whole.get(m, {})
        differ += sum(1 for c, v in a.items() if c in w and w[c][1] != v[1])
        from_ref += 1 if a else 0
    else:
        a = {c: v for c, v in whole.get(m, {}).items() if sysonly(c)}
        from_all += 1 if a else 0
    rows.extend((m, c, v[0], v[1]) for c, v in a.items())
rows.sort()
out = os.path.join(here, 'jit_calls.tsv')
text = ''.join('%s\t%s\t%d\t%d\n' % r for r in rows)
old = open(out, encoding='utf-8', newline='').read() if os.path.exists(out) else None
if old != text:
    open(out, 'w', encoding='utf-8', newline='').write(text)
inl = sum(1 for r in rows if r[3] == 0)
full = sum(1 for r in rows if r[3] >= r[2])
print('jit_calls.tsv: %d (caller, callee) pairs: %d all inlined, %d all calls, %d mixed (%s); callers from the reference runs %d, '
      'from the whole-game dump %d; mscorlib pairs where a reference run and the whole-game dump differ: %d' % (
          len(rows), inl, full, len(rows) - inl - full, 'unchanged' if old == text else 'written', from_ref, from_all, differ))

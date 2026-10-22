"""jit_bodies.tsv: at which call sites the real 32-bit JIT compiled a call instruction to an XNA member, in the processes
that wrote the reference data the gates compare. Input of config `jitBodySites` (float mode "jit32"): at such a site the
member's own compiled body runs, and config `jitBodies` gives that body's x87 evaluation (Vector2.Length: `x*x + y*y` at
double precision, `fsqrt`, rounded to float once), in place of the f32 `nativeMembers` template.

Input: the dumps of the reference runs themselves (tools/refdump_worldgen/run_refjit.sh -> work/jit/ref/<scenario>;
each run reproduced its refdata byte for byte, then read the code of every method that ran in it). The legacy JIT's
inlining depends on what was compiled before, so only those processes' code says what produced the reference data.
A (method, XNA callee) pair with a different answer in two runs is left out (listed by gen_jit_refcalls.py in
jit_refcalls_conflicts.tsv); the f32 template stays there.
Columns: caller, callee (`Full.Type::Name(ParamType,...)`), IL call sites of that callee in the caller, call
instructions to it in the caller's machine code. Only XNA callees (Microsoft.Xna.Framework.*). Deterministic: sorted,
no timestamp.

    python gen_jit_bodies.py [dump base dir, default work/jit/ref]
"""
import os
import sys

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(here, '..', 'refdump_worldgen'))
import jit_calls  # noqa: E402

base = sys.argv[1] if len(sys.argv) > 1 else os.path.join(here, '..', '..', 'work', 'jit', 'ref')
dirs = jit_calls.ref_dirs(base)
if not dirs:
    sys.exit('no dumps under ' + base + ' (run tools/refdump_worldgen/run_refjit.sh)')
xna = lambda c: c.startswith('Microsoft.Xna.Framework.')  # noqa: E731
rows_m, conflicts, _ = jit_calls.union(dirs, None, xna)
rows = sorted((m, c, v[0], v[1]) for m, a in rows_m.items() for c, v in a.items())
out = os.path.join(here, 'jit_bodies.tsv')
text = ''.join('%s\t%s\t%d\t%d\n' % r for r in rows)
old = open(out, encoding='utf-8', newline='').read() if os.path.exists(out) else None
if old != text:
    open(out, 'w', encoding='utf-8', newline='').write(text)
full = sum(1 for r in rows if r[3] >= r[2])
print('jit_bodies.tsv: %d (caller, XNA callee) pairs, %d with a call instruction at every site (%s); %d callers with a pair left out (different answers in two runs)' % (
    len(rows), full, 'unchanged' if old == text else 'written', len(conflicts)))

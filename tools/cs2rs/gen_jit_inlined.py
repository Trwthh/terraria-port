"""jit_inlined.tsv: the game methods with a float parameter that the real 32-bit JIT inlined into a caller at every IL
site, in the processes that wrote the reference data (the same dumps as jit_refcalls.tsv). Input of config
`jitInlined` (float mode "jit32", Emit.cs `InlinedArgsTwin`): the JIT substitutes a caller's local for an inlinee's
parameter that the inlinee never writes, so a float local the caller keeps on the x87 stack enters the inlined body
unrounded (FLOAT_RULE.md section 10, item 26: `DungeonShapes.HourglassRoom.Perform` passes `percent` to the inlined
`Utils.WrappedLerp`, `fmul dword [2.0]` on the register).

Unlike jit_refcalls.tsv, an inlined callee whose body still executes a call instruction (WrappedLerp calls
MathHelper.Lerp) counts as inlined here: only a call instruction to the callee ITSELF makes a site a call. The same
care as jit_calls.call_answers: a callee without a named call instruction in a caller that has calls the dump cannot
name is left out unless the callee's assembly names its calls; a pair the runs answer differently is left out.

Columns: caller, callee (`Full.Type::Name(ParamType,...)`), IL call sites of the callee in the caller. Deterministic.

    python gen_jit_inlined.py [dump base dir, default work/jit/ref]
"""
import os
import sys
from collections import defaultdict

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(here, '..', 'refdump_worldgen'))
import jit_calls  # noqa: E402


def float_param(callee):
    sig = callee.split('(', 1)[1].rstrip(')') if '(' in callee else ''
    return any(p == 'System.Single' for p in sig.split(','))


def answers(d):
    """{caller: {callee: (sites, direct call instructions)}} of one dump: callers that ran with a float local, game
    callees with a float parameter."""
    per = jit_calls.load_json(os.path.join(d, 'jit_method_callees.json'))
    jit_calls.load_cells(d)
    by_addr = jit_calls.names(d)
    named = jit_calls.NAMED_RAN if os.path.exists(os.path.join(d, 'jit_targets.json')) else jit_calls.NAMED
    out = {}
    for name, addr, code, info in jit_calls.methods(d):
        if info and info.get('compiled_before') is False:
            continue
        if not jit_calls.has_float_local(info):
            continue
        sites = defaultdict(int)
        for e in per.get(name, []):
            kind, callee = e.split(' ', 1)
            if kind.endswith('-virtual') or kind.startswith('ld'):
                continue
            if callee.startswith('Terraria.') and float_param(callee):
                sites[callee] += 1
        if not sites:
            continue
        calls = defaultdict(int)
        unnamed = 0
        for t in jit_calls.machine_calls(addr, code, bool(info and info.get('exact_size'))):
            if t in by_addr:
                calls[by_addr[t]] += 1
            else:
                unnamed += 1
        ans = {}
        for callee, n in sites.items():
            got = calls.get(callee, 0)
            if got == 0 and unnamed > 0 and not callee.startswith(named):
                continue
            ans[callee] = (n, got)
        if ans:
            out[name] = ans
    return out


def main():
    base = sys.argv[1] if len(sys.argv) > 1 else os.path.join(here, '..', '..', 'work', 'jit', 'ref')
    dirs = jit_calls.ref_dirs(base)
    if not dirs:
        sys.exit('no dumps under ' + base)

    def union(ds):
        merged, bad = {}, set()
        for d in ds:
            for m, a in answers(d).items():
                for c, v in a.items():
                    k = (m, c)
                    if k in merged and merged[k][1] != v[1]:
                        bad.add(k)
                    merged.setdefault(k, v)
        return merged, bad

    rows, bad = union(list(dirs))
    seen = {m for m, _ in rows} | {m for m, _ in bad}
    fill = jit_calls.fill_dirs(dirs)
    if fill:
        r2, b2 = union(fill)
        for k, v in r2.items():
            if k[0] in seen:
                continue
            if k in b2:
                bad.add(k)
            else:
                rows[k] = v
    out = sorted((m, c, v[0]) for (m, c), v in rows.items() if (m, c) not in bad and v[1] == 0)
    text = ''.join('%s\t%s\t%d\n' % r for r in out)
    path = os.path.join(here, 'jit_inlined.tsv')
    old = open(path, encoding='utf-8', newline='').read() if os.path.exists(path) else None
    if old != text:
        open(path, 'w', encoding='utf-8', newline='').write(text)
    print('jit_inlined.tsv: %d runs (+%d client), %d (caller, game callee with a float parameter) pairs inlined at every '
          'site, %d pairs with different answers left out (%s)' % (len(dirs), len(fill), len(out), len(bad),
                                                                   'unchanged' if old == text else 'written'))


if __name__ == '__main__':
    main()

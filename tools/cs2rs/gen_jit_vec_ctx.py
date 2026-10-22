"""jit_vec_ctx.tsv: what the XNA Vector2 operators of an INLINED game method became inside the caller that inlined it,
per call site, read from the machine code of the reference runs (work/jit/ref/<scenario>). Input of config
`jitVecCtx` (float mode "jit32", Emit.cs VecTwin; FLOAT_RULE.md section 6).

The legacy JIT decides the inlining of an inlinee's own callees again inside every caller. `Utils.FactorAcceleration`
compiled on its own keeps `descendOfProjectile * (num * num) / 2f / timeToInterception` and the subtraction on the x87
stack (jit_vec_temps.tsv "stack"); inlined into `Projectile.RainbowBoulder_JumpToPosition` (its first call site, in the
loop) it calls `op_Division` twice and `op_Subtraction` (refdata/entities proj 1047, tick 26: velocity.Y).

Columns: caller, inlined callee, k (0-based index of the callee's IL call site among the caller's call sites of that
callee, IL order), what each Vector2 operator of the callee is there: `op(params)=c` (a call instruction at each of
its sites) or `=i` (inlined at each), `;`-separated, then `|stack` / `|memory` when one is inlined (its results on the
x87 stack / in memory, as jit_vec_temps.tsv decides it, read in the code of that site). A site is attributed through the CLR's IL-to-native map of the caller (jit_debug.bin bounds: each call
instruction belongs to the statement whose native range holds it); a site that shares its statement with another call
site of the same callee, or whose operators are partly called and partly inlined, is left out. A (caller, callee, k)
with different answers in two runs is left out. Only callees with Vector2 operator IL sites; only inlined sites.

    python gen_jit_vec_ctx.py [dump base dir, default work/jit/ref]
"""
import bisect
import json
import os
import struct
import sys
from collections import Counter, defaultdict

import capstone

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(here, '..', 'refdump_worldgen'))
import jit_calls  # noqa: E402
import jit_debuginfo  # noqa: E402

base = sys.argv[1] if len(sys.argv) > 1 else os.path.join(here, '..', '..', 'work', 'jit', 'ref')
dirs = jit_calls.ref_dirs(base)
if not dirs:
    sys.exit('no dumps under ' + base + ' (run tools/refdump_worldgen/run_refjit.sh)')
OPS = ('op_Addition(', 'op_Subtraction(', 'op_Multiply(', 'op_Division(', 'op_UnaryNegation(')
VEC = 'Microsoft.Xna.Framework.Vector2::'


def is_op(x):
    return x.startswith(VEC) and any(o in x for o in OPS)


def il_call_offsets(il_hex):
    """IL offsets of call / callvirt / newobj, in order (jit_calls.il_calls with offsets)."""
    il = bytes.fromhex(il_hex)
    out = []
    i = 0
    one = {0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x1F, 0xDE} | set(range(0x2B, 0x38))
    four = {0x20, 0x22, 0x28, 0x29, 0x27, 0x6F, 0x70, 0x71, 0x72, 0x73, 0x74, 0x75, 0x79, 0x8C, 0x8D, 0x8F, 0xA3, 0xA4,
            0xA5, 0xC2, 0xC6, 0xD0, 0xDD} | set(range(0x38, 0x45)) | set(range(0x7B, 0x82))
    while i < len(il):
        op = il[i]
        if op == 0xFE:
            op2 = il[i + 1]
            size = 4 if op2 in (0x06, 0x07, 0x15, 0x16, 0x1C) else 2 if 0x09 <= op2 <= 0x0E else 1 if op2 in (0x12, 0x19) else 0
            i += 2 + size
            continue
        if op in (0x28, 0x6F, 0x73):
            out.append(i)
        if op == 0x45:
            n = struct.unpack_from('<i', il, i + 1)[0]
            i += 1 + 4 + 4 * n
            continue
        i += 1 + (1 if op in one else 4 if op in four else 8 if op in (0x21, 0x23) else 0)
    return out


md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)
answers = defaultdict(dict)   # (caller, callee, k) -> {scenario: mode}
ran_in_reference = set()
for d in list(dirs) + jit_calls.fill_dirs(dirs):
    primary = d in dirs
    per = jit_calls.load_json(os.path.join(d, 'jit_method_callees.json'))
    dbg = jit_debuginfo.load(d)
    jit_calls.load_cells(d)
    by_addr = jit_calls.names(d)
    scen = os.path.basename(d)

    def il(m):
        return [e.split(' ', 1)[1] for e in per.get(m, []) if not e.startswith('ld')]
    for name, addr, code, info in jit_calls.methods(d):
        if primary:
            ran_in_reference.add(name)
        elif name in ran_in_reference:
            continue   # a client run (jit_calls.Dirs) only answers for methods no reference run ran
        if not info or info.get('compiled_before') is False or name not in dbg or not info.get('il'):
            continue
        callees = il(name)
        offs = il_call_offsets(info['il'])
        if len(offs) != len(callees):
            continue
        # inlined game callees with Vector2 operator sites of their own
        cand = [g for g in set(callees) if g.startswith('Terraria.') and any(is_op(x) for x in il(g))]
        if not cand:
            continue
        bounds, _ = jit_debuginfo.parse(dbg[name])
        stmt_ils = sorted(set(b[1] for b in bounds if b[1] >= 0))
        if not stmt_ils:
            continue
        # the statement of a native offset: the bound with the greatest native offset at or below it
        bs = sorted(bounds)
        starts = [b[0] for b in bs]

        def stmt_of(o):
            j = bisect.bisect_right(stmt_ils, o) - 1
            return stmt_ils[j] if j >= 0 else None
        ins = list(md.disasm(code, addr))
        # call instructions with their statement
        calls_at = defaultdict(Counter)
        zero_at = defaultdict(int)
        for k, a in enumerate(ins):
            off = a.address - addr
            j = bisect.bisect_right(starts, off) - 1
            if j < 0 or bs[j][1] < 0:
                continue
            st = bs[j][1]
            if a.mnemonic == 'call':
                t = None
                if a.op_str.startswith('0x'):
                    t = int(a.op_str, 16)
                elif a.op_str.startswith('dword ptr [0x') and a.op_str.endswith(']'):
                    t = jit_calls.CELLS.get(int(a.op_str[len('dword ptr ['):-1], 16))
                if t is not None and t in by_addr:
                    calls_at[st][by_addr[t]] += 1
            if a.mnemonic == 'xorps' and a.op_str == 'xmm0, xmm0' and k + 1 < len(ins) and ins[k + 1].mnemonic == 'movq' and ins[k + 1].op_str.endswith('], xmm0'):
                zero_at[st] += 1
        site_stmt = [stmt_of(o) for o in offs]
        for g in cand:
            idx = [i for i, c in enumerate(callees) if c == g]
            gops = Counter(x for x in il(g) if is_op(x))
            for k, i in enumerate(idx):
                st = site_stmt[i]
                if st is None:
                    continue
                same = [j for j, c in enumerate(callees) if site_stmt[j] == st]
                if sum(1 for j in same if callees[j] == g) != 1:
                    continue
                if calls_at[st][g] > 0:
                    continue   # called here: its own code runs
                ops, ok = [], True
                for x, n in sorted(gops.items()):
                    own = sum(1 for j in same if callees[j] == x)
                    got = calls_at[st][x]
                    if own:
                        # the caller's own sites of that operator in the same statement: decided only when all are calls
                        a = 'c' if got >= n + own else None
                    else:
                        a = 'c' if got >= n else 'i' if got == 0 else None
                    if a is None:
                        ok = False
                        break
                    ops.append(x.split('::', 1)[1] + '=' + a)
                if not ok:
                    continue
                m = ';'.join(ops) + ('|' + ('memory' if zero_at[st] else 'stack') if any(o.endswith('=i') for o in ops) else '')
                answers[(name, g, k)][scen] = m
rows, conflicts = [], 0
for key in sorted(answers):
    vals = set(answers[key].values())
    if len(vals) != 1:
        conflicts += 1
        continue
    rows.append(key + (vals.pop(),))
out = os.path.join(here, 'jit_vec_ctx.tsv')
text = ''.join('%s\t%s\t%d\t%s\n' % r for r in rows)
old = open(out, encoding='utf-8', newline='').read() if os.path.exists(out) else None
if old != text:
    open(out, 'w', encoding='utf-8', newline='').write(text)
c = Counter('inlined operators' if '=i' in r[3] else 'all operators called' for r in rows)
print('jit_vec_ctx.tsv: %d inlined call sites of game methods with Vector2 operators (%s; %s); %d left out (different answers in two runs)' % (
    len(rows), ', '.join('%s %d' % kv for kv in sorted(c.items())), 'unchanged' if old == text else 'written', conflicts))

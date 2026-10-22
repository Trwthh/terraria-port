"""jit_refcalls.tsv: at which call sites the real 32-bit JIT compiled a call instruction, and which it inlined, in the
processes that wrote the reference data the gates compare. Input of config `jitRefCalls` (float mode "jit32",
FloatAlloc.cs): for the pairs it lists this answer replaces both jit_calls.tsv and the size rule as the place where x87
values are rounded (a value on the x87 stack is spilled to a 4-byte slot at every call instruction it is live across,
and only there). The size rule stays the register-allocation input.

Input: the dumps of the reference runs themselves (tools/refdump_worldgen/run_refjit.sh -> work/jit/ref/<scenario>:
p1, framed, collision, light, sky, liquid, ticks, worldgen; each run reproduced its refdata byte for byte,
refjit_compare.py, and then read the code of every game / ReLogic / XNA method that ran in it, JitDump.cs DumpRan).
The legacy JIT's inlining of GAME methods depends on what was compiled before (jit_calls.tsv took only mscorlib from
a whole-game precompile for that reason), so only those processes' code says what produced the reference data.
Callers: the methods that ran, with a float or double IL local. Virtual dispatches are left out (always calls).

Several runs: a method that ran in more than one of them and got a different answer for a callee (another number of
call instructions: the JIT compiled it differently in the two processes) has no single answer in the original for
that callee: that pair is left out (the size rule and jit_calls.tsv decide there) and listed in jit_refcalls_conflicts.tsv with
each run's answer; the caller's other pairs stay. Answers that the runs agree on are merged.

Columns: caller, callee (`Full.Type::Name(ParamType,...)`), IL call sites of that callee in the caller, call
instructions to it in the caller's machine code (an inlined callee whose body still executes a call counts as a call at
its sites). Deterministic: sorted, no timestamp.

    python gen_jit_refcalls.py [dump base dir, default work/jit/ref]
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
rows, conflicts, seen = jit_calls.union(dirs, lambda name, info: jit_calls.has_float_local(info))


def write(path, text):
    old = open(path, encoding='utf-8', newline='').read() if os.path.exists(path) else None
    if old != text:
        open(path, 'w', encoding='utf-8', newline='').write(text)
    return old == text


out = sorted((m, c, v[0], v[1]) for m, a in rows.items() for c, v in a.items())
same = write(os.path.join(here, 'jit_refcalls.tsv'), ''.join('%s\t%s\t%d\t%d\n' % r for r in out))
cl = []
for m in sorted(conflicts):
    for c in sorted(conflicts[m]):
        cl.append('%s\t%s\t%s\n' % (m, c, ' '.join('%s=%d/%d' % (s, v[0], v[1]) for s, v in sorted(conflicts[m][c].items()))))
write(os.path.join(here, 'jit_refcalls_conflicts.tsv'), ''.join(cl))
inl = sum(1 for r in out if r[3] == 0)
full = sum(1 for r in out if r[3] >= r[2])
print('jit_refcalls.tsv: %d runs, %d callers with float locals ran, %d with one answer: %d (caller, callee) pairs: %d all inlined, '
      '%d all calls, %d mixed (%s); %d callers with different answers in two runs (jit_refcalls_conflicts.tsv, %d pairs)' % (
          len(dirs), len(seen), len(rows), len(out), inl, full, len(out) - inl - full, 'unchanged' if same else 'written',
          len(conflicts), len(cl)))


# ---- jit_refcall_sites.tsv: the sites of the mixed pairs ------------------------------------------------------------
# A pair with call instructions at some of its sites only (Dust.GetAlpha: Color.get_R at 14 sites, 2 calls) has no
# answer for the whole method; the size rule decided there (a call everywhere: `num` rounded before
# `(float)newColor.R * num`, where the JIT inlined get_R and kept num unrounded on the x87 stack). Per site: each call
# instruction of the caller's code -> its IL offset (the CLR's IL / native map, jit_debuginfo.py: the last boundary at
# or before the instruction; CALL_INSTRUCTION boundaries name the call itself) -> the first IL site of that callee at or
# after it not yet taken; each IL site -> the decompiled statement of its sequence point (ILSpy's PDB placed in
# decompile\, work/pdb/pdb_seq.tsv, `tmeta pdbseq`) and its ordinal among that callee's sites in the statement.
# A pair is written only where every site maps to a sequence point on the start of a syntax node, the call
# instructions found add up to the pair's count, and all runs that ran the caller agree.
# Columns: caller, callee, file (relative to the project), start line, start column, end line, end column (the
# statement's span, 1-based, end exclusive), ordinal of the site in the statement (IL order), 1 = call instruction.
import json  # noqa: E402
import capstone  # noqa: E402
from collections import defaultdict  # noqa: E402
import jit_debuginfo  # noqa: E402


def il_fnv(il):
    """FNV-1a 64 of the IL with every metadata token operand zeroed (tmeta PdbLocals.Normalized, gen_jit_float_alloc.py)."""
    one = {0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x1F, 0xDE} | set(range(0x2B, 0x38))
    four = {0x20, 0x22, 0x28, 0x29, 0x27, 0x6F, 0x70, 0x71, 0x72, 0x73, 0x74, 0x75, 0x79, 0x8C, 0x8D, 0x8F, 0xA3, 0xA4,
            0xA5, 0xC2, 0xC6, 0xD0, 0xDD} | set(range(0x38, 0x45)) | set(range(0x7B, 0x82))
    tokens = {0x27, 0x28, 0x29, 0x6F, 0x70, 0x71, 0x72, 0x73, 0x74, 0x75, 0x79, 0x7B, 0x7C, 0x7D, 0x7E, 0x7F, 0x80, 0x81,
              0x8C, 0x8D, 0x8F, 0xA3, 0xA4, 0xA5, 0xC2, 0xC6, 0xD0}
    o = bytearray(il)
    i = 0
    while i < len(o):
        op = o[i]
        if op == 0xFE:
            op2 = o[i + 1]
            size = 4 if op2 in (0x06, 0x07, 0x15, 0x16, 0x1C) else 2 if 0x09 <= op2 <= 0x0E else 1 if op2 in (0x12, 0x19) else 0
            if size == 4:
                o[i + 2:i + 6] = bytes(4)
            i += 2 + size
            continue
        if op == 0x45:
            i += 1 + 4 + 4 * int.from_bytes(o[i + 1:i + 5], 'little')
            continue
        if op in tokens:
            o[i + 1:i + 5] = bytes(4)
        i += 1 + (1 if op in one else 4 if op in four else 8 if op in (0x21, 0x23) else 0)
    h = 0xcbf29ce484222325
    for x in o:
        h = ((h ^ x) * 0x100000001b3) & 0xFFFFFFFFFFFFFFFF
    return '%016x' % h


seqf = os.path.join(here, '..', '..', 'work', 'pdb', 'pdb_seq.tsv')
mixed = defaultdict(set)
for m, a in rows.items():
    for c, v in a.items():
        if 0 < v[1] < v[0]:
            mixed[m].add(c)
site_rows = []
site_stats = defaultdict(int)
if not os.path.exists(seqf):
    print('jit_refcall_sites.tsv: no %s (tmeta pdbseq), left as it is' % seqf)
else:
    seq = defaultdict(list)
    for line in open(seqf, encoding='utf-8'):
        t, mn, h, off, f, sl, sc, el, ec, ok = line.rstrip('\n').split('\t')
        seq[(t, mn, h)].append((int(off), f, int(sl), int(sc), int(el), int(ec), ok == '1'))
    for k in seq:
        seq[k].sort()

    def il_sites(il_hex):
        """(IL offset, opcode) of every call / callvirt / newobj (jit_calls.il_calls with offsets)."""
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
                n = int.from_bytes(il[i + 1:i + 5], 'little')
                i += 1 + 4 + 4 * n
                continue
            i += 1 + (1 if op in one else 4 if op in four else 8 if op in (0x21, 0x23) else 0)
        return out

    def machine_sites(addr, code, exact):
        """(native offset, target) of every direct call, as jit_calls.machine_calls decodes them."""
        md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)
        insns = list(md.disasm(code, addr))
        last = 0
        for k, ins in enumerate(insns):
            if ins.mnemonic == 'ret' or exact:
                last = k
        out = []
        for ins in insns[:last + 1]:
            if ins.mnemonic != 'call':
                continue
            if ins.op_str.startswith('0x'):
                out.append((ins.address - addr, int(ins.op_str, 16)))
            elif ins.op_str.startswith('dword ptr [0x') and ins.op_str.endswith(']'):
                cell = int(ins.op_str[len('dword ptr ['):-1], 16)
                out.append((ins.address - addr, jit_calls.CELLS.get(cell, -cell)))
        return out

    def answer(d, caller, callees, per, by_addr, code_of, dbg):
        """{callee: [(file, sl, sc, el, ec, ordinal, call)]} of one run, None for a callee it cannot place."""
        addr, code, info = code_of[caller]
        entries = [e.split(' ', 1) for e in per.get(caller, [])]
        entries = [(k, n) for k, n in entries if not k.startswith('ld')]
        offs = il_sites(info['il'])
        if len(offs) != len(entries) or caller not in dbg:
            return {}
        t, mn = caller.split('(')[0].rsplit('::', 1)
        sps = seq.get((t, mn, il_fnv(bytes.fromhex(info['il']))))
        if not sps:
            return {}
        bounds = sorted(b for b in jit_debuginfo.parse(dbg[caller])[0] if b[1] >= 0)
        calls = machine_sites(addr, code, bool(info.get('exact_size')))
        out = {}
        for callee in callees:
            sites = [o for o, (k, n) in zip(offs, entries) if n == callee and not k.endswith('-virtual')]
            taken = set()
            for nat, target in calls:
                if by_addr.get(target) != callee:
                    continue
                b = [x for x in bounds if x[0] <= nat]
                if not b:
                    continue
                o = b[-1][1]
                cand = [s for s in sites if s >= o and s not in taken]
                if cand:
                    taken.add(cand[0])
            rows_ = []
            ordinal = defaultdict(int)
            ok = True
            for s in sites:
                sp = [x for x in sps if x[0] <= s]
                if not sp or not sp[-1][6]:
                    ok = False
                    break
                key = sp[-1][1:6]
                rows_.append(key + (ordinal[key], 1 if s in taken else 0))
                ordinal[key] += 1
            out[callee] = rows_ if ok and len(sites) == rows[caller][callee][0] and len(taken) == rows[caller][callee][1] else None
        return out

    primary = list(dirs)
    fill = jit_calls.fill_dirs(dirs)
    found = defaultdict(list)   # caller -> callee -> answers of each run
    seen_primary = set()
    for group in (primary, fill):
        for d in group:
            per = jit_calls.load_json(os.path.join(d, 'jit_method_callees.json'))
            jit_calls.load_cells(d)
            by_addr = jit_calls.names(d)
            want = [m for m in mixed if (group is primary or m not in seen_primary)]
            code_of = {n: (a, c, i) for n, a, c, i in jit_calls.methods(d) if n in mixed and i and i.get('il') and i.get('compiled_before') is not False}
            if not code_of:
                continue
            dbg = jit_debuginfo.load(d) if os.path.exists(os.path.join(d, 'jit_debug.bin')) else {}
            for caller in want:
                if caller not in code_of:
                    continue
                if group is primary:
                    seen_primary.add(caller)
                found[caller].append(answer(d, caller, mixed[caller], per, by_addr, code_of, dbg))
    for caller in sorted(mixed):
        for callee in sorted(mixed[caller]):
            ans = [a.get(callee) for a in found.get(caller, [])]
            if not ans or any(a is None for a in ans) or any(a != ans[0] for a in ans):
                site_stats['unplaced'] += 1
                continue
            site_stats['placed'] += 1
            for r in ans[0]:
                site_rows.append('%s\t%s\t%s\t%d\t%d\t%d\t%d\t%d\t%d\n' % ((caller, callee) + r))
    same2 = write(os.path.join(here, 'jit_refcall_sites.tsv'), ''.join(site_rows))
    print('jit_refcall_sites.tsv: %d mixed pairs placed site by site (%d sites), %d not (%s)' % (
        site_stats['placed'], len(site_rows), site_stats['unplaced'], 'unchanged' if same2 else 'written'))

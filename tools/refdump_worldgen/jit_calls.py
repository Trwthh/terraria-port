#!/usr/bin/env python3
"""Which calls the real 32-bit JIT compiled as call instructions and which it inlined, per method (needs capstone).

usage: python jit_calls.py <dir of a jitdump run> [NAME_SUBSTRING] [--table]

Reads jit_code.bin / jit_code.json / jit_callees.json of ONE `RefDumpWorldgenX.exe jitdump` run (addresses differ between
runs: the three files must come from the same run). For every dumped method: the callees its IL names
(call / callvirt / newobj, resolved by the harness), and for each whether a machine `call rel32` to that callee's
function pointer (or the code behind its stub) is in the method's code. A callee with no such call was inlined (or is
reached only through a virtual / interface dispatch: `callvirt` of a virtual method is listed as "dispatch?").

--table: one line per callee over all methods: "<callee> calls=N inlined=M", the input for the translator's list of
library methods the JIT inlines (FLOAT_RULE.md section 6: a call instruction rounds the float values that are live
on the x87 stack; an inlined callee does not).
"""
import glob
import json
import os
import struct
import sys
from collections import defaultdict

import capstone

import jit_debuginfo  # noqa: E402  (canonical names)


def load_json(path):
    """A dump's JSON file, with the game assembly named canonically: the server runs' TerrariaServer.exe and the client
    runs' Terraria.exe are one assembly to the translator, and generic arguments carry the assembly's name
    (`List`1[[Terraria.DataStructures.DrawData, TerrariaServer, Version=1.4.5.8, ..]]`); every key reads `Terraria, `; the stand-in XNA assemblies' 0.0.0.0 reads 4.0.0.0."""
    with open(path, encoding="utf-8") as f:
        text = f.read().replace("TerrariaServer, Version=", "Terraria, Version=")
        # the client runs' XNA stand-in (tools/refdump_draw/xna) reports version 0.0.0.0 in generic arguments
        for n in ("Graphics", "Game"):
            text = text.replace("Microsoft.Xna.Framework.%s, Version=0.0.0.0," % n, "Microsoft.Xna.Framework.%s, Version=4.0.0.0," % n)
        return json.loads(text)

OPS = {0x28: "call", 0x6F: "callvirt", 0x73: "newobj"}


def il_calls(il_hex):
    """Call tokens of an IL body, in order (opcode sizes as JitDump.cs)."""
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
        if op in OPS:
            out.append((OPS[op], struct.unpack_from("<i", il, i + 1)[0]))
        if op == 0x45:
            n = struct.unpack_from("<i", il, i + 1)[0]
            i += 1 + 4 + 4 * n
            continue
        i += 1 + (1 if op in one else 4 if op in four else 8 if op in (0x21, 0x23) else 0)
    return out


def methods(d):
    """(name, address, code, json info) per dumped method. The bytes read after a method's start can run into the
    next method the JIT placed behind it (a whole-game run fills the code heap contiguously): each method is cut at
    the lowest start address of another dumped method above its own."""
    rows = list(_raw_methods(d))
    starts = sorted(set(r[1] for r in rows))
    import bisect
    for name, addr, code, info in rows:
        i = bisect.bisect_right(starts, addr)
        if i < len(starts) and starts[i] - addr < len(code):
            code = code[:starts[i] - addr]
        yield name, addr, code, info


def _raw_methods(d):
    b = open(os.path.join(d, "jit_code.bin"), "rb").read()
    assert b[:4] == b"JITC"
    n = struct.unpack_from("<I", b, 8)[0]
    o = 12
    info = {m["name"]: m for m in load_json(os.path.join(d, "jit_code.json"))}
    for _ in range(n):
        ln = struct.unpack_from("<I", b, o)[0]
        # canonical like jit_code.json (load_json): the server runs' generic argument types name TerrariaServer
        name = jit_debuginfo.canonical(b[o + 4:o + 4 + ln].decode())
        o += 4 + ln
        addr, nb = struct.unpack_from("<II", b, o)
        o += 8
        yield name, addr, b[o:o + nb], info.get(name)
        o += nb


CELLS = {}


def load_cells(d):
    """jit_cells.json of the run (call cells -> the address in them); empty for runs before it existed."""
    global CELLS
    f = os.path.join(d, "jit_cells.json")
    CELLS = {int(k): v for k, v in json.load(open(f)).items()} if os.path.exists(f) else {}


def names(d, callees=None):
    """{call target address: method name} of a dump: the function pointers / entries of jit_callees.json, plus
    jit_targets.json where the dump has it (DumpRan, 2026-10-14: precodes of callees compiled after the caller,
    followed in-process to the method they lead to)."""
    if callees is None:
        callees = load_json(os.path.join(d, "jit_callees.json"))
    by_addr = {}
    for c in callees:
        for a in (c["fp"], c["entry"]):
            if a:
                by_addr[a] = c["name"]
    f = os.path.join(d, "jit_targets.json")
    if os.path.exists(f):
        for k, v in load_json(f).items():
            by_addr.setdefault(int(k), v)
    return by_addr


def machine_calls(addr, code, exact=False):
    """Targets of `call rel32` / `call [cell]` (straight-line decode; good enough for JIT output). `exact`: the code is
    exactly the method (DumpRan reads the size from the method's GC info; jit_code.json "exact_size"), all of it is
    decoded, throw blocks after the last ret included; otherwise up to the last reachable ret."""
    md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)
    targets = []
    last_ret = 0
    insns = list(md.disasm(code, addr))
    for k, ins in enumerate(insns):
        if ins.mnemonic == "ret" or exact:
            last_ret = k
    for ins in insns[:last_ret + 1]:
        if ins.mnemonic == "call" and ins.op_str.startswith("0x"):
            targets.append(int(ins.op_str, 16))
        elif ins.mnemonic == "call" and ins.op_str.startswith("dword ptr [0x") and ins.op_str.endswith("]"):
            cell = int(ins.op_str[len("dword ptr ["):-1], 16)
            targets.append(CELLS.get(cell, -cell))
    return targets



# Callee prefixes whose calls are named in a dump (so "no call instruction to it" means inlined): the game and mscorlib
# always (compiled callees first / precompiled); with jit_targets.json (DumpRan) also ReLogic and XNA, whose precodes
# and code headers are resolved in-process.
NAMED = ('Terraria.', 'System.')
NAMED_RAN = NAMED + ('ReLogic.', 'Microsoft.Xna.Framework.')


def has_float_local(info):
    return bool(info) and any(l['type'] in ('System.Single', 'System.Double') for l in info['locals'])


def call_answers(d, want_caller=None):
    """{caller: {callee: (IL call sites, call instructions)}} of one dump, for the methods that ran in its process
    (jit_code.json "compiled_before" not false) and `want_caller(name, info)`. Virtual dispatches and ldftn are left
    out. A callee with no call instruction while the caller has calls that are not named is left out unless the callee
    is of an assembly whose calls are named (NAMED / NAMED_RAN). An inlined callee whose body still executes a call
    instruction (FastRandom.Next(int, int) inlined, calling Next(int)) counts as a call at all its sites, when that
    call can be attributed to it alone. Also returns {caller: number of call instructions not named} as second value."""
    per = load_json(os.path.join(d, "jit_method_callees.json"))
    load_cells(d)
    by_addr = names(d)
    named = NAMED_RAN if os.path.exists(os.path.join(d, "jit_targets.json")) else NAMED

    def il(m):
        return [e.split(" ", 1)[1] for e in per.get(m, []) if not e.split(" ", 1)[0].endswith("-virtual") and not e.startswith("ld")]

    def inner(m):
        one = il(m)
        return set(one) | set(c for o in one for c in il(o))

    out, unnamed_of = {}, {}
    for name, addr, code, info in methods(d):
        if info and info.get("compiled_before") is False:
            continue
        if want_caller and not want_caller(name, info):
            continue
        sites = defaultdict(int)
        for c in il(name):
            sites[c] += 1
        if not sites:
            continue
        calls = defaultdict(int)
        unnamed = 0
        for t in machine_calls(addr, code, bool(info and info.get("exact_size"))):
            if t in by_addr:
                calls[by_addr[t]] += 1
            else:
                unnamed += 1
        ans = {}
        for callee, n in sites.items():
            got = calls.get(callee, 0)
            # the accessors of a multi-dimensional array (`Terraria.Tile[,]::Get`) are runtime methods without a
            # MethodDesc of the game: their call target is not reliably named (collision 2 calls / ticks 0 for the
            # same code shape), so "no named call" proves nothing there either
            if got == 0 and unnamed > 0 and (not callee.startswith(named) or '[,' in callee.split('::')[0]):
                continue
            if got == 0:
                inn = inner(callee)
                hit = [c for c in inn if calls.get(c, 0) > 0 and c not in sites]
                if hit:
                    others = [o for o in sites if o != callee and calls.get(o, 0) == 0 and any(c in inner(o) for c in hit)]
                    if others:
                        continue
                    got = n
                elif inn and unnamed > 0:
                    continue
            ans[callee] = (n, got)
        out[name] = ans
        unnamed_of[name] = unnamed
    return out, unnamed_of


class Dirs(list):
    """The dump folders of the reference runs, sorted by scenario; `fill`: the client runs' dumps (the draw reference,
    tools/refdump_draw/run_refjit_draw.sh -> <base>/../client/<scenario>), which only answer for methods no reference
    run ran. A client process compiles in another order (and against the XNA stand-in), so for a method both ran its
    answers can differ ("do not inline" after a failed attempt): the server runs' answers keep precedence there."""
    fill = ()


def ref_dirs(base):
    """The dumps of the reference runs (run_refjit.sh): <base>/<scenario>/jit_code.json, sorted by scenario; with
    `.fill` the client runs' dumps next to <base> (see Dirs)."""
    d = Dirs(sorted(os.path.dirname(p) for p in glob.glob(os.path.join(base, "*", "jit_code.json"))))
    d.fill = sorted(os.path.dirname(p) for p in glob.glob(os.path.join(os.path.dirname(os.path.normpath(base)), "client", "*", "jit_code.json")))
    return d


def fill_dirs(dirs):
    """The client runs' dumps of a `ref_dirs` result (empty for a plain list)."""
    return list(getattr(dirs, "fill", ()))


def union(dirs, want_caller=None, want_callee=None):
    """`union_of` over the reference runs, then over the client runs (`ref_dirs(..).fill`) for the methods none of
    the reference runs ran."""
    rows, conflicts, seen = union_of(dirs, want_caller, want_callee)
    fill = fill_dirs(dirs)
    if fill:
        r2, c2, s2 = union_of(fill, want_caller, want_callee)
        for m in s2:
            if m in seen:
                continue
            seen[m] = s2[m]
            if m in r2:
                rows[m] = r2[m]
            if m in c2:
                conflicts[m] = c2[m]
    return rows, conflicts, seen


def union_of(dirs, want_caller=None, want_callee=None):
    """The answers of several reference runs together. A (caller, callee) pair whose answers differ between two runs
    (another number of call instructions: the legacy JIT marks a callee "do not inline" after one failed attempt, so
    what was compiled before decides) has no single answer in the original: that pair is left out of `rows` and listed
    in `conflicts` {caller: {callee: {scenario: (sites, calls)}}}; the caller's other pairs, which every run that ran it
    agrees on, stay (until 2026-10-14 a conflict left the whole caller out; with the entity runs that dropped every
    agreed fact of NPC.AI, Projectile.AI and Player.Update because of one generic callee each).
    Returns (rows {caller: {callee: (sites, calls)}}, conflicts, seen {caller: [scenarios]})."""
    per_scen = {}
    for d in dirs:
        a, _ = call_answers(d, want_caller)
        per_scen[os.path.basename(d)] = a
    seen = defaultdict(list)
    for s, a in per_scen.items():
        for m in a:
            seen[m].append(s)
    rows, conflicts = {}, {}
    for m, scens in seen.items():
        merged, bad = {}, {}
        for s in scens:
            for c, v in per_scen[s][m].items():
                if c in merged and merged[c][1] != v[1]:
                    bad[c] = {x: per_scen[x][m][c] for x in scens if c in per_scen[x][m]}
                merged.setdefault(c, v)
        # a conflicting pair is left out, the agreed pairs of the same caller stay; `want_callee` only filters what
        # is returned
        merged = {c: v for c, v in merged.items() if c not in bad}
        if want_callee:
            merged = {c: v for c, v in merged.items() if want_callee(c)}
            bad = {c: v for c, v in bad.items() if want_callee(c)}
        if bad:
            conflicts[m] = bad
        if merged:
            rows[m] = merged
    return rows, conflicts, dict(seen)


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    table = "--table" in sys.argv
    d = args[0]
    want = args[1] if len(args) > 1 else None
    callees = load_json(os.path.join(d, "jit_callees.json"))
    load_cells(d)
    by_addr = names(d, callees)
    per = load_json(os.path.join(d, "jit_method_callees.json"))
    totals = defaultdict(lambda: [0, 0, 0])   # IL call sites, compiled as calls, inlined
    for name, addr, code, info in methods(d):
        if want and want not in name:
            continue
        calls = defaultdict(int)
        for t in machine_calls(addr, code, bool(info and info.get("exact_size"))):
            calls[by_addr.get(t, "?%08x" % t)] += 1
        sites = defaultdict(int)
        kind = {}
        for entry in per.get(name, []):
            how, callee = entry.split(" ", 1)
            sites[callee] += 1
            kind[callee] = how
        if not table:
            print("== " + name)
            print("   machine calls: %d (%d to named callees)" % (sum(calls.values()), sum(v for k, v in calls.items() if not k.startswith("?"))))
        for callee, n in sorted(sites.items()):
            got = calls.get(callee, 0)
            virt = kind[callee].endswith("-virtual") or kind[callee].startswith("ld")
            state = "dispatch?" if virt and got == 0 else "INLINED" if got == 0 else "call" if got >= n else "partly inlined"
            if not table:
                print("     %-15s IL x%-2d machine x%-2d %s" % (state, n, got, callee))
            if not virt:
                totals[callee][0] += n
                totals[callee][1] += min(got, n)
                totals[callee][2] += max(0, n - got)
        if not table:
            for k, v in sorted(calls.items()):
                if k not in sites:
                    print("     (helper)        machine x%-2d %s" % (v, k))
    if table:
        named = {c["name"]: c for c in callees}
        for k in sorted(totals):
            t = totals[k]
            print("%-110s sites=%-4d calls=%-4d inlined=%-4d il=%d" % (k[:110], t[0], t[1], t[2], named.get(k, {}).get("il_size", -1)))

if __name__ == "__main__":
    main()

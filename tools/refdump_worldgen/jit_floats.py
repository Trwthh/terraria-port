#!/usr/bin/env python3
"""Reads what `RefDumpWorldgenX.exe jitdump` wrote (jit_code.bin + jit_code.json) and shows, per method, where the
real 32-bit JIT keeps its float values (needs capstone).

usage: python jit_floats.py <dir with jit_code.bin> [NAME_SUBSTRING] [--asm]

Per method:
  * the frame slots that receive a 32-bit float store (`fstp dword [ebp-N]` / `fst dword [ebp-N]`, or relative to esp
    in a frameless method): values the JIT keeps IN MEMORY, rounded to float at every store (FLOAT_RULE.md: "spilled"),
    with the number of stores and loads of each slot;
  * the same for 64-bit stores (double locals and spilled x87 temporaries);
  * how many x87 register-to-register moves there are (`fld st(i)`, `fxch`, `fstp st(i)`): values that live on the
    x87 stack between statements, not rounded;
  * the float parameters (`fld dword [ebp+N]`).
`--asm` prints the disassembly up to the last reachable `ret`.

What this does NOT do: name the slots. The JIT allocates IL locals (jit_code.json has them: index and type), and
the decompiled source may split or merge them, so turning "slot ebp-0x1c is in memory" into a `floatInMemory`
entry `Type.Method:variable` of tools/cs2rs/game.json still takes a look at the stores around it (the IL of the
method is in jit_code.json for that). One such entry exists (Main.UpdateWeather:num7); this tool shows the store
that it describes.
"""
import json
import os
import re
import struct
import sys

import capstone

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import jit_calls  # noqa: E402


def methods(d):
    b = open(os.path.join(d, "jit_code.bin"), "rb").read()
    assert b[:4] == b"JITC", "not a JITC file"
    n = struct.unpack_from("<I", b, 8)[0]
    o = 12
    info = {m["name"]: m for m in jit_calls.load_json(os.path.join(d, "jit_code.json"))}
    for _ in range(n):
        ln = struct.unpack_from("<I", b, o)[0]
        name = jit_calls.jit_debuginfo.canonical(b[o + 4:o + 4 + ln].decode())
        o += 4 + ln
        addr, nb = struct.unpack_from("<II", b, o)
        o += 8
        yield name, addr, b[o:o + nb], info.get(name, {})
        o += nb


def disasm(addr, code):
    md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)
    far = addr
    out = []
    for ins in md.disasm(code, addr):
        out.append(ins)
        if ins.mnemonic.startswith("j") and ins.op_str.startswith("0x"):
            t = int(ins.op_str, 16)
            if addr <= t < addr + len(code):
                far = max(far, t)
        if ins.mnemonic == "ret" and ins.address >= far:
            break
    return out


SLOT = re.compile(r"(dword|qword) ptr \[(ebp|esp)( [+-] 0x[0-9a-f]+| [+-] \d+)?\]")


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    if not args:
        sys.exit(__doc__)
    d = args[0]
    want = args[1] if len(args) > 1 else ""
    show = "--asm" in sys.argv
    for name, addr, code, info in methods(d):
        if want not in name:
            continue
        ins = disasm(addr, code)
        stores, loads, params = {}, {}, {}
        moves = 0
        for i in ins:
            m = SLOT.search(i.op_str)
            if i.mnemonic in ("fld", "fxch", "fstp", "fst") and i.op_str.startswith("st("):
                moves += 1
            if not m:
                continue
            width, base, off = m.group(1), m.group(2), (m.group(3) or " + 0").replace(" ", "")
            key = (width, "%s%s" % (base, off))
            if i.mnemonic in ("fstp", "fst"):
                stores[key] = stores.get(key, 0) + 1
            elif i.mnemonic in ("fld", "fadd", "fsub", "fsubr", "fmul", "fdiv", "fdivr", "fcomp", "fcom", "fild"):
                if base == "ebp" and off.startswith("+"):
                    params[key] = params.get(key, 0) + 1
                else:
                    loads[key] = loads.get(key, 0) + 1
        fl = [l for l in info.get("locals", []) if l["type"] in ("System.Single", "System.Double")]
        print("== %s" % name)
        print("   %d instructions, IL %s bytes; IL locals: %d, of them float / double: %s" % (
            len(ins), info.get("il_size", "?"), len(info.get("locals", [])), ", ".join("V_%d %s" % (l["index"], l["type"].split(".")[-1]) for l in fl) or "none"))
        for width in ("dword", "qword"):
            slots = sorted(set(k for k in list(stores) + list(loads) if k[0] == width), key=lambda k: k[1])
            mem = [k for k in slots if k in stores]
            print("   %s slots written by an x87 store (in memory, rounded there): %d" % ("float" if width == "dword" else "double / spill", len(mem)))
            for k in mem:
                print("      [%s]  stores %d, loads %d" % (k[1], stores[k], loads.get(k, 0)))
        print("   float / double parameter reads: %s" % (", ".join("[%s] x%d" % (k[1], n) for k, n in sorted(params.items(), key=lambda kv: kv[0][1])) or "none"))
        print("   x87 register moves (values kept on the x87 stack): %d" % moves)
        if show:
            for i in ins:
                print("  %08x %-8s %s" % (i.address, i.mnemonic, i.op_str))


if __name__ == "__main__":
    main()

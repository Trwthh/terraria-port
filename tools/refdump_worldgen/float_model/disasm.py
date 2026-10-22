#!/usr/bin/env python3
"""Disassembles the JIT's machine code of a probe from refdata/worldgen/float_probes2/probes2_code.bin (needs capstone).
usage: python disasm.py NAME_SUBSTRING [refdata/worldgen dir]        e.g. python disasm.py L06
Prints the method up to its last reachable ret. Reading guide: "fld dword [ebp+8/0xc/0x10]" loads a parameter (z / y / x: pushed left to
right, so the last float parameter is nearest to the return address); "fstp dword [ebp-N]" is a float local kept in memory (rounded
here); a value that is only moved with fld st(i) / fxch / fstp st(i) lives on the x87 stack (not rounded)."""
import os, struct, sys
import capstone

D = sys.argv[2] if len(sys.argv) > 2 else os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", "refdata", "worldgen")
b = open(os.path.join(D, "float_probes2", "probes2_code.bin"), "rb").read()
assert b[:4] == b"JITC"
n = struct.unpack_from("<I", b, 8)[0]; o = 12
md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)
for _ in range(n):
    ln = struct.unpack_from("<I", b, o)[0]; name = b[o + 4:o + 4 + ln].decode(); o += 4 + ln
    addr, nb = struct.unpack_from("<II", b, o); o += 8
    code = b[o:o + nb]; o += nb
    if sys.argv[1] not in name: continue
    print("== %s @%08x" % (name, addr))
    far = addr; lines = []
    for ins in md.disasm(code, addr):
        lines.append("  %08x %-8s %s" % (ins.address, ins.mnemonic, ins.op_str))
        if ins.mnemonic.startswith("j") and ins.op_str.startswith("0x"):
            t = int(ins.op_str, 16)
            if addr <= t < addr + nb: far = max(far, t)
        if ins.mnemonic == "ret" and ins.address >= far: break
        if len(lines) > int(os.environ.get("MAXLINES", "400")): break
    print("\n".join(lines))

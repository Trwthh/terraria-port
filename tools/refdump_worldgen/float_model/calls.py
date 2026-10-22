#!/usr/bin/env python3
"""Lists, for every probe of the second set, the call instructions left in the JIT's machine code and the float stores to the frame
(fstp dword [ebp-N] = a float kept in memory). A helper that is "inlined" leaves no call. usage: python calls.py [filter] [refdata/worldgen dir]"""
import os, struct, sys
import capstone

flt = sys.argv[1] if len(sys.argv) > 1 else ""
D = sys.argv[2] if len(sys.argv) > 2 else os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", "refdata", "worldgen")
b = open(os.path.join(D, "float_probes2", "probes2_code.bin"), "rb").read()
n = struct.unpack_from("<I", b, 8)[0]; o = 12
md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)
print("%-40s %5s %5s %6s %6s %6s" % ("probe", "bytes", "calls", "fstp4", "fstp8", "fld4"))
for _ in range(n):
    ln = struct.unpack_from("<I", b, o)[0]; name = b[o + 4:o + 4 + ln].decode(); o += 4 + ln
    addr, nb = struct.unpack_from("<II", b, o); o += 8
    code = b[o:o + nb]; o += nb
    if flt not in name: continue
    far = addr; calls = st4 = st8 = ld4 = 0; end = addr
    for ins in md.disasm(code, addr):
        if ins.mnemonic.startswith("j") and ins.op_str.startswith("0x"):
            t = int(ins.op_str, 16)
            if addr <= t < addr + nb: far = max(far, t)
        if ins.mnemonic == "call": calls += 1
        if ins.mnemonic == "fstp" and "dword ptr [ebp -" in ins.op_str: st4 += 1
        if ins.mnemonic == "fstp" and "qword ptr [ebp -" in ins.op_str: st8 += 1
        if ins.mnemonic == "fld" and "dword ptr [ebp -" in ins.op_str: ld4 += 1
        end = ins.address + ins.size
        if ins.mnemonic == "ret" and ins.address >= far: break
    print("%-40s %5d %5d %6d %6d %6d" % (name, end - addr, calls, st4, st8, ld4))

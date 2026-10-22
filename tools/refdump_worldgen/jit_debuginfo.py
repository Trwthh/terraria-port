#!/usr/bin/env python3
"""Reads the CLR's debug info of the methods a reference run dumped (jit_debug.bin, JitDump.cs WriteDebugInfo): the
IL-offset / native-offset map and the variable locations the JIT reported for every IL argument and local
(ICorDebugInfo::NativeVarInfo: from native offset, to native offset, IL variable number, location).

Format (.NET Framework 4.x debuginfostore.cpp / nibblestream.h): a nibble stream, low nibble of a byte first; an
unsigned value is groups of 3 bits, most significant first, a set high bit in the nibble = more groups; a signed value
is the unsigned one with the sign in bit 0; a frame offset is (offset / 4) << 1 in 31 bits. Header: size of the bounds part, size of the vars part. Bounds: count, then
per entry native offset (delta), IL offset (+ MAX_MAPPING_VALUE = -3), source type. Vars: count, then per entry start
offset, length, variable number (+ MAX_ILNUM = -4), location type and its data (VLT_REG 0, VLT_REG_BYREF 1,
VLT_REG_FP 2, VLT_STK 3, VLT_STK_BYREF 4, VLT_REG_REG 5, VLT_REG_STK 6, VLT_STK_REG 7, VLT_STK2 8, VLT_FPSTK 9,
VLT_FIXED_VA 10; a stack offset is stored divided by 4).

IL variable numbers: the arguments first (`this` is 0 for an instance method), then the locals.

    python jit_debuginfo.py <scenario|dump dir> <method name substring>      (prints the variables of each match)
"""
import os
import struct
import sys

here = os.path.dirname(os.path.abspath(__file__))
REGS = ['eax', 'ecx', 'edx', 'ebx', 'esp', 'ebp', 'esi', 'edi']
VLT = ['REG', 'REG_BYREF', 'REG_FP', 'STK', 'STK_BYREF', 'REG_REG', 'REG_STK', 'STK_REG', 'STK2', 'FPSTK', 'FIXED_VA']


class Nibbles:
    def __init__(self, b, start=0):
        self.b = b
        self.i = start * 2

    def nibble(self):
        byte = self.b[self.i // 2]
        n = (byte & 0xF) if self.i % 2 == 0 else (byte >> 4)
        self.i += 1
        return n

    def u32(self):
        v = 0
        while True:
            n = self.nibble()
            v = (v << 3) | (n & 7)
            if not (n & 8):
                return v & 0xFFFFFFFF

    def stack_offset(self):
        """A frame offset: stored as (offset / 4) << 1 in 31 bits (measured on the CLR 4.8 x86 JIT's output:
        `Collision.TileCollision` keeps its return buffer at [ebp-0x88], recorded as 0x7fffffbc)."""
        x = self.u32() >> 1
        if x & 0x20000000:
            x -= 0x40000000
        return x * 4

    def byte_index(self):
        return (self.i + 1) // 2


def parse(blob):
    """(bounds [(native, il, source)], vars [(start, end, varnum, kind, data)])."""
    r = Nibbles(blob)
    cb_bounds = r.u32()
    cb_vars = r.u32()
    o = r.byte_index()
    bounds = []
    if cb_bounds:
        rb = Nibbles(blob, o)
        n = rb.u32()
        last = 0
        for _ in range(n):
            last += rb.u32()
            il = (rb.u32() - 3) & 0xFFFFFFFF
            src = rb.u32()
            bounds.append((last, il if il < 0x80000000 else il - (1 << 32), src))
    vars_ = []
    if cb_vars:
        rv = Nibbles(blob, o + cb_bounds)
        n = rv.u32()
        for _ in range(n):
            start = rv.u32()
            end = start + rv.u32()
            var = rv.u32() - 4
            kind = rv.u32()
            name = VLT[kind] if kind < len(VLT) else str(kind)
            if name in ('REG', 'REG_BYREF', 'REG_FP'):
                data = (REGS[rv.u32() & 7],)
            elif name in ('STK', 'STK_BYREF', 'STK2'):
                data = (REGS[rv.u32() & 7], rv.stack_offset())
            elif name == 'REG_REG':
                data = (REGS[rv.u32() & 7], REGS[rv.u32() & 7])
            elif name == 'REG_STK':
                data = (REGS[rv.u32() & 7], REGS[rv.u32() & 7], rv.stack_offset())
            elif name == 'STK_REG':
                data = (rv.stack_offset(), REGS[rv.u32() & 7], REGS[rv.u32() & 7])
            elif name == 'FPSTK':
                data = (rv.u32(),)
            elif name == 'FIXED_VA':
                data = (rv.u32(),)
            else:
                raise ValueError('location type %d' % kind)
            vars_.append((start, end, var, name, data))
    return bounds, vars_


def canonical(name):
    """A method name as jit_calls.load_json writes it."""
    name = name.replace("TerrariaServer, Version=", "Terraria, Version=")
    for n in ("Graphics", "Game"):
        name = name.replace("Microsoft.Xna.Framework.%s, Version=0.0.0.0," % n, "Microsoft.Xna.Framework.%s, Version=4.0.0.0," % n)
    return name


def load(d):
    """{method name: blob} of a dump directory."""
    b = open(os.path.join(d, 'jit_debug.bin'), 'rb').read()
    assert b[:4] == b'JDBG'
    n = struct.unpack_from('<I', b, 8)[0]
    o = 12
    out = {}
    for _ in range(n):
        ln = struct.unpack_from('<I', b, o)[0]
        # the same canonical names as jit_calls.load_json gives jit_code.json (generic arguments name the assembly: the
        # server runs' `TerrariaServer, Version=` reads `Terraria, Version=`, the XNA stand-in's 0.0.0.0 reads 4.0.0.0);
        # without it no method with a generic argument type of the server runs matched its jit_code.json entry
        name = canonical(b[o + 4:o + 4 + ln].decode())
        o += 4 + ln
        _ptr, nb = struct.unpack_from('<II', b, o)
        o += 8
        out[name] = b[o:o + nb]
        o += nb
    return out


def main():
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    d = sys.argv[1]
    if not os.path.isdir(d):
        d = os.path.join(here, '..', '..', 'work', 'jit', 'ref', d)
    for name, blob in load(d).items():
        if sys.argv[2] not in name:
            continue
        bounds, vars_ = parse(blob)
        print('== %s: %d bounds, %d variable ranges' % (name, len(bounds), len(vars_)))
        for v in sorted(vars_, key=lambda x: (x[2], x[0])):
            print('   V%-4d %6x-%-6x %-9s %s' % (v[2], v[0], v[1], v[3], v[4]))


if __name__ == '__main__':
    main()

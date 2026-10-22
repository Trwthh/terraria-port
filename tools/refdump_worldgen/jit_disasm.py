#!/usr/bin/env python3
"""Disassembles methods of a reference run's dump (work/jit/ref/<scenario>/jit_code.bin, JitDump.cs DumpRan), with
call targets named (jit_calls.names) and the method's IL locals listed (jit_code.json).

    python jit_disasm.py <scenario|dump dir> <name substring> [--max N]

`<scenario>` is a folder name under work/jit/ref (entities_proj, ticks_world ...). Reading guide (FLOAT_RULE.md):
`fstp dword [ebp-N]` stores a float to memory (rounded there); `fstp qword` keeps 53 bits; a value moved only with
fld st(i) / fxch / fstp st(i) stays on the x87 stack."""
import os
import sys

import capstone

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, here)
import jit_calls  # noqa: E402


def main():
    args = [a for a in sys.argv[1:] if not a.startswith('--')]
    mx = int(sys.argv[sys.argv.index('--max') + 1]) if '--max' in sys.argv else 4000
    if len(args) < 2:
        sys.exit(__doc__)
    d = args[0]
    if not os.path.isdir(d):
        d = os.path.join(here, '..', '..', 'work', 'jit', 'ref', args[0])
    want = args[1]
    jit_calls.load_cells(d)
    by_addr = jit_calls.names(d)
    md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)
    for name, addr, code, info in jit_calls.methods(d):
        if want not in name:
            continue
        print('== %s @%08x, %d bytes' % (name, addr, len(code)))
        if info:
            print('   IL size %s, locals: %s' % (info.get('il_size'), ', '.join('V_%d %s' % (l['index'], l['type'].split('.')[-1]) for l in info['locals'])))
        n = 0
        for ins in md.disasm(code, addr):
            note = ''
            if ins.mnemonic == 'call':
                t = None
                if ins.op_str.startswith('0x'):
                    t = int(ins.op_str, 16)
                elif ins.op_str.startswith('dword ptr [0x') and ins.op_str.endswith(']'):
                    cell = int(ins.op_str[len('dword ptr ['):-1], 16)
                    t = jit_calls.CELLS.get(cell)
                if t is not None:
                    note = '    ; ' + by_addr.get(t, '?')
            print('  %08x %-7s %s%s' % (ins.address, ins.mnemonic, ins.op_str, note))
            n += 1
            if n >= mx:
                print('  ...')
                break


if __name__ == '__main__':
    main()

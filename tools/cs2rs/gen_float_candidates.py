"""float_mem_candidates.tsv: methods where the reference runs' machine code keeps more float values in memory than the
translation rounds, i.e. candidates for config `floatInMemory` (a float local the allocation rule puts on the x87
stack but the real JIT stores as a float). A REVIEW LIST, not applied: turning a candidate into an entry needs the
local named (the JIT allocates IL locals, the rule works on the decompiler's; FLOAT_RULE.md 4.8) and evidence, as the
existing entries have.

Inputs:
* the dumps of the reference runs (work/jit/ref/<scenario>, run_refjit.sh): the methods that ran with a float or double
  IL local and one answer in all runs (jit_calls.union; a method compiled differently in two runs is skipped);
* the translator's own allocation, one line per body: `CS2RS_FLOAT_REPORT=<file> sh tools/cs2rs/run.sh`
  (default file work/jit/float_alloc.tsv; Emit.cs writes CLR key, MinOpts, float locals, stack locals, stack locals
  rounded at a call).

Per method, from the machine code: the frame slots `[ebp - N]` that receive a 32-bit x87 store (`fstp` / `fst dword`)
and are read back with `fld dword` (a float local in memory, or a stack value spilled around a call). From the
translation: the float locals in memory plus the stack locals rounded at a call. A method whose code has MORE such
slots than that is listed (column `excess`), with both counts and the names. Known sources of false candidates: fields
of a local struct in memory (`Vector2 v` with a non-inlined call on it), JIT temporaries spilled as floats, IL slots
the decompiler split. Methods compiled with MinOpts by the original (the translation keeps every local in memory)
are skipped.

    python gen_float_candidates.py [float_alloc.tsv] [dump base dir]
"""
import os
import re
import sys
from collections import defaultdict

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(here, '..', 'refdump_worldgen'))
import capstone  # noqa: E402
import jit_calls  # noqa: E402

rep = sys.argv[1] if len(sys.argv) > 1 else os.path.join(here, '..', '..', 'work', 'jit', 'float_alloc.tsv')
base = sys.argv[2] if len(sys.argv) > 2 else os.path.join(here, '..', '..', 'work', 'jit', 'ref')
alloc = {}
for line in open(rep, encoding='utf-8'):
    p = line.rstrip('\n').split('\t')
    if len(p) >= 5:
        alloc[p[0]] = (p[1] == '1', [x for x in p[2].split(',') if x], [x for x in p[3].split(',') if x], [x for x in p[4].split(',') if x])
dirs = jit_calls.ref_dirs(base)
want = lambda name, info: jit_calls.has_float_local(info)  # noqa: E731
rows_m, conflicts, seen = jit_calls.union(dirs, want)
SLOT = re.compile(r'dword ptr \[ebp - (0x[0-9a-f]+|\d+)\]')
md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)


def slots(code, addr):
    st, ld = set(), set()
    for ins in md.disasm(code, addr):
        if ins.mnemonic in ('fstp', 'fst', 'fld'):
            m = SLOT.search(ins.op_str)
            if m:
                (ld if ins.mnemonic == 'fld' else st).add(int(m.group(1), 0))
    return st & ld


# the code of each method once (the first run that has it; the runs agree on its calls)
code_of = {}
for d in dirs:
    for name, addr, code, info in jit_calls.methods(d):
        if name in seen and name not in conflicts and name not in code_of:
            code_of[name] = (addr, code)
out = []
checked = no_alloc = minopts = 0
for name in sorted(code_of):
    a = alloc.get(name)
    if a is None:
        no_alloc += 1
        continue
    if a[0]:
        minopts += 1
        continue
    checked += 1
    locals_, stack, spilled = a[1], a[2], a[3]
    in_mem = [x for x in locals_ if x not in stack]
    n = len(slots(code_of[name][1], code_of[name][0]))
    expect = len(in_mem) + len(spilled)
    if n > expect:
        out.append((n - expect, name, n, expect, ','.join(stack), ','.join(in_mem), ','.join(spilled)))
out.sort(key=lambda r: (-r[0], r[1]))
text = 'excess\tmethod\tjit_float_slots\ttranslation_rounded\tstack_locals\tin_memory_locals\tstack_locals_rounded_at_calls\n'
text += ''.join('%d\t%s\t%d\t%d\t%s\t%s\t%s\n' % r for r in out)
open(os.path.join(here, 'float_mem_candidates.tsv'), 'w', encoding='utf-8', newline='').write(text)
print('float_mem_candidates.tsv: %d methods with float locals ran with one answer; %d compared (%d MinOpts, %d not in the '
      'translator report); %d candidates (more float slots in the machine code than the translation rounds)' % (
          len(code_of), checked, minopts, no_alloc, len(out)))

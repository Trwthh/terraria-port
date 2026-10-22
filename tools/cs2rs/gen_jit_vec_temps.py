"""jit_vec_temps.tsv: where the real 32-bit JIT keeps the results of the XNA Vector2 operators it inlined, per method,
read from the machine code of the processes that wrote the reference data (tools/refdump_worldgen/run_refjit.sh ->
work/jit/ref/<scenario>). Input of config `jitVecTemps` (float mode "jit32", Emit.cs VecInlined; FLOAT_RULE.md section 6).

An inlined `Vector2 op_Multiply(Vector2, float)` has a local `result` whose fields it writes. Two forms occur:
  stack   the inlinee's result struct is promoted and its fields are x87 registers: `a + v * s` is computed per
          component without a store and rounded once where the caller stores it
          (BallCollision.ClosestPointOnLineSegment: `fmul st(4)`, `faddp st(2)`, `fstp dword [edi]`);
  memory  the result struct is a frame temporary, zeroed first (`xorps xmm0, xmm0` / `movq qword [reg], xmm0`) and
          written per field with `fstp dword`: every operator's components are rounded
          (GolfHelper.ContactListener.OnCollision).
A method is `stack` when its code has no such struct temporary at all (zeroed, or, second form, not zeroed but
addressed through `lea reg, [ebp - N]`, written per field with `fstp dword` and used from its slot by x87
arithmetic), `memory` otherwise (a zeroed 8-byte temporary of
another struct type also counts: the conservative answer, the per-operator rounding; a zeroing of more than 8 bytes,
a Nullable<Vector2> or an enumerator, does not). Only methods with an inlined
Vector2 operator site (jit_bodies.tsv: IL sites of the operator, fewer call instructions) are listed. A method whose
answer differs between two runs is left out.

    python gen_jit_vec_temps.py [dump base dir, default work/jit/ref]
"""
import os
import sys
from collections import defaultdict

import capstone

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(here, '..', 'refdump_worldgen'))
import jit_calls  # noqa: E402

base = sys.argv[1] if len(sys.argv) > 1 else os.path.join(here, '..', '..', 'work', 'jit', 'ref')
dirs = jit_calls.ref_dirs(base)
if not dirs:
    sys.exit('no dumps under ' + base + ' (run tools/refdump_worldgen/run_refjit.sh)')
OPS = ('op_Addition(', 'op_Subtraction(', 'op_Multiply(', 'op_Division(', 'op_UnaryNegation(')
inlined = set()
for line in open(os.path.join(here, 'jit_bodies.tsv'), encoding='utf-8'):
    p = line.rstrip('\n').split('\t')
    if len(p) == 4 and p[1].startswith('Microsoft.Xna.Framework.Vector2::') and any(o in p[1] for o in OPS) and int(p[3]) < int(p[2]):
        inlined.add(p[0])
md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)
answers = defaultdict(dict)
ran_in_reference = set()
for d in list(dirs) + jit_calls.fill_dirs(dirs):
    primary = d in dirs
    for name, addr, code, info in jit_calls.methods(d):
        if primary:
            ran_in_reference.add(name)
        if name not in inlined:
            continue
        # a client run (jit_calls.Dirs) only answers for methods no reference run ran
        if not primary and name in ran_in_reference:
            continue
        ins = list(md.disasm(code, addr))
        # an 8-byte zeroing only: a Vector2 temporary. A larger struct is zeroed by the same `movq` followed by more
        # (`movq qword [reg + 8], xmm0`: a List enumerator, LightningGenerator.Generate; `add reg, 8; stosd`: a
        # Nullable<Vector2>, FinalFractalProfile.StripDust); counting those made both methods `memory` although their
        # inlined operators keep every product on the x87 stack (gallery run)
        def bigger(b, c):
            r = b.op_str[len('qword ptr ['):b.op_str.index(']')]
            return (c.mnemonic == 'movq' and c.op_str == 'qword ptr [%s + 8], xmm0' % r) or (c.mnemonic == 'add' and c.op_str == '%s, 8' % r)
        zeroed = any(a.mnemonic == 'xorps' and a.op_str == 'xmm0, xmm0' and b.mnemonic == 'movq' and b.op_str.startswith('qword ptr [e') and b.op_str.endswith('], xmm0')
                     and not bigger(b, c) for a, b, c in zip(ins, ins[1:], ins[2:]))
        # the second form of a frame temporary: not zeroed, addressed through `lea reg, [ebp - N]`, written per field
        # (`fstp dword ptr [reg]` / `[reg + 4]`) and then used as an operand of x87 arithmetic straight from its slot
        # (`fadd dword ptr [ebp - N]`): an operator result rounded before the next operator. Rain.Update materialises
        # `-velocity * 0.025f` that way before adding it to the dust's velocity
        if not zeroed:
            temps = set()
            for i, a in enumerate(ins):
                if a.mnemonic != 'lea' or not a.op_str.split(', ', 1)[-1].startswith('[ebp - '):
                    continue
                reg, slot = a.op_str.split(', ', 1)
                n = int(slot[len('[ebp - '):-1], 0)
                nxt = ins[i + 1:i + 6]
                if any(b.mnemonic == 'fstp' and b.op_str == 'dword ptr [%s]' % reg for b in nxt) and any(b.mnemonic == 'fstp' and b.op_str == 'dword ptr [%s + 4]' % reg for b in nxt):
                    temps.update(('dword ptr [ebp - %s]' % hex(n), 'dword ptr [ebp - %s]' % hex(n - 4)))
            zeroed = any(b.mnemonic in ('fadd', 'fsub', 'fsubr', 'fmul', 'fdiv', 'fdivr') and b.op_str in temps for b in ins)
        answers[name][os.path.basename(d)] = 'memory' if zeroed else 'stack'
rows, conflicts = [], 0
for name in sorted(answers):
    vals = set(answers[name].values())
    if len(vals) != 1:
        conflicts += 1
        continue
    rows.append((name, vals.pop()))
out = os.path.join(here, 'jit_vec_temps.tsv')
text = ''.join('%s\t%s\n' % r for r in rows)
old = open(out, encoding='utf-8', newline='').read() if os.path.exists(out) else None
if old != text:
    open(out, 'w', encoding='utf-8', newline='').write(text)
print('jit_vec_temps.tsv: %d methods with inlined Vector2 operators (%d stack, %d memory, %s); %d left out (different answers in two runs)' % (
    len(rows), sum(1 for r in rows if r[1] == 'stack'), sum(1 for r in rows if r[1] == 'memory'), 'unchanged' if old == text else 'written', conflicts))

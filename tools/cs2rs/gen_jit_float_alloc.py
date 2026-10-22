"""jit_float_alloc.tsv: where the real 32-bit JIT kept each float local of the methods that ran in the reference runs:
in a frame slot (every store rounds to float) or on the x87 register stack (stores do not round). Input of config
`jitFloatAlloc` (float mode "jit32", FloatAlloc.cs): for the methods listed it replaces the allocation rule of
FLOAT_RULE.md section 3, which is a model of the same decision.

Inputs:
* the dumps of the reference runs (tools/refdump_worldgen/run_refjit.sh -> work/jit/ref/<scenario>): jit_debug.bin,
  the debug info the CLR keeps for each method's code (jit_debuginfo.py: per IL argument / local the native ranges and
  their location, VLT_STK = a frame slot, VLT_FPSTK = an x87 register), and jit_code.json (IL, locals, parameters);
* work/pdb/pdb_locals.tsv (`tmeta pdblocals` over the portable PDB `ilspycmd -genpdb` writes for the decompilation of
  Terraria.exe): which decompiled variable is which IL local slot. A method is used only where the IL of the run's
  method (TerrariaServer.exe) and of Terraria.exe are identical apart from metadata tokens (FNV-1a 64 of the IL
  bytes with token operands zeroed).

Rows: method (the dumps' name, `Type::Name(ParamTypes)`), decompiled variable, kind:
  memory   a float IL local whose every range is a frame slot
  stack    a float IL local whose every range is an x87 register
  none     a float IL local without a range (dead, or the JIT reported nothing): the rule decides
  conflict a float IL local with different answers in two runs or both kinds: the rule decides
  covered  (variable `*`) the method is in the table: a float variable of it with no row is not an IL local
A float variable of the decompiled method that has no row is not an IL local: ILSpy made it from a value the IL keeps
on the evaluation stack, which the JIT keeps unrounded and spills to an 8-byte slot around calls (AI_065_Butterflies
num22: `fstp qword [ebp-0x160]`). Deterministic: sorted, no timestamp.

    python gen_jit_float_alloc.py [dump base dir] [pdb_locals.tsv]
"""
import json
import re
import os
import sys
from collections import defaultdict

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(here, '..', 'refdump_worldgen'))
import jit_calls  # noqa: E402
import jit_debuginfo  # noqa: E402

base = sys.argv[1] if len(sys.argv) > 1 else os.path.join(here, '..', '..', 'work', 'jit', 'ref')
pdbf = sys.argv[2] if len(sys.argv) > 2 else os.path.join(here, '..', '..', 'work', 'pdb', 'pdb_locals.tsv')
all_dirs = jit_calls.ref_dirs(base)
dirs = [d for d in all_dirs if os.path.exists(os.path.join(d, 'jit_debug.bin'))]
# the client runs' dumps (jit_calls.Dirs): only for methods none of the reference runs ran
fill = [d for d in jit_calls.fill_dirs(all_dirs) if os.path.exists(os.path.join(d, 'jit_debug.bin'))]
if not dirs:
    sys.exit('no dumps with jit_debug.bin under ' + base + ' (run tools/refdump_worldgen/run_refjit.sh)')

# (type, method, il fnv) -> {slot: [names]}
pdb = defaultdict(lambda: defaultdict(list))
for line in open(pdbf, encoding='utf-8'):
    t, m, h, slot, name = line.rstrip('\n').split('\t')
    pdb[(t, m, h)][int(slot)].append(name)


ONE = {0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x1F, 0xDE} | set(range(0x2B, 0x38))
FOUR = {0x20, 0x22, 0x28, 0x29, 0x27, 0x6F, 0x70, 0x71, 0x72, 0x73, 0x74, 0x75, 0x79, 0x8C, 0x8D, 0x8F, 0xA3, 0xA4, 0xA5,
        0xC2, 0xC6, 0xD0, 0xDD} | set(range(0x38, 0x45)) | set(range(0x7B, 0x82))
TOKENS = {0x27, 0x28, 0x29, 0x6F, 0x70, 0x71, 0x72, 0x73, 0x74, 0x75, 0x79, 0x7B, 0x7C, 0x7D, 0x7E, 0x7F, 0x80, 0x81, 0x8C,
          0x8D, 0x8F, 0xA3, 0xA4, 0xA5, 0xC2, 0xC6, 0xD0}


def normalized(il):
    """The IL with every metadata token operand zeroed (tmeta PdbLocals.Normalized): the same method in Terraria.exe
    and TerrariaServer.exe differs only in its tokens."""
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
            n = int.from_bytes(o[i + 1:i + 5], 'little')
            i += 1 + 4 + 4 * n
            continue
        if op in TOKENS:
            o[i + 1:i + 5] = bytes(4)
        i += 1 + (1 if op in ONE else 4 if op in FOUR else 8 if op in (0x21, 0x23) else 0)
    return bytes(o)


def fnv(b):
    h = 0xcbf29ce484222325
    for x in b:
        h = ((h ^ x) * 0x100000001b3) & 0xFFFFFFFFFFFFFFFF
    return '%016x' % h


def address_taken(il):
    """IL local indices whose address is taken (ldloca / ldloca.s): untracked by the JIT, homed in the frame."""
    a = set()
    i = 0
    while i < len(il):
        op = il[i]
        if op == 0xFE:
            op2 = il[i + 1]
            if op2 == 0x0D:
                a.add(int.from_bytes(il[i + 2:i + 4], 'little'))
            size = 4 if op2 in (0x06, 0x07, 0x15, 0x16, 0x1C) else 2 if 0x09 <= op2 <= 0x0E else 1 if op2 in (0x12, 0x19) else 0
            i += 2 + size
            continue
        if op == 0x12:
            a.add(il[i + 1])
        if op == 0x45:
            n = int.from_bytes(il[i + 1:i + 5], 'little')
            i += 1 + 4 + 4 * n
            continue
        i += 1 + (1 if op in ONE else 4 if op in FOUR else 8 if op in (0x21, 0x23) else 0)
    return a


# methods with at most this many IL locals + arguments: every local is tracked (the JIT tracks 64 lclVars, its own
# temporaries included; FLOAT_RULE.md 4.1)
TRACKED_SAFE = 48

answers = defaultdict(dict)   # method -> slot -> set of kinds per run
info_of = {}
ran_in_reference = set()
for d in dirs + fill:
    infos = {m['name']: m for m in jit_calls.load_json(os.path.join(d, 'jit_code.json'))}
    if d in dirs:
        ran_in_reference.update(infos)
    for name, blob in jit_debuginfo.load(d).items():
        m = infos.get(name)
        if not m or not m.get('il'):
            continue
        if d not in dirs and name in ran_in_reference:
            continue
        if not any(l['type'] == 'System.Single' for l in m['locals']):
            continue
        info_of[name] = m
        _, vars_ = jit_debuginfo.parse(blob)
        nargs = len(m['params']) + (0 if m['static'] else 1)
        kinds = defaultdict(set)
        for _s, _e, var, k, _d in vars_:
            if var >= nargs:
                kinds[var - nargs].add(k)
        addr = address_taken(bytes.fromhex(m['il']))
        for l in m['locals']:
            if l['type'] != 'System.Single':
                continue
            k = kinds.get(l['index'], set())
            kind = 'none' if not k else 'memory' if k <= {'STK', 'STK2'} else 'stack' if k == {'FPSTK'} else 'conflict'
            # no location at all: an untracked local lives in the frame (address taken, or beyond the JIT's limit of
            # tracked locals in a method with many), any other has no home: an unrounded temporary ('temp';
            # NextHorizonRenderer.OriginalColorsForCloud keeps both of its locals on the x87 stack, audio run)
            if kind == 'none' and l['index'] not in addr and len(m['locals']) + len(m['params']) + 1 <= TRACKED_SAFE:
                kind = 'temp'
            prev = answers[name].get(l['index'])
            answers[name][l['index']] = kind if prev is None or prev == kind else 'conflict'

rows = []
matched = unmatched = 0
counts = defaultdict(int)
for name in sorted(answers):
    m = info_of[name]
    tm = name.split('(')[0]
    t, meth = tm.rsplit('::', 1)
    key = (t, meth, fnv(normalized(bytes.fromhex(m['il']))))
    if key not in pdb:
        unmatched += 1
        continue
    matched += 1
    slots = pdb[key]
    types = {l['index']: l['type'] for l in m['locals']}
    rows.append('%s\t*\tcovered\n' % name)
    for slot in sorted(slots):
        if types.get(slot) != 'System.Single':
            continue
        for v in sorted(slots[slot]):
            # V_n: ILSpy's name for a slot that no decompiled variable uses
            if re.fullmatch(r'V_[0-9]+', v):
                continue
            kind = answers[name].get(slot, 'none')
            counts[kind] += 1
            rows.append('%s\t%s\t%s\n' % (name, v, kind))

out = os.path.join(here, 'jit_float_alloc.tsv')
text = ''.join(rows)
old = open(out, encoding='utf-8', newline='').read() if os.path.exists(out) else None
if old != text:
    open(out, 'w', encoding='utf-8', newline='').write(text)
print('jit_float_alloc.tsv: %d runs, %d methods with float locals (%d with the same IL in Terraria.exe, %d not): %s (%s)' % (
    len(dirs), matched + unmatched, matched, unmatched, ', '.join('%s %d' % kv for kv in sorted(counts.items())),
    'unchanged' if old == text else 'written'))

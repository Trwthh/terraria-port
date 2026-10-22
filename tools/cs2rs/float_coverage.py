#!/usr/bin/env python3
"""Float-rule coverage: of the game's methods that do float arithmetic, how many ran in a reference process whose
machine code the fact tables are read from (the JIT dumps of work/jit/ref/* and work/jit/client/*: JitDump.cs DumpRan,
"methods that ran"), and which do not, by namespace / type, with their static call sites.

    python tools/cs2rs/float_coverage.py [--md FLOAT_COVERAGE.md] [--tsv float_uncovered.tsv] [--dirs d1,d2,...]

A method "does float arithmetic" when its IL (read from work/refclient/Terraria.exe, the client copy; the install is
not opened) has a float or double value AND an arithmetic instruction (add, sub, mul, div, rem, neg). A float value:
ldc.r4 / ldc.r8, conv.r4 / conv.r8 / conv.r.un, a read of a float, double or float-struct field (Vector2, Vector3,
Vector4, Matrix, Color is not one), a float, double or float-struct parameter, a call that returns one, or a call to
an XNA Vector2 / Vector3 / Vector4 / MathHelper / Matrix member. (An over-approximation: the arithmetic may be on
integers in such a method.)

A method is covered when a dump has a method of the same type and name whose IL equals it with every metadata token
zeroed (the server's TerrariaServer.exe differs from the client's Terraria.exe in tokens only; tmeta PdbLocals uses the
same key). A method of a generic type is covered by a closed instantiation's code (the dump names it
`Definition`n<Args>`, JitDump.cs JitOwnerName). `--dirs` overrides the dump folders (comma separated).

Output: a markdown report (totals; uncovered by namespace and by type; the 60 uncovered methods with the most call
sites) and a TSV of every float method (key, covered by which dumps, call sites, IL bytes).
"""
import collections
import glob
import json
import os
import sqlite3
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, '..', '..'))
EXE = os.path.join(ROOT, 'work', 'refclient', 'Terraria.exe')
DB = os.path.join(ROOT, 'db', 'terraria.sqlite')

ONE = {0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x1F, 0xDE} | set(range(0x2B, 0x38))
FOUR = {0x20, 0x22, 0x28, 0x29, 0x27, 0x6F, 0x70, 0x71, 0x72, 0x73, 0x74, 0x75, 0x79, 0x8C, 0x8D, 0x8F, 0xA3, 0xA4, 0xA5,
        0xC2, 0xC6, 0xD0, 0xDD} | set(range(0x38, 0x45)) | set(range(0x7B, 0x82))
TOKENS = {0x27, 0x28, 0x29, 0x6F, 0x70, 0x71, 0x72, 0x73, 0x74, 0x75, 0x79, 0x7B, 0x7C, 0x7D, 0x7E, 0x7F, 0x80, 0x81, 0x8C,
          0x8D, 0x8F, 0xA3, 0xA4, 0xA5, 0xC2, 0xC6, 0xD0}
ARITH = {0x58, 0x59, 0x5A, 0x5B, 0x5D, 0x65}
FLOAT_TYPES = {'float', 'double', 'Vector2', 'Vector3', 'Vector4', 'Matrix', 'System.Single', 'System.Double'}
XNA_FLOAT = ('Microsoft.Xna.Framework.Vector2', 'Microsoft.Xna.Framework.Vector3', 'Microsoft.Xna.Framework.Vector4',
             'Microsoft.Xna.Framework.MathHelper', 'Microsoft.Xna.Framework.Matrix')


def ops(il):
    """(offset, opcode) of every instruction (two-byte opcodes as 0xFE00 | b)."""
    i = 0
    while i < len(il):
        op = il[i]
        if op == 0xFE:
            op2 = il[i + 1]
            size = 4 if op2 in (0x06, 0x07, 0x15, 0x16, 0x1C) else 2 if 0x09 <= op2 <= 0x0E else 1 if op2 in (0x12, 0x19) else 0
            yield i, 0xFE00 | op2
            i += 2 + size
            continue
        yield i, op
        if op == 0x45:
            n = int.from_bytes(il[i + 1:i + 5], 'little')
            i += 1 + 4 + 4 * n
            continue
        i += 1 + (1 if op in ONE else 4 if op in FOUR else 8 if op in (0x21, 0x23) else 0)


def normalized(il):
    o = bytearray(il)
    for i, op in ops(il):
        if op == 0xFE06 or op == 0xFE07 or op == 0xFE15 or op == 0xFE16 or op == 0xFE1C:
            o[i + 2:i + 6] = bytes(4)
        elif op < 0x100 and op in TOKENS:
            o[i + 1:i + 5] = bytes(4)
    return bytes(o)


def fnv(b):
    h = 0xcbf29ce484222325
    for x in b:
        h = ((h ^ x) * 0x100000001b3) & 0xFFFFFFFFFFFFFFFF
    return h


class Pe:
    def __init__(self, path):
        self.b = open(path, 'rb').read()
        pe = struct.unpack_from('<I', self.b, 0x3C)[0]
        nsec = struct.unpack_from('<H', self.b, pe + 6)[0]
        opt = struct.unpack_from('<H', self.b, pe + 20)[0]
        sec = pe + 24 + opt
        self.secs = []
        for k in range(nsec):
            vsize, va, rsize, raw = struct.unpack_from('<IIII', self.b, sec + 40 * k + 8)
            self.secs.append((va, max(vsize, rsize), raw))

    def off(self, rva):
        for va, size, raw in self.secs:
            if va <= rva < va + size:
                return rva - va + raw
        raise ValueError(rva)

    def il(self, rva):
        o = self.off(rva)
        h = self.b[o]
        if h & 3 == 2:
            return self.b[o + 1:o + 1 + (h >> 2)]
        size = struct.unpack_from('<I', self.b, o + 4)[0]
        hs = (struct.unpack_from('<H', self.b, o)[0] >> 12) * 4
        return self.b[o + hs:o + hs + size]


def dump_dirs():
    return sorted(d for d in glob.glob(os.path.join(ROOT, 'work', 'jit', 'ref', '*')) + glob.glob(os.path.join(ROOT, 'work', 'jit', 'client', '*'))
                  if os.path.isfile(os.path.join(d, 'jit_code.json')))


def main():
    args = sys.argv[1:]
    md = os.path.join(HERE, 'FLOAT_COVERAGE.md') if '--md' not in args else args[args.index('--md') + 1]
    tsv = os.path.join(HERE, 'float_uncovered.tsv') if '--tsv' not in args else args[args.index('--tsv') + 1]
    dirs = args[args.index('--dirs') + 1].split(',') if '--dirs' in args else dump_dirs()
    db = sqlite3.connect(DB)
    pe = Pe(EXE)
    field_type = {tok: t for tok, t in db.execute("select token, type from port_fields where assembly='Terraria'")}
    ret_of = {tok: r for tok, r in db.execute("select token, ret from port_methods where assembly='Terraria'")}
    reads = collections.defaultdict(list)
    for src, off, tok in db.execute("select src_token, il_offset, field_token from port_field_refs where assembly='Terraria' and access='read' and dst_assembly='Terraria'"):
        reads[src].append(tok)
    calls = collections.defaultdict(list)
    sites = collections.Counter()
    for src, dtok, dtype, dname, dasm in db.execute("select src_token, dst_token, dst_type, dst_name, dst_assembly from port_calls where assembly='Terraria'"):
        calls[src].append((dasm, dtok, dtype, dname))
        if dasm == 'Terraria' and dtok is not None:
            sites[dtok] += 1
    methods = []
    for tok, tname, name, params, rva, il_size, ret in db.execute("select token, type_name, name, params, rva, il_size, ret from port_methods where assembly='Terraria' and has_body=1 and rva>0"):
        il = pe.il(rva)
        src = any(op in (0x22, 0x23, 0x6B, 0x6C, 0x76) for _, op in ops(il))
        if not src:
            src = any(p.strip() in FLOAT_TYPES for p in (params or '').split(',') if p.strip())
        if not src:
            src = any((field_type.get(f) or '') in FLOAT_TYPES for f in reads.get(tok, ()))
        if not src:
            for dasm, dtok, dtype, dname in calls.get(tok, ()):
                if dtype in XNA_FLOAT or (dasm == 'Terraria' and (ret_of.get(dtok) or '') in FLOAT_TYPES):
                    src = True
                    break
        if not src:
            continue
        if not any(op in ARITH for _, op in ops(il)):
            continue
        methods.append((tok, tname, name, len(il), fnv(normalized(il))))
    # the dumps: (type, method, normalized IL) -> dump names
    ran = collections.defaultdict(set)
    for d in dirs:
        tag = os.path.basename(os.path.dirname(d)) + '/' + os.path.basename(d)
        for m in json.load(open(os.path.join(d, 'jit_code.json'), encoding='utf-8')):
            if not m.get('il'):
                continue
            t, meth = m['name'].split('(')[0].rsplit('::', 1)
            # a closed instantiation of a generic type of the game (JitDump.cs JitOwnerName: `Def`n<Args>`) covers
            # the definition's method
            if t.endswith('>') and '<' in t:
                t = t[:t.index('<')]
            # a closed instantiation of a generic method (`Name<Args>`) covers the definition
            if meth.endswith('>') and meth.find('<', 1) > 0:
                meth = meth[:meth.find('<', 1)]
            ran[(t, meth, fnv(normalized(bytes.fromhex(m['il']))))].add(tag)
    rows = []
    for tok, tname, name, size, h in methods:
        by = ran.get((tname, name, h), set())
        rows.append((tname, name, tok, size, sites[tok], sorted(by)))
    cov_ref = sum(1 for r in rows if any(b.startswith('ref/') for b in r[5]))
    cov_any = sum(1 for r in rows if r[5])
    cov_client_only = cov_any - cov_ref
    unc = [r for r in rows if not r[5]]
    total_sites = sum(r[4] for r in rows)
    out = []
    out.append('# Float-rule coverage\n')
    out.append('Generated by `tools/cs2rs/float_coverage.py` (definitions in its header). Dumps: %d (%s).\n' % (len(dirs), ', '.join(os.path.basename(os.path.dirname(d)) + '/' + os.path.basename(d) for d in dirs)))
    out.append('| | methods | IL bytes | static call sites |\n|---|---:|---:|---:|')
    def agg(rs):
        return '%d | %d | %d' % (len(rs), sum(r[3] for r in rs), sum(r[4] for r in rs))
    out.append('| game methods with float arithmetic | %s |' % agg(rows))
    out.append('| ran in a server reference process (work/jit/ref) | %s |' % agg([r for r in rows if any(b.startswith('ref/') for b in r[5])]))
    out.append('| ran only in a client reference process (work/jit/client) | %s |' % agg([r for r in rows if r[5] and not any(b.startswith('ref/') for b in r[5])]))
    out.append('| covered (either) | %s |' % agg([r for r in rows if r[5]]))
    out.append('| not covered | %s |' % agg(unc))
    out.append('')
    ns = collections.defaultdict(list)
    ty = collections.defaultdict(list)
    for r in unc:
        n = r[0].rsplit('.', 1)[0] if '.' in r[0] else '(global)'
        ns[n.split('+')[0]].append(r)
        ty[r[0].split('+')[0]].append(r)
    out.append('## Not covered, by namespace\n\n| namespace | methods | IL bytes | call sites |\n|---|---:|---:|---:|')
    for n, rs in sorted(ns.items(), key=lambda kv: -sum(r[3] for r in kv[1])):
        out.append('| `%s` | %s |' % (n, agg(rs)))
    out.append('\n## Not covered, by type (the 60 largest by IL bytes)\n\n| type | methods | IL bytes | call sites |\n|---|---:|---:|---:|')
    for n, rs in sorted(ty.items(), key=lambda kv: -sum(r[3] for r in kv[1]))[:60]:
        out.append('| `%s` | %s |' % (n, agg(rs)))
    out.append('\n## Not covered: the 60 methods with the most static call sites\n\n| method | IL bytes | call sites |\n|---|---:|---:|')
    for r in sorted(unc, key=lambda r: (-r[4], -r[3]))[:60]:
        out.append('| `%s.%s` | %d | %d |' % (r[0], r[1], r[3], r[4]))
    text = '\n'.join(out) + '\n'
    open(md, 'w', encoding='utf-8', newline='\n').write(text)
    with open(tsv, 'w', encoding='utf-8', newline='\n') as f:
        for r in sorted(rows):
            f.write('%s\t%s\t%d\t%d\t%d\t%s\n' % (r[0], r[1], r[2], r[3], r[4], ','.join(r[5]) or '-'))
    print('float methods %d (IL %d bytes, %d call sites): covered %d (server runs %d, client runs only %d), not covered %d (IL %d bytes); %s, %s' % (
        len(rows), sum(r[3] for r in rows), total_sites, cov_any, cov_ref, cov_client_only, len(unc), sum(r[3] for r in unc), md, tsv))


if __name__ == '__main__':
    main()

"""draw_diff.py: the original's draw trace against the port's, frame by frame, for diagnosis.

    python tools/refdump_draw/draw_diff.py <reference dir> <port dir> [scene ...] [--show N]

Reference: <scene>.trace.zlib (tools/refdump_draw/RefDumpDraw.cs); port: <scene>.trace (port/crates/terraria-gen-game/tests/
draw.rs, written to port/target-gen/fast/draw). Per scene: calls, lines equal, lines whose only difference is a float
field (with the largest distance in units in the last place), lines that differ otherwise, and the first N of each kind.
Lines are paired in order within a frame while both frames have the same operations; a frame whose operation sequence
differs is reported as structural and its first differing operation shown.
"""
import os
import re
import struct
import sys
import zlib
from collections import Counter

HEX8 = re.compile(r'^[0-9a-f]{8}$')


def load(path, z):
    data = open(path, 'rb').read()
    if z:
        data = zlib.decompress(data)
    frames, cur, name = [], None, None
    for line in data.decode('utf-8').split('\n'):
        if not line:
            continue
        if line.startswith('frame '):
            cur = []
            frames.append((line, cur))
        else:
            cur.append(line)
    return frames


def ulps(a, b):
    def key(h):
        v = int(h, 16)
        return v if v < 0x80000000 else 0x80000000 - v
    return abs(key(a) - key(b))


def fields(line):
    out = []
    for tok in line.split(' '):
        out.extend(tok.split(','))
    return out


def compare_line(a, b):
    """('same'|'float', max ulps)|('other', None)"""
    if a == b:
        return 'same', 0
    fa, fb = fields(a), fields(b)
    if len(fa) != len(fb):
        return 'other', None
    worst = 0
    for x, y in zip(fa, fb):
        if x == y:
            continue
        if HEX8.match(x) and HEX8.match(y) and not x.startswith('ff') and not y.startswith('ff'):
            worst = max(worst, ulps(x, y))
            continue
        return 'other', None
    return 'float', worst


def op(line):
    p = line.split(' ')
    return p[0] if p[0] != 'sb.draw' else 'sb.draw ' + p[2] + ' ' + p[3]


def main():
    args = sys.argv[1:]
    show = 5
    if '--show' in args:
        i = args.index('--show')
        show = int(args[i + 1])
        del args[i:i + 2]
    ref, port = args[0], args[1]
    scenes = args[2:] or sorted(f[:-11] for f in os.listdir(ref) if f.endswith('.trace.zlib'))
    for scene in scenes:
        rp, pp = os.path.join(ref, scene + '.trace.zlib'), os.path.join(port, scene + '.trace')
        if not os.path.exists(rp) or not os.path.exists(pp):
            print(f'{scene}: missing {rp if not os.path.exists(rp) else pp}')
            continue
        r, p = load(rp, True), load(pp, False)
        calls = sum(len(f[1]) for f in r)
        same = flt = other = 0
        worst = 0
        examples = {'float': [], 'other': []}
        structural = []
        ops = Counter()
        for (rh, rl), (ph, pl) in zip(r, p):
            if [op(x) for x in rl] != [op(x) for x in pl]:
                ro, po = [op(x) for x in rl], [op(x) for x in pl]
                k = next((i for i in range(min(len(ro), len(po))) if ro[i] != po[i]), min(len(ro), len(po)))
                structural.append(f'{rh}: {len(rl)} / {len(pl)} lines, first different operation at {k}: {rl[k] if k < len(rl) else "<end>"} | {pl[k] if k < len(pl) else "<end>"}')
            for a, b in zip(rl, pl):
                kind, u = compare_line(a, b)
                if kind == 'same':
                    same += 1
                elif kind == 'float':
                    flt += 1
                    worst = max(worst, u)
                    ops[op(a)] += 1
                    if len(examples['float']) < show:
                        examples['float'].append((rh, a, b, u))
                else:
                    other += 1
                    ops[op(a)] += 1
                    if len(examples['other']) < show:
                        examples['other'].append((rh, a, b, None))
        print(f'{scene}: {calls} calls in {len(r)} frames (port {sum(len(f[1]) for f in p)} in {len(p)}); equal {same}, float-only {flt} (at most {worst} ulp), other {other}; structural frames {len(structural)}')
        for s in structural[:show]:
            print('  STRUCT ' + s[:400])
        for kind in ('float', 'other'):
            for rh, a, b, u in examples[kind]:
                print(f'  {kind.upper()} {rh}' + (f' ({u} ulp)' if u is not None else ''))
                print('    original: ' + a[:300])
                print('    port:     ' + b[:300])
        if ops:
            print('  differing lines by operation: ' + ', '.join(f'{k} {v}' for k, v in ops.most_common(12)))


if __name__ == '__main__':
    main()

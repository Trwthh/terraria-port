#!/usr/bin/env python3
"""The first frame where two effect traces differ (DRAW_FXTRACE of RefDumpDraw.cs / tests/draw.rs: every active gore
and dust after a frame, floats as bits), with the differing entries; and per gore / dust index the first frame it
differs at (to tell one diverging effect from many).

    python tools/refdump_draw/fx_diff.py <original fx.trace> <port fx.trace> [--max N]"""
import sys


def frames(path):
    out = {}
    cur = None
    for line in open(path, encoding='utf-8'):
        line = line.rstrip('\n')
        if line.startswith('frame '):
            cur = int(line[6:])
            out[cur] = {}
        elif cur is not None and line:
            p = line.split(' ')
            out[cur][(p[0], int(p[1]))] = line
    return out


def main():
    a, b = frames(sys.argv[1]), frames(sys.argv[2])
    mx = int(sys.argv[sys.argv.index('--max') + 1]) if '--max' in sys.argv else 12
    shown = False
    first_of = {}
    for f in sorted(set(a) & set(b)):
        ka, kb = a[f], b[f]
        diff = sorted(k for k in set(ka) | set(kb) if ka.get(k) != kb.get(k))
        for k in diff:
            first_of.setdefault(k, (f, ka.get(k, '<none>'), kb.get(k, '<none>')))
        if diff and not shown:
            shown = True
            print('first differing frame', f, ':', len(diff), 'entries')
            for k in diff[:mx]:
                print('  original:', ka.get(k, '<none>'))
                print('  port:    ', kb.get(k, '<none>'))
    if not shown:
        print('no difference in', len(set(a) & set(b)), 'common frames')
        return
    print('\nfirst difference per entry (the', min(len(first_of), 40), 'earliest):')
    for k, (f, x, y) in sorted(first_of.items(), key=lambda kv: kv[1][0])[:40]:
        print('  frame %d %s %d\n    original: %s\n    port:     %s' % (f, k[0], k[1], x, y))


if __name__ == '__main__':
    main()

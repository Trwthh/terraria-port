"""Per batch of an entities dump (refdata/entities/<kind>.ents.zlib format, README there): the types, the most active
NPCs / projectiles / world items in one tick, the tiles changed (citem: in the box after the last tick) and the player's
mana / life / ammo leaves that changed. A quick look at what a scenario exercises.

    python ents_stats.py <dump.ents.zlib> [max batches]
"""
import struct
import sys
import zlib


class Rd:
    def __init__(self, b):
        self.b, self.p = b, 0

    def take(self, fmt):
        v = struct.unpack_from('<' + fmt, self.b, self.p)
        self.p += struct.calcsize('<' + fmt)
        return v if len(v) > 1 else v[0]

    def s(self):
        n = self.take('H')
        v = self.b[self.p:self.p + n].decode()
        self.p += n
        return v


def main():
    data = zlib.decompress(open(sys.argv[1], 'rb').read())
    mx = int(sys.argv[2]) if len(sys.argv) > 2 else 1 << 30
    r = Rd(data)
    assert r.b[:4] == b'ENTS'
    r.p = 8
    kind = r.s()
    ticks, nb, ncls = r.take('i'), r.take('i'), r.take('i')
    leaves = []
    for _ in range(ncls):
        r.s()
        n = r.take('i')
        leaves.append([(r.s(), r.take('B')) for _ in range(n)])
    if kind == 'citem':
        r.take('ii'); r.take('4i'); r.take('4i')   # screen, tile box, reset region
        na = r.take('i')
        r.take('%di' % na) if na else None
    pl = {n: i for i, (n, k) in enumerate(leaves[2])}
    watch = [pl[x] for x in ('statMana', 'statLife', 'itemAnimation') if x in pl]
    for b in range(nb):
        r.take('i'); r.take('i'); tc = r.take('i'); r.take('Q'); ns = r.take('i')
        types = []
        for _ in range(ns):
            t = r.take('iiiiiii'); r.take('ff'); r.take('i')
            types.append(t[0])
        if kind == 'citem':
            r.take('iii')
        mxa = [0, 0, 0, 0]
        box = 0
        changed = set()
        err = False
        for _ in range(ticks + 1):
            r.take('i')
            if r.take('B'):
                err = True
                break
            r.take('I'); r.take('Q')
            for _ in range(r.take('H')):
                r.s(); r.take('I'); r.take('Q')
            for c in range(4):
                n = r.take('H')
                mxa[c] = max(mxa[c], n)
                for _ in range(n):
                    r.take('H')
                    for _ in range(r.take('H')):
                        leaf = r.take('H'); r.take('Q')
                        if c == 2 and leaf in watch:
                            changed.add(leaves[2][leaf][0])
            if kind == 'citem':
                box = r.take('i'); r.take('Q')
        if b < mx:
            print('batch %4d types %s: npc %d proj %d player %d item %d; tiles box %d; player leaves %s%s' % (
                b, types[:3], mxa[0], mxa[1], mxa[2], mxa[3], box, sorted(changed), ' ERROR' if err else ''))


if __name__ == '__main__':
    main()

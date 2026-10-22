"""Compares two Terraria .wld files section by section (the file format's section table: 0 header, 1 tiles,
2 chests, 3 signs, 4 NPCs, 5 tile entities, 6 pressure plates, 7 town manager, 8 bestiary, 9 creative powers,
10 footer).
A world the browser build generated against one the original generated with the same seed / size / options: the
header and the footer differ by design (name, GUID, world id, creation time); everything else must be equal byte for
byte.
    python web/ci/compare_wld.py a.wld b.wld
"""
import hashlib
import struct
import sys

NAMES = ["header", "tiles", "chests", "signs", "NPCs", "tile entities", "pressure plates", "town manager", "bestiary", "creative powers", "footer"]
# differ by design: the header (name, GUID, world id, creation time) and the footer (name, world id)
EXPECTED = {0, 10}


def sections(path):
    b = open(path, "rb").read()
    version = struct.unpack_from("<i", b, 0)[0]
    at = 4
    if version >= 135:
        assert b[at:at + 7] == b"relogic", "no relogic magic"
        at += 7 + 1 + 4 + 8
    count = struct.unpack_from("<h", b, at)[0]
    at += 2
    ptrs = list(struct.unpack_from("<%di" % count, b, at))
    bounds = ptrs + [len(b)]
    return version, [b[bounds[i]:bounds[i + 1]] for i in range(count)]


def main(a, b):
    va, sa = sections(a)
    vb, sb = sections(b)
    print(f"versions {va} / {vb}; sections {len(sa)} / {len(sb)}")
    bad = 0
    for i in range(max(len(sa), len(sb))):
        x = sa[i] if i < len(sa) else b""
        y = sb[i] if i < len(sb) else b""
        name = NAMES[i] if i < len(NAMES) else f"section {i}"
        same = x == y
        if not same and i not in EXPECTED:
            bad += 1
        print(f"{i:2} {name:16} {len(x):>10} / {len(y):>10} bytes  {'equal' if same else ('different (expected)' if i in EXPECTED else 'DIFFERENT')}  {hashlib.sha1(x).hexdigest()[:12]} {hashlib.sha1(y).hexdigest()[:12]}")
    print("all sections but the header and the footer equal" if bad == 0 else f"{bad} sections differ")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1], sys.argv[2]))

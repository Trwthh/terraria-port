"""Audio trace differences: the reference (refdata/audio/run.audio.zlib) against a port run (run.audio).

    python tools/refdump_draw/audio_diff.py [port run.audio] [--mask-labels] [--max N] [--ref <dir>]

Prints the first N differing lines with their frame and a few lines of context; --mask-labels compares sound effect
lines without their label (to look past a labelling difference). Diagnosis only; the gate is tests/draw.rs.
"""
import os
import re
import sys
import zlib

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))


def main():
    args = sys.argv[1:]
    mask = "--mask-labels" in args
    n = 5
    ref = os.path.join(ROOT, "refdata", "audio")
    if "--max" in args:
        n = int(args[args.index("--max") + 1])
    if "--ref" in args:
        ref = args[args.index("--ref") + 1]
    pos = [a for a in args if not a.startswith("--") and not a.isdigit() and a != ref]
    port_path = pos[0] if pos else os.path.join(ROOT, "port", "target-gen", "fast", "audio", "run.audio")
    a = zlib.decompress(open(os.path.join(ref, "run.audio.zlib"), "rb").read()).decode().split("\n")
    b = open(port_path, encoding="utf-8").read().split("\n")

    def norm(l):
        if mask and l.startswith("se.create "):
            return re.sub(r"^se.create \S+", "se.create *", l)
        if mask and l.startswith("si.create "):
            return re.sub(r"^(si.create \S+) \S+", r"\1 *", l)
        return l

    frame = "frame 0"
    shown = 0
    i = j = 0
    while i < len(a) and j < len(b):
        if a[i].startswith("frame "):
            frame = a[i]
        if norm(a[i]) != norm(b[j]):
            print(f"line {i + 1} ({frame}):")
            for k in range(max(0, i - 3), min(len(a), i + 4)):
                print(f"  {'>' if k == i else ' '} ref  {a[k]}")
            for k in range(max(0, j - 3), min(len(b), j + 4)):
                print(f"  {'>' if k == j else ' '} port {b[k]}")
            shown += 1
            if shown >= n:
                return
            # resynchronise at the next frame header of both
            while i < len(a) and not a[i].startswith("frame "):
                i += 1
            while j < len(b) and not b[j].startswith("frame "):
                j += 1
            while i < len(a) and j < len(b) and a[i] != b[j]:
                fa, fb = int(a[i][6:]), int(b[j][6:])
                if fa < fb:
                    i += 1
                    while i < len(a) and not a[i].startswith("frame "):
                        i += 1
                else:
                    j += 1
                    while j < len(b) and not b[j].startswith("frame "):
                        j += 1
            continue
        i += 1
        j += 1
    print(f"compared up to ref line {i}, port line {j} of {len(a)} / {len(b)}; {shown} difference(s) shown")


if __name__ == "__main__":
    main()

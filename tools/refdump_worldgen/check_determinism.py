#!/usr/bin/env python3
"""Compares the world-generation runs made by tools/refdump_worldgen (fresh process each) and prints the determinism report.

usage: python check_determinism.py <scratch dir containing out_<tag>/ and <tag>/Worlds/ref_small.wld> <ref_small.wld> tag[:label] ...

For every tag: sha256 of ref_small.passes.jsonl and ref_small.columns.bin (the first tag is the baseline), whether they equal the baseline, the keys of the
jsonl that differ (for the forced-date experiments), and wld_diff's verdict against the original server file.
"""
import hashlib
import json
import os
import subprocess
import sys

scratch, ref = sys.argv[1], sys.argv[2]
tags = sys.argv[3:]
here = os.path.dirname(os.path.abspath(__file__))


def sha(p):
    return hashlib.sha256(open(p, "rb").read()).hexdigest()


def rows(p):
    return [json.loads(l) for l in open(p, encoding="utf-8")]


base = None
for t in tags:
    tag = t.split(":")[0]
    d = os.path.join(scratch, "out_" + tag)
    pj, cb = os.path.join(d, "ref_small.passes.jsonl"), os.path.join(d, "ref_small.columns.bin")
    if not os.path.exists(pj):
        print("%-6s (no per-pass dump: unmodified-game run)" % tag)
    else:
        if base is None:
            base = (tag, rows(pj), sha(pj), sha(cb))
        r = rows(pj)
        diff = set()
        for a, b in zip(base[1], r):
            for k in a:
                if a[k] != b[k]:
                    if isinstance(a[k], dict):
                        diff.update(k + "." + kk for kk in a[k] if a[k][kk] != b[k][kk])
                    else:
                        diff.add(k)
        print("%-6s passes.jsonl sha256 %s  columns.bin sha256 %s  vs baseline %s: jsonl %s, columns %s%s" % (
            tag, sha(pj)[:16], sha(cb)[:16], base[0], "identical" if sha(pj) == base[2] else "DIFFERENT", "identical" if sha(cb) == base[3] else "DIFFERENT",
            ("; keys that differ: " + ", ".join(sorted(diff))) if diff else ""))
    w = os.path.join(scratch, tag, "Worlds", "ref_small.wld")
    out = subprocess.run([sys.executable, os.path.join(here, "wld_diff.py"), ref, w], capture_output=True, text=True).stdout.strip().splitlines()
    print("       .wld %d bytes, sha256 %s" % (os.path.getsize(w), sha(w)[:16]))
    for line in out:
        if line.startswith(("WorldHeader before", "manifest GenPassResults", "verdict", "Tiles ", "Chests")):
            print("       " + line)

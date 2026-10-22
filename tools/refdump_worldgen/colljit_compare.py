#!/usr/bin/env python3
r"""Proof that a RefDumpCollisionJit.exe run (CollisionJit.cs) is the run that wrote refdata\collision: every
<family>.bin, stamps.bin and world_columns.bin of its data directory must be byte-identical to refdata\collision.

usage: python colljit_compare.py <data dir of the run> [refdata\collision]
"""
import os
import sys

here = os.path.dirname(os.path.abspath(__file__))
data = sys.argv[1]
ref = sys.argv[2] if len(sys.argv) > 2 else os.path.join(here, "..", "..", "refdata", "collision")
names = sorted(n for n in os.listdir(ref) if n.endswith(".bin"))
bad = [n for n in names if not os.path.exists(os.path.join(data, n)) or open(os.path.join(data, n), "rb").read() != open(os.path.join(ref, n), "rb").read()]
print("%d of %d files identical%s" % (len(names) - len(bad), len(names), (": differ " + ", ".join(bad)) if bad else ""))
sys.exit(1 if bad else 0)

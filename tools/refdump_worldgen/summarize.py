#!/usr/bin/env python3
"""Parses the refdata/worldgen dumps with the formats documented in refdata/worldgen/README.md and prints a summary.

usage: python summarize.py [refdata/worldgen dir] [--full]

  passes      one line per row of ref_small.passes.jsonl (row, pass index, name, tile hash, genRand inext / next value)
  columns     ref_small.columns.bin: size check, and cols_fnv64 of every row recomputed from the per-column hashes
  snapshots   every *.tfrm.z: zlib + TFRM header, per-column FNV-1a 64 recomputed from the tile records and compared with the row's column hashes
              (--full also recomputes the whole-map tiles_fnv64, a sequential 70 M byte loop: about a minute per snapshot in plain Python)
  x87         every x87/*.bin: header, count, and agreement with Python's math (libm / IEEE) for the exact functions
  probes      float_probes/probes.bin: header, per-probe sample
"""
import json
import math
import os
import struct
import sys
import zlib

import numpy as np

FNV_BASIS = 14695981039346656037
FNV_PRIME = 1099511628211
MASK = (1 << 64) - 1
REC = 14


def fnv_bytes(b, h=FNV_BASIS):
    for x in b:
        h = ((h ^ x) * FNV_PRIME) & MASK
    return h


def fnv_u64s(vals):
    h = FNV_BASIS
    for v in vals:
        for k in range(8):
            h = ((h ^ ((int(v) >> (8 * k)) & 0xFF)) * FNV_PRIME) & MASK
    return h


def column_hashes(buf, w, h):
    """FNV-1a 64 of every column's h*14 bytes, vectorised over the columns."""
    a = np.frombuffer(buf, dtype=np.uint8).reshape(w, h * REC)
    hs = np.full(w, FNV_BASIS, dtype=np.uint64)
    prime = np.uint64(FNV_PRIME)
    with np.errstate(over="ignore"):
        for k in range(h * REC):
            hs = (hs ^ a[:, k].astype(np.uint64)) * prime
    return hs


def load_rows(d):
    rows = [json.loads(l) for l in open(os.path.join(d, "ref_small.passes.jsonl"), encoding="utf-8")]
    return rows


def cmd_passes(d):
    rows = load_rows(d)
    print("== passes: %d rows (row 0 = state before pass 0, rows 1..%d = after pass i, last row = after WorldGen.GenerateWorld returned)" % (len(rows), len(rows) - 2))
    for r in rows:
        print("row %3d  i %4d  %-45s tiles %s  cols %s  rand.inext %2d next %11d  active %8d" % (
            r["row"], r["i"], r["name"][:45], r["tiles_fnv64"], r["cols_fnv64"], r["rand"]["inext"], r["rand"]["next"], r["n_active"]))
    last = rows[-1]
    print("final: surface %s rockLayer %s spawn (%s,%s) dungeon (%s,%s) crimson %s chests %d npcs %d" % (
        last["main"]["worldSurface"], last["main"]["rockLayer"], last["main"]["spawnTileX"], last["main"]["spawnTileY"], last["main"]["dungeonX"],
        last["main"]["dungeonY"], last["worldgen_static"].get("crimson"), last["counts"]["chests"], last["counts"]["npcs_active"]))
    return rows


def cmd_columns(d, rows):
    p = os.path.join(d, "ref_small.columns.bin")
    data = np.fromfile(p, dtype="<u8")
    w = rows[0]["main"]["maxTilesX"]
    ok = data.size == len(rows) * w
    print("== columns.bin: %d u64 = %d rows x %d columns: %s" % (data.size, len(rows), w, "OK" if ok else "MISMATCH"))
    data = data.reshape(len(rows), w)
    bad = 0
    for i, r in enumerate(rows):
        if "%016x" % fnv_u64s(data[i]) != r["cols_fnv64"]:
            bad += 1
    print("   cols_fnv64 recomputed from the per-column hashes matches the jsonl for %d / %d rows" % (len(rows) - bad, len(rows)))
    return data


def cmd_snapshots(d, rows, cols, full):
    files = sorted(f for f in os.listdir(d) if f.endswith(".tfrm.z"))
    print("== snapshots: %d files, %.1f MB" % (len(files), sum(os.path.getsize(os.path.join(d, f)) for f in files) / 1e6))
    for f in files:
        raw = zlib.decompress(open(os.path.join(d, f), "rb").read())
        magic, ver, w, h, rec = struct.unpack_from("<4sIiiI", raw, 0)
        assert magic == b"TFRM" and ver == 1 and rec == REC, (magic, ver, rec)
        body = raw[20:]
        assert len(body) == w * h * rec
        if f.startswith("ref_small.before_pass"):
            idx = -1
        elif f.startswith("ref_small.after_generate_world"):
            idx = len(rows) - 2
        else:
            idx = int(f.split("after_pass_")[1][:3])
        row = [r for r in rows if r["i"] == idx and not (idx == len(rows) - 2 and r["extra"] is False)][-1] if idx >= len(rows) - 2 else [r for r in rows if r["i"] == idx][0]
        hs = column_hashes(body, w, h)
        ok = np.array_equal(hs, cols[row["row"]])
        line = "%-44s %dx%d  %9d B -> %9d B  row %3d: per-column hashes %s" % (f, w, h, os.path.getsize(os.path.join(d, f)), len(raw), row["row"], "MATCH" if ok else "MISMATCH")
        if full:
            line += "  tiles_fnv64 %s" % ("MATCH" if "%016x" % fnv_bytes(body) == row["tiles_fnv64"] else "MISMATCH")
        print(line)


def cmd_x87(d):
    xd = os.path.join(d, "x87")
    print("== x87 tables in", xd)
    for f in sorted(os.listdir(xd)):
        if not f.endswith(".bin"):
            continue
        raw = open(os.path.join(xd, f), "rb").read()
        magic, ver, nargs, count, _ = struct.unpack_from("<4sIIII", raw, 0)
        assert magic == b"X87T" and ver == 1
        a = np.frombuffer(raw, dtype="<u8", offset=20).reshape(count, nargs + 1)
        vals = a.view("<f8")
        name = f[:-4]
        ref = {"Sqrt": lambda x: math.sqrt(x) if x >= 0 else float("nan"), "Abs": abs, "Floor": lambda x: float(math.floor(x)) if math.isfinite(x) else x,
               "Ceiling": lambda x: float(math.ceil(x)) if math.isfinite(x) else x, "Sin": math.sin, "Cos": math.cos, "Tan": math.tan, "Atan": math.atan,
               "Exp": lambda x: math.exp(x) if x < 709.78 else float("inf"), "Log": lambda x: math.log(x) if x > 0 else (float("-inf") if x == 0 else float("nan"))}.get(name)
        note = ""
        if ref is not None:
            diff = 0
            for row, u in zip(vals, a):
                try:
                    r = ref(row[0])
                except (ValueError, OverflowError):
                    continue
                rb = struct.unpack("<Q", struct.pack("<d", r))[0]
                if rb != u[nargs] and not (math.isnan(r) and math.isnan(row[nargs])):
                    diff += 1
            note = "  bit-different from Python/libm in %d of %d records" % (diff, count)
        print("%-10s nargs %d  count %5d  first record: %s -> %r%s" % (name, nargs, count, ", ".join(repr(x) for x in vals[0][:nargs]), vals[0][nargs], note))


def cmd_probes(d):
    pd = os.path.join(d, "float_probes")
    meta = json.load(open(os.path.join(pd, "probes.json")))
    raw = open(os.path.join(pd, "probes.bin"), "rb").read()
    magic, ver, nprobes, n, _ = struct.unpack_from("<4sIIII", raw, 0)
    assert magic == b"FPRB" and nprobes == len(meta["probes"]) and n == meta["ninputs"]
    inp = np.frombuffer(raw, dtype=np.dtype([("x", "<f4"), ("y", "<f4"), ("z", "<f4"), ("n", "<i4")]), count=n, offset=20)
    off = 20 + n * 16
    print("== float probes: %d probes x %d inputs; input 0 = (%r, %r, %r, %d)" % (nprobes, n, inp[0]["x"], inp[0]["y"], inp[0]["z"], inp[0]["n"]))
    for p in meta["probes"]:
        col = np.frombuffer(raw, dtype="<u8", count=n, offset=off)
        off += n * 8
        v = col[0]
        if p["kind"] == "f":
            shown = np.array([v & 0xFFFFFFFF], dtype=np.uint32).view("<f4")[0]
        elif p["kind"] == "d":
            shown = np.array([v], dtype=np.uint64).view("<f8")[0]
        else:
            shown = np.array([v], dtype=np.uint64).view("<i8")[0]
        print("   %-34s kind %s  result[0] = %r" % (p["name"], p["kind"], shown))


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    d = args[0] if args else os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "refdata", "worldgen")
    rows = cmd_passes(d)
    cols = cmd_columns(d, rows)
    cmd_snapshots(d, rows, cols, "--full" in sys.argv)
    cmd_x87(d)
    cmd_probes(d)


if __name__ == "__main__":
    main()

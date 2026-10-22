#!/usr/bin/env python3
"""Parses the refdata/ticks dumps with the formats documented in refdata/ticks/README.md and prints a summary.

usage: python summarize.py [refdata/ticks dir]
"""
import json
import os
import struct
import sys

import numpy as np

FNV_BASIS = 14695981039346656037
FNV_PRIME = 1099511628211
MASK = (1 << 64) - 1


def fnv_u64s(vals):
    h = FNV_BASIS
    for v in vals:
        for k in range(8):
            h = ((h ^ ((int(v) >> (8 * k)) & 0xFF)) * FNV_PRIME) & MASK
    return h


def jl(path):
    return [json.loads(l) for l in open(path, encoding="utf-8")]


def scenario(d, mode):
    print("== scenario %s" % mode)
    summ = json.load(open(os.path.join(d, mode + ".summary.json")))
    print("   ticks requested %s, tick error %s, records %s, world seed %s, main.rand seed %s, halloween %s" % (
        summ["ticks_requested"], summ["tick_error"], summ["records"], summ["world_seed"], summ["main_rand_seed"], summ["main_halloween"]))
    rows = jl(os.path.join(d, mode + ".ticks.jsonl"))
    cols = np.fromfile(os.path.join(d, mode + ".columns.bin"), dtype="<u8")
    w = rows[0]["main"].get("maxTilesX", 4200) if "maxTilesX" in rows[0]["main"] else 4200
    cols = cols.reshape(len(rows), -1)
    print("   columns.bin: %d rows x %d columns; cols_fnv64 recomputed for every row: %s" % (
        cols.shape[0], cols.shape[1], "all match" if all("%016x" % fnv_u64s(cols[i]) == r["cols_fnv64"] for i, r in enumerate(rows)) else "MISMATCH"))
    for r in rows:
        m = r["main"]
        print("   row %2d tick %4d  tiles %s  changed-vs-load %5d  time %8.1f  wind %.6f  npcs %d items %d proj %d  Liquid.numLiquid %d  buffer %d  rngs %d" % (
            r["row"], r["tick"], r["tiles_fnv64"], r["tiles_changed_vs_load"], m["time"], m["windSpeedCurrent"], r["counts"]["npcs"], r["counts"]["items"], r["counts"]["projectiles"],
            r["liquid"]["Liquid.numLiquid"], r["liquid"]["LiquidBuffer.numLiquidBuffer"], len(r["rand"]["rngs"])))
    rp = jl(os.path.join(d, mode + ".rand_per_tick.jsonl"))
    print("   rand_per_tick: %d records (ticks %d..%d); main_rand.inext at tick 1/2/3 = %s; first tick where each stream moved: %s" % (
        len(rp), rp[0]["tick"], rp[-1]["tick"], [x["main_rand"]["inext"] for x in rp[:3]],
        {k: next((x["tick"] for x in rp if k in x["rngs"] and x["rngs"][k]["inext"] != 0), None) for k in sorted(rp[-1]["rngs"])}))
    tc = os.path.join(d, mode + ".tile_changes.bin")
    if os.path.exists(tc):
        raw = open(tc, "rb").read()
        magic, ver, ww, hh, n = struct.unpack_from("<4sIiiI", raw, 0)
        assert magic == b"TCHG" and len(raw) == 20 + n * (8 + 28)
        print("   tile_changes.bin: %d changed tiles (final vs load); first: x %d y %d" % (n, *struct.unpack_from("<ii", raw, 20)))
    last = rows[-1]
    print("   last row npcs:", [(e["index"], e["v"]["type"], round(e["v"]["position"]["X"], 3), round(e["v"]["position"]["Y"], 3)) for e in last["npcs"]])
    changed = json.load(open(os.path.join(d, mode + ".main_statics_changed.json")))
    print("   Main statics changed between tick 0 and the last tick: %s" % ", ".join(sorted(changed)))
    pp = os.path.join(d, "player.per_tick.jsonl")
    if mode == "player" and os.path.exists(pp):
        L = jl(pp)
        for t in (1, 60, 120, 150, 200, 201, 202, 240, 300, 360, 420, 600):
            if t <= len(L):
                p = L[t - 1]["player"]
                print("   player tick %3d pos (%9.4f, %9.4f) vel (%8.4f, %8.4f) dir %2d jump %d wingTime %s fallStart %s controls L%d R%d J%d" % (
                    t, p["position"]["X"], p["position"]["Y"], p["velocity"]["X"], p["velocity"]["Y"], p["direction"], p["jump"], p.get("wingTime"), p.get("fallStart"),
                    p["controlLeft"], p["controlRight"], p["controlJump"]))


def main():
    d = sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "refdata", "ticks")
    for mode in ("world", "player"):
        if os.path.exists(os.path.join(d, mode + ".summary.json")):
            scenario(d, mode)


if __name__ == "__main__":
    main()

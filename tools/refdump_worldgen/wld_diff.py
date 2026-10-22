#!/usr/bin/env python3
"""Field-level diff of two .wld files written by the game (version 326).

usage: python wld_diff.py A.wld B.wld

Splits both files with the section pointers of the file format header (layout: port/crates/terraria-formats/src/wld/mod.rs), compares section by
section, and inside WorldHeader classifies every difference:
  guid           WorldFileData.UniqueId (16 bytes at header offset 24 after the two strings and the generator version)
  creation_time  WorldFlags.creation_time (i64 DateTime.ToBinary)
  last_played    WorldFlags.last_played   (i64)
  manifest       the JSON string WorldGen.Manifest.Serialize(): compared field by field, DurationMs (a Stopwatch value) listed separately
  pointers       the 11 section pointers in the FileFormatHeader (they move when the manifest text changes length)
  other          anything else (must be empty for a deterministic generation)
Sections after the header (Tiles, Chests, Signs, NPCs, TileEntities, WeightedPressurePlates, TownManager, Bestiary, CreativePowers, Footer)
are compared byte for byte.
"""
import json
import struct
import sys

NAMES = ["FileFormatHeader", "WorldHeader", "Tiles", "Chests", "Signs", "NPCs", "TileEntities", "WeightedPressurePlates",
         "TownManager", "Bestiary", "CreativePowers", "Footer"]


def read7(d, o):
    n = 0
    shift = 0
    while True:
        b = d[o]
        o += 1
        n |= (b & 0x7F) << shift
        if not b & 0x80:
            return n, o
        shift += 7


def parse(path):
    d = open(path, "rb").read()
    version, = struct.unpack_from("<i", d, 0)
    magic, revision, fav = struct.unpack_from("<QIQ", d, 4)
    count, = struct.unpack_from("<h", d, 24)
    ptrs = list(struct.unpack_from("<%di" % count, d, 26))
    bounds = [0] + ptrs + [len(d)]
    sec = {n: d[bounds[i]:bounds[i + 1]] for i, n in enumerate(NAMES)}
    # header: name, seed text, generator version u64, guid[16], world id i32, 6 x i32, flags (game_mode i32, 9 bools, creation_time i64, last_played i64, ...)
    h = sec["WorldHeader"]
    o = 0
    n, o = read7(h, o); name = h[o:o + n]; o += n
    n, o = read7(h, o); seed = h[o:o + n]; o += n
    gen_ver, = struct.unpack_from("<Q", h, o); o += 8
    guid_off = o; o += 16
    world_id, = struct.unpack_from("<i", h, o); o += 4
    o += 24
    o += 4 + 9
    ct_off = o; creation, = struct.unpack_from("<q", h, o); o += 8
    lp_off = o; last, = struct.unpack_from("<q", h, o); o += 8
    mi = h.index(b'{"GenPassResults"')
    # the string length prefix is 1-3 bytes before the JSON
    k = mi - 1
    while k > 0 and h[k - 1] & 0x80:
        k -= 1
    mlen, mo = read7(h, k)
    assert mo == mi, (mo, mi)
    manifest = json.loads(h[mi:mi + mlen])
    return dict(path=path, d=d, version=version, magic=magic, revision=revision, fav=fav, ptrs=ptrs, sec=sec, name=name, seed=seed, gen_ver=gen_ver,
                guid=h[guid_off:guid_off + 16], guid_off=guid_off, world_id=world_id, creation=creation, last=last, ct_off=ct_off, lp_off=lp_off,
                manifest=manifest, manifest_off=k, manifest_len=mlen, header=h)


def main():
    a, b = parse(sys.argv[1]), parse(sys.argv[2])
    for w in (a, b):
        print("%s: %d bytes, version %d, revision %d, ptrs %s" % (w["path"], len(w["d"]), w["version"], w["revision"], w["ptrs"]))
        print("   name=%r seed=%r gen_ver=%d world_id=%d guid=%s creation=%d last_played=%d manifest_json_bytes=%d" % (
            w["name"], w["seed"], w["gen_ver"], w["world_id"], w["guid"].hex(), w["creation"], w["last"], w["manifest_len"]))
    print()
    for n in NAMES:
        x, y = a["sec"][n], b["sec"][n]
        if n in ("FileFormatHeader", "WorldHeader"):
            continue
        print("%-24s %9d vs %9d  %s" % (n, len(x), len(y), "IDENTICAL" if x == y else "DIFFERENT"))
    # FileFormatHeader: everything but pointers
    x, y = a["sec"]["FileFormatHeader"], b["sec"]["FileFormatHeader"]
    pdiff = [i for i in range(len(x)) if x[i] != y[i]]
    inptr = all(26 <= i < 26 + 44 for i in pdiff)
    print("%-24s %9d vs %9d  bytes differing: %d, all inside the 11 section pointers (file offsets 26..70): %s" % ("FileFormatHeader", len(x), len(y), len(pdiff), inptr))
    # WorldHeader
    ha, hb = a["header"], b["header"]
    ma, mb = a["manifest_off"], b["manifest_off"]
    pre_a, pre_b = bytearray(ha[:ma]), bytearray(hb[:mb])
    diffs = {"guid": 0, "creation_time": 0, "last_played": 0, "other": []}
    if len(pre_a) != len(pre_b):
        print("WorldHeader before the manifest differs in length: %d vs %d" % (len(pre_a), len(pre_b)))
    else:
        for i in range(len(pre_a)):
            if pre_a[i] != pre_b[i]:
                if a["guid_off"] <= i < a["guid_off"] + 16: diffs["guid"] += 1
                elif a["ct_off"] <= i < a["ct_off"] + 8: diffs["creation_time"] += 1
                elif a["lp_off"] <= i < a["lp_off"] + 8: diffs["last_played"] += 1
                else: diffs["other"].append(i)
        print("WorldHeader before the manifest (%d bytes): differing bytes: guid %d, creation_time %d, last_played %d, other %s" % (
            len(pre_a), diffs["guid"], diffs["creation_time"], diffs["last_played"], diffs["other"][:20]))
    ja, jb = a["manifest"], b["manifest"]
    print("manifest keys", list(ja.keys()), "| Version %r/%r GitSHA %r/%r FinalHash %r/%r" % (ja["Version"], jb["Version"], ja["GitSHA"], jb["GitSHA"], ja["FinalHash"], jb["FinalHash"]))
    ra, rb = ja["GenPassResults"], jb["GenPassResults"]
    same = len(ra) == len(rb) and all(p["Name"] == q["Name"] and p["RandNext"] == q["RandNext"] and p["Hash"] == q["Hash"] and p["Skipped"] == q["Skipped"] for p, q in zip(ra, rb))
    print("manifest GenPassResults: %d vs %d passes; Name/RandNext/Hash/Skipped identical for every pass: %s" % (len(ra), len(rb), same))
    print("manifest DurationMs (wall-clock, differs by design): first 5: %s vs %s" % ([r["DurationMs"] for r in ra[:5]], [r["DurationMs"] for r in rb[:5]]))
    tail_a, tail_b = ha[ma + (a["manifest_len"] and 0):], hb[mb:]
    print("verdict:", "tiles/chests/signs/NPCs/... identical, header differs only in guid/creation_time/last_played/manifest durations (+pointers)"
          if all(a["sec"][n] == b["sec"][n] for n in NAMES[2:]) and not diffs["other"] and same and inptr else "UNEXPECTED DIFFERENCES: investigate")


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Tests the translator's float rule "expression trees in f64, rounded to f32 at stores / arguments / returns / casts" against the real 32-bit JIT results
in refdata/worldgen/float_probes/probes.bin. Prints, per probe, how many of the 2000 results the model reproduces bit for bit.

usage: python floatprobe_model.py [refdata/worldgen dir]

Trigonometry / Pow in the model use Python's libm, so probes 25, 26 and 30 also measure the x87 maths difference (see x87/). The XNA Vector2 methods are
modelled from the XNA 4.0 source (Length: float num = X*X + Y*Y; sqrt((double)num); Normalize: num2 = 1f / (float)Math.Sqrt(num); X *= num2; Y *= num2).
"""
import json
import math
import os
import struct
import sys

import numpy as np

d = sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "refdata", "worldgen")
pd = os.path.join(d, "float_probes")
meta = json.load(open(os.path.join(pd, "probes.json")))
raw = open(os.path.join(pd, "probes.bin"), "rb").read()
_, _, nprobes, N, _ = struct.unpack_from("<4sIIII", raw, 0)
inp = np.frombuffer(raw, dtype=np.dtype([("x", "<f4"), ("y", "<f4"), ("z", "<f4"), ("n", "<i4")]), count=N, offset=20)
off = 20 + N * 16
res = {}
for p in meta["probes"]:
    res[p["name"]] = np.frombuffer(raw, dtype="<u8", count=N, offset=off)
    off += N * 8

x, y, z = (inp[k].astype(np.float64) for k in "xyz")
n = inp["n"].astype(np.int64)


def r(a):
    return np.asarray(a, dtype=np.float64).astype(np.float32).astype(np.float64)


def bits(a):
    return np.asarray(a, dtype=np.float64).astype(np.float32).view("<u4").astype("<u8")


def ibits(a):
    return np.asarray(a, dtype=np.int64).astype(np.int32).astype(np.int64).view("<u8")


def trunc(a):
    return np.trunc(a)


def sin(a): return np.array([math.sin(v) for v in a])
def cos(a): return np.array([math.cos(v) for v in a])
def atan2(b, a): return np.array([math.atan2(p, q) for p, q in zip(b, a)])


with np.errstate(all="ignore"):
    c01 = r(0.1); c097 = r(0.97); c05 = r(0.5); c001 = r(0.01); c0625 = r(0.0625); c3 = r(1 / 3)
    model = {}
    model["P01_mul_add_store_sub"] = bits(r(r(x * y + z) - x))
    model["P02_mul_add_return"] = bits(x * y + z)
    a = r(x); a = r(a + y * z); a = r(a * x); a = r(a - y)
    model["P03_compound_locals"] = bits(a)
    model["P04_compound_static_field"] = bits(a)
    model["P05_compound_instance_field"] = bits(a)
    model["P06_int_cast_mul"] = ibits(trunc(x * y))
    model["P07_int_cast_mul_add"] = ibits(trunc(x * y + z))
    model["P08_cmp_store_vs_expr"] = (r(x * y) == x * y).astype("<u8")
    model["P09_cmp_products"] = ((x * y > z * z).astype("<u8") | ((x * y == y * x).astype("<u8") << 1) | (((x + y) * z == x * z + y * z).astype("<u8") << 2) | ((x * y + z == z + y * x).astype("<u8") << 3))
    s = np.zeros(N)
    for i in range(32):
        s = np.where(i < (n & 31) + 1, r(s + x * c01), s)
    model["P10_loop_accum"] = bits(s)
    s = np.ones(N)
    for i in range(8):
        s = r(s * c097 + x * c05 - z * c001)
    model["P11_loop_recurrence"] = bits(s)
    vx = r(x + y * z); vy = r(vx * z - y)
    model["P12_vector2_fields"] = bits(vx + vy)
    cx = r(r(x + r(z * c05)) - r(x * z)); cy = r(r(y + r(x * c05)) - r(y * z))
    model["P13_vector2_ops"] = bits(cx + cy * 2)
    num = r(x * x + y * y)
    model["P14_vector2_length"] = bits(np.sqrt(num))
    num2 = r(1.0 / r(np.sqrt(num)))
    model["P15_vector2_normalize"] = bits(r(x * num2) + r(y * num2) * 3)
    model["P16_call_arg"] = bits(r(x * y) + z)
    model["P17_double_from_float_product"] = (x * y * z).view("<u8")
    model["P18_double_from_float_expr"] = (x * y + z).view("<u8")
    model["P19_sqrt_float_product"] = bits(np.sqrt(np.abs(x * y)))
    model["P20_sqrt_sum_squares"] = bits(np.sqrt(x * x + y * y))
    q = np.where(n >= 0, n // 7, -((-n) // 7)); m5 = np.where(n >= 0, n % 5, -((-n) % 5))
    model["P21_mixed_int_float"] = bits(q * x + (n / 3.0) * y + m5 / 2.0)
    model["P22_product_chain"] = bits(x * y * z)
    model["P23_assoc_residue"] = bits((x + y) + z - (x + (y + z)))
    model["P24_tile_coord"] = ibits(trunc(x * c0625) + trunc((x + y) / 16.0) * 1000)
    model["P25_trig_cast"] = bits(r(cos(x)) * y + r(sin(x)) * z)
    model["P26_atan2_cast"] = bits(r(atan2(y, x)) * z)
    model["P27_minmax"] = bits(np.maximum(r(x * y), r(z * z)) - np.minimum(r(x * y), z))
    model["P28_div_const"] = bits(x / 3 * y + x * c3 * z)
    model["P29_int_to_float"] = bits(n * x + y - n)
    model["P30_pow_cast"] = bits(r(np.array([math.pow(abs(v), 1.5) for v in x])) * y)

    # ---- model B: float LOCALS (and fields of local structs, inlined operator temporaries) are not rounded; only stores to fields / statics / arrays,
    # ---- call arguments, returns and explicit casts round.
    B = {}
    B["P01_mul_add_store_sub"] = bits((x * y + z) - x)
    a = x.copy(); a = a + y * z; a = a * x; a = a - y
    B["P03_compound_locals"] = bits(a)
    B["P08_cmp_store_vs_expr"] = np.ones(N, dtype="<u8")
    s = np.zeros(N)
    for i in range(32):
        s = np.where(i < (n & 31) + 1, s + x * c01, s)
    B["P10_loop_accum"] = bits(s)
    s = np.ones(N)
    for i in range(8):
        s = s * c097 + x * c05 - z * c001
    B["P11_loop_recurrence"] = bits(s)
    vx = x + y * z; vy = vx * z - y
    B["P12_vector2_fields"] = bits(vx + vy)
    B["P13_vector2_ops"] = bits(((x + z * c05) - x * z) + ((y + x * c05) - y * z) * 2)
    B["P14_vector2_length"] = bits(np.sqrt(x * x + y * y))
    n2 = 1.0 / r(np.sqrt(x * x + y * y))
    B["P15_vector2_normalize"] = bits(r(x * n2) + r(y * n2) * 3)
    # probes 31-34: A = every float local store rounds, B = locals never round
    a0 = x * y; a1 = a0 + z; a2 = a1 * x; a3 = a2 - y; a4 = a3 * z; a5 = a4 + a0; a6 = a5 * a1; a7 = a6 - a2; a8 = a7 + a3 * c05; a9 = a8 * a4 + a5
    B["P31_ten_locals"] = bits(a9 + a0 + a1 + a2 + a3 + a4 + a5 + a6 + a7 + a8)
    ra0 = r(x * y); ra1 = r(ra0 + z); ra2 = r(ra1 * x); ra3 = r(ra2 - y); ra4 = r(ra3 * z); ra5 = r(ra4 + ra0); ra6 = r(ra5 * ra1); ra7 = r(ra6 - ra2); ra8 = r(ra7 + ra3 * c05); ra9 = r(ra8 * ra4 + ra5)
    model["P31_ten_locals"] = bits(ra9 + ra0 + ra1 + ra2 + ra3 + ra4 + ra5 + ra6 + ra7 + ra8)
    model["P32_local_across_call"] = bits(r(x * y + z) - x); B["P32_local_across_call"] = bits((x * y + z) - x)
    s = np.zeros(N); sb = np.zeros(N)
    for i in range(8):
        s = np.where(i < (n & 7) + 1, r(s + x * c01), s); sb = np.where(i < (n & 7) + 1, sb + x * c01, sb)
    model["P33_loop_with_call"] = bits(s); B["P33_loop_with_call"] = bits(sb)
    model["P34_local_address_taken"] = bits(r(x * y + z) - x); B["P34_local_address_taken"] = bits((x * y + z) - x)

print("%-34s %-22s %-22s" % ("probe", "model A: round at stores", "model B: locals unrounded"))
for p in meta["probes"]:
    nm = p["name"]
    a_ok = int((model[nm] == res[nm]).sum())
    b = ("%4d / %d" % (int((B[nm] == res[nm]).sum()), N)) if nm in B else "(same as A)"
    print("%-34s %4d / %d%s   %s" % (nm, a_ok, N, "  " if a_ok == N else " *", b))

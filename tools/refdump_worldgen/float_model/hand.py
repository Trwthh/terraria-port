#!/usr/bin/env python3
"""The hand-written probes of the second set (real XNA members, helpers without NoInlining): for each, the candidate readings of the
C# and how many of the 2000 results each reproduces. `r` = rounding to float. The machine code (calls.py / disasm.py) says which
calls were inlined; this script says what that means for the values.   usage: python hand.py [refdata/worldgen dir]"""
import os, sys
import numpy as np
from dsl import r32 as r
from score import load

D = sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", "refdata", "worldgen")
inp, res, meta = load("float_probes2", "probes2.bin", "probes2.json")
x, y, z = (inp[k].astype(np.float64) for k in "xyz")
n = inp["n"].astype(np.int64)
c05, c15, c16 = 0.5, 1.5, 16.0


def fb(a): return r(a).astype(np.float32).view("<u4").astype("<u8")
def ib(a): return np.asarray(a, dtype=np.int64).astype(np.int32).astype(np.int64).view("<u8")
def clamp(v, lo, hi):
    v = np.where(v > hi, hi, v)
    return np.where(v < lo, lo, v)


with np.errstate(all="ignore"):
    xy = x * y
    T = [
        ("Q_I02_float_arg_int_result", [("inlined, argument rounded to float", ib(np.trunc(r(xy) * c16))), ("argument unrounded", ib(np.trunc(xy * c16)))]),
        ("Q_I03_int_arg_float_result", [("n * 0.5f * x", fb(n * c05 * x))]),
        ("Q_I04_float_args_bool_result", [("arguments rounded", ib(r(xy) > r(z * z))), ("arguments unrounded", ib(xy > z * z))]),
        ("Q_I05_float_arg_void", [("argument rounded, then the field store", fb(r(r(xy) * c15))), ("argument unrounded", fb(r(xy * c15)))]),
        ("Q_I06_double_helper", [("double argument = unrounded product", fb(xy * c15 + z)), ("product rounded to float first", fb(r(xy) * c15 + z))]),
        ("Q_I07_struct_helper", [("struct fields unrounded (inlined, promoted)", fb(x * z + y * z * 2)), ("result fields rounded", fb(r(x * z) + r(y * z) * 2))]),
        ("Q_I08_property_getters", [("Half returns its unrounded expression, (float)int rounded to float", fb(r(xy) * z + r(xy) * c05 + r((np.trunc(r(xy)) + 0.0)))),
                                    ("the same with (float)int unrounded", fb(r(xy) * z + r(xy) * c05 + (np.trunc(r(xy)) + 0.0))),
                                    ("Half rounded at return", fb(r(xy) * z + r(r(xy) * c05) + r((np.trunc(r(xy)) + 0.0))))]),
        ("Q_V01_distance", [("locals of Distance unrounded", fb(r(np.sqrt((x - z) ** 2 + (y - x) ** 2)))),
                            ("num2 / num / num3 rounded", fb(r(np.sqrt(r(r(x - z) ** 2 + r(y - x) ** 2)))))]),
        ("Q_V02_dot", [("return value unrounded", fb((x * z + y * x) + z)), ("return value rounded", fb(r(x * z + y * x) + z))]),
        ("Q_V03_scale_length", [("scaled vector stored (rounded) for the instance call", fb(r(np.sqrt(r(x * z) ** 2 + r(y * z) ** 2)))),
                                ("components unrounded", fb(r(np.sqrt((x * z) ** 2 + (y * z) ** 2))))]),
        ("Q_V04_static_vector_field", [("operator temporaries unrounded, field stores rounded", fb(r(x + z * c05) + r(y + x * c05))),
                                       ("temporaries rounded", fb(r(x + r(z * c05)) + r(y + r(x * c05))))]),
        ("Q_V05_lerp", [("return value unrounded", fb((x + (y - x) * z) - x)), ("return value rounded", fb(r(x + (y - x) * z) - x))]),
        ("Q_V06_clamp", [("argument rounded, x*y computed once and reused unrounded", fb(clamp(r(xy), z, 10.0) + xy)),
                         ("argument unrounded", fb(clamp(xy, z, 10.0) + xy))]),
        ("Q_V07_local_vector_scaled", [("fields of the local vector unrounded", fb((x * z + y) * 3 + (y * z + z))),
                                       ("fields rounded at each operator", fb(r(r(x * z) + y) * 3 + r(r(y * z) + z)))]),
        ("Q_V08_length_squared", [("vector stored (rounded), return value unrounded", fb((r(xy) ** 2 + z ** 2) - x)),
                                  ("return value rounded", fb(r(r(xy) ** 2 + z ** 2) - x)), ("x*y unrounded", fb((xy ** 2 + z ** 2) - x))]),
    ]
for name, cands in T:
    want = res[name]
    print(name)
    for label, got in cands:
        k = int((got == want).sum())
        print("    %4d / %d  %s%s" % (k, len(want), label, "" if k == len(want) else "   *"))

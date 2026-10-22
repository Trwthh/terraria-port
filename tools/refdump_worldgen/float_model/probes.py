"""Probe definitions in the language of dsl.py.
NEW: the second probe set (compiled into ExtProbesGen.cs by gen_cs.py, results in refdata/worldgen/float_probes2/probes2.bin).
OLD: the first probe set (tools/refdump_worldgen/FloatProbe.cs, results in refdata/worldgen/float_probes/probes.bin) re-expressed in the same
language so that every rule is scored on all probes. P12..P15 call real XNA Vector2 members; they are written here the way the JIT
compiles them (constructor and operators inlined, the struct's fields promoted to separate locals: section 6 of FLOAT_RULE.md).
P25, P26, P30 need the x87 sin/cos/atan2/pow: they are checked by the Rust crate (tests/probes.rs), not here.
"""
x, y, z = ("p", "x"), ("p", "y"), ("p", "z")
N = ("n",)


def L(n): return ("l", n)
def K(v): return ("k", v)
def add(*a):
    e = a[0]
    for b in a[1:]: e = ("+", e, b)
    return e
def sub(a, b): return ("-", a, b)
def mul(*a):
    e = a[0]
    for b in a[1:]: e = ("*", e, b)
    return e
def div(a, b): return ("/", a, b)
def call(f, *a): return ("call", f, list(a))
def S(n, e): return ("set", n, e)
def ret(e): return ("ret", e)


CNT8 = ("iadd", ("iand", N, ("ki", 7)), ("ki", 1))        # (n & 7) + 1
CNT32 = ("iadd", ("iand", N, ("ki", 31)), ("ki", 1))

NEW = {}


def new(name, doc, body, **kw):
    d = {"doc": doc, "body": body}; d.update(kw); NEW[name] = d


# No common subexpressions (the JIT would give them their own register, see C2x) and every parameter read once (a parameter read once
# is not enregistered), so that all six registers are available to the locals.
C = [0.37, 0.71, 1.3, 1.7, 0.9, 1.9, 0.3, 2.3, 0.43, 1.1, 0.77, 3.1]
base = [S("a0", mul(x, K(C[0]))), S("a1", mul(y, K(C[1]))), S("a2", mul(z, K(C[2]))), S("a3", mul(L("a0"), K(C[3]))), S("a4", mul(L("a1"), K(C[4]))), S("a5", mul(L("a2"), K(C[5])))]
new("L06_six_live", "six float locals live at the end (a0..a2: 3 references, a3..a5: 2)", base + [ret(add(*[L("a%d" % i) for i in range(6)]))])
seven = base + [S("a6", mul(L("a3"), K(C[6])))]
new("L07_seven_live", "seven locals live together: a0..a3 have 3 references, a4..a6 have 2", seven + [ret(add(*[L("a%d" % i) for i in range(7)]))])
new("L12_decl_reversed", "the same seven locals declared in the reverse order", seven + [ret(add(*[L("a%d" % i) for i in range(7)]))], decl=["a6", "a5", "a4", "a3", "a2", "a1", "a0"])
new("L08_refcount_order", "eight locals live together, the two declared last referenced most",
    seven + [S("a7", mul(L("a4"), K(C[7]))), ret(add(add(*[L("a%d" % i) for i in range(8)]), mul(L("a6"), L("a7")), mul(L("a7"), L("a6"))))])
new("L09_two_phases", "eight locals, never more than four live at a time",
    [S("a0", mul(x, K(C[0]))), S("a1", mul(y, K(C[1]))), S("a2", mul(z, K(C[2]))), S("a3", mul(L("a0"), K(C[3]))), ("ssf", add(L("a0"), L("a1"), L("a2"), L("a3"))),
     S("b0", mul(("sf",), K(C[4]))), S("b1", mul(L("b0"), K(C[5]))), S("b2", mul(L("b1"), K(C[6]))), S("b3", mul(L("b2"), K(C[7]))), ret(add(L("b0"), L("b1"), L("b2"), L("b3")))])
new("L10_seven_one_dead", "seven locals, the first one dead before the seventh is assigned",
    base + [("ssf", mul(L("a0"), K(C[8]))), S("a6", mul(L("a3"), K(C[6]))), ret(add(L("a1"), L("a2"), L("a3"), L("a4"), L("a5"), L("a6"), ("sf",)))])
new("L13_param_competes", "six locals and one parameter that is read six times",
    [S("a0", mul(x, K(C[0]))), S("a1", mul(x, K(C[1]))), S("a2", mul(x, K(C[2]))), S("a3", mul(x, K(C[3]))), S("a4", mul(x, K(C[4]))), S("a5", mul(x, K(C[5]))),
     ret(add(*[L("a%d" % i) for i in range(6)]))])
new("L14_deep_expression", "six locals live while an expression needs three more stack slots",
    base + [ret(add(add(*[L("a%d" % i) for i in range(6)]), mul(add(mul(x, K(C[8])), mul(y, K(C[9]))), add(mul(z, K(C[10])), mul(y, K(C[11]))))))])
new("C20_common_subexpression", "the same product used by two locals (the JIT computes it once)",
    [S("a", add(mul(x, y), z)), S("b", sub(mul(x, y), z)), ret(mul(L("a"), L("b")))])
new("C21_cse_and_many_locals", "three products each used twice plus six locals",
    [S("a0", mul(x, y)), S("a1", mul(y, z)), S("a2", mul(x, z)), S("a3", add(mul(x, y), z)), S("a4", add(mul(y, z), x)), S("a5", add(mul(x, z), y)), ret(add(*[L("a%d" % i) for i in range(6)]))])

new("C01_use_before_and_after_call", "local used before a call (stored product) and twice after it",
    [S("a", add(mul(x, y), z)), ("ssf", mul(L("a"), y)), S("b", call("Id", x)), ret(add(mul(L("a"), L("b")), L("a"), ("sf",)))])
new("C02_three_refs_one_call", "local with three references, live across one call",
    [S("a", add(mul(x, y), z)), ("ssf", sub(L("a"), x)), ("do", call("Id", y)), ret(add(L("a"), ("sf",)))])
new("C03_four_refs_two_calls", "local with four references, live across two calls",
    [S("a", add(mul(x, y), z)), ("ssf", sub(L("a"), x)), ("do", call("Id", y)), ("do", call("Id", z)), ret(add(mul(L("a"), y), L("a"), ("sf",)))])
new("C04_temp_across_call", "an expression temporary (float product) pending while a call runs", [ret(add(mul(x, y), call("Id", z)))])
new("C05_local_then_call", "an enregistered local as the left operand of an addition whose right operand is a call",
    [S("a", mul(x, y)), ret(add(L("a"), call("Id", z)))])
new("C07_inline_single_use", "argument expression of an inlined method that uses its parameter once", [ret(add(call("InlMul", mul(x, y)), z))])
new("C08_inline_multi_use", "argument expression of an inlined method that uses its parameter three times", [ret(sub(call("InlSq", mul(x, y)), z))])
new("C09_noinline_return", "float expression returned by a non-inlined method, used by the caller without a store", [ret(sub(call("Mad", x, y, z), x))])
new("C10_inline_three_args", "inlined a * b + c with one expression argument", [ret(sub(call("InlMad", mul(x, y), z, x), y))])
new("C11_loop_call_use_before", "accumulator used before the call inside the loop",
    [S("s", K(0.0)), ("loop", CNT8, [("aug", "s", "+", mul(x, K(0.1))), ("ssf", mul(L("s"), y)), ("do", call("Id", K(1.0)))]), ret(add(L("s"), ("sf",)))])
new("C12_many_refs_many_calls", "local with six references live across two calls",
    [S("a", add(mul(x, y), z)), ("ssf", sub(L("a"), x)), ("do", call("Id", y)), ("asf", "+", mul(L("a"), z)), ("do", call("Id", z)),
     ret(add(mul(L("a"), y), mul(L("a"), L("a")), ("sf",)))])

new("S01_if_both_branches", "local assigned in both branches of an if",
    [("if", (">", x, y), [S("a", add(mul(x, y), z))], [S("a", add(mul(x, z), y))]), ret(sub(L("a"), x))])
new("S03_param_in_place", "float parameter modified in place", [("augp", "x", "+", mul(y, z)), ret(sub(mul(x, y), z))])
new("S03b_param_assigned_once_used_once", "float parameter overwritten, then read once", [("setp", "y", add(mul(x, z), y)), ret(sub(y, x))])
new("S04_conditional_expr", "conditional expression stored in a local", [S("a", ("cond", (">", x, y), mul(x, y), mul(z, y))), ret(add(L("a"), mul(L("a"), z)))])
new("S05_cmp_local_vs_field", "an unrounded local compared with the field it was stored to", [S("a", mul(x, y)), ("ssf", L("a")), ("reti", ("b2i", ("==", L("a"), ("sf",))))])
new("S06_loop_two_accumulators", "two interdependent loop-carried locals",
    [S("s", K(0.0)), S("t", K(1.0)), ("loop", CNT8, [("aug", "s", "+", mul(x, L("t"))), S("t", add(mul(L("t"), K(0.97)), mul(y, K(0.01))))]), ret(add(L("s"), L("t")))])
new("S07_loop_if", "accumulator updated in both branches of an if inside a loop",
    [S("s", K(0.0)), ("loop", CNT8, [("if", (">", x, y), [("aug", "s", "+", mul(x, K(0.1)))], [("aug", "s", "-", mul(y, K(0.1)))])]), ret(L("s"))])
new("S08_nested_loops", "accumulator in a nested loop, a second local only outside",
    [S("a", add(mul(x, y), z)), S("s", K(0.0)), ("loop", ("ki", 3), [("loop", ("ki", 3), [("aug", "s", "+", mul(x, K(0.1)))]), ("aug", "s", "*", K(0.97))]), ret(add(L("s"), sub(L("a"), x)))])

new("T01_try_assigned_in_try", "local assigned inside a try block and in its handler",
    [S("a", add(mul(x, y), z)), ("try", [S("a", add(mul(L("a"), y), x)), ("ssf", L("a"))], [S("a", K(0.0))]), ret(sub(L("a"), x))])
new("T02_try_unrelated_local", "method with a try/catch; the local is assigned and used after it",
    [("try", [("ssf", mul(x, y))], [("ssf", K(0.0))]), S("b", add(mul(x, y), z)), ret(add(sub(L("b"), x), ("sf",)))])
new("T03_local_live_through_try", "local assigned before a try, read inside it and after it",
    [S("a", add(mul(x, y), z)), ("try", [("ssf", mul(L("a"), y))], [("ssf", K(0.0))]), ret(add(sub(L("a"), x), ("sf",)))])

new("M01_array_element", "array element store and reload", [("sar", add(mul(x, y), z)), ret(sub(("ar",), x))])
new("M02_local_struct_fields", "fields of a local struct (own type V2) written and read",
    [S("v.X", add(mul(x, y), z)), S("v.Y", sub(mul(L("v.X"), z), y)), ret(add(L("v.X"), mul(L("v.Y"), x)))])
new("M03_cast_float_expr", "(float) cast of a float expression inside a larger expression", [ret(add(("castf", mul(x, y)), z))])
new("M03b_cast_float_local", "(float) cast stored to a local", [S("a", ("castf", add(mul(x, y), z))), ret(sub(L("a"), x))])
new("M03c_cast_float_of_local", "(float) cast applied to an unrounded local", [S("a", add(mul(x, y), z)), ret(sub(("castf", L("a")), x))])
new("M05_int_cast_of_local", "(int) casts of a local holding a float product",
    [S("a", mul(x, y)), ("reti", ("iadd", ("f2i", L("a")), ("imul", ("f2i", mul(L("a"), K(16.0))), ("ki", 1000))))])
new("M06_ref_local_other_local", "one local passed by ref, another one not",
    [S("a", add(mul(x, y), z)), S("b", add(mul(x, z), y)), ("ssf", sub(L("b"), x)), ("ref", "a"), ret(add(sub(L("a"), x), ("sf",)))])


def filler(k):
    return [("raw", "SI += n * 3 + 1;")] * k


core = [S("a", add(mul(x, y), z)), S("b", sub(L("a"), x)), ("aug", "b", "+", mul(y, z)), ("aug", "b", "*", x), ret(add(sub(L("b"), y), L("a")))]
new("B00_core_small", "the float core of the B probes alone", core)
new("B01_il_64k", "the same core after 4000 integer statements (about 64000 bytes of IL, 32000 instructions)", filler(4000) + core, instrs=32030)
new("B02_il_32k", "the same core after 2000 integer statements (about 32000 bytes of IL, 16000 instructions)", filler(2000) + core, instrs=16030)
new("B03_il_42k", "the same core after 2600 integer statements (about 41600 bytes of IL, 20800 instructions)", filler(2600) + core, instrs=20830)
new("B03b_il_56k", "the same core after 3500 integer statements (about 56000 bytes of IL, 28000 instructions)", filler(3500) + core, instrs=28030)


def hot_ints(k):
    decl = " ".join("int j%d = n + %d;" % (i, i) for i in range(k))
    use = " ".join("j%d += n ^ %d; j%d *= 3; j%d -= n; j%d ^= j%d;" % (i, i, i, i, i, (i + 1) % k) for i in range(k))
    end = "SI = " + " + ".join("j%d" % i for i in range(k)) + ";"
    return decl + " " + use, end


for k in (40, 70, 140, 300):
    pre, end = hot_ints(k)
    new("B1%d_%d_hot_ints" % ({40: 0, 70: 1, 140: 2, 300: 3}[k], k), "the same core in a method with %d int locals that are referenced more often than the floats" % k,
        [("raw", pre)] + core[:-1] + [("raw", end)] + core[-1:], hotter_locals=k + 1)      # k int locals and the parameter n

# ---------------------------------------------------------------------------------------------- first probe set, re-expressed
OLD = {}


def old(name, body):
    OLD[name] = {"doc": name, "body": body}


old("P01_mul_add_store_sub", [S("a", add(mul(x, y), z)), ret(sub(L("a"), x))])
old("P02_mul_add_return", [ret(add(mul(x, y), z))])
old("P03_compound_locals", [S("a", x), ("aug", "a", "+", mul(y, z)), ("aug", "a", "*", x), ("aug", "a", "-", y), ret(L("a"))])
old("P04_compound_static_field", [("ssf", x), ("asf", "+", mul(y, z)), ("asf", "*", x), ("asf", "-", y), ret(("sf",))])
old("P05_compound_instance_field", [("ssf", x), ("asf", "+", mul(y, z)), ("asf", "*", x), ("asf", "-", y), ret(("sf",))])
old("P06_int_cast_mul", [("reti", ("f2i", mul(x, y)))])
old("P07_int_cast_mul_add", [("reti", ("f2i", add(mul(x, y), z)))])
old("P10_loop_accum", [S("s", K(0.0)), ("loop", CNT32, [("aug", "s", "+", mul(x, K(0.1)))]), ret(L("s"))])
old("P11_loop_recurrence", [S("s", K(1.0)), ("loop", ("ki", 8), [S("s", sub(add(mul(L("s"), K(0.97)), mul(x, K(0.5))), mul(z, K(0.01))))]), ret(L("s"))])
old("P12_vector2_fields", [S("v.X", x), S("v.Y", y), ("aug", "v.X", "+", mul(L("v.Y"), z)), S("v.Y", sub(mul(L("v.X"), z), L("v.Y"))), ret(add(L("v.X"), L("v.Y")))])
old("P13_vector2_ops", [S("t1.X", mul(z, K(0.5))), S("t1.Y", mul(x, K(0.5))), S("t2.X", add(x, L("t1.X"))), S("t2.Y", add(y, L("t1.Y"))), S("t3.X", mul(x, z)), S("t3.Y", mul(y, z)),
                        S("c.X", sub(L("t2.X"), L("t3.X"))), S("c.Y", sub(L("t2.Y"), L("t3.Y"))), ret(add(L("c.X"), mul(L("c.Y"), K(2.0))))])
old("P14_vector2_length", [S("num", add(mul(x, x), mul(y, y))), ret(("sqrtf", L("num")))])
old("P16_call_arg", [ret(add(call("Id", mul(x, y)), z))])
old("P19_sqrt_float_product", [S("r", ("sqrtf", ("abs", mul(x, y)))), ret(L("r"))])
old("P20_sqrt_sum_squares", [ret(("sqrtf", add(mul(x, x), mul(y, y))))])
old("P22_product_chain", [ret(mul(x, y, z))])
old("P23_assoc_residue", [ret(sub(add(add(x, y), z), add(x, add(y, z))))])
old("P28_div_const", [ret(add(mul(div(x, K(3.0)), y), mul(x, K(1.0 / 3.0), z)))])
old("P29_int_to_float", [ret(sub(add(mul(("i2f", N), x), y), ("i2f", N)))])
old("P31_ten_locals", [S("a0", mul(x, y)), S("a1", add(L("a0"), z)), S("a2", mul(L("a1"), x)), S("a3", sub(L("a2"), y)), S("a4", mul(L("a3"), z)), S("a5", add(L("a4"), L("a0"))),
                       S("a6", mul(L("a5"), L("a1"))), S("a7", sub(L("a6"), L("a2"))), S("a8", add(L("a7"), mul(L("a3"), K(0.5)))), S("a9", add(mul(L("a8"), L("a4")), L("a5"))),
                       ret(add(*[L("a%d" % i) for i in (9, 0, 1, 2, 3, 4, 5, 6, 7, 8)]))])
old("P32_local_across_call", [S("a", add(mul(x, y), z)), S("b", call("Id", x)), ret(sub(L("a"), L("b")))])
old("P33_loop_with_call", [S("s", K(0.0)), ("loop", CNT8, [("aug", "s", "+", mul(x, K(0.1))), ("do", call("Id", K(1.0)))]), ret(L("s"))])
old("P34_local_address_taken", [S("a", add(mul(x, y), z)), ("ref", "a"), ret(sub(L("a"), x))])
old("P08_cmp_store_vs_expr", [S("p", mul(x, y)), ("reti", ("b2i", ("==", L("p"), mul(x, y))))])
old("P15_vector2_normalize", [S("num", add(mul(x, x), mul(y, y))), S("num2", div(K(1.0), ("sqrtf", L("num")))), ("ssf", mul(x, L("num2"))), ("shf", mul(y, L("num2"))), ret(add(("sf",), mul(("hf",), K(3.0))))])
old("P17_double_from_float_product", [("retd", mul(mul(x, y), z))])
old("P18_double_from_float_expr", [("retd", add(mul(x, y), z))])
old("P24_tile_coord", [("reti", ("iadd", ("f2i", mul(x, K(0.0625))), ("imul", ("f2i", div(add(x, y), K(16.0))), ("ki", 1000))))])
old("P27_minmax", [ret(sub(call("Max", mul(x, y), mul(z, z)), call("Min", mul(x, y), z)))])

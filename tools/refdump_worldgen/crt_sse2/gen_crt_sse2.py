#!/usr/bin/env python3
"""Generates port/crates/terraria-x87/src/crt_sse2_gen.rs: a mechanical, instruction-by-instruction transliteration of the SSE2 maths routines
that the 32-bit CLR 4 actually runs for Math.Pow / Log / Log10 / Acos / Asin / Exp.

Why: clr.dll (x86) imports _CIpow, _CIlog, _CIlog10, _CIacos, _CIasin, _CIexp (and _CIsinh/_CIcosh/_CItanh/_CIfmod) from ucrtbase_clr0400.dll.
Each _CIxxx stub tests the "SSE2 available" flag, MXCSR (all exceptions masked) and the x87 control word (all exceptions masked) and then jumps
to an SSE2 routine (Intel LIBM style: table driven, about 0.5 ulp, NOT correctly rounded). The only way to return the same bits is to run the
same sequence of IEEE double operations with the same tables. This script disassembles those routines (capstone), follows every reachable
branch, and emits one Rust function per routine operating on the small register machine in crt_cpu.rs. The constant tables the routines read
are copied into the generated file. Nothing is tuned by hand: re-running the script reproduces the file byte for byte.

usage: python gen_crt_sse2.py [path to ucrtbase_clr0400.dll (32-bit)] [output .rs]
Requires: capstone (pip install capstone).
"""
import hashlib, os, re, struct, sys
import capstone

HERE = os.path.dirname(os.path.abspath(__file__))
DLL = sys.argv[1] if len(sys.argv) > 1 else r"C:\Windows\SysWOW64\ucrtbase_clr0400.dll"
OUT = sys.argv[2] if len(sys.argv) > 2 else os.path.join(HERE, "..", "..", "..", "port", "crates", "terraria-x87", "src", "crt_sse2_gen.rs")

# export name -> rust function name
FUNCS = [("_CIpow", "pow"), ("_CIlog", "log"), ("_CIlog10", "log10"), ("_CIacos", "acos"), ("_CIasin", "asin"), ("_CIexp", "exp")]


class PE:
    def __init__(s, path):
        s.d = d = open(path, "rb").read()
        pe = struct.unpack_from("<I", d, 0x3C)[0]
        nsec = struct.unpack_from("<H", d, pe + 6)[0]
        optsz = struct.unpack_from("<H", d, pe + 20)[0]
        opt = pe + 24
        s.base = struct.unpack_from("<I", d, opt + 28)[0]
        s.secs = []
        so = opt + optsz
        for i in range(nsec):
            vs, va, rs, ro = struct.unpack_from("<IIII", d, so + 8)
            s.secs.append((va, vs, ro, rs)); so += 40
        exp_rva = struct.unpack_from("<I", d, opt + 96)[0]
        o = s.off(exp_rva)
        nfun, nnames, afun, anames, aords = struct.unpack_from("<IIIII", d, o + 20)
        s.exports = {}
        for i in range(nnames):
            no = s.off(struct.unpack_from("<I", d, s.off(anames) + 4 * i)[0])
            name = d[no:d.index(b"\0", no)].decode()
            ordi = struct.unpack_from("<H", d, s.off(aords) + 2 * i)[0]
            s.exports[name] = struct.unpack_from("<I", d, s.off(afun) + 4 * ordi)[0]

    def off(s, rva):
        for va, vs, ro, rs in s.secs:
            if va <= rva < va + max(vs, rs): return rva - va + ro
        raise KeyError(hex(rva))

    def read(s, va, n):
        o = s.off(va - s.base); return s.d[o:o + n]


pe = PE(DLL)
md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)


def one(va):
    for ins in md.disasm(pe.read(va, 16), va):
        return ins
    raise Exception("cannot decode %x" % va)


def sse2_target(stub_va):
    """the _CIxxx stub: ... cmp ax,0x7f ; lea esp,[esp+8] ; jne x87 ; jmp <sse2 routine>"""
    va = stub_va
    for _ in range(40):
        ins = one(va)
        if ins.mnemonic == "jmp": return int(ins.op_str, 16)
        va += ins.size
    raise Exception("no jmp in stub")


JCC = {"je": "c.zf", "jne": "!c.zf", "ja": "(!c.cf && !c.zf)", "jae": "!c.cf", "jb": "c.cf", "jbe": "(c.cf || c.zf)", "jg": "(!c.zf && c.sf == c.of)",
       "jge": "(c.sf == c.of)", "jl": "(c.sf != c.of)", "jle": "(c.zf || c.sf != c.of)", "jp": "c.pf", "jnp": "!c.pf", "js": "c.sf", "jns": "!c.sf"}
R32 = ["eax", "ecx", "edx", "ebx", "esp", "ebp", "esi", "edi"]
EXTERNAL = {}          # va -> name; filled while collecting: the error-support routine


def is_external(t):
    # a call target that starts with "mov edi, edi" is a compiled C function (hot-patch prologue), not part of the hand-written routine
    ins = one(t)
    if ins.mnemonic == "mov" and ins.op_str == "edi, edi":
        EXTERNAL[t] = "ext_%x" % t
        return True
    return False


class Patch:
    """a short instruction run replaced by one helper call (see PATCHES)"""
    def __init__(s, va, end, code, text):
        s.address = va; s.size = end - va; s.mnemonic = "patch"; s.op_str = text; s.code = code


# _CIexp, result needs scaling by 2^k outside the normal range: the routine switches the x87 unit to 64-bit precision (fnstcw / or 0x300 /
# fldcw), computes (xmm0 * xmm6 + xmm6) * xmm4 with fmul / faddp / fmulp, stores the result as a double and restores the control word.
# The register machine has no extended-precision x87 stack; the two runs are replaced by a no-op and by one helper that performs the
# three operations with 64-bit significands (crt_cpu::ext_mul_add_mul). (va, end, first mnemonic, last mnemonic, code, text)
PATCHES = [
    (0x1008b287, 0x1008b29c, "fnstcw", "fldcw", "", "x87 control word := 64-bit precision"),
    (0x1008b2cf, 0x1008b2fb, "movlpd", "fldcw", "c.x[0][0] = crate::crt_cpu::ext_mul_add_mul(c.x[0][0], c.x[6][0], c.x[4][0]);",
     "st = (xmm0 * xmm6 + xmm6) * xmm4 at 64-bit precision; xmm0 = (double)st; restore control word"),
]
PATCH_AT = {}
for va, end, first, last, code, text in PATCHES:
    a = va; names = []
    while a < end:
        i = one(a); names.append(i.mnemonic); a += i.size
    if a == end and names[0] == first and names[-1] == last:
        PATCH_AT[va] = Patch(va, end, code, text)
    else:
        print("WARNING: patch site 0x%x does not match this DLL build (%s); _CIexp will report 'unsupported' there" % (va, names[:3]))


def collect(entry):
    seen = {}; work = [entry]; leaders = {entry}
    while work:
        va = work.pop()
        while va not in seen:
            ins = PATCH_AT.get(va) or one(va); seen[va] = ins
            m = ins.mnemonic; nxt = va + ins.size
            if m in JCC:
                t = int(ins.op_str, 16); leaders.add(t); leaders.add(nxt); work.append(t)
            elif m == "jmp":
                t = int(ins.op_str, 16); leaders.add(t); work.append(t); break
            elif m == "call":
                t = int(ins.op_str, 16)
                leaders.add(nxt)
                if t not in EXTERNAL and not is_external(t):
                    leaders.add(t); work.append(t)
            elif m == "ret":
                break
            va = nxt
    return seen, leaders


class Unsupported(Exception):
    pass


data_refs = set()


def mem_addr(expr):
    """'eax*4 + 0x10013f60' -> rust u32 expression"""
    expr = expr.replace(" ", "")
    terms = re.findall(r"[+-]?[^+-]+", expr)
    out = []
    for t in terms:
        sign = "-" if t.startswith("-") else "+"
        t = t.lstrip("+-")
        if "*" in t:
            r, sc = t.split("*"); v = "c.r[%d].wrapping_mul(%s)" % (R32.index(r), sc)
        elif t in R32:
            v = "c.r[%d]" % R32.index(t)
        else:
            n = int(t, 16)
            if n >= pe.base: data_refs.add(n)
            v = "0x%x" % n
        out.append((sign, v))
    s = out[0][1] if out[0][0] == "+" else "0u32.wrapping_sub(%s)" % out[0][1]
    for sign, v in out[1:]:
        s = "%s.wrapping_%s(%s)" % (s, "add" if sign == "+" else "sub", v)
    return s


def parse(op):
    op = op.strip()
    m = re.match(r"(byte|word|dword|qword|xmmword|tbyte) ptr \[(.*)\]$", op)
    if m: return ("m", m.group(1), mem_addr(m.group(2)))
    if op.startswith("xmm"): return ("x", int(op[3:]))
    if op in R32: return ("r", R32.index(op))
    if re.match(r"-?(0x)?[0-9a-f]+$", op): return ("i", int(op, 16) if "0x" in op else int(op))
    if op.startswith("st("): return ("st", int(op[3]))
    raise Unsupported("operand " + op)


def rd32(o):
    if o[0] == "r": return "c.r[%d]" % o[1]
    if o[0] == "i": return "0x%xu32" % (o[1] & 0xFFFFFFFF)
    if o[0] == "m" and o[1] == "dword": return "c.rd32(%s)" % o[2]
    raise Unsupported("rd32 %r" % (o,))


def wr32(o, v):
    if o[0] == "r": return "c.r[%d] = %s;" % (o[1], v)
    if o[0] == "m" and o[1] == "dword": return "{ let v = %s; c.wr32(%s, v); }" % (v, o[2])
    raise Unsupported("wr32 %r" % (o,))


def lo64(o):
    """low 64 bits of an xmm register or a qword memory operand"""
    if o[0] == "x": return "c.x[%d][0]" % o[1]
    if o[0] == "m" and o[1] in ("qword", "xmmword"): return "c.rd64(%s)" % o[2]
    raise Unsupported("lo64 %r" % (o,))


def v128(o):
    if o[0] == "x": return "c.x[%d]" % o[1]
    if o[0] == "m" and o[1] == "xmmword": return "c.rd128(%s)" % o[2]
    raise Unsupported("v128 %r" % (o,))


def emit(ins, ids):
    m = ins.mnemonic
    if m == "patch": return ins.code or "{}"
    ops = [parse(o) for o in ins.op_str.split(",")] if ins.op_str else []
    nxt = ins.address + ins.size
    if m in JCC: return "if %s { pc = %d; continue; }" % (JCC[m], ids[int(ins.op_str, 16)])
    if m == "jmp": return "pc = %d; continue;" % ids[int(ins.op_str, 16)]
    if m == "call":
        t = int(ins.op_str, 16)
        if t in EXTERNAL: return "c.error_support();"
        return "c.push32(%d); pc = %d; continue;" % (ids[nxt], ids[t])
    if m == "ret": return "pc = c.pop32(); continue;"
    if m == "nop" or m == "wait": return ""
    if m == "leave": return "c.r[4] = c.r[5]; c.r[5] = c.pop32();"
    if m == "push": return "{ let v = %s; c.push32(v); }" % rd32(ops[0])
    if m == "pop": return "c.r[%d] = c.pop32();" % ops[0][1]
    if m == "lea": return "c.r[%d] = %s;" % (ops[0][1], ops[1][2])
    if m == "mov": return wr32(ops[0], rd32(ops[1]))
    if m in ("add", "sub", "and", "or", "xor", "cmp", "test", "sbb", "adc"):
        a, b = rd32(ops[0]), rd32(ops[1])
        if m == "cmp": return "{ let (a, b) = (%s, %s); c.f_sub(a, b); }" % (a, b)
        if m == "test": return "{ let (a, b) = (%s, %s); c.f_and(a, b); }" % (a, b)
        return "{ let (a, b) = (%s, %s); let v = c.f_%s(a, b); %s }" % (a, b, m, wr32(ops[0], "v"))
    if m in ("shl", "shr", "sar"):
        return "{ let a = %s; let v = c.f_%s(a, %d); %s }" % (rd32(ops[0]), m, ops[1][1], wr32(ops[0], "v"))
    if m in ("neg", "inc", "dec", "not"):
        return "{ let a = %s; let v = c.f_%s(a); %s }" % (rd32(ops[0]), m, wr32(ops[0], "v"))
    if m == "cmove": return "if c.zf { c.r[%d] = %s; }" % (ops[0][1], rd32(ops[1]))
    # ---- SSE2 moves
    if m in ("movapd", "movdqa", "movupd", "movaps"):
        if ops[0][0] == "x": return "c.x[%d] = %s;" % (ops[0][1], v128(ops[1]))
        return "{ let v = c.x[%d]; let a = %s; c.wr64(a, v[0]); c.wr64(a.wrapping_add(8), v[1]); }" % (ops[1][1], ops[0][2])
    if m == "movsd":
        if ops[0][0] == "x" and ops[1][0] == "x": return "c.x[%d][0] = c.x[%d][0];" % (ops[0][1], ops[1][1])
        if ops[0][0] == "x": return "c.x[%d] = [%s, 0];" % (ops[0][1], lo64(ops[1]))
        return "{ let v = c.x[%d][0]; c.wr64(%s, v); }" % (ops[1][1], ops[0][2])
    if m == "movlpd":
        if ops[0][0] == "x": return "c.x[%d][0] = %s;" % (ops[0][1], lo64(ops[1]))
        return "{ let v = c.x[%d][0]; c.wr64(%s, v); }" % (ops[1][1], ops[0][2])
    if m == "movq":
        if ops[0][0] == "x": return "c.x[%d] = [%s, 0];" % (ops[0][1], lo64(ops[1]))
        return "{ let v = c.x[%d][0]; c.wr64(%s, v); }" % (ops[1][1], ops[0][2])
    if m == "movd":
        if ops[0][0] == "x": return "c.x[%d] = [%s as u64, 0];" % (ops[0][1], rd32(ops[1]))
        return wr32(ops[0], "c.x[%d][0] as u32" % ops[1][1])
    # ---- scalar double arithmetic (low lane), packed double arithmetic (both lanes)
    sd = {"addsd": "add", "subsd": "sub", "mulsd": "mul", "divsd": "div"}
    if m in sd: return "c.x[%d][0] = Cpu::%s(c.x[%d][0], %s);" % (ops[0][1], sd[m], ops[0][1], lo64(ops[1]))
    pd = {"addpd": "add", "subpd": "sub", "mulpd": "mul", "divpd": "div"}
    if m in pd: return "{ let b = %s; let a = c.x[%d]; c.x[%d] = [Cpu::%s(a[0], b[0]), Cpu::%s(a[1], b[1])]; }" % (v128(ops[1]), ops[0][1], ops[0][1], pd[m], pd[m])
    if m == "sqrtsd": return "c.x[%d][0] = Cpu::sqrt(%s);" % (ops[0][1], lo64(ops[1]))
    lg = {"andpd": "A & B", "orpd": "A | B", "xorpd": "A ^ B", "pxor": "A ^ B", "pand": "A & B", "por": "A | B", "andnpd": "!A & B", "pandn": "!A & B"}
    if m in lg:
        e0 = lg[m].replace("A", "a[0]").replace("B", "b[0]"); e1 = lg[m].replace("A", "a[1]").replace("B", "b[1]")
        return "{ let b = %s; let a = c.x[%d]; c.x[%d] = [%s, %s]; }" % (v128(ops[1]), ops[0][1], ops[0][1], e0, e1)
    if m == "unpcklpd": return "{ let b = %s; c.x[%d][1] = b[0]; }" % (v128(ops[1]), ops[0][1])
    if m == "unpckhpd": return "{ let b = %s; let a = c.x[%d]; c.x[%d] = [a[1], b[1]]; }" % (v128(ops[1]), ops[0][1], ops[0][1])
    if m == "shufpd": return "{ let b = %s; let a = c.x[%d]; c.x[%d] = [a[%d], b[%d]]; }" % (v128(ops[1]), ops[0][1], ops[0][1], ops[2][1] & 1, (ops[2][1] >> 1) & 1)
    if m == "pshufd": return "{ let b = %s; c.x[%d] = Cpu::pshufd(b, %d); }" % (v128(ops[1]), ops[0][1], ops[2][1])
    if m in ("psrlq", "psllq"):
        op = ">>" if m == "psrlq" else "<<"; n = ops[1][1]
        if ops[1][0] != "i":      # shift count taken from the low quadword of an xmm register / memory operand
            return "{ let n = %s; let a = c.x[%d]; c.x[%d] = if n > 63 { [0, 0] } else { [a[0] %s n, a[1] %s n] }; }" % (lo64(ops[1]), ops[0][1], ops[0][1], op, op)
        if n >= 64: return "c.x[%d] = [0, 0];" % ops[0][1]
        return "{ let a = c.x[%d]; c.x[%d] = [a[0] %s %d, a[1] %s %d]; }" % (ops[0][1], ops[0][1], op, n, op, n)
    if m in ("psubq", "paddq"):
        f = "wrapping_sub" if m == "psubq" else "wrapping_add"
        return "{ let b = %s; let a = c.x[%d]; c.x[%d] = [a[0].%s(b[0]), a[1].%s(b[1])]; }" % (v128(ops[1]), ops[0][1], ops[0][1], f, f)
    if m in ("psubd", "paddd", "pcmpeqd", "pmaxsw"):
        return "{ let b = %s; let a = c.x[%d]; c.x[%d] = Cpu::%s(a, b); }" % (v128(ops[1]), ops[0][1], ops[0][1], m)
    if m == "pmovmskb": return "c.r[%d] = Cpu::pmovmskb(c.x[%d]);" % (ops[0][1], ops[1][1])
    if m == "pextrw": return "c.r[%d] = ((c.x[%d][%d] >> %d) & 0xffff) as u32;" % (ops[0][1], ops[1][1], ops[2][1] // 4, 16 * (ops[2][1] % 4))
    if m == "pinsrw":
        l, sh = ops[2][1] // 4, 16 * (ops[2][1] % 4)
        return "{ let v = (%s as u64) & 0xffff; c.x[%d][%d] = (c.x[%d][%d] & !(0xffffu64 << %d)) | (v << %d); }" % (rd32(ops[1]), ops[0][1], l, ops[0][1], l, sh, sh)
    if m == "cvtsi2sd": return "c.x[%d][0] = ((%s as i32) as f64).to_bits();" % (ops[0][1], rd32(ops[1]))
    if m == "cvtdq2pd": return "{ let b = %s; c.x[%d] = [((b[0] as u32 as i32) as f64).to_bits(), (((b[0] >> 32) as u32 as i32) as f64).to_bits()]; }" % (v128(ops[1]), ops[0][1])
    if m == "cvtsd2si": return "c.r[%d] = Cpu::cvtsd2si(%s);" % (ops[0][1], lo64(ops[1]))
    if m == "cvttsd2si": return "c.r[%d] = Cpu::cvttsd2si(%s);" % (ops[0][1], lo64(ops[1]))
    if m in ("cmpeqsd", "cmpltsd", "cmplesd", "cmpneqsd", "cmpnltsd", "cmpnlesd", "cmpunordsd", "cmpordsd"):
        return "c.x[%d][0] = Cpu::cmpsd(c.x[%d][0], %s, %d);" % (ops[0][1], ops[0][1], lo64(ops[1]), ["eq", "lt", "le", "unord", "neq", "nlt", "nle", "ord"].index(m[3:-2]))
    if m in ("comisd", "ucomisd"): return "{ let b = %s; let a = c.x[%d][0]; c.comisd(a, b); }" % (lo64(ops[1]), ops[0][1])
    # ---- the few x87 instructions the SSE2 routines use to hand the result back (values are doubles: exact)
    if m == "fld":
        if ops[0][0] == "m" and ops[0][1] == "qword": return "{ let v = c.rd64(%s); c.fpush(v); }" % ops[0][2]
        if ops[0][0] == "st": return "{ let v = c.fst(%d); c.fpush(v); }" % ops[0][1]
    if m == "fld1": return "c.fpush(0x3ff0000000000000);"
    if m == "fldz": return "c.fpush(0);"
    if m == "fstp":
        if ops[0][0] == "m" and ops[0][1] == "qword": return "{ let v = c.fpop(); c.wr64(%s, v); }" % ops[0][2]
        if ops[0][0] == "st": return "{ let v = c.fpop(); if %d > 0 { c.fset(%d, v); } }" % (ops[0][1], ops[0][1] - 1)
    if m == "fst" and ops[0][0] == "m" and ops[0][1] == "qword": return "{ let v = c.fst(0); c.wr64(%s, v); }" % ops[0][2]
    if m == "fxch": return "c.fxch(%d);" % (ops[0][1] if ops else 1)
    if m in ("fadd", "fmul", "fsub", "fdiv") and ops and ops[0][0] == "m" and ops[0][1] == "qword":
        return "{ let b = c.rd64(%s); let a = c.fpop(); c.fpush(Cpu::%s(a, b)); }" % (ops[0][2], m[1:])
    if m in ("fadd", "fmul") and len(ops) == 1 and ops[0][0] == "st":
        return "{ let b = c.fst(%d); let a = c.fpop(); c.fpush(Cpu::%s(a, b)); }" % (ops[0][1], m[1:])
    if m == "fabs": return "{ let a = c.fpop(); c.fpush(a & 0x7fffffffffffffff); }"
    if m == "fchs": return "{ let a = c.fpop(); c.fpush(a ^ 0x8000000000000000); }"
    raise Unsupported("%s %s" % (m, ins.op_str))


def gen(export, rust):
    entry = sse2_target(pe.base + pe.exports[export])
    seen, leaders = collect(entry)
    addrs = sorted(seen)
    leaders = sorted(l for l in leaders if l in seen)
    ids = {a: i for i, a in enumerate(leaders)}
    lines = []; unsup = []
    lines.append("/// `%s` of ucrtbase_clr0400.dll (x86), SSE2 routine at 0x%08x, %d instructions reachable." % (export, entry, len(addrs)))
    lines.append("#[allow(unused_assignments, unused_braces, unused_variables, clippy::all)]")
    lines.append("pub(crate) fn %s(c: &mut Cpu) {" % rust)
    lines.append("    let mut pc: u32 = 0;")
    lines.append("    loop {")
    lines.append("        match pc {")
    for bi, lead in enumerate(leaders):
        lines.append("            %d => {" % bi)
        if os.environ.get("CRT_TRACE"): lines.append('                eprintln!("blk %d @%08x eax={:x} ecx={:x} edx={:x}", c.r[0], c.r[1], c.r[2]);' % (bi, lead))
        a = lead; closed = False
        while True:
            ins = seen[a]
            try:
                code = emit(ins, ids)
            except Unsupported as e:
                unsup.append("%08x %s %s" % (ins.address, ins.mnemonic, ins.op_str))
                code = "c.unsupported(0x%x); return;" % ins.address
            if code: lines.append("                %-110s // %08x %s %s" % (code, ins.address, ins.mnemonic, ins.op_str))
            nxt = a + ins.size
            if ins.mnemonic in ("jmp", "ret") or code.startswith("c.unsupported") or (ins.mnemonic == "call" and "continue" in code):
                closed = True; break
            if nxt in ids or nxt not in seen:
                break
            a = nxt
        if not closed:
            if nxt in ids: lines.append("                pc = %d;" % ids[nxt])
            else: lines.append("                c.unsupported(0x%x); return;" % nxt)
        lines.append("            }")
    lines.append("            _ => return,")
    lines.append("        }")
    lines.append("    }")
    lines.append("}")
    return "\n".join(lines), unsup, entry, len(addrs)


body = []; report = []
for export, rust in FUNCS:
    code, unsup, entry, n = gen(export, rust)
    body.append(code)
    report.append((export, entry, n, unsup))

lo = min(data_refs) & ~0xF
hi = (max(data_refs) + 0x1010) & ~0xF
blob = pe.read(lo, hi - lo)                 # every table read is "base + small index": one contiguous range of the image
words = struct.unpack("<%dQ" % (len(blob) // 8), blob)
sha = hashlib.sha256(pe.d).hexdigest()
hdr = []
hdr.append("// GENERATED by tools/refdump_worldgen/crt_sse2/gen_crt_sse2.py. Do not edit.")
hdr.append("// Source binary: ucrtbase_clr0400.dll (32-bit), %d bytes, sha256 %s" % (len(pe.d), sha))
hdr.append("// A literal transliteration of the SSE2 code paths behind _CIpow/_CIlog/_CIlog10/_CIacos/_CIasin/_CIexp; the comment on each line is the")
hdr.append("// original instruction. Constant tables: image range 0x%08x..0x%08x." % (lo, hi))
hdr.append("#![allow(unused_parens)]")
hdr.append("use crate::crt_cpu::Cpu;")
hdr.append("")
hdr.append("pub(crate) const DATA_BASE: u32 = 0x%08x;" % lo)
hdr.append("pub(crate) static DATA: [u64; %d] = [" % len(words))
for i in range(0, len(words), 6):
    hdr.append("    " + " ".join("0x%016x," % w for w in words[i:i + 6]))
hdr.append("];")
hdr.append("")
open(OUT, "w", newline="\n").write("\n".join(hdr) + "\n" + "\n\n".join(body) + "\n")
for export, entry, n, unsup in report:
    print("%-9s sse2 routine 0x%08x  %4d instructions  unsupported: %d" % (export, entry, n, len(unsup)))
    for u in unsup: print("      " + u)
print("data 0x%08x..0x%08x (%d bytes), refs %d; external calls: %s" % (lo, hi, hi - lo, len(data_refs), {hex(k): v for k, v in EXTERNAL.items()}))
print("dll sha256", sha)

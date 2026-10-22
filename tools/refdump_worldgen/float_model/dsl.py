"""A tiny language for float probes: one definition per probe, used three ways:
  * emit_cs()        the C# method the 32-bit runtime executes (tools/refdump_worldgen/ExtProbesGen.cs);
  * Interp           a vectorised evaluator (numpy float64 = the x87 unit at 53-bit precision) with a pluggable rounding policy;
  * jit_enreg()      the legacy x86 JIT's choice of float locals that live on the x87 stack (FLOAT_RULE.md section 3).

Expressions (tuples):
  ('p', 'x')            float parameter x / y / z            ('n',)            the int parameter n
  ('k', 0.5)            float constant                       ('l', 'a')        float local (name 'v.X' = field X of the local struct v)
  ('+'|'-'|'*'|'/', a, b)                                    ('neg', a)
  ('sf',) ('hf',) ('ar',)   static field SF / instance field H.F / array element F[0]
  ('call', 'Id', [args])    call of a helper (HELPERS below)
  ('castf', e) ('castd', e) (float)e, (double)e              ('i2f', ie)       (float)intExpr
  ('sqrtf', e)              (float)Math.Sqrt(e)              ('cond', c, a, b) c ? a : b
Int expressions: ('n',) ('ki', 3) ('f2i', e) ('iadd', a, b) ('imul', a, b) ('iand', a, b)
Conditions: ('<'|'>'|'=='|'<='|'>='|'!=', a, b) on float expressions.
Statements:
  ('set', 'a', e) ('aug', 'a', '+', e)     local store / compound assignment      ('setp', 'x', e) ('augp', 'x', '+', e)   the same on a parameter
  ('ssf', e) ('shf', e) ('sar', e)         stores to the static field, the instance field, the array element
  ('asf', '+', e)                          SF += e
  ('if', c, [then], [else])   ('loop', ie, [body])   for (int i = 0; i < ie; i++)
  ('do', callexpr)            ('ref', 'a')  Touch(ref a)
  ('try', [body], [handler])  try { } catch (Exception) { }
  ('raw', 'C# text')          emitted verbatim, ignored by the evaluator (integer filler)
  ('ret', e) / ('reti', ie)
"""
import numpy as np

# helper methods available to probes: name -> (inlining, params, body expression over ('a',i))
HELPERS = {
    "Id": ("no", 1, ("a", 0)),                                             # [NoInlining] float Id(float f) { return f; }
    "Mad": ("no", 3, ("+", ("*", ("a", 0), ("a", 1)), ("a", 2))),           # [NoInlining] float Mad(float a, float b, float c) { return a * b + c; }
    # The three "Inl" helpers carry no NoInlining attribute and are tiny, but the JIT does NOT inline them: a method whose float return
    # value is an arithmetic expression is never inlined (probes2_code.bin: the call instruction is there; FLOAT_RULE.md section 6).
    "InlMul": ("no", 1, ("*", ("a", 0), ("k", 1.5))),                       # float InlMul(float a) { return a * 1.5f; }
    "InlSq": ("no", 1, ("+", ("*", ("a", 0), ("a", 0)), ("a", 0))),         # float InlSq(float a) { return a * a + a; }
    "Max": ("no", 2, ("max", ("a", 0), ("a", 1))),                          # Math.Max(float, float) / Math.Min: not inlined (first probe set, P27)
    "Min": ("no", 2, ("min", ("a", 0), ("a", 1))),
    "InlMad": ("no", 3, ("+", ("*", ("a", 0), ("a", 1)), ("a", 2))),       # float InlMad(float a, float b, float c) { return a * b + c; }
}
HELPERS_CS = """    [MethodImpl(MethodImplOptions.NoInlining)] static float Id(float f) { return f; }
    [MethodImpl(MethodImplOptions.NoInlining)] static float Mad(float a, float b, float c) { return a * b + c; }
    static float InlMul(float a) { return a * 1.5f; }
    static float InlSq(float a) { return a * a + a; }
    static float InlMad(float a, float b, float c) { return a * b + c; }
    [MethodImpl(MethodImplOptions.NoInlining)] static void Touch(ref float f) { f = f; }
"""


# ------------------------------------------------------------------------------------------------ C# emission
def fl(v):
    s = repr(float(np.float32(v)))
    return (s if "e" not in s and "." in s else repr(float(v))) + "f"


def cs_e(e):
    t = e[0]
    if t == "p": return e[1]
    if t == "n": return "n"
    if t == "k": return fl(e[1])
    if t == "ki": return str(e[1])
    if t == "l": return e[1]
    if t in "+-*/": return "(%s %s %s)" % (cs_e(e[1]), t, cs_e(e[2]))
    if t == "neg": return "(-%s)" % cs_e(e[1])
    if t == "abs": return "Math.Abs(%s)" % cs_e(e[1])
    if t == "sf": return "SF"
    if t == "hf": return "H.F"
    if t == "ar": return "F[0]"
    if t == "call": return "%s(%s)" % (e[1], ", ".join(cs_e(a) for a in e[2]))
    if t == "castf": return "(float)(%s)" % cs_e(e[1])
    if t == "castd": return "(double)(%s)" % cs_e(e[1])
    if t == "i2f": return "(float)(%s)" % cs_e(e[1])
    if t == "f2i": return "(int)(%s)" % cs_e(e[1])
    if t == "sqrtf": return "(float)Math.Sqrt(%s)" % cs_e(e[1])
    if t == "cond": return "(%s ? %s : %s)" % (cs_c(e[1]), cs_e(e[2]), cs_e(e[3]))
    if t == "iadd": return "(%s + %s)" % (cs_e(e[1]), cs_e(e[2]))
    if t == "imul": return "(%s * %s)" % (cs_e(e[1]), cs_e(e[2]))
    if t == "iand": return "(%s & %s)" % (cs_e(e[1]), cs_e(e[2]))
    if t == "b2i": return "(%s ? 1 : 0)" % cs_c(e[1])
    raise Exception("cs_e " + repr(e))


def cs_c(c):
    return "%s %s %s" % (cs_e(c[1]), c[0], cs_e(c[2]))


def cs_s(stmts, depth=[0]):
    out = []
    for s in stmts:
        t = s[0]
        if t == "set": out.append("%s = %s;" % (s[1], cs_e(s[2])))
        elif t == "aug": out.append("%s %s= %s;" % (s[1], s[2], cs_e(s[3])))
        elif t == "setp": out.append("%s = %s;" % (s[1], cs_e(s[2])))
        elif t == "augp": out.append("%s %s= %s;" % (s[1], s[2], cs_e(s[3])))
        elif t == "ssf": out.append("SF = %s;" % cs_e(s[1]))
        elif t == "asf": out.append("SF %s= %s;" % (s[1], cs_e(s[2])))
        elif t == "shf": out.append("H.F = %s;" % cs_e(s[1]))
        elif t == "sar": out.append("F[0] = %s;" % cs_e(s[1]))
        elif t == "if":
            out.append("if (%s) { %s }%s" % (cs_c(s[1]), cs_s(s[2]), (" else { %s }" % cs_s(s[3])) if s[3] else ""))
        elif t == "loop":
            v = "i%d" % depth[0]; depth[0] += 1
            out.append("for (int %s = 0; %s < %s; %s++) { %s }" % (v, v, cs_e(s[1]), v, cs_s(s[2])))
            depth[0] -= 1
        elif t == "do": out.append(cs_e(s[1]) + ";")
        elif t == "ref": out.append("Touch(ref %s);" % s[1])
        elif t == "try": out.append("try { %s } catch (Exception) { %s }" % (cs_s(s[1]), cs_s(s[2])))
        elif t == "raw": out.append(s[1])
        elif t == "ret": out.append("return %s;" % cs_e(s[1]))
        elif t == "reti": out.append("return %s;" % cs_e(s[1]))
        else: raise Exception("cs_s " + repr(s))
    return " ".join(out)


def locals_of(stmts, acc=None):
    """float locals in order of first assignment (= C# declaration order = IL local order)"""
    acc = [] if acc is None else acc
    for s in stmts:
        if s[0] in ("set", "aug") and s[1] not in acc: acc.append(s[1])
        if s[0] == "if": locals_of(s[2], acc); locals_of(s[3], acc)
        if s[0] in ("loop",): locals_of(s[2], acc)
        if s[0] == "try": locals_of(s[1], acc); locals_of(s[2], acc)
    return acc


def emit_cs(name, probe):
    body = probe["body"]
    kind = "int" if any(s[0] == "reti" for s in walk(body)) else "float"
    decl = []
    structs = []
    for l in (probe.get("decl") or locals_of(body)):
        if "." in l:
            st = l.split(".")[0]
            if st not in structs: structs.append(st); decl.append("V2 %s;" % st)
        else:
            decl.append("float %s;" % l)
    return "    // %s\n    [MethodImpl(MethodImplOptions.NoInlining)] public static %s Q_%s(float x, float y, float z, int n) { %s %s }" % (
        probe["doc"], kind, name, " ".join(decl), cs_s(body))


def walk(stmts):
    for s in stmts:
        yield s
        if s[0] == "if": yield from walk(s[2]); yield from walk(s[3])
        if s[0] == "loop": yield from walk(s[2])
        if s[0] == "try": yield from walk(s[1]); yield from walk(s[2])


# ------------------------------------------------------------------------------------------------ evaluation
def r32(a):
    with np.errstate(all="ignore"):
        return np.asarray(a, dtype=np.float64).astype(np.float32).astype(np.float64)


class Policy:
    """what gets rounded to float. Subclasses: rules.py"""
    name = "?"
    def prepare(self, probe): pass
    def store_local(self, name, v): return v            # value kept in the variable after `name = v`
    def at_call(self, env, mask): pass                   # may round variables at a call to a non-inlined method (mask = lanes executing it)
    def cast_float(self, v): return r32(v)               # (float)expr
    def loaded_temp_across_call(self, name, v): return v # a variable's value already loaded when a later call in the same expression runs
    def inline_arg(self, helper, i, uses, v): return v   # argument of an inlined helper
    def ret_noinline(self, v): return v                  # float returned by a non-inlined callee, as the caller sees it


class Interp:
    def __init__(self, x, y, z, n, policy):
        self.inp = {"x": x.astype(np.float64), "y": y.astype(np.float64), "z": z.astype(np.float64)}
        self.n = n.astype(np.int64); self.N = len(x); self.pol = policy

    def run(self, probe):
        self.pol.prepare(probe)
        self.env = dict(self.inp)             # parameters are variables too
        self.SF = np.zeros(self.N); self.HF = np.zeros(self.N); self.AR = np.zeros(self.N)
        self.ret = np.zeros(self.N); self.done = np.zeros(self.N, dtype=bool); self.isint = False; self.isdbl = False
        with np.errstate(all="ignore"):
            self.block(probe["body"], np.ones(self.N, dtype=bool))
        if getattr(self, "isdbl", False):
            return self.ret.view("<u8")
        if self.isint:
            return self.ret.astype(np.int64).astype(np.int32).astype(np.int64).view("<u8")
        return r32(self.ret).astype(np.float32).view("<u4").astype("<u8")

    # expressions return float64 arrays (or int64 for int expressions)
    def e(self, e, pending=()):
        t = e[0]
        if t == "p" or t == "l": return self.env[e[1]]
        if t == "n": return self.n
        if t == "k": return np.full(self.N, float(np.float32(e[1])))
        if t == "ki": return np.full(self.N, e[1], dtype=np.int64)
        if t in "+-*/":
            a = self.e(e[1])
            if has_call(e[2]) and e[1][0] in ("l", "p"):
                a = self.pol.loaded_temp_across_call(e[1][1], a)
            b = self.e(e[2])
            return a + b if t == "+" else a - b if t == "-" else a * b if t == "*" else a / b
        if t == "neg": return -self.e(e[1])
        if t == "abs": return np.abs(self.e(e[1]))
        if t == "sf": return self.SF
        if t == "hf": return self.HF
        if t == "ar": return self.AR
        if t == "call":
            inl, npar, body = HELPERS[e[1]]
            args = [self.e(a) for a in e[2]]
            if inl == "no":
                args = [r32(a) for a in args]                       # float arguments are stored to the outgoing stack slot
                self.pol.at_call(self.env, self.m)
                return self.pol.ret_noinline(self.h(body, args))
            uses = [count_arg(body, i) for i in range(npar)]
            args = [a if e[2][i][0] in ("p", "l", "k") else self.pol.inline_arg(e[1], i, uses[i], a) for i, a in enumerate(args)]
            return self.h(body, args)
        if t == "castf": return self.pol.cast_float(self.e(e[1]))
        if t == "castd": return self.e(e[1])
        if t == "i2f": return self.e(e[1]).astype(np.float64)
        if t == "f2i": return np.trunc(self.e(e[1])).astype(np.int64)
        if t == "sqrtf": return r32(np.sqrt(self.e(e[1])))
        if t == "cond": return np.where(self.c(e[1]), self.e(e[2]), self.e(e[3]))
        if t == "iadd": return self.e(e[1]) + self.e(e[2])
        if t == "imul": return self.e(e[1]) * self.e(e[2])
        if t == "iand": return self.e(e[1]) & self.e(e[2])
        if t == "b2i": return self.c(e[1]).astype(np.int64)
        raise Exception("eval " + repr(e))

    def h(self, body, args):
        t = body[0]
        if t == "a": return args[body[1]]
        if t == "k": return np.full(self.N, float(np.float32(body[1])))
        a, b = self.h(body[1], args), self.h(body[2], args)
        if t == "max": return np.where(a > b, a, b)
        if t == "min": return np.where(a < b, a, b)
        return a + b if t == "+" else a - b if t == "-" else a * b if t == "*" else a / b

    def c(self, c):
        a, b = self.e(c[1]), self.e(c[2])
        return {"<": a < b, ">": a > b, "==": a == b, "<=": a <= b, ">=": a >= b, "!=": a != b}[c[0]]

    def assign(self, name, v, m):
        self.env[name] = np.where(m, self.pol.store_local(name, v), self.env.get(name, np.zeros(self.N)))

    def block(self, stmts, m):
        for s in stmts:
            t = s[0]; m = m & ~self.done; self.m = m
            if t in ("set", "setp"): self.assign(s[1], self.e(s[2]), m)
            elif t in ("aug", "augp"):
                a = self.env[s[1]]
                if has_call(s[3]): a = self.pol.loaded_temp_across_call(s[1], a)
                b = self.e(s[3]); op = s[2]
                self.assign(s[1], a + b if op == "+" else a - b if op == "-" else a * b if op == "*" else a / b, m)
            elif t == "ssf": self.SF = np.where(m, r32(self.e(s[1])), self.SF)
            elif t == "asf":
                b = self.e(s[2]); op = s[1]; a = self.SF
                self.SF = np.where(m, r32(a + b if op == "+" else a - b if op == "-" else a * b), self.SF)
            elif t == "shf": self.HF = np.where(m, r32(self.e(s[1])), self.HF)
            elif t == "sar": self.AR = np.where(m, r32(self.e(s[1])), self.AR)
            elif t == "if":
                c = self.c(s[1]); self.block(s[2], m & c); self.block(s[3], m & ~c)
            elif t == "loop":
                cnt = self.e(s[1])
                for i in range(int(cnt.max()) if len(cnt) else 0):
                    self.block(s[2], m & (i < cnt))
            elif t == "do": self.e(s[1])
            elif t == "ref": self.pol.at_call(self.env, m)       # Touch(ref a): a itself lives in memory (the policies round it at its stores)
            elif t == "try": self.block(s[1], m)                # the probes never throw
            elif t == "raw": pass
            elif t == "ret":
                self.ret = np.where(m, self.e(s[1]), self.ret); self.done = self.done | m
            elif t == "retd":
                self.isdbl = True
                self.ret = np.where(m, self.e(s[1]), self.ret); self.done = self.done | m
            elif t == "reti":
                self.isint = True
                v = self.e(s[1]); v = v.astype(np.int64) if v.dtype != np.int64 else v
                self.ret = np.where(m, v, self.ret); self.done = self.done | m
            else: raise Exception("exec " + repr(s))


def has_call(e):
    if not isinstance(e, tuple): return False
    if e[0] == "call" and HELPERS[e[1]][0] == "no": return True
    return any(has_call(a) for a in e[1:] if isinstance(a, tuple)) or (e[0] == "call" and any(has_call(a) for a in e[2]))


def count_arg(body, i):
    if body[0] == "a": return 1 if body[1] == i else 0
    if body[0] == "k": return 0
    return count_arg(body[1], i) + count_arg(body[2], i)


# ------------------------------------------------------------------------------------------------ the JIT's view: events, liveness, enregistration
class Node:
    __slots__ = ("kind", "var", "succ", "w", "live_out", "depth")
    def __init__(self, kind, var, w):
        self.kind = kind; self.var = var; self.succ = []; self.w = w; self.live_out = set(); self.depth = 0


class Flow:
    """linear events with successors: ('use', v) ('def', v) ('call',) ('nop',)"""
    def __init__(self, probe):
        self.nodes = []; self.exit = Node("exit", None, 0)
        self.addr = set(); self.eh_vars = set(); self.in_try = 0; self.handlers = []
        first = self.seq(probe["body"], 1, None)
        self.entry = first[0] if first else self.exit
        self.link(first[1] if first else [], self.exit)

    def new(self, kind, var, w, cur):
        n = Node(kind, var, w); self.nodes.append(n)
        self.link(cur, n)
        return [n]

    def link(self, tails, n):
        for t in tails or []:
            t.succ.append(n)

    # every builder takes the list of dangling predecessors and returns the new list
    def ex(self, e, w, cur):
        t = e[0]
        if t in ("p", "l"): return self.new("use", e[1], w, cur)
        if t in ("k", "ki", "n", "sf", "hf", "ar"): return cur
        if t == "b2i": return self.cond(e[1], w, cur)
        if t == "call":
            inl, npar, body = HELPERS[e[1]]
            if inl == "no":
                for a in e[2]: cur = self.ex(a, w, cur)
                return self.new("call", None, w, cur)
            # inlined: an argument that is a variable is used once per use in the body; an expression argument is evaluated once
            uses = [count_arg(body, i) for i in range(npar)]
            for i, a in enumerate(e[2]):
                if a[0] in ("p", "l"):
                    for _ in range(uses[i]): cur = self.new("use", a[1], w, cur)
                else:
                    cur = self.ex(a, w, cur)
            return cur
        if t == "cond":
            cur = self.cond(e[1], w, cur)
            a = self.ex(e[2], w, cur); b = self.ex(e[3], w, cur)
            return a + b
        for a in e[1:]:
            if isinstance(a, tuple): cur = self.ex(a, w, cur)
        return cur

    def cond(self, c, w, cur):
        cur = self.ex(c[1], w, cur); cur = self.ex(c[2], w, cur)
        return self.new("nop", None, w, cur)

    def seq(self, stmts, w, cur):
        """returns (entry node, tails)"""
        start = len(self.nodes)
        marker = self.new("nop", None, w, cur)
        cur = marker
        for s in stmts:
            t = s[0]
            if t in ("set", "setp"):
                cur = self.ex(s[2], w, cur); cur = self.new("def", s[1], w, cur)
                if self.in_try: self.eh_vars.add(s[1])
            elif t in ("aug", "augp"):
                cur = self.new("use", s[1], w, cur); cur = self.ex(s[3], w, cur); cur = self.new("def", s[1], w, cur)
                if self.in_try: self.eh_vars.add(s[1])
            elif t in ("ssf", "shf", "sar"): cur = self.ex(s[1], w, cur)
            elif t == "asf": cur = self.ex(s[2], w, cur)
            elif t == "do": cur = self.ex(s[1], w, cur)
            elif t == "ref":
                self.addr.add(s[1]); cur = self.new("use", s[1], w, cur); cur = self.new("call", None, w, cur); cur = self.new("def", s[1], w, cur)
            elif t == "if":
                cur = self.cond(s[1], w, cur)
                a = self.seq(s[2], w, cur); b = self.seq(s[3], w, cur)
                cur = a[1] + b[1]
            elif t == "loop":
                # for (i = 0; i < cnt; i++) body   ->  head (test) ; body ; back edge.  Loop blocks weigh 8 times their parent.
                head = self.new("nop", None, w * 8, cur)
                body = self.seq(s[2], w * 8, head)
                self.link(body[1], head[0])
                cur = head
            elif t == "try":
                self.in_try += 1
                a = self.seq(s[1], w, cur)
                self.in_try -= 1
                h = self.seq(s[2], 0, cur + a[1])        # the handler can be entered from anywhere in the body; handler blocks run rarely
                self.handlers.append(h[0])
                cur = a[1] + h[1]
            elif t == "raw": pass
            elif t in ("ret", "reti", "retd"):
                cur = self.ex(s[1], w, cur); self.link(cur, self.exit); cur = []
        return (marker[0], cur)

    def liveness(self):
        changed = True
        while changed:
            changed = False
            for n in reversed(self.nodes):
                out = set()
                for s in n.succ:
                    if s.kind == "exit": continue
                    li = set(s.live_out)
                    if s.kind == "def": li.discard(s.var)
                    if s.kind == "use": li.add(s.var)
                    out |= li
                if out != n.live_out:
                    n.live_out = out; changed = True


TRACKED_LIMIT = 64   # the JIT tracks (does liveness and register allocation for) at most 64 locals, the most referenced ones
FP_REGS = 6      # FPV2..FPV7: two of the eight x87 registers are kept for expression temporaries


def jit_enreg(probe, tracked_limit=None):
    """returns dict: enregistered set, per-variable diagnostics"""
    f = Flow(probe); f.liveness()
    params = ["x", "y", "z"]
    order = params + (probe.get("decl") or locals_of(probe["body"]))                   # local numbers: parameters first, then IL locals in declaration order
    ref = {v: 0 for v in order}; wtd = {v: 0 for v in order}; pay = {v: 0 for v in order}
    intf = {v: set() for v in order}
    for n in f.nodes:
        if n.kind in ("use", "def"):
            ref[n.var] += 1; wtd[n.var] += n.w
        if n.kind == "call":
            for v in n.live_out: pay[v] += 2 * n.w
        live = set(n.live_out)
        if n.kind == "def": live.add(n.var)
        for a in live:
            intf[a] |= live - {a}
    live_in_handler = set()
    for h in f.handlers: live_in_handler |= h.live_out            # the entry marker of a handler is a no-op: live-out = live-in
    hotter = probe.get("hotter_locals", 0)                         # non-float locals with a higher weighted reference count (not in the probe language)
    tracked = sorted([v for v in order if ref[v] > 0], key=lambda v: (-wtd[v], order.index(v)))[:max(0, TRACKED_LIMIT - hotter)]
    cand = []
    for v in order:
        if ref[v] == 0 or v not in tracked: continue
        if v in f.addr: continue                                # address taken: lives in memory
        if v in live_in_handler: continue                       # live on entry to an exception handler: kept in memory
        if v in params and ref[v] <= 1: continue                # a parameter used once is loaded lazily from its stack slot
        if wtd[v] - pay[v] - 1 < 0: continue
        cand.append(v)
    weight = {v: wtd[v] - pay[v] for v in cand}
    arr = sorted(cand, key=lambda v: (-wtd[v], order.index(v)))  # the tracked-variable order: weighted reference count, then local number
    regs = [set() for _ in range(FP_REGS)]
    enreg = []
    for i in range(len(arr)):
        best = i
        for j in range(i + 1, len(arr)):                         # the JIT's selection loop: the LAST candidate that beats arr[i]
            d = weight[arr[j]] - weight[arr[i]]
            if d > 0 or (d == 0 and ref[arr[j]] != ref[arr[i]]): best = j
        arr[i], arr[best] = arr[best], arr[i]
        v = arr[i]
        for r in regs:
            if v not in r:
                r |= intf[v]; enreg.append(v); break
    return {"enreg": set(enreg), "ref": ref, "wtd": wtd, "pay": pay, "order": order, "addr": f.addr, "intf": intf}

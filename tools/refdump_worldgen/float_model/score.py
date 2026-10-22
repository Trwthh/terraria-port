#!/usr/bin/env python3
"""Scores three float rules on every probe of both probe sets (bit-exact, per probe):

  A  "x87"         the translator's current mode: expression trees unrounded, EVERY store to a float variable rounds, arguments / returns /
                   (float) casts round;
  B  "x87-locals"  as A, but a float local or parameter whose address is never taken is never rounded;
  C  "jit32"       the rule of port/crates/terraria-x87/FLOAT_RULE.md: a float variable is rounded at a store only when the legacy JIT keeps it
                   in memory (dsl.jit_enreg), an enregistered variable is rounded when a non-inlined call runs while it holds a value, ...

usage: python score.py [refdata/worldgen dir] [--oracle] [--why NAME]
  --oracle   for every probe C misses, search all subsets of "variables kept in a register" for the ones that reproduce the data
  --why NAME print the JIT model's bookkeeping for one probe
"""
import json, os, struct, sys
import numpy as np
from dsl import Interp, Policy, Flow, jit_enreg, r32, locals_of
from probes import NEW, OLD

args = [a for a in sys.argv[1:] if not a.startswith("--")]
D = args[0] if args else os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", "refdata", "worldgen")


def load(sub, fn, jn):
    meta = json.load(open(os.path.join(D, sub, jn)))
    raw = open(os.path.join(D, sub, fn), "rb").read()
    _, _, nprobes, n, _ = struct.unpack_from("<4sIIII", raw, 0)
    inp = np.frombuffer(raw, dtype=np.dtype([("x", "<f4"), ("y", "<f4"), ("z", "<f4"), ("n", "<i4")]), count=n, offset=20)
    off = 20 + n * 16; res = {}
    for p in meta["probes"]:
        res[p["name"]] = np.frombuffer(raw, dtype="<u8", count=n, offset=off); off += n * 8
    return inp, res, {p["name"]: p for p in meta["probes"]}


class RuleA(Policy):
    name = "A x87"
    def store_local(self, name, v): return r32(v)
    def inline_arg(self, helper, i, uses, v): return r32(v)
    def ret_noinline(self, v): return r32(v)


class RuleB(Policy):
    name = "B x87-locals"
    def prepare(self, probe): self.addr = Flow(probe).addr
    def store_local(self, name, v): return r32(v) if name in self.addr else v
    def inline_arg(self, helper, i, uses, v): return r32(v)
    def ret_noinline(self, v): return r32(v)


# JIT limits established by the B probes (FLOAT_RULE.md section 5)
MINOPTS_IL_BYTES = 60000
MINOPTS_INSTRS = 20000


class RuleC(Policy):
    name = "C jit32"
    def __init__(self, force=None): self.force = force
    def prepare(self, probe):
        self.info = jit_enreg(probe)
        self.enreg = set(self.info["enreg"]) if self.force is None else set(self.force)
        if self.force is None and probe.get("minopts"): self.enreg = set()
        if self.force is None and probe.get("untracked"): self.enreg -= set(probe["untracked"])
    def store_local(self, name, v): return v if name in self.enreg else r32(v)
    def at_call(self, env, mask):
        for v in self.enreg:
            if v in env: env[v] = np.where(mask, r32(env[v]), env[v])
    def loaded_temp_across_call(self, name, v): return r32(v) if name in self.enreg else v


def fill_flags(name, probe, meta):
    """facts about the compiled method that the rule needs and the probe language does not carry"""
    m = meta.get("Q_" + name)
    if m and m.get("il_bytes", 0) > MINOPTS_IL_BYTES: probe["minopts"] = True
    if m and probe.get("instrs", 0) > MINOPTS_INSTRS: probe["minopts"] = True


def main():
    inp1, res1, meta1 = load("float_probes", "probes.bin", "probes.json")
    try:
        inp2, res2, meta2 = load("float_probes2", "probes2.bin", "probes2.json")
    except FileNotFoundError:
        inp2, res2, meta2 = None, {}, {}
    rules = [RuleA(), RuleB(), RuleC()]
    tot = [0, 0, 0]; nprobes = 0
    print("%-36s %-6s %-14s %-14s %-14s  %s" % ("probe", "inputs", rules[0].name, rules[1].name, rules[2].name, "enregistered by C"))
    misses = []
    for label, table, inp, res, prefix, meta in (("first set", OLD, inp1, res1, "", meta1), ("second set", NEW, inp2, res2, "Q_", meta2)):
        if inp is None:
            print("(%s: data missing)" % label); continue
        for name in sorted(table):
            probe = table[name]
            if prefix + name not in res: continue
            fill_flags(name, probe, meta)
            want = res[prefix + name]; row = []
            for k, rule in enumerate(rules):
                got = Interp(inp["x"], inp["y"], inp["z"], inp["n"], rule).run(probe)
                ok = int((got == want).sum()); row.append(ok); tot[k] += ok == len(want)
            nprobes += 1
            en = rules[2].enreg
            print("%-36s %-6d %-14s %-14s %-14s  %s" % (name, len(want), *("%d%s" % (r, "" if r == len(want) else " *") for r in row), " ".join(sorted(en)) + (" [MinOpts]" if probe.get("minopts") else "")))
            if row[2] != len(want): misses.append((name, probe, inp, want))
    print("\nprobes reproduced exactly: A %d / %d, B %d / %d, C %d / %d" % (tot[0], nprobes, tot[1], nprobes, tot[2], nprobes))
    if "--oracle" in sys.argv:
        for name, probe, inp, want in misses:
            oracle(name, probe, inp, want)
    if "--why" in sys.argv:
        nm = sys.argv[sys.argv.index("--why") + 1]
        probe = NEW.get(nm) or OLD[nm]
        info = jit_enreg(probe)
        for v in info["order"]:
            print("  %-6s refs %2d weighted %4d call payload %4d interferes with %s" % (v, info["ref"][v], info["wtd"][v], info["pay"][v], " ".join(sorted(info["intf"][v]))))
        print("  enregistered:", sorted(info["enreg"]))


def oracle(name, probe, inp, want):
    vs = ["x", "y", "z"] + (probe.get("decl") or locals_of(probe["body"]))
    vs = [v for v in vs if any(True for _ in [0])]
    if len(vs) > 14:
        print("oracle %s: too many variables" % name); return
    found = []
    for mask in range(1 << len(vs)):
        force = [v for i, v in enumerate(vs) if mask >> i & 1]
        got = Interp(inp["x"], inp["y"], inp["z"], inp["n"], RuleC(force)).run(probe)
        if (got == want).all(): found.append(force)
    # parameters that are never assigned do not matter: reduce to the distinct sets of locals
    red = sorted({tuple(sorted(v for v in f if v not in ("x", "y", "z") or assigned(probe, v))) for f in found})
    print("oracle %s: %d consistent register sets%s" % (name, len(red), ": " + "; ".join("{" + " ".join(r) + "}" for r in red[:6]) if red else " (the rule's semantics cannot produce the data with any set)"))


def assigned(probe, v):
    from dsl import walk
    return any(s[0] in ("setp", "augp") and s[1] == v for s in walk(probe["body"]))


if __name__ == "__main__":
    main()

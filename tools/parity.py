#!/usr/bin/env python3
"""Parity check for db/terraria.sqlite. Read-only on the database, the decompile and the game folder.

Parity means, concretely:
  A  files     every file under decompile/ is in the database once, byte-identical (blob -> sha256 -> disk),
               every .cs file parses as complete C#, and gamedb indexed every source file at its full size
  B  types     every type in the binary is in the database; every type the programmer wrote has a source span;
               every source type declaration maps back to the binary; an independent tool (ILSpy) lists the same types
  C  methods   every method with IL has a row with its IL hash; every hand-written one maps to a source span that
               slices identically from the stored blob and from disk; every source method maps back to the binary
  D  fields    same, plus every enum constant in the source equals the value in the binary
  E  strings   string literals loaded by the IL appear in the decompiled file of the same method
  F  graph     every call / field reference row points at rows that exist
  G  gamedb    what gamedb's own parser found vs the exact declaration list (reported as gaps, not failures)
  H  resources every embedded resource is on disk with the same hash
  I  rerun     the database fingerprint equals the previous run's when the inputs are the same

A FAILED check means the database cannot be trusted for that item. A GAP is a known limit that is listed
with exact paths and symbols so the port can plan around it. Exit code: 1 if any check failed.
"""
import hashlib, json, re, sqlite3, subprocess, sys, time, zlib
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DB = ROOT / "db" / "terraria.sqlite"
SRC = ROOT / "decompile"
META = ROOT / "meta"
GAMEDB = ROOT.parent / "VAEL - GAMELAB" / "tools" / "gamedb" / "target" / "release" / "gamedb.exe"
REPORT = ROOT / "logs" / "parity-report.md"
FINGERPRINT = ROOT / "logs" / "parity-fingerprint.json"
READ_SAMPLE = 400

checks, gaps = [], []


def check(cid, title, ok, detail="", items=()):
    checks.append(dict(id=cid, title=title, ok=bool(ok), detail=detail, items=list(items)))


def gap(cid, title, detail, items=(), blocks=False):
    gaps.append(dict(id=cid, title=title, detail=detail, items=list(items), blocks=blocks))


def sha(b):
    return hashlib.sha256(b).hexdigest()


STR = re.compile(r'@"((?:[^"]|"")*)"|"((?:[^"\\\n]|\\.)*)"')
ESC = re.compile(r'\\(u[0-9a-fA-F]{4}|U[0-9a-fA-F]{8}|x[0-9a-fA-F]{1,4}|.)', re.S)
SIMPLE = {"n": "\n", "r": "\r", "t": "\t", "0": "\0", "a": "\a", "b": "\b", "f": "\f", "v": "\v", "\\": "\\", '"': '"', "'": "'"}


def literals(text):
    """Every C# string literal in a file, unescaped."""
    out = set()
    for m in STR.finditer(text):
        if m.group(1) is not None:
            out.add(m.group(1).replace('""', '"'))
        else:
            out.add(ESC.sub(lambda e: SIMPLE.get(e.group(1)) or (chr(int(e.group(1)[1:], 16)) if e.group(1)[0] in "uUx" else e.group(1)), m.group(2)))
    return out


def main():
    t0 = time.time()
    con = sqlite3.connect(f"file:{DB.as_posix()}?mode=ro", uri=True)
    q = lambda s, *a: con.execute(s, a).fetchall()
    one = lambda s, *a: con.execute(s, a).fetchone()[0]
    totals = {}

    # ------------------------------------------------------------------ A files
    disk = {p.relative_to(SRC).as_posix(): p for p in SRC.rglob("*")
            if p.is_file() and not p.relative_to(SRC).parts[0].startswith(".")}
    dbf = {r[0]: r for r in q("SELECT path, id, size, lines, sha256, kind, gamedb_file_id FROM port_files")}
    check("A1", "every file under decompile/ has exactly one port_files row and nothing else does",
          set(disk) == set(dbf), f"disk {len(disk)}, database {len(dbf)}",
          [f"only on disk: {p}" for p in sorted(set(disk) - set(dbf))] + [f"only in database: {p}" for p in sorted(set(dbf) - set(disk))])
    lower = Counter(p.lower() for p in dbf)
    check("A2", "no two paths differ only by letter case (would overwrite each other on Windows)",
          all(v == 1 for v in lower.values()), "", [p for p, v in lower.items() if v > 1])
    bad, texts, raws = [], {}, {}
    for path, fid, size, lines, h, kind, gid in dbf.values():
        blob = zlib.decompress(one("SELECT content FROM port_files WHERE id = ?", fid))
        raw = disk[path].read_bytes() if path in disk else None
        if raw is None or sha(blob) != h or blob != raw or len(blob) != size:
            bad.append(path)
        if kind == "source":
            raws[fid] = blob
            texts[fid] = blob.decode("utf-8-sig", errors="replace")
    check("A3", "stored content decompresses to bytes identical to the file on disk (sha256 and size)", not bad,
          f"{len(dbf) - len(bad)} of {len(dbf)} identical", bad)
    totals["files"] = (len(dbf), len(disk))

    gfiles = {r[0]: r for r in q("SELECT path, id, size FROM files")}
    src_paths = {p for p, r in dbf.items() if r[5] == "source"}
    check("A4", "gamedb indexed every .cs file and no other path", set(gfiles) == src_paths,
          f"gamedb {len(gfiles)}, source files {len(src_paths)}",
          [f"missing from gamedb: {p}" for p in sorted(src_paths - set(gfiles))] + [f"gamedb only: {p}" for p in sorted(set(gfiles) - src_paths)])
    trunc = [p for p in src_paths & set(gfiles) if gfiles[p][2] != dbf[p][2]]
    check("A5", "gamedb recorded the full size of every source file (no truncated read)", not trunc, "", trunc)
    errs = [(r[0], r[2]) for r in map(json.loads, open(META / "src" / "src_files.jsonl", encoding="utf-8")) if r[1]]
    check("A6", "every .cs file parses as complete, valid C# (no truncated or malformed file)", not errs,
          f"{len(src_paths) - len(errs)} of {len(src_paths)} parse with 0 errors", [f"{p}: {e}" for p, e in errs])
    marker = re.compile(r"Error decompiling|Unknown result type|Expected O, but got|Could not decode|/\*Error|//IL_[0-9a-f]{4}: ")
    marked = sorted(dbf_path for dbf_path, r in dbf.items() if r[5] == "source" and marker.search(texts[r[1]]))
    check("A7", "no decompiler failure markers in the source text", not marked, "", marked)
    empty = sorted(p for p, r in dbf.items() if r[2] == 0)
    check("A8", "no empty files", not empty, "", empty)
    not_indexed = sorted(p for p, r in dbf.items() if r[5] != "source")
    by_ext = Counter(p.rsplit(".", 1)[-1] for p in not_indexed)
    gap("G-A1", "files gamedb does not read (not a source extension)",
        f"{len(not_indexed)} files ({', '.join(f'{v} .{k}' for k, v in sorted(by_ext.items()))}). "
        "They are stored in full in port_files (kind = resource / project); gamedb search/strings do not see them.", not_indexed)

    # ------------------------------------------------------------------ B types
    for asm in ("Terraria", "ReLogic"):
        info = json.loads((META / asm / "assembly.json").read_text())
        n = one("SELECT COUNT(*) FROM port_types WHERE assembly = ?", asm)
        check(f"B1-{asm}", f"{asm}: every type in the binary has a row", n == info["types"], f"{n} of {info['types']}")
        ilspy = META / f"{asm}.types.txt"
        if ilspy.exists():
            want = Counter()
            for line in ilspy.read_text(encoding="utf-8", errors="replace").splitlines():
                parts = line.strip().split(" ", 1)
                if len(parts) == 2 and parts[0] in ("Class", "Interface", "Struct", "Delegate", "Enum"):
                    want[(parts[0].lower(), re.sub(r"`\d+", "", parts[1]))] += 1
            have = Counter((k, re.sub(r"`\d+", "", f).replace("+", ".")) for k, f in q("SELECT kind, full_name FROM port_types WHERE assembly = ?", asm))
            diff = [f"ILSpy only: {k} {n_}" for (k, n_) in (want - have)] + [f"database only: {k} {n_}" for (k, n_) in (have - want)]
            check(f"B2-{asm}", f"{asm}: type list equals ILSpy's own listing (independent reader)", not diff,
                  f"{sum(have.values())} types, {sum(want.values())} listed by ILSpy", diff)
        miss = q("SELECT full_name, kind FROM port_types WHERE assembly = ? AND compiler_generated = 0 AND file_id IS NULL AND name != '<Module>'", asm)
        check(f"B3-{asm}", f"{asm}: every hand-written type has a source file and line span", not miss,
              f"{one('SELECT COUNT(*) FROM port_types WHERE assembly = ? AND compiler_generated = 0', asm)} hand-written types", [f"{k} {n_}" for n_, k in miss])
        dup = q("SELECT full_name, COUNT(*) FROM port_types WHERE assembly = ? GROUP BY 1 HAVING COUNT(*) > 1", asm)
        check(f"B4-{asm}", f"{asm}: no duplicate type names", not dup, "", [f"{n_} x{c}" for n_, c in dup])
        kinds = q("""SELECT t.kind, d.kind, COUNT(*) FROM port_types t JOIN port_src_decls d ON d.assembly = t.assembly AND d.token = t.token
                     AND d.kind IN ('class','struct','interface','enum','delegate') WHERE t.assembly = ? AND t.kind != d.kind GROUP BY 1, 2""", asm)
        check(f"B5-{asm}", f"{asm}: kind in the source (class/struct/enum/interface/delegate) equals kind in the binary", not kinds, "",
              [f"binary {a} vs source {b}: {c}" for a, b, c in kinds])
    orphan = q("""SELECT f.path, d.kind, d.container, d.name FROM port_src_decls d JOIN port_files f ON f.id = d.file_id
                  WHERE d.kind IN ('class','struct','interface','enum','delegate') AND d.token IS NULL""")
    check("B6", "every type declared in the source exists in the binary", not orphan, "", [f"{p}: {k} {c}.{n_}" for p, k, c, n_ in orphan])
    no_type = q("SELECT path FROM port_files f WHERE kind = 'source' AND NOT EXISTS (SELECT 1 FROM port_src_decls d WHERE d.file_id = f.id AND d.kind IN ('class','struct','interface','enum','delegate'))")
    gap("G-B1", "source files that declare no type", f"{len(no_type)} (assembly attribute files)", [p for p, in no_type])
    totals["types"] = (one("SELECT COUNT(*) FROM port_types"), sum(json.loads((META / a / 'assembly.json').read_text())["types"] for a in ("Terraria", "ReLogic")))

    # ------------------------------------------------------------------ C methods
    exp_m = sum(json.loads((META / a / "assembly.json").read_text())["methods"] for a in ("Terraria", "ReLogic"))
    n_m = one("SELECT COUNT(*) FROM port_methods")
    check("C1", "every method in the binary has a row", n_m == exp_m, f"{n_m} of {exp_m}")
    il_err = sum(json.loads((META / a / "assembly.json").read_text())["il_errors"] for a in ("Terraria", "ReLogic"))
    no_hash = q("SELECT assembly, type_name, name FROM port_methods WHERE rva != 0 AND (il_sha256 IS NULL OR il_size <= 0) AND impl_attrs & 3 = 0")
    check("C2", "every method body was read in full (IL walked to its last byte, hash stored)", il_err == 0 and not no_hash,
          f"{one('SELECT COUNT(*) FROM port_methods WHERE has_body = 1')} bodies, {one('SELECT SUM(il_size) FROM port_methods')} IL bytes, {il_err} read errors",
          [f"{a}: {t}::{n_}" for a, t, n_ in no_hash])
    unmatched = q("""SELECT assembly, type_name, name, params, src_match FROM port_methods
                     WHERE compiler_generated = 0 AND src_match NOT IN ('exact','by-count','implicit','runtime')""")
    check("C3", "every hand-written method maps to exactly one source declaration", not unmatched,
          f"{one('SELECT COUNT(*) FROM port_methods WHERE compiler_generated = 0')} hand-written methods",
          [f"{a}: {t}::{n_}({p}) [{m}]" for a, t, n_, p, m in unmatched])
    gen_none = q("SELECT assembly, type_name, name FROM port_methods WHERE compiler_generated = 1 AND src_match IN ('none','ambiguous')")
    check("C4", "every compiler-generated method (lambda, iterator, closure) is attributed to a source location or marked synthetic", not gen_none, "",
          [f"{a}: {t}::{n_}" for a, t, n_ in gen_none])
    src_orphan = q("""SELECT f.path, d.kind, d.container, d.name, d.params, d.start_line FROM port_src_decls d JOIN port_files f ON f.id = d.file_id
                      WHERE d.kind IN ('method','ctor','dtor','operator') AND d.token IS NULL""")
    check("C5", "every method, constructor and operator in the source exists in the binary", not src_orphan,
          f"{one('''SELECT COUNT(*) FROM port_src_decls WHERE kind IN ('method','ctor','dtor','operator')''')} source declarations",
          [f"{p}:{ln} {k} {c}::{n_}({ps})" for p, k, c, n_, ps, ln in src_orphan])
    acc_orphan = q("""SELECT f.path, d.container, d.name, d.start_line FROM port_src_decls d JOIN port_files f ON f.id = d.file_id
                      WHERE d.kind = 'accessor' AND d.token IS NULL""")
    check("C6", "every property / event accessor in the source exists in the binary", not acc_orphan, "",
          [f"{p}:{ln} {c}::{n_}" for p, c, n_, ln in acc_orphan])
    # span integrity: stored hash == hash of the same lines from the blob; spans inside the file; bodies not empty
    bad_span, empty_body, shared, n_slices = [], [], defaultdict(list), 0
    lines_of = {fid: raw.split(b"\n") for fid, raw in raws.items()}
    for asm, tname, name, fid, a, b, h, has_body in q("""SELECT assembly, type_name, name, file_id, start_line, end_line, src_sha256, has_body
                                                         FROM port_methods WHERE src_match IN ('exact','by-count')"""):
        ls = lines_of[fid]
        if not (1 <= a <= b <= len(ls)) or sha(b"\n".join(ls[a - 1:b])) != h:
            bad_span.append(f"{asm}: {tname}::{name} lines {a}-{b}")
            continue
        body = b"\n".join(ls[a - 1:b])
        if has_body and b"{" not in body and b"=>" not in body and b";" not in body:
            empty_body.append(f"{asm}: {tname}::{name} lines {a}-{b}")
        if not name.startswith(("get_", "set_", "add_", "remove_")) or a != b:   # `{ get; set; }` on one line is two accessors
            shared[(fid, a, b)].append(f"{tname}::{name}")
        n_slices += 1
    check("C7", "every mapped method's source slice round-trips (lines from the stored blob hash to the stored value)", not bad_span,
          f"{n_slices} slices verified", bad_span)
    check("C8", "no mapped method with IL has an empty source body", not empty_body, "", empty_body)
    dup_span = [f"{k}: {', '.join(v)}" for k, v in shared.items() if len(v) > 1]
    check("C9", "no two methods are mapped to the same source span", not dup_span, "", dup_span[:50])
    dup_sig = q("""SELECT assembly, type_name, name, params, ret, generic_arity, COUNT(*) FROM port_methods
                   GROUP BY 1,2,3,4,5,6 HAVING COUNT(*) > 1""")
    check("C10", "no duplicate method signatures within a type", not dup_sig, "", [f"{a}: {t}::{n_}({p}) -> {r} x{c}" for a, t, n_, p, r, g, c in dup_sig])
    totals["methods"] = (n_m, exp_m)
    by_match = dict(q("SELECT src_match, COUNT(*) FROM port_methods GROUP BY 1"))
    gap("G-C1", "methods with no source text of their own",
        f"{by_match.get('implicit', 0)} implicit (default constructors and field initialisers: the code is the field declarations of the type); "
        f"{by_match.get('runtime', 0)} runtime-provided delegate methods (no IL); "
        f"{by_match.get('origin', 0) + by_match.get('origin-overloaded', 0) + by_match.get('origin-type', 0)} compiler-generated "
        f"(lambdas, iterators, closures: their code is inside the method named in origin_token, or inside the type for field-initialiser lambdas; "
        f"{by_match.get('origin-overloaded', 0)} of them sit in an overloaded method so only the type is recorded); "
        f"{by_match.get('synthetic', 0)} synthetic (anonymous types, <PrivateImplementationDetails>). Their IL hash and call rows are complete.")
    bc = q("SELECT assembly, type_name, name, params FROM port_methods WHERE src_match = 'by-count'")
    if bc:
        gap("G-C2", "methods matched by name and parameter count where the parameter type text differs",
            f"{len(bc)}. Unique within their type, so the mapping is not in doubt; the type spelling differs between metadata and C#.",
            [f"{a}: {t}::{n_}({p})" for a, t, n_, p in bc])

    # ------------------------------------------------------------------ D fields
    exp_f = sum(json.loads((META / a / "assembly.json").read_text())["fields"] for a in ("Terraria", "ReLogic"))
    n_f = one("SELECT COUNT(*) FROM port_fields")
    check("D1", "every field in the binary has a row", n_f == exp_f, f"{n_f} of {exp_f}")
    f_un = q("SELECT assembly, type_name, name, src_match FROM port_fields WHERE compiler_generated = 0 AND src_match NOT IN ('exact','runtime')")
    check("D2", "every hand-written field and enum member maps to one source declaration", not f_un,
          f"{one('SELECT COUNT(*) FROM port_fields WHERE compiler_generated = 0')} hand-written fields", [f"{a}: {t}.{n_} [{m}]" for a, t, n_, m in f_un])
    f_orphan = q("""SELECT f.path, d.kind, d.container, d.name FROM port_src_decls d JOIN port_files f ON f.id = d.file_id
                    WHERE d.kind IN ('field','enum_member') AND d.token IS NULL""")
    check("D3", "every field and enum member in the source exists in the binary", not f_orphan, "", [f"{p}: {k} {c}.{n_}" for p, k, c, n_ in f_orphan])
    wrong, compared = [], 0
    for asm, tname, name, cst, extra in q("""SELECT pf.assembly, pf.type_name, pf.name, pf.constant, d.extra FROM port_fields pf
                                             JOIN port_src_decls d ON d.assembly = pf.assembly AND d.token = pf.token AND d.kind = 'enum_member'
                                             WHERE pf.constant IS NOT NULL AND d.extra IS NOT NULL"""):
        text = extra.replace("_", "").rstrip("uUlL")
        try:
            val = int(text, 16) if text.lower().startswith(("0x", "-0x")) else int(text)
        except ValueError:
            continue
        compared += 1
        if val != int(cst):
            wrong.append(f"{asm}: {tname}.{name} source {extra} vs binary {cst}")
    check("D4", "every enum constant written as a number in the source equals the value in the binary", not wrong, f"{compared} compared", wrong)
    prop_un = q("SELECT assembly, type_name, name, kind FROM port_props p WHERE file_id IS NULL AND NOT EXISTS (SELECT 1 FROM port_types t WHERE t.assembly = p.assembly AND t.token = p.type_token AND t.compiler_generated = 1)")
    check("D5", "every hand-written property and event maps to a source declaration", not prop_un,
          f"{one('SELECT COUNT(*) FROM port_props')} properties and events", [f"{a}: {k} {t}.{n_}" for a, t, n_, k in prop_un])
    totals["fields"] = (n_f, exp_f)

    # ------------------------------------------------------------------ E strings
    lit_cache, found, miss_rows = {}, 0, []
    rows_ = q("""SELECT s.assembly, m.type_name, m.name, m.file_id, s.text FROM port_strings s
                 JOIN port_methods m ON m.assembly = s.assembly AND m.token = s.method_token WHERE m.file_id IS NOT NULL""")
    for asm, tname, name, fid, text in rows_:
        if fid not in lit_cache:
            lit_cache[fid] = literals(texts[fid])
        if text in lit_cache[fid] or text == "" or text in texts[fid]:
            found += 1
        else:
            miss_rows.append(f"{asm}: {tname}::{name} {json.dumps(text[:80])}")
    total_s = one("SELECT COUNT(*) FROM port_strings")
    pct = 100.0 * found / max(len(rows_), 1)
    check("E1", "string literals loaded by the IL appear in the decompiled file of the same method (at least 99%)", pct >= 99.0,
          f"{found} of {len(rows_)} ({pct:.2f}%); {total_s - len(rows_)} more belong to synthetic methods with no file")
    if miss_rows:
        gap("G-E1", "IL string literals not found verbatim in the source file",
            f"{len(miss_rows)}. Expected cases: the decompiler turns a string switch or a concatenation into different text, or prints a character with a different escape. "
            "port_strings holds the exact value from the binary either way.", miss_rows[:40])
    totals["strings"] = (total_s, sum(json.loads((META / a / "assembly.json").read_text())["strings"] for a in ("Terraria", "ReLogic")))
    check("E2", "every string literal row in the binary is stored", totals["strings"][0] == totals["strings"][1], f"{totals['strings'][0]} of {totals['strings'][1]}")

    # ------------------------------------------------------------------ F graph
    exp_c = sum(json.loads((META / a / "assembly.json").read_text())["calls"] for a in ("Terraria", "ReLogic"))
    exp_r = sum(json.loads((META / a / "assembly.json").read_text())["fieldrefs"] for a in ("Terraria", "ReLogic"))
    n_c, n_r = one("SELECT COUNT(*) FROM port_calls"), one("SELECT COUNT(*) FROM port_field_refs")
    check("F1", "every call site and field access in the IL is stored", n_c == exp_c and n_r == exp_r, f"calls {n_c} of {exp_c}, field accesses {n_r} of {exp_r}")
    dangling = one("""SELECT COUNT(*) FROM port_calls c WHERE NOT EXISTS (SELECT 1 FROM port_methods m WHERE m.assembly = c.assembly AND m.token = c.src_token)""") \
        + one("""SELECT COUNT(*) FROM port_calls c WHERE c.dst_token IS NOT NULL AND NOT EXISTS (SELECT 1 FROM port_methods m WHERE m.assembly = c.dst_assembly AND m.token = c.dst_token)""") \
        + one("""SELECT COUNT(*) FROM port_field_refs r WHERE r.field_token IS NOT NULL AND NOT EXISTS (SELECT 1 FROM port_fields f WHERE f.assembly = r.dst_assembly AND f.token = r.field_token)""")
    check("F2", "every caller, resolved callee and resolved field in the graph exists", dangling == 0, f"{dangling} dangling rows")
    unres = q("""SELECT dst_assembly, dst_type, dst_name, dst_param_count, COUNT(*) FROM port_calls
                 WHERE dst_assembly IS NOT NULL AND dst_token IS NULL GROUP BY 1,2,3,4 ORDER BY 5 DESC""")
    unres_f = one("SELECT COUNT(*) FROM port_field_refs WHERE dst_assembly IS NOT NULL AND field_token IS NULL")
    internal = one("SELECT COUNT(*) FROM port_calls WHERE dst_assembly IS NOT NULL")
    if unres or unres_f:
        gap("G-F1", "calls into the two indexed assemblies whose exact overload could not be resolved",
            f"{sum(r[4] for r in unres)} of {internal} internal call sites ({len(unres)} distinct targets), {unres_f} field accesses. "
            "These go through a generic instantiation where several overloads share a name and parameter count; the target type and name are stored.",
            [f"{a}: {t}::{n_}/{pc} x{c}" for a, t, n_, pc, c in unres[:40]])
    ext = q("SELECT substr(dst_type, 1, instr(dst_type || '.', '.') - 1), COUNT(*) FROM port_calls WHERE dst_assembly IS NULL GROUP BY 1 ORDER BY 2 DESC LIMIT 12")
    gap("G-F2", "calls that leave the indexed code (framework and third-party libraries)",
        f"{n_c - internal} of {n_c} call sites. Top namespaces: " + ", ".join(f"{n_} {c}" for n_, c in ext) +
        ". These are the API surface the Rust port has to replace (XNA -> Bevy, System.* -> std, Steamworks, Newtonsoft...).")
    totals["calls"] = (n_c, exp_c)
    totals["field accesses"] = (n_r, exp_r)

    # ------------------------------------------------------------------ G gamedb's own parser vs the exact list
    g_fn = one("SELECT COUNT(*) FROM functions")
    bodies = one("SELECT COUNT(*) FROM port_methods WHERE src_match IN ('exact','by-count') AND has_body = 1")
    linked = one("SELECT COUNT(*) FROM port_methods WHERE gamedb_function_id IS NOT NULL")
    missed = q("""SELECT f.path, COUNT(*) FROM port_methods m JOIN port_files f ON f.id = m.file_id
                  WHERE m.src_match IN ('exact','by-count') AND m.has_body = 1 AND m.gamedb_function_id IS NULL GROUP BY 1 ORDER BY 2 DESC""")
    extra = one("SELECT COUNT(*) FROM functions f WHERE NOT EXISTS (SELECT 1 FROM port_methods m WHERE m.gamedb_function_id = f.id)")
    by_kind = q("""SELECT CASE WHEN name IN ('.ctor','.cctor') THEN 'constructor' WHEN name LIKE 'get\\_%' ESCAPE '\\' OR name LIKE 'set\\_%' ESCAPE '\\'
                   OR name LIKE 'add\\_%' ESCAPE '\\' OR name LIKE 'remove\\_%' ESCAPE '\\' THEN 'accessor' WHEN name LIKE 'op\\_%' ESCAPE '\\' THEN 'operator' ELSE 'method' END, COUNT(*)
                   FROM port_methods WHERE src_match IN ('exact','by-count') AND has_body = 1 AND gamedb_function_id IS NULL GROUP BY 1 ORDER BY 2 DESC""")
    gap("G-G1", "methods gamedb's heuristic parser did not record as functions",
        f"gamedb functions: {g_fn}. Hand-written methods with a body: {bodies}; linked to a gamedb function: {linked}; not found by gamedb: {bodies - linked} "
        f"({', '.join(f'{c} {k}s' for k, c in by_kind)}). {extra} gamedb functions have no single matching method (overloads sharing a name in one file, local functions, lambdas). "
        "Every one of the missed methods is in port_methods with its file and line span, so `gamedb search/read/graph` are incomplete but the database is not. "
        "Use port_methods + port_calls for anything that must be exhaustive.",
        [f"{p}: {c} missed" for p, c in missed[:25]])
    g_edges = one("SELECT COUNT(*) FROM edges")
    il_pairs = one("SELECT COUNT(*) FROM (SELECT DISTINCT assembly, src_token, dst_assembly, dst_token FROM port_calls WHERE dst_token IS NOT NULL)")
    gap("G-G2", "gamedb call edges are name-based",
        f"gamedb edges: {g_edges} (text matches of a name inside a body, including field and type names). Exact caller -> callee pairs from the IL: {il_pairs}. "
        "gamedb cannot tell overloads apart or see virtual / interface dispatch targets; port_calls records the opcode (call / callvirt / newobj / ldftn) and the exact overload.")
    # gamedb read round-trip on a deterministic sample of names that are unique inside their file
    sample = q("""SELECT g.name, f.path, g.start_line, g.end_line, f.id FROM functions g JOIN port_files f ON f.gamedb_file_id = g.file_id
                  WHERE (SELECT COUNT(*) FROM functions g2 WHERE g2.file_id = g.file_id AND g2.name = g.name) = 1
                    AND (SELECT COUNT(*) FROM functions g3 WHERE g3.name = g.name) = 1
                  ORDER BY g.name""")
    step = max(1, len(sample) // READ_SAMPLE)
    sample = sample[::step][:READ_SAMPLE]
    read_bad, crlf_only = [], 0
    for name, path, a, b, fid in sample:
        p = subprocess.run([str(GAMEDB), "read", "-r", str(SRC), "--db", str(DB), name, "--path", path], capture_output=True)
        want = b"\n".join(lines_of[fid][a - 1:b])
        got = p.stdout
        if got.rstrip(b"\r\n") == want.rstrip(b"\r\n"):
            continue
        if got.replace(b"\r\n", b"\n").rstrip(b"\r\n") == want.replace(b"\r\n", b"\n").rstrip(b"\r\n"):
            crlf_only += 1
        else:
            read_bad.append(f"{path}: {name} lines {a}-{b}")
    check("G3", f"`gamedb read` returns the exact function text for a sample of {len(sample)} functions (line endings aside)", not read_bad,
          f"{len(sample) - len(read_bad) - crlf_only} byte-identical, {crlf_only} identical apart from CRLF vs LF, {len(read_bad)} different", read_bad)
    if crlf_only:
        gap("G-G3", "`gamedb read` prints LF line endings", f"The files on disk use CRLF. {crlf_only} of {len(sample)} sampled reads differ only in that. "
            "Diff against port_files.content (exact bytes) when line endings matter.")
    unm = q("SELECT path FROM files WHERE module IS NULL OR module = 'unmapped'")
    check("G4", "every source file belongs to a module", not unm, f"{one('SELECT COUNT(DISTINCT module) FROM files')} modules", [p for p, in unm])
    totals["functions (gamedb)"] = (g_fn, bodies)

    # ------------------------------------------------------------------ H resources
    res_bad, res_n = [], 0
    for asm, name, size, h, fid, is_lib in q("SELECT assembly, name, size, sha256, file_id, embedded_library FROM port_resources"):
        res_n += 1
        if is_lib:
            p = META / "embedded" / name
            if not p.exists() or sha(p.read_bytes()) != h:
                res_bad.append(f"{asm}: {name} (embedded library)")
        elif fid is None or one("SELECT sha256 FROM port_files WHERE id = ?", fid) != h:
            res_bad.append(f"{asm}: {name}")
    check("H1", "every resource embedded in the binary is on disk with the same sha256", not res_bad, f"{res_n - len(res_bad)} of {res_n}", res_bad)
    libs = q("SELECT resource, size, decompiled_as FROM port_embedded ORDER BY 1")
    gap("G-H1", "embedded libraries that are not decompiled",
        f"{sum(1 for l in libs if not l[2])} of {len(libs)} libraries inside Terraria.exe are third-party and only recorded by name, size and hash "
        "(calls into them are in port_calls with dst_assembly NULL). ReLogic.dll is decompiled and indexed.",
        [f"{n_} ({s} bytes)" for n_, s, d in libs if not d])
    refs = json.loads(one("SELECT refs FROM port_assemblies WHERE name = 'Terraria'"))
    gap("G-H2", "code that is not in this database at all",
        "Microsoft XNA 4.0 (rendering, input, audio, content loading) and the .NET Framework are separate products the game calls into; they are not part of Terraria.exe. "
        "ReLogic.Native.dll, nfd.dll, CUESDK and the Logitech wrapper are native code. The `Content/` folder (textures, sounds, fonts, shaders as .xnb / .xwb) is game data, "
        "not source, and is not indexed: the port needs its own readers for those formats. References of Terraria.exe: " + ", ".join(refs) + ".", blocks=False)

    # ------------------------------------------------------------------ I re-run fingerprint
    fp = {}
    tables = [r[0] for r in q("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY 1")]
    for t in tables:
        if t == "port_runs":
            continue
        cols = [c[1] for c in q(f"PRAGMA table_info({t})")]
        # ids and mtimes are assigned per run by gamedb; everything else must be stable
        keep = [c for c in cols if not (t in ("files", "functions", "symbols", "strings", "edges") and c in ("id", "file_id", "sym_id", "src_id", "dst_id", "mtime", "edges_mtime"))
                and c not in ("gamedb_function_id", "gamedb_file_id")]
        h = hashlib.sha256()
        n = 0
        for row in con.execute(f"SELECT {', '.join(keep)} FROM {t} ORDER BY {', '.join(str(i + 1) for i in range(len(keep)))}"):
            h.update(repr(row).encode("utf-8", "surrogatepass"))
            n += 1
        fp[t] = dict(rows=n, sha256=h.hexdigest())
    inputs = {a: json.loads((META / a / "assembly.json").read_text())["sha256"] for a in ("Terraria", "ReLogic")}
    inputs["source_tree"] = sha("".join(f"{p}:{r[4]}\n" for p, r in sorted(dbf.items())).encode())
    inputs["modules"] = sha((ROOT / "tools" / "modules.txt").read_bytes())
    prev = json.loads(FINGERPRINT.read_text()) if FINGERPRINT.exists() else None
    if prev and prev["inputs"] == inputs:
        changed = [t for t in fp if prev["tables"].get(t) != fp[t]]
        check("I1", "re-running on unchanged inputs gives an identical database (table fingerprints)", not changed,
              f"{len(fp)} tables compared with the run of {prev['at']}", changed)
    else:
        gap("G-I1", "re-run comparison not possible on this run", "No earlier fingerprint for the same inputs. Run the pipeline again to compare.")
    FINGERPRINT.write_text(json.dumps(dict(at=time.strftime("%Y-%m-%d %H:%M:%S"), inputs=inputs, tables=fp), indent=1))

    # ------------------------------------------------------------------ report
    failed = [c for c in checks if not c["ok"]]
    out = ["# Terraria reference database: parity report", "",
           f"Generated {time.strftime('%Y-%m-%d %H:%M:%S')} by `tools/parity.py` in {time.time() - t0:.0f}s. Database: `db/terraria.sqlite` ({DB.stat().st_size / 1e6:.0f} MB).",
           "", f"**Result: {'PASS' if not failed else 'FAIL'}** - {len(checks)} checks, {len(failed)} failed, {len(gaps)} gaps listed.", "",
           "## Indexed vs expected", "", "| Artifact | In database | Expected | Source of the expected number |", "|---|---:|---:|---|"]
    why = {"files": "files on disk under decompile/", "types": "type definitions in the binaries' metadata", "methods": "method definitions in the metadata",
           "fields": "field definitions in the metadata", "strings": "ldstr instructions in the IL", "calls": "call / callvirt / newobj / ldftn instructions in the IL",
           "field accesses": "ldfld / stfld / ldsfld / stsfld / ldflda instructions in the IL",
           "functions (gamedb)": "hand-written methods with a body (gamedb's parser is heuristic, see G-G1)"}
    for k, (a, b) in totals.items():
        out.append(f"| {k} | {a:,} | {b:,} | {why[k]} |")
    for asm, path, h, size, ver in q("SELECT name, path, sha256, size, version FROM port_assemblies"):
        out.append(f"\n`{asm}` {ver}: `{path}` sha256 `{h}` ({size:,} bytes)")
    out += ["", "## Checks", "", "| | Check | Result | Detail |", "|---|---|---|---|"]
    for c in checks:
        out.append(f"| {c['id']} | {c['title']} | {'ok' if c['ok'] else '**FAILED**'} | {c['detail']} |")
    if failed:
        out += ["", "## Failures (exact items)"]
        for c in failed:
            out += ["", f"### {c['id']} {c['title']}", ""] + [f"- `{i}`" for i in c["items"][:200]]
            if len(c["items"]) > 200:
                out.append(f"- ... {len(c['items']) - 200} more")
    out += ["", "## Gaps (known limits, with the items involved)"]
    for g in gaps:
        out += ["", f"### {g['id']} {g['title']}{' (blocks the port)' if g['blocks'] else ''}", "", g["detail"]]
        if g["items"]:
            out += [""] + [f"- `{i}`" for i in g["items"][:60]]
            if len(g["items"]) > 60:
                out.append(f"- ... {len(g['items']) - 60} more (query the database for the full list)")
    REPORT.write_text("\n".join(out) + "\n", encoding="utf-8")
    print(f"parity: {'PASS' if not failed else 'FAIL'} - {len(checks)} checks, {len(failed)} failed, {len(gaps)} gaps -> {REPORT.relative_to(ROOT)}")
    for c in failed:
        print(f"  FAILED {c['id']} {c['title']}: {c['detail']} {c['items'][:5]}")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Build the port_* tables in db/terraria.sqlite next to gamedb's own tables.

gamedb's tables (files, functions, symbols, strings, edges, module_status) are never written here.
The port_* tables are derived data: they are rebuilt from meta/*.jsonl and decompile/ in one transaction,
so a re-run with unchanged inputs produces identical rows. `port_runs` is the only table that grows.

Sources of truth
  meta/<asm>/*.jsonl   what the binary contains (ECMA-335 metadata + IL), written by tools/tmeta
  meta/src/*.jsonl     what the decompiled C# declares, with exact line spans (Roslyn syntax trees)
  decompile/**         the files themselves (stored zlib-compressed in port_files.content)
"""
import hashlib, json, re, sqlite3, subprocess, sys, time, zlib
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DB = ROOT / "db" / "terraria.sqlite"
SRC = ROOT / "decompile"
META = ROOT / "meta"
ASSEMBLIES = ["Terraria", "ReLogic"]
GAMEDB_DIR = ROOT.parent / "VAEL - GAMELAB" / "tools" / "gamedb"

SCHEMA = """
CREATE TABLE IF NOT EXISTS port_runs(
  id INTEGER PRIMARY KEY, at TEXT NOT NULL, gamedb_commit TEXT, tool TEXT, counts TEXT);

DROP VIEW  IF EXISTS port_callgraph;
DROP TABLE IF EXISTS port_assemblies;   DROP TABLE IF EXISTS port_files;      DROP TABLE IF EXISTS port_types;
DROP TABLE IF EXISTS port_methods;      DROP TABLE IF EXISTS port_fields;     DROP TABLE IF EXISTS port_props;
DROP TABLE IF EXISTS port_calls;        DROP TABLE IF EXISTS port_field_refs; DROP TABLE IF EXISTS port_strings;
DROP TABLE IF EXISTS port_resources;    DROP TABLE IF EXISTS port_src_decls;  DROP TABLE IF EXISTS port_embedded;

CREATE TABLE port_assemblies(
  name TEXT PRIMARY KEY, path TEXT, sha256 TEXT, size INTEGER, version TEXT, refs TEXT,
  types INTEGER, methods INTEGER, fields INTEGER);

-- every file under decompile/, source or not. content = zlib(raw bytes), sha256 is of the raw bytes.
CREATE TABLE port_files(
  id INTEGER PRIMARY KEY, path TEXT UNIQUE NOT NULL, assembly TEXT NOT NULL,
  kind TEXT NOT NULL,            -- source | resource | project
  ext TEXT, namespace TEXT, size INTEGER NOT NULL, lines INTEGER NOT NULL, sha256 TEXT NOT NULL,
  newline TEXT,                  -- crlf | lf | mixed | none
  content BLOB NOT NULL,
  gamedb_file_id INTEGER);       -- files.id, NULL when gamedb does not index this extension

-- everything the C# parser saw, one row per declaration
CREATE TABLE port_src_decls(
  id INTEGER PRIMARY KEY, file_id INTEGER NOT NULL, kind TEXT NOT NULL, container TEXT NOT NULL, name TEXT NOT NULL,
  param_count INTEGER, params TEXT, start_line INTEGER, end_line INTEGER, has_body INTEGER, arity INTEGER, extra TEXT,
  assembly TEXT, token INTEGER);  -- metadata token of the member this declaration was matched to

CREATE TABLE port_types(
  assembly TEXT NOT NULL, token INTEGER NOT NULL, full_name TEXT NOT NULL, namespace TEXT, name TEXT,
  kind TEXT,                     -- class | struct | enum | interface | delegate
  base TEXT, declaring_token INTEGER, attrs INTEGER, generic_arity INTEGER, interfaces TEXT,
  compiler_generated INTEGER NOT NULL,
  file_id INTEGER, start_line INTEGER, end_line INTEGER,
  PRIMARY KEY(assembly, token));
CREATE INDEX port_types_name ON port_types(full_name);

CREATE TABLE port_methods(
  assembly TEXT NOT NULL, token INTEGER NOT NULL, type_token INTEGER NOT NULL, type_name TEXT NOT NULL, name TEXT NOT NULL,
  ret TEXT, params TEXT, param_count INTEGER, generic_arity INTEGER, attrs INTEGER, impl_attrs INTEGER,
  rva INTEGER, il_size INTEGER, il_sha256 TEXT,
  has_body INTEGER NOT NULL,     -- 1 = has IL in the binary
  compiler_generated INTEGER NOT NULL,
  src_match TEXT NOT NULL,       -- exact | by-count | origin | implicit | runtime | synthetic | none | ambiguous
  origin_token INTEGER,          -- for compiler-generated code: the user method it was lifted out of
  file_id INTEGER, start_line INTEGER, end_line INTEGER, src_sha256 TEXT,
  gamedb_function_id INTEGER,    -- functions.id when gamedb found the same method
  PRIMARY KEY(assembly, token));
CREATE INDEX port_methods_name ON port_methods(name);
CREATE INDEX port_methods_type ON port_methods(assembly, type_token);
CREATE INDEX port_methods_file ON port_methods(file_id);

CREATE TABLE port_fields(
  assembly TEXT NOT NULL, token INTEGER NOT NULL, type_token INTEGER NOT NULL, type_name TEXT NOT NULL, name TEXT NOT NULL,
  type TEXT, attrs INTEGER, constant TEXT, compiler_generated INTEGER NOT NULL,
  src_match TEXT NOT NULL, file_id INTEGER, line INTEGER,
  PRIMARY KEY(assembly, token));
CREATE INDEX port_fields_name ON port_fields(name);
CREATE INDEX port_fields_type ON port_fields(assembly, type_token);

CREATE TABLE port_props(
  assembly TEXT NOT NULL, kind TEXT NOT NULL, type_token INTEGER NOT NULL, type_name TEXT, name TEXT NOT NULL, type TEXT,
  getter_token INTEGER, setter_token INTEGER, file_id INTEGER, start_line INTEGER, end_line INTEGER);

-- exact call sites from IL. dst_token is NULL when the target is outside the two indexed assemblies.
CREATE TABLE port_calls(
  assembly TEXT NOT NULL, src_token INTEGER NOT NULL, il_offset INTEGER NOT NULL, opcode TEXT NOT NULL,
  dst_assembly TEXT, dst_token INTEGER, dst_type TEXT NOT NULL, dst_name TEXT NOT NULL, dst_param_count INTEGER);
CREATE INDEX port_calls_src ON port_calls(assembly, src_token);
CREATE INDEX port_calls_dst ON port_calls(dst_assembly, dst_token);
CREATE INDEX port_calls_ext ON port_calls(dst_type, dst_name);

CREATE TABLE port_field_refs(
  assembly TEXT NOT NULL, src_token INTEGER NOT NULL, il_offset INTEGER NOT NULL, opcode TEXT NOT NULL,
  access TEXT NOT NULL,          -- read | write | address | token
  dst_assembly TEXT, field_token INTEGER, field_type TEXT NOT NULL, field_name TEXT NOT NULL);
CREATE INDEX port_field_refs_src ON port_field_refs(assembly, src_token);
CREATE INDEX port_field_refs_dst ON port_field_refs(dst_assembly, field_token);

CREATE TABLE port_strings(
  assembly TEXT NOT NULL, method_token INTEGER NOT NULL, il_offset INTEGER NOT NULL, text TEXT NOT NULL);
CREATE INDEX port_strings_text ON port_strings(text);
CREATE INDEX port_strings_method ON port_strings(assembly, method_token);

CREATE TABLE port_resources(
  assembly TEXT NOT NULL, name TEXT NOT NULL, size INTEGER, sha256 TEXT,
  file_id INTEGER,               -- port_files row when ILSpy wrote the resource into decompile/
  embedded_library INTEGER NOT NULL DEFAULT 0);

CREATE TABLE port_embedded(
  resource TEXT PRIMARY KEY, size INTEGER, sha256 TEXT, decompiled_as TEXT);

CREATE VIEW port_callgraph AS
  SELECT c.assembly AS src_assembly, s.type_name AS src_type, s.name AS src_name, c.src_token,
         c.opcode, c.dst_assembly, c.dst_type, c.dst_name, c.dst_token, COUNT(*) AS sites
  FROM port_calls c JOIN port_methods s ON s.assembly = c.assembly AND s.token = c.src_token
  GROUP BY c.assembly, c.src_token, c.opcode, c.dst_assembly, c.dst_type, c.dst_name, c.dst_token;
"""


def rows(path):
    with open(path, encoding="utf-8") as f:
        for line in f:
            yield json.loads(line)


def sha(b):
    return hashlib.sha256(b).hexdigest()


def is_generated(name):
    """Compiler-generated names start with '<' (closures, iterators, anonymous types, backing fields)."""
    return name.startswith("<") or name.startswith("__StaticArrayInit") or name.startswith("$")


def last_segment(name):
    """`System.IDisposable.Dispose` -> `Dispose`; generic arguments are not split on."""
    depth, cut = 0, -1
    for i, ch in enumerate(name):
        if ch == "<":
            depth += 1
        elif ch == ">":
            depth -= 1
        elif ch == "." and depth == 0:
            cut = i
    return name[cut + 1:]


def member_key(name):
    """Explicit interface implementations are compared by their last segment, marked so they cannot collide with a normal member."""
    if name.startswith(".") or "." not in name:
        return name
    return "~" + last_segment(name)


ORIGIN = re.compile(r"^<([^>]*)>")


def main():
    t0 = time.time()
    con = sqlite3.connect(DB, isolation_level=None)   # transactions are explicit
    con.execute("PRAGMA journal_mode=WAL")
    con.execute("PRAGMA synchronous=NORMAL")
    if not con.execute("SELECT 1 FROM sqlite_master WHERE name='files'").fetchone():
        sys.exit("gamedb tables missing: run the index step first")
    con.execute("BEGIN IMMEDIATE")
    for stmt in [s.strip() for s in SCHEMA.split(";") if s.strip()]:
        con.execute(stmt)

    # ---------------------------------------------------------------- files
    gamedb_files = dict(con.execute("SELECT path, id FROM files"))
    file_id, file_lines = {}, {}
    disk = sorted(p for p in SRC.rglob("*") if p.is_file() and ".gamedb" not in p.parts and not p.parts[len(SRC.parts)].startswith("."))
    for i, p in enumerate(disk, 1):
        rel = p.relative_to(SRC).as_posix()
        raw = p.read_bytes()
        ext = p.suffix.lower().lstrip(".")
        kind = "source" if ext == "cs" else "project" if ext in ("csproj", "manifest", "ico") else "resource"
        crlf, lf = raw.count(b"\r\n"), raw.count(b"\n")
        newline = "none" if lf == 0 else "crlf" if crlf == lf else "lf" if crlf == 0 else "mixed"
        nlines = lf + (1 if raw and not raw.endswith(b"\n") else 0)
        con.execute("INSERT INTO port_files VALUES(?,?,?,?,?,?,?,?,?,?,?,?)",
                    (i, rel, rel.split("/")[0], kind, ext, None, len(raw), nlines, sha(raw), newline,
                     zlib.compress(raw, 9), gamedb_files.get(rel)))
        file_id[rel] = i
        if kind == "source":
            file_lines[i] = raw.split(b"\n")

    def span_sha(fid, a, b):
        # sha256 of lines a..b (1-based, inclusive) exactly as stored, joined with "\n"
        return sha(b"\n".join(file_lines[fid][a - 1:b]))

    # ---------------------------------------------------------------- source declarations
    decl_id = 0
    type_decl = {}                      # (asm, candidate full name) -> decl row
    members = defaultdict(list)         # (asm, container, key) -> [decl]
    fieldish = defaultdict(list)        # (asm, container, name) -> [decl]
    explicit = defaultdict(list)        # (asm, container, last segment) -> [explicit interface property / event]
    ns_of_file = {}
    decls = []
    for r in rows(META / "src" / "src_decls.jsonl"):
        path, kind, container, name, pc, ps, a, b, body, arity, extra = r
        decl_id += 1
        asm = path.split("/")[0]
        d = dict(id=decl_id, file_id=file_id[path], kind=kind, container=container, name=name, pc=pc, ps=ps, a=a, b=b,
                 body=body, arity=arity, extra=extra, asm=asm, token=None)
        decls.append(d)
        if kind in ("class", "struct", "interface", "enum", "delegate", "type"):
            # the container is a namespace or an enclosing type; the metadata decides which
            type_decl[(asm, container + "." + name if container else name)] = d
            type_decl.setdefault((asm, container + "+" + name), d)
            ns_of_file.setdefault(d["file_id"], container)
        if kind in ("method", "ctor", "dtor", "operator", "accessor"):
            members[(asm, container, member_key(name))].append(d)
        if kind in ("field", "enum_member", "event", "property"):
            fieldish[(asm, container, name)].append(d)
            if "." in name:             # explicit interface property / event: also findable by its last segment
                explicit[(asm, container, last_segment(name))].append(d)

    # ---------------------------------------------------------------- metadata
    counts = {}
    type_by_name = {}                   # (asm, full_name) -> token
    method_lookup = defaultdict(list)   # (asm, type full name, name, pc) -> [token]
    field_lookup = {}                   # (asm, type full name, name) -> token
    for asm in ASSEMBLIES:
        info = json.loads((META / asm / "assembly.json").read_text())
        con.execute("INSERT INTO port_assemblies VALUES(?,?,?,?,?,?,?,?,?)",
                    (asm, info["path"], info["sha256"], info["size"], info["version"], json.dumps(info["references"]),
                     info["types"], info["methods"], info["fields"]))
        types = {}
        for tok, full, ns, name, kind, base, decl_tok, attrs, arity, ifaces in rows(META / asm / "types.jsonl"):
            types[tok] = dict(full=full, ns=ns, name=name, kind=kind, base=base, decl=decl_tok, attrs=attrs, arity=arity, ifaces=ifaces)
            type_by_name[(asm, full)] = tok

        def chain_generated(tok):
            while tok:
                if is_generated(types[tok]["name"]):
                    return True
                tok = types[tok]["decl"]
            return False

        def user_owner(tok):
            """First enclosing type that the programmer wrote (skips closure / iterator classes)."""
            while tok and is_generated(types[tok]["name"]):
                tok = types[tok]["decl"]
            return tok

        src_types_with_ctor = set()
        for (a_, container, key), ds in members.items():
            if a_ == asm and key == ".ctor":
                src_types_with_ctor.add(container)

        for tok, t in types.items():
            gen = chain_generated(tok)
            d = type_decl.get((asm, t["full"]))
            if d is not None and d["token"] is None:
                d["token"] = tok
            con.execute("INSERT INTO port_types VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
                        (asm, tok, t["full"], t["ns"], t["name"], t["kind"], t["base"], t["decl"] or None, t["attrs"], t["arity"],
                         t["ifaces"], int(gen), d and d["file_id"], d and d["a"], d and d["b"]))
            t["decl_row"] = d
            t["gen"] = gen

        # ---- methods
        meths = list(rows(META / asm / "methods.jsonl"))
        by_group = defaultdict(list)
        for m in meths:
            by_group[(m[1], member_key(m[2]), m[5])].append(m)
        resolved = {}                   # token -> (match, decl or None)
        for (ttok, key, pc), group in by_group.items():
            t = types[ttok]
            cands = [d for d in members.get((asm, t["full"], key), []) if d["pc"] == pc]
            if not cands or t["gen"] or is_generated(group[0][2]):
                continue
            if len(group) == 1 and len(cands) == 1:
                resolved[group[0][0]] = ("exact" if cands[0]["ps"] == group[0][4] else "by-count", cands[0])
                continue
            free = list(cands)
            pending = []
            for m in group:             # overloads with the same arity: pair by parameter types
                hit = [d for d in free if d["ps"] == m[4]]
                if len(hit) > 1:        # same parameter list: generic arity, then return type (conversion operators,
                    narrowed = [d for d in hit if (d["arity"] or 0) == m[6]] or hit   # explicit interface members)
                    if len(narrowed) > 1:
                        narrowed = [d for d in narrowed if d["extra"] == m[3]] or narrowed
                    hit = narrowed
                if len(hit) == 1:
                    resolved[m[0]] = ("exact", hit[0]); free.remove(hit[0])
                else:
                    pending.append(m)
            if len(pending) == 1 and len(free) == 1:
                resolved[pending[0][0]] = ("by-count", free[0])
            else:
                for m in pending:
                    resolved[m[0]] = ("ambiguous", None)

        method_rows = {}
        for m in meths:
            tok, ttok, name, ret, ps, pc, garity, attrs, impl, rva, il_size, il_sha, _stack = m
            t = types[ttok]
            gen = t["gen"] or is_generated(name)
            match, d = resolved.get(tok, ("none", None))
            if d is None and match != "ambiguous":
                if t["kind"] == "delegate":
                    match, d = "runtime", t["decl_row"]           # Invoke/BeginInvoke/EndInvoke/.ctor have no IL
                elif not gen and name == ".cctor":
                    match, d = "implicit", t["decl_row"]          # static field initialisers
                elif not gen and name == ".ctor" and t["full"] not in src_types_with_ctor:
                    match, d = "implicit", t["decl_row"]          # default constructor / instance field initialisers
                elif not gen and name == ".ctor" and pc == 0 and t["kind"] == "struct":
                    match, d = "implicit", t["decl_row"]
            method_rows[tok] = dict(m=m, t=t, gen=gen, match=match, d=d, origin=None)
            method_lookup[(asm, t["full"], name, pc)].append(tok)

        # compiler-generated code -> the user method it came from
        user_methods = defaultdict(list)    # (owner type token, simple name) -> [token]
        for tok, r in method_rows.items():
            if not r["gen"]:
                user_methods[(r["m"][1], last_segment(r["m"][2]) if not r["m"][2].startswith(".") else r["m"][2])].append(tok)
        for tok, r in method_rows.items():
            if not r["gen"]:
                continue
            ttok, name = r["m"][1], r["m"][2]
            owner = user_owner(ttok)
            origin_name = None
            mo = ORIGIN.match(name)
            if mo and mo.group(1):
                origin_name = mo.group(1)                 # <Method>b__0, <Method>g__Local|1_0
            else:
                tt = ttok
                while tt and origin_name is None:         # <Method>d__12 (iterator), nested inside closures
                    mt = ORIGIN.match(types[tt]["name"])
                    if mt and mt.group(1):
                        origin_name = mt.group(1)
                    tt = types[tt]["decl"]
            if not owner:
                r["match"] = "synthetic"                  # <Module>, <PrivateImplementationDetails>, anonymous types
                continue
            if origin_name:
                c = user_methods.get((owner, last_segment(origin_name) if not origin_name.startswith(".") else origin_name), [])
                c = [x for x in c if method_rows[x]["d"] is not None]
                if c:
                    r["origin"] = c[0]
                    r["match"] = "origin" if len(c) == 1 else "origin-overloaded"
                    r["d"] = method_rows[c[0]]["d"] if len(c) == 1 else types[owner]["decl_row"]
                    continue
            # lambdas in field initialisers (<.cctor>b__, <>c::<.ctor>b__) and anything else: the owning type
            if types[owner]["decl_row"] is not None:
                r["match"], r["d"] = "origin-type", types[owner]["decl_row"]
            else:
                r["match"] = "synthetic"

        # link to gamedb's functions: same file, same simple name, header line inside the declaration span
        gfun = defaultdict(list)
        for fid, fname, start, gid in con.execute(
                "SELECT pf.id, f.name, f.start_line, f.id FROM functions f JOIN port_files pf ON pf.gamedb_file_id = f.file_id WHERE pf.assembly = ?", (asm,)):
            gfun[(fid, fname)].append((start, gid))
        for tok, r in method_rows.items():
            m, d = r["m"], r["d"]
            exact = r["match"] in ("exact", "by-count")
            gid = None
            if exact:
                simple = m[2]
                if simple in (".ctor", ".cctor"):
                    simple = r["t"]["name"].split("`")[0]
                elif "." in simple:
                    simple = last_segment(simple)
                near = [g for s, g in gfun.get((d["file_id"], simple), []) if d["a"] <= s <= d["b"]]
                if len(near) == 1:
                    gid = near[0]
                d["token"] = tok
            con.execute("INSERT INTO port_methods VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
                        (asm, tok, m[1], r["t"]["full"], m[2], m[3], m[4], m[5], m[6], m[7], m[8], m[9], max(m[10], 0), m[11],
                         int(m[9] != 0 and m[11] is not None), int(r["gen"]), r["match"], r["origin"],
                         d and d["file_id"], d and d["a"], d and d["b"],
                         span_sha(d["file_id"], d["a"], d["b"]) if exact else None, gid))

        # ---- fields
        nf = 0
        for tok, ttok, name, ftype, attrs, cst in rows(META / asm / "fields.jsonl"):
            t = types[ttok]
            gen = t["gen"] or is_generated(name)
            match, d = "none", None
            if name == "value__" and t["kind"] == "enum":
                match, d = "runtime", t["decl_row"]
            elif not gen:
                c = fieldish.get((asm, t["full"], name), [])
                if len(c) == 1:
                    match, d = "exact", c[0]; d["token"] = tok
                elif len(c) > 1:
                    match = "ambiguous"
            else:
                mo = ORIGIN.match(name)
                owner = user_owner(ttok)
                if mo and mo.group(1) and owner:          # <Prop>k__BackingField -> the property
                    c = fieldish.get((asm, types[owner]["full"], mo.group(1)), [])
                    if len(c) == 1:
                        match, d = "origin", c[0]
                if d is None:
                    match = "synthetic"
            field_lookup[(asm, t["full"], name)] = tok
            con.execute("INSERT INTO port_fields VALUES(?,?,?,?,?,?,?,?,?,?,?,?)",
                        (asm, tok, ttok, t["full"], name, ftype, attrs, cst, int(gen), match, d and d["file_id"], d and d["a"]))
            nf += 1

        for kind, ttok, name, ptype, g, s in rows(META / asm / "props.jsonl"):
            t = types[ttok]
            c = [d for d in fieldish.get((asm, t["full"], name), []) if d["kind"] in ("property", "event")]
            if not c and "." in name:   # several explicit implementations can share a last segment: pair by declared type
                c = explicit.get((asm, t["full"], last_segment(name)), [])
                c = [d for d in c if d["extra"] == ptype] or c
            if not c and name == "Item":
                c = [d for d in fieldish.get((asm, t["full"], "Item"), [])]
            d = c[0] if len(c) >= 1 else None
            con.execute("INSERT INTO port_props VALUES(?,?,?,?,?,?,?,?,?,?,?)",
                        (asm, kind, ttok, t["full"], name, ptype, g or None, s or None, d and d["file_id"], d and d["a"], d and d["b"]))

        for name, size, h in rows(META / asm / "resources.jsonl"):
            fid = file_id.get(f"{asm}/{name}")
            con.execute("INSERT INTO port_resources VALUES(?,?,?,?,?,?)", (asm, name, size, h, fid, int(name.lower().endswith(".dll"))))
        counts[asm] = dict(types=len(types), methods=len(meths), fields=nf)

    # ---------------------------------------------------------------- IL references (second pass: cross-assembly targets resolve now)
    def resolve_method(asm, tok, tname, name, pc):
        if tok:
            return asm, tok
        for other in ASSEMBLIES:
            if (other, tname) in type_by_name:
                c = method_lookup.get((other, tname, name, pc), [])
                return (other, c[0]) if len(c) == 1 else (other, None)
        return None, None

    ACCESS = {"ldfld": "read", "ldsfld": "read", "stfld": "write", "stsfld": "write", "ldflda": "address", "ldsflda": "address", "ldtoken": "token"}
    for asm in ASSEMBLIES:
        batch = []
        for src, off, op, tok, tname, name, pc in rows(META / asm / "calls.jsonl"):
            da, dt = resolve_method(asm, tok, tname, name, pc)
            batch.append((asm, src, off, op, da, dt, tname, name, pc if pc >= 0 else None))
        con.executemany("INSERT INTO port_calls VALUES(?,?,?,?,?,?,?,?,?)", batch)
        counts[asm]["calls"] = len(batch)
        batch = []
        for src, off, op, tok, tname, name in rows(META / asm / "fieldrefs.jsonl"):
            da, dt = (asm, tok) if tok else (None, None)
            if not tok:
                for other in ASSEMBLIES:
                    if (other, tname) in type_by_name:
                        da, dt = other, field_lookup.get((other, tname, name))
                        break
            batch.append((asm, src, off, op, ACCESS.get(op, op), da, dt, tname, name))
        con.executemany("INSERT INTO port_field_refs VALUES(?,?,?,?,?,?,?,?,?)", batch)
        counts[asm]["field_refs"] = len(batch)
        batch = [(asm, m, off, s) for m, off, s in rows(META / asm / "strings.jsonl")]
        con.executemany("INSERT INTO port_strings VALUES(?,?,?,?)", batch)
        counts[asm]["strings"] = len(batch)

    con.executemany("INSERT INTO port_src_decls VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
                    [(d["id"], d["file_id"], d["kind"], d["container"], d["name"], d["pc"], d["ps"], d["a"], d["b"], d["body"],
                      d["arity"], d["extra"], d["asm"], d["token"]) for d in decls])
    for fid, ns in ns_of_file.items():
        con.execute("UPDATE port_files SET namespace = ? WHERE id = ?", (ns, fid))

    emb = META / "embedded"
    decompiled = {"Terraria.Libraries.ReLogic.ReLogic.dll": "ReLogic"}
    for p in sorted(emb.glob("*.dll")):
        raw = p.read_bytes()
        con.execute("INSERT INTO port_embedded VALUES(?,?,?,?)", (p.name, len(raw), sha(raw), decompiled.get(p.name)))

    commit = subprocess.run(["git", "-C", str(GAMEDB_DIR), "rev-parse", "HEAD"], capture_output=True, text=True).stdout.strip()
    counts["files"] = len(disk)
    counts["src_decls"] = len(decls)
    con.execute("INSERT INTO port_runs(at, gamedb_commit, tool, counts) VALUES(?,?,?,?)",
                (time.strftime("%Y-%m-%d %H:%M:%S"), commit, "port_extend.py", json.dumps(counts, sort_keys=True)))
    con.execute("COMMIT")
    con.execute("PRAGMA wal_checkpoint(TRUNCATE)")
    con.close()
    print(f"port_extend: {json.dumps(counts, sort_keys=True)} in {time.time() - t0:.1f}s")


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""One command for the whole reference build. Re-runnable: every step skips or converges when nothing changed.

    python tools/pipeline.py                 all steps
    python tools/pipeline.py --steps index,extend,parity
    python tools/pipeline.py --force-decompile

Steps
  decompile  ILSpy -> decompile/<assembly>/  (skipped while the assembly's sha256 and the tool version are unchanged)
  meta       tmeta  -> meta/<assembly>/*.jsonl (binary ground truth) and meta/src/*.jsonl (exact C# declarations)
  index      gamedb index -r decompile --db db/terraria.sqlite   (incremental by mtime + size)
  extend     tools/port_extend.py   port_* tables in the same database
  parity     tools/parity.py        logs/parity-report.md, exit code 1 when a check fails

Nothing under the game folder is written to. An existing database is updated in place by gamedb's own
incremental index and by port_extend (which only rebuilds port_* tables); it is never deleted here.
"""
import argparse, hashlib, json, os, shutil, subprocess, sys, time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
GAMELAB = ROOT.parent / "VAEL - GAMELAB" / "tools"
GAME = Path(r"C:\Users\user\Documents\Games\Terraria")
ILSPY = GAMELAB / "ilspycmd" / "ilspycmd.exe"
XNA = GAMELAB / "xna40" / "Program Files" / "Microsoft XNA" / "XNA Game Studio"
GAMEDB = GAMELAB / "gamedb" / "target" / "release" / "gamedb.exe"
TMETA = ROOT / "tools" / "tmeta" / "bin" / "Release" / "net8.0" / "tmeta.exe"
DB = ROOT / "db" / "terraria.sqlite"
EMBEDDED = ROOT / "meta" / "embedded"   # embedded libraries under their resource names
REFS = ROOT / "meta" / "refs"           # the same files under their real assembly names (ReLogic.dll, ...)

# name -> assembly path. ReLogic.dll is Re-Logic's own engine library; it is stored inside Terraria.exe as a
# resource and written out by the Terraria decompile. The other embedded libraries are third-party (JSON.NET,
# Steamworks.NET, NVorbis...) and are listed in the database but not decompiled.
ASSEMBLIES = {
    "Terraria": GAME / "Terraria.exe",
    "ReLogic": EMBEDDED / "Terraria.Libraries.ReLogic.ReLogic.dll",
}


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def run(cmd, log=None, check=True):
    print("  $", " ".join(f'"{c}"' if " " in str(c) else str(c) for c in cmd), flush=True)
    p = subprocess.run([str(c) for c in cmd], cwd=ROOT, capture_output=True, text=True, encoding="utf-8", errors="replace")
    out = (p.stdout or "") + (p.stderr or "")
    if log:
        (ROOT / "logs" / log).write_text(out, encoding="utf-8")
    tail = [l for l in out.strip().splitlines() if l.strip()][-3:]
    for l in tail:
        print("   ", l[:300])
    if check and p.returncode != 0:
        sys.exit(f"step failed (exit {p.returncode}): {cmd[0]}")
    return p.returncode, out


def ilspy_version():
    p = subprocess.run([str(ILSPY), "--version"], capture_output=True, text=True)
    return p.stdout.strip().splitlines()[0] if p.stdout else "unknown"


def build_tmeta():
    src = ROOT / "tools" / "tmeta" / "Program.cs"
    if not TMETA.exists() or TMETA.stat().st_mtime < src.stat().st_mtime:
        run(["dotnet", "build", ROOT / "tools" / "tmeta", "-c", "Release", "-nologo", "-v", "q"], log="tmeta-build.log")


def decompile(force):
    build_tmeta()
    # Without these the decompiler cannot resolve ReLogic / Steamworks / XNA types and writes invalid C#.
    run([TMETA, "embedded", GAME / "Terraria.exe", EMBEDDED, REFS], log="embedded.log")
    ver = ilspy_version()
    for name, asm in ASSEMBLIES.items():
        if not asm.exists():
            sys.exit(f"missing assembly: {asm}")
        out = ROOT / "decompile" / name
        stamp_file = ROOT / "meta" / f"{name}.decompile.json"
        stamp = {"assembly": str(asm), "sha256": sha256(asm), "ilspy": ver, "language": "CSharp7_3",
                 "references": [str(GAME), str(XNA), str(REFS)]}
        if not force and out.exists() and stamp_file.exists() and json.loads(stamp_file.read_text())["input"] == stamp:
            print(f"decompile {name}: up to date")
            continue
        tmp = ROOT / "decompile" / f".{name}.tmp"
        if tmp.exists():
            shutil.rmtree(tmp)
        tmp.mkdir(parents=True)
        t = time.time()
        run([ILSPY, "--disable-updatecheck", "-p", "--nested-directories", "-lv", "CSharp7_3",
             "-r", GAME, "-r", XNA, "-r", REFS, "-o", tmp, asm], log=f"decompile-{name}.log")
        # ILSpy also writes the embedded libraries as loose .dll files; they already live in meta/embedded.
        for dll in tmp.glob("*.dll"):
            kept = EMBEDDED / dll.name
            if not kept.exists() or sha256(kept) != sha256(dll):
                sys.exit(f"embedded library mismatch: {dll.name}")
            dll.unlink()
        if out.exists():  # archive, never delete
            arch = ROOT / "_archive" / f"decompile-{name}-{time.strftime('%Y%m%d-%H%M%S')}"
            arch.parent.mkdir(exist_ok=True)
            os.replace(out, arch)
            print(f"  previous tree archived to {arch.relative_to(ROOT)}")
        os.replace(tmp, out)
        n = sum(1 for _ in out.rglob("*.cs"))
        stamp_file.write_text(json.dumps({"input": stamp, "cs_files": n, "seconds": round(time.time() - t, 1)}, indent=1))
        print(f"decompile {name}: {n} .cs files in {time.time() - t:.0f}s")


def meta():
    build_tmeta()
    for name, asm in ASSEMBLIES.items():
        run([TMETA, "meta", asm, ROOT / "meta" / name], log=f"meta-{name}.log")
    run([TMETA, "src", ROOT / "decompile", ROOT / "meta" / "src"], log="meta-src.log")


def index():
    DB.parent.mkdir(exist_ok=True)
    run([sys.executable, ROOT / "tools" / "modules_gen.py"])
    rules = ROOT / "tools" / "modules.txt"
    stamp = ROOT / "meta" / "modules.sha256"
    cmd = [GAMEDB, "index", "-r", "decompile", "--db", DB, "--rules", rules]
    # gamedb only re-buckets files it re-reads, so changed rules need one forced pass
    if DB.exists() and (not stamp.exists() or stamp.read_text() != sha256(rules)):
        cmd.append("--force")
    run(cmd, log="index.log")
    stamp.write_text(sha256(rules))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--steps", default="decompile,meta,index,extend,parity")
    ap.add_argument("--force-decompile", action="store_true")
    a = ap.parse_args()
    (ROOT / "logs").mkdir(exist_ok=True)
    rc = 0
    for step in a.steps.split(","):
        print(f"== {step}", flush=True)
        if step == "decompile":
            decompile(a.force_decompile)
        elif step == "meta":
            meta()
        elif step == "index":
            index()
        elif step == "extend":
            run([sys.executable, ROOT / "tools" / "port_extend.py"], log="extend.log")
        elif step == "parity":
            rc, _ = run([sys.executable, ROOT / "tools" / "parity.py"], log="parity.log", check=False)
        else:
            sys.exit(f"unknown step: {step}")
    sys.exit(rc)


if __name__ == "__main__":
    main()

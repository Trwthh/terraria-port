"""Terraria in the browser, served from this PC only. Python standard library only.

    python web/host/terraria_host.py [--port 7790] [--install <Terraria folder>] [--quiet] [--log FILE] [--no-gzip]

Open http://terraria.localhost:7790/ (or http://127.0.0.1:7790/). Listens on 127.0.0.1 only; nothing is reachable
from another computer. Stop it with Ctrl+C, or by ending its process (play-in-browser.cmd prints how).

What it serves (contract: web/www/HOST_ABI.md "Install over HTTP"):
  /                          the play page (web/www/play.html): only the game, full window
  /dev.html                  the developer page (web/www/index.html: toolbar, log, panels, OPFS import)
  /<file>                    the rest of web/www (scripts, game.wasm); ETag + "no-cache" (revalidated, so the
                             browser's HTTP cache and V8's compiled-code cache for game.wasm are reused)
  /game.wasm                 gzip-compressed once (cached next to this script by size + mtime) when the browser
                             accepts it; "br" too when the optional `brotli` module happens to be installed
  /install/manifest.json     every file of the install the game may read: path, size, mtime, content hash, and where
                             it sits in the pack; ETag; the directory tree is derived from the paths
  /install/pack/<id>         the small files (everything but the big wave bank) concatenated in one stream, in the
                             order the game opens them at start-up (pack-order.txt) then by path; immutable (the id
                             is a hash of the contents); byte ranges
  /install/file/<hash>       one file by content hash; immutable; byte ranges (the wave bank is read in blocks)

The install is read-only. Only Terraria.exe at its top and the files under Content/ are listed or opened; no other
program file (.exe, .dll, ... e.g. OnlineFix.dll, steam_api.dll) is ever opened, at the top or inside Content.
A file's id is BLAKE2b-128 of its path, size and modification time (no file is read for it; a changed file gets a
new id, so the browser's cache never serves a stale copy). --content-hash makes it a hash of the bytes instead
(one full read of the install, 731 MB, kept in web/host/cache/hashes.json by path, size and mtime).
"""
import argparse
import bisect
import email.utils
import gzip
import hashlib
import http.server
import json
import os
import re
import socketserver
import sys
import threading
import time
import urllib.parse

HERE = os.path.dirname(os.path.abspath(__file__))
WWW = os.path.normpath(os.path.join(HERE, "..", "www"))
CACHE = os.path.join(HERE, "cache")
ORDER_FILE = os.path.join(HERE, "pack-order.txt")
DEFAULT_INSTALL = r"C:\Users\user\Documents\Games\Terraria"
PROGRAM_FILE = re.compile(r"\.(exe|dll|pdb|so|dylib|bat|cmd|ps1)$", re.IGNORECASE)
BIG = 32 << 20  # files above this are not packed: read by block (/install/file/<hash>)
CHUNK = 1 << 20

TYPES = {
    ".html": "text/html; charset=utf-8",
    ".js": "text/javascript; charset=utf-8",
    ".mjs": "text/javascript; charset=utf-8",
    ".json": "application/json",
    ".wasm": "application/wasm",
    ".css": "text/css; charset=utf-8",
    ".png": "image/png",
    ".svg": "image/svg+xml",
    ".ico": "image/x-icon",
    ".md": "text/plain; charset=utf-8",
    ".txt": "text/plain; charset=utf-8",
}

try:  # optional, not part of the standard library: used only when present
    import brotli  # type: ignore
except ImportError:
    brotli = None


def log_line(text):
    if sys.stderr is not None:  # none under pythonw (play-in-browser.cmd starts it without a window)
        sys.stderr.write(time.strftime("%H:%M:%S ") + text + "\n")
    if Handler.log_file:
        with open(Handler.log_file, "a", encoding="utf-8") as f:
            f.write(time.strftime("%H:%M:%S ") + text + "\n")


# ------------------------------------------------------------------------------------------------ the install
def install_files(root):
    """[(rel, abs)] for Terraria.exe + Content/**, program files excluded (the rule of web/www/lib/import-plan.js)."""
    out = []
    exe = os.path.join(root, "Terraria.exe")
    if os.path.isfile(exe):
        out.append(("Terraria.exe", exe))
    for dirpath, dirnames, filenames in os.walk(os.path.join(root, "Content")):
        dirnames.sort()
        for name in sorted(filenames):
            if PROGRAM_FILE.search(name):
                continue
            full = os.path.join(dirpath, name)
            out.append((os.path.relpath(full, root).replace(os.sep, "/"), full))
    return out


def file_hash(path):
    h = hashlib.blake2b(digest_size=16)
    with open(path, "rb") as f:  # read-only
        while True:
            b = f.read(CHUNK)
            if not b:
                break
            h.update(b)
    return h.hexdigest()


class Install:
    """The manifest, the pack layout and the hash -> file map of one install."""

    def __init__(self, root, content_hash=False):
        self.root = root
        t0 = time.time()
        os.makedirs(CACHE, exist_ok=True)
        cache_path = os.path.join(CACHE, "hashes.json")
        try:
            with open(cache_path, encoding="utf-8") as f:
                old = json.load(f)
        except (OSError, ValueError):
            old = {}
        if old.get("root") != root or old.get("content") != content_hash:
            old = {"root": root, "files": {}}
        known = old.get("files", {})
        files = []
        hashed = 0
        hashed_bytes = 0
        for rel, full in install_files(root):
            st = os.stat(full)
            key = known.get(rel)
            if key and key[0] == st.st_size and key[1] == st.st_mtime_ns:
                h = key[2]
            elif content_hash:
                h = file_hash(full)
                hashed += 1
                hashed_bytes += st.st_size
            else:
                h = hashlib.blake2b(f"{rel}\0{st.st_size}\0{st.st_mtime_ns}".encode(), digest_size=16).hexdigest()
            files.append({"path": rel, "size": st.st_size, "mtime": st.st_mtime_ns // 1_000_000, "hash": h, "abs": full, "mtime_ns": st.st_mtime_ns})
        with open(cache_path + ".tmp", "w", encoding="utf-8") as f:
            json.dump({"root": root, "content": content_hash, "files": {x["path"]: [x["size"], x["mtime_ns"], x["hash"]] for x in files}}, f)
        os.replace(cache_path + ".tmp", cache_path)

        # pack order: the paths of pack-order.txt first (the order the game opens them at start-up), then by path
        order = {}
        try:
            with open(ORDER_FILE, encoding="utf-8") as f:
                for line in f:
                    p = line.strip()
                    if p and not p.startswith("#") and p not in order:
                        order[p] = len(order)
        except OSError:
            pass
        small = sorted((x for x in files if x["size"] <= BIG), key=lambda x: (order.get(x["path"], len(order)), x["path"]))
        off = 0
        ph = hashlib.blake2b(digest_size=16)
        self.segments = []  # [(start, end, abs)] of the pack, in order
        for x in small:
            x["off"] = off
            self.segments.append((off, off + x["size"], x["abs"]))
            ph.update(f"{x['path']}\0{x['size']}\0{x['hash']}\n".encode())
            off += x["size"]
        for x in files:
            x.setdefault("off", -1)
        self.starts = [a for a, _, _ in self.segments]
        self.pack_size = off
        self.pack_id = ph.hexdigest()
        self.by_hash = {x["hash"]: x for x in files}
        self.files = files
        manifest = {
            "version": 1,
            "pack": {"id": self.pack_id, "size": self.pack_size, "url": f"install/pack/{self.pack_id}", "files": len(small)},
            "files": [{"p": x["path"], "s": x["size"], "m": x["mtime"], "h": x["hash"], "o": x["off"]} for x in files],
        }
        self.manifest = json.dumps(manifest, separators=(",", ":")).encode()
        self.manifest_etag = '"m-' + hashlib.blake2b(self.manifest, digest_size=12).hexdigest() + '"'
        self.manifest_gz = gzip.compress(self.manifest, 6)
        total = sum(x["size"] for x in files)
        log_line(f"install {root}: {len(files)} files, {total / 1048576:.0f} MB ({hashed} hashed now, {hashed_bytes / 1048576:.0f} MB); "
                 f"pack {len(small)} files {self.pack_size / 1048576:.0f} MB ({len(order)} in start-up order); "
                 f"{time.time() - t0:.1f} s")
        # The pack as one file next to this script: opening 16,000 small files costs about 7 ms each on Windows
        # the first time (the on-access virus scan), 110 s for the whole pack; one file reads at disk speed. Made once
        # per pack id (in the background; until then the pack is read from the install's files).
        self.pack_cache = os.path.join(CACHE, f"pack-{self.pack_id}.bin")
        self.pack_ready = os.path.isfile(self.pack_cache) and os.path.getsize(self.pack_cache) == self.pack_size
        if not self.pack_ready:
            threading.Thread(target=self.build_pack_cache, daemon=True).start()

    def build_pack_cache(self):
        t0 = time.time()
        tmp = self.pack_cache + ".tmp"
        try:
            def read(path):
                with open(path, "rb") as f:  # read-only
                    return f.read()

            # the files are opened by 8 threads at once (the per-open cost is mostly waiting), written in order
            from concurrent.futures import ThreadPoolExecutor
            paths = [p for _, _, p in self.segments]
            with open(tmp, "wb") as out, ThreadPoolExecutor(8) as pool:
                for i in range(0, len(paths), 256):
                    for b in pool.map(read, paths[i:i + 256]):
                        out.write(b)
            if os.path.getsize(tmp) != self.pack_size:
                raise OSError("size differs from the manifest (a file changed meanwhile?)")
            os.replace(tmp, self.pack_cache)
            self.pack_ready = True
            # packs of other installs / builds (this script's own cache files)
            for name in os.listdir(CACHE):
                if name.startswith("pack-") and name.endswith(".bin") and name != os.path.basename(self.pack_cache):
                    try:
                        os.remove(os.path.join(CACHE, name))
                    except OSError:
                        pass
            log_line(f"pack cache {os.path.basename(self.pack_cache)}: {self.pack_size / 1048576:.0f} MB written in {time.time() - t0:.1f} s")
        except OSError as e:
            log_line(f"pack cache: not made ({e}); the pack is read from the install's files")

    def pack_read(self, start, end, write):
        """Writes pack bytes [start, end) through write(bytes)."""
        if self.pack_ready:
            with open(self.pack_cache, "rb") as f:
                f.seek(start)
                left = end - start
                while left > 0:
                    b = f.read(min(CHUNK, left))
                    if not b:
                        raise OSError("the pack cache is shorter than the pack")
                    write(b)
                    left -= len(b)
            return
        i = max(0, bisect.bisect_right(self.starts, start) - 1)
        pos = start
        while pos < end and i < len(self.segments):
            s, e, path = self.segments[i]
            if e <= pos:
                i += 1
                continue
            with open(path, "rb") as f:  # read-only
                f.seek(pos - s)
                left = min(e, end) - pos
                while left > 0:
                    b = f.read(min(CHUNK, left))
                    if not b:
                        raise OSError(f"{path} is shorter than at start-up (changed while serving?)")
                    write(b)
                    left -= len(b)
                    pos += len(b)
            i += 1


# ------------------------------------------------------------------------------------------------ precompressed game.wasm
_compress_lock = threading.Lock()


def precompressed(path, encoding):
    """The cached compressed copy of `path` for "gzip" / "br", made once per size + mtime; None if unavailable."""
    if encoding == "br" and brotli is None:
        return None
    st = os.stat(path)
    tag = f"{st.st_size}-{st.st_mtime_ns}"
    out = os.path.join(CACHE, os.path.basename(path) + "." + tag + (".gz" if encoding == "gzip" else ".br"))
    with _compress_lock:
        if not os.path.exists(out):
            t0 = time.time()
            with open(path, "rb") as f:
                data = f.read()
            comp = gzip.compress(data, 9, mtime=0) if encoding == "gzip" else brotli.compress(data, quality=9)
            # older copies of the same file are replaced (they belong to a previous build)
            for old in os.listdir(CACHE):
                if old.startswith(os.path.basename(path) + ".") and old.endswith(out[-3:]) and old != os.path.basename(out):
                    try:
                        os.remove(os.path.join(CACHE, old))  # this script's own derived cache file (re-made on demand)
                    except OSError:
                        pass
            with open(out + ".tmp", "wb") as f:
                f.write(comp)
            os.replace(out + ".tmp", out)
            log_line(f"{os.path.basename(path)}: {encoding} {len(data) / 1048576:.1f} -> {len(comp) / 1048576:.1f} MB in {time.time() - t0:.1f} s (cached)")
    return out


# ------------------------------------------------------------------------------------------------ HTTP
def parse_range(header, size):
    """(start, end) exclusive for a single "bytes=a-b" range, None for none, False when unsatisfiable."""
    if not header:
        return None
    m = re.fullmatch(r"\s*bytes=(\d*)-(\d*)\s*", header)
    if not m or (m.group(1) == "" and m.group(2) == ""):
        return None  # several ranges or garbage: the whole body (allowed by RFC 9110)
    if m.group(1) == "":
        n = int(m.group(2))
        if n == 0:
            return False
        return (max(0, size - n), size)
    a = int(m.group(1))
    b = int(m.group(2)) + 1 if m.group(2) else size
    if a >= size:
        return False
    return (a, min(b, size))


class Handler(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"  # keep-alive: one connection carries many requests
    server_version = "terraria-host"
    install = None
    quiet = False
    log_file = None
    use_gzip = True

    def log_message(self, fmt, *args):
        if not self.quiet:
            log_line("%s %s" % (self.address_string(), fmt % args))

    def common_headers(self):
        # cross-origin isolation (SharedArrayBuffer for the audio ring, JSPI-friendly workers)
        self.send_header("Cross-Origin-Opener-Policy", "same-origin")
        self.send_header("Cross-Origin-Embedder-Policy", "require-corp")
        self.send_header("Cross-Origin-Resource-Policy", "same-origin")
        self.send_header("X-Content-Type-Options", "nosniff")

    def fail(self, code, text):
        body = (text + "\n").encode()
        self.send_response(code)
        self.common_headers()
        self.send_header("Content-Type", "text/plain; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        if self.command != "HEAD":
            self.wfile.write(body)

    def do_HEAD(self):
        self.do_GET()

    def do_GET(self):
        try:
            self.route()
        except (ConnectionResetError, ConnectionAbortedError, BrokenPipeError):
            pass  # the page stopped reading (a cancelled fetch): normal

    def route(self):
        path = urllib.parse.unquote(urllib.parse.urlsplit(self.path).path)
        if path in ("/", "/play", "/play.html"):
            return self.static(os.path.join(WWW, "play.html"))
        if path == "/dev.html":
            return self.static(os.path.join(WWW, "index.html"))
        if path.startswith("/install/"):
            return self.install_route(path[len("/install/"):])
        if path.startswith("/__devinstall/"):
            return self.fail(404, "this server has no dev import: /install/ serves the files directly")
        rel = path.lstrip("/")
        full = os.path.normpath(os.path.join(WWW, rel))
        if not full.startswith(WWW + os.sep) or not os.path.isfile(full):
            return self.fail(404, "not found")
        return self.static(full)

    # ---------------------------------------------------------------- web/www files
    def static(self, full):
        try:
            st = os.stat(full)
        except OSError:
            return self.fail(404, "not found")
        etag = f'"{st.st_size:x}-{st.st_mtime_ns:x}"'
        ctype = TYPES.get(os.path.splitext(full)[1].lower(), "application/octet-stream")
        encoding = None
        send = full
        if full.endswith(".wasm") and self.use_gzip:
            accept = self.headers.get("Accept-Encoding", "")
            for enc in ("br", "gzip"):
                if re.search(r"(^|[,\s])" + enc + r"($|[,;\s])", accept):
                    p = precompressed(full, enc)
                    if p:
                        encoding, send = enc, p
                        break
        if encoding:
            etag = etag[:-1] + "-" + encoding + '"'
        if self.headers.get("If-None-Match") == etag:
            self.send_response(304)
            self.common_headers()
            self.send_header("ETag", etag)
            self.send_header("Cache-Control", "no-cache")
            self.send_header("Vary", "Accept-Encoding")
            self.send_header("Content-Length", "0")
            self.end_headers()
            return
        size = os.path.getsize(send)
        self.send_response(200)
        self.common_headers()
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(size))
        self.send_header("ETag", etag)
        self.send_header("Cache-Control", "no-cache")  # revalidated every time: a rebuilt file is seen at once
        self.send_header("Vary", "Accept-Encoding")
        self.send_header("Last-Modified", email.utils.formatdate(st.st_mtime, usegmt=True))
        if encoding:
            self.send_header("Content-Encoding", encoding)
        self.end_headers()
        if self.command == "HEAD":
            return
        with open(send, "rb") as f:
            while True:
                b = f.read(CHUNK)
                if not b:
                    break
                self.wfile.write(b)

    # ---------------------------------------------------------------- the install
    def install_route(self, rest):
        inst = self.install
        if inst is None:
            return self.fail(404, "no install (start with --install <Terraria folder>)")
        if rest == "manifest.json":
            if self.headers.get("If-None-Match") == inst.manifest_etag:
                self.send_response(304)
                self.common_headers()
                self.send_header("ETag", inst.manifest_etag)
                self.send_header("Cache-Control", "no-cache")
                self.send_header("Content-Length", "0")
                self.end_headers()
                return
            gz = "gzip" in self.headers.get("Accept-Encoding", "")
            body = inst.manifest_gz if gz else inst.manifest
            self.send_response(200)
            self.common_headers()
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.send_header("ETag", inst.manifest_etag)
            self.send_header("Cache-Control", "no-cache")
            self.send_header("Vary", "Accept-Encoding")
            if gz:
                self.send_header("Content-Encoding", "gzip")
            self.end_headers()
            if self.command != "HEAD":
                self.wfile.write(body)
            return
        m = re.fullmatch(r"pack/([0-9a-f]{32})", rest)
        if m:
            if m.group(1) != inst.pack_id:
                return self.fail(404, "this pack is not the install's current one (reload the manifest)")
            return self.ranged(inst.pack_size, f'"p-{inst.pack_id}"', lambda a, b: inst.pack_read(a, b, self.wfile.write))
        m = re.fullmatch(r"file/([0-9a-f]{32})", rest)
        if m:
            x = inst.by_hash.get(m.group(1))
            if x is None:
                return self.fail(404, "no file with this hash in the install")

            def body(a, b, path=x["abs"]):
                with open(path, "rb") as f:  # read-only
                    f.seek(a)
                    left = b - a
                    while left > 0:
                        chunk = f.read(min(CHUNK, left))
                        if not chunk:
                            break
                        self.wfile.write(chunk)
                        left -= len(chunk)
            return self.ranged(x["size"], f'"f-{m.group(1)}"', body)
        return self.fail(404, "not found")

    def ranged(self, size, etag, body):
        """An immutable resource with byte ranges."""
        if self.headers.get("If-None-Match") == etag:
            self.send_response(304)
            self.common_headers()
            self.send_header("ETag", etag)
            self.send_header("Cache-Control", "public, max-age=31536000, immutable")
            self.send_header("Content-Length", "0")
            self.end_headers()
            return
        r = parse_range(self.headers.get("Range"), size)
        if r is False:
            self.send_response(416)
            self.common_headers()
            self.send_header("Content-Range", f"bytes */{size}")
            self.send_header("Content-Length", "0")
            self.end_headers()
            return
        a, b = r if r else (0, size)
        self.send_response(206 if r else 200)
        self.common_headers()
        self.send_header("Content-Type", "application/octet-stream")
        self.send_header("Accept-Ranges", "bytes")
        self.send_header("Content-Length", str(b - a))
        if r:
            self.send_header("Content-Range", f"bytes {a}-{b - 1}/{size}")
        self.send_header("ETag", etag)
        self.send_header("Cache-Control", "public, max-age=31536000, immutable")
        self.end_headers()
        if self.command != "HEAD" and b > a:
            body(a, b)


class Server(socketserver.ThreadingMixIn, http.server.HTTPServer):
    daemon_threads = True
    allow_reuse_address = False  # a second copy on the same port fails instead of sharing it


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--port", type=int, default=7790, help="port on 127.0.0.1 (default 7790; 0 picks a free one)")
    ap.add_argument("--install", default=os.environ.get("TERRARIA_DIR", DEFAULT_INSTALL), help="the Terraria folder (read-only)")
    ap.add_argument("--quiet", action="store_true", help="do not log requests")
    ap.add_argument("--log", help="also append the log to this file")
    ap.add_argument("--content-hash", action="store_true", help="file ids from the bytes (one full read) instead of path + size + mtime")
    ap.add_argument("--no-gzip", action="store_true", help="serve game.wasm uncompressed")
    args = ap.parse_args()
    root = os.path.abspath(args.install)
    if not os.path.isdir(os.path.join(root, "Content")) or not os.path.isfile(os.path.join(root, "Terraria.exe")):
        sys.exit(f"--install {root}: needs Terraria.exe and a Content folder")
    Handler.quiet = args.quiet
    Handler.log_file = args.log
    Handler.use_gzip = not args.no_gzip
    Handler.install = Install(root, args.content_hash)
    wasm = os.path.join(WWW, "game.wasm")
    if Handler.use_gzip and os.path.isfile(wasm):
        precompressed(wasm, "gzip")
        if brotli is not None:
            precompressed(wasm, "br")
    with Server(("127.0.0.1", args.port), Handler) as httpd:  # this PC only
        port = httpd.server_address[1]
        print(f"serving Terraria at http://terraria.localhost:{port}/ and http://127.0.0.1:{port}/ (Ctrl+C stops)", flush=True)
        log_line(f"listening on 127.0.0.1:{port} (pid {os.getpid()})")
        try:
            httpd.serve_forever()
        except KeyboardInterrupt:
            pass


if __name__ == "__main__":
    main()

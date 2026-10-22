"""Tiny local static server for the web shell. Test use only.

    python serve.py [--port 8137] [--quiet] [--install <Terraria folder>]

Serves this folder on 127.0.0.1 only, with the cross-origin isolation headers (COOP same-origin + COEP
require-corp) so SharedArrayBuffer is available, and application/wasm for .wasm. Ctrl+C stops it.
--port 0 picks a free port; the first line printed names it.

Developer tool, off unless --install is given: the page's ?devimport reads the install from this server instead of
the folder picker (for automated runs). It serves, read-only:
  /__devinstall/list.json        [{"path": "Terraria.exe" | "Content/...", "size": n}, ...]
  /__devinstall/file/<path>      the file itself (only paths in that list)
Only Terraria.exe at the top of the folder and the files under Content/ are listed or served; no other program file
(.exe, .dll, ...) is ever opened, at the top or inside Content.
"""
import argparse
import functools
import http.server
import json
import os
import re
import sys
import urllib.parse

HERE = os.path.dirname(os.path.abspath(__file__))
PROGRAM_FILE = re.compile(r"\.(exe|dll|pdb|so|dylib|bat|cmd|ps1)$", re.IGNORECASE)
DEV_PREFIX = "/__devinstall/"


def install_listing(root):
    """{relative path: absolute path} for Terraria.exe + Content/** (the same rule as lib/import-plan.js)."""
    files = {}
    exe = os.path.join(root, "Terraria.exe")
    if os.path.isfile(exe):
        files["Terraria.exe"] = exe
    content = os.path.join(root, "Content")
    for dirpath, dirnames, filenames in os.walk(content):
        dirnames.sort()
        for name in sorted(filenames):
            if PROGRAM_FILE.search(name):
                continue
            full = os.path.join(dirpath, name)
            rel = os.path.relpath(full, root).replace(os.sep, "/")
            files[rel] = full
    return files


class Handler(http.server.SimpleHTTPRequestHandler):
    extensions_map = {
        **http.server.SimpleHTTPRequestHandler.extensions_map,
        ".wasm": "application/wasm",
        ".js": "text/javascript",
        ".mjs": "text/javascript",
        ".json": "application/json",
        ".md": "text/plain; charset=utf-8",
        ".html": "text/html; charset=utf-8",
    }
    install = None  # {rel: abs} when --install is given
    quiet = False

    def end_headers(self):
        self.send_header("Cross-Origin-Opener-Policy", "same-origin")
        self.send_header("Cross-Origin-Embedder-Policy", "require-corp")
        self.send_header("Cross-Origin-Resource-Policy", "same-origin")
        self.send_header("Cache-Control", "no-store")
        super().end_headers()

    def do_GET(self):
        path = urllib.parse.urlsplit(self.path).path
        if path.startswith(DEV_PREFIX):
            return self.dev_install(urllib.parse.unquote(path[len(DEV_PREFIX):]))
        return super().do_GET()

    def dev_install(self, rest):
        if self.install is None:
            return self.send_error(404, "dev install is off (start serve.py with --install <dir>)")
        if rest == "list.json":
            body = json.dumps([{"path": rel, "size": os.path.getsize(full)} for rel, full in self.install.items()]).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        if rest.startswith("file/"):
            full = self.install.get(rest[len("file/"):])
            if full is None:
                return self.send_error(404, "not in the dev install listing")
            size = os.path.getsize(full)
            self.send_response(200)
            self.send_header("Content-Type", "application/octet-stream")
            self.send_header("Content-Length", str(size))
            self.end_headers()
            try:
                with open(full, "rb") as f:  # read-only
                    while True:
                        chunk = f.read(1 << 20)
                        if not chunk:
                            break
                        self.wfile.write(chunk)
            except (ConnectionResetError, ConnectionAbortedError, BrokenPipeError):
                pass  # the page stopped reading (e.g. after the "MZ" check): normal
            return
        return self.send_error(404)

    def log_message(self, fmt, *args):
        if not self.quiet:
            sys.stderr.write("%s %s\n" % (self.address_string(), fmt % args))


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--port", type=int, default=8137)
    ap.add_argument("--quiet", action="store_true", help="do not log requests")
    ap.add_argument("--install", help="developer tool: serve this Terraria folder's Terraria.exe + Content/** for ?devimport")
    args = ap.parse_args()
    if args.install:
        root = os.path.abspath(args.install)
        if not os.path.isdir(os.path.join(root, "Content")):
            sys.exit(f"--install {root}: no Content folder")
        Handler.install = install_listing(root)
        total = sum(os.path.getsize(p) for p in Handler.install.values())
        print(f"dev install: {len(Handler.install)} files, {total / 1048576:.0f} MB from {root} (read-only)", flush=True)
    Handler.quiet = args.quiet
    handler = functools.partial(Handler, directory=HERE)
    with http.server.ThreadingHTTPServer(("127.0.0.1", args.port), handler) as httpd:
        port = httpd.server_address[1]
        print(f"serving {HERE} at http://127.0.0.1:{port}/ (Ctrl+C stops)", flush=True)
        try:
            httpd.serve_forever()
        except KeyboardInterrupt:
            pass


if __name__ == "__main__":
    main()

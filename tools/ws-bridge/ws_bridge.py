"""WebSocket <-> TCP bridge for the browser build's multiplayer (web/www/lib/net-bridge.js, HOST_ABI.md "Network").

    python tools/ws-bridge/ws_bridge.py [--port 7780] [--allow-lan] [--origin URL ...] [--trace-terraria] [--log FILE]
                                        [--listen <tcp port> --to-page]

A page cannot open TCP sockets. When the game in the browser joins a server ("Join via IP", address and port typed
as usual), the page opens a WebSocket to this bridge, ws://127.0.0.1:<port>/?host=<address>&port=<n>, and the bridge
opens the TCP connection to <address>:<n> and copies bytes both ways (binary WebSocket messages; a close on either
side closes the other).

- Listens on 127.0.0.1 only. Python standard library only.
- Targets: loopback addresses (127.0.0.0/8, ::1) only. `--allow-lan` also allows private LAN addresses
  (10/8, 172.16/12, 192.168/16, 169.254/16, fc00::/7, fe80::/10). Public addresses are always refused. Only IP
  addresses (and "localhost"): no name lookups.
- The TCP connection is made BEFORE the WebSocket handshake is answered: a page sees "open" only for a connected
  socket, and a refused / failed connection as a failed handshake (HTTP 502), which the page reports as
  "connection refused".
- Origins: a web page anywhere could otherwise use the bridge to reach this machine's local ports, so the
  handshake's Origin must be a loopback page (http(s)://127.0.0.1:*, http(s)://localhost:*, [::1]) or one given
  with --origin. A request without an Origin header (not a browser) is accepted.
- Logs every connection (open, refused, closed, bytes each way, duration) to stdout and --log.
  `--trace-terraria`: also reads the Terraria message stream (u16 length, u8 type, payload) in both directions and
  logs the notable messages (connect request, player info, tile changes, chat) and per-type counts at the close.

Reverse mode (`--listen <tcp port> --to-page`): the page HOSTS (Host & Play runs the dedicated server in a Worker
of the page) and other players connect with plain TCP. The bridge also listens on 127.0.0.1:<tcp port>. The page's
server Worker registers itself with a control WebSocket, ws://127.0.0.1:<port>/?listen=<the server's port>; for
each TCP connection the bridge sends the page a text message {"conn": <id>, "peer": "<address>:<port>"} on it, the
page opens a WebSocket ws://127.0.0.1:<port>/?accept=<id> for that connection, and the bridge copies bytes between
the two (binary messages; a close on either side closes the other). A TCP connection while no page is registered,
or one the page does not take within 10 s, is closed. One page at a time (a new registration replaces the old).
"""
import argparse
import asyncio
import base64
import hashlib
import ipaddress
import os
import struct
import sys
import time
import urllib.parse

GUID = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"
MAX_FRAME = 16 << 20

LOG_FILE = None


def log(text):
    line = f"{time.strftime('%H:%M:%S')} [ws-bridge] {text}"
    print(line, flush=True)
    if LOG_FILE:
        with open(LOG_FILE, "a", encoding="utf-8") as f:
            f.write(line + "\n")


# ------------------------------------------------------------------------------------------------ policy
def target_allowed(host, allow_lan):
    """(address, None) when allowed, else (None, reason)."""
    h = host.strip().strip("[]")
    if h.lower() == "localhost":
        h = "127.0.0.1"
    try:
        a = ipaddress.ip_address(h)
    except ValueError:
        return None, f"not an IP address: {host!r} (the bridge does no name lookups)"
    if a.is_loopback:
        return str(a), None
    if allow_lan and (a.is_private or a.is_link_local) and not a.is_multicast and not a.is_unspecified:
        return str(a), None
    if a.is_private or a.is_link_local:
        return None, f"{a} is a LAN address: start the bridge with --allow-lan to allow it"
    return None, f"{a} is not a loopback or LAN address"


def origin_allowed(origin, extra):
    if origin is None:
        return True
    if origin in extra:
        return True
    try:
        u = urllib.parse.urlsplit(origin)
    except ValueError:
        return False
    return u.scheme in ("http", "https") and (u.hostname or "") in ("127.0.0.1", "localhost", "::1")


# ------------------------------------------------------------------------------------------------ Terraria trace
class TerrariaTrace:
    """Splits one direction's bytes into Terraria messages (u16 total length incl. itself, u8 type) and logs the
    notable ones. Best effort: a parse problem only stops the trace of that direction."""

    NAMES = {1: "ConnectRequest", 2: "Kick", 3: "PlayerSlot", 4: "PlayerInfo", 6: "WorldRequest", 7: "WorldInfo",
             8: "TileSectionRequest", 9: "Status", 10: "TileSection", 12: "PlayerSpawn", 13: "PlayerControls",
             17: "TileChange", 37: "PasswordRequest", 38: "Password", 49: "InitialSpawn", 82: "NetModule"}

    def __init__(self, cid, direction, to_server=None):
        self.cid, self.dir, self.buf, self.counts, self.ok = cid, direction, bytearray(), {}, True
        # the client -> server direction (whose chat messages are commands); by default the page's way up
        self.to_server = direction == "page->server" if to_server is None else to_server

    @staticmethod
    def read_string(b, o):
        # .NET BinaryWriter string: 7-bit encoded length, UTF-8
        n, shift = 0, 0
        while True:
            c = b[o]
            o += 1
            n |= (c & 0x7F) << shift
            shift += 7
            if c < 0x80:
                break
        return b[o:o + n].decode("utf-8", "replace"), o + n

    def feed(self, data):
        if not self.ok:
            return
        self.buf += data
        while len(self.buf) >= 3:
            n = struct.unpack_from("<H", self.buf, 0)[0]
            if n < 3:
                log(f"#{self.cid} trace {self.dir}: bad message length {n}; trace stopped")
                self.ok = False
                return
            if len(self.buf) < n:
                return
            msg = bytes(self.buf[2:n])
            del self.buf[:n]
            t = msg[0]
            self.counts[t] = self.counts.get(t, 0) + 1
            try:
                self.note(t, msg[1:])
            except Exception as e:  # noqa: BLE001 - tracing must never break the bridge
                log(f"#{self.cid} trace {self.dir}: message {t}: {e}")

    def note(self, t, p):
        d = f"#{self.cid} {self.dir}"
        if t == 1:
            log(f"{d} ConnectRequest {self.read_string(p, 0)[0]!r}")
        elif t == 4 and self.counts[t] <= 3:
            name, _ = self.read_string(p, 8)  # slot, skin variant, voice variant, voice pitch (f32), hair
            log(f"{d} PlayerInfo slot {p[0]} name {name!r}")
        elif t == 17:
            action, x, y = p[0], *struct.unpack_from("<hh", p, 1)
            log(f"{d} TileChange action {action} at tile ({x}, {y})")
        elif t == 82:
            module = struct.unpack_from("<H", p, 0)[0]
            if module == 1:  # NetTextModule
                if self.to_server:
                    cmd, o = self.read_string(p, 2)
                    text, _ = self.read_string(p, o)
                    log(f"{d} chat command {cmd!r} text {text!r}")
                else:
                    # server -> client: author byte, NetworkText (mode byte + text ...), color
                    log(f"{d} chat broadcast ({len(p)} bytes): {p[3:].decode('utf-8', 'replace')[:120]!r}")
        elif t == 2:
            log(f"{d} Kick ({len(p)} bytes)")
        elif t == 37:
            log(f"{d} PasswordRequest")

    def summary(self):
        parts = [f"{self.NAMES.get(k, k)} {v}" for k, v in sorted(self.counts.items())]
        return f"{self.dir}: {sum(self.counts.values())} messages ({', '.join(parts)})"


# ------------------------------------------------------------------------------------------------ WebSocket
async def read_frame(reader):
    """(opcode, fin, payload) of one client frame (masked, as RFC 6455 requires of clients)."""
    h = await reader.readexactly(2)
    fin, op = h[0] & 0x80, h[0] & 0x0F
    masked, n = h[1] & 0x80, h[1] & 0x7F
    if n == 126:
        n = struct.unpack(">H", await reader.readexactly(2))[0]
    elif n == 127:
        n = struct.unpack(">Q", await reader.readexactly(8))[0]
    if n > MAX_FRAME:
        raise ValueError(f"frame of {n} bytes")
    if not masked:
        raise ValueError("unmasked client frame")
    key = await reader.readexactly(4)
    data = bytearray(await reader.readexactly(n))
    for i in range(n):
        data[i] ^= key[i & 3]
    return op, bool(fin), bytes(data)


def frame(op, payload=b""):
    n = len(payload)
    if n < 126:
        head = struct.pack(">BB", 0x80 | op, n)
    elif n < 65536:
        head = struct.pack(">BBH", 0x80 | op, 126, n)
    else:
        head = struct.pack(">BBQ", 0x80 | op, 127, n)
    return head + payload


async def splice(cid, reader, writer, tr, tw, up, down, t0, ws_name="page", tcp_name="server"):
    """Copies bytes between a WebSocket (reader / writer, after the handshake) and a TCP connection (tr / tw) until
    either side closes; logs the totals. `up` traces WebSocket -> TCP, `down` TCP -> WebSocket (or None)."""
    stats = {"up": 0, "down": 0, "end": None}
    wlock = asyncio.Lock()

    async def send_ws(op, payload=b""):
        async with wlock:
            writer.write(frame(op, payload))
            await writer.drain()

    async def ws_to_tcp():
        frag = bytearray()
        while True:
            op, fin, data = await read_frame(reader)
            if op == 0x8:  # close
                stats["end"] = stats["end"] or f"the {ws_name} closed"
                try:
                    await send_ws(0x8, data[:2] if len(data) >= 2 else struct.pack(">H", 1000))
                except ConnectionError:
                    pass
                # half-close the TCP side (FIN after the last bytes) instead of closing it at once: a close while
                # the other end's bytes are still unread is a reset on Windows, and a reset makes the other end drop
                # what it received but has not read yet (a server's Kick message just before it closes: the
                # console's kick / ban). tcp_to_ws then reads on until that end closes too (splice waits 2 s).
                if tw.can_write_eof():
                    try:
                        tw.write_eof()
                    except OSError:
                        pass
                return
            if op == 0x9:
                await send_ws(0xA, data)
                continue
            if op == 0xA:
                continue
            if op in (0x0, 0x1, 0x2):
                frag += data
                if not fin:
                    continue
                payload, frag = bytes(frag), bytearray()
                stats["up"] += len(payload)
                if up:
                    up.feed(payload)
                tw.write(payload)
                await tw.drain()
            else:
                raise ValueError(f"unknown opcode {op}")

    async def tcp_to_ws():
        while True:
            data = await tr.read(65536)
            if not data:
                stats["end"] = stats["end"] or f"the {tcp_name} closed"
                try:
                    await send_ws(0x8, struct.pack(">H", 1000))
                except (ConnectionError, RuntimeError):
                    pass  # the WebSocket side closed first (half-close above)
                return
            stats["down"] += len(data)
            if down:
                down.feed(data)
            await send_ws(0x2, data)

    tasks = [asyncio.create_task(ws_to_tcp()), asyncio.create_task(tcp_to_ws())]
    try:
        done, pending = await asyncio.wait(tasks, return_when=asyncio.FIRST_COMPLETED)
        if tasks[0] in done and pending:
            # the WebSocket side closed and the TCP side is half-closed: let its end read what came and close
            more, pending = await asyncio.wait(pending, timeout=2)
            done |= more
        for t in done:
            e = t.exception()
            if e and not stats["end"]:
                stats["end"] = f"{type(e).__name__}"
        for t in pending:
            t.cancel()
    finally:
        tw.close()
    log(f"#{cid} closed ({stats['end']}) after {time.monotonic() - t0:.1f} s: {stats['up']} bytes {ws_name}->{tcp_name}, {stats['down']} bytes {tcp_name}->{ws_name}")
    if up:
        log(f"#{cid} {up.summary()}")
        log(f"#{cid} {down.summary()}")


def accept_key(key):
    return base64.b64encode(hashlib.sha1((key + GUID).encode()).digest()).decode()


async def handshake(writer, hdr):
    writer.write(("HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n"
                  f"Sec-WebSocket-Accept: {accept_key(hdr['sec-websocket-key'])}\r\n\r\n").encode())
    await writer.drain()


class Bridge:
    def __init__(self, args):
        self.args = args
        self.next_id = 1
        # reverse mode: the registered page (its control connection's writer and a lock), TCP connections waiting
        # for the page's accept WebSocket (id -> dict)
        self.page = None
        self.waiting = {}

    async def handle(self, reader, writer):
        cid = self.next_id
        self.next_id += 1
        peer = writer.get_extra_info("peername")
        try:
            await self.serve(cid, reader, writer)
        except (asyncio.IncompleteReadError, ConnectionError) as e:
            log(f"#{cid} from {peer[0]}:{peer[1]}: the page's connection ended ({type(e).__name__})")
        except Exception as e:  # noqa: BLE001
            log(f"#{cid}: error {type(e).__name__}: {e}")
        finally:
            writer.close()

    async def http_error(self, writer, cid, status, reason):
        log(f"#{cid} refused: {status} {reason}")
        body = reason.encode()
        writer.write(f"HTTP/1.1 {status}\r\nContent-Type: text/plain\r\nContent-Length: {len(body)}\r\nConnection: close\r\n\r\n".encode() + body)
        await writer.drain()

    async def serve(self, cid, reader, writer):
        head = await asyncio.wait_for(reader.readuntil(b"\r\n\r\n"), 10)
        lines = head.decode("latin-1").split("\r\n")
        method, target, _ = (lines[0].split(" ") + ["", "", ""])[:3]
        hdr = {}
        for l in lines[1:]:
            if ":" in l:
                k, v = l.split(":", 1)
                hdr[k.strip().lower()] = v.strip()
        if method != "GET" or hdr.get("upgrade", "").lower() != "websocket" or "sec-websocket-key" not in hdr:
            return await self.http_error(writer, cid, "400 Bad Request", "this is a WebSocket bridge (tools/ws-bridge)")
        origin = hdr.get("origin")
        if not origin_allowed(origin, self.args.origin):
            return await self.http_error(writer, cid, "403 Forbidden", f"origin {origin} is not allowed (--origin)")
        q = urllib.parse.parse_qs(urllib.parse.urlsplit(target).query)
        if "listen" in q or "accept" in q:
            if not self.args.to_page:
                return await self.http_error(writer, cid, "403 Forbidden", "reverse mode is off (start the bridge with --listen <tcp port> --to-page)")
            if "listen" in q:
                return await self.serve_control(cid, reader, writer, hdr, origin, (q.get("listen") or [""])[0])
            return await self.serve_accept(cid, reader, writer, hdr, (q.get("accept") or [""])[0])
        host = (q.get("host") or [""])[0]
        try:
            port = int((q.get("port") or [""])[0])
            if not 1 <= port <= 65535:
                raise ValueError
        except ValueError:
            return await self.http_error(writer, cid, "400 Bad Request", "want ?host=<address>&port=<1..65535>")
        addr, why = target_allowed(host, self.args.allow_lan)
        if addr is None:
            return await self.http_error(writer, cid, "403 Forbidden", why)
        t0 = time.monotonic()
        try:
            tr, tw = await asyncio.wait_for(asyncio.open_connection(addr, port), 21)
        except (OSError, asyncio.TimeoutError) as e:
            return await self.http_error(writer, cid, "502 Bad Gateway", f"cannot connect to {addr}:{port}: {e or type(e).__name__}")
        sock = tw.get_extra_info("socket")
        if sock is not None:
            import socket
            sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        await handshake(writer, hdr)
        log(f"#{cid} open: page {origin or '(no origin)'} -> tcp {addr}:{port} (connected in {(time.monotonic() - t0) * 1000:.0f} ms)")
        up = TerrariaTrace(cid, "page->server") if self.args.trace_terraria else None
        down = TerrariaTrace(cid, "server->page") if self.args.trace_terraria else None
        await splice(cid, reader, writer, tr, tw, up, down, t0)

    # -------------------------------------------------------------------------------------------- reverse mode
    async def serve_control(self, cid, reader, writer, hdr, origin, listen):
        """The page's server Worker registers (?listen=<its port>): connections to --listen go to it."""
        await handshake(writer, hdr)
        lock = asyncio.Lock()
        old = self.page
        self.page = {"cid": cid, "writer": writer, "lock": lock, "listen": listen}
        log(f"#{cid} page {origin or '(no origin)'} hosts (its server's port {listen}): TCP connections to 127.0.0.1:{self.args.listen} go to it"
            + (f" (replaces #{old['cid']})" if old else ""))
        try:
            while True:
                op, _fin, data = await read_frame(reader)
                if op == 0x8:
                    async with lock:
                        writer.write(frame(0x8, data[:2] if len(data) >= 2 else struct.pack(">H", 1000)))
                        await writer.drain()
                    return
                if op == 0x9:
                    async with lock:
                        writer.write(frame(0xA, data))
                        await writer.drain()
        finally:
            if self.page and self.page["cid"] == cid:
                self.page = None
                log(f"#{cid} the hosting page left (its control connection closed): TCP connections to 127.0.0.1:{self.args.listen} are refused")

    async def serve_accept(self, cid, reader, writer, hdr, conn):
        """The page takes TCP connection `conn` (?accept=<id>): the WebSocket and the TCP connection are spliced."""
        try:
            w = self.waiting.pop(int(conn))
        except (ValueError, KeyError):
            return await self.http_error(writer, cid, "404 Not Found", f"no TCP connection {conn!r} is waiting")
        await handshake(writer, hdr)
        w["taken"].set_result(True)
        log(f"#{w['cid']} the page took the connection from {w['peer']} (WebSocket #{cid})")
        up = TerrariaTrace(w["cid"], "page-server->client", to_server=False) if self.args.trace_terraria else None
        down = TerrariaTrace(w["cid"], "client->page-server", to_server=True) if self.args.trace_terraria else None
        try:
            await splice(w["cid"], reader, writer, w["tr"], w["tw"], up, down, w["t0"], ws_name="page server", tcp_name="client")
        finally:
            w["done"].set_result(True)

    async def tcp_incoming(self, tr, tw):
        """A TCP connection to --listen: handed to the registered page, or closed."""
        cid = self.next_id
        self.next_id += 1
        p = tw.get_extra_info("peername")
        peer = f"{p[0]}:{p[1]}" if p else "?"
        sock = tw.get_extra_info("socket")
        if sock is not None:
            import socket
            sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        page = self.page
        if page is None:
            log(f"#{cid} tcp from {peer} to 127.0.0.1:{self.args.listen}: no page hosts now; closed")
            tw.close()
            return
        loop = asyncio.get_running_loop()
        w = {"cid": cid, "peer": peer, "tr": tr, "tw": tw, "t0": time.monotonic(), "taken": loop.create_future(), "done": loop.create_future()}
        self.waiting[cid] = w
        log(f"#{cid} tcp from {peer} to 127.0.0.1:{self.args.listen}: offered to the hosting page #{page['cid']}")
        try:
            async with page["lock"]:
                page["writer"].write(frame(0x1, ('{"conn": %d, "peer": "%s"}' % (cid, peer)).encode()))
                await page["writer"].drain()
            await asyncio.wait_for(asyncio.shield(w["taken"]), 10)
        except (asyncio.TimeoutError, ConnectionError) as e:
            self.waiting.pop(cid, None)
            log(f"#{cid} the page did not take the connection from {peer} ({type(e).__name__}); closed")
            tw.close()
            return
        await w["done"]


async def main():
    global LOG_FILE
    ap = argparse.ArgumentParser(description="WebSocket <-> TCP bridge for the Terraria web port (127.0.0.1 only).")
    ap.add_argument("--port", type=int, default=7780, help="WebSocket port on 127.0.0.1 (default 7780)")
    ap.add_argument("--allow-lan", action="store_true", help="also allow private LAN addresses as targets")
    ap.add_argument("--origin", action="append", default=[], help="an extra page origin to accept (repeatable)")
    ap.add_argument("--trace-terraria", action="store_true", help="log notable Terraria messages")
    ap.add_argument("--log", help="also append the log to this file")
    ap.add_argument("--listen", type=int, help="reverse mode: also accept TCP connections on 127.0.0.1:<port> ...")
    ap.add_argument("--to-page", action="store_true", help="... and hand them to the page that hosts (Host & Play in the browser)")
    args = ap.parse_args()
    if bool(args.listen) != args.to_page:
        ap.error("--listen <tcp port> and --to-page go together")
    for stream in (sys.stdout, sys.stderr):
        try:
            stream.reconfigure(encoding="utf-8", errors="replace")
        except (AttributeError, ValueError):
            pass
    LOG_FILE = args.log
    bridge = Bridge(args)
    server = await asyncio.start_server(bridge.handle, "127.0.0.1", args.port)
    log(f"listening on ws://127.0.0.1:{args.port} (pid {os.getpid()}); targets: loopback{' + LAN' if args.allow_lan else ' only'}")
    if args.to_page:
        rev = await asyncio.start_server(bridge.tcp_incoming, "127.0.0.1", args.listen)
        log(f"reverse mode: listening on tcp 127.0.0.1:{args.listen} for the page that hosts (it registers with ws://127.0.0.1:{args.port}/?listen=<port>)")
        async with rev:
            async with server:
                await server.serve_forever()
    async with server:
        await server.serve_forever()


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        sys.exit(0)

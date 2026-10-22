// Multiplayer connections of game.wasm (HOST_ABI.md "Network"; port/crates/terraria-host/src/sys/net.rs
// `wasm_net`). A connection of the game is one of:
// - a WebSocket to a local bridge (tools/ws-bridge), which opens the TCP connection to the address the game asked for
//   and copies the bytes both ways ("Join via IP": the game still types an address and a port; the page only knows
//   where the bridge is);
// - a MessagePort pair to the dedicated server of this page (Host & Play: the server runs in a second Worker,
//   server-worker.js): a connection of the hosting client to 127.0.0.1:<the server's port> goes there, through the
//   page (main.js relays the port), not through the bridge;
// - on the server Worker: a connection a listener of the game accepted, from the hosting client (a MessagePort) or
//   from outside through the bridge in reverse mode (a WebSocket the page opened for one TCP connection).
// Above the transport everything is the same: a byte stream with a state, polled by the module.
//
// Every import is synchronous and never blocks: the module polls `net_status` / `net_recv` / `net_accept` and gives
// the frame back in between (a fiber suspend, or a parked state machine), so the events arrive between frames.
//
// Wire contract with the bridge: `<bridge>/?host=<address>&port=<n>`; the bridge connects the TCP side BEFORE it
// answers the WebSocket handshake, so "open" means "connected" and a refused connection is a failed handshake.
// Binary messages both ways; a close from the bridge is the end of the TCP stream. Reverse mode: `?listen=<port>`
// (the control connection of a hosting page: text messages {"conn": id, "peer": "addr:port"}) and `?accept=<id>`.
//
// MessagePort contract (in-page): binary messages are bytes; {t: 'open'} (the server's listener took it),
// {t: 'refused'} (no listener on that port, or the server ended), {t: 'close'} (end of stream).

export const NET_ERRNO = Object.freeze({ EADDRINUSE: 3, EAGAIN: 6, EBADF: 8, EFAULT: 21, EINVAL: 28, ECONNREFUSED: 14, ECONNRESET: 15, ENOSYS: 52, EPIPE: 64 });
const { EADDRINUSE, EAGAIN, EBADF, EFAULT, EINVAL, ECONNREFUSED, ECONNRESET, ENOSYS, EPIPE } = NET_ERRNO;

/** The default bridge address (tools/ws-bridge's default). */
export const DEFAULT_BRIDGE = 'ws://127.0.0.1:7780';

/** `ws://` / `wss://` URL of the bridge from what the player typed ("127.0.0.1:7780" is accepted too); null if unusable. */
export function normalizeBridge(text) {
  let t = String(text ?? '').trim();
  if (!t) return null;
  if (/^[a-z][a-z0-9+.-]*:\/\//i.test(t) && !/^wss?:\/\//i.test(t)) return null;
  if (!/^wss?:\/\//i.test(t)) t = 'ws://' + t;
  try {
    const u = new URL(t);
    if (u.protocol !== 'ws:' && u.protocol !== 'wss:') return null;
    return `${u.protocol}//${u.host}`;
  } catch {
    return null;
  }
}

/** The bridge URL for one connection. */
export function connectUrl(bridge, host, port) {
  return `${bridge}/?host=${encodeURIComponent(host)}&port=${port}`;
}

/** A loopback address as the game types it. */
export function isLoopback(host) {
  const h = String(host).trim().replace(/^\[|\]$/g, '').toLowerCase();
  return h === 'localhost' || h === '::1' || /^127\.\d{1,3}\.\d{1,3}\.\d{1,3}$/.test(h);
}

/** The port a server started with these launch parameters listens on (`-port n`, else Terraria's 7777). */
export function serverPort(args) {
  const m = /(?:^|\s)-port\s+"?(\d{1,5})"?/i.exec(String(args ?? ''));
  const p = m ? Number(m[1]) : 7777;
  return p >= 1 && p <= 65535 ? p : 7777;
}

/**
 * @param {object} o
 * @param {() => string|null} o.bridge  current bridge URL (normalizeBridge form), or null: connecting fails
 * @param {(level: number, text: string) => void} [o.log]
 * @param {typeof WebSocket} [o.WebSocket]  (tests pass a fake)
 * @param {(host: string, port: number) => ((port1: MessagePort) => void) | null} [o.local]  the client Worker: when
 *   a connection to (host, port) goes to this page's own server, a function that hands the far end of a fresh
 *   MessageChannel to the page (otherwise null: the bridge)
 * @param {{ onListen?: (port: number) => void, onUnlisten?: (port: number) => void }} [o.listeners]  the server Worker
 * @param {{ start?: (args: string) => number, status?: (id: number) => number, kill?: (id: number) => number }} [o.servers]
 *   the client Worker: the page's dedicated server (Host & Play)
 * @param {typeof MessageChannel} [o.MessageChannel]
 */
export function createNet({ bridge, log = () => {}, WebSocket: WS = globalThis.WebSocket, local = () => null, listeners: lst = {}, servers = {}, MessageChannel: MC = globalThis.MessageChannel }) {
  let memory = null;
  const dec = new TextDecoder();
  const enc = new TextEncoder();
  const conns = new Map(); // handle -> { state: 0|1|2|-errno, chunks: Uint8Array[], queued, sent, received, label, peer, tx }
  const listening = new Map(); // listener handle -> { port, backlog: handle[] }
  let next = 1;
  const stats = { opened: 0, failed: 0, closed: 0, sent: 0, received: 0, accepted: 0, local: 0 };
  const range = (ptr, len) => {
    ptr >>>= 0;
    len >>>= 0;
    if (!memory || ptr + len > memory.buffer.byteLength) return null;
    return new Uint8Array(memory.buffer, ptr, len);
  };

  function newConn(label, state = 0, peer = '') {
    const h = next++;
    const c = { state, chunks: [], queued: 0, sent: 0, received: 0, label, peer, t0: performance.now(), tx: null };
    conns.set(h, c);
    return [h, c];
  }
  function received(c, bytes) {
    if (bytes.length === 0) return;
    c.chunks.push(bytes);
    c.queued += bytes.length;
    c.received += bytes.length;
    stats.received += bytes.length;
  }
  const toBytes = (d) => (d instanceof ArrayBuffer ? new Uint8Array(d) : ArrayBuffer.isView(d) ? new Uint8Array(d.buffer, d.byteOffset, d.byteLength) : enc.encode(String(d)));

  /** A WebSocket under connection `c` (`open`: it is open already, an accepted reverse connection). */
  function overWebSocket(h, c, ws, open) {
    ws.binaryType = 'arraybuffer';
    c.tx = {
      send: (v) => ws.send(v.slice()), // copied: the module's memory may change before the socket reads it
      close: () => {
        if (ws.readyState === undefined || ws.readyState <= 1) {
          try {
            ws.close(1000);
          } catch {}
        }
        ws.onclose = ws.onmessage = ws.onopen = null;
      },
    };
    ws.onopen = () => {
      if (c.state !== 0) return;
      c.state = 1;
      stats.opened++;
      log(2, `net: connection ${h} to ${c.label} is open (${(performance.now() - c.t0).toFixed(0)} ms)`);
    };
    if (open) c.state = 1;
    ws.onmessage = (ev) => received(c, toBytes(ev.data));
    ws.onerror = () => {}; // the close event follows and carries what is known
    ws.onclose = (ev) => {
      if (c.state === 0) {
        c.state = -ECONNREFUSED; // the handshake failed: the bridge could not connect, or is not running
        stats.failed++;
        log(3, `net: connection ${h} to ${c.label} failed (the bridge refused or is not running; WebSocket close ${ev.code}${ev.reason ? ` "${ev.reason}"` : ''})`);
      } else if (c.state === 1) {
        c.state = ev.code === 1000 || ev.code === 1005 ? 2 : -ECONNRESET;
        stats.closed++;
        log(2, `net: connection ${h} to ${c.label} closed by the other side (WebSocket close ${ev.code}${ev.reason ? ` "${ev.reason}"` : ''}); ${c.sent} bytes sent, ${c.received} received`);
      }
    };
  }

  /** A MessagePort under connection `c` (in-page: the hosting client and its server). */
  function overPort(h, c, port) {
    c.tx = {
      send: (v) => {
        const b = v.slice().buffer;
        port.postMessage(b, [b]);
      },
      close: () => {
        try {
          port.postMessage({ t: 'close' });
        } catch {}
        port.onmessage = null;
        port.close();
      },
    };
    port.onmessage = (ev) => {
      const d = ev.data;
      if (d instanceof ArrayBuffer || ArrayBuffer.isView(d)) return received(c, toBytes(d));
      if (!d || typeof d !== 'object') return;
      if (d.t === 'open' && c.state === 0) {
        c.state = 1;
        stats.opened++;
        log(2, `net: connection ${h} to ${c.label} is open (${(performance.now() - c.t0).toFixed(0)} ms)`);
      } else if (d.t === 'refused' && c.state === 0) {
        c.state = -ECONNREFUSED;
        stats.failed++;
        log(3, `net: connection ${h} to ${c.label} refused${d.why ? ` (${d.why})` : ''}`);
      } else if (d.t === 'close' && (c.state === 0 || c.state === 1)) {
        c.state = c.state === 1 ? 2 : -ECONNRESET;
        stats.closed++;
        log(2, `net: connection ${h} to ${c.label} closed by the other side; ${c.sent} bytes sent, ${c.received} received`);
      }
    };
    port.start?.();
  }

  function net_connect(hostPtr, hostLen, port) {
    const hb = range(hostPtr, hostLen);
    if (!hb) return -EFAULT;
    const host = dec.decode(hb.slice());
    if (!(port >= 1 && port <= 65535) || !host) return -EINVAL;
    const label = `${host}:${port}`;
    // the page's own server (Host & Play): through the page, not the bridge
    const toLocal = local(host, port);
    if (toLocal && typeof MC === 'function') {
      const [h, c] = newConn(`${label} (this page's server)`);
      const ch = new MC();
      overPort(h, c, ch.port1);
      stats.local++;
      log(2, `net: connection ${h}: ${label} goes to this page's own server`);
      toLocal(ch.port2);
      return h;
    }
    const b = bridge();
    if (!b || typeof WS !== 'function') {
      log(4, `net: no WebSocket bridge is set, cannot connect to ${label} (start tools/ws-bridge and set ?bridge=)`);
      return -ENOSYS;
    }
    const [h, c] = newConn(label);
    let ws;
    try {
      ws = new WS(connectUrl(b, host, port));
    } catch (e) {
      log(4, `net: connection ${h} to ${label} through ${b}: ${e.message ?? e}`);
      c.state = -ECONNREFUSED;
      stats.failed++;
      return h;
    }
    overWebSocket(h, c, ws, false);
    log(2, `net: connection ${h}: ${label} through the bridge ${b}`);
    return h;
  }

  function net_status(h) {
    const c = conns.get(h);
    return c ? c.state : -EBADF;
  }

  function net_send(h, ptr, len) {
    const c = conns.get(h);
    if (!c) return -EBADF;
    if (c.state === 0) return -EAGAIN;
    if (c.state === 2) return -EPIPE;
    if (c.state < 0) return c.state === -ECONNREFUSED ? -ECONNREFUSED : -ECONNRESET;
    const v = range(ptr, len);
    if (!v) return -EFAULT;
    if (len === 0) return 0;
    c.tx.send(v);
    c.sent += v.length;
    stats.sent += v.length;
    return v.length;
  }

  function net_recv(h, ptr, cap, peek) {
    const c = conns.get(h);
    if (!c) return -EBADF;
    if (c.queued === 0) {
      if (c.state === 0 || c.state === 1) return -EAGAIN;
      if (c.state === 2) return 0;
      return c.state;
    }
    const out = range(ptr, cap);
    if (!out) return -EFAULT;
    let n = 0;
    let i = 0;
    while (n < out.length && i < c.chunks.length) {
      const ch = c.chunks[i];
      const k = Math.min(ch.length, out.length - n);
      out.set(ch.subarray(0, k), n);
      n += k;
      if (!peek) {
        if (k === ch.length) c.chunks.shift();
        else c.chunks[i] = ch.subarray(k);
      } else i++;
    }
    if (!peek) c.queued -= n;
    return n;
  }

  function net_close(h) {
    const c = conns.get(h);
    if (!c) return -EBADF;
    conns.delete(h);
    if (c.tx) {
      if (c.state === 0 || c.state === 1) log(2, `net: connection ${h} to ${c.label} closed by the game; ${c.sent} bytes sent, ${c.received} received`);
      c.tx.close();
    }
    return 0;
  }

  // ---------------------------------------------------------------- listeners (the server Worker)
  function net_listen(port) {
    if (!lst.onListen) return -ENOSYS; // only the server Worker listens
    if (!(port >= 0 && port <= 65535)) return -EINVAL;
    for (const l of listening.values()) if (l.port === port) return -EADDRINUSE;
    const h = next++;
    listening.set(h, { port, backlog: [] });
    log(2, `net: listening on port ${port} of the page`);
    lst.onListen(port);
    return h;
  }
  function net_accept(l) {
    const L = listening.get(l);
    if (!L) return -EBADF;
    return L.backlog.length ? L.backlog.shift() : -EAGAIN;
  }
  function net_unlisten(l) {
    const L = listening.get(l);
    if (!L) return -EBADF;
    listening.delete(l);
    for (const h of L.backlog) net_close(h);
    log(2, `net: stopped listening on port ${L.port}`);
    lst.onUnlisten?.(L.port);
    return 0;
  }
  function net_peer(h, outPtr, cap) {
    const c = conns.get(h);
    if (!c) return -EBADF;
    const b = enc.encode(c.peer || '');
    const out = range(outPtr, Math.min(cap >>> 0, b.length));
    if (!out) return -EFAULT;
    out.set(b.subarray(0, out.length));
    return out.length;
  }
  const listenerOn = (port) => [...listening.values()].find((l) => l.port === port) ?? null;

  /** Server Worker: a connection for `port` from the hosting client of this page (the far end of its channel). */
  function acceptPort(port, mport, peer = '') {
    const L = listenerOn(port);
    if (!L) {
      try {
        mport.postMessage({ t: 'refused', why: `nothing listens on port ${port} in this page` });
        mport.close();
      } catch {}
      return 0;
    }
    const [h, c] = newConn(`the hosting client (${peer || 'in the page'})`, 1, peer);
    overPort(h, c, mport);
    mport.postMessage({ t: 'open' });
    L.backlog.push(h);
    stats.accepted++;
    log(2, `net: connection ${h} accepted on port ${port} from this page's own client`);
    return h;
  }
  /** Server Worker: an open WebSocket from the bridge's reverse mode for one TCP connection of `peer`. */
  function acceptWebSocket(port, ws, peer = '') {
    const L = listenerOn(port);
    if (!L) {
      try {
        ws.close(1000);
      } catch {}
      return 0;
    }
    const [h, c] = newConn(`${peer || 'a client'} (through the bridge)`, 1, peer);
    overWebSocket(h, c, ws, true);
    L.backlog.push(h);
    stats.accepted++;
    log(2, `net: connection ${h} accepted on port ${port} from ${peer || '?'} through the bridge`);
    return h;
  }

  // ---------------------------------------------------------------- the page's server (the client Worker)
  function server_start(ptr, len) {
    if (!servers.start) return -ENOSYS;
    const b = range(ptr, len);
    if (!b) return -EFAULT;
    return servers.start(dec.decode(b.slice()));
  }
  const server_status = (id) => (servers.status ? servers.status(id) : -ENOSYS);
  const server_kill = (id) => (servers.kill ? servers.kill(id) : -ENOSYS);

  return {
    imports: { net_connect, net_status, net_send, net_recv, net_close, net_listen, net_accept, net_unlisten, net_peer, server_start, server_status, server_kill },
    setMemory(m) { memory = m; },
    acceptPort,
    acceptWebSocket,
    get live() { return conns.size; },
    get listening() { return [...listening.values()].map((l) => l.port); },
    stats,
  };
}

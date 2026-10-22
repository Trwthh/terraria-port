// lib/net-bridge.js: the `tw.net_*` imports over a fake WebSocket (the real bridge: tools/ws-bridge/selftest.mjs;
// the whole path: test/run-game-headless.mjs with test/actions/mp_join.txt).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createNet, normalizeBridge, connectUrl, NET_ERRNO } from '../lib/net-bridge.js';

class FakeWS {
  static all = [];
  constructor(url) {
    this.url = url;
    this.sent = [];
    this.closed = null;
    FakeWS.all.push(this);
  }
  send(b) { this.sent.push(Uint8Array.from(b)); }
  close(code) { this.closed = code; }
  // test side
  open() { this.onopen?.({}); }
  recv(bytes) { this.onmessage?.({ data: Uint8Array.from(bytes).buffer }); }
  end(code, reason = '') { this.onclose?.({ code, reason }); }
}

function setup(bridge = 'ws://127.0.0.1:7780') {
  FakeWS.all = [];
  const memory = new WebAssembly.Memory({ initial: 1 });
  const logs = [];
  const net = createNet({ bridge: () => bridge, log: (l, t) => logs.push([l, t]), WebSocket: FakeWS });
  net.setMemory(memory);
  const mem = new Uint8Array(memory.buffer);
  const put = (at, s) => {
    const b = typeof s === 'string' ? new TextEncoder().encode(s) : Uint8Array.from(s);
    mem.set(b, at);
    return b.length;
  };
  return { net, t: net.imports, mem, put, logs };
}

test('bridge addresses', () => {
  assert.equal(normalizeBridge('127.0.0.1:7780'), 'ws://127.0.0.1:7780');
  assert.equal(normalizeBridge(' ws://localhost:9000/ '), 'ws://localhost:9000');
  assert.equal(normalizeBridge('wss://example.test:443'), 'wss://example.test');
  assert.equal(normalizeBridge('http://127.0.0.1:1'), null);
  assert.equal(normalizeBridge(''), null);
  assert.equal(connectUrl('ws://127.0.0.1:7780', '127.0.0.1', 47800), 'ws://127.0.0.1:7780/?host=127.0.0.1&port=47800');
  assert.equal(connectUrl('ws://b:1', '::1', 7777), 'ws://b:1/?host=%3A%3A1&port=7777');
});

test('connect, send, receive in pieces, peek, end of stream', () => {
  const { t, mem, put } = setup();
  const n = put(0, '127.0.0.1');
  const h = t.net_connect(0, n, 47800);
  assert.ok(h >= 1);
  const ws = FakeWS.all[0];
  assert.equal(ws.url, 'ws://127.0.0.1:7780/?host=127.0.0.1&port=47800');
  assert.equal(t.net_status(h), 0);
  assert.equal(t.net_send(h, 100, 3), -NET_ERRNO.EAGAIN, 'not open yet');
  assert.equal(t.net_recv(h, 200, 16, 0), -NET_ERRNO.EAGAIN);
  ws.open();
  assert.equal(t.net_status(h), 1);
  put(100, [1, 2, 3]);
  assert.equal(t.net_send(h, 100, 3), 3);
  mem[100] = 9; // the module's memory changes after the call: the sent bytes were copied
  assert.deepEqual([...ws.sent[0]], [1, 2, 3]);
  ws.recv([10, 11, 12]);
  ws.recv([13, 14]);
  assert.equal(t.net_recv(h, 200, 1, 1), 1, 'peek one');
  assert.equal(t.net_recv(h, 200, 64, 1), 5, 'peek all, across messages');
  assert.deepEqual([...mem.subarray(200, 205)], [10, 11, 12, 13, 14]);
  assert.equal(t.net_recv(h, 300, 2, 0), 2);
  assert.deepEqual([...mem.subarray(300, 302)], [10, 11]);
  assert.equal(t.net_recv(h, 300, 64, 0), 3);
  assert.deepEqual([...mem.subarray(300, 303)], [12, 13, 14]);
  assert.equal(t.net_recv(h, 300, 64, 0), -NET_ERRNO.EAGAIN);
  ws.recv([7]);
  ws.end(1000);
  assert.equal(t.net_status(h), 2);
  assert.equal(t.net_recv(h, 300, 64, 0), 1, 'bytes that came before the close are still read');
  assert.equal(t.net_recv(h, 300, 64, 0), 0, 'then the end of the stream');
  assert.equal(t.net_send(h, 100, 3), -NET_ERRNO.EPIPE);
  assert.equal(t.net_close(h), 0);
  assert.equal(t.net_status(h), -NET_ERRNO.EBADF);
});

test('a refused connection and a reset one', () => {
  const { t, put, net } = setup();
  const n = put(0, '127.0.0.1');
  const h = t.net_connect(0, n, 1);
  FakeWS.all[0].end(1006);
  assert.equal(t.net_status(h), -NET_ERRNO.ECONNREFUSED);
  assert.equal(net.stats.failed, 1);
  const h2 = t.net_connect(0, n, 2);
  FakeWS.all[1].open();
  FakeWS.all[1].end(1006);
  assert.equal(t.net_status(h2), -NET_ERRNO.ECONNRESET);
  assert.equal(t.net_recv(h2, 300, 4, 0), -NET_ERRNO.ECONNRESET);
  // the game closes an open connection: the WebSocket is closed normally
  const h3 = t.net_connect(0, n, 3);
  FakeWS.all[2].open();
  t.net_close(h3);
  assert.equal(FakeWS.all[2].closed, 1000);
  assert.equal(net.live, 2);
});

test('no bridge, bad arguments', () => {
  const { t, put } = setup(null);
  const n = put(0, '127.0.0.1');
  assert.equal(t.net_connect(0, n, 7777), -NET_ERRNO.ENOSYS);
  const s = setup();
  assert.equal(s.t.net_connect(0, s.put(0, '127.0.0.1'), 0), -NET_ERRNO.EINVAL);
  assert.equal(s.t.net_connect(0x7fffff00, 1000, 7777), -NET_ERRNO.EFAULT);
  assert.equal(s.t.net_status(99), -NET_ERRNO.EBADF);
});

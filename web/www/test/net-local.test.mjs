// lib/net-bridge.js, Host & Play: the hosting client's connection to the page's own server (a MessageChannel the
// page relays), the server's listener (net_listen / net_accept / net_peer), a connection through the bridge's reverse
// mode (an open WebSocket handed to the listener), and the server_* imports of the game Worker.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createNet, isLoopback, serverPort, NET_ERRNO } from '../lib/net-bridge.js';

const tick = () => new Promise((r) => setTimeout(r, 5));
const until = async (f) => {
  for (let i = 0; i < 200; i++) {
    if (f()) return true;
    await tick();
  }
  return false;
};

function side(opts) {
  const memory = new WebAssembly.Memory({ initial: 1 });
  const net = createNet({ bridge: () => null, log: () => {}, ...opts });
  net.setMemory(memory);
  const mem = new Uint8Array(memory.buffer);
  const put = (at, s) => {
    const b = typeof s === 'string' ? new TextEncoder().encode(s) : Uint8Array.from(s);
    mem.set(b, at);
    return b.length;
  };
  return { net, t: net.imports, mem, put };
}

test('addresses and the server port of Host & Play', () => {
  assert.ok(isLoopback('127.0.0.1') && isLoopback('localhost') && isLoopback('::1') && isLoopback('[::1]') && isLoopback('127.1.2.3'));
  assert.ok(!isLoopback('192.168.1.2') && !isLoopback('example.com'));
  assert.equal(serverPort('-autoshutdown -password "" -lang 1 -world "/saves/Worlds/a.wld"'), 7777);
  assert.equal(serverPort('-world x -port 47800'), 47800);
  assert.equal(serverPort('-port "7790" -x'), 7790);
});

test('the hosting client connects to the page server through a MessageChannel, held until the server listens', async () => {
  // the page: holds the channel until the server listens (main.js localConnect / startServer)
  const held = [];
  let server = null;
  let listened = null;
  const relay = (port, channel) => {
    if (server && listened === port) server.net.acceptPort(port, channel, '');
    else held.push({ port, channel });
  };
  const client = side({ local: (host, port) => (isLoopback(host) && port === 7777 ? (ch) => relay(port, ch) : null) });
  const h = client.t.net_connect(0, client.put(0, '127.0.0.1'), 7777);
  assert.ok(h >= 1);
  assert.equal(client.net.stats.local, 1);
  await tick();
  assert.equal(client.t.net_status(h), 0, 'connecting while the server is not listening yet');
  // the server starts and listens
  server = side({ listeners: { onListen: (p) => (listened = p) } });
  const l = server.t.net_listen(7777);
  assert.ok(l >= 1);
  assert.equal(server.t.net_listen(7777), -NET_ERRNO.EADDRINUSE);
  assert.equal(server.t.net_accept(l), -NET_ERRNO.EAGAIN);
  for (const x of held.splice(0)) server.net.acceptPort(x.port, x.channel, '');
  assert.ok(await until(() => client.t.net_status(h) === 1), 'open once the server took it');
  const s = server.t.net_accept(l);
  assert.ok(s >= 1 && server.t.net_status(s) === 1);
  assert.equal(server.t.net_peer(s, 100, 64), 0, 'the hosting client has no address of its own');
  // bytes both ways, in order, copied out of the sender's memory
  client.put(200, [1, 2, 3, 4]);
  assert.equal(client.t.net_send(h, 200, 4), 4);
  client.mem[200] = 99;
  server.put(300, [9, 8]);
  assert.equal(server.t.net_send(s, 300, 2), 2);
  assert.ok(await until(() => server.t.net_recv(s, 400, 16, 1) === 4));
  assert.equal(server.t.net_recv(s, 400, 16, 0), 4);
  assert.deepEqual([...server.mem.subarray(400, 404)], [1, 2, 3, 4]);
  assert.ok(await until(() => client.t.net_recv(h, 500, 16, 0) === 2));
  assert.deepEqual([...client.mem.subarray(500, 502)], [9, 8]);
  // the client leaves: the server reads the end of the stream
  client.t.net_close(h);
  assert.ok(await until(() => server.t.net_status(s) === 2));
  assert.equal(server.t.net_recv(s, 400, 16, 0), 0);
  server.t.net_close(s);
  assert.equal(server.t.net_unlisten(l), 0);
});

test('nothing listens: refused; the game Worker cannot listen; a bridge connection is accepted with its peer', async () => {
  const server = side({ listeners: { onListen: () => {} } });
  const { port1, port2 } = new MessageChannel();
  const client = side({ local: () => (ch) => server.net.acceptPort(7777, ch, '') });
  const h = client.t.net_connect(0, client.put(0, 'localhost'), 7777);
  assert.ok(await until(() => client.t.net_status(h) === -NET_ERRNO.ECONNREFUSED), 'refused: no listener on the port');
  port1.close();
  port2.close();
  client.t.net_close(h);
  assert.equal(client.t.net_listen(7777), -NET_ERRNO.ENOSYS, 'only the server Worker listens');
  // reverse mode: an open WebSocket for one TCP connection
  const l = server.t.net_listen(7777);
  const ws = { sent: [], readyState: 1, send(b) { this.sent.push(Uint8Array.from(b)); }, close(c) { this.closed = c; } };
  server.net.acceptWebSocket(7777, ws, '127.0.0.1:50123');
  const s = server.t.net_accept(l);
  assert.equal(server.t.net_status(s), 1);
  const n = server.t.net_peer(s, 100, 64);
  assert.equal(new TextDecoder().decode(server.mem.slice(100, 100 + n)), '127.0.0.1:50123');
  ws.onmessage({ data: Uint8Array.from([5, 6]).buffer });
  assert.equal(server.t.net_recv(s, 200, 8, 0), 2);
  server.put(300, [7]);
  server.t.net_send(s, 300, 1);
  assert.deepEqual([...ws.sent[0]], [7]);
  ws.onclose({ code: 1000 });
  assert.equal(server.t.net_status(s), 2);
});

test('server_start / server_status / server_kill go to the page', () => {
  const calls = [];
  const c = side({ servers: { start: (a) => (calls.push(['start', a]), 1), status: (id) => (id === 1 ? 0 : -NET_ERRNO.EBADF), kill: (id) => (calls.push(['kill', id]), 0) } });
  const n = c.put(0, '-autoshutdown -world "/saves/Worlds/a.wld"');
  assert.equal(c.t.server_start(0, n), 1);
  assert.equal(c.t.server_status(1), 0);
  assert.equal(c.t.server_kill(1), 0);
  assert.deepEqual(calls, [['start', '-autoshutdown -world "/saves/Worlds/a.wld"'], ['kill', 1]]);
  assert.equal(side({}).t.server_start(0, 1), -NET_ERRNO.ENOSYS);
});

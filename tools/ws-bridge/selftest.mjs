// Self-test of ws_bridge.py with Node's own WebSocket client and a TCP echo server, all on 127.0.0.1:
//
//   node tools/ws-bridge/selftest.mjs
//
// 1 MB round trip in pieces; the server closing (the page sees a normal close); a refused port (failed handshake);
// a public target and a foreign page origin refused. The bridge and the echo server are stopped at the end.
import { spawn } from 'node:child_process';
import net from 'node:net';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import assert from 'node:assert/strict';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const free = () => new Promise((res) => { const s = net.createServer().listen(0, '127.0.0.1', () => { const p = s.address().port; s.close(() => res(p)); }); });

const echo = net.createServer((c) => {
  c.on('data', (d) => {
    if (d.includes('BYE')) c.end();
    else c.write(d);
  });
  c.on('error', () => {});
});
await new Promise((r) => echo.listen(0, '127.0.0.1', r));
const echoPort = echo.address().port;
const bridgePort = await free();
const bridge = spawn('python', [path.join(HERE, 'ws_bridge.py'), '--port', String(bridgePort)], { stdio: ['ignore', 'pipe', 'pipe'], windowsHide: true });
let out = '';
bridge.stdout.on('data', (d) => (out += d));
bridge.stderr.on('data', (d) => (out += d));
const results = [];
try {
  for (let i = 0; i < 100 && !out.includes('listening'); i++) await new Promise((r) => setTimeout(r, 100));
  assert.ok(out.includes('listening'), 'bridge did not start: ' + out);
  const url = (host, port) => `ws://127.0.0.1:${bridgePort}/?host=${encodeURIComponent(host)}&port=${port}`;
  const open = (u, opts) => new Promise((res) => {
    const ws = new WebSocket(u, opts);
    ws.binaryType = 'arraybuffer';
    ws.onopen = () => res({ ws, ok: true });
    ws.onclose = (e) => res({ ws, ok: false, code: e.code });
  });

  // round trip
  const { ws, ok } = await open(url('127.0.0.1', echoPort));
  assert.ok(ok, 'echo connection opened');
  const data = new Uint8Array(1 << 20).map((_, i) => (i * 31 + (i >> 9)) & 255);
  let got = [];
  let n = 0;
  const closed = new Promise((res) => { ws.onclose = (e) => res(e.code); });
  ws.onmessage = (e) => { const b = new Uint8Array(e.data); got.push(b); n += b.length; };
  for (let o = 0; o < data.length; o += 1000) ws.send(data.subarray(o, Math.min(o + 1000, data.length)));
  for (let i = 0; i < 300 && n < data.length; i++) await new Promise((r) => setTimeout(r, 20));
  const all = new Uint8Array(n);
  let o = 0;
  for (const b of got) { all.set(b, o); o += b.length; }
  assert.equal(n, data.length, 'all bytes came back');
  assert.ok(all.every((v, i) => v === data[i]), 'the bytes are the same, in order');
  results.push(`round trip: ${n} bytes in ${got.length} messages`);
  ws.send(new TextEncoder().encode('BYE'));
  const code = await closed;
  assert.equal(code, 1000, 'the server closing is a normal close');
  results.push(`server close -> WebSocket close ${code}`);

  // refused
  const deadPort = await free();
  const r1 = await open(url('127.0.0.1', deadPort));
  assert.ok(!r1.ok, 'refused port fails the handshake');
  results.push(`refused port -> handshake failed (close ${r1.code})`);
  // not allowed targets
  const r2 = await open(url('8.8.8.8', 53));
  assert.ok(!r2.ok);
  const r3 = await open(url('192.168.1.1', 7777));
  assert.ok(!r3.ok);
  const r4 = await open(url('example.com', 80));
  assert.ok(!r4.ok);
  results.push('public / LAN (without --allow-lan) / name targets refused');
  // foreign origin
  const r5 = await open(url('127.0.0.1', echoPort), { headers: { origin: 'https://evil.example' } });
  assert.ok(!r5.ok, 'a foreign origin is refused');
  const r6 = await open(url('127.0.0.1', echoPort), { headers: { origin: 'http://127.0.0.1:8147' } });
  assert.ok(r6.ok, 'the local page origin is accepted');
  r6.ws.close();
  results.push('origin https://evil.example refused, http://127.0.0.1:8147 accepted');
  // reverse mode is off without --listen / --to-page
  const r7 = await open(`ws://127.0.0.1:${bridgePort}/?listen=7777`);
  assert.ok(!r7.ok, 'no reverse registration without --to-page');
  results.push('reverse registration refused without --listen / --to-page');
  await new Promise((r) => setTimeout(r, 200));
  console.log(results.map((l) => 'ok  ' + l).join('\n'));
  console.log('--- bridge log\n' + out.trim());
} catch (e) {
  console.log(results.map((l) => 'ok  ' + l).join('\n'));
  console.log('FAIL ' + e.message + '\n--- bridge log\n' + out.trim());
  process.exitCode = 1;
} finally {
  bridge.kill();
  echo.close();
}

// ---------------------------------------------------------------- reverse mode (--listen <tcp port> --to-page)
// A page that hosts registers (?listen=), a TCP client connects to the bridge's TCP port, the page is told
// ({"conn", "peer"}) and takes it (?accept=<id>); bytes both ways; the TCP client closing closes the WebSocket.
{
  const bport = await free();
  const tport = await free();
  const rb = spawn('python', [path.join(HERE, 'ws_bridge.py'), '--port', String(bport), '--listen', String(tport), '--to-page', '--trace-terraria'], { stdio: ['ignore', 'pipe', 'pipe'], windowsHide: true });
  let rout = '';
  rb.stdout.on('data', (d) => (rout += d));
  rb.stderr.on('data', (d) => (rout += d));
  const res2 = [];
  const tcp = (port) => new Promise((res) => {
    const c = net.connect(port, '127.0.0.1');
    const chunks = [];
    const st = { c, chunks, closed: false };
    c.on('data', (d) => chunks.push(d));
    c.on('close', () => (st.closed = true));
    c.on('error', () => {});
    c.on('connect', () => res(st));
  });
  const until = async (f, ms = 5000) => { for (let i = 0; i < ms / 20 && !f(); i++) await new Promise((r) => setTimeout(r, 20)); return f(); };
  try {
    await until(() => rout.includes('reverse mode: listening'));
    assert.ok(rout.includes('reverse mode: listening'), 'reverse bridge did not start: ' + rout);
    // no page yet: a TCP connection is closed
    const lone = await tcp(tport);
    assert.ok(await until(() => lone.closed), 'a TCP connection without a hosting page is closed');
    res2.push('reverse: TCP connection without a hosting page closed');
    // the page registers
    const ctl = new WebSocket(`ws://127.0.0.1:${bport}/?listen=7777`, { headers: { origin: 'http://127.0.0.1:8147' } });
    const offers = [];
    ctl.onmessage = (e) => offers.push(JSON.parse(e.data));
    assert.ok(await until(() => ctl.readyState === 1), 'control connection open');
    const client = await tcp(tport);
    assert.ok(await until(() => offers.length === 1), 'the page is offered the connection');
    assert.ok(/^127\.0\.0\.1:\d+$/.test(offers[0].peer), 'with the client address ' + offers[0].peer);
    const acc = new WebSocket(`ws://127.0.0.1:${bport}/?accept=${offers[0].conn}`, { headers: { origin: 'http://127.0.0.1:8147' } });
    acc.binaryType = 'arraybuffer';
    const got = [];
    let accClose = null;
    acc.onmessage = (e) => got.push(new Uint8Array(e.data));
    acc.onclose = (e) => (accClose = e.code);
    assert.ok(await until(() => acc.readyState === 1), 'accept connection open');
    // a Terraria ConnectRequest from the client (u16 length, type 1, string "Terraria279")
    const hello = Buffer.from([15, 0, 1, 11, ...Buffer.from('Terraria279')]);
    client.c.write(hello);
    assert.ok(await until(() => got.reduce((n, b) => n + b.length, 0) === hello.length), 'client -> page bytes');
    acc.send(new Uint8Array([5, 0, 3, 0, 0]));
    assert.ok(await until(() => client.chunks.reduce((n, b) => n + b.length, 0) === 5), 'page -> client bytes');
    res2.push(`reverse: page took conn ${offers[0].conn} from ${offers[0].peer}; ${hello.length} bytes up, 5 down`);
    client.c.end();
    assert.ok(await until(() => accClose !== null), 'the client closing closes the page WebSocket');
    assert.equal(accClose, 1000);
    // an unknown accept id is refused
    const bad = await new Promise((res) => { const w = new WebSocket(`ws://127.0.0.1:${bport}/?accept=999`); w.onopen = () => res({ ok: true }); w.onclose = () => res({ ok: false }); });
    assert.ok(!bad.ok, 'unknown accept id refused');
    // the page leaves: TCP refused again
    ctl.close();
    await until(() => rout.includes('the hosting page left'));
    const late = await tcp(tport);
    assert.ok(await until(() => late.closed), 'after the page left, TCP connections are closed');
    assert.ok(rout.includes("ConnectRequest 'Terraria279'"), 'the trace reads the client -> page-server direction');
    res2.push('reverse: client close -> WebSocket close 1000; unknown accept id refused; page left -> TCP closed; trace ok');
    console.log(res2.map((l) => 'ok  ' + l).join('\n'));
    console.log('--- reverse bridge log\n' + rout.trim());
  } catch (e) {
    console.log(res2.map((l) => 'ok  ' + l).join('\n'));
    console.log('FAIL ' + e.message + '\n--- reverse bridge log\n' + rout.trim());
    process.exitCode = 1;
  } finally {
    rb.kill();
  }
}

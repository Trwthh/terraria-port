import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  REC, KIND, MOD, KEY_CODES, codeId, keyCodepoint, encodeKey, encodeText, encodeMouseMove, encodeMouseButton,
  encodeWheel, encodeFocus, encodeResize, encodeGamepadConnect, encodeGamepadAxes, encodeGamepadButtons,
  decode, decodeAll, concat, rebaseTimes, InputQueue, modsOf,
} from '../lib/input-codec.js';

test('key code table: stable ids, no duplicates', () => {
  assert.equal(new Set(KEY_CODES).size, KEY_CODES.length);
  assert.equal(codeId('Unidentified'), 0);
  assert.equal(codeId('KeyA'), 1);
  assert.equal(codeId('KeyZ'), 26);
  assert.equal(codeId('Digit0'), 27);
  assert.equal(codeId('Enter'), 37);
  assert.equal(codeId('Space'), 41);
  assert.equal(codeId('F1'), 54);
  assert.equal(codeId('ArrowUp'), 78);
  assert.equal(codeId('ShiftLeft'), 112);
  assert.equal(codeId('NumpadComma'), 135);
  assert.equal(codeId('NoSuchKey'), 0);
});

test('key record round trip and exact byte layout', () => {
  const r = encodeKey({ down: true, code: 'KeyW', key: 'w', mods: MOD.SHIFT | MOD.CTRL, repeat: true, location: 0, time: 1234.5 });
  assert.equal(r.length, REC);
  const dv = new DataView(r.buffer);
  assert.equal(dv.getUint8(0), KIND.KEY);
  assert.equal(dv.getUint8(1), 1);
  assert.equal(dv.getUint16(2, true), MOD.SHIFT | MOD.CTRL | MOD.REPEAT);
  assert.equal(dv.getUint32(4, true), codeId('KeyW'));
  assert.equal(dv.getFloat64(8, true), 1234.5);
  assert.equal(dv.getUint32(16, true), 'w'.codePointAt(0));
  const d = decode(r);
  assert.equal(d.code, 'KeyW');
  assert.equal(d.down, true);
  assert.equal(d.repeat, true);
  assert.equal(d.key, 0x77);
  const up = decode(encodeKey({ down: false, code: 'Escape', key: 'Escape' }));
  assert.equal(up.down, false);
  assert.equal(up.key, 0, 'named keys carry no code point');
});

test('keyCodepoint: single scalar values only', () => {
  assert.equal(keyCodepoint('a'), 97);
  assert.equal(keyCodepoint('😀'), 0x1f600);
  assert.equal(keyCodepoint('Enter'), 0);
  assert.equal(keyCodepoint(''), 0);
});

test('text: split at code point boundaries, max 16 bytes per record', () => {
  const s = 'héllo wörld, 日本語のテキスト 😀!';
  const recs = encodeText(s, 7);
  assert.equal(recs.length % REC, 0);
  const all = decodeAll(recs);
  assert.ok(all.length > 1);
  for (const r of all) {
    assert.equal(r.kind, KIND.TEXT);
    assert.ok(r.a >= 1 && r.a <= 16);
    assert.equal(r.time, 7);
  }
  assert.equal(all.map((r) => r.text).join(''), s);
  assert.equal(encodeText('').length, 0);
});

test('pointer, wheel, focus, resize, gamepad round trips', () => {
  const recs = concat([
    encodeMouseMove({ x: 10.5, y: 20.25, buttons: 1 }),
    encodeMouseButton({ down: true, button: 2, x: 1, y: 2, buttons: 2, mods: MOD.ALT }),
    encodeWheel({ x: 3, y: 4, dx: -5, dy: 120 }),
    encodeFocus({ focused: false }),
    encodeResize({ pixelWidth: 2560, pixelHeight: 1440, cssWidth: 1280, cssHeight: 720, dpr: 2 }),
    encodeGamepadConnect({ index: 1, connected: true, standard: true, axes: 4, buttons: 17 }),
    encodeGamepadAxes({ index: 1, axes: [0.5, -0.25, 1, -1] }),
    encodeGamepadButtons({ index: 1, pressed: 0x80000001, lt: 0.5, rt: 1 }),
  ]);
  const [mv, mb, wh, fo, rs, gc, ga, gb] = decodeAll(recs);
  assert.deepEqual([mv.x, mv.y, mv.buttons], [10.5, 20.25, 1]);
  assert.deepEqual([mb.down, mb.button, mb.buttons, mb.mods], [true, 2, 2, MOD.ALT]);
  assert.deepEqual([wh.dx, wh.dy], [-5, 120]);
  assert.equal(fo.focused, false);
  assert.deepEqual([rs.pixelWidth, rs.pixelHeight, rs.cssWidth, rs.cssHeight, rs.dpr], [2560, 1440, 1280, 720, 2]);
  assert.deepEqual([gc.index, gc.connected, gc.standard, gc.axes, gc.buttons], [1, true, true, 4, 17]);
  assert.deepEqual(ga.axes, [0.5, -0.25, 1, -1]);
  assert.deepEqual([gb.pressed, gb.lt, gb.rt], [0x80000001, 0.5, 1]);
});

test('modsOf reads DOM event flags', () => {
  const e = { shiftKey: true, ctrlKey: false, altKey: true, metaKey: false, getModifierState: (k) => k === 'CapsLock' };
  assert.equal(modsOf(e), MOD.SHIFT | MOD.ALT | MOD.CAPS);
  assert.equal(modsOf({}, true), MOD.REPEAT);
});

test('rebaseTimes shifts every record', () => {
  const u8 = concat([encodeFocus({ focused: true, time: 1000 }), encodeFocus({ focused: false, time: 2000 })]);
  rebaseTimes(u8, -900);
  assert.deepEqual(decodeAll(u8).map((r) => r.time), [100, 1100]);
});

test('InputQueue: drains whole records only, in order', () => {
  const q = new InputQueue();
  for (let i = 0; i < 5; i++) q.push(encodeKey({ down: true, code: 'KeyA', time: i }));
  const out = new Uint8Array(REC * 2 + 10);
  assert.equal(q.drain(out), REC * 2);
  assert.deepEqual(decodeAll(out.subarray(0, REC * 2)).map((r) => r.time), [0, 1]);
  const big = new Uint8Array(REC * 10);
  assert.equal(q.drain(big), REC * 3);
  assert.equal(q.drain(big), 0);
  assert.equal(q.length, 0);
});

test('InputQueue: coalesces consecutive mouse moves with the same buttons', () => {
  const q = new InputQueue();
  q.push(encodeMouseMove({ x: 1, y: 1, buttons: 0 }));
  q.push(encodeMouseMove({ x: 2, y: 2, buttons: 0 }));
  q.push(encodeMouseMove({ x: 3, y: 3, buttons: 1 })); // buttons changed: kept
  q.push(encodeMouseButton({ down: true, button: 0, x: 3, y: 3 }));
  q.push(encodeMouseMove({ x: 4, y: 4, buttons: 1 }));
  q.push(encodeMouseMove({ x: 5, y: 5, buttons: 1 }));
  const out = new Uint8Array(REC * 16);
  const n = q.drain(out);
  const recs = decodeAll(out.subarray(0, n));
  assert.deepEqual(recs.map((r) => [r.kind, r.x]), [[KIND.MOUSE_MOVE, 2], [KIND.MOUSE_MOVE, 3], [KIND.MOUSE_BUTTON, 3], [KIND.MOUSE_MOVE, 5]]);
});

test('InputQueue: overflow drops the oldest and reports it first', () => {
  const q = new InputQueue(4);
  for (let i = 0; i < 10; i++) q.push(encodeKey({ down: true, code: 'KeyA', time: i }));
  const out = new Uint8Array(REC * 16);
  const recs = decodeAll(out.subarray(0, q.drain(out, 99)));
  assert.equal(recs[0].kind, KIND.OVERFLOW);
  assert.equal(recs[0].dropped, 6);
  assert.deepEqual(recs.slice(1).map((r) => r.time), [6, 7, 8, 9]);
  assert.throws(() => q.push(new Uint8Array(31)), RangeError);
});

test('InputQueue drains into a view of a larger buffer at an offset (wasm memory)', () => {
  const mem = new Uint8Array(4096);
  const q = new InputQueue();
  q.pushAll(concat([encodeFocus({ focused: true }), encodeText('hi')]));
  const n = q.drain(new Uint8Array(mem.buffer, 1000, 64));
  assert.equal(n, 64);
  const recs = decodeAll(new Uint8Array(mem.buffer, 1000, n));
  assert.equal(recs[0].kind, KIND.FOCUS);
  assert.equal(recs[1].text, 'hi');
});

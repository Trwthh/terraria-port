import { test } from 'node:test';
import assert from 'node:assert/strict';
import { locateContent, validateInstall, isSkippedName, needsCopy, planCopy, hasRoom, fmtBytes, findGameExe, isMzHeader, isCopyAllowed, GAME_EXE } from '../lib/import-plan.js';

const D = (name) => ({ name, kind: 'directory' });
const F = (name) => ({ name, kind: 'file' });
const goodContent = [D('Images'), D('Fonts'), D('Sounds'), F('Wave Bank.xwb'), F('Sound Bank.xsb')];

test('locateContent: install folder, Content folder itself, wrong folder', () => {
  assert.equal(locateContent([F('Terraria.exe'), D('Content')]), 'Content');
  assert.equal(locateContent([D('content')]), 'content', 'case-insensitive, real name returned');
  assert.equal(locateContent(goodContent), '.');
  assert.equal(locateContent([F('Content')]), null, 'a file named Content does not count');
  assert.equal(locateContent([D('Documents'), F('readme.txt')]), null);
});

test('validateInstall: accepts a good Content folder', () => {
  assert.deepEqual(validateInstall('Content', goodContent), { ok: true, problems: [] });
  assert.equal(validateInstall('Content', [D('images'), D('FONTS'), F('Music.XWB')]).ok, true, 'case-insensitive');
  const inside = validateInstall('.', goodContent);
  assert.equal(inside.ok, false, 'the Content folder itself has no Terraria.exe next to it');
  assert.match(inside.problems[0], /Terraria folder above it/);
});

test('validateInstall: names each missing part', () => {
  const v = validateInstall('Content', [D('Images'), F('readme.txt')]);
  assert.equal(v.ok, false);
  assert.equal(v.problems.length, 2);
  assert.match(v.problems.join(' '), /Fonts/);
  assert.match(v.problems.join(' '), /xwb/);
  assert.equal(validateInstall(null, null).ok, false);
  assert.equal(validateInstall('Content', [D('Images'), D('Fonts'), D('Wave Bank.xwb')]).ok, false, 'a folder named .xwb is not a sound bank');
});

test('program files are never copied', () => {
  for (const n of ['Terraria.exe', 'FNA.dll', 'x.PDB', 'a.so']) assert.equal(isSkippedName(n), true, n);
  for (const n of ['Item_1.xnb', 'Wave Bank.xwb', 'dll.txt']) assert.equal(isSkippedName(n), false, n);
});

test('resumable copy decision: same size skips, anything else copies', () => {
  assert.equal(needsCopy(100, 100), false);
  assert.equal(needsCopy(100, 99), true);
  assert.equal(needsCopy(100, 0), true, 'an empty leftover is recopied');
  assert.equal(needsCopy(100, undefined), true);
  assert.equal(needsCopy(0, 0), false);
});

test('planCopy: first run, partial resume, complete', () => {
  const src = [
    { path: 'Content/Images/Item_1.xnb', size: 10 },
    { path: 'Content/Images/Item_2.xnb', size: 20 },
    { path: 'Content/Fonts/Mouse_Text.xnb', size: 30 },
    { path: 'Content/Wave Bank.xwb', size: 1000 },
    { path: 'Content/stray.dll', size: 5 },
    { path: 'Terraria.exe', size: 4000 },
  ];
  const first = planCopy(src, new Map());
  assert.equal(first.copy.length, 5);
  assert.equal(first.copyBytes, 5060);
  assert.equal(first.totalFiles, 5);

  const partial = planCopy(src, new Map([['Content/Images/Item_1.xnb', 10], ['Content/Images/Item_2.xnb', 7], ['Content/Extra/old.xnb', 1], ['Terraria.exe', 4000]]));
  assert.deepEqual(partial.copy.map((f) => f.path), ['Content/Images/Item_2.xnb', 'Content/Fonts/Mouse_Text.xnb', 'Content/Wave Bank.xwb']);
  assert.deepEqual(partial.skip.map((f) => f.path), ['Content/Images/Item_1.xnb', 'Terraria.exe']);
  assert.equal(partial.skipBytes, 4010);
  assert.equal(partial.totalBytes, 5060);

  const exeChanged = planCopy(src, new Map(src.map((f) => [f.path, f.path === 'Terraria.exe' ? 3999 : f.size])));
  assert.deepEqual(exeChanged.copy.map((f) => f.path), ['Terraria.exe'], 'a Terraria.exe of another size is copied again');

  const done = planCopy(src, new Map(src.map((f) => [f.path, f.size])));
  assert.equal(done.copy.length, 0);
  assert.equal(done.skip.length, 5);
});

test('hasRoom keeps a 2% margin; unknown quota tries anyway', () => {
  assert.equal(hasRoom({ quota: 1000, usage: 0 }, 900), true);
  assert.equal(hasRoom({ quota: 1000, usage: 100 }, 900), false);
  assert.equal(hasRoom(null, 1e12), true);
});

test('fmtBytes', () => {
  assert.equal(fmtBytes(512), '512 B');
  assert.equal(fmtBytes(1536), '1.5 KB');
  assert.equal(fmtBytes(759 * 1024 * 1024), '759 MB');
});

test('Terraria.exe: found case-insensitively, must start with MZ, required', () => {
  assert.equal(GAME_EXE, 'Terraria.exe');
  assert.equal(findGameExe([D('Content'), F('terraria.EXE'), F('TerrariaServer.exe')]), 'terraria.EXE');
  assert.equal(findGameExe([D('Content'), D('Terraria.exe')]), null, 'a folder of that name does not count');
  assert.equal(findGameExe([D('Content'), F('TerrariaServer.exe')]), null);
  assert.equal(isMzHeader(Uint8Array.of(0x4d, 0x5a, 0x90, 0)), true);
  assert.equal(isMzHeader(Uint8Array.of(0x5a, 0x4d)), false);
  assert.equal(isMzHeader(Uint8Array.of(0x4d)), false);
  assert.equal(isMzHeader(new Uint8Array(0)), false);
  const missing = validateInstall('Content', goodContent, { found: false, mz: null });
  assert.equal(missing.ok, false);
  assert.match(missing.problems.join(' '), /Terraria\.exe is missing/);
  const notPe = validateInstall('Content', goodContent, { found: true, mz: false });
  assert.equal(notPe.ok, false);
  assert.match(notPe.problems.join(' '), /MZ/);
  assert.equal(validateInstall('Content', goodContent, { found: true, mz: true }).ok, true);
});

test('only Terraria.exe at the top and non-program files under Content/ are ever copied', () => {
  assert.equal(isCopyAllowed('Terraria.exe'), true);
  for (const p of ['OnlineFix.dll', 'steam_api.dll', 'steam_api64.dll', 'TerrariaServer.exe', 'FNA.dll', 'terraria.exe',
    'Content/OnlineFix.dll', 'Content/steam_api.dll', 'Content/Terraria.exe', 'Content/sub/x.dll', 'Saves/x.plr', 'Images/Item_1.xnb']) {
    assert.equal(isCopyAllowed(p), false, p);
  }
  for (const p of ['Content/Images/Item_1.xnb', 'Content/Wave Bank.xwb', 'Content/Fonts/Mouse_Text.xnb']) assert.equal(isCopyAllowed(p), true, p);
  const plan = planCopy([
    { path: 'Terraria.exe', size: 1 }, { path: 'OnlineFix.dll', size: 1 }, { path: 'steam_api.dll', size: 1 },
    { path: 'Content/steam_api.dll', size: 1 }, { path: 'Content/a.xnb', size: 1 },
  ], new Map());
  assert.deepEqual(plan.copy.map((f) => f.path), ['Terraria.exe', 'Content/a.xnb']);
});

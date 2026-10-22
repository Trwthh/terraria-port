// First-run import: does the picked folder look like a Terraria install, and which files still need copying.
// Pure: no browser APIs, tested by test/import-plan.test.mjs.

const lower = (s) => s.toLowerCase();
const findEntry = (entries, name, kind) =>
  entries.find((e) => lower(e.name) === lower(name) && (!kind || e.kind === kind)) ?? null;

/**
 * Where is Content? `rootEntries` = [{ name, kind: 'file' | 'directory' }] of the picked folder.
 * Returns the Content folder's real name, '.' when the picked folder IS a Content folder
 * (it holds Images and Fonts), or null.
 */
export function locateContent(rootEntries) {
  const c = findEntry(rootEntries, 'Content', 'directory');
  if (c) return c.name;
  if (findEntry(rootEntries, 'Images', 'directory') && findEntry(rootEntries, 'Fonts', 'directory')) return '.';
  return null;
}

/**
 * Check a Content folder's entries. `where` is locateContent's result, `contentEntries` the entries of that folder
 * (or null when there is none). Returns { ok, problems: [string] } with player-facing messages.
 */
export function validateInstall(where, contentEntries, exe = { found: true, mz: true }) {
  const problems = [];
  if (where === '.') {
    problems.push('This is the Content folder. Pick the Terraria folder above it (the one that holds Terraria.exe and Content).');
    return { ok: false, problems };
  }
  if (where === null || !contentEntries) {
    problems.push('There is no Content folder here. Pick the Terraria folder (the one that holds Terraria.exe and Content).');
    return { ok: false, problems };
  }
  if (!exe.found) problems.push('Terraria.exe is missing (the game reads its built-in resources from it).');
  else if (exe.mz === false) problems.push('Terraria.exe is not a Windows program (it does not start with "MZ").');
  if (!findEntry(contentEntries, 'Images', 'directory')) problems.push('Content/Images is missing.');
  if (!findEntry(contentEntries, 'Fonts', 'directory')) problems.push('Content/Fonts is missing.');
  const audio = contentEntries.some((e) => e.kind === 'file' && /\.(xwb|xsb)$/i.test(e.name));
  if (!audio) problems.push('Content has no sound bank (.xwb / .xsb file).');
  return { ok: problems.length === 0, problems };
}

/** Files that are never copied, whatever folder they sit in: program code, not content. */
export function isSkippedName(name) {
  return /\.(exe|dll|pdb|so|dylib|bat|cmd|ps1)$/i.test(name);
}

/** The one program file that is copied: the game reads its embedded resources from it. Lands at /install/Terraria.exe. */
export const GAME_EXE = 'Terraria.exe';

/** The real name of Terraria.exe among the picked folder's entries (case-insensitive), or null. */
export function findGameExe(rootEntries) {
  return findEntry(rootEntries, GAME_EXE, 'file')?.name ?? null;
}

/** A Windows executable starts with "MZ". */
export function isMzHeader(bytes) {
  return bytes.length >= 2 && bytes[0] === 0x4d && bytes[1] === 0x5a;
}

/**
 * What may be copied, by path relative to /install: exactly "Terraria.exe" at the top, and anything under
 * "Content/" that is not program code. No other .exe / .dll is ever copied, wherever it sits.
 */
export function isCopyAllowed(path) {
  if (path === GAME_EXE) return true;
  if (!path.startsWith('Content/')) return false;
  return !isSkippedName(path.split('/').pop());
}

/** The resumable-copy decision for one file: copy unless the destination exists with the same size. */
export function needsCopy(srcSize, dstSize) {
  return dstSize === undefined || dstSize === null || dstSize !== srcSize;
}

/**
 * Plan the copy. `src` = [{ path, size, ... }] (path relative to /install, '/' separated: "Terraria.exe",
 * "Content/Images/Item_1.xnb", ...); `dstSizes` = Map(path -> size) of what OPFS already has under /install.
 * Paths that isCopyAllowed refuses are dropped.
 * Files present only in the destination are left alone (nothing is deleted).
 */
export function planCopy(src, dstSizes) {
  const copy = [];
  const skip = [];
  let copyBytes = 0;
  let skipBytes = 0;
  for (const f of src) {
    if (!isCopyAllowed(f.path)) continue;
    if (needsCopy(f.size, dstSizes.get(f.path))) {
      copy.push(f);
      copyBytes += f.size;
    } else {
      skip.push(f);
      skipBytes += f.size;
    }
  }
  return { copy, skip, copyBytes, skipBytes, totalBytes: copyBytes + skipBytes, totalFiles: copy.length + skip.length };
}

/** Is there room? `estimate` = navigator.storage.estimate() result. A 2% margin is kept. */
export function hasRoom(estimate, bytesNeeded) {
  if (!estimate || typeof estimate.quota !== 'number') return true; // unknown: try anyway
  const free = estimate.quota - (estimate.usage ?? 0);
  return free >= bytesNeeded * 1.02;
}

export function fmtBytes(n) {
  if (n < 1024) return `${n} B`;
  const u = ['KB', 'MB', 'GB', 'TB'];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < u.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(v < 10 ? 1 : 0)} ${u[i]}`;
}

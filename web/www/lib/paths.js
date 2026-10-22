// Path parsing and the write guard for the host file system (HOST_ABI.md, "Files").
// Pure: no browser APIs, tested by test/paths.test.mjs.

// WASI preview1 errno numbers (wasi_snapshot_preview1), shared by the `tw` file imports and wasi.js.
export const ERRNO = Object.freeze({
  EACCES: 2,
  EAGAIN: 6,
  EBADF: 8,
  EBUSY: 10,
  EEXIST: 20,
  EFAULT: 21,
  EINVAL: 28,
  EIO: 29,
  EISDIR: 31,
  EMFILE: 33,
  ENAMETOOLONG: 37,
  ENOENT: 44,
  ENOSPC: 51,
  ENOSYS: 52,
  ENOTDIR: 54,
  ENOTEMPTY: 55,
  ENOTSUP: 58,
  EOVERFLOW: 61,
  ESPIPE: 70,
  ENOTCAPABLE: 76,
});

export const ROOTS = Object.freeze(['install', 'saves']);
export const WRITABLE_ROOT = 'saves';
export const MAX_PATH = 4096;
export const MAX_NAME = 255;

/**
 * Parse a host path. Accepts `/` and `\` as separators, collapses repeated separators and ignores a trailing one.
 * Returns { parts, root } (parts[0] is the root name; "/" itself is parts = [], root = null) or { err } (WASI errno).
 */
export function parsePath(s) {
  if (typeof s !== 'string' || s.length === 0) return { err: ERRNO.EINVAL };
  if (s.length > MAX_PATH) return { err: ERRNO.ENAMETOOLONG };
  if (s.includes('\0')) return { err: ERRNO.EINVAL };
  const norm = s.replace(/\\/g, '/');
  if (norm[0] !== '/') return { err: ERRNO.EINVAL };
  const parts = norm.split('/').filter((p) => p.length > 0);
  for (const p of parts) {
    if (p === '.' || p === '..') return { err: ERRNO.EINVAL };
    if (p.length > MAX_NAME) return { err: ERRNO.ENAMETOOLONG };
  }
  if (parts.length === 0) return { parts, root: null };
  if (!ROOTS.includes(parts[0])) return { err: ERRNO.ENOENT };
  return { parts, root: parts[0] };
}

/**
 * The write guard. Every operation that creates, truncates, appends, renames or removes goes through it.
 * Returns 0 when allowed, else a positive errno (the import returns its negation).
 * - Anything under /install is read-only: EACCES.
 * - "/", "/install" and "/saves" themselves cannot be created, removed or renamed: EACCES.
 */
export function checkWrite(parsed) {
  if (!parsed || parsed.err) return parsed?.err ?? ERRNO.EINVAL;
  if (parsed.root !== WRITABLE_ROOT) return ERRNO.EACCES;
  if (parsed.parts.length < 2) return ERRNO.EACCES;
  return 0;
}

/** Convenience: parse + guard in one step for a write. Returns { parts, root } or { err }. */
export function parseForWrite(s) {
  const p = parsePath(s);
  if (p.err) return p;
  const g = checkWrite(p);
  return g ? { err: g } : p;
}

/** Open modes of fs_open. */
export const MODE = Object.freeze({ READ: 0, WRITE: 1, APPEND: 2 });

/** Maps a DOMException (OPFS) to an errno. */
export function errnoFromDomError(e) {
  switch (e && e.name) {
    case 'NotFoundError': return ERRNO.ENOENT;
    case 'NoModificationAllowedError': return ERRNO.EBUSY; // another sync access handle holds the lock
    case 'InvalidModificationError': return ERRNO.ENOTEMPTY;
    case 'TypeMismatchError': return ERRNO.ENOTDIR;
    case 'QuotaExceededError': return ERRNO.ENOSPC;
    case 'NotAllowedError':
    case 'SecurityError': return ERRNO.EACCES;
    case 'InvalidStateError': return ERRNO.EBADF;
    default: return ERRNO.EIO;
  }
}

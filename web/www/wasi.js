// WASI preview1 (`wasi_snapshot_preview1`) for the game Worker, over the same file tree as the `tw` imports (vfs.js):
// /install read-only, /saves read-write, case-insensitive lookup, the same write guard and shared OPFS handles.
//
// Preopens: fd 3 = "/install", fd 4 = "/saves". fd 0 reads as empty, fd 1 / fd 2 go to the log (line-buffered).
// Errno numbers are WASI preview1's (lib/paths.js ERRNO). Functions in WASI_SUSPENDING may return a Promise and are
// wrapped in WebAssembly.Suspending by the worker. proc_exit throws WasiExit, which the worker catches.
// Contract: HOST_ABI.md section "wasi_snapshot_preview1".

import { ERRNO, errnoFromDomError } from './lib/paths.js';
import { then } from './vfs.js';
import { fillRandom } from './host.js';

export const WASI_SUSPENDING = new Set([
  'path_open', 'path_filestat_get', 'path_create_directory', 'path_remove_directory', 'path_unlink_file', 'path_rename',
]);

export class WasiExit extends Error {
  constructor(code) {
    super(`proc_exit(${code})`);
    this.name = 'WasiExit';
    this.code = code;
  }
}

export const FILETYPE = Object.freeze({ UNKNOWN: 0, CHARACTER_DEVICE: 2, DIRECTORY: 3, REGULAR_FILE: 4 });
export const OFLAGS = Object.freeze({ CREAT: 1, DIRECTORY: 2, EXCL: 4, TRUNC: 8 });
export const FDFLAGS = Object.freeze({ APPEND: 1, DSYNC: 2, NONBLOCK: 4, RSYNC: 8, SYNC: 16 });
export const WHENCE = Object.freeze({ SET: 0, CUR: 1, END: 2 });
const b = (n) => 1n << BigInt(n);
export const RIGHTS = Object.freeze({
  FD_DATASYNC: b(0), FD_READ: b(1), FD_SEEK: b(2), FD_FDSTAT_SET_FLAGS: b(3), FD_SYNC: b(4), FD_TELL: b(5),
  FD_WRITE: b(6), FD_ADVISE: b(7), FD_ALLOCATE: b(8), PATH_CREATE_DIRECTORY: b(9), PATH_CREATE_FILE: b(10),
  PATH_LINK_SOURCE: b(11), PATH_LINK_TARGET: b(12), PATH_OPEN: b(13), FD_READDIR: b(14), PATH_READLINK: b(15),
  PATH_RENAME_SOURCE: b(16), PATH_RENAME_TARGET: b(17), PATH_FILESTAT_GET: b(18), PATH_FILESTAT_SET_SIZE: b(19),
  PATH_FILESTAT_SET_TIMES: b(20), FD_FILESTAT_GET: b(21), FD_FILESTAT_SET_SIZE: b(22), FD_FILESTAT_SET_TIMES: b(23),
  PATH_SYMLINK: b(24), PATH_REMOVE_DIRECTORY: b(25), PATH_UNLINK_FILE: b(26), POLL_FD_READWRITE: b(27),
  SOCK_SHUTDOWN: b(28), SOCK_ACCEPT: b(29),
});
const R = RIGHTS;
const ALL_RIGHTS = b(30) - 1n;
const FILE_WRITE_RIGHTS = R.FD_WRITE | R.FD_ALLOCATE | R.FD_FILESTAT_SET_SIZE | R.FD_DATASYNC | R.FD_SYNC;
const FILE_RIGHTS = R.FD_READ | R.FD_SEEK | R.FD_TELL | R.FD_FDSTAT_SET_FLAGS | R.FD_ADVISE | R.FD_FILESTAT_GET |
  R.FD_FILESTAT_SET_TIMES | R.POLL_FD_READWRITE | FILE_WRITE_RIGHTS;
const DIR_READ_RIGHTS = R.PATH_OPEN | R.FD_READDIR | R.PATH_FILESTAT_GET | R.FD_FILESTAT_GET | R.FD_FDSTAT_SET_FLAGS;
const DIR_WRITE_RIGHTS = R.PATH_CREATE_DIRECTORY | R.PATH_CREATE_FILE | R.PATH_RENAME_SOURCE | R.PATH_RENAME_TARGET |
  R.PATH_FILESTAT_SET_SIZE | R.PATH_FILESTAT_SET_TIMES | R.PATH_REMOVE_DIRECTORY | R.PATH_UNLINK_FILE;

/** Every preview1 function name. Those not implemented below are ENOSYS stubs. */
export const PREVIEW1 = Object.freeze([
  'args_get', 'args_sizes_get', 'environ_get', 'environ_sizes_get', 'clock_res_get', 'clock_time_get',
  'fd_advise', 'fd_allocate', 'fd_close', 'fd_datasync', 'fd_fdstat_get', 'fd_fdstat_set_flags', 'fd_fdstat_set_rights',
  'fd_filestat_get', 'fd_filestat_set_size', 'fd_filestat_set_times', 'fd_pread', 'fd_prestat_get', 'fd_prestat_dir_name',
  'fd_pwrite', 'fd_read', 'fd_readdir', 'fd_renumber', 'fd_seek', 'fd_sync', 'fd_tell', 'fd_write',
  'path_create_directory', 'path_filestat_get', 'path_filestat_set_times', 'path_link', 'path_open', 'path_readlink',
  'path_remove_directory', 'path_rename', 'path_symlink', 'path_unlink_file', 'poll_oneoff', 'proc_exit', 'proc_raise',
  'sched_yield', 'random_get', 'sock_accept', 'sock_recv', 'sock_send', 'sock_shutdown',
]);

export const DEFAULT_ENV = Object.freeze({ TERRARIA_DIR: '/install', TERRARIA_SAVE_DIR: '/saves', TERRARIA_PROJECT_DIR: '/saves' });

const enc = new TextEncoder();
const dec = new TextDecoder();
const MAX_FDS = 1024;
/** vfs results (0 or -errno) -> WASI results (0 or errno), never -0. */
const errnoOf = (v) => (v < 0 ? -v : 0);
const isThenable = (v) => v !== null && typeof v === 'object' && typeof v.then === 'function';

/**
 * @param {object} o
 * @param {ReturnType<import('./vfs.js').createVfs>} o.vfs
 * @param {(level: number, text: string) => void} o.log
 * @param {string[]} [o.args]
 * @param {Record<string,string>} [o.env]
 * @param {{ read(max: number): Uint8Array | null }} [o.stdin] fd 0 (HOST_ABI.md "Server console"): `read` returns up to
 *   `max` bytes, an empty array at the end of input, or null when nothing is there yet (fd_read answers EAGAIN).
 *   Without it fd 0 is at its end at once.
 * @param {(fd: number, text: string) => void} [o.onStd] every piece of text written to fd 1 / 2 as it comes (the
 *   server console's output), besides the line log
 */
export function createWasi({ vfs, log, args = ['terraria'], env = DEFAULT_ENV, stdin = null, onStd = null }) {
  let memory = null;
  const dv = () => new DataView(memory.buffer);
  const u8 = () => new Uint8Array(memory.buffer);
  const inBounds = (ptr, len) => memory !== null && ptr + len <= memory.buffer.byteLength;
  const readStr = (ptr, len) => (inBounds(ptr >>> 0, len >>> 0) ? dec.decode(u8().slice(ptr >>> 0, (ptr >>> 0) + (len >>> 0))) : null);

  // ---------------------------------------------------------------- descriptors
  const fds = new Map();
  const stdio = (n) => ({ type: 'stdio', n, rightsBase: n === 0 ? R.FD_READ : R.FD_WRITE, rightsInh: 0n, fdflags: 0 });
  fds.set(0, stdio(0));
  fds.set(1, stdio(1));
  fds.set(2, stdio(2));
  const preopen = (name, writable) => ({
    type: 'dir', parts: [name], base: 1, preopen: '/' + name,
    rightsBase: DIR_READ_RIGHTS | (writable ? DIR_WRITE_RIGHTS : 0n),
    rightsInh: writable ? ALL_RIGHTS : ALL_RIGHTS & ~(FILE_WRITE_RIGHTS | DIR_WRITE_RIGHTS),
    fdflags: 0,
  });
  fds.set(3, preopen('install', false));
  fds.set(4, preopen('saves', true));
  // fd 5 = "/": libc picks the longest matching preopen, so "/install/..." and "/saves/..." still go to 3 / 4.
  // "/" catches what they cannot: paths written with Windows separators ("/saves\config.json", "\saves"),
  // which libc does not match against "/saves"; resolvePath turns each "\" into "/" and the same tree, lookup and
  // write guard apply (nothing directly under "/" can be created, removed or renamed).
  fds.set(5, {
    type: 'dir', parts: [], base: 0, preopen: '/',
    rightsBase: DIR_READ_RIGHTS | DIR_WRITE_RIGHTS, rightsInh: ALL_RIGHTS, fdflags: 0,
  });

  function allocFd(entry) {
    for (let fd = 6; fd < MAX_FDS + 6; fd++) {
      if (!fds.has(fd)) {
        fds.set(fd, entry);
        return fd;
      }
    }
    return -1;
  }

  /** Resolve `path` relative to directory fd `dirfd`. Returns { parts } (absolute, from the tree root) or { err }. */
  function resolvePath(dirfd, ptr, len) {
    const d = fds.get(dirfd);
    if (!d) return { err: ERRNO.EBADF };
    if (d.type !== 'dir') return { err: ERRNO.ENOTDIR };
    const s = readStr(ptr, len);
    if (s === null) return { err: ERRNO.EFAULT };
    if (s.includes('\0')) return { err: ERRNO.EINVAL };
    // An absolute path never reaches here from libc (it strips the preopen prefix). A leading "\" is a Windows
    // rooted path ("\saves\Worlds"): the root of the current drive, here the root of the tree, whichever directory
    // fd it came with (libc hands over "\saves\Worlds" relative to /saves when the current directory is /saves).
    // The write guard still applies to whatever it names.
    if (s.startsWith('/')) return { err: ERRNO.ENOTCAPABLE };
    const rooted = s.startsWith('\\');
    const norm = s.replace(/\\/g, '/');
    const parts = rooted ? [] : d.parts.slice();
    const base = rooted ? 0 : d.base;
    for (const p of norm.split('/')) {
      if (p === '' || p === '.') continue;
      if (p === '..') {
        if (parts.length <= base) return { err: ERRNO.ENOTCAPABLE }; // above the preopen
        parts.pop();
        continue;
      }
      if (enc.encode(p).length > 255) return { err: ERRNO.ENAMETOOLONG };
      parts.push(p);
    }
    return { parts, dir: d };
  }

  const need = (f, right) => (f.rightsBase & right) === right;

  // ---------------------------------------------------------------- stdout / stderr
  const pending = { 1: '', 2: '' };
  const stdDec = { 1: new TextDecoder(), 2: new TextDecoder() };
  function writeStd(n, text) {
    pending[n] += text;
    let i;
    while ((i = pending[n].indexOf('\n')) >= 0) {
      log(n === 2 ? 3 : 2, pending[n].slice(0, i));
      pending[n] = pending[n].slice(i + 1);
    }
    if (pending[n].length > 4096) {
      log(n === 2 ? 3 : 2, pending[n]);
      pending[n] = '';
    }
  }
  function flushStd() {
    for (const n of [1, 2]) {
      if (pending[n]) log(n === 2 ? 3 : 2, pending[n]);
      pending[n] = '';
    }
  }

  // ---------------------------------------------------------------- iovecs
  function iovs(ptr, n) {
    ptr >>>= 0;
    n >>>= 0;
    if (!inBounds(ptr, n * 8)) return null;
    const v = dv();
    const out = [];
    for (let i = 0; i < n; i++) {
      const p = v.getUint32(ptr + i * 8, true);
      const l = v.getUint32(ptr + i * 8 + 4, true);
      if (!inBounds(p, l)) return null;
      out.push([p, l]);
    }
    return out;
  }
  function readAt(f, list, at) {
    let total = 0;
    for (const [p, l] of list) {
      if (l === 0) continue;
      const n = f.s.access.read(new Uint8Array(memory.buffer, p, l), { at: at + total });
      total += n;
      if (n < l) break;
    }
    return total;
  }
  function writeAt(f, list, at) {
    let total = 0;
    for (const [p, l] of list) {
      if (l === 0) continue;
      total += f.s.access.write(new Uint8Array(memory.buffer, p, l), { at: at + total });
    }
    return total;
  }
  const setU32 = (ptr, v) => dv().setUint32(ptr >>> 0, v >>> 0, true);
  const setU64 = (ptr, v) => dv().setBigUint64(ptr >>> 0, BigInt(v), true);

  // ---------------------------------------------------------------- the functions
  const reported = new Set();
  const stub = (name) => () => {
    if (!reported.has(name)) {
      reported.add(name);
      log(3, `wasi: ${name} is not implemented (ENOSYS)`);
    }
    return ERRNO.ENOSYS;
  };

  function strings(list) {
    const bufs = list.map((s) => enc.encode(s + '\0'));
    return { bufs, size: bufs.reduce((n, x) => n + x.length, 0) };
  }
  const argv = strings(args);
  const envv = strings(Object.entries(env).map(([k, v]) => `${k}=${v}`));
  function sizesGet(t, countPtr, sizePtr) {
    if (!inBounds(countPtr >>> 0, 4) || !inBounds(sizePtr >>> 0, 4)) return ERRNO.EFAULT;
    setU32(countPtr, t.bufs.length);
    setU32(sizePtr, t.size);
    return 0;
  }
  function listGet(t, ptrsPtr, bufPtr) {
    ptrsPtr >>>= 0;
    bufPtr >>>= 0;
    if (!inBounds(ptrsPtr, t.bufs.length * 4) || !inBounds(bufPtr, t.size)) return ERRNO.EFAULT;
    let o = bufPtr;
    t.bufs.forEach((x, i) => {
      setU32(ptrsPtr + i * 4, o);
      u8().set(x, o);
      o += x.length;
    });
    return 0;
  }

  function filestat(ptr, { filetype, size = 0, ino = 0, mtime = 0 }) {
    ptr >>>= 0;
    if (!inBounds(ptr, 64)) return ERRNO.EFAULT;
    const v = dv();
    const ns = BigInt(Math.round(mtime)) * 1_000_000n;
    v.setBigUint64(ptr, 1n, true); // dev
    v.setBigUint64(ptr + 8, BigInt(ino), true);
    v.setUint8(ptr + 16, filetype);
    for (let i = 17; i < 24; i++) v.setUint8(ptr + i, 0);
    v.setBigUint64(ptr + 24, 1n, true); // nlink
    v.setBigUint64(ptr + 32, BigInt(size), true);
    v.setBigUint64(ptr + 40, ns, true); // atim
    v.setBigUint64(ptr + 48, ns, true); // mtim
    v.setBigUint64(ptr + 56, ns, true); // ctim
    return 0;
  }

  const fns = {
    args_sizes_get: (c, s) => sizesGet(argv, c, s),
    args_get: (p, bufp) => listGet(argv, p, bufp),
    environ_sizes_get: (c, s) => sizesGet(envv, c, s),
    environ_get: (p, bufp) => listGet(envv, p, bufp),

    clock_res_get(id, ptr) {
      if (!inBounds(ptr >>> 0, 8)) return ERRNO.EFAULT;
      if (id === 0) setU64(ptr, 1_000_000n);
      else if (id >= 1 && id <= 3) setU64(ptr, 1_000n);
      else return ERRNO.EINVAL;
      return 0;
    },
    clock_time_get(id, _precision, ptr) {
      if (!inBounds(ptr >>> 0, 8)) return ERRNO.EFAULT;
      if (id === 0) setU64(ptr, BigInt(Date.now()) * 1_000_000n);
      else if (id >= 1 && id <= 3) setU64(ptr, BigInt(Math.round(performance.now() * 1e6)));
      else return ERRNO.EINVAL;
      return 0;
    },
    random_get(ptr, len) {
      ptr >>>= 0;
      len >>>= 0;
      if (!inBounds(ptr, len)) return ERRNO.EFAULT;
      fillRandom(new Uint8Array(memory.buffer, ptr, len));
      return 0;
    },
    sched_yield: () => 0,
    proc_exit(code) {
      flushStd();
      throw new WasiExit(code);
    },

    fd_prestat_get(fd, ptr) {
      const f = fds.get(fd);
      if (!f || !f.preopen) return ERRNO.EBADF;
      if (!inBounds(ptr >>> 0, 8)) return ERRNO.EFAULT;
      const v = dv();
      v.setUint8(ptr >>> 0, 0); // preopentype dir
      v.setUint32((ptr >>> 0) + 4, enc.encode(f.preopen).length, true);
      return 0;
    },
    fd_prestat_dir_name(fd, ptr, len) {
      const f = fds.get(fd);
      if (!f || !f.preopen) return ERRNO.EBADF;
      const name = enc.encode(f.preopen);
      if ((len >>> 0) < name.length) return ERRNO.EINVAL;
      if (!inBounds(ptr >>> 0, name.length)) return ERRNO.EFAULT;
      u8().set(name, ptr >>> 0);
      return 0;
    },

    fd_fdstat_get(fd, ptr) {
      const f = fds.get(fd);
      if (!f) return ERRNO.EBADF;
      ptr >>>= 0;
      if (!inBounds(ptr, 24)) return ERRNO.EFAULT;
      const v = dv();
      v.setUint8(ptr, f.type === 'stdio' ? FILETYPE.CHARACTER_DEVICE : f.type === 'dir' ? FILETYPE.DIRECTORY : FILETYPE.REGULAR_FILE);
      v.setUint8(ptr + 1, 0);
      v.setUint16(ptr + 2, f.fdflags, true);
      v.setUint32(ptr + 4, 0, true);
      v.setBigUint64(ptr + 8, f.rightsBase, true);
      v.setBigUint64(ptr + 16, f.rightsInh, true);
      return 0;
    },
    fd_fdstat_set_flags(fd, flags) {
      const f = fds.get(fd);
      if (!f) return ERRNO.EBADF;
      f.fdflags = flags & 0xffff;
      if (f.type === 'file') f.append = (flags & FDFLAGS.APPEND) !== 0;
      return 0;
    },
    fd_filestat_get(fd, ptr) {
      const f = fds.get(fd);
      if (!f) return ERRNO.EBADF;
      if (f.type === 'stdio') return filestat(ptr, { filetype: FILETYPE.CHARACTER_DEVICE });
      if (f.type === 'dir') return filestat(ptr, { filetype: FILETYPE.DIRECTORY, ino: vfs.resolve(f.parts)?.ino ?? 0 });
      const st = vfs.fileStat(f.node); // synchronous: the file is open
      return filestat(ptr, { filetype: FILETYPE.REGULAR_FILE, size: st.size, ino: f.node.ino, mtime: st.mtime });
    },
    fd_filestat_set_size(fd, size) {
      const f = fds.get(fd);
      if (!f) return ERRNO.EBADF;
      if (f.type !== 'file') return ERRNO.EINVAL;
      if (!need(f, R.FD_FILESTAT_SET_SIZE)) return ERRNO.ENOTCAPABLE;
      try {
        f.s.access.truncate(Number(size));
        f.wrote = true;
        return 0;
      } catch (e) {
        return errnoFromDomError(e);
      }
    },

    fd_read(fd, iovPtr, iovCnt, nreadPtr) {
      const f = fds.get(fd);
      if (!f) return ERRNO.EBADF;
      const list = iovs(iovPtr, iovCnt);
      if (!list || !inBounds(nreadPtr >>> 0, 4)) return ERRNO.EFAULT;
      if (f.type === 'stdio') {
        if (f.n !== 0) return ERRNO.EBADF;
        if (!stdin) {
          setU32(nreadPtr, 0);
          return 0;
        }
        const max = list.reduce((n, [, l]) => n + l, 0);
        const got = stdin.read(max);
        if (got === null) return ERRNO.EAGAIN;
        let off = 0;
        for (const [p, l] of list) {
          if (off >= got.length) break;
          const n = Math.min(l, got.length - off);
          u8().set(got.subarray(off, off + n), p);
          off += n;
        }
        setU32(nreadPtr, off);
        return 0;
      }
      if (f.type === 'dir') return ERRNO.EISDIR;
      if (!need(f, R.FD_READ)) return ERRNO.ENOTCAPABLE;
      try {
        const n = readAt(f, list, f.pos);
        f.pos += n;
        setU32(nreadPtr, n);
        return 0;
      } catch (e) {
        return errnoFromDomError(e);
      }
    },
    fd_pread(fd, iovPtr, iovCnt, offset, nreadPtr) {
      const f = fds.get(fd);
      if (!f) return ERRNO.EBADF;
      if (f.type === 'stdio') return ERRNO.ESPIPE;
      if (f.type === 'dir') return ERRNO.EISDIR;
      if (!need(f, R.FD_READ)) return ERRNO.ENOTCAPABLE;
      const list = iovs(iovPtr, iovCnt);
      if (!list || !inBounds(nreadPtr >>> 0, 4)) return ERRNO.EFAULT;
      try {
        setU32(nreadPtr, readAt(f, list, Number(offset)));
        return 0;
      } catch (e) {
        return errnoFromDomError(e);
      }
    },
    fd_write(fd, iovPtr, iovCnt, nwrittenPtr) {
      const f = fds.get(fd);
      if (!f) return ERRNO.EBADF;
      const list = iovs(iovPtr, iovCnt);
      if (!list || !inBounds(nwrittenPtr >>> 0, 4)) return ERRNO.EFAULT;
      if (f.type === 'stdio') {
        if (f.n === 0) return ERRNO.EBADF;
        let total = 0;
        for (const [p, l] of list) {
          const text = stdDec[f.n].decode(u8().slice(p, p + l), { stream: true });
          writeStd(f.n, text);
          if (onStd && text) onStd(f.n, text);
          total += l;
        }
        setU32(nwrittenPtr, total);
        return 0;
      }
      if (f.type === 'dir') return ERRNO.EISDIR;
      if (!need(f, R.FD_WRITE)) return ERRNO.ENOTCAPABLE;
      try {
        if (f.append) f.pos = f.s.access.getSize();
        const n = writeAt(f, list, f.pos);
        f.pos += n;
        f.wrote = true;
        setU32(nwrittenPtr, n);
        return 0;
      } catch (e) {
        return errnoFromDomError(e);
      }
    },
    fd_pwrite(fd, iovPtr, iovCnt, offset, nwrittenPtr) {
      const f = fds.get(fd);
      if (!f) return ERRNO.EBADF;
      if (f.type === 'stdio') return ERRNO.ESPIPE;
      if (f.type === 'dir') return ERRNO.EISDIR;
      if (!need(f, R.FD_WRITE)) return ERRNO.ENOTCAPABLE;
      const list = iovs(iovPtr, iovCnt);
      if (!list || !inBounds(nwrittenPtr >>> 0, 4)) return ERRNO.EFAULT;
      try {
        setU32(nwrittenPtr, writeAt(f, list, Number(offset)));
        f.wrote = true;
        return 0;
      } catch (e) {
        return errnoFromDomError(e);
      }
    },
    fd_seek(fd, offset, whence, outPtr) {
      const f = fds.get(fd);
      if (!f) return ERRNO.EBADF;
      if (f.type !== 'file') return f.type === 'dir' ? ERRNO.EISDIR : ERRNO.ESPIPE;
      if (!need(f, R.FD_SEEK) && !(whence === WHENCE.CUR && offset === 0n && need(f, R.FD_TELL))) return ERRNO.ENOTCAPABLE;
      if (!inBounds(outPtr >>> 0, 8)) return ERRNO.EFAULT;
      const off = Number(offset);
      let pos;
      if (whence === WHENCE.SET) pos = off;
      else if (whence === WHENCE.CUR) pos = f.pos + off;
      else if (whence === WHENCE.END) pos = f.s.access.getSize() + off;
      else return ERRNO.EINVAL;
      if (pos < 0) return ERRNO.EINVAL;
      f.pos = pos;
      setU64(outPtr, pos);
      return 0;
    },
    fd_tell(fd, outPtr) {
      const f = fds.get(fd);
      if (!f) return ERRNO.EBADF;
      if (f.type !== 'file') return ERRNO.ESPIPE;
      if (!inBounds(outPtr >>> 0, 8)) return ERRNO.EFAULT;
      setU64(outPtr, f.pos);
      return 0;
    },
    fd_sync(fd) {
      const f = fds.get(fd);
      if (!f) return ERRNO.EBADF;
      if (f.type === 'file' && f.s.rw) {
        try {
          f.s.access.flush();
        } catch (e) {
          return errnoFromDomError(e);
        }
      }
      return 0;
    },
    fd_datasync(fd) {
      return fns.fd_sync(fd);
    },
    fd_close(fd) {
      const f = fds.get(fd);
      if (!f) return ERRNO.EBADF;
      if (f.type === 'stdio') return 0;
      fds.delete(fd);
      if (f.type === 'file') {
        const r = vfs.release(f.node, f.s, f.wrote);
        return r < 0 ? -r : 0;
      }
      return 0;
    },
    fd_readdir(fd, bufPtr, bufLen, cookie, usedPtr) {
      const f = fds.get(fd);
      if (!f) return ERRNO.EBADF;
      if (f.type !== 'dir') return ERRNO.ENOTDIR;
      if (!need(f, R.FD_READDIR)) return ERRNO.ENOTCAPABLE;
      bufPtr >>>= 0;
      bufLen >>>= 0;
      if (!inBounds(bufPtr, bufLen) || !inBounds(usedPtr >>> 0, 4)) return ERRNO.EFAULT;
      const node = vfs.resolve(f.parts);
      if (!node || node.kind !== 'dir') return ERRNO.ENOENT;
      const entries = [{ name: '.', kind: 'dir', ino: node.ino }, { name: '..', kind: 'dir', ino: 0 }, ...vfs.list(node)];
      const out = u8();
      let used = 0;
      for (let i = Number(cookie); i < entries.length && used < bufLen; i++) {
        const e = entries[i];
        const name = enc.encode(e.name);
        const rec = new Uint8Array(24 + name.length);
        const v = new DataView(rec.buffer);
        v.setBigUint64(0, BigInt(i + 1), true); // d_next: the cookie of the following entry
        v.setBigUint64(8, BigInt(e.ino ?? 0), true);
        v.setUint32(16, name.length, true);
        v.setUint8(20, e.kind === 'dir' ? FILETYPE.DIRECTORY : FILETYPE.REGULAR_FILE);
        rec.set(name, 24);
        const n = Math.min(rec.length, bufLen - used); // the last entry may be cut: the caller retries from its cookie
        out.set(rec.subarray(0, n), bufPtr + used);
        used += n;
      }
      setU32(usedPtr, used);
      return 0;
    },

    path_open(dirfd, _dirflags, pathPtr, pathLen, oflags, rightsBase, rightsInh, fdflags, fdOutPtr) {
      const r = resolvePath(dirfd, pathPtr, pathLen);
      if (r.err) return r.err;
      if (!inBounds(fdOutPtr >>> 0, 4)) return ERRNO.EFAULT;
      if (!need(r.dir, R.PATH_OPEN)) return ERRNO.ENOTCAPABLE;
      const create = (oflags & OFLAGS.CREAT) !== 0;
      const excl = (oflags & OFLAGS.EXCL) !== 0;
      const trunc = (oflags & OFLAGS.TRUNC) !== 0;
      const wantDir = (oflags & OFLAGS.DIRECTORY) !== 0;
      const write = (BigInt(rightsBase) & R.FD_WRITE) !== 0n;
      // another Worker's changes under /saves first (vfs.js: the shared tree)
      return then(vfs.ready(r.parts), () => {
      const node = vfs.resolve(r.parts);
      if (node && node.kind === 'dir') {
        if (create && excl) return ERRNO.EEXIST;
        if (write || trunc) return ERRNO.EISDIR;
        const base = r.parts[0] === 'install' ? preopenRights(false) : preopenRights(true);
        const fd = allocFd({
          type: 'dir', parts: r.parts, base: 1,
          rightsBase: BigInt(rightsBase) & base.rightsBase, rightsInh: BigInt(rightsInh) & base.rightsInh, fdflags: 0,
        });
        if (fd < 0) return ERRNO.EMFILE;
        setU32(fdOutPtr, fd);
        return 0;
      }
      if (wantDir) return node ? ERRNO.ENOTDIR : create ? ERRNO.EINVAL : ERRNO.ENOENT;
      if (fds.size >= MAX_FDS) return ERRNO.EMFILE;
      const opened = vfs.openFile(r.parts, { write, create, excl, trunc });
      return then(opened, (o) => {
        if (typeof o === 'number') return -o;
        let granted = BigInt(rightsBase) & FILE_RIGHTS;
        if (!write) granted &= ~FILE_WRITE_RIGHTS;
        const fd = allocFd({
          type: 'file', parts: r.parts, node: o.node, s: o.s, pos: 0,
          append: (fdflags & FDFLAGS.APPEND) !== 0, wrote: trunc,
          rightsBase: granted, rightsInh: 0n, fdflags: fdflags & 0xffff,
        });
        if (fd < 0) {
          vfs.release(o.node, o.s, false);
          return ERRNO.EMFILE;
        }
        setU32(fdOutPtr, fd);
        return 0;
      });
      });
    },
    path_filestat_get(dirfd, _flags, pathPtr, pathLen, bufPtr) {
      const r = resolvePath(dirfd, pathPtr, pathLen);
      if (r.err) return r.err;
      if (!inBounds(bufPtr >>> 0, 64)) return ERRNO.EFAULT;
      return then(vfs.ready(r.parts), () => {
        const node = vfs.resolve(r.parts);
        if (!node) return ERRNO.ENOENT;
        if (node.kind === 'dir') return filestat(bufPtr, { filetype: FILETYPE.DIRECTORY, ino: node.ino });
        return then(vfs.fileStat(node), (st) => (typeof st === 'number' ? -st : filestat(bufPtr, { filetype: FILETYPE.REGULAR_FILE, size: st.size, ino: node.ino, mtime: st.mtime })));
      });
    },
    path_create_directory(dirfd, pathPtr, pathLen) {
      const r = resolvePath(dirfd, pathPtr, pathLen);
      if (r.err) return r.err;
      return then(vfs.mkdir(r.parts, { recursive: false }), errnoOf);
    },
    path_remove_directory(dirfd, pathPtr, pathLen) {
      const r = resolvePath(dirfd, pathPtr, pathLen);
      if (r.err) return r.err;
      return then(vfs.remove(r.parts, 'dir'), errnoOf);
    },
    path_unlink_file(dirfd, pathPtr, pathLen) {
      const r = resolvePath(dirfd, pathPtr, pathLen);
      if (r.err) return r.err;
      return then(vfs.remove(r.parts, 'file'), errnoOf);
    },
    path_rename(fd, oldPtr, oldLen, newFd, newPtr, newLen) {
      const a = resolvePath(fd, oldPtr, oldLen);
      if (a.err) return a.err;
      const c = resolvePath(newFd, newPtr, newLen);
      if (c.err) return c.err;
      return then(vfs.rename(a.parts, c.parts), errnoOf);
    },

    poll_oneoff(inPtr, outPtr, nsubs, neventsPtr) {
      inPtr >>>= 0;
      outPtr >>>= 0;
      nsubs >>>= 0;
      if (nsubs === 0) return ERRNO.EINVAL;
      if (!inBounds(inPtr, nsubs * 48) || !inBounds(outPtr, nsubs * 32) || !inBounds(neventsPtr >>> 0, 4)) return ERRNO.EFAULT;
      const v = dv();
      // single-threaded game: never block. A clock subscription fires at once; fd subscriptions report ENOTSUP.
      for (let i = 0; i < nsubs; i++) {
        const sub = inPtr + i * 48;
        const ev = outPtr + i * 32;
        const tag = v.getUint8(sub + 8);
        for (let k = 0; k < 32; k++) v.setUint8(ev + k, 0);
        v.setBigUint64(ev, v.getBigUint64(sub, true), true); // userdata
        v.setUint16(ev + 8, tag === 0 ? 0 : ERRNO.ENOTSUP, true);
        v.setUint8(ev + 10, tag);
      }
      setU32(neventsPtr, nsubs);
      return 0;
    },
  };

  function preopenRights(writable) {
    return preopen(writable ? 'saves' : 'install', writable);
  }

  const imports = {};
  for (const name of PREVIEW1) imports[name] = fns[name] ?? stub(name);
  // anything in fns that is not in PREVIEW1 would be a bug
  for (const name of Object.keys(fns)) if (!PREVIEW1.includes(name)) throw new Error(`wasi.js: ${name} is not a preview1 function`);

  /** Normalise return values: a thrown DOMException becomes EIO / its errno instead of a trap. */
  for (const name of PREVIEW1) {
    const fn = imports[name];
    if (name === 'proc_exit') continue;
    imports[name] = (...a) => {
      try {
        const r = fn(...a);
        return isThenable(r) ? r.then((x) => x, (e) => errnoFromDomError(e)) : r;
      } catch (e) {
        if (e instanceof WasiExit) throw e;
        return errnoFromDomError(e);
      }
    };
  }

  return {
    imports: { wasi_snapshot_preview1: imports },
    setMemory(m) { memory = m; },
    flush: flushStd,
    _debug: { fds },
  };
}

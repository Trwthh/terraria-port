// The virtual file tree shared by the `tw` file imports (host.js) and the WASI implementation (wasi.js).
//
// Two roots, both in OPFS: /install (the player's Content, read-only) and /saves (read-write).
// - Names and kinds are kept in an in-memory index (built once by init(), updated on every change), so stat and
//   directory listing are synchronous.
// - Lookup below the roots is case-insensitive when there is no exact match (the game was written for Windows).
// - One OPFS sync access handle per open file, shared by every descriptor on it (tw handles and WASI fds alike).
// - Every operation that changes something goes through the write guard (lib/paths.js checkWrite).
// - Several Workers of the page use the same /saves (the game Worker and the dedicated server of Host & Play,
//   server-worker.js), each with an index of its own. Every change under /saves is announced on a BroadcastChannel
//   ("tw-vfs:<saves folder>"): the other indexes mark the directories concerned stale, drop cached sizes of written
//   files and re-read those directories from OPFS at once (in the background, so stat and listing stay synchronous);
//   a lookup through an async entry point (`ready`: WASI path_open / path_filestat_get, tw fs_open) waits for that
//   re-read, and re-reads the parent on a miss under /saves, so the other Worker's file is there at its next lookup.
//
// Functions return a value, a negative errno, or a Promise of either (only when an async OPFS call is needed).

import { ERRNO, checkWrite, errnoFromDomError } from './lib/paths.js';

const isThenable = (v) => v !== null && typeof v === 'object' && typeof v.then === 'function';

/** Apply `fn` to `v`, or to its resolution when `v` is a Promise. */
export const then = (v, fn) => (isThenable(v) ? v.then(fn) : fn(v));

/**
 * @param {FileSystemDirectoryHandle} opfsRoot
 * @param {{ savesDir?: string, channel?: { postMessage(m: any): void, onmessage: any } | null }} [opts]  savesDir: the
 *   OPFS folder that is /saves (default "saves"; developer runs use their own, so each starts with no players or
 *   worlds); channel: where changes under /saves are announced and heard (default: a BroadcastChannel
 *   "tw-vfs:<savesDir>" when the platform has one; null: none); install: /install over HTTP (lib/remote-install.js,
 *   after its init()): the tree comes from its manifest and files open through it, instead of OPFS "install"
 */
export function createVfs(opfsRoot, { savesDir = 'saves', channel, install = null } = {}) {
  let nextIno = 1;
  const mkDir = (handle) => ({ kind: 'dir', handle, children: new Map(), lower: new Map(), ino: nextIno++, parent: null, name: '', stale: false, refreshing: null });
  const mkFile = (handle) => ({ kind: 'file', handle, ino: nextIno++, parent: null, name: '' });
  const root = mkDir(opfsRoot);
  const stats = { announced: 0, heard: 0, refreshed: 0, missRefreshes: 0 };

  function addChild(dir, name, node) {
    node.parent = dir;
    node.name = name;
    dir.children.set(name, node);
    const l = name.toLowerCase();
    if (!dir.lower.has(l)) dir.lower.set(l, name);
  }
  function delChild(dir, name) {
    dir.children.delete(name);
    const l = name.toLowerCase();
    if (dir.lower.get(l) === name) {
      dir.lower.delete(l);
      for (const k of dir.children.keys()) if (k.toLowerCase() === l) { dir.lower.set(l, k); break; }
    }
  }
  /** Exact name first, then case-insensitive. Returns [realName, node] or null. */
  function child(dir, name) {
    const n = dir.children.get(name);
    if (n) return [name, n];
    const real = dir.lower.get(name.toLowerCase());
    return real !== undefined ? [real, dir.children.get(real)] : null;
  }
  function resolve(parts) {
    let node = root;
    for (const p of parts) {
      if (node.kind !== 'dir') return null;
      const c = child(node, p);
      if (!c) return null;
      node = c[1];
    }
    return node;
  }
  /** The parent directory and the entry for `parts` (which must be non-empty). */
  function lookup(parts) {
    const parent = resolve(parts.slice(0, -1));
    const wanted = parts[parts.length - 1];
    if (!parent) return { parent: null, wanted, name: null, node: null, err: ERRNO.ENOENT };
    if (parent.kind !== 'dir') return { parent: null, wanted, name: null, node: null, err: ERRNO.ENOTDIR };
    const c = child(parent, wanted);
    return { parent, wanted, name: c ? c[0] : null, node: c ? c[1] : null, err: 0 };
  }
  const guard = (parts) => checkWrite({ parts, root: parts[0] ?? null });

  // ---------------------------------------------------------------- the /saves tree shared between Workers
  if (channel === undefined) {
    channel = typeof BroadcastChannel === 'function' ? new BroadcastChannel(`tw-vfs:${savesDir}`) : null;
    channel?.unref?.(); // Node (tests): the channel does not keep the process alive
  }
  /** The index path of a node (["saves", "Worlds", "x.wld"]). */
  function pathOf(node) {
    const out = [];
    for (let n = node; n && n !== root; n = n.parent) out.unshift(n.name);
    return out;
  }
  /** Tell the other Workers: these directories changed (entries), these files were written (contents / size). */
  function announce(dirs, written = []) {
    if (!channel) return;
    const d = dirs.map(pathOf).filter((p) => p[0] === 'saves');
    const w = written.map(pathOf).filter((p) => p[0] === 'saves');
    if (!d.length && !w.length) return;
    stats.announced++;
    try {
      channel.postMessage({ type: 'vfs-changed', dirs: d, written: w });
    } catch { /* a closed channel: nothing to tell */ }
  }
  /** The existing node deepest along `parts` (case-insensitive, as lookups are). */
  function deepest(parts) {
    let node = root;
    for (const p of parts) {
      const c = node.kind === 'dir' ? child(node, p) : null;
      if (!c) break;
      node = c[1];
    }
    return node;
  }
  /** Re-reads a directory's entries from OPFS: new ones are added (new directories stale, read at their first
   * lookup), gone ones removed (unless open here), cached sizes of its files dropped. One read at a time per node. */
  function refreshDir(node) {
    if (node.refreshing) return node.refreshing;
    node.stale = false;
    node.refreshing = (async () => {
      const seen = new Set();
      try {
        for await (const [name, h] of node.handle.entries()) {
          if (name.endsWith('.crswap')) continue;
          seen.add(name);
          const c = node.children.get(name);
          const isDir = h.kind === 'directory';
          if (c && (c.kind === 'dir') === isDir) {
            if (!isDir && !shared.has(c)) {
              c.handle = h;
              delete c.size;
              delete c.mtime;
            }
            continue;
          }
          if (c) {
            if (anyOpenUnder(c)) continue;
            delChild(node, name);
          }
          const n = isDir ? mkDir(h) : mkFile(h);
          if (isDir) n.stale = true;
          addChild(node, name, n);
        }
        for (const [name, c] of [...node.children]) if (!seen.has(name) && !anyOpenUnder(c)) delChild(node, name);
        stats.refreshed++;
      } catch {
        // the directory itself is gone: its parent's next read drops it
        if (node.parent) node.parent.stale = true;
      } finally {
        node.refreshing = null;
      }
    })();
    return node.refreshing;
  }
  if (channel) {
    channel.onmessage = (ev) => {
      const m = ev.data;
      if (!m || m.type !== 'vfs-changed') return;
      stats.heard++;
      for (const parts of m.written ?? []) {
        const n = resolve(parts);
        if (n && n.kind === 'file' && !shared.has(n)) {
          delete n.size;
          delete n.mtime;
        }
      }
      for (const parts of m.dirs ?? []) {
        let n = deepest(parts);
        if (n.kind !== 'dir') n = n.parent;
        if (!n || n === root) continue;
        n.stale = true;
        refreshDir(n);
      }
    };
  }
  /** Every directory under /saves is re-read (another Worker that changed things ended: its last announcements may
   * not have been delivered before it was terminated). Resolves when all are read. */
  function refreshSaves() {
    const saves = root.children.get('saves');
    if (!saves) return Promise.resolve();
    const dirs = [];
    const walk = (n) => {
      if (n.kind !== 'dir') return;
      dirs.push(n);
      for (const c of n.children.values()) walk(c);
    };
    walk(saves);
    for (const d of dirs) d.stale = true;
    return Promise.all(dirs.map(refreshDir)).then(() => undefined);
  }
  /**
   * Before a lookup of `parts` through an async entry point: undefined when the index can answer now, else a Promise
   * that resolves once the stale directories on the path are re-read, and, for a miss under /saves, the deepest
   * existing directory too (a file another Worker just created).
   */
  function ready(parts, onMiss = true, rounds = 0) {
    if (parts[0] !== 'saves') return undefined;
    const waits = [];
    let node = root;
    for (const p of parts) {
      if (node.kind !== 'dir') break;
      if (node.stale || node.refreshing) waits.push(refreshDir(node));
      const c = child(node, p);
      if (!c) {
        if (onMiss && !waits.length && node !== root) {
          stats.missRefreshes++;
          waits.push(refreshDir(node));
        }
        break;
      }
      node = c[1];
    }
    if (node.kind === 'dir' && (node.stale || node.refreshing)) waits.push(refreshDir(node));
    if (!waits.length) return undefined;
    // after the re-reads the path may go deeper (a new directory): once more
    return Promise.all(waits).then(() => {
      // after the re-reads the path may go deeper (a new directory, read when first looked into): once more, a few
      // levels at most, without re-reading on a miss again
      const again = rounds < 8 ? ready(parts, false, rounds + 1) : undefined;
      return again === undefined ? 0 : again.then(() => 0);
    });
  }

  async function indexDir(node) {
    let count = 0;
    for await (const [name, h] of node.handle.entries()) {
      if (name.endsWith('.crswap')) continue; // Chrome's in-progress createWritable swap files
      if (h.kind === 'directory') {
        const d = mkDir(h);
        addChild(node, name, d);
        count += 1 + (await indexDir(d));
      } else {
        addChild(node, name, mkFile(h));
        count++;
      }
    }
    return count;
  }

  async function init() {
    const t0 = performance.now();
    let entries = 0;
    if (install) {
      // /install from the server's manifest: names, kinds, sizes and mtimes are all known, nothing is opened
      const top = mkDir(null);
      addChild(root, 'install', top);
      for (const f of install.files) {
        const parts = f.path.split('/');
        let d = top;
        for (let i = 0; i < parts.length - 1; i++) {
          let c = d.children.get(parts[i]);
          if (!c) {
            c = mkDir(null);
            addChild(d, parts[i], c);
            entries++;
          }
          d = c;
        }
        const n = mkFile(null);
        n.remote = f;
        n.size = f.size;
        n.mtime = f.mtime;
        addChild(d, parts[parts.length - 1], n);
        entries++;
      }
    }
    for (const name of install ? ['saves'] : ['install', 'saves']) {
      const h = await opfsRoot.getDirectoryHandle(name === 'saves' ? savesDir : name, { create: true });
      const d = mkDir(h);
      addChild(root, name, d);
      entries += await indexDir(d);
    }
    return { entries, ms: performance.now() - t0 };
  }

  /** Sorted entries of a directory node. */
  function list(node) {
    return [...node.children.entries()].map(([name, c]) => ({ name, kind: c.kind, ino: c.ino })).sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0));
  }

  // ---------------------------------------------------------------- shared access handles
  const shared = new Map(); // node -> { access, rw, refs }
  const isOpen = (node) => shared.has(node);
  function anyOpenUnder(node) {
    if (node.kind === 'file') return isOpen(node);
    for (const c of node.children.values()) if (anyOpenUnder(c)) return true;
    return false;
  }

  /** A shared access handle for `node`, readwrite when `rw`. Synchronous when one that fits is already open. */
  const opening = new Map(); // remote node -> Promise of its shared entry (two opens at once share one)
  function acquire(node, rw) {
    if (node.remote) {
      // /install over HTTP: read-only (the write guard refused any write before this)
      const s = shared.get(node);
      if (s) {
        s.refs++;
        return s;
      }
      const wait = opening.get(node);
      if (wait) return wait.then((x) => (x.refs++, x));
      const made = (access) => {
        const n = { access, rw: false, refs: 1 };
        shared.set(node, n);
        return n;
      };
      const got = install.open(node.remote);
      if (!isThenable(got)) return made(got);
      const p = got.then(made).finally(() => opening.delete(node));
      opening.set(node, p);
      return p;
    }
    const s = shared.get(node);
    if (s && (s.rw || !rw)) {
      s.refs++;
      return s;
    }
    return (async () => {
      if (s) {
        // upgrade read -> readwrite: reopen exclusively; descriptors sharing `s` follow
        s.access.close();
        s.access = await node.handle.createSyncAccessHandle();
        s.rw = true;
        s.refs++;
        return s;
      }
      let access;
      if (rw) access = await node.handle.createSyncAccessHandle();
      else {
        try {
          access = await node.handle.createSyncAccessHandle({ mode: 'read-only' });
        } catch {
          access = await node.handle.createSyncAccessHandle();
        }
      }
      const n = { access, rw, refs: 1 };
      shared.set(node, n);
      return n;
    })();
  }

  /** Drop one reference. Returns 0 or -errno. */
  function release(node, s, wrote) {
    s.refs--;
    try {
      if (wrote) {
        node.size = s.access.getSize();
        node.mtime = Date.now();
        announce([node.parent], [node]);
      }
      if (s.refs <= 0) {
        shared.delete(node);
        if (s.rw) s.access.flush();
        s.access.close();
      } else if (wrote) {
        s.access.flush();
      }
      return 0;
    } catch (e) {
      return -errnoFromDomError(e);
    }
  }

  /**
   * Open a file. opts: { write, create, excl, trunc }.
   * Returns { node, s } (s = the shared access handle, one reference taken), -errno, or a Promise of either.
   * A directory gives -EISDIR (callers that accept directories check resolve() first).
   */
  function openFile(parts, { write = false, create = false, excl = false, trunc = false } = {}) {
    if (parts.length === 0) return -ERRNO.EISDIR;
    if (write || create || trunc) {
      const g = guard(parts);
      if (g) return -g;
    }
    const l = lookup(parts);
    if (l.err) return -l.err;
    const rw = write || trunc;
    const finish = (node, s) => {
      if (trunc) {
        try {
          s.access.truncate(0);
        } catch (e) {
          release(node, s, false);
          return -errnoFromDomError(e);
        }
      }
      return { node, s };
    };
    if (l.node) {
      if (create && excl) return -ERRNO.EEXIST;
      if (l.node.kind === 'dir') return -ERRNO.EISDIR;
      const node = l.node;
      try {
        const got = acquire(node, rw);
        return isThenable(got) ? got.then((s) => finish(node, s), (e) => -errnoFromDomError(e)) : finish(node, got);
      } catch (e) {
        return -errnoFromDomError(e);
      }
    }
    if (!create) return -ERRNO.ENOENT;
    return (async () => {
      try {
        const h = await l.parent.handle.getFileHandle(l.wanted, { create: true });
        const node = mkFile(h);
        addChild(l.parent, l.wanted, node);
        announce([l.parent]);
        const s = await acquire(node, true);
        return { node, s };
      } catch (e) {
        return -errnoFromDomError(e);
      }
    })();
  }

  /** Create a directory. recursive: also the missing parents, and an existing directory is fine. */
  function mkdir(parts, { recursive = false } = {}) {
    // an existing entry answers first, as POSIX does: "mkdir /saves" is EEXIST (create_dir_all treats it as done),
    // not EACCES; the write guard only refuses what would change something
    const existing = resolve(parts);
    if (existing) return recursive ? (existing.kind === 'dir' ? 0 : -ERRNO.ENOTDIR) : -ERRNO.EEXIST;
    const g = guard(parts);
    if (g) return -g;
    if (!recursive) {
      const l = lookup(parts);
      if (l.err) return -l.err;
      if (l.node) return -ERRNO.EEXIST;
      return l.parent.handle.getDirectoryHandle(l.wanted, { create: true }).then(
        (h) => {
          addChild(l.parent, l.wanted, mkDir(h));
          announce([l.parent]);
          return 0;
        },
        (e) => -errnoFromDomError(e),
      );
    }
    return (async () => {
      try {
        let node = root;
        for (const part of parts) {
          const c = child(node, part);
          if (c) {
            if (c[1].kind !== 'dir') return -ERRNO.ENOTDIR;
            node = c[1];
            continue;
          }
          const h = await node.handle.getDirectoryHandle(part, { create: true });
          const d = mkDir(h);
          addChild(node, part, d);
          announce([node]);
          node = d;
        }
        return 0;
      } catch (e) {
        return -errnoFromDomError(e);
      }
    })();
  }

  /** Remove. kind: 'file' (unlink: a directory gives EISDIR), 'dir' (rmdir: a file gives ENOTDIR) or 'any'. */
  function remove(parts, kind = 'any') {
    const g = guard(parts);
    if (g) return -g;
    const l = lookup(parts);
    if (l.err) return -l.err;
    if (!l.node) return -ERRNO.ENOENT;
    const node = l.node;
    if (kind === 'file' && node.kind === 'dir') return -ERRNO.EISDIR;
    if (kind === 'dir' && node.kind !== 'dir') return -ERRNO.ENOTDIR;
    if (node.kind === 'dir' && node.children.size > 0) return -ERRNO.ENOTEMPTY;
    if (node.kind === 'file' && isOpen(node)) return -ERRNO.EBUSY;
    return l.parent.handle.removeEntry(l.name).then(
      () => {
        delChild(l.parent, l.name);
        announce([l.parent]);
        return 0;
      },
      (e) => -errnoFromDomError(e),
    );
  }

  /** Move / rename a file, replacing an existing destination file. Directories: ENOTSUP. */
  function rename(aParts, bParts) {
    const ga = guard(aParts);
    if (ga) return -ga;
    const gb = guard(bParts);
    if (gb) return -gb;
    const a = lookup(aParts);
    if (a.err) return -a.err;
    if (!a.node) return -ERRNO.ENOENT;
    const node = a.node;
    if (node.kind === 'dir') return -ERRNO.ENOTSUP;
    if (isOpen(node)) return -ERRNO.EBUSY;
    const b = lookup(bParts);
    if (b.err) return -b.err;
    if (b.node === node && b.name === b.wanted) return 0; // same file, same name
    if (b.node && b.node !== node) {
      if (b.node.kind === 'dir') return -ERRNO.EISDIR;
      if (isOpen(b.node)) return -ERRNO.EBUSY;
    }
    return (async () => {
      try {
        if (b.node && b.node !== node) {
          await b.parent.handle.removeEntry(b.name);
          delChild(b.parent, b.name);
        }
        if (typeof node.handle.move === 'function') {
          await node.handle.move(b.parent.handle, b.wanted);
          delChild(a.parent, a.name);
          addChild(b.parent, b.wanted, node); // the handle now points at the new location
        } else {
          // fallback: copy through sync handles, then remove the source
          const src = await node.handle.createSyncAccessHandle();
          const nh = await b.parent.handle.getFileHandle(b.wanted, { create: true });
          const dst = await nh.createSyncAccessHandle();
          try {
            const size = src.getSize();
            const tmp = new Uint8Array(Math.min(size, 1 << 20) || 1);
            dst.truncate(0);
            for (let at = 0; at < size; ) {
              const n = src.read(tmp, { at });
              if (n <= 0) break;
              dst.write(tmp.subarray(0, n), { at });
              at += n;
            }
            dst.flush();
          } finally {
            src.close();
            dst.close();
          }
          await a.parent.handle.removeEntry(a.name);
          delChild(a.parent, a.name);
          addChild(b.parent, b.wanted, mkFile(nh));
        }
        announce(a.parent === b.parent ? [a.parent] : [a.parent, b.parent]);
        return 0;
      } catch (e) {
        return -errnoFromDomError(e);
      }
    })();
  }

  /**
   * { size, mtime } of a file node (mtime in ms since 1970, 0 when unknown). Synchronous when the file is open or
   * was stat'ed / written before (cached on the node), else a Promise (File.size / lastModified).
   */
  function fileStat(node) {
    const s = shared.get(node);
    if (s) {
      try {
        node.size = s.access.getSize();
      } catch { /* keep the cached size */ }
    }
    if (node.size !== undefined) return { size: node.size, mtime: node.mtime ?? 0 };
    return node.handle.getFile().then(
      (f) => {
        node.size = f.size;
        node.mtime = f.lastModified;
        return { size: f.size, mtime: f.lastModified };
      },
      (e) => -errnoFromDomError(e),
    );
  }

  return { root, init, resolve, lookup, child, list, acquire, release, openFile, mkdir, remove, rename, isOpen, anyOpenUnder, fileStat, guard, shared, ready, refreshDir, refreshSaves, stats, close: () => channel?.close?.() };
}

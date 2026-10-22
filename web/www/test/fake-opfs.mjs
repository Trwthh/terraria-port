// An in-memory stand-in for OPFS with the same async / sync split as the real API: directory operations and
// createSyncAccessHandle are async, the access handle itself is synchronous. Used by the host and WASI tests.

const domErr = (name) => Object.assign(new Error(name), { name });

export class FakeFile {
  constructor(name, bytes = new Uint8Array(0)) {
    this.kind = 'file';
    this.name = name;
    this.bytes = bytes;
    this.locked = 0; // 0 free, 'rw' exclusive, n>0 readers
    this.parent = null;
  }
  async createSyncAccessHandle(opts = {}) {
    const ro = opts.mode === 'read-only';
    const unsafe = opts.mode === 'readwrite-unsafe'; // shared writers (Chrome 121+): only with other such handles
    if (unsafe) {
      if (this.locked !== 0 && this.locked !== 'unsafe' && !(typeof this.locked === 'string' && this.locked.startsWith('unsafe'))) throw domErr('NoModificationAllowedError');
      this.unsafeCount = (this.unsafeCount ?? 0) + 1;
      this.locked = 'unsafe';
    } else {
      if (this.locked === 'rw' || this.locked === 'unsafe' || (!ro && this.locked)) throw domErr('NoModificationAllowedError');
      this.locked = ro ? (this.locked || 0) + 1 : 'rw';
    }
    const f = this;
    let open = true;
    const live = () => { if (!open) throw domErr('InvalidStateError'); };
    return {
      read(view, { at = 0 } = {}) {
        live();
        const n = Math.max(0, Math.min(view.length, f.bytes.length - at));
        view.set(f.bytes.subarray(at, at + n));
        return n;
      },
      write(view, { at = 0 } = {}) {
        live();
        f.writes = (f.writes ?? 0) + 1;
        if (ro) throw domErr('NoModificationAllowedError');
        const end = at + view.length;
        if (end > f.bytes.length) {
          const nb = new Uint8Array(end);
          nb.set(f.bytes);
          f.bytes = nb;
        }
        f.bytes.set(view, at);
        return view.length;
      },
      getSize() { live(); return f.bytes.length; },
      truncate(n) { live(); f.bytes = f.bytes.slice(0, n); },
      flush() { live(); },
      close() {
        if (!open) return;
        open = false;
        if (unsafe) {
          f.unsafeCount--;
          if (!f.unsafeCount) f.locked = 0;
        } else f.locked = ro ? f.locked - 1 : 0;
      },
    };
  }
  async getFile() {
    const bytes = this.bytes.slice();
    return { size: bytes.length, lastModified: this.mtime ?? Date.now(), text: async () => new TextDecoder().decode(bytes), arrayBuffer: async () => bytes.buffer };
  }
  async move(dir, name) {
    this.parent.map.delete(this.name);
    this.name = name;
    this.parent = dir;
    dir.map.set(name, this);
  }
}

export class FakeDir {
  constructor(name) {
    this.kind = 'directory';
    this.name = name;
    this.map = new Map();
  }
  async *entries() {
    for (const e of [...this.map.entries()]) yield e;
  }
  async getDirectoryHandle(name, { create = false } = {}) {
    const e = this.map.get(name);
    if (e) {
      if (e.kind !== 'directory') throw domErr('TypeMismatchError');
      return e;
    }
    if (!create) throw domErr('NotFoundError');
    const d = new FakeDir(name);
    this.map.set(name, d);
    return d;
  }
  async getFileHandle(name, { create = false } = {}) {
    const e = this.map.get(name);
    if (e) {
      if (e.kind !== 'file') throw domErr('TypeMismatchError');
      return e;
    }
    if (!create) throw domErr('NotFoundError');
    const f = new FakeFile(name);
    f.parent = this;
    this.map.set(name, f);
    return f;
  }
  async removeEntry(name) {
    const e = this.map.get(name);
    if (!e) throw domErr('NotFoundError');
    if (e.kind === 'directory' && e.map.size) throw domErr('InvalidModificationError');
    this.map.delete(name);
  }
  addFile(name, text) {
    const f = new FakeFile(name, new TextEncoder().encode(text));
    f.parent = this;
    this.map.set(name, f);
    return f;
  }
}

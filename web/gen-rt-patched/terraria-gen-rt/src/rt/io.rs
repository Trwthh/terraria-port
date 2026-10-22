//! `System.IO` streams and binary readers / writers as natively mapped classes (`tools/cs2rs/native.d/rt_io.json`).
//! This module is also the worked example of `tools/cs2rs/NATIVE_SCHEMA.md` ("a library class as a native Rust type").
//!
//! One Rust type, [`Stream`], stands for the whole C# hierarchy `Stream` / `MemoryStream` / `FileStream`: a
//! `Gc<MemoryStream>` passed where the C# code expects a `Stream` is the same pointer, so the pointee must be
//! the same Rust type whatever the static C# type is.
//!
//! A `MemoryStream` over a caller's array (`new MemoryStream(byte[])` and the range overloads) WRAPS that array, as
//! in .NET: writes through the stream land in the array, later changes of the array are seen by reads, and the
//! stream cannot grow (`CachedBuffer`, `MessageBuffer`: a `BinaryWriter` builds a packet in a shared `byte[]`).
//!
//! Deviations from .NET, on purpose:
//! - A `FileStream` is a memory buffer with a path: the file is read when the stream is opened and written back
//!   by `Flush` / `Close` / `Dispose` (and when the collector frees an unflushed stream). Nothing else in the
//!   process reads a file while the game has it open for writing, so the difference cannot be observed.
//! - `Read*` past the end throws `EndOfStreamException` with .NET's message; other messages are shortened.

use super::exc::{exc, throw};
use super::gc::{alloc_init, null_reference, Gc, Marker, Trace};
use super::string::Str;
use super::Arr;

#[cold]
fn end_of_stream() -> ! {
    throw(exc("System.IO.EndOfStreamException", Str::lit("Unable to read beyond the end of the stream.")))
}
#[cold]
fn disposed() -> ! {
    throw(exc("System.ObjectDisposedException", Str::lit("Cannot access a closed Stream.")))
}
#[cold]
fn not_supported(what: &'static str) -> ! {
    throw(exc("System.NotSupportedException", Str::lit(what)))
}
#[cold]
fn io_error(what: String) -> ! {
    throw(exc("System.IO.IOException", Str::new(&what)))
}
#[cold]
fn arg(kind: &'static str, what: &'static str) -> ! {
    throw(exc(kind, Str::lit(what)))
}

/// `System.IO.SeekOrigin`.
pub mod seek_origin {
    pub const BEGIN: i32 = 0;
    pub const CURRENT: i32 = 1;
    pub const END: i32 = 2;
}

/// `System.IO.FileMode` / `FileAccess` values the game passes.
pub mod file_mode {
    pub const CREATE_NEW: i32 = 1;
    pub const CREATE: i32 = 2;
    pub const OPEN: i32 = 3;
    pub const OPEN_OR_CREATE: i32 = 4;
    pub const TRUNCATE: i32 = 5;
    pub const APPEND: i32 = 6;
}

/// `Stream`, `MemoryStream` and `FileStream`.
pub struct Stream {
    /// The bytes of a stream that owns its storage (a `FileStream`, `new MemoryStream()`, host data). Unused
    /// (empty) while `wrap` is set.
    buf: Vec<u8>,
    /// `new MemoryStream(byte[] ...)`: the caller's array, the first byte of the stream in it, and the current
    /// length of the stream (`MemoryStream._buffer`, `_origin`, `_length - _origin`). Null otherwise.
    wrap: Arr<u8>,
    wrap_origin: usize,
    wrap_len: usize,
    /// `_capacity - _origin` of a wrapped stream: the `count` it was created with.
    wrap_cap: usize,
    /// `GetBuffer()` is allowed (`publiclyVisible`, or a stream that owns its buffer).
    exposable: bool,
    pos: usize,
    readable: bool,
    writable: bool,
    /// `new MemoryStream(byte[])` cannot grow.
    fixed: bool,
    closed: bool,
    /// `Some` for a `FileStream`: where `flush` writes the buffer.
    path: Option<std::path::PathBuf>,
    dirty: bool,
    /// Runs once, when the stream is closed (after the last flush): the host's wrappers that have to finish their
    /// work then (`CryptoStream`: the final block; a compressing `DeflateStream`: the compressed bytes to the inner
    /// stream). The hook gets the stream's bytes; the handles it needs are reported through `close_roots`.
    on_close: Option<Box<dyn FnMut(&[u8])>>,
    /// Handles the close hook uses (the inner stream of a wrapper, a transform object): kept alive with the stream.
    close_roots: Vec<Gc<super::gc::Object>>,
    /// The stream a wrapper closes when it is closed (`CryptoStream.Dispose`, `DeflateStream.Dispose` without
    /// `leaveOpen`). Null otherwise.
    close_inner: Gc<Stream>,
    /// Which host wrapper this is (0: none), for `x is CryptoStream`.
    tag: u32,
}

// SAFETY: the wrapped array and the hook's handles are reported.
unsafe impl Trace for Stream {
    /// `Dispose()` through `IDisposable`: `Close()`.
    unsafe fn obj_dispose(this: *mut Self) {
        (*this).close()
    }
    unsafe fn trace(&self, m: &mut Marker) {
        self.wrap.trace(m);
        for r in &self.close_roots {
            m.mark(*r)
        }
        m.mark(self.close_inner);
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        // what the FileStream finalizer does
        if self.dirty && !self.closed {
            if let Some(p) = &self.path {
                let _ = std::fs::write(p, &self.buf);
            }
        }
    }
}

/// Folders no stream of this runtime may write below (the game's install folder: read-only by contract). Process
/// wide, set by the host before any translated code runs.
static READ_ONLY_ROOTS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

fn norm_path(p: &std::path::Path) -> String {
    let abs = if p.is_absolute() { p.to_path_buf() } else { std::env::current_dir().map(|c| c.join(p)).unwrap_or_else(|_| p.to_path_buf()) };
    // lexical normalisation: `..` and `.` resolved without touching the file system, separators and case folded
    let mut parts: Vec<String> = Vec::new();
    for c in abs.to_string_lossy().replace('/', "\\").split('\\') {
        match c {
            "" | "." => {}
            ".." => {
                if parts.len() > 1 {
                    parts.pop();
                }
            }
            c => parts.push(c.to_ascii_lowercase()),
        }
    }
    parts.join("\\")
}

/// Declares `root` read-only for every file stream of the runtime: opening a file below it for writing (any mode
/// that can create, truncate or write) throws `UnauthorizedAccessException`, and so does a flush.
pub fn add_read_only_root(root: &std::path::Path) {
    let n = norm_path(root);
    let mut r = match READ_ONLY_ROOTS.lock() {
        Ok(r) => r,
        Err(p) => p.into_inner(),
    };
    if !n.is_empty() && !r.contains(&n) {
        r.push(n);
    }
}

/// Is `p` inside a read-only root?
pub fn is_read_only_path(p: &std::path::Path) -> bool {
    let n = norm_path(p);
    let r = match READ_ONLY_ROOTS.lock() {
        Ok(r) => r,
        Err(p) => p.into_inner(),
    };
    r.iter().any(|root| n == *root || n.starts_with(&(root.clone() + "\\")))
}

fn refuse_write(p: &std::path::Path) -> ! {
    throw(exc("System.UnauthorizedAccessException", Str::new(&format!("Access to the path '{}' is denied: the runtime does not write inside a read-only root (the game's install folder).", p.display()))))
}

impl Stream {
    fn memory(buf: Vec<u8>, writable: bool, fixed: bool) -> Stream {
        Stream {
            buf,
            wrap: Arr::NULL,
            wrap_origin: 0,
            wrap_len: 0,
            wrap_cap: 0,
            exposable: true,
            pos: 0,
            readable: true,
            writable,
            fixed,
            closed: false,
            path: None,
            dirty: false,
            on_close: None,
            close_roots: Vec::new(),
            close_inner: Gc::NULL,
            tag: 0,
        }
    }

    /// `new MemoryStream()`.
    pub fn new_memory() -> Gc<Stream> {
        alloc_init(Stream::memory(Vec::new(), true, false))
    }

    /// `new MemoryStream(capacity)`: an empty, growing stream; the capacity is reserved.
    pub fn new_memory_capacity(capacity: i32) -> Gc<Stream> {
        if capacity < 0 {
            arg("System.ArgumentOutOfRangeException", "capacity");
        }
        alloc_init(Stream::memory(Vec::with_capacity(capacity as usize), true, false))
    }

    /// `new MemoryStream(byte[])`: the stream IS the array (writable, not growable, `GetBuffer` refused).
    pub fn from_bytes(bytes: Arr<u8>) -> Gc<Stream> {
        if bytes.is_null() {
            arg("System.ArgumentNullException", "buffer");
        }
        Stream::from_bytes_range(bytes, 0, bytes.len(), true, false)
    }

    /// `new MemoryStream(byte[], writable)`.
    pub fn from_bytes_writable(bytes: Arr<u8>, writable: bool) -> Gc<Stream> {
        if bytes.is_null() {
            arg("System.ArgumentNullException", "buffer");
        }
        Stream::from_bytes_range(bytes, 0, bytes.len(), writable, false)
    }

    /// `new MemoryStream(buffer, index, count, writable, publiclyVisible)`: a fixed-size stream over that part of
    /// the array, wrapping it.
    pub fn from_bytes_range(bytes: Arr<u8>, index: i32, count: i32, writable: bool, publicly_visible: bool) -> Gc<Stream> {
        if bytes.is_null() {
            arg("System.ArgumentNullException", "buffer");
        }
        if index < 0 || count < 0 {
            arg("System.ArgumentOutOfRangeException", if index < 0 { "index" } else { "count" });
        }
        if bytes.len() - index < count {
            arg("System.ArgumentException", "Offset and length were out of bounds for the array or count is greater than the number of elements from index to the end of the source collection.");
        }
        let mut s = Stream::memory(Vec::new(), writable, true);
        s.wrap = bytes;
        s.wrap_origin = index as usize;
        s.wrap_len = count as usize;
        s.wrap_cap = count as usize;
        s.exposable = publicly_visible;
        alloc_init(s)
    }

    /// A read-only stream over bytes the host owns (embedded resources).
    pub fn from_vec(bytes: Vec<u8>) -> Gc<Stream> {
        alloc_init(Stream::memory(bytes, false, true))
    }

    /// A stream that collects what is written to it and hands the bytes to `hook` when it is closed (once): the
    /// host's write-side wrappers (`CryptoStream` in write mode, a compressing `DeflateStream`). `roots` are the
    /// handles the hook uses.
    pub fn new_sink(hook: Box<dyn FnMut(&[u8])>, roots: Vec<Gc<super::gc::Object>>) -> Gc<Stream> {
        let mut s = Stream::memory(Vec::new(), true, false);
        s.readable = false;
        s.on_close = Some(hook);
        s.close_roots = roots;
        alloc_init(s)
    }
    /// Makes this stream a wrapper of `inner`: closing it closes `inner` too. `tag` names the wrapper class.
    pub fn set_wrapper(&mut self, inner: Gc<Stream>, tag: u32) {
        self.close_inner = inner;
        self.tag = tag;
    }
    pub fn tag(&self) -> u32 {
        self.tag
    }

    /// Runs the close hook now if it has not run (`CryptoStream.FlushFinalBlock`).
    pub fn finish(&mut self) {
        if let Some(mut h) = self.on_close.take() {
            let data = core::mem::take(&mut self.buf);
            h(&data);
            self.buf = data;
            self.close_roots.clear();
        }
    }
    /// Has this stream a close hook that did not run yet?
    pub fn has_close_hook(&self) -> bool {
        self.on_close.is_some()
    }

    /// The stream's bytes, `[0, Length)`.
    fn data(&self) -> &[u8] {
        if self.wrap.is_null() {
            &self.buf
        } else if self.wrap_len == 0 {
            &[]
        } else {
            // SAFETY: `[origin, origin + cap)` was checked against the array when the stream was created, and
            // `wrap_len <= wrap_cap`; arrays never shrink or move.
            unsafe { core::slice::from_raw_parts(self.wrap.at(0).add(self.wrap_origin), self.wrap_len) }
        }
    }
    fn len_usize(&self) -> usize {
        if self.wrap.is_null() {
            self.buf.len()
        } else {
            self.wrap_len
        }
    }
    /// `MemoryStream.SetLength` / the growth inside `Write`: bytes between the old and the new length are zero.
    fn resize(&mut self, n: usize) {
        if self.wrap.is_null() {
            if self.fixed && n > self.buf.len() {
                not_supported("Memory stream is not expandable.");
            }
            self.buf.resize(n, 0);
        } else {
            if n > self.wrap_cap {
                not_supported("Memory stream is not expandable.");
            }
            if n > self.wrap_len {
                // SAFETY: inside the wrapped range (n <= cap).
                unsafe { core::ptr::write_bytes(self.wrap.at(0).add(self.wrap_origin + self.wrap_len), 0, n - self.wrap_len) };
            }
            self.wrap_len = n;
        }
    }

    /// `new FileStream(path, mode)` / `File.Open*`: see the module comment for the buffering rule.
    pub fn open_file(path: Str, mode: i32, read: bool, write: bool) -> Gc<Stream> {
        if path.is_null() {
            arg("System.ArgumentNullException", "path");
        }
        let p = std::path::PathBuf::from(path.as_str());
        // the install folder is read-only: nothing that can create, truncate or write a file may open below it
        if (write || !matches!(mode, file_mode::OPEN)) && is_read_only_path(&p) {
            refuse_write(&p);
        }
        let exists = p.is_file();
        let not_found = || -> ! { throw(exc("System.IO.FileNotFoundException", Str::new(&format!("Could not find file '{}'.", p.display())))) };
        let mut buf = Vec::new();
        let mut dirty = false;
        match mode {
            file_mode::CREATE_NEW => {
                if exists {
                    io_error(format!("The file '{}' already exists.", p.display()));
                }
                dirty = true;
            }
            file_mode::CREATE => dirty = true,
            file_mode::OPEN | file_mode::TRUNCATE | file_mode::APPEND | file_mode::OPEN_OR_CREATE => {
                if !exists {
                    if mode == file_mode::OPEN || mode == file_mode::TRUNCATE {
                        not_found();
                    }
                    dirty = true;
                } else if mode == file_mode::TRUNCATE {
                    dirty = true;
                } else {
                    buf = match std::fs::read(&p) {
                        Ok(b) => b,
                        Err(e) => io_error(format!("{}: {e}", p.display())),
                    };
                }
            }
            _ => arg("System.ArgumentOutOfRangeException", "mode"),
        }
        if dirty {
            // the file exists (empty) from the moment it is created, as in .NET
            if let Err(e) = std::fs::write(&p, b"") {
                if p.parent().map(|d| !d.as_os_str().is_empty() && !d.is_dir()).unwrap_or(false) {
                    throw(exc("System.IO.DirectoryNotFoundException", Str::new(&format!("Could not find a part of the path '{}'.", p.display()))));
                }
                io_error(format!("{}: {e}", p.display()));
            }
        }
        let pos = if mode == file_mode::APPEND { buf.len() } else { 0 };
        let mut s = Stream::memory(buf, write, false);
        s.pos = pos;
        s.readable = read;
        s.path = Some(p);
        alloc_init(s)
    }

    fn open(&self) {
        if self.closed {
            disposed();
        }
    }

    pub fn can_read(&self) -> bool {
        self.readable && !self.closed
    }
    pub fn can_write(&self) -> bool {
        self.writable && !self.closed
    }
    pub fn can_seek(&self) -> bool {
        !self.closed
    }
    pub fn length(&self) -> i64 {
        self.open();
        self.len_usize() as i64
    }
    /// `MemoryStream.Capacity`.
    pub fn capacity(&self) -> i32 {
        self.open();
        if self.wrap.is_null() {
            self.buf.capacity().min(i32::MAX as usize) as i32
        } else {
            self.wrap_cap as i32
        }
    }
    pub fn position(&self) -> i64 {
        self.open();
        self.pos as i64
    }
    pub fn set_position(&mut self, v: i64) {
        self.open();
        if v < 0 {
            arg("System.ArgumentOutOfRangeException", "value");
        }
        self.pos = v as usize;
    }
    pub fn seek(&mut self, offset: i64, origin: i32) -> i64 {
        self.open();
        let base = match origin {
            seek_origin::BEGIN => 0,
            seek_origin::CURRENT => self.pos as i64,
            seek_origin::END => self.len_usize() as i64,
            _ => arg("System.ArgumentException", "origin"),
        };
        let p = base + offset;
        if p < 0 {
            io_error("An attempt was made to move the position before the beginning of the stream.".to_owned());
        }
        self.pos = p as usize;
        p
    }
    pub fn set_length(&mut self, v: i64) {
        self.open();
        if !self.writable {
            not_supported("Stream does not support writing.");
        }
        if v < 0 || v > i32::MAX as i64 {
            arg("System.ArgumentOutOfRangeException", "value");
        }
        self.resize(v as usize);
        if self.pos > self.len_usize() {
            self.pos = self.len_usize();
        }
        self.dirty = true;
    }

    /// Up to `out.len()` bytes; 0 at the end.
    pub fn read_slice(&mut self, out: &mut [u8]) -> usize {
        self.open();
        if !self.readable {
            not_supported("Stream does not support reading.");
        }
        let data = self.data();
        let avail = data.len().saturating_sub(self.pos);
        let n = avail.min(out.len());
        if n == 0 {
            // also a position behind the end (allowed in .NET): nothing to read
            return 0;
        }
        out[..n].copy_from_slice(&data[self.pos..self.pos + n]);
        self.pos += n;
        n
    }
    pub fn write_slice(&mut self, data: &[u8]) {
        self.open();
        if !self.writable {
            not_supported("Stream does not support writing.");
        }
        let end = self.pos + data.len();
        if end > self.len_usize() {
            self.resize(end);
        }
        if self.wrap.is_null() {
            self.buf[self.pos..end].copy_from_slice(data);
        } else if !data.is_empty() {
            // SAFETY: `end <= wrap_len <= wrap_cap`, inside the wrapped range; `copy` because `data` may be a
            // snapshot of the same array.
            unsafe { core::ptr::copy(data.as_ptr(), self.wrap.at(0).add(self.wrap_origin + self.pos), data.len()) };
        }
        self.pos = end;
        self.dirty = true;
    }

    /// `stream.Read(buffer, offset, count)`.
    pub fn read(&mut self, buffer: Arr<u8>, offset: i32, count: i32) -> i32 {
        let n = check_range(buffer, offset, count);
        // SAFETY: the range was checked against the array; the slice lives for this call only.
        let out = unsafe { core::slice::from_raw_parts_mut(buffer.at(0).wrapping_add(offset as usize), n) };
        self.read_slice(out) as i32
    }
    /// `stream.Write(buffer, offset, count)`.
    pub fn write(&mut self, buffer: Arr<u8>, offset: i32, count: i32) {
        let n = check_range(buffer, offset, count);
        let data = buffer.as_slice()[offset as usize..offset as usize + n].to_vec();
        self.write_slice(&data);
    }
    /// `stream.ReadByte()`: -1 at the end.
    pub fn read_byte(&mut self) -> i32 {
        let mut b = [0u8];
        if self.read_slice(&mut b) == 0 {
            -1
        } else {
            b[0] as i32
        }
    }
    pub fn write_byte(&mut self, v: u8) {
        self.write_slice(&[v]);
    }
    pub fn flush(&mut self) {
        if self.dirty {
            if let Some(p) = &self.path {
                if let Err(e) = std::fs::write(p, &self.buf) {
                    io_error(format!("{}: {e}", p.display()));
                }
            }
            self.dirty = false;
        }
    }
    /// `Close()` / `Dispose()`: idempotent.
    pub fn close(&mut self) {
        if !self.closed {
            self.flush();
            self.finish();
            self.closed = true;
            let inner = core::mem::replace(&mut self.close_inner, Gc::NULL);
            if !inner.is_null() && inner.0 != self as *mut Stream {
                // SAFETY: a live stream (reported to the collector by `trace`), not this one.
                unsafe { (*inner.ptr()).close() }
            }
        }
    }
    /// `MemoryStream.ToArray()` (works on a closed stream, as in .NET).
    pub fn to_array(&self) -> Arr<u8> {
        Arr::from_slice(self.data())
    }
    /// `MemoryStream.GetBuffer()`: the array behind the stream, not a copy (its length is the capacity, not the
    /// stream's length). A stream over a caller's array returns that array, unless it was created without
    /// `publiclyVisible` (`UnauthorizedAccessException`). A stream that owned its bytes moves them into an array of
    /// its capacity (at least 256, as .NET's first growth) and wraps it from then on.
    pub fn get_buffer(&mut self) -> Arr<u8> {
        if !self.exposable {
            throw(exc("System.UnauthorizedAccessException", Str::lit("MemoryStream's internal buffer cannot be accessed.")));
        }
        if self.wrap.is_null() {
            if self.path.is_some() {
                not_supported("GetBuffer on a FileStream");
            }
            let len = self.buf.len();
            let cap = self.buf.capacity().max(len).max(if self.fixed { len } else { 256 });
            let a = Arr::<u8>::new(cap as i32);
            if len != 0 {
                // SAFETY: `cap >= len`; a fresh array.
                unsafe { core::ptr::copy_nonoverlapping(self.buf.as_ptr(), a.at(0), len) };
            }
            self.buf = Vec::new();
            self.wrap = a;
            self.wrap_origin = 0;
            self.wrap_len = len;
            self.wrap_cap = cap;
            // .NET would grow such a stream; this one cannot any more (see the tests): a write past the capacity
            // after GetBuffer() throws NotSupportedException instead of silently detaching the returned array.
            self.fixed = true;
        }
        self.wrap
    }
    /// The bytes written so far, for hosts and tests.
    pub fn bytes(&self) -> &[u8] {
        self.data()
    }
    pub fn is_file(&self) -> bool {
        self.path.is_some()
    }
}

fn check_range(buffer: Arr<u8>, offset: i32, count: i32) -> usize {
    if buffer.is_null() {
        arg("System.ArgumentNullException", "buffer");
    }
    if offset < 0 || count < 0 {
        arg("System.ArgumentOutOfRangeException", "offset / count");
    }
    if buffer.len() - offset < count {
        arg("System.ArgumentException", "Offset and length were out of bounds for the array.");
    }
    count as usize
}

/// `System.IO.BinaryReader` (UTF-8, little endian: the only configuration the game uses).
#[derive(Clone, Copy)]
pub struct BinaryReader {
    pub stream: Gc<Stream>,
    leave_open: bool,
}
// SAFETY: the one handle is reported.
unsafe impl Trace for BinaryReader {
    /// `Dispose()` through `IDisposable`: `Close()`.
    unsafe fn obj_dispose(this: *mut Self) {
        (*this).close()
    }
    unsafe fn trace(&self, m: &mut Marker) {
        m.mark(self.stream)
    }
}

impl BinaryReader {
    /// `new BinaryReader(stream)`.
    pub fn new_gc(stream: Gc<Stream>) -> Gc<BinaryReader> {
        if stream.is_null() {
            arg("System.ArgumentNullException", "input");
        }
        // SAFETY: checked non-null above; live runtime object.
        if !unsafe { (*stream.ptr()).can_read() } {
            arg("System.ArgumentException", "Stream was not readable.");
        }
        alloc_init(BinaryReader { stream, leave_open: false })
    }

    fn s(&self) -> &mut Stream {
        if self.stream.is_null() {
            disposed();
        }
        // SAFETY: a live stream object; the reference lives for one read and nothing else touches the stream meanwhile.
        unsafe { &mut *self.stream.ptr() }
    }
    fn fill<const N: usize>(&self) -> [u8; N] {
        let mut b = [0u8; N];
        if self.s().read_slice(&mut b) != N {
            end_of_stream();
        }
        b
    }

    pub fn base_stream(&self) -> Gc<Stream> {
        self.stream
    }
    pub fn read_byte(&mut self) -> u8 {
        self.fill::<1>()[0]
    }
    pub fn read_sbyte(&mut self) -> i8 {
        self.fill::<1>()[0] as i8
    }
    pub fn read_boolean(&mut self) -> bool {
        self.fill::<1>()[0] != 0
    }
    pub fn read_i16(&mut self) -> i16 {
        i16::from_le_bytes(self.fill())
    }
    pub fn read_u16(&mut self) -> u16 {
        u16::from_le_bytes(self.fill())
    }
    pub fn read_i32(&mut self) -> i32 {
        i32::from_le_bytes(self.fill())
    }
    pub fn read_u32(&mut self) -> u32 {
        u32::from_le_bytes(self.fill())
    }
    pub fn read_i64(&mut self) -> i64 {
        i64::from_le_bytes(self.fill())
    }
    pub fn read_u64(&mut self) -> u64 {
        u64::from_le_bytes(self.fill())
    }
    pub fn read_f32(&mut self) -> f32 {
        f32::from_le_bytes(self.fill())
    }
    pub fn read_f64(&mut self) -> f64 {
        f64::from_le_bytes(self.fill())
    }
    /// `Read7BitEncodedInt()`: `FormatException` after five bytes with the continuation bit.
    pub fn read_7bit_encoded_int(&mut self) -> i32 {
        let mut count: u32 = 0;
        let mut shift = 0;
        loop {
            if shift == 35 {
                throw(exc("System.FormatException", Str::lit("Too many bytes in what should have been a 7 bit encoded Int32.")));
            }
            let b = self.read_byte();
            count |= ((b & 0x7F) as u32) << shift;
            shift += 7;
            if b & 0x80 == 0 {
                return count as i32;
            }
        }
    }
    /// `ReadString()`: 7-bit length prefix, UTF-8 bytes (invalid sequences become U+FFFD, as .NET's decoder does).
    pub fn read_string(&mut self) -> Str {
        let n = self.read_7bit_encoded_int();
        if n < 0 {
            io_error("BinaryReader encountered an invalid string length.".to_owned());
        }
        let mut bytes = vec![0u8; n as usize];
        if self.s().read_slice(&mut bytes) != n as usize {
            end_of_stream();
        }
        Str::new(&String::from_utf8_lossy(&bytes))
    }
    /// `ReadBytes(count)`: fewer bytes at the end of the stream, no exception.
    pub fn read_bytes(&mut self, count: i32) -> Arr<u8> {
        if count < 0 {
            arg("System.ArgumentOutOfRangeException", "count");
        }
        let mut bytes = vec![0u8; count as usize];
        let n = self.s().read_slice(&mut bytes);
        Arr::from_slice(&bytes[..n])
    }
    /// `Read(buffer, index, count)`.
    pub fn read(&mut self, buffer: Arr<u8>, index: i32, count: i32) -> i32 {
        self.s().read(buffer, index, count)
    }
    /// `ReadChar()`: one UTF-8 encoded character below U+10000 (the game writes ASCII).
    pub fn read_char(&mut self) -> u16 {
        let b0 = self.read_byte();
        let (extra, mut cp) = match b0 {
            0x00..=0x7F => (0, b0 as u32),
            0xC0..=0xDF => (1, (b0 & 0x1F) as u32),
            0xE0..=0xEF => (2, (b0 & 0x0F) as u32),
            _ => arg("System.ArgumentException", "The output char buffer is too small to contain the decoded characters."),
        };
        for _ in 0..extra {
            cp = (cp << 6) | (self.read_byte() & 0x3F) as u32;
        }
        cp as u16
    }
    /// `Close()` / `Dispose()`: closes the stream too.
    pub fn close(&mut self) {
        if !self.stream.is_null() && !self.leave_open {
            self.s().close();
        }
        self.stream = Gc::NULL;
    }
}

/// `System.IO.BinaryWriter` (UTF-8, little endian).
#[derive(Clone, Copy)]
pub struct BinaryWriter {
    pub stream: Gc<Stream>,
}
// SAFETY: the one handle is reported.
unsafe impl Trace for BinaryWriter {
    /// `Dispose()` through `IDisposable`: `Close()`.
    unsafe fn obj_dispose(this: *mut Self) {
        (*this).close()
    }
    unsafe fn trace(&self, m: &mut Marker) {
        m.mark(self.stream)
    }
}

/// What `BinaryWriter.Write` accepts: one method per overload in C#, one trait here.
pub trait BinWrite {
    fn put(self, w: &mut Stream);
}
macro_rules! bin_write {
    ($($t:ty),*) => {$( impl BinWrite for $t { fn put(self, w: &mut Stream) { w.write_slice(&self.to_le_bytes()) } } )*};
}
bin_write!(u8, i8, i16, u16, i32, u32, i64, u64, f32, f64);
impl BinWrite for bool {
    fn put(self, w: &mut Stream) {
        w.write_slice(&[self as u8])
    }
}
impl BinWrite for Str {
    fn put(self, w: &mut Stream) {
        if self.is_null() {
            arg("System.ArgumentNullException", "value");
        }
        let b = self.as_str().as_bytes();
        let mut n = b.len() as u32;
        while n >= 0x80 {
            w.write_slice(&[(n as u8) | 0x80]);
            n >>= 7;
        }
        w.write_slice(&[n as u8]);
        w.write_slice(b);
    }
}
impl BinWrite for Arr<u8> {
    fn put(self, w: &mut Stream) {
        if self.is_null() {
            arg("System.ArgumentNullException", "buffer");
        }
        let v = self.as_slice().to_vec();
        w.write_slice(&v)
    }
}

impl BinaryWriter {
    /// `new BinaryWriter(stream)`.
    pub fn new_gc(stream: Gc<Stream>) -> Gc<BinaryWriter> {
        if stream.is_null() {
            arg("System.ArgumentNullException", "output");
        }
        // SAFETY: checked non-null above.
        if !unsafe { (*stream.ptr()).can_write() } {
            arg("System.ArgumentException", "Stream was not writable.");
        }
        alloc_init(BinaryWriter { stream })
    }
    fn s(&self) -> &mut Stream {
        if self.stream.is_null() {
            null_reference();
        }
        // SAFETY: as `BinaryReader::s`.
        unsafe { &mut *self.stream.ptr() }
    }
    pub fn base_stream(&self) -> Gc<Stream> {
        self.s().flush();
        self.stream
    }
    /// Every `Write(x)` overload except `Write(char)`.
    pub fn write<T: BinWrite>(&mut self, v: T) {
        v.put(self.s())
    }
    /// `Write(char)`: UTF-8 (a lone surrogate throws in .NET; cannot occur here, see `Str`).
    pub fn write_char(&mut self, c: u16) {
        let mut b = [0u8; 4];
        let ch = char::from_u32(c as u32).unwrap_or_else(|| arg("System.ArgumentException", "Surrogates are not allowed as a single char."));
        let s = ch.encode_utf8(&mut b);
        self.s().write_slice(s.as_bytes());
    }
    /// `Write(buffer, index, count)`.
    pub fn write_range(&mut self, buffer: Arr<u8>, index: i32, count: i32) {
        self.s().write(buffer, index, count)
    }
    pub fn seek(&mut self, offset: i32, origin: i32) -> i64 {
        self.s().seek(offset as i64, origin)
    }
    pub fn flush(&mut self) {
        self.s().flush()
    }
    /// `Close()` / `Dispose()`.
    pub fn close(&mut self) {
        self.s().close()
    }
}

#[cfg(test)]
mod write_guard_tests {
    use super::*;

    #[test]
    fn nothing_writes_below_a_read_only_root() {
        let dir = std::env::temp_dir().join(format!("cs2rs-guard-{}", std::process::id()));
        let ro = dir.join("Install");
        std::fs::create_dir_all(ro.join("Content")).unwrap();
        std::fs::write(ro.join("Content").join("a.bin"), b"abc").unwrap();
        add_read_only_root(&ro);
        let p = |rel: &str| Str::new(&ro.join(rel).to_string_lossy());
        // reading is fine
        let s = Stream::open_file(p("Content/a.bin"), file_mode::OPEN, true, false);
        assert_eq!(unsafe { (*s.ptr()).length() }, 3);
        let refused = |f: &dyn Fn()| {
            let r = crate::rt::try_(|| -> crate::rt::Flow<()> {
                f();
                crate::rt::Flow::Next
            });
            match r {
                Err(e) => crate::rt::exc_is(e, "System.UnauthorizedAccessException"),
                Ok(_) => false,
            }
        };
        assert!(refused(&|| { let _ = Stream::open_file(p("Content/new.bin"), file_mode::CREATE, false, true); }));
        assert!(refused(&|| { let _ = Stream::open_file(p("Content/a.bin"), file_mode::OPEN, true, true); }));
        assert!(refused(&|| { let _ = Stream::open_file(p("Content/a.bin"), file_mode::TRUNCATE, true, false); }));
        assert!(refused(&|| { let _ = Stream::open_file(p("Content/../Content/b.bin"), file_mode::OPEN_OR_CREATE, true, true); }));
        assert!(!ro.join("Content").join("new.bin").exists() && !ro.join("Content").join("b.bin").exists());
        assert_eq!(std::fs::read(ro.join("Content").join("a.bin")).unwrap(), b"abc");
        // outside the root nothing changes
        let out = dir.join("save.bin");
        let w = Stream::open_file(Str::new(&out.to_string_lossy()), file_mode::CREATE, false, true);
        unsafe { (*w.ptr()).close() };
        assert!(out.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod range_tests {
    use super::*;

    #[test]
    fn memory_stream_over_a_range_reads_only_that_range() {
        let a = Arr::<u8>::from_slice(&[1, 2, 3, 4, 5, 6]);
        let s = Stream::from_bytes_range(a, 2, 3, false, true);
        // SAFETY: fresh object.
        unsafe {
            assert!(!(*s.ptr()).can_write());
            assert_eq!((*s.ptr()).length(), 3);
            let mut out = [0u8; 8];
            assert_eq!((*s.ptr()).read_slice(&mut out), 3);
            assert_eq!(&out[..3], &[3, 4, 5]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rt::try_;

    #[test]
    fn writer_and_reader_round_trip_net_encoding() {
        let ms = Stream::new_memory();
        let w = BinaryWriter::new_gc(ms);
        unsafe {
            let w = &mut *w.ptr();
            w.write(0x1234_5678i32);
            w.write(true);
            w.write(1.5f32);
            w.write(Str::lit("h\u{e9}llo"));
            w.write(Str::new(&"x".repeat(200)));
            w.write_char(0x41);
            let s = &mut *ms.ptr();
            // little endian int, bool, float, then the 7-bit length prefix of a 6-byte UTF-8 string
            assert_eq!(&s.bytes()[..10], &[0x78, 0x56, 0x34, 0x12, 1, 0, 0, 0xC0, 0x3F, 6]);
            // a 200-byte string has a two-byte prefix: 0xC8 0x01
            assert_eq!(&s.bytes()[16..18], &[0xC8, 0x01]);
            s.set_position(0);
            let r = &mut *BinaryReader::new_gc(ms).ptr();
            assert_eq!(r.read_i32(), 0x1234_5678);
            assert!(r.read_boolean());
            assert_eq!(r.read_f32(), 1.5);
            assert_eq!(r.read_string().as_str(), "h\u{e9}llo");
            assert_eq!(r.read_string().len(), 200);
            assert_eq!(r.read_char(), 0x41);
            let e = try_(|| r.read_byte()).unwrap_err();
            assert_eq!((*e.ptr()).kind, "System.IO.EndOfStreamException");
            r.close();
            assert!(!s.can_read());
            assert!(try_(|| s.length()).is_err());
        }
    }

    /// `new MemoryStream(byte[])` against .NET Framework 4.8 (`MemoryStream.cs` of the reference source): the
    /// stream wraps the array, is writable, cannot grow, and refuses `GetBuffer`.
    #[test]
    fn memory_stream_over_an_array_wraps_it() {
        let a = Arr::<u8>::from_slice(&[1, 2, 3]);
        let ms = Stream::from_bytes(a);
        unsafe {
            let s = &mut *ms.ptr();
            assert!(s.can_read() && s.can_write() && s.can_seek());
            assert_eq!((s.length(), s.capacity()), (3, 3));
            assert_eq!((s.read_byte(), s.read_byte(), s.read_byte(), s.read_byte()), (1, 2, 3, -1));
            assert_eq!(s.seek(-1, seek_origin::END), 2);
            // a write lands in the caller's array
            s.write_byte(9);
            assert_eq!(a.as_slice(), &[1, 2, 9]);
            // a change of the array is seen by the stream
            *a.at(0) = 7;
            s.set_position(0);
            assert_eq!(s.read_byte(), 7);
            // not expandable: the write that would pass the end throws and writes nothing
            s.set_position(2);
            let e = try_(|| s.write_slice(&[4, 5])).unwrap_err();
            assert_eq!((*e.ptr()).kind, "System.NotSupportedException");
            assert_eq!(a.as_slice(), &[7, 2, 9]);
            // SetLength inside the capacity is allowed; growing back zero-fills
            s.set_length(1);
            assert_eq!(s.length(), 1);
            s.set_length(3);
            assert_eq!(a.as_slice(), &[7, 0, 0]);
            assert!(try_(|| s.set_length(4)).is_err());
            // the CachedBuffer pattern: a writer and a reader over the same stream, position reset between packets
            let w = &mut *BinaryWriter::new_gc(ms).ptr();
            s.set_position(0);
            w.write(0x0201i16);
            assert_eq!(&a.as_slice()[..2], &[1, 2]);
            assert_eq!(s.position(), 2);
            // GetBuffer is refused for a stream created with this constructor
            let e = try_(|| s.get_buffer().len()).unwrap_err();
            assert_eq!((*e.ptr()).kind, "System.UnauthorizedAccessException");
            assert_eq!(s.to_array().as_slice(), &[1, 2, 0]);
            // a read-only wrapper
            let ro = Stream::from_bytes_writable(a, false);
            assert!(!(*ro.ptr()).can_write());
            assert!(try_(|| (*ro.ptr()).write_byte(1)).is_err());
            assert!(try_(|| BinaryWriter::new_gc(ro)).is_err());
        }
    }

    #[test]
    fn memory_stream_over_a_range_writes_into_that_range() {
        let a = Arr::<u8>::from_slice(&[1, 2, 3, 4, 5, 6]);
        let ms = Stream::from_bytes_range(a, 2, 3, true, true);
        unsafe {
            let s = &mut *ms.ptr();
            assert_eq!((s.length(), s.capacity()), (3, 3));
            s.write_slice(&[30, 40]);
            assert_eq!(a.as_slice(), &[1, 2, 30, 40, 5, 6]);
            // publiclyVisible: GetBuffer is the whole array
            assert!(s.get_buffer() == a);
            assert_eq!(s.to_array().as_slice(), &[30, 40, 5]);
            s.set_position(2);
            assert!(try_(|| s.write_slice(&[1, 2])).is_err());
            assert!(try_(|| Stream::from_bytes_range(a, 4, 3, true, true)).is_err());
            assert!(try_(|| Stream::from_bytes_range(a, -1, 3, true, true)).is_err());
        }
    }

    #[test]
    fn growing_memory_stream_matches_net() {
        unsafe {
            let ms = Stream::new_memory_capacity(16);
            let s = &mut *ms.ptr();
            assert!(s.capacity() >= 16);
            assert_eq!(s.length(), 0);
            // a write behind the end zero-fills the gap
            s.set_position(3);
            s.write_byte(5);
            assert_eq!(s.to_array().as_slice(), &[0, 0, 0, 5]);
            // a position behind the end reads nothing
            s.set_position(10);
            assert_eq!(s.read_byte(), -1);
            assert_eq!(s.length(), 4);
            // GetBuffer: the live buffer, longer than the stream
            let b = s.get_buffer();
            assert!(b.len() >= 256 && &b.as_slice()[..4] == &[0, 0, 0, 5]);
            s.set_position(0);
            s.write_byte(1);
            assert_eq!(*b.at(0), 1);
            // ToArray and GetBuffer work on a closed stream; Length does not
            s.close();
            assert_eq!(s.to_array().as_slice(), &[1, 0, 0, 5]);
            assert!(try_(|| s.length()).is_err());
            assert!(try_(|| s.write_byte(1)).is_err());
            assert!(try_(|| Stream::new_memory_capacity(-1)).is_err());
        }
    }

    #[test]
    fn close_hook_runs_once_with_the_written_bytes() {
        use std::cell::RefCell;
        use std::rc::Rc;
        let seen: Rc<RefCell<Vec<Vec<u8>>>> = Rc::new(RefCell::new(Vec::new()));
        let seen2 = seen.clone();
        let ms = Stream::new_sink(Box::new(move |b| seen2.borrow_mut().push(b.to_vec())), Vec::new());
        unsafe {
            let s = &mut *ms.ptr();
            assert!(s.can_write() && !s.can_read());
            s.write_slice(&[1, 2, 3]);
            s.flush();
            assert!(seen.borrow().is_empty());
            let inner = Stream::new_memory();
            s.set_wrapper(inner, 7);
            assert_eq!(s.tag(), 7);
            s.close();
            s.close();
            assert_eq!(*seen.borrow(), vec![vec![1u8, 2, 3]]);
            // a wrapper closes the stream it wraps
            assert!(!(*inner.ptr()).can_write());
        }
    }

    #[test]
    fn file_stream_writes_back_on_close() {
        let dir = std::env::temp_dir().join(format!("cs2rs-io-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.bin");
        let p = Str::new(path.to_str().unwrap());
        unsafe {
            let f = Stream::open_file(p, file_mode::CREATE, false, true);
            assert!(path.is_file());
            (*f.ptr()).write_slice(&[7, 8, 9]);
            (*f.ptr()).close();
            assert_eq!(std::fs::read(&path).unwrap(), vec![7, 8, 9]);
            let f = Stream::open_file(p, file_mode::OPEN, true, false);
            assert_eq!((*f.ptr()).length(), 3);
            assert!(try_(|| Stream::open_file(Str::new(dir.join("missing").to_str().unwrap()), file_mode::OPEN, true, false)).is_err());
            assert!(try_(|| Stream::open_file(p, file_mode::CREATE_NEW, true, true)).is_err());
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

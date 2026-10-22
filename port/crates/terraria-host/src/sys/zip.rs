//! `Ionic.Zip.CF.dll` as the game uses it: read-only zip files (resource packs: `ResourcePack`, ZipContentSource) and
//! `Ionic.Zlib` (`DeflateStream` for the map file and network world data, `ZlibCodec.Inflate` for old map files).
//! Inflate / deflate come from `miniz_oxide` 0.8 (already in the lock file through `png`).
//!
//! `ZipFile`: `ZipFile.Read(path)`, `this[name]` (case-insensitive, `\` and `/` are the same, as Ionic's default),
//! `ContainsEntry`, `Entries` (central directory order), `Dispose`. `ZipEntry`: `FileName` (forward slashes),
//! `IsDirectory`, `UncompressedSize`, `CompressedSize`, `Crc`, `Extract(Stream)` (CRC checked, `BadCrcException`
//! otherwise), `OpenReader()`. Not supported, with a clear error: zip64, encryption, methods other than stored and
//! deflate, writing archives (nothing in the game writes one).
//!
//! `DeflateStream`: **raw** deflate. Read mode inflates the whole source at construction (the world / map data is
//! a few MB). Write mode collects what is written and compresses it at [`deflate_stream_finish`], which the
//! translated `Dispose()` / `Close()` has to reach (REQUESTS.md, "close hook"). The compressed bytes are,
//! at level 6 (Ionic's default, the only level the game uses) the bytes Ionic's zlib port produces: zlib 1.3.1's
//! deflate written out in `zlib_deflate.rs`; other levels are valid deflate from miniz_oxide, not Ionic's bytes.



use terraria_gen_rt::rt::{self, io::Stream, Arr, Gc, Marker, Str, Trace};

use crate::gfx::states::throw;

fn u16_at(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn u32_at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

/// CRC-32 (IEEE), as zip stores it.
pub fn crc32(data: &[u8]) -> u32 {
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    let t = TABLE.get_or_init(|| {
        let mut t = [0u32; 256];
        for (i, e) in t.iter_mut().enumerate() {
            let mut c = i as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            }
            *e = c;
        }
        t
    });
    !data.iter().fold(!0u32, |c, b| t[((c ^ *b as u32) & 0xFF) as usize] ^ (c >> 8))
}

fn zip_exception(kind: &'static str, msg: &str) -> ! {
    rt::throw(rt::exc(kind, Str::new(msg)))
}

/// CP437 to Unicode for the upper half (names without the UTF-8 flag; Ionic's default `ProvisionalAlternateEncoding`).
fn cp437(b: &[u8]) -> String {
    const HI: &str = "ÇüéâäàåçêëèïîìÄÅÉæÆôöòûùÿÖÜ¢£¥₧ƒáíóúñÑªº¿⌐¬½¼¡«»░▒▓│┤╡╢╖╕╣║╗╝╜╛┐└┴┬├─┼╞╟╚╔╩╦╠═╬╧╨╤╥╙╘╒╓╫╪┘┌█▄▌▐▀αßΓπΣσµτΦΘΩδ∞φε∩≡±≥≤⌠⌡÷≈°∙·√ⁿ²■\u{a0}";
    let hi: Vec<char> = HI.chars().collect();
    b.iter().map(|c| if *c < 0x80 { *c as char } else { hi[(*c - 0x80) as usize] }).collect()
}

pub struct ZipEntry {
    pub name: Str,
    pub is_directory: bool,
    pub uncompressed: i64,
    pub compressed: i64,
    pub crc: u32,
    method: u16,
    /// the compressed bytes of the entry (a copy: entries outlive nothing, and archives are small)
    data: std::sync::Arc<Vec<u8>>,
    start: usize,
}
// SAFETY: the name is reported.
unsafe impl Trace for ZipEntry {
    unsafe fn trace(&self, m: &mut Marker) {
        self.name.trace(m)
    }
}

impl ZipEntry {
    fn raw(&self) -> &[u8] {
        &self.data[self.start..self.start + self.compressed as usize]
    }
    /// The entry's uncompressed bytes; `BadCrcException` when the CRC does not match.
    pub fn bytes(&self) -> Vec<u8> {
        let out = match self.method {
            0 => self.raw().to_vec(),
            8 => match miniz_oxide::inflate::decompress_to_vec(self.raw()) {
                Ok(v) => v,
                Err(e) => zip_exception("Ionic.Zip.BadReadException", &format!("Cannot read compressed data for entry {}: {e:?}", self.name.as_str())),
            },
            m => zip_exception("Ionic.Zip.ZipException", &format!("The compression method {m} is not supported (stored and deflate only)")),
        };
        if out.len() as i64 != self.uncompressed || crc32(&out) != self.crc {
            zip_exception("Ionic.Zip.BadCrcException", &format!("CRC error: the file being extracted, \"{}\", appears to be corrupted.", self.name.as_str()));
        }
        out
    }
    /// `FileName`
    pub fn file_name(&self) -> Str {
        self.name
    }
    pub fn uncompressed_size(&self) -> i64 {
        self.uncompressed
    }
    pub fn compressed_size(&self) -> i64 {
        self.compressed
    }
    pub fn crc(&self) -> i32 {
        self.crc as i32
    }
    pub fn is_directory(&self) -> bool {
        self.is_directory
    }
    /// `Extract(Stream)`
    pub fn extract(&self, stream: Gc<Stream>) {
        if stream.is_null() {
            throw("System.ArgumentNullException", "stream");
        }
        let b = self.bytes();
        // SAFETY: a live stream
        unsafe { (*stream.ptr()).write_slice(&b) };
    }
    /// `OpenReader()`: a stream over the uncompressed bytes.
    pub fn open_reader(&self) -> Gc<Stream> {
        Stream::from_vec(self.bytes())
    }
}

pub struct ZipFile {
    entries: Vec<Gc<ZipEntry>>,
    disposed: bool,
}
// SAFETY: the entries are reported.
unsafe impl Trace for ZipFile {
    unsafe fn trace(&self, m: &mut Marker) {
        for e in &self.entries {
            m.mark(*e);
        }
    }
}

fn normalize(name: &str) -> String {
    name.replace('\\', "/").to_lowercase()
}

impl ZipFile {
    /// `ZipFile.Read(path)`
    pub fn read(path: Str) -> Gc<ZipFile> {
        let p = path.get().unwrap_or_else(|| throw("System.ArgumentNullException", "fileName"));
        let full = std::path::PathBuf::from(p);
        let bytes = match std::fs::read(&full) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => zip_exception("System.IO.FileNotFoundException", &format!("Could not find file '{}'.", full.display())),
            Err(e) => zip_exception("System.IO.IOException", &e.to_string()),
        };
        Self::from_bytes(bytes)
    }
    /// `ZipFile.Read(Stream)`
    pub fn read_stream(stream: Gc<Stream>) -> Gc<ZipFile> {
        if stream.is_null() {
            throw("System.ArgumentNullException", "zipStream");
        }
        // SAFETY: a live stream
        let s = unsafe { &mut *stream.ptr() };
        let mut b = vec![0u8; (s.length() - s.position()).max(0) as usize];
        let n = s.read_slice(&mut b);
        b.truncate(n);
        Self::from_bytes(b)
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Gc<ZipFile> {
        let bad = |m: &str| -> ! { zip_exception("Ionic.Zip.BadReadException", m) };
        // end of central directory record: the last "PK\5\6" in the final 64 KB + 22 bytes
        let lo = bytes.len().saturating_sub(65_557);
        let eocd = (lo..bytes.len().saturating_sub(21)).rev().find(|i| &bytes[*i..*i + 4] == b"PK\x05\x06").unwrap_or_else(|| bad("Cannot read that as a ZipFile"));
        let total = u16_at(&bytes, eocd + 10) as usize;
        let cd_size = u32_at(&bytes, eocd + 12) as usize;
        let cd_off = u32_at(&bytes, eocd + 16) as usize;
        if total == 0xFFFF || cd_off == u32::MAX as usize || cd_size == u32::MAX as usize {
            zip_exception("Ionic.Zip.ZipException", "Zip64 archives are not supported");
        }
        let data = std::sync::Arc::new(bytes);
        let b = &data[..];
        let mut entries = Vec::with_capacity(total);
        let mut o = cd_off;
        for _ in 0..total {
            if o + 46 > b.len() || &b[o..o + 4] != b"PK\x01\x02" {
                bad("Bad signature in the central directory");
            }
            let flags = u16_at(b, o + 8);
            let method = u16_at(b, o + 10);
            let crc = u32_at(b, o + 16);
            let compressed = u32_at(b, o + 20) as i64;
            let uncompressed = u32_at(b, o + 24) as i64;
            let (nl, el, cl) = (u16_at(b, o + 28) as usize, u16_at(b, o + 30) as usize, u16_at(b, o + 32) as usize);
            let local = u32_at(b, o + 42) as usize;
            let raw_name = &b[o + 46..o + 46 + nl];
            let mut name = if flags & 0x800 != 0 { String::from_utf8_lossy(raw_name).into_owned() } else { cp437(raw_name) };
            name = name.replace('\\', "/");
            if flags & 1 != 0 {
                zip_exception("Ionic.Zip.ZipException", &format!("Entry {name} is encrypted: not supported"));
            }
            // the data starts after the LOCAL header, whose extra field may differ from the central one
            if local + 30 > b.len() || &b[local..local + 4] != b"PK\x03\x04" {
                bad("Bad signature in a local header");
            }
            let start = local + 30 + u16_at(b, local + 26) as usize + u16_at(b, local + 28) as usize;
            if start + compressed as usize > b.len() {
                bad("An entry extends beyond the end of the archive");
            }
            let is_directory = name.ends_with('/');
            entries.push(rt::alloc_init(ZipEntry { name: Str::new(&name), is_directory, uncompressed, compressed, crc, method, data: data.clone(), start }));
            o += 46 + nl + el + cl;
        }
        rt::alloc_init(ZipFile { entries, disposed: false })
    }

    fn check(&self) {
        if self.disposed {
            throw("System.ObjectDisposedException", "Cannot access a disposed object. Object name: 'ZipFile'.");
        }
    }
    /// `this[fileName]`: null when absent.
    pub fn get(&self, name: Str) -> Gc<ZipEntry> {
        self.check();
        let n = normalize(name.get().unwrap_or_else(|| throw("System.ArgumentNullException", "fileName")));
        // SAFETY: live entries
        self.entries.iter().find(|e| normalize(unsafe { (**e).ptr().as_ref() }.expect("entry").name.as_str()) == n).copied().unwrap_or(Gc::NULL)
    }
    pub fn contains_entry(&self, name: Str) -> bool {
        !self.get(name).is_null()
    }
    /// `Entries`
    pub fn entries(&self) -> Arr<Gc<ZipEntry>> {
        self.check();
        Arr::from_slice(&self.entries)
    }
    pub fn count(&self) -> i32 {
        self.entries.len() as i32
    }
    pub fn dispose(&mut self) {
        self.disposed = true;
    }
}

// ------------------------------------------------------------------------------------------------ DeflateStream / ZlibCodec

/// `Ionic.Zlib.CompressionLevel` (None / Level0 = 0, BestSpeed = 1 .. BestCompression = 9, Default = 6) -> miniz level
fn level_of(l: i32) -> u8 {
    l.clamp(0, 9) as u8
}

/// `Stream::tag` of a `DeflateStream`.
pub const DEFLATE_STREAM_TAG: u32 = 2;

/// `new DeflateStream(stream, mode)` (level 6, the inner stream is closed with it) / `(stream, mode, level)` /
/// `(stream, mode, leaveOpen)`: `mode` Compress 0, Decompress 1. See the module docs.
///
/// Compress mode: the stream collects what is written; when it is closed (`Dispose` / `Close`, also through
/// `Stream` or `IDisposable`: the runtime's close hook) the bytes are compressed and written to `stream`.
pub fn deflate_stream_new(stream: Gc<Stream>, mode: i32, level: i32, leave_open: bool) -> Gc<Stream> {
    if stream.is_null() {
        throw("System.ArgumentNullException", "stream");
    }
    // SAFETY: a live runtime object
    let inner = unsafe { &mut *stream.ptr() };
    let s = if mode == 1 {
        let mut rest = vec![0u8; (inner.length() - inner.position()).max(0) as usize];
        let n = inner.read_slice(&mut rest);
        rest.truncate(n);
        match miniz_oxide::inflate::decompress_to_vec(&rest) {
            Ok(v) => Stream::from_vec(v),
            Err(e) => zip_exception("Ionic.Zlib.ZlibException", &format!("Bad state ({e:?})")),
        }
    } else {
        let level = level_of(level);
        let hook = Box::new(move |bytes: &[u8]| {
            // level 6 (Ionic's Default, what the game uses: NetMessage.CompressTileBlock, MapHelper): zlib 1.3.1's own
            // bytes (zlib_deflate.rs; Ionic.Zlib writes the same, and nothing for an empty stream)
            let out = if level == 6 {
                if bytes.is_empty() { Vec::new() } else { super::zlib_deflate::deflate_raw_level6(bytes) }
            } else {
                miniz_oxide::deflate::compress_to_vec(bytes, level)
            };
            // SAFETY: the target is kept alive by the deflate stream (`close_roots`) and is a different object.
            unsafe { (*stream.ptr()).write_slice(&out) }
        });
        Stream::new_sink(hook, vec![stream.up()])
    };
    // SAFETY: fresh object.
    unsafe { (*s.ptr()).set_wrapper(if leave_open { Gc::NULL } else { stream }, DEFLATE_STREAM_TAG) };
    s
}

/// `DeflateStream.Dispose()` / `Close()`: see [`deflate_stream_new`]. A second call does nothing.
pub fn deflate_stream_finish(stream: Gc<Stream>) {
    // SAFETY: `ptr()` checks null.
    unsafe { (*stream.ptr()).close() }
}

/// `Ionic.Zlib.ZlibCodec` (inflate only): the fields MapHelper sets, `InitializeInflate()`, `Inflate(FlushType.Finish)`.
pub struct ZlibCodec {
    pub input_buffer: Arr<u8>,
    pub output_buffer: Arr<u8>,
    pub next_in: i32,
    pub available_bytes_in: i32,
    pub next_out: i32,
    pub available_bytes_out: i32,
    pub total_bytes_in: i64,
    pub total_bytes_out: i64,
}
// SAFETY: both arrays are reported.
unsafe impl Trace for ZlibCodec {
    unsafe fn trace(&self, m: &mut Marker) {
        self.input_buffer.trace(m);
        self.output_buffer.trace(m);
    }
}
impl ZlibCodec {
    pub fn new_gc(_mode: i32) -> Gc<ZlibCodec> {
        // SAFETY: a zeroed codec is the state of a fresh one (null buffers, zero counters)
        rt::alloc_init(ZlibCodec { input_buffer: Arr::NULL, output_buffer: Arr::NULL, next_in: 0, available_bytes_in: 0, next_out: 0, available_bytes_out: 0, total_bytes_in: 0, total_bytes_out: 0 })
    }
    /// `InitializeInflate()`: `ZlibConstants.Z_OK` (0). Resets the counters.
    pub fn initialize_inflate(&mut self) -> i32 {
        self.total_bytes_in = 0;
        self.total_bytes_out = 0;
        0
    }
    /// `Inflate(FlushType)`: inflates the zlib stream in `InputBuffer[NextIn..NextIn + AvailableBytesIn]` into
    /// `OutputBuffer[NextOut..]`. Returns `Z_STREAM_END` (1) when the stream ended, `Z_BUF_ERROR` (-5) when the
    /// output buffer is too small or the input is truncated, `Z_DATA_ERROR` (-3) for corrupt data. Only
    /// `FlushType.Finish` (4) is meaningful: the whole input is consumed in one call.
    pub fn inflate(&mut self, _flush: i32) -> i32 {
        if self.input_buffer.is_null() || self.output_buffer.is_null() {
            return -2;
        }
        let inp = &self.input_buffer.as_slice()[self.next_in as usize..(self.next_in + self.available_bytes_in) as usize];
        match miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(inp, self.available_bytes_out.max(0) as usize) {
            Ok(v) => {
                // SAFETY: the output range was sized by the caller (limit above)
                unsafe { core::ptr::copy_nonoverlapping(v.as_ptr(), self.output_buffer.as_slice().as_ptr().add(self.next_out as usize) as *mut u8, v.len()) };
                self.total_bytes_out += v.len() as i64;
                self.total_bytes_in += inp.len() as i64;
                self.next_out += v.len() as i32;
                self.available_bytes_out -= v.len() as i32;
                self.next_in += inp.len() as i32;
                self.available_bytes_in = 0;
                1
            }
            Err(e) => match e.status {
                miniz_oxide::inflate::TINFLStatus::HasMoreOutput => -5,
                miniz_oxide::inflate::TINFLStatus::FailedCannotMakeProgress | miniz_oxide::inflate::TINFLStatus::NeedsMoreInput => -5,
                _ => -3,
            },
        }
    }
    pub fn total_bytes_out(&self) -> i64 {
        self.total_bytes_out
    }
}

#[cfg(test)]
mod tests;

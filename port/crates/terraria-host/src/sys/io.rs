//! `System.IO`: `Path`, `File`, `Directory`, `FileInfo`, `StreamReader`, `StreamWriter`.
//!
//! Streams, `BinaryReader` and `BinaryWriter` are the runtime's (`terraria_gen_rt::rt::io`); this module adds what
//! is around them. Paths are Windows paths as strings with .NET Framework's rules (expected values:
//! tests/dotnet/system_probe.ps1).
//!
//! Where files go: the game builds every save path from `Main.SavePath`, which it gets from
//! `IPathService.GetStoragePath` ([`crate::os::storage_path`]) -> [`save_root`]. This module does not redirect
//! paths; it refuses to WRITE inside the game's install folder ([`crate::content::install_dir`]), which is
//! read-only by contract.

use std::path::{Path as StdPath, PathBuf};

use terraria_gen_rt::rt::{self, io::Stream, Arr, Gc, Marker, Str, Trace};

use super::time::DateTime;
use crate::gfx::states::throw;

/// `Main.SavePath` for this process: `TERRARIA_SAVE_DIR`, else `<project>\work\port-save` (so a development run
/// never touches the real `Documents\My Games\Terraria`).
pub fn save_root() -> PathBuf {
    match std::env::var_os("TERRARIA_SAVE_DIR") {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        _ => project_dir().join("work").join("port-save"),
    }
}

/// The project folder (`VAEL - TERRARIA`): `TERRARIA_PROJECT_DIR`, else three levels above this crate.
pub fn project_dir() -> PathBuf {
    match std::env::var_os("TERRARIA_PROJECT_DIR") {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        // <project>/port/crates/terraria-host -> <project>
        _ => crate::manifest_dir().ancestors().nth(3).map(PathBuf::from).unwrap_or_default(),
    }
}

#[cold]
fn io_error(message: String) -> ! {
    rt::throw(rt::exc("System.IO.IOException", Str::new(&message)))
}

fn text(s: Str, parameter: &'static str) -> &'static str {
    match s.get() {
        Some(t) => t,
        None => throw("System.ArgumentNullException", parameter),
    }
}

/// Refuses writes below the game's install folder.
fn writable(path: &str) -> PathBuf {
    let p = PathBuf::from(path);
    let install = crate::content::install_dir();
    let norm = |p: &StdPath| p.to_string_lossy().replace('/', "\\").to_ascii_lowercase();
    let abs = if p.is_absolute() { p.clone() } else { std::env::current_dir().map(|c| c.join(&p)).unwrap_or_else(|_| p.clone()) };
    if norm(&abs).starts_with(&(norm(&install) + "\\")) || norm(&abs) == norm(&install) {
        rt::throw(rt::exc("System.UnauthorizedAccessException", Str::new(&format!("The host does not write inside the game's install folder: '{path}'."))));
    }
    p
}

// ------------------------------------------------------------------------------------------------ Path

fn is_sep(c: char) -> bool {
    c == '\\' || c == '/'
}

/// `Path.GetFileName`
pub fn path_get_file_name(path: Str) -> Str {
    let Some(p) = path.get() else { return Str::NULL };
    match p.rfind(|c| is_sep(c) || c == ':') {
        Some(i) => Str::new(&p[i + 1..]),
        None => path,
    }
}

fn extension_of(p: &str) -> &str {
    for (i, c) in p.char_indices().rev() {
        if c == '.' {
            return if i != p.len() - 1 { &p[i..] } else { "" };
        }
        if is_sep(c) || c == ':' {
            break;
        }
    }
    ""
}

/// `Path.GetExtension`: with the dot; empty when there is none or the name ends with a dot.
pub fn path_get_extension(path: Str) -> Str {
    match path.get() {
        Some(p) => Str::new(extension_of(p)),
        None => Str::NULL,
    }
}

/// `Path.HasExtension`
pub fn path_has_extension(path: Str) -> bool {
    path.get().is_some_and(|p| !extension_of(p).is_empty())
}

/// `Path.GetFileNameWithoutExtension`
pub fn path_get_file_name_without_extension(path: Str) -> Str {
    let name = path_get_file_name(path);
    let Some(n) = name.get() else { return Str::NULL };
    match n.rfind('.') {
        Some(i) => Str::new(&n[..i]),
        None => name,
    }
}

fn root_length(p: &str) -> usize {
    let b = p.as_bytes();
    let sep = |i: usize| b.get(i).is_some_and(|c| *c == b'\\' || *c == b'/');
    if sep(0) {
        if sep(1) {
            // \\server\share
            let mut i = 2;
            let mut parts = 2;
            while i < b.len() && (!sep(i) || {
                parts -= 1;
                parts > 0
            }) {
                i += 1;
            }
            return i;
        }
        return 1;
    }
    if b.len() >= 2 && b[1] == b':' {
        return if sep(2) { 3 } else { 2 };
    }
    0
}

/// `Path.GetDirectoryName`: null for null and for a root; separators come back as `\`.
pub fn path_get_directory_name(path: Str) -> Str {
    let Some(p) = path.get() else { return Str::NULL };
    if p.trim().is_empty() {
        throw("System.ArgumentException", "The path is not of a legal form.");
    }
    // NormalizePath(fullCheck: false): `/` -> `\`, runs of separators collapse (a leading `\\` stays)
    let mut n = String::with_capacity(p.len());
    for (i, c) in p.chars().enumerate() {
        let c = if c == '/' { '\\' } else { c };
        if c == '\\' && n.ends_with('\\') && i > 1 {
            continue;
        }
        n.push(c);
    }
    let root = root_length(&n);
    let mut i = n.len();
    if i > root {
        let b = n.as_bytes();
        loop {
            i -= 1;
            if i <= root || b[i] == b'\\' {
                break;
            }
        }
        if i < root {
            i = root;
        }
        return Str::new(&n[..i]);
    }
    Str::NULL
}

/// `Path.IsPathRooted`
pub fn path_is_path_rooted(path: Str) -> bool {
    path.get().is_some_and(|p| {
        let b = p.as_bytes();
        (!b.is_empty() && (b[0] == b'\\' || b[0] == b'/')) || (b.len() >= 2 && b[1] == b':')
    })
}

/// `Path.ChangeExtension`: a null extension removes it (the dot stays), a missing leading dot is added.
pub fn path_change_extension(path: Str, extension: Str) -> Str {
    let Some(p) = path.get() else { return Str::NULL };
    let mut s = p.to_owned();
    for (i, c) in p.char_indices().rev() {
        if c == '.' {
            s.truncate(i);
            break;
        }
        if is_sep(c) || c == ':' {
            break;
        }
    }
    if let Some(e) = extension.get() {
        if !p.is_empty() {
            if !e.starts_with('.') {
                s.push('.');
            }
            s.push_str(e);
        }
    } else if !p.is_empty() {
        s.push('.');
    }
    Str::new(&s)
}

/// `Path.GetInvalidFileNameChars()` (Windows): `" < > |`, 0..31, `: * ? \ /`.
pub fn path_get_invalid_file_name_chars() -> Arr<u16> {
    let mut v: Vec<u16> = vec![34, 60, 62, 124];
    v.extend(0..32u16);
    v.extend([58, 42, 63, 92, 47]);
    Arr::from_slice(&v)
}

/// `Path.GetInvalidPathChars()` (Windows): `" < > |`, 0..31.
pub fn path_get_invalid_path_chars() -> Arr<u16> {
    let mut v: Vec<u16> = vec![34, 60, 62, 124];
    v.extend(0..32u16);
    Arr::from_slice(&v)
}

/// `Path.DirectorySeparatorChar`
pub const DIRECTORY_SEPARATOR_CHAR: u16 = b'\\' as u16;

/// `Path.GetFullPath`: against the process's current directory; `.` and `..` resolved, `/` -> `\`.
pub fn path_get_full_path(path: Str) -> Str {
    let p = text(path, "path");
    let abs = if StdPath::new(p).is_absolute() { PathBuf::from(p) } else { std::env::current_dir().unwrap_or_default().join(p) };
    let mut out: Vec<String> = Vec::new();
    for part in abs.to_string_lossy().replace('/', "\\").split('\\') {
        match part {
            "." | "" if !out.is_empty() => {}
            ".." if out.len() > 1 => {
                out.pop();
            }
            ".." => {}
            _ => out.push(part.to_owned()),
        }
    }
    let mut s = out.join("\\");
    if s.len() == 2 && s.ends_with(':') {
        s.push('\\');
    }
    Str::new(&s)
}

/// `Path.GetTempPath()`: a folder below the save root (the real temp folder is not this process's to litter).
pub fn path_get_temp_path() -> Str {
    Str::new(&format!("{}\\", save_root().join("_temp").display()))
}

// ------------------------------------------------------------------------------------------------ File

/// `File.Exists`: false for null, empty, a directory, or an inaccessible path (never throws).
pub fn file_exists(path: Str) -> bool {
    path.get().is_some_and(|p| !p.is_empty() && StdPath::new(p).is_file())
}

/// `File.Delete`: no error when the file does not exist; `DirectoryNotFoundException` when its folder does not.
pub fn file_delete(path: Str) {
    let p = writable(text(path, "path"));
    match std::fs::remove_file(&p) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            if p.parent().is_some_and(|d| !d.as_os_str().is_empty() && !d.is_dir()) {
                rt::throw(rt::exc("System.IO.DirectoryNotFoundException", Str::new(&format!("Could not find a part of the path '{}'.", p.display()))));
            }
        }
        Err(e) => io_error(format!("{}: {e}", p.display())),
    }
}

fn read(path: Str) -> Vec<u8> {
    let p = text(path, "path");
    match std::fs::read(p) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => rt::throw(rt::exc("System.IO.FileNotFoundException", Str::new(&format!("Could not find file '{p}'.")))),
        Err(e) => io_error(format!("{p}: {e}")),
    }
}

fn write(path: Str, bytes: &[u8], append: bool) {
    let p = writable(text(path, "path"));
    use std::io::Write;
    let r = std::fs::OpenOptions::new().create(true).write(true).append(append).truncate(!append).open(&p).and_then(|mut f| f.write_all(bytes));
    if let Err(e) = r {
        if e.kind() == std::io::ErrorKind::NotFound {
            rt::throw(rt::exc("System.IO.DirectoryNotFoundException", Str::new(&format!("Could not find a part of the path '{}'.", p.display()))));
        }
        io_error(format!("{}: {e}", p.display()));
    }
}

/// `File.OpenRead`
pub fn file_open_read(path: Str) -> Gc<Stream> {
    Stream::open_file(path, rt::io::file_mode::OPEN, true, false)
}

/// `File.Open(path, mode)`: `FileAccess.ReadWrite`, or `Write` for `FileMode.Append` (as .NET: `Append` cannot
/// read). The runtime's stream refuses anything below the read-only root (the game's install folder).
pub fn file_open(path: Str, mode: i32) -> Gc<Stream> {
    Stream::open_file(path, mode, mode != rt::io::file_mode::APPEND, true)
}

/// `TextWriter.WriteLine(string)` on a writer held through the base class: the host's one `TextWriter` is
/// [`StreamWriter`] (the game's crash logs: `new StreamWriter("client-crashlog.txt", true)`).
pub fn text_writer_write_line(writer: rt::Obj, value: Str) {
    if rt::obj_is::<StreamWriter>(writer) {
        // SAFETY: type checked; the reference ends with this statement.
        unsafe { (*(writer.0 as *mut StreamWriter)).write_line(value) }
    } else {
        let _ = writer.ptr();
        terraria_gen_rt::host::unimplemented("System.IO.TextWriter.WriteLine(string) on a writer that is not a StreamWriter");
    }
}
/// `TextWriter.WriteLine(object)`: the object's `ToString()` (null: an empty line).
pub fn text_writer_write_line_obj(writer: rt::Obj, value: rt::Obj) {
    text_writer_write_line(writer, rt::obj_to_string_or_empty(value))
}
/// `TextWriter.Flush()`
pub fn text_writer_flush(writer: rt::Obj) {
    if rt::obj_is::<StreamWriter>(writer) {
        // SAFETY: as above.
        unsafe { (*(writer.0 as *mut StreamWriter)).flush() }
    } else {
        let _ = writer.ptr();
        terraria_gen_rt::host::unimplemented("System.IO.TextWriter.Flush() on a writer that is not a StreamWriter");
    }
}
/// `TextWriter.Dispose()` / `Close()`
pub fn text_writer_dispose(writer: rt::Obj) {
    if rt::obj_is::<StreamWriter>(writer) {
        // SAFETY: as above.
        unsafe { (*(writer.0 as *mut StreamWriter)).close() }
    } else {
        let _ = writer.ptr();
        terraria_gen_rt::host::unimplemented("System.IO.TextWriter.Dispose() on a writer that is not a StreamWriter");
    }
}

/// `File.ReadAllBytes`
pub fn file_read_all_bytes(path: Str) -> Arr<u8> {
    Arr::from_slice(&read(path))
}

/// Decoding of `File.ReadAllText` / `StreamReader` with `detectEncodingFromByteOrderMarks`: UTF-8 (BOM skipped),
/// UTF-16 LE / BE by BOM; invalid UTF-8 becomes U+FFFD as in .NET.
pub fn decode_text(bytes: &[u8]) -> String {
    if let Some(b) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(b).into_owned();
    }
    if bytes.len() >= 2 && (bytes[..2] == [0xFF, 0xFE] || bytes[..2] == [0xFE, 0xFF]) {
        let le = bytes[0] == 0xFF;
        let units: Vec<u16> = bytes[2..].chunks_exact(2).map(|c| if le { u16::from_le_bytes([c[0], c[1]]) } else { u16::from_be_bytes([c[0], c[1]]) }).collect();
        return String::from_utf16_lossy(&units);
    }
    String::from_utf8_lossy(bytes).into_owned()
}

/// `File.ReadAllText(path)`
pub fn file_read_all_text(path: Str) -> Str {
    Str::new(&decode_text(&read(path)))
}

/// `File.WriteAllBytes`
pub fn file_write_all_bytes(path: Str, bytes: Arr<u8>) {
    if bytes.is_null() {
        throw("System.ArgumentNullException", "bytes");
    }
    write(path, bytes.as_slice(), false);
}

/// `File.WriteAllText(path, contents)`: UTF-8 without a byte order mark.
pub fn file_write_all_text(path: Str, contents: Str) {
    write(path, contents.get().unwrap_or("").as_bytes(), false);
}

/// `File.AppendAllText(path, contents)`
pub fn file_append_all_text(path: Str, contents: Str) {
    write(path, contents.get().unwrap_or("").as_bytes(), true);
}

/// `File.Copy(source, dest, overwrite)`
pub fn file_copy(source: Str, dest: Str, overwrite: bool) {
    let d = writable(text(dest, "destFileName"));
    if d.exists() && !overwrite {
        io_error(format!("The file '{}' already exists.", d.display()));
    }
    let bytes = read(source);
    if let Err(e) = std::fs::write(&d, bytes) {
        io_error(format!("{}: {e}", d.display()));
    }
}

/// `File.Move(source, dest)`
pub fn file_move(source: Str, dest: Str) {
    let s = writable(text(source, "sourceFileName"));
    let d = writable(text(dest, "destFileName"));
    if d.exists() {
        io_error("Cannot create a file when that file already exists.".into());
    }
    if let Err(e) = std::fs::rename(&s, &d) {
        if e.kind() == std::io::ErrorKind::NotFound {
            rt::throw(rt::exc("System.IO.FileNotFoundException", Str::new(&format!("Could not find file '{}'.", s.display()))));
        }
        io_error(format!("{}: {e}", s.display()));
    }
}

fn modified_ticks(p: &StdPath) -> Option<i64> {
    let t = std::fs::metadata(p).ok()?.modified().ok()?;
    let d = t.duration_since(std::time::UNIX_EPOCH).ok()?;
    Some(super::time::UNIX_EPOCH_TICKS + (d.as_nanos() / 100) as i64)
}

/// `File.GetCreationTime(path)`: local time; 1601-01-01 UTC converted to local time when the file does not exist
/// (.NET's documented value). A file system without creation times answers the last write time.
pub fn file_get_creation_time(path: Str) -> DateTime {
    let p = text(path, "path");
    let ticks = std::fs::metadata(p)
        .ok()
        .and_then(|m| m.created().or_else(|_| m.modified()).ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| super::time::UNIX_EPOCH_TICKS + (d.as_nanos() / 100) as i64)
        .unwrap_or(super::time::FILE_TIME_EPOCH_TICKS);
    DateTime::from_ticks_kind(ticks, super::time::KIND_UTC).to_local_time()
}

/// `File.GetLastWriteTimeUtc`: 1601-01-01 UTC when the file does not exist (.NET's documented value).
pub fn file_get_last_write_time_utc(path: Str) -> DateTime {
    let p = text(path, "path");
    DateTime::from_ticks_kind(modified_ticks(StdPath::new(p)).unwrap_or(super::time::FILE_TIME_EPOCH_TICKS), super::time::KIND_UTC)
}

/// `File.GetAttributes(path)`: `ReadOnly` (1) from the file's permissions, `Directory` (16) for a folder, otherwise
/// `Archive` (32), which is what Windows reports for an ordinary file the game wrote. A missing path throws
/// `FileNotFoundException`, as .NET does. (`FileUtilities.RemoveReadOnlyAttribute` reads the read-only bit.)
pub fn file_get_attributes(path: Str) -> i32 {
    let p = text(path, "path");
    match std::fs::metadata(p) {
        Ok(m) => {
            let mut a = if m.is_dir() { 16 } else { 32 };
            if m.permissions().readonly() {
                a |= 1;
            }
            a
        }
        Err(_) => rt::throw(rt::exc("System.IO.FileNotFoundException", Str::new(&format!("Could not find file '{p}'.")))),
    }
}

/// `File.SetAttributes(path, attributes)`: only the read-only bit (`FileAttributes.ReadOnly` = 1) exists outside
/// Windows' own file system API; `Normal` (128) clears it, which is what the game uses it for before overwriting.
pub fn file_set_attributes(path: Str, attributes: i32) {
    let p = writable(text(path, "path"));
    match std::fs::metadata(&p) {
        Ok(m) => {
            // WASI has no file attributes (set_permissions is "unsupported"); the browser's file system none either:
            // the game only clears ReadOnly before it writes (FileUtilities), which there is nothing to clear
            #[cfg(target_os = "wasi")]
            let _ = (m, attributes);
            #[cfg(not(target_os = "wasi"))]
            {
                let mut perm = m.permissions();
                perm.set_readonly(attributes & 1 != 0);
                if let Err(e) = std::fs::set_permissions(&p, perm) {
                    io_error(format!("{}: {e}", p.display()));
                }
            }
        }
        Err(_) => rt::throw(rt::exc("System.IO.FileNotFoundException", Str::new(&format!("Could not find file '{}'.", p.display())))),
    }
}

// ------------------------------------------------------------------------------------------------ Directory

/// `Directory.Exists`
pub fn directory_exists(path: Str) -> bool {
    path.get().is_some_and(|p| !p.is_empty() && StdPath::new(p).is_dir())
}

/// `Directory.CreateDirectory`: creates every missing folder; returns the path (the game ignores the `DirectoryInfo`).
pub fn directory_create_directory(path: Str) -> Str {
    let p = writable(text(path, "path"));
    if let Err(e) = std::fs::create_dir_all(&p) {
        io_error(format!("{}: {e}", p.display()));
    }
    path
}

/// `Directory.Delete(path, recursive)`
pub fn directory_delete(path: Str, recursive: bool) {
    let p = writable(text(path, "path"));
    if !p.is_dir() {
        rt::throw(rt::exc("System.IO.DirectoryNotFoundException", Str::new(&format!("Could not find a part of the path '{}'.", p.display()))));
    }
    let r = if recursive { std::fs::remove_dir_all(&p) } else { std::fs::remove_dir(&p) };
    if let Err(e) = r {
        io_error(format!("{}: {e}", p.display()));
    }
}

/// Windows wildcard match as `Directory.GetFiles` uses it: `*` any run, `?` one character, case-insensitive.
/// (.NET's legacy 8.3 quirk, where `*.wld` also matches `x.wldbak`, is not reproduced: it depends on short names
/// existing on the volume.)
pub fn wildcard_match(pattern: &str, name: &str) -> bool {
    fn go(p: &[char], n: &[char]) -> bool {
        match p.split_first() {
            None => n.is_empty(),
            Some(('*', rest)) => (0..=n.len()).any(|k| go(rest, &n[k..])),
            Some(('?', rest)) => !n.is_empty() && go(rest, &n[1..]),
            Some((c, rest)) => n.first().is_some_and(|d| d.to_lowercase().eq(c.to_lowercase())) && go(rest, &n[1..]),
        }
    }
    let p: Vec<char> = pattern.chars().collect();
    let n: Vec<char> = name.chars().collect();
    // "*.*" and "*" match every name, with or without a dot
    pattern == "*.*" || go(&p, &n)
}

fn list(path: Str, pattern: &str, dirs: bool) -> Arr<Str> {
    let p = text(path, "path");
    let Ok(rd) = std::fs::read_dir(p) else {
        rt::throw(rt::exc("System.IO.DirectoryNotFoundException", Str::new(&format!("Could not find a part of the path '{p}'."))));
    };
    let mut names: Vec<String> = rd
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir() == dirs))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| wildcard_match(pattern, n))
        .collect();
    // NTFS enumerates in name order (case-insensitive); read_dir's order is the file system's, so make it explicit
    names.sort_by_key(|n| n.to_uppercase());
    let sep = if p.ends_with(['\\', '/']) { "" } else { "\\" };
    let out: Vec<Str> = names.iter().map(|n| Str::new(&format!("{p}{sep}{n}"))).collect();
    Arr::from_slice(&out)
}

/// `Directory.GetFiles(path, searchPattern)`: full paths (the given path + name), top directory only.
pub fn directory_get_files(path: Str, pattern: Str) -> Arr<Str> {
    list(path, text(pattern, "searchPattern"), false)
}
/// `Directory.GetFiles(path)`
pub fn directory_get_files_all(path: Str) -> Arr<Str> {
    list(path, "*", false)
}
/// `Directory.GetDirectories(path)`
pub fn directory_get_directories(path: Str) -> Arr<Str> {
    list(path, "*", true)
}

// ------------------------------------------------------------------------------------------------ FileInfo

/// `System.IO.FileInfo`: the path; every property looks at the file when asked.
pub struct FileInfo {
    pub path: Str,
}
// SAFETY: the one handle is reported.
unsafe impl Trace for FileInfo {
    unsafe fn trace(&self, m: &mut Marker) {
        self.path.trace(m)
    }
}
impl FileInfo {
    pub fn new_gc(path: Str) -> Gc<FileInfo> {
        text(path, "fileName");
        rt::alloc_init(FileInfo { path: path_get_full_path(path) })
    }
    pub fn exists(&self) -> bool {
        file_exists(self.path)
    }
    pub fn full_name(&self) -> Str {
        self.path
    }
    pub fn name(&self) -> Str {
        path_get_file_name(self.path)
    }
    pub fn directory_name(&self) -> Str {
        path_get_directory_name(self.path)
    }
    /// `Length`: `FileNotFoundException` when the file does not exist.
    pub fn length(&self) -> i64 {
        match std::fs::metadata(self.path.as_str()) {
            Ok(m) if m.is_file() => m.len() as i64,
            _ => rt::throw(rt::exc("System.IO.FileNotFoundException", Str::new(&format!("Could not find file '{}'.", self.path.as_str())))),
        }
    }
    /// `IsReadOnly.set`
    pub fn set_is_read_only(&mut self, v: bool) {
        file_set_attributes(self.path, if v { 1 } else { 128 });
    }
    pub fn is_read_only(&self) -> bool {
        std::fs::metadata(self.path.as_str()).is_ok_and(|m| m.permissions().readonly())
    }
    pub fn delete(&mut self) {
        file_delete(self.path);
    }
}

/// `System.IO.DirectoryInfo`: the full path; every member looks at the folder when asked.
pub struct DirectoryInfo {
    pub path: Str,
}
// SAFETY: the one handle is reported.
unsafe impl Trace for DirectoryInfo {
    unsafe fn trace(&self, m: &mut Marker) {
        self.path.trace(m)
    }
}
impl DirectoryInfo {
    /// `Directory.GetParent(path)`: the parent of the full path; null for a root.
    pub fn parent_of(path: Str) -> Gc<DirectoryInfo> {
        text(path, "path");
        let full = path_get_full_path(path);
        let parent = path_get_directory_name(full);
        if parent.is_null() {
            return Gc::NULL;
        }
        rt::alloc_init(DirectoryInfo { path: parent })
    }
    pub fn full_name(&self) -> Str {
        self.path
    }
    pub fn exists(&self) -> bool {
        directory_exists(self.path)
    }
    /// `Create()`: every missing folder of the path (refused below the game's install, like every write here).
    pub fn create(&mut self) {
        directory_create_directory(self.path);
    }
}

// ------------------------------------------------------------------------------------------------ StreamReader / StreamWriter

/// `System.IO.StreamReader` (also the `TextReader` the game holds it as): the whole stream is decoded when the
/// reader is created (UTF-8, or UTF-16 by byte order mark), which is indistinguishable for sequential reads.
pub struct StreamReader {
    pub stream: Gc<Stream>,
    text: Vec<char>,
    pos: usize,
}
// SAFETY: the one handle is reported.
unsafe impl Trace for StreamReader {
    unsafe fn trace(&self, m: &mut Marker) {
        m.mark(self.stream)
    }
}
impl StreamReader {
    /// `new StringReader(text)`: a reader over a string.
    pub fn from_text(text: Str) -> Gc<StreamReader> {
        let t = text.get().unwrap_or_else(|| throw("System.ArgumentNullException", "s"));
        Self::new_gc(Stream::from_vec(t.as_bytes().to_vec()))
    }
    /// `new StreamReader(stream)`
    pub fn new_gc(stream: Gc<Stream>) -> Gc<StreamReader> {
        if stream.is_null() {
            throw("System.ArgumentNullException", "stream");
        }
        // SAFETY: checked non-null; a live runtime object.
        let s = unsafe { &mut *stream.ptr() };
        if !s.can_read() {
            throw("System.ArgumentException", "Stream was not readable.");
        }
        let mut rest = vec![0u8; (s.length() - s.position()).max(0) as usize];
        let n = s.read_slice(&mut rest);
        rest.truncate(n);
        rt::alloc_init(StreamReader { stream, text: decode_text(&rest).chars().collect(), pos: 0 })
    }
    /// `new StreamReader(path)`
    pub fn open_gc(path: Str) -> Gc<StreamReader> {
        Self::new_gc(file_open_read(path))
    }
    pub fn end_of_stream(&self) -> bool {
        self.pos >= self.text.len()
    }
    /// `ReadLine()`: null at the end; a line ends at `\n`, `\r` or `\r\n` (not part of the result).
    pub fn read_line(&mut self) -> Str {
        if self.pos >= self.text.len() {
            return Str::NULL;
        }
        let start = self.pos;
        while self.pos < self.text.len() && self.text[self.pos] != '\n' && self.text[self.pos] != '\r' {
            self.pos += 1;
        }
        let line: String = self.text[start..self.pos].iter().collect();
        if self.pos < self.text.len() {
            let c = self.text[self.pos];
            self.pos += 1;
            if c == '\r' && self.text.get(self.pos) == Some(&'\n') {
                self.pos += 1;
            }
        }
        Str::new(&line)
    }
    /// `ReadToEnd()`
    pub fn read_to_end(&mut self) -> Str {
        let s: String = self.text[self.pos..].iter().collect();
        self.pos = self.text.len();
        Str::new(&s)
    }
    /// `Read()`: the next UTF-16 unit, or -1 (characters outside the BMP are returned as U+FFFD: not split here).
    pub fn read(&mut self) -> i32 {
        match self.text.get(self.pos) {
            Some(c) => {
                self.pos += 1;
                if (*c as u32) < 0x10000 {
                    *c as i32
                } else {
                    0xFFFD
                }
            }
            None => -1,
        }
    }
    pub fn peek(&self) -> i32 {
        self.text.get(self.pos).map_or(-1, |c| if (*c as u32) < 0x10000 { *c as i32 } else { 0xFFFD })
    }
    /// `Close()` / `Dispose()`: closes the stream too.
    pub fn close(&mut self) {
        if !self.stream.is_null() {
            // SAFETY: checked non-null.
            unsafe { (*self.stream.ptr()).close() };
        }
    }
}

/// `System.IO.StreamWriter`: UTF-8 without a byte order mark, `NewLine` = `\r\n`.
pub struct StreamWriter {
    pub stream: Gc<Stream>,
}
// SAFETY: the one handle is reported.
unsafe impl Trace for StreamWriter {
    unsafe fn trace(&self, m: &mut Marker) {
        m.mark(self.stream)
    }
}
impl StreamWriter {
    /// `new StreamWriter(stream)`
    pub fn new_gc(stream: Gc<Stream>) -> Gc<StreamWriter> {
        if stream.is_null() {
            throw("System.ArgumentNullException", "stream");
        }
        rt::alloc_init(StreamWriter { stream })
    }
    /// `new StreamWriter(path)` / `(path, append)`
    pub fn open_gc(path: Str, append: bool) -> Gc<StreamWriter> {
        writable(text(path, "path"));
        let mode = if append { rt::io::file_mode::APPEND } else { rt::io::file_mode::CREATE };
        Self::new_gc(Stream::open_file(path, mode, false, true))
    }
    fn s(&self) -> &mut Stream {
        // SAFETY: non-null since construction; a live runtime object.
        unsafe { &mut *self.stream.ptr() }
    }
    /// `Write(string)`: null writes nothing.
    pub fn write(&mut self, value: Str) {
        if let Some(v) = value.get() {
            self.s().write_slice(v.as_bytes());
        }
    }
    /// `WriteLine(string)`
    pub fn write_line(&mut self, value: Str) {
        self.write(value);
        self.s().write_slice(b"\r\n");
    }
    pub fn flush(&mut self) {
        self.s().flush();
    }
    /// `Close()` / `Dispose()`
    pub fn close(&mut self) {
        self.s().close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: Str) -> String {
        match v.get() {
            Some(t) => format!("[{t}]"),
            None => "[]".into(),
        }
    }

    /// Every `GetFileName` / `GetExtension` / ... line of tests/dotnet/system_probe.out (real .NET Framework).
    #[test]
    fn path_functions_match_dotnet() {
        let probe = include_str!("../../tests/dotnet/system_probe.out");
        let mut checked = 0;
        for line in probe.lines() {
            let Some((call, expected)) = line.trim_start_matches('\u{feff}').split_once(" = ") else { continue };
            let Some((name, arg)) = call.split_once('(') else { continue };
            let arg = Str::new(arg.strip_suffix(')').unwrap_or(arg));
            let got = match name {
                "GetFileName" => s(path_get_file_name(arg)),
                "GetFileNameWithoutExtension" => s(path_get_file_name_without_extension(arg)),
                "GetExtension" => s(path_get_extension(arg)),
                "GetDirectoryName" => s(path_get_directory_name(arg)),
                "HasExtension" => if path_has_extension(arg) { "True" } else { "False" }.to_owned(),
                "IsPathRooted" => if path_is_path_rooted(arg) { "True" } else { "False" }.to_owned(),
                _ => continue,
            };
            assert_eq!(got, expected, "{call}");
            checked += 1;
        }
        assert_eq!(checked, 59, "all Path lines of the probe");
        assert_eq!(path_change_extension(Str::lit("a\\b.txt"), Str::lit(".bak")).as_str(), "a\\b.bak");
        assert_eq!(path_change_extension(Str::lit("a\\b"), Str::lit("bak")).as_str(), "a\\b.bak");
        assert_eq!(path_change_extension(Str::lit("a\\b.txt"), Str::NULL).as_str(), "a\\b.");
        let chars: Vec<String> = path_get_invalid_file_name_chars().as_slice().iter().map(|c| c.to_string()).collect();
        assert!(probe.contains(&format!("InvalidFileNameChars = {}", chars.join(","))));
        let chars: Vec<String> = path_get_invalid_path_chars().as_slice().iter().map(|c| c.to_string()).collect();
        assert!(probe.contains(&format!("InvalidPathChars = {}", chars.join(","))));
        assert!(path_get_directory_name(Str::NULL).is_null() && path_get_file_name(Str::NULL).is_null());
    }

    fn temp(name: &str) -> PathBuf {
        let d = crate::scratch_dir().join("terraria_host_tests").join(name);
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn files_and_directories() {
        let d = temp("io");
        let dir = Str::new(&d.to_string_lossy());
        let f = Str::new(&d.join("a.txt").to_string_lossy());
        assert!(!file_exists(f) && directory_exists(dir) && !file_exists(dir) && !file_exists(Str::NULL));
        file_write_all_text(f, Str::lit("one\r\ntwo\nthree"));
        file_append_all_text(f, Str::lit("\rfour"));
        assert_eq!(file_read_all_text(f).as_str(), "one\r\ntwo\nthree\rfour");
        let r = StreamReader::open_gc(f);
        unsafe {
            let r = &mut *r.ptr();
            assert_eq!([r.read_line().as_str(), r.read_line().as_str(), r.read_line().as_str(), r.read_line().as_str()], ["one", "two", "three", "four"]);
            assert!(r.read_line().is_null() && r.end_of_stream());
        }
        let info = FileInfo::new_gc(f);
        unsafe {
            assert_eq!(((*info.ptr()).length(), (*info.ptr()).exists(), (*info.ptr()).name().as_str()), (19, true, "a.txt"));
        }
        let b = Str::new(&d.join("B.WLD").to_string_lossy());
        file_copy(f, b, false);
        let e = rt::try_(|| file_copy(f, b, false)).unwrap_err();
        assert!(rt::exc_is(e, "System.IO.IOException"));
        let sub = Str::new(&d.join("sub").join("deep").to_string_lossy());
        directory_create_directory(sub);
        let lists = crate::host_lists_directories();
        if lists {
            let files: Vec<&str> = directory_get_files(dir, Str::lit("*.wld")).as_slice().iter().map(|s| s.as_str()).collect();
            assert_eq!(files, [b.as_str()]);
            assert_eq!(directory_get_files_all(dir).len(), 2);
            assert_eq!(directory_get_directories(dir).len(), 1);
        } else {
            println!("WASM-PARTIAL: directory listing and recursive delete not checked (this host cannot list a folder)");
        }
        let t = file_get_last_write_time_utc(f);
        assert!(t.ticks() > 638_000_000_000_000_000 && t.kind() == crate::sys::time::KIND_UTC);
        assert_eq!(file_get_last_write_time_utc(Str::lit("C:\\no\\such\\file")).ticks(), 504911232000000000);
        file_delete(f);
        file_delete(f);
        assert!(!file_exists(f));
        let e = rt::try_(|| file_read_all_text(f)).unwrap_err();
        assert!(rt::exc_is(e, "System.IO.FileNotFoundException"));
        let e = rt::try_(|| file_delete(Str::new(&d.join("nodir").join("x").to_string_lossy()))).unwrap_err();
        assert!(rt::exc_is(e, "System.IO.DirectoryNotFoundException"));
        if lists {
            directory_delete(dir, true);
            assert!(!directory_exists(dir));
        }
    }

    #[test]
    fn text_decoding_and_writer() {
        assert_eq!(decode_text(&[0xEF, 0xBB, 0xBF, b'a']), "a");
        assert_eq!(decode_text(&[0xFF, 0xFE, b'a', 0, 0xE9, 0]), "a\u{e9}");
        assert_eq!(decode_text(&[0xFE, 0xFF, 0, b'a']), "a");
        assert_eq!(decode_text(&[b'a', 0xFF]), "a\u{fffd}");
        let ms = Stream::new_memory();
        let w = StreamWriter::new_gc(ms);
        unsafe {
            (*w.ptr()).write_line(Str::lit("h\u{e9}"));
            (*w.ptr()).write(Str::NULL);
            (*w.ptr()).write(Str::lit("x"));
            assert_eq!((*ms.ptr()).bytes(), [b'h', 0xC3, 0xA9, b'\r', b'\n', b'x']);
        }
        assert!(wildcard_match("*.plr", "Name.PLR") && wildcard_match("*.*", "noext") && wildcard_match("a?c*", "abcdef") && !wildcard_match("*.plr", "a.plr.bak"));
    }

    #[test]
    fn the_install_folder_is_read_only() {
        let inside = Str::new(&crate::content::install_dir().join("Content").join("x.txt").to_string_lossy());
        let e = rt::try_(|| file_write_all_text(inside, Str::lit("no"))).unwrap_err();
        assert!(rt::exc_is(e, "System.UnauthorizedAccessException"));
        assert!(save_root().ends_with("port-save") || std::env::var_os("TERRARIA_SAVE_DIR").is_some());
    }
}

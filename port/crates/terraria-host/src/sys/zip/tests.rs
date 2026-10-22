use super::*;
use crate::json::tests::{b64d, sha256_hex};
use std::path::Path;

fn dotnet(file: &str) -> std::path::PathBuf {
    crate::manifest_dir().join("tests").join("dotnet").join(file)
}
fn data(file: &str) -> std::path::PathBuf {
    crate::manifest_dir().join("tests").join("data").join(file)
}
fn s(x: &str) -> Str {
    Str::new(x)
}

/// pack.zip (made by System.IO.Compression: deflate, stored, empty, non-ASCII name, directory entry) against what
/// Ionic.Zip.CF.dll says about it: names, flags, sizes, CRC and SHA-256 of the extracted bytes.
#[test]
fn zip_entries_match_ionic() {
    let expected = std::fs::read_to_string(dotnet("zip.expected")).expect("zip.expected");
    let z = ZipFile::read(s(data("pack.zip").to_str().unwrap()));
    let z = unsafe { &*z.ptr() };
    let entries = z.entries();
    let lines: Vec<&str> = expected.lines().collect();
    assert_eq!(entries.len() as usize, lines.len() - 1, "entry count");
    for (e, line) in entries.as_slice().iter().zip(&lines) {
        let p: Vec<&str> = line.split('\t').collect();
        let e = unsafe { &*e.ptr() };
        assert_eq!(e.file_name().as_str(), p[0]);
        assert_eq!(if e.is_directory() { "True" } else { "False" }, p[1], "{}", p[0]);
        assert_eq!(e.uncompressed_size().to_string(), p[2], "{}", p[0]);
        assert_eq!(e.compressed_size().to_string(), p[3], "{}", p[0]);
        assert_eq!(e.crc().to_string(), p[4], "{}", p[0]);
        let bytes = if e.is_directory() { Vec::new() } else { e.bytes() };
        assert_eq!(sha256_hex(&bytes), p[5], "{} content", p[0]);
    }
    // lookups: case-insensitive, \ = /, ContainsEntry, count
    let l: Vec<&str> = lines[lines.len() - 1].split('\t').collect();
    let b = |x: bool| if x { "True" } else { "False" };
    assert_eq!(b(z.contains_entry(s("content\\images\\item_1.png"))), l[1]);
    assert_eq!(b(z.contains_entry(s("Content/Images/Item_1.png"))), l[2]);
    assert_eq!(b(z.contains_entry(s("nope"))), l[3]);
    assert_eq!(b(!z.get(s("CONTENT/DESCRIPTION.TXT")).is_null()), l[4]);
    assert_eq!(z.count().to_string(), l[5]);
    // Extract into a stream and OpenReader
    let e = unsafe { &*z.get(s("pack.json")).ptr() };
    let out = Stream::new_memory();
    e.extract(out);
    assert_eq!(unsafe { (*out.ptr()).bytes().to_vec() }, e.bytes());
    assert!(unsafe { (*e.open_reader().ptr()).length() } == e.uncompressed_size());
}

#[test]
fn broken_archives_are_reported_not_misread() {
    assert!(rt::run(|| ZipFile::from_bytes(vec![1, 2, 3, 4])).is_err());
    // a flipped byte in the compressed data: Ionic throws BadCrcException (or BadReadException for bad deflate data)
    let mut bytes = std::fs::read(data("pack.zip")).unwrap();
    let z = ZipFile::from_bytes(bytes.clone());
    let e = unsafe { &*(&*z.ptr()).get(s("Content/stored.bin")).ptr() };
    bytes[e.start + 100] ^= 0xFF;
    let z2 = ZipFile::from_bytes(bytes);
    let e2 = unsafe { &*(&*z2.ptr()).get(s("Content/stored.bin")).ptr() };
    let err = rt::run(|| e2.bytes()).unwrap_err();
    assert!(err.starts_with("Ionic.Zip.BadCrcException"), "{err}");
    assert!(rt::run(|| ZipFile::read(s("C:/no/such/file.zip"))).unwrap_err().contains("FileNotFoundException"));
}

#[test]
fn crc32_known_answer() {
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    assert_eq!(crc32(b""), 0);
}

fn payload() -> Vec<u8> {
    (0..20000usize).map(|i| ((i * 7 + i / 13) % 251) as u8).collect()
}

/// Streams made by Ionic.Zlib (raw deflate and zlib) decode here; what DeflateStream compresses decodes here again.
#[test]
fn deflate_stream_and_zlib_codec_match_ionic() {
    let t = std::fs::read_to_string(dotnet("deflate.expected")).expect("deflate.expected");
    let mut lines = t.lines().map(|l| l.split('\t').collect::<Vec<_>>());
    let p = lines.next().unwrap();
    let (hash, len) = (p[1].to_owned(), p[2].parse::<usize>().unwrap());
    let raw = decode_b64(lines.next().unwrap()[1]);
    let zl = decode_b64(lines.next().unwrap()[1]);
    // DeflateStream(Decompress)
    let out = deflate_stream_new(Stream::from_vec(raw), 1, 0, false);
    let got = unsafe { (*out.ptr()).bytes().to_vec() };
    assert_eq!((got.len(), sha256_hex(&got)), (len, hash.clone()), "Ionic raw deflate");
    // ZlibCodec.Inflate (MapHelper's call sequence)
    let codec = ZlibCodec::new_gc(1);
    let c = unsafe { &mut *codec.ptr() };
    assert_eq!(c.initialize_inflate(), 0);
    c.input_buffer = Arr::from_slice(&zl);
    c.output_buffer = Arr::<u8>::new(len as i32 + 100);
    c.next_in = 0;
    c.available_bytes_in = zl.len() as i32;
    c.next_out = 0;
    c.available_bytes_out = len as i32 + 100;
    assert_eq!(c.inflate(4), 1, "Z_STREAM_END");
    assert_eq!(c.total_bytes_out() as usize, len);
    assert_eq!(sha256_hex(&c.output_buffer.as_slice()[..len]), hash, "Ionic zlib");
    // a buffer that is too small is Z_BUF_ERROR, not a partial result
    c.initialize_inflate();
    c.next_in = 0;
    c.available_bytes_in = zl.len() as i32;
    c.next_out = 0;
    c.available_bytes_out = 100;
    assert_eq!(c.inflate(4), -5);
    // DeflateStream(Compress): written data comes out at finish, as deflate that inflates to the same bytes
    let target = Stream::new_memory();
    let w = deflate_stream_new(target, 0, 6, true);
    unsafe { (*w.ptr()).write_slice(&payload()) };
    assert_eq!(unsafe { (*target.ptr()).length() }, 0, "nothing reaches the target before Dispose");
    deflate_stream_finish(w);
    deflate_stream_finish(w);
    let comp = unsafe { (*target.ptr()).bytes().to_vec() };
    assert!(comp.len() < 2000, "{} bytes", comp.len());
    assert_eq!(miniz_oxide::inflate::decompress_to_vec(&comp).unwrap(), payload());
    // written to a file for the cross-check with Ionic / .NET (tests/dotnet/zip_probe.ps1 documents the reader side)
    if let Some(dir) = std::env::var_os("TERRARIA_TEST_OUT") {
        std::fs::write(Path::new(&dir).join("rust.deflate"), &comp).unwrap();
    }
}

fn decode_b64(s: &str) -> Vec<u8> {
    let t = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::new();
    let (mut acc, mut bits) = (0u32, 0);
    for c in s.bytes().filter(|c| *c != b'=') {
        acc = acc << 6 | t.iter().position(|x| *x == c).unwrap() as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    let _ = b64d;
    out
}

//! Walks the real Terraria `Content` folder and parses every `.xnb` file.
//! Skipped (with a message) when the folder does not exist.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use terraria_formats::xnb::{read_xnb, Asset, SurfaceFormat, Xnb};

const CONTENT: &str = r"C:\Users\user\Documents\Games\Terraria\Content";

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.filter_map(Result::ok).map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            collect(&p, out);
        } else if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("xnb")) {
            out.push(p);
        }
    }
}

#[derive(Default)]
struct Stats {
    files: usize,
    errors: Vec<String>,
    unknown: Vec<String>,
    per_type: BTreeMap<&'static str, usize>,
    surface_formats: BTreeMap<String, usize>,
    dxt_files: Vec<String>,
    readers: BTreeSet<String>,
    compressed: usize,
    file_bytes: u64,
    payload_bytes: u64,
    decoded_asset_bytes: u64,
    header_size_mismatch: usize,
    shared_resources: usize,
    wave_formats: BTreeSet<String>,
    tiles0: Option<(u32, u32, usize, usize)>,
}

fn account(s: &mut Stats, rel: &str, x: &Xnb, file_len: usize) {
    s.files += 1;
    s.file_bytes += file_len as u64;
    s.payload_bytes += x.payload_size as u64;
    s.compressed += x.compressed as usize;
    s.shared_resources += x.shared.len();
    if x.file_size as usize != file_len {
        s.header_size_mismatch += 1;
    }
    for r in &x.readers {
        s.readers.insert(r.name.clone());
    }
    *s.per_type.entry(x.asset.kind()).or_default() += 1;
    match &x.asset {
        Asset::Texture2D(t) => {
            let name = format!("{:?}", t.format);
            *s.surface_formats.entry(name).or_default() += 1;
            if matches!(t.format, SurfaceFormat::Dxt1 | SurfaceFormat::Dxt3 | SurfaceFormat::Dxt5) {
                s.dxt_files.push(rel.to_string());
            }
            s.decoded_asset_bytes += t.mips.iter().map(|m| m.len() as u64).sum::<u64>();
            match t.top_mip_rgba8() {
                Ok(rgba) => {
                    assert_eq!(rgba.len(), t.width as usize * t.height as usize * 4, "{rel}");
                    if rel == r"Images\Tiles_0.xnb" {
                        let nonzero_alpha = rgba.chunks_exact(4).filter(|p| p[3] != 0).count();
                        s.tiles0 = Some((t.width, t.height, nonzero_alpha, t.mips.len()));
                    }
                }
                Err(e) => s.errors.push(format!("{rel}: rgba8: {e}")),
            }
        }
        Asset::SoundEffect(snd) => {
            s.decoded_asset_bytes += snd.data.len() as u64;
            s.wave_formats.insert(format!(
                "tag={} ch={} bits={} extra={}B",
                snd.format_tag,
                snd.channels,
                snd.bits_per_sample,
                snd.format_extra.len()
            ));
        }
        Asset::Effect(e) => s.decoded_asset_bytes += e.bytecode.len() as u64,
        Asset::DynamicSpriteFont(f) => {
            for p in &f.pages {
                s.decoded_asset_bytes += p.texture.mips.iter().map(|m| m.len() as u64).sum::<u64>();
                *s.surface_formats.entry(format!("{:?} (font page)", p.texture.format)).or_default() += 1;
                match p.texture.top_mip_rgba8() {
                    Ok(rgba) => assert_eq!(rgba.len(), p.texture.width as usize * p.texture.height as usize * 4),
                    Err(e) => s.errors.push(format!("{rel}: font page rgba8: {e}")),
                }
                assert_eq!(p.glyphs.len(), p.characters.len(), "{rel}: glyph/char count");
                assert_eq!(p.padding.len(), p.characters.len(), "{rel}: padding/char count");
                assert_eq!(p.kerning.len(), p.characters.len(), "{rel}: kerning/char count");
            }
        }
        Asset::SpriteFont(f) => {
            s.decoded_asset_bytes += f.texture.mips.iter().map(|m| m.len() as u64).sum::<u64>();
        }
        Asset::Unknown { reader_name } => s.unknown.push(format!("{rel}: {reader_name}")),
        Asset::Null => s.unknown.push(format!("{rel}: null primary object")),
    }
}

#[test]
fn parse_entire_content_folder() {
    let root = Path::new(CONTENT);
    if !root.is_dir() {
        eprintln!("SKIPPED: Terraria Content folder not found at {CONTENT}");
        return;
    }
    let mut files = Vec::new();
    collect(root, &mut files);
    assert!(!files.is_empty(), "no .xnb files under {CONTENT}");

    let stats = Mutex::new(Stats::default());
    let next = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some(path) = files.get(i) else { break };
                let rel = path.strip_prefix(root).unwrap_or(path).display().to_string();
                let result = std::fs::read(path)
                    .map_err(|e| format!("{rel}: read: {e}"))
                    .and_then(|b| read_xnb(&b).map(|x| (x, b.len())).map_err(|e| format!("{rel}: {e}")));
                let mut s = stats.lock().unwrap();
                match result {
                    Ok((x, len)) => account(&mut s, &rel, &x, len),
                    Err(e) => {
                        s.files += 1;
                        s.errors.push(e);
                    }
                }
            });
        }
    });
    let s = stats.into_inner().unwrap();

    println!("files parsed: {}", s.files);
    println!("compressed files: {}", s.compressed);
    println!("assets per type: {:?}", s.per_type);
    println!("file bytes: {}  decompressed payload bytes: {}", s.file_bytes, s.payload_bytes);
    println!("decoded asset bytes (mips + pcm + bytecode): {}", s.decoded_asset_bytes);
    println!("surface formats: {:?}", s.surface_formats);
    println!("DXT files: {} {:?}", s.dxt_files.len(), &s.dxt_files[..s.dxt_files.len().min(10)]);
    println!("wave formats: {:?}", s.wave_formats);
    println!("shared resources: {}", s.shared_resources);
    println!("distinct reader names ({}):", s.readers.len());
    for r in &s.readers {
        println!("  {r}");
    }
    println!("Images\\Tiles_0.xnb: {:?} (width, height, nonzero-alpha px, mips)", s.tiles0);

    for e in s.errors.iter().take(20) {
        eprintln!("ERROR {e}");
    }
    for u in s.unknown.iter().take(20) {
        eprintln!("UNKNOWN {u}");
    }
    assert_eq!(s.errors.len(), 0, "{} files failed", s.errors.len());
    assert_eq!(s.unknown.len(), 0, "{} Unknown assets", s.unknown.len());
    assert_eq!(s.header_size_mismatch, 0);
    assert_eq!(s.files, files.len());

    // Spot check: the dirt tile sheet.
    let (w, h, opaque, mips) = s.tiles0.expect("Images\\Tiles_0.xnb missing or not a texture");
    assert_eq!((w, h), TILES0_SIZE, "Tiles_0 dimensions");
    assert!(opaque > 0, "Tiles_0 has no non-zero alpha");
    assert_eq!(mips, 1);
}

/// Measured from the file itself (see the printed line of this test).
const TILES0_SIZE: (u32, u32) = (288, 270);

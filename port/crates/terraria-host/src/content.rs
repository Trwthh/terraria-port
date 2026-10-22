//! Content: `ContentManager.Load<T>` over `terraria_formats::xnb`, and the native edge of ReLogic's asset system.
//!
//! Everything is read from the player's own install (`TERRARIA_DIR`, default `C:\Users\user\Documents\Games\Terraria`),
//! which is READ-ONLY for this crate: nothing here writes, and `sys::io` refuses writes below it.
//!
//! # ReLogic's asset system
//! `Asset<T>`, `AssetRepository`, the content sources and `DynamicSpriteFont` are ReLogic code and are translated.
//! What they need from the platform is small, so the edge is a direct native hook instead of XNA's
//! `ContentReader` / `ContentTypeReader` machinery (which `XnbReader.InternalContentManager` drives through
//! `ContentManager.ReadAsset<T>` and reflection over reader type names):
//! - `ReLogic.Content.Readers.XnbReader.FromStream<T>(Stream)` -> [`xnb_from_stream_texture`], [`xnb_from_stream_effect`],
//!   [`xnb_from_stream_sound`], and [`xnb_from_stream_font`] (a [`FontData`] the translated side turns into a
//!   `DynamicSpriteFont` with its own constructor + `SetPages`; REQUESTS.md "DynamicSpriteFont from FontData");
//! - `ReLogic.Content.Readers.PngReader` is translated as is: it calls `Texture2D.FromStream` ([`texture_from_stream`])
//!   and `GetData` / `SetData` (premultiplication), which this crate provides;
//! - `Microsoft.Xna.Framework.TitleContainer.OpenStream(name)` -> [`title_container_open_stream`];
//! - `Directory.GetFiles(root, "*.xnb", SearchOption.AllDirectories)` -> [`get_files_recursive`].

use std::collections::HashMap;
use std::path::PathBuf;

use terraria_core::xna::{Rectangle, Vector3};
use terraria_formats::xnb::{self, Asset as XnbAsset};
use terraria_gen_rt::host;
use terraria_gen_rt::rt::{self, io::Stream, Arr, Gc, Marker, Str, Trace};

use crate::audio::SoundEffect;
use crate::gfx::effect::Effect;
use crate::gfx::states::throw;
use crate::gfx::{GraphicsDevice, Texture2D};

/// The game's install folder: `TERRARIA_DIR`, else the path this project was built against; on WASI the sandbox's
/// `/install` (the runner maps the player's install there: web/ci/wasi-run.mjs).
pub fn install_dir() -> PathBuf {
    match std::env::var_os("TERRARIA_DIR") {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        #[cfg(target_os = "wasi")]
        _ => PathBuf::from("/install"),
        #[cfg(not(target_os = "wasi"))]
        _ => PathBuf::from(r"C:\Users\user\Documents\Games\Terraria"),
    }
}

#[cold]
fn load_failed(name: &str, why: &str) -> ! {
    rt::throw(rt::exc("Microsoft.Xna.Framework.Content.ContentLoadException", Str::new(&format!("Error loading \"{name}\". {why}"))))
}

fn parse(name: &str, bytes: &[u8]) -> xnb::Xnb {
    // the browser build: a long run of content loads on the main stack gives the display frame back between assets,
    // and a large compressed one (a font of 10 MB: 0.35 s of LZX in one call) between its LZX frames
    #[cfg(target_family = "wasm")]
    {
        crate::sys::threading::main_yield::check();
        match parse_sliced(bytes, SLICED_LZX_FROM, crate::sys::threading::main_yield::check) {
            Some(Ok(x)) => return x,
            Some(Err(e)) => load_failed(name, &e),
            None => {}
        }
    }
    match xnb::read_xnb(bytes) {
        Ok(x) => x,
        Err(e) => load_failed(name, &format!("{e:?}")),
    }
}

/// XNB files at least this large are decompressed by [`decompress_in_slices`] in the browser build.
#[cfg(target_family = "wasm")]
const SLICED_LZX_FROM: usize = 512 << 10;

/// An LZX-compressed XNB file of `min_size` bytes or more as the same file uncompressed (header flag cleared, sizes
/// adjusted), decompressed frame by frame with `between()` called after every 16 frames (512 KB of output): the
/// browser build's main stack gives the display frame back there. The framing and the decoder are those of
/// `terraria_formats::xnb::lzx_decompress` (the same `lzxd` frames in the same order), so the payload is identical
/// (test `sliced_lzx_equals_the_formats_decoder`). `None`: not compressed, or smaller than `min_size`.
pub fn decompress_in_slices(bytes: &[u8], min_size: usize, mut between: impl FnMut()) -> Option<Result<Vec<u8>, String>> {
    const HEADER: usize = 10;
    if bytes.len() < min_size || bytes.len() < HEADER + 4 || &bytes[..3] != b"XNB" || bytes[5] & 0x80 == 0 {
        return None;
    }
    let expected = u32::from_le_bytes([bytes[10], bytes[11], bytes[12], bytes[13]]) as usize;
    let data = &bytes[HEADER + 4..];
    let mut out = Vec::with_capacity(HEADER + expected);
    out.extend_from_slice(&bytes[..HEADER]);
    out[5] &= !0x80;
    let mut dec = lzxd::Lzxd::new(lzxd::WindowSize::KB64);
    let mut at = 0usize;
    let mut frames = 0u32;
    let take = |at: &mut usize, n: usize| -> Result<&[u8], String> {
        let s = data.get(*at..*at + n).ok_or_else(|| String::from("LZX stream truncated"))?;
        *at += n;
        Ok(s)
    };
    while at < data.len() {
        let first = match take(&mut at, 1) {
            Ok(h) => h[0],
            Err(e) => return Some(Err(e)),
        };
        let (frame_size, block_size) = if first == 0xFF {
            let h = match take(&mut at, 4) {
                Ok(h) => h,
                Err(e) => return Some(Err(e)),
            };
            (u16::from_be_bytes([h[0], h[1]]) as usize, u16::from_be_bytes([h[2], h[3]]) as usize)
        } else {
            let second = match take(&mut at, 1) {
                Ok(h) => h[0],
                Err(e) => return Some(Err(e)),
            };
            (0x8000, ((first as usize) << 8) | second as usize)
        };
        if block_size == 0 || frame_size == 0 {
            break;
        }
        if frame_size > lzxd::MAX_CHUNK_SIZE {
            return Some(Err("LZX frame larger than 32 KB".into()));
        }
        let block = match take(&mut at, block_size) {
            Ok(b) => b,
            Err(e) => return Some(Err(e)),
        };
        match dec.decompress_next(block, frame_size) {
            Ok(f) => out.extend_from_slice(f),
            Err(e) => return Some(Err(e.to_string())),
        }
        frames += 1;
        if frames % 16 == 0 {
            between();
        }
    }
    if out.len() != HEADER + expected {
        return Some(Err(format!("decompressed size {} instead of {expected}", out.len() - HEADER)));
    }
    let size = (out.len() as u32).to_le_bytes();
    out[6..10].copy_from_slice(&size);
    Some(Ok(out))
}

/// `xnb::read_xnb` of a file [`decompress_in_slices`] takes: the same `Xnb` (its header fields are the compressed
/// file's, as `read_xnb` gives them). `None`: as `decompress_in_slices`.
pub fn parse_sliced(bytes: &[u8], min_size: usize, between: impl FnMut()) -> Option<Result<xnb::Xnb, String>> {
    let plain = match decompress_in_slices(bytes, min_size, between)? {
        Ok(p) => p,
        Err(e) => return Some(Err(e)),
    };
    Some(match xnb::read_xnb(&plain) {
        Ok(mut x) => {
            x.flags = bytes[5];
            x.compressed = true;
            x.file_size = bytes.len() as u32;
            Ok(x)
        }
        Err(e) => Err(format!("{e:?}")),
    })
}

/// A `Texture2D` from a decoded XNB texture. `source`: the file to re-read for `GetData` (no CPU copy is kept).
pub fn texture_from_xnb(device: Gc<GraphicsDevice>, name: &str, tex: &xnb::Texture2D, source: Option<PathBuf>) -> Gc<Texture2D> {
    // as in `parse`: between two textures (a font's pages)
    #[cfg(target_family = "wasm")]
    crate::sys::threading::main_yield::check();
    let rgba = match tex.top_mip_rgba8() {
        Ok(p) => p,
        Err(e) => load_failed(name, &format!("{e:?}")),
    };
    let t = Texture2D::from_rgba(device, tex.width as i32, tex.height as i32, rgba, false);
    // SAFETY: fresh object.
    unsafe {
        (*t.ptr()).name = Str::new(name);
        if let Some(path) = source {
            (*t.ptr()).reload = Some(Box::new(move || match xnb::read_xnb(&std::fs::read(&path).ok()?).ok()?.asset {
                XnbAsset::Texture2D(t) => t.top_mip_rgba8().ok(),
                _ => None,
            }));
        }
    }
    t
}

/// An `Effect` from compiled effect bytecode: its techniques, passes and parameters come from the D3DX effect
/// binary ([`crate::gfx::fx_binary::parse`]). When the binary cannot be parsed the load fails
/// (`unimplemented_value`): an effect whose pass names are unknown cannot answer `Passes["name"]` truthfully.
pub fn effect_from_bytecode(device: Gc<GraphicsDevice>, name: &str, bytecode: &[u8]) -> Gc<Effect> {
    match crate::gfx::fx_binary::parse(bytecode) {
        Ok(desc) => Effect::new_gc(device, name, &desc),
        Err(e) => {
            eprintln!("[content] effect {name}: {e}");
            host::unimplemented_value("Microsoft.Xna.Framework.Content.ContentManager.Load<Effect>(string): this effect binary could not be parsed")
        }
    }
}

/// `ContentManager`: loads `<install>\<RootDirectory>\<assetName>.xnb`, caching by name like XNA.
pub struct ContentManager {
    pub device: Gc<GraphicsDevice>,
    pub root_directory: Str,
    textures: HashMap<String, Gc<Texture2D>>,
    effects: HashMap<String, Gc<Effect>>,
    sounds: HashMap<String, Gc<SoundEffect>>,
}
// SAFETY: every handle is reported.
unsafe impl Trace for ContentManager {
    unsafe fn trace(&self, m: &mut Marker) {
        m.mark(self.device);
        self.root_directory.trace(m);
        for t in self.textures.values() {
            m.mark(*t);
        }
        for e in self.effects.values() {
            m.mark(*e);
        }
        for s in self.sounds.values() {
            m.mark(*s);
        }
    }
}

impl ContentManager {
    /// `new ContentManager(services)` / `(services, rootDirectory)`; the graphics device is what XNA takes from
    /// `IGraphicsDeviceService` in the service provider.
    pub fn new_gc(device: Gc<GraphicsDevice>, root_directory: Str) -> Gc<ContentManager> {
        rt::alloc_init(ContentManager { device, root_directory: if root_directory.is_null() { Str::lit("") } else { root_directory }, textures: HashMap::new(), effects: HashMap::new(), sounds: HashMap::new() })
    }
    pub fn root_directory(&self) -> Str {
        self.root_directory
    }
    pub fn set_root_directory(&mut self, v: Str) {
        if v.is_null() {
            throw("System.ArgumentNullException", "value");
        }
        self.root_directory = v;
    }

    /// `<install>\<RootDirectory>\<assetName>.xnb`
    pub fn path_of(&self, asset_name: &str) -> PathBuf {
        install_dir().join(self.root_directory.as_str()).join(format!("{asset_name}.xnb"))
    }

    fn read(&self, asset_name: Str) -> (String, PathBuf, xnb::Xnb) {
        let Some(name) = asset_name.get().filter(|n| !n.is_empty()) else { throw("System.ArgumentNullException", "assetName") };
        let path = self.path_of(name);
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(_) => load_failed(name, "File not found."),
        };
        let x = parse(name, &bytes);
        (name.to_owned(), path, x)
    }

    /// The cache key XNA uses: the asset name with `/` as `\`... XNA normalises with `Path` rules; the game always
    /// spells a name the same way, so the name itself (separators unified) is the key.
    fn key(name: &str) -> String {
        name.replace('/', "\\")
    }

    /// `Load<Texture2D>(assetName)`
    pub fn load_texture(&mut self, asset_name: Str) -> Gc<Texture2D> {
        if let Some(t) = asset_name.get().and_then(|n| self.textures.get(&Self::key(n))) {
            return *t;
        }
        let (name, path, x) = self.read(asset_name);
        let XnbAsset::Texture2D(tex) = &x.asset else { load_failed(&name, &format!("The file holds a {}, not a Texture2D.", x.asset.kind())) };
        let t = texture_from_xnb(self.device, &name, tex, Some(path));
        self.textures.insert(Self::key(&name), t);
        t
    }
    /// `Load<Effect>(assetName)`
    pub fn load_effect(&mut self, asset_name: Str) -> Gc<Effect> {
        if let Some(e) = asset_name.get().and_then(|n| self.effects.get(&Self::key(n))) {
            return *e;
        }
        let (name, _, x) = self.read(asset_name);
        let XnbAsset::Effect(fx) = &x.asset else { load_failed(&name, &format!("The file holds a {}, not an Effect.", x.asset.kind())) };
        let e = effect_from_bytecode(self.device, &name, &fx.bytecode);
        self.effects.insert(Self::key(&name), e);
        e
    }
    /// `Load<SoundEffect>(assetName)`
    pub fn load_sound_effect(&mut self, asset_name: Str) -> Gc<SoundEffect> {
        if let Some(s) = asset_name.get().and_then(|n| self.sounds.get(&Self::key(n))) {
            return *s;
        }
        let (name, _, x) = self.read(asset_name);
        let XnbAsset::SoundEffect(snd) = &x.asset else { load_failed(&name, &format!("The file holds a {}, not a SoundEffect.", x.asset.kind())) };
        let s = SoundEffect::from_xnb(&name, snd);
        self.sounds.insert(Self::key(&name), s);
        s
    }
    /// `Load<DynamicSpriteFont>` / `Load<SpriteFont>` as raw font data (see the module docs).
    pub fn load_font(&mut self, asset_name: Str) -> Gc<FontData> {
        let (name, _, x) = self.read(asset_name);
        font_from_xnb(self.device, &name, &x)
    }
    /// `Unload()`: drops the cache (XNA also disposes what it loaded).
    pub fn unload(&mut self) {
        for t in self.textures.values() {
            // SAFETY: cached objects are alive (traced by this manager).
            unsafe { (*t.ptr()).dispose() };
        }
        self.textures.clear();
        self.effects.clear();
        self.sounds.clear();
    }
}

// ------------------------------------------------------------------------------------------------ fonts

/// One page of a `DynamicSpriteFont` (ReLogic `FontPage`'s constructor arguments).
#[derive(Clone, Copy)]
pub struct FontPageData {
    pub texture: Gc<Texture2D>,
    pub glyphs: Arr<Rectangle>,
    pub padding: Arr<Rectangle>,
    pub characters: Arr<u16>,
    pub kerning: Arr<Vector3>,
}
// SAFETY: every handle is reported.
unsafe impl Trace for FontPageData {
    unsafe fn trace(&self, m: &mut Marker) {
        m.mark(self.texture);
        self.glyphs.trace(m);
        self.padding.trace(m);
        self.characters.trace(m);
        self.kerning.trace(m);
    }
}

/// The fields `DynamicSpriteFontReader.Read` reads, in its order.
pub struct FontData {
    pub spacing: f32,
    pub line_spacing: i32,
    pub default_character: u16,
    pub pages: Arr<FontPageData>,
}
// SAFETY: the one handle is reported.
unsafe impl Trace for FontData {
    unsafe fn trace(&self, m: &mut Marker) {
        self.pages.trace(m)
    }
}
impl FontData {
    pub fn spacing(&self) -> f32 {
        self.spacing
    }
    pub fn line_spacing(&self) -> i32 {
        self.line_spacing
    }
    pub fn default_character(&self) -> u16 {
        self.default_character
    }
    pub fn pages(&self) -> Arr<FontPageData> {
        self.pages
    }
}

fn char16(c: char) -> u16 {
    let mut b = [0u16; 2];
    c.encode_utf16(&mut b)[0]
}

fn font_from_xnb(device: Gc<GraphicsDevice>, name: &str, x: &xnb::Xnb) -> Gc<FontData> {
    let XnbAsset::DynamicSpriteFont(f) = &x.asset else { load_failed(name, &format!("The file holds a {}, not a DynamicSpriteFont.", x.asset.kind())) };
    let rect = |r: &xnb::Rect| Rectangle::new(r.x, r.y, r.w, r.h);
    let pages: Vec<FontPageData> = f
        .pages
        .iter()
        .enumerate()
        .map(|(i, p)| FontPageData {
            texture: texture_from_xnb(device, &format!("{name}#page{i}"), &p.texture, None),
            glyphs: Arr::from_slice(&p.glyphs.iter().map(rect).collect::<Vec<_>>()),
            padding: Arr::from_slice(&p.padding.iter().map(rect).collect::<Vec<_>>()),
            characters: Arr::from_slice(&p.characters.iter().map(|c| char16(*c)).collect::<Vec<_>>()),
            kerning: Arr::from_slice(&p.kerning.iter().map(|k| Vector3 { x: k.x, y: k.y, z: k.z }).collect::<Vec<_>>()),
        })
        .collect();
    rt::alloc_init(FontData { spacing: f.spacing, line_spacing: f.line_spacing, default_character: char16(f.default_character), pages: Arr::from_slice(&pages) })
}

// ------------------------------------------------------------------------------------------------ ReLogic's edge

fn rest_of(stream: Gc<Stream>) -> Vec<u8> {
    if stream.is_null() {
        throw("System.ArgumentNullException", "stream");
    }
    // SAFETY: checked non-null; a live runtime object.
    let s = unsafe { &mut *stream.ptr() };
    let mut rest = vec![0u8; (s.length() - s.position()).max(0) as usize];
    let n = s.read_slice(&mut rest);
    rest.truncate(n);
    rest
}

/// `XnbReader.FromStream<Texture2D>(stream)`
pub fn xnb_from_stream_texture(device: Gc<GraphicsDevice>, stream: Gc<Stream>) -> Gc<Texture2D> {
    // a stream that holds a whole file: GetData re-reads the file instead of the pixels being kept (they were, for
    // every texture of the game: hundreds of MB, which the browser build's 4 GB address space cannot spare)
    // SAFETY: a live stream object.
    let source = unsafe { (*stream.ptr()).source_file().map(std::path::Path::to_path_buf) }.filter(|_| unsafe { (*stream.ptr()).position() } == 0);
    let x = parse("XnaAsset", &rest_of(stream));
    let XnbAsset::Texture2D(tex) = &x.asset else { load_failed("XnaAsset", &format!("The stream holds a {}, not a Texture2D.", x.asset.kind())) };
    if let Some(path) = source {
        let label = path.to_string_lossy().into_owned();
        let t = texture_from_xnb(device, &label, tex, Some(path));
        // this path never named its textures (Texture2D.Name stays null, as before)
        // SAFETY: fresh object.
        unsafe { (*t.ptr()).name = Str::NULL };
        return t;
    }
    // no file to re-read: keep the pixels for GetData
    let rgba = tex.top_mip_rgba8().unwrap_or_else(|e| load_failed("XnaAsset", &format!("{e:?}")));
    Texture2D::from_rgba(device, tex.width as i32, tex.height as i32, rgba, true)
}
/// `XnbReader.FromStream<Effect>(stream)`
pub fn xnb_from_stream_effect(device: Gc<GraphicsDevice>, stream: Gc<Stream>, name: Str) -> Gc<Effect> {
    let x = parse("XnaAsset", &rest_of(stream));
    let XnbAsset::Effect(fx) = &x.asset else { load_failed("XnaAsset", &format!("The stream holds a {}, not an Effect.", x.asset.kind())) };
    effect_from_bytecode(device, name.get().unwrap_or("XnaAsset"), &fx.bytecode)
}
/// `XnbReader.FromStream<SoundEffect>(stream)`
pub fn xnb_from_stream_sound(stream: Gc<Stream>) -> Gc<SoundEffect> {
    // the content file the stream reads (the audio trace names a sound effect by it, as the reference names it by the
    // file XNA's content reader opened)
    // SAFETY: a live stream object.
    let name = unsafe { (*stream.ptr()).source_file().map(|p| p.to_string_lossy().into_owned()) }.unwrap_or_else(|| "XnaAsset".to_owned());
    let x = parse("XnaAsset", &rest_of(stream));
    let XnbAsset::SoundEffect(s) = &x.asset else { load_failed("XnaAsset", &format!("The stream holds a {}, not a SoundEffect.", x.asset.kind())) };
    SoundEffect::from_xnb(&name, s)
}
/// `XnbReader.FromStream<DynamicSpriteFont>(stream)` as [`FontData`].
pub fn xnb_from_stream_font(device: Gc<GraphicsDevice>, stream: Gc<Stream>) -> Gc<FontData> {
    let x = parse("XnaAsset", &rest_of(stream));
    font_from_xnb(device, "XnaAsset", &x)
}

/// A class `XnbReader.FromStream<T>` can produce. The host implements it for its own classes; the translated
/// crate implements it for `DynamicSpriteFont` (built from [`FontData`]), which the host cannot construct.
pub trait XnbObject: Sized + 'static {
    fn from_xnb_stream(stream: Gc<Stream>) -> Gc<Self>;
}
impl XnbObject for Texture2D {
    fn from_xnb_stream(stream: Gc<Stream>) -> Gc<Self> {
        xnb_from_stream_texture(crate::game::graphics_device(), stream)
    }
}
impl XnbObject for Effect {
    fn from_xnb_stream(stream: Gc<Stream>) -> Gc<Self> {
        xnb_from_stream_effect(crate::game::graphics_device(), stream, Str::NULL)
    }
}
impl XnbObject for SoundEffect {
    fn from_xnb_stream(stream: Gc<Stream>) -> Gc<Self> {
        xnb_from_stream_sound(stream)
    }
}
/// The handle type of an [`XnbObject`] (`T` of `FromStream<T>` arrives as `Gc<Class>`).
pub trait XnbHandle: Copy {
    fn load(stream: Gc<Stream>) -> Self;
}
impl<T: XnbObject> XnbHandle for Gc<T> {
    fn load(stream: Gc<Stream>) -> Self {
        T::from_xnb_stream(stream)
    }
}
/// `XnbReader.FromStream<T>(stream)` (ReLogic): the platform edge of the translated asset system. The original
/// goes through a private `ContentManager` subclass and XNA's `ContentTypeReader`s, found by reflection over reader
/// type names; here the XNB container is parsed directly.
pub fn xnb_from_stream<H: XnbHandle>(stream: Gc<Stream>) -> H {
    H::load(stream)
}

/// `TitleContainer.OpenStream(name)`: a read-only stream over a file below the install folder
/// (`FileNotFoundException` when it is not there, as XNA).
pub fn title_container_open_stream(name: Str) -> Gc<Stream> {
    let Some(n) = name.get() else { throw("System.ArgumentNullException", "name") };
    let path = install_dir().join(n);
    match std::fs::read(&path) {
        Ok(b) => {
            let s = Stream::from_vec(b);
            // SAFETY: fresh object.
            unsafe { (*s.ptr()).set_source_file(path) };
            s
        }
        Err(_) => rt::throw(rt::exc("System.IO.FileNotFoundException", Str::new(&format!("Error loading \"{n}\". File not found.")))),
    }
}

/// `Directory.GetFiles(path, searchPattern, SearchOption.AllDirectories)`: `path` relative to the install folder
/// when it is not rooted (XNA content roots are title-relative). Full paths come back as `path` + the relative
/// part with `\`, folder by folder, names in upper-case order.
pub fn get_files_recursive(path: Str, pattern: Str) -> Arr<Str> {
    let (Some(p), Some(pat)) = (path.get(), pattern.get()) else { throw("System.ArgumentNullException", "path") };
    // the game writes Windows separators; a host where `\` is no separator gets its own (the result keeps `\`)
    let host = if cfg!(windows) { p.to_owned() } else { p.replace('\\', "/") };
    let root = if std::path::Path::new(&host).is_absolute() { PathBuf::from(&host) } else { install_dir().join(&host) };
    if !root.is_dir() {
        rt::throw(rt::exc("System.IO.DirectoryNotFoundException", Str::new(&format!("Could not find a part of the path '{p}'."))));
    }
    fn walk(dir: &std::path::Path, shown: &str, pat: &str, out: &mut Vec<Str>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        let mut entries: Vec<(String, bool)> = rd.filter_map(|e| e.ok()).map(|e| (e.file_name().to_string_lossy().into_owned(), e.file_type().is_ok_and(|t| t.is_dir()))).collect();
        entries.sort_by_key(|(n, _)| n.to_uppercase());
        for (n, is_dir) in entries.iter().filter(|(_, d)| !d) {
            let _ = is_dir;
            if crate::sys::io::wildcard_match(pat, n) {
                out.push(Str::new(&format!("{shown}\\{n}")));
            }
        }
        for (n, _) in entries.iter().filter(|(_, d)| *d) {
            walk(&dir.join(n), &format!("{shown}\\{n}"), pat, out);
        }
    }
    let mut out = Vec::new();
    walk(&root, p.trim_end_matches(['\\', '/']), pat, &mut out);
    Arr::from_slice(&out)
}

/// `Texture2D.FromStream(graphicsDevice, stream)`: PNG only (XNA also reads JPG / GIF / BMP; the game's callers
/// give it PNG files from resource packs and workshop previews). Pixels are NOT premultiplied, as in XNA 4.
/// The pixels are kept on the CPU: `PngReader` reads them back at once.
pub fn texture_from_stream(device: Gc<GraphicsDevice>, stream: Gc<Stream>) -> Gc<Texture2D> {
    let bytes = rest_of(stream);
    if !bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        return host::unimplemented_value("Microsoft.Xna.Framework.Graphics.Texture2D.FromStream(GraphicsDevice, Stream) for an image that is not a PNG");
    }
    match decode_png(&bytes) {
        Ok((w, h, rgba)) => Texture2D::from_rgba(device, w as i32, h as i32, rgba, true),
        Err(e) => rt::throw(rt::exc("System.InvalidOperationException", Str::new(&format!("This image format is not supported: {e}")))),
    }
}

/// PNG -> (width, height, RGBA8 rows), any colour type and bit depth.
pub fn decode_png(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    buf.truncate(info.buffer_size());
    let rgba: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => buf.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        png::ColorType::Grayscale => buf.iter().flat_map(|g| [*g, *g, *g, 255]).collect(),
        png::ColorType::GrayscaleAlpha => buf.chunks_exact(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
        png::ColorType::Indexed => return Err("indexed colour was not expanded".into()),
    };
    Ok((info.width, info.height, rgba))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn have_install() -> bool {
        install_dir().join("Content").join("Images").join("Tiles_0.xnb").is_file()
    }

    /// The browser build's sliced LZX decompression gives the bytes terraria-formats' decoder gives, for every font
    /// (the largest compressed files) and a texture; the uncompressed file it makes parses to the same asset.
    #[test]
    fn sliced_lzx_equals_the_formats_decoder() {
        if !have_install() {
            eprintln!("skipped: no Terraria install at {}", install_dir().display());
            return;
        }
        for f in ["Fonts/Death_Text", "Fonts/Combat_Crit", "Fonts/Combat_Text", "Fonts/Mouse_Text", "Fonts/Item_Stack", "Images/Tiles_0"] {
            let bytes = std::fs::read(install_dir().join("Content").join(format!("{f}.xnb"))).unwrap();
            assert!(bytes[5] & 0x80 != 0, "{f}: compressed");
            let mut calls = 0;
            let out = super::decompress_in_slices(&bytes, 0, || calls += 1).unwrap().unwrap();
            let expected = u32::from_le_bytes([bytes[10], bytes[11], bytes[12], bytes[13]]) as usize;
            let reference = xnb::lzx_decompress(&bytes[14..], expected).unwrap();
            assert_eq!(&out[10..], &reference[..], "{f}: payload");
            assert_eq!((out[5] & 0x80, u32::from_le_bytes([out[6], out[7], out[8], out[9]]) as usize), (0, out.len()), "{f}: header");
            assert_eq!(calls, (reference.len().div_ceil(0x8000) / 16) as i32, "{f}: a call every 16 frames");
            assert!(super::parse_sliced(&bytes, 0, || ()).unwrap().unwrap() == xnb::read_xnb(&bytes).unwrap(), "{f}: the same Xnb");
        }
        assert!(super::decompress_in_slices(b"XNBw\x05\x00\x0e\x00\x00\x00abcd", 0, || ()).is_none(), "not compressed");
    }

    /// Needs the player's install; skipped (with a note) when it is not there.
    #[test]
    fn loads_from_the_install_and_caches() {
        if !have_install() {
            eprintln!("skipped: no Terraria install at {}", install_dir().display());
            return;
        }
        unsafe {
            let d = GraphicsDevice::new_gc(800, 600);
            let cm = ContentManager::new_gc(d, Str::lit("Content"));
            let c = &mut *cm.ptr();
            let t = c.load_texture(Str::lit("Images/Tiles_0"));
            assert_eq!(c.load_texture(Str::lit("Images\\Tiles_0")), t, "one object per asset");
            let tex = &mut *t.ptr();
            assert_eq!((tex.width(), tex.height()), (288, 270));
            // GetData re-reads the file
            let px = Arr::<terraria_core::xna::Color>::new(288 * 270);
            tex.get_data(px);
            assert!(px.as_slice().iter().any(|c| c.a() != 0));
            let s = &*c.load_sound_effect(Str::lit("Sounds/Item_1")).ptr();
            assert!(s.duration().ticks > 0 && !s.pcm().is_empty());
            let e = rt::try_(|| c.load_texture(Str::lit("Images/NoSuchThing"))).unwrap_err();
            assert!(rt::exc_text(e).contains("NoSuchThing"));
            let font = &*c.load_font(Str::lit("Fonts/Mouse_Text")).ptr();
            assert!(font.line_spacing() > 0 && font.pages().len() > 0);
            let page = *font.pages().at(0);
            assert_eq!(page.glyphs.len(), page.characters.len());
            // the ReLogic path: a stream from TitleContainer through the XNB hook
            let stream = title_container_open_stream(Str::lit("Content\\Images\\Tiles_1.xnb"));
            let t2 = &mut *xnb_from_stream_texture(d, stream).ptr();
            assert!(t2.width() > 0 && t2.pixels().is_some());
            if crate::host_lists_directories() {
                let files = get_files_recursive(Str::lit("Content\\Fonts"), Str::lit("*.xnb"));
                assert!(files.len() >= 5 && files.as_slice()[0].as_str().starts_with("Content\\Fonts\\"));
            } else {
                println!("WASM-PARTIAL: get_files_recursive not checked (this host cannot list a folder)");
            }
            let e = rt::try_(|| title_container_open_stream(Str::lit("Content\\nope.xnb"))).unwrap_err();
            assert!(rt::exc_is(e, "System.IO.FileNotFoundException"));
        }
    }

    #[test]
    fn png_from_stream_is_not_premultiplied() {
        // 2x1 RGBA PNG: (255, 0, 0, 128), (0, 255, 0, 255)
        let mut png_bytes = Vec::new();
        {
            let mut e = png::Encoder::new(&mut png_bytes, 2, 1);
            e.set_color(png::ColorType::Rgba);
            e.set_depth(png::BitDepth::Eight);
            e.write_header().unwrap().write_image_data(&[255, 0, 0, 128, 0, 255, 0, 255]).unwrap();
        }
        unsafe {
            let d = GraphicsDevice::new_gc(8, 8);
            let t = &mut *texture_from_stream(d, Stream::from_vec(png_bytes)).ptr();
            assert_eq!((t.width(), t.height()), (2, 1));
            let px = Arr::<terraria_core::xna::Color>::new(2);
            t.get_data_range(px, 0, 2);
            assert_eq!(px.as_slice()[0].to_rgba_bytes(), [255, 0, 0, 128]);
            // what PngReader does next: premultiply and SetData
            *px.at(0) = terraria_core::xna::Color::new(128, 0, 0, 128);
            t.set_data_range(px, 0, 2);
            assert_eq!(t.pixels().unwrap(), [128, 0, 0, 128, 0, 255, 0, 255]);
        }
    }
}

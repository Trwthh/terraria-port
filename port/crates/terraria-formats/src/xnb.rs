//! XNB content container (XNA Game Studio 4.0, format version 5), as used by
//! Terraria 1.4.x `Content\**\*.xnb`.
//!
//! Layout of a file:
//!
//! ```text
//! "XNB" platform:u8 version:u8 flags:u8 file_size:u32
//! [decompressed_size:u32]            only when flags & 0x80 (LZX compressed)
//! payload                            (LZX framed when compressed)
//! ```
//!
//! The payload is `reader_count:7bit { name:string version:i32 }* shared_count:7bit`
//! followed by the primary object and then `shared_count` shared objects. Every object
//! is a 7-bit type id (1-based index into the reader list, 0 = null) plus data.
//!
//! Supported readers: `Texture2DReader`, `SoundEffectReader`, `EffectReader`,
//! `SpriteFontReader` and ReLogic's `DynamicSpriteFontReader` (with the helper readers
//! they use: `ListReader<Rectangle|char|Vector3>`). Anything else is reported as
//! [`Asset::Unknown`] without being parsed.

use std::fmt;

use lzxd::{Lzxd, WindowSize, MAX_CHUNK_SIZE};

/// XNB flag: payload is LZX compressed.
const FLAG_COMPRESSED: u8 = 0x80;
/// XNB flag: content targets the HiDef profile (informational).
const FLAG_HIDEF: u8 = 0x01;

/// Errors produced while reading an XNB file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XnbError {
    /// The data ended before `needed` more bytes could be read while reading `context`.
    Truncated { context: &'static str, needed: usize, available: usize },
    /// The first three bytes are not `XNB`.
    BadMagic,
    /// Only XNB version 5 (XNA 4.0) is supported.
    UnsupportedVersion(u8),
    /// A flag bit other than HiDef (0x01) / LZX (0x80) is set.
    UnsupportedFlags(u8),
    /// A size recorded in the file does not match what was found.
    SizeMismatch { what: &'static str, expected: usize, actual: usize },
    /// LZX framing is malformed.
    BadLzxFraming(&'static str),
    /// The LZX decoder rejected a block.
    Lzx(String),
    /// A string was not valid UTF-8.
    InvalidString { context: &'static str },
    /// A 7-bit encoded integer was malformed or too large.
    BadVarInt,
    /// An object referenced a type id outside the reader table.
    InvalidTypeId { id: u32, reader_count: usize },
    /// A null object appeared where a value was required.
    NullObject { context: &'static str },
    /// A nested object used a different reader than required.
    ReaderMismatch { context: &'static str, found: String },
    /// A count or size field is negative or implausibly large.
    InvalidValue { context: &'static str, value: i64 },
    /// Bytes remained after the last object was read.
    TrailingBytes { count: usize },
    /// The texture surface format has no RGBA8 conversion.
    UnsupportedSurfaceFormat(i32),
    /// The texture has no mip levels.
    NoMips,
    /// Texture data length does not match its dimensions and format.
    BadTextureData { expected: usize, actual: usize },
}

impl fmt::Display for XnbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated { context, needed, available } => {
                write!(f, "truncated while reading {context}: need {needed} bytes, {available} available")
            }
            Self::BadMagic => write!(f, "not an XNB file (bad magic)"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported XNB version {v} (only 5 is supported)"),
            Self::UnsupportedFlags(x) => write!(f, "unsupported XNB flags 0x{x:02x}"),
            Self::SizeMismatch { what, expected, actual } => {
                write!(f, "{what} mismatch: header says {expected}, found {actual}")
            }
            Self::BadLzxFraming(m) => write!(f, "bad LZX framing: {m}"),
            Self::Lzx(m) => write!(f, "LZX decode error: {m}"),
            Self::InvalidString { context } => write!(f, "invalid UTF-8 string in {context}"),
            Self::BadVarInt => write!(f, "malformed 7-bit encoded integer"),
            Self::InvalidTypeId { id, reader_count } => {
                write!(f, "type id {id} out of range ({reader_count} readers)")
            }
            Self::NullObject { context } => write!(f, "unexpected null object in {context}"),
            Self::ReaderMismatch { context, found } => {
                write!(f, "wrong reader in {context}: found {found}")
            }
            Self::InvalidValue { context, value } => write!(f, "invalid value {value} in {context}"),
            Self::TrailingBytes { count } => write!(f, "{count} unread bytes after the last object"),
            Self::UnsupportedSurfaceFormat(x) => write!(f, "no RGBA8 conversion for surface format {x}"),
            Self::NoMips => write!(f, "texture has no mip levels"),
            Self::BadTextureData { expected, actual } => {
                write!(f, "texture data is {actual} bytes, expected {expected}")
            }
        }
    }
}

impl std::error::Error for XnbError {}

/// One entry of the type reader table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeReader {
    /// Full reader name including the assembly suffix, as stored in the file.
    pub name: String,
    pub version: i32,
}

/// Texture surface formats (XNA `SurfaceFormat`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceFormat {
    Color,
    Bgr565,
    Bgra5551,
    Bgra4444,
    Dxt1,
    Dxt3,
    Dxt5,
    /// Any other XNA surface format value (not convertible to RGBA8 by this module).
    Other(i32),
}

impl SurfaceFormat {
    pub fn from_i32(v: i32) -> Self {
        match v {
            0 => Self::Color,
            1 => Self::Bgr565,
            2 => Self::Bgra5551,
            3 => Self::Bgra4444,
            4 => Self::Dxt1,
            5 => Self::Dxt3,
            6 => Self::Dxt5,
            x => Self::Other(x),
        }
    }
}

/// A `Texture2D`: mip chain in the stored surface format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Texture2D {
    pub format: SurfaceFormat,
    pub width: u32,
    pub height: u32,
    /// Raw mip data, level 0 first.
    pub mips: Vec<Vec<u8>>,
}

impl Texture2D {
    /// Top mip (level 0) converted to tightly packed RGBA8 (`width * height * 4` bytes,
    /// straight copy for `Color`).
    pub fn top_mip_rgba8(&self) -> Result<Vec<u8>, XnbError> {
        let data = self.mips.first().ok_or(XnbError::NoMips)?;
        let (w, h) = (self.width as usize, self.height as usize);
        let px = w.checked_mul(h).ok_or(XnbError::InvalidValue {
            context: "texture dimensions",
            value: -1,
        })?;
        let blocks = w.div_ceil(4) * h.div_ceil(4);
        let (expected, bytes_per_px_or_block) = match self.format {
            SurfaceFormat::Color => (px * 4, 4),
            SurfaceFormat::Bgr565 | SurfaceFormat::Bgra5551 | SurfaceFormat::Bgra4444 => (px * 2, 2),
            SurfaceFormat::Dxt1 => (blocks * 8, 8),
            SurfaceFormat::Dxt3 | SurfaceFormat::Dxt5 => (blocks * 16, 16),
            SurfaceFormat::Other(x) => return Err(XnbError::UnsupportedSurfaceFormat(x)),
        };
        let _ = bytes_per_px_or_block;
        if data.len() != expected {
            return Err(XnbError::BadTextureData { expected, actual: data.len() });
        }
        let mut out = vec![0u8; px * 4];
        match self.format {
            SurfaceFormat::Color => out.copy_from_slice(data),
            SurfaceFormat::Bgr565 => {
                for (o, s) in out.chunks_exact_mut(4).zip(data.chunks_exact(2)) {
                    let (r, g, b) = unpack565(u16::from_le_bytes([s[0], s[1]]));
                    o.copy_from_slice(&[r, g, b, 255]);
                }
            }
            SurfaceFormat::Bgra5551 => {
                for (o, s) in out.chunks_exact_mut(4).zip(data.chunks_exact(2)) {
                    let v = u16::from_le_bytes([s[0], s[1]]);
                    let e5 = |x: u16| ((x << 3) | (x >> 2)) as u8;
                    let a = if v & 0x8000 != 0 { 255 } else { 0 };
                    o.copy_from_slice(&[e5((v >> 10) & 31), e5((v >> 5) & 31), e5(v & 31), a]);
                }
            }
            SurfaceFormat::Bgra4444 => {
                for (o, s) in out.chunks_exact_mut(4).zip(data.chunks_exact(2)) {
                    let v = u16::from_le_bytes([s[0], s[1]]);
                    let e4 = |x: u16| (x * 17) as u8;
                    o.copy_from_slice(&[e4((v >> 8) & 15), e4((v >> 4) & 15), e4(v & 15), e4(v >> 12)]);
                }
            }
            SurfaceFormat::Dxt1 | SurfaceFormat::Dxt3 | SurfaceFormat::Dxt5 => {
                let block_bytes = bytes_per_px_or_block;
                let bw = w.div_ceil(4);
                for (i, blk) in data.chunks_exact(block_bytes).enumerate() {
                    let (bx, by) = (i % bw, i / bw);
                    let mut px16 = [[0u8; 4]; 16];
                    match self.format {
                        SurfaceFormat::Dxt1 => decode_color_block(&blk[0..8], true, &mut px16),
                        SurfaceFormat::Dxt3 => {
                            decode_color_block(&blk[8..16], false, &mut px16);
                            for k in 0..16 {
                                let nib = (blk[k / 2] >> ((k % 2) * 4)) & 15;
                                px16[k][3] = nib * 17;
                            }
                        }
                        _ => {
                            decode_color_block(&blk[8..16], false, &mut px16);
                            let (a0, a1) = (blk[0] as u32, blk[1] as u32);
                            let mut pal = [0u8; 8];
                            pal[0] = a0 as u8;
                            pal[1] = a1 as u8;
                            if a0 > a1 {
                                for j in 1..7u32 {
                                    pal[1 + j as usize] = (((7 - j) * a0 + j * a1) / 7) as u8;
                                }
                            } else {
                                for j in 1..5u32 {
                                    pal[1 + j as usize] = (((5 - j) * a0 + j * a1) / 5) as u8;
                                }
                                pal[6] = 0;
                                pal[7] = 255;
                            }
                            let mut bits = 0u64;
                            for (k, b) in blk[2..8].iter().enumerate() {
                                bits |= (*b as u64) << (8 * k);
                            }
                            for (k, p) in px16.iter_mut().enumerate() {
                                p[3] = pal[((bits >> (3 * k)) & 7) as usize];
                            }
                        }
                    }
                    for (k, p) in px16.iter().enumerate() {
                        let (x, y) = (bx * 4 + k % 4, by * 4 + k / 4);
                        if x < w && y < h {
                            let o = (y * w + x) * 4;
                            out[o..o + 4].copy_from_slice(p);
                        }
                    }
                }
            }
            SurfaceFormat::Other(x) => return Err(XnbError::UnsupportedSurfaceFormat(x)),
        }
        Ok(out)
    }
}

fn unpack565(v: u16) -> (u8, u8, u8) {
    let (r, g, b) = ((v >> 11) & 31, (v >> 5) & 63, v & 31);
    (
        ((r << 3) | (r >> 2)) as u8,
        ((g << 2) | (g >> 4)) as u8,
        ((b << 3) | (b >> 2)) as u8,
    )
}

/// Decode an 8-byte BC1 colour block. `dxt1` enables the 3-colour + transparent mode.
fn decode_color_block(blk: &[u8], dxt1: bool, out: &mut [[u8; 4]; 16]) {
    let c0 = u16::from_le_bytes([blk[0], blk[1]]);
    let c1 = u16::from_le_bytes([blk[2], blk[3]]);
    let (r0, g0, b0) = unpack565(c0);
    let (r1, g1, b1) = unpack565(c1);
    let mix = |a: u8, b: u8, wa: u32, wb: u32, d: u32| ((wa * a as u32 + wb * b as u32) / d) as u8;
    let mut pal = [[r0, g0, b0, 255], [r1, g1, b1, 255], [0; 4], [0; 4]];
    if c0 > c1 || !dxt1 {
        pal[2] = [mix(r0, r1, 2, 1, 3), mix(g0, g1, 2, 1, 3), mix(b0, b1, 2, 1, 3), 255];
        pal[3] = [mix(r0, r1, 1, 2, 3), mix(g0, g1, 1, 2, 3), mix(b0, b1, 1, 2, 3), 255];
    } else {
        pal[2] = [mix(r0, r1, 1, 1, 2), mix(g0, g1, 1, 1, 2), mix(b0, b1, 1, 1, 2), 255];
        pal[3] = [0, 0, 0, 0];
    }
    let idx = u32::from_le_bytes([blk[4], blk[5], blk[6], blk[7]]);
    for (k, p) in out.iter_mut().enumerate() {
        *p = pal[((idx >> (2 * k)) & 3) as usize];
    }
}

/// A `SoundEffect`: a WAVEFORMATEX header plus raw sample data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoundEffect {
    /// WAVE format tag (1 = PCM, 2 = MS ADPCM, ...).
    pub format_tag: u16,
    pub channels: u16,
    pub sample_rate: u32,
    pub avg_bytes_per_sec: u32,
    pub block_align: u16,
    pub bits_per_sample: u16,
    /// Format bytes after the 16 byte base header (`cbSize` field and extra data).
    pub format_extra: Vec<u8>,
    pub data: Vec<u8>,
    /// Loop start, in samples.
    pub loop_start: i32,
    /// Loop length, in samples.
    pub loop_length: i32,
    pub duration_ms: i32,
}

/// A rectangle (`x`, `y`, `width`, `height`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// Kerning triple (XNA `Vector3`: left bearing, width, right bearing).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// One glyph page of a [`DynamicSpriteFont`] (ReLogic `FontPage`).
#[derive(Debug, Clone, PartialEq)]
pub struct FontPage {
    pub texture: Texture2D,
    pub glyphs: Vec<Rect>,
    pub padding: Vec<Rect>,
    pub characters: Vec<char>,
    pub kerning: Vec<Vec3>,
}

/// ReLogic `DynamicSpriteFont`, field order as in `DynamicSpriteFontReader.Read`.
#[derive(Debug, Clone, PartialEq)]
pub struct DynamicSpriteFont {
    pub spacing: f32,
    pub line_spacing: i32,
    pub default_character: char,
    pub pages: Vec<FontPage>,
}

/// XNA `SpriteFont`.
#[derive(Debug, Clone, PartialEq)]
pub struct SpriteFont {
    pub texture: Texture2D,
    pub glyphs: Vec<Rect>,
    pub cropping: Vec<Rect>,
    pub characters: Vec<char>,
    pub line_spacing: i32,
    pub spacing: f32,
    pub kerning: Vec<Vec3>,
    pub default_character: Option<char>,
}

/// Compiled effect (shader) bytecode blob.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Effect {
    pub bytecode: Vec<u8>,
}

/// A decoded top-level or shared object.
#[derive(Debug, Clone, PartialEq)]
pub enum Asset {
    Texture2D(Texture2D),
    SoundEffect(SoundEffect),
    DynamicSpriteFont(DynamicSpriteFont),
    SpriteFont(SpriteFont),
    Effect(Effect),
    /// Reader not understood; the object was not parsed.
    Unknown { reader_name: String },
    /// A null object (type id 0).
    Null,
}

impl Asset {
    /// Short kind label for statistics.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Texture2D(_) => "Texture2D",
            Self::SoundEffect(_) => "SoundEffect",
            Self::DynamicSpriteFont(_) => "DynamicSpriteFont",
            Self::SpriteFont(_) => "SpriteFont",
            Self::Effect(_) => "Effect",
            Self::Unknown { .. } => "Unknown",
            Self::Null => "Null",
        }
    }
}

/// A parsed XNB file.
#[derive(Debug, Clone, PartialEq)]
pub struct Xnb {
    /// Platform byte: `b'w'` Windows, `b'x'` Xbox 360, `b'm'` Windows Phone.
    pub platform: u8,
    pub version: u8,
    pub flags: u8,
    /// Whether the payload was LZX compressed.
    pub compressed: bool,
    pub hidef: bool,
    /// File size field from the header.
    pub file_size: u32,
    /// Size of the decompressed payload (equals the payload length when uncompressed).
    pub payload_size: usize,
    pub readers: Vec<TypeReader>,
    pub asset: Asset,
    /// Shared resources that follow the primary object.
    pub shared: Vec<Asset>,
}

/// Parse an XNB file. Never panics on malformed input.
pub fn read_xnb(bytes: &[u8]) -> Result<Xnb, XnbError> {
    let mut hdr = Cursor::new(bytes);
    if hdr.take(3, "magic")? != b"XNB" {
        return Err(XnbError::BadMagic);
    }
    let platform = hdr.u8("platform")?;
    let version = hdr.u8("version")?;
    if version != 5 {
        return Err(XnbError::UnsupportedVersion(version));
    }
    let flags = hdr.u8("flags")?;
    if flags & !(FLAG_COMPRESSED | FLAG_HIDEF) != 0 {
        return Err(XnbError::UnsupportedFlags(flags));
    }
    let file_size = hdr.u32("file size")?;
    if file_size as usize != bytes.len() {
        return Err(XnbError::SizeMismatch {
            what: "file size",
            expected: file_size as usize,
            actual: bytes.len(),
        });
    }
    let compressed = flags & FLAG_COMPRESSED != 0;
    let owned;
    let payload: &[u8] = if compressed {
        let decompressed = hdr.u32("decompressed size")? as usize;
        owned = lzx_decompress(hdr.rest(), decompressed)?;
        &owned
    } else {
        hdr.rest()
    };

    let mut c = Cursor::new(payload);
    let reader_count = c.varint("reader count")? as usize;
    let mut readers = Vec::with_capacity(reader_count.min(64));
    for _ in 0..reader_count {
        let name = c.string("reader name")?;
        let version = c.i32("reader version")?;
        readers.push(TypeReader { name, version });
    }
    let kinds: Vec<Kind> = readers.iter().map(|r| Kind::of(&r.name)).collect();
    let shared_count = c.varint("shared resource count")? as usize;

    let ctx = Ctx { readers: &readers, kinds: &kinds };
    let asset = ctx.read_asset(&mut c)?;
    let mut shared = Vec::new();
    if !matches!(asset, Asset::Unknown { .. }) {
        for _ in 0..shared_count {
            let a = ctx.read_asset(&mut c)?;
            let stop = matches!(a, Asset::Unknown { .. });
            shared.push(a);
            if stop {
                break;
            }
        }
        if c.remaining() != 0 {
            return Err(XnbError::TrailingBytes { count: c.remaining() });
        }
    }

    Ok(Xnb {
        platform,
        version,
        flags,
        compressed,
        hidef: flags & FLAG_HIDEF != 0,
        file_size,
        payload_size: payload.len(),
        readers,
        asset,
        shared,
    })
}

/// Undo XNA's LZX framing and decode with a 64 KB window.
///
/// Each frame is `[0xFF frame_size:u16be] block_size:u16be data[block_size]`; without the
/// 0xFF marker the frame size is 0x8000. A zero frame or block size ends the stream.
/// `expected` is the decompressed size from the header and must match exactly.
pub fn lzx_decompress(data: &[u8], expected: usize) -> Result<Vec<u8>, XnbError> {
    let mut dec = Lzxd::new(WindowSize::KB64);
    let mut out = Vec::with_capacity(expected.min(64 << 20));
    let mut c = Cursor::new(data);
    while c.remaining() > 0 {
        let first = c.u8("lzx frame header")?;
        let (frame_size, block_size) = if first == 0xFF {
            let fs = c.u16_be("lzx frame size")? as usize;
            (fs, c.u16_be("lzx block size")? as usize)
        } else {
            let second = c.u8("lzx block size")?;
            (0x8000, ((first as usize) << 8) | second as usize)
        };
        if block_size == 0 || frame_size == 0 {
            break;
        }
        if frame_size > MAX_CHUNK_SIZE {
            return Err(XnbError::BadLzxFraming("frame larger than 32 KB"));
        }
        let block = c.take(block_size, "lzx block")?;
        let frame = dec
            .decompress_next(block, frame_size)
            .map_err(|e| XnbError::Lzx(e.to_string()))?;
        out.extend_from_slice(frame);
    }
    if out.len() != expected {
        return Err(XnbError::SizeMismatch {
            what: "decompressed size",
            expected,
            actual: out.len(),
        });
    }
    Ok(out)
}

/// What a type reader name means to this module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Texture2D,
    SoundEffect,
    Effect,
    SpriteFont,
    DynamicSpriteFont,
    ListRect,
    ListChar,
    ListVec3,
    Other,
}

impl Kind {
    fn of(name: &str) -> Self {
        // Names look like `Ns.Reader, Assembly, Version=...` or
        // `Ns.ListReader`1[[Ns.Rectangle, Assembly, ...]], Assembly, ...`.
        if let Some(rest) = name.strip_prefix("Microsoft.Xna.Framework.Content.ListReader`1[[") {
            let elem = rest.split(',').next().unwrap_or("");
            return match elem {
                "Microsoft.Xna.Framework.Rectangle" => Self::ListRect,
                "System.Char" => Self::ListChar,
                "Microsoft.Xna.Framework.Vector3" => Self::ListVec3,
                _ => Self::Other,
            };
        }
        match name.split(',').next().unwrap_or("") {
            "Microsoft.Xna.Framework.Content.Texture2DReader" => Self::Texture2D,
            "Microsoft.Xna.Framework.Content.SoundEffectReader" => Self::SoundEffect,
            "Microsoft.Xna.Framework.Content.EffectReader" => Self::Effect,
            "Microsoft.Xna.Framework.Content.SpriteFontReader" => Self::SpriteFont,
            "ReLogic.Graphics.DynamicSpriteFontReader" => Self::DynamicSpriteFont,
            _ => Self::Other,
        }
    }
}

struct Ctx<'a> {
    readers: &'a [TypeReader],
    kinds: &'a [Kind],
}

impl Ctx<'_> {
    /// Read a type id; `None` for a null object.
    fn type_id(&self, c: &mut Cursor<'_>) -> Result<Option<usize>, XnbError> {
        let id = c.varint("type id")?;
        if id == 0 {
            return Ok(None);
        }
        let idx = id as usize - 1;
        if idx >= self.readers.len() {
            return Err(XnbError::InvalidTypeId { id, reader_count: self.readers.len() });
        }
        Ok(Some(idx))
    }

    /// Read an object whose reader must be `want`.
    fn expect(&self, c: &mut Cursor<'_>, want: Kind, context: &'static str) -> Result<(), XnbError> {
        let idx = self.type_id(c)?.ok_or(XnbError::NullObject { context })?;
        if self.kinds[idx] != want {
            return Err(XnbError::ReaderMismatch { context, found: self.readers[idx].name.clone() });
        }
        Ok(())
    }

    fn read_asset(&self, c: &mut Cursor<'_>) -> Result<Asset, XnbError> {
        let Some(idx) = self.type_id(c)? else {
            return Ok(Asset::Null);
        };
        Ok(match self.kinds[idx] {
            Kind::Texture2D => Asset::Texture2D(read_texture(c)?),
            Kind::SoundEffect => Asset::SoundEffect(read_sound(c)?),
            Kind::Effect => {
                let n = c.count("effect length")?;
                Asset::Effect(Effect { bytecode: c.take(n, "effect bytecode")?.to_vec() })
            }
            Kind::SpriteFont => Asset::SpriteFont(self.read_sprite_font(c)?),
            Kind::DynamicSpriteFont => Asset::DynamicSpriteFont(self.read_dynamic_font(c)?),
            Kind::ListRect | Kind::ListChar | Kind::ListVec3 | Kind::Other => {
                Asset::Unknown { reader_name: self.readers[idx].name.clone() }
            }
        })
    }

    fn texture(&self, c: &mut Cursor<'_>) -> Result<Texture2D, XnbError> {
        self.expect(c, Kind::Texture2D, "font texture")?;
        read_texture(c)
    }

    fn rects(&self, c: &mut Cursor<'_>, context: &'static str) -> Result<Vec<Rect>, XnbError> {
        self.expect(c, Kind::ListRect, context)?;
        let n = c.count(context)?;
        c.check_available(n, 16, context)?;
        (0..n)
            .map(|_| {
                Ok(Rect {
                    x: c.i32(context)?,
                    y: c.i32(context)?,
                    w: c.i32(context)?,
                    h: c.i32(context)?,
                })
            })
            .collect()
    }

    fn chars(&self, c: &mut Cursor<'_>, context: &'static str) -> Result<Vec<char>, XnbError> {
        self.expect(c, Kind::ListChar, context)?;
        let n = c.count(context)?;
        c.check_available(n, 1, context)?;
        (0..n).map(|_| c.char(context)).collect()
    }

    fn vec3s(&self, c: &mut Cursor<'_>, context: &'static str) -> Result<Vec<Vec3>, XnbError> {
        self.expect(c, Kind::ListVec3, context)?;
        let n = c.count(context)?;
        c.check_available(n, 12, context)?;
        (0..n)
            .map(|_| Ok(Vec3 { x: c.f32(context)?, y: c.f32(context)?, z: c.f32(context)? }))
            .collect()
    }

    /// ReLogic `DynamicSpriteFontReader.Read`.
    fn read_dynamic_font(&self, c: &mut Cursor<'_>) -> Result<DynamicSpriteFont, XnbError> {
        let spacing = c.f32("font spacing")?;
        let line_spacing = c.i32("font line spacing")?;
        let default_character = c.char("font default character")?;
        let n = c.count("font page count")?;
        let mut pages = Vec::with_capacity(n.min(64));
        for _ in 0..n {
            pages.push(FontPage {
                texture: self.texture(c)?,
                glyphs: self.rects(c, "font glyphs")?,
                padding: self.rects(c, "font padding")?,
                characters: self.chars(c, "font characters")?,
                kerning: self.vec3s(c, "font kerning")?,
            });
        }
        Ok(DynamicSpriteFont { spacing, line_spacing, default_character, pages })
    }

    /// XNA `SpriteFontReader.Read`.
    fn read_sprite_font(&self, c: &mut Cursor<'_>) -> Result<SpriteFont, XnbError> {
        let texture = self.texture(c)?;
        let glyphs = self.rects(c, "font glyphs")?;
        let cropping = self.rects(c, "font cropping")?;
        let characters = self.chars(c, "font characters")?;
        let line_spacing = c.i32("font line spacing")?;
        let spacing = c.f32("font spacing")?;
        let kerning = self.vec3s(c, "font kerning")?;
        let default_character = if c.u8("default character flag")? != 0 {
            Some(c.char("default character")?)
        } else {
            None
        };
        Ok(SpriteFont {
            texture,
            glyphs,
            cropping,
            characters,
            line_spacing,
            spacing,
            kerning,
            default_character,
        })
    }
}

/// `Texture2DReader.Read`.
fn read_texture(c: &mut Cursor<'_>) -> Result<Texture2D, XnbError> {
    let format = SurfaceFormat::from_i32(c.i32("surface format")?);
    let width = c.u32("texture width")?;
    let height = c.u32("texture height")?;
    let mip_count = c.count("mip count")?;
    let mut mips = Vec::with_capacity(mip_count.min(32));
    for _ in 0..mip_count {
        let n = c.count("mip size")?;
        mips.push(c.take(n, "mip data")?.to_vec());
    }
    Ok(Texture2D { format, width, height, mips })
}

/// `SoundEffectReader.Read`.
fn read_sound(c: &mut Cursor<'_>) -> Result<SoundEffect, XnbError> {
    let fmt_len = c.count("sound format length")?;
    if fmt_len < 16 {
        return Err(XnbError::InvalidValue { context: "sound format length", value: fmt_len as i64 });
    }
    let fmt = c.take(fmt_len, "sound format")?;
    let u16at = |i: usize| u16::from_le_bytes([fmt[i], fmt[i + 1]]);
    let u32at = |i: usize| u32::from_le_bytes([fmt[i], fmt[i + 1], fmt[i + 2], fmt[i + 3]]);
    let n = c.count("sound data length")?;
    let data = c.take(n, "sound data")?.to_vec();
    Ok(SoundEffect {
        format_tag: u16at(0),
        channels: u16at(2),
        sample_rate: u32at(4),
        avg_bytes_per_sec: u32at(8),
        block_align: u16at(12),
        bits_per_sample: u16at(14),
        format_extra: fmt[16..].to_vec(),
        data,
        loop_start: c.i32("loop start")?,
        loop_length: c.i32("loop length")?,
        duration_ms: c.i32("duration")?,
    })
}

/// Bounds-checked little-endian byte reader.
struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    fn rest(&mut self) -> &'a [u8] {
        let r = &self.data[self.pos..];
        self.pos = self.data.len();
        r
    }

    fn take(&mut self, n: usize, context: &'static str) -> Result<&'a [u8], XnbError> {
        if n > self.remaining() {
            return Err(XnbError::Truncated { context, needed: n, available: self.remaining() });
        }
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    /// Fail early if `n` items of `each` bytes cannot possibly fit in the rest.
    fn check_available(&self, n: usize, each: usize, context: &'static str) -> Result<(), XnbError> {
        let needed = n.saturating_mul(each);
        if needed > self.remaining() {
            return Err(XnbError::Truncated { context, needed, available: self.remaining() });
        }
        Ok(())
    }

    fn u8(&mut self, context: &'static str) -> Result<u8, XnbError> {
        Ok(self.take(1, context)?[0])
    }

    fn u16_be(&mut self, context: &'static str) -> Result<u16, XnbError> {
        let b = self.take(2, context)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }

    fn u32(&mut self, context: &'static str) -> Result<u32, XnbError> {
        let b = self.take(4, context)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn i32(&mut self, context: &'static str) -> Result<i32, XnbError> {
        Ok(self.u32(context)? as i32)
    }

    fn f32(&mut self, context: &'static str) -> Result<f32, XnbError> {
        Ok(f32::from_bits(self.u32(context)?))
    }

    /// A non-negative i32 length/count.
    fn count(&mut self, context: &'static str) -> Result<usize, XnbError> {
        let v = self.i32(context)?;
        usize::try_from(v).map_err(|_| XnbError::InvalidValue { context, value: v as i64 })
    }

    /// .NET `Write7BitEncodedInt` (at most 5 bytes).
    fn varint(&mut self, _context: &'static str) -> Result<u32, XnbError> {
        let mut v = 0u32;
        for i in 0..5 {
            let b = self.u8("7-bit integer")?;
            v |= ((b & 0x7F) as u32) << (7 * i);
            if b & 0x80 == 0 {
                return Ok(v);
            }
        }
        Err(XnbError::BadVarInt)
    }

    fn string(&mut self, context: &'static str) -> Result<String, XnbError> {
        let n = self.varint(context)? as usize;
        let b = self.take(n, context)?;
        String::from_utf8(b.to_vec()).map_err(|_| XnbError::InvalidString { context })
    }

    /// .NET `BinaryReader.ReadChar` with UTF-8 (1 to 3 bytes for a UTF-16 unit; a 4-byte
    /// sequence is accepted as a full scalar).
    fn char(&mut self, context: &'static str) -> Result<char, XnbError> {
        let b0 = self.u8(context)?;
        let extra = match b0 {
            0x00..=0x7F => 0,
            0xC0..=0xDF => 1,
            0xE0..=0xEF => 2,
            0xF0..=0xF7 => 3,
            _ => return Err(XnbError::InvalidString { context }),
        };
        let mut buf = [b0, 0, 0, 0];
        buf[1..=extra].copy_from_slice(self.take(extra, context)?);
        std::str::from_utf8(&buf[..=extra])
            .ok()
            .and_then(|s| s.chars().next())
            .ok_or(XnbError::InvalidString { context })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn push_str(v: &mut Vec<u8>, s: &str) {
        let mut n = s.len();
        while n >= 0x80 {
            v.push((n & 0x7F) as u8 | 0x80);
            n >>= 7;
        }
        v.push(n as u8);
        v.extend_from_slice(s.as_bytes());
    }

    const TEX: &str = "Microsoft.Xna.Framework.Content.Texture2DReader, Microsoft.Xna.Framework.Graphics, Version=4.0.0.0, Culture=neutral, PublicKeyToken=842cf8be1de50553";
    const SND: &str = "Microsoft.Xna.Framework.Content.SoundEffectReader";
    const FX: &str = "Microsoft.Xna.Framework.Content.EffectReader";
    const LRECT: &str = "Microsoft.Xna.Framework.Content.ListReader`1[[Microsoft.Xna.Framework.Rectangle, Microsoft.Xna.Framework, Version=4.0.0.0]]";
    const LCHAR: &str = "Microsoft.Xna.Framework.Content.ListReader`1[[System.Char, mscorlib]]";
    const LVEC3: &str = "Microsoft.Xna.Framework.Content.ListReader`1[[Microsoft.Xna.Framework.Vector3, Microsoft.Xna.Framework]]";
    const DFONT: &str = "ReLogic.Graphics.DynamicSpriteFontReader, ReLogic";
    const SFONT: &str = "Microsoft.Xna.Framework.Content.SpriteFontReader";

    /// Build an uncompressed XNB file from readers and a body (after the shared count).
    fn xnb(readers: &[&str], shared: u8, body: &[u8]) -> Vec<u8> {
        let mut p = vec![readers.len() as u8];
        for r in readers {
            push_str(&mut p, r);
            p.extend_from_slice(&0i32.to_le_bytes());
        }
        p.push(shared);
        p.extend_from_slice(body);
        let mut f = b"XNBw".to_vec();
        f.extend_from_slice(&[5, 0]);
        f.extend_from_slice(&((p.len() + 10) as u32).to_le_bytes());
        f.extend_from_slice(&p);
        f
    }

    fn tex_body(fmt: i32, w: u32, h: u32, mip: &[u8]) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&fmt.to_le_bytes());
        b.extend_from_slice(&w.to_le_bytes());
        b.extend_from_slice(&h.to_le_bytes());
        b.extend_from_slice(&1i32.to_le_bytes());
        b.extend_from_slice(&(mip.len() as i32).to_le_bytes());
        b.extend_from_slice(mip);
        b
    }

    #[test]
    fn varint_and_string() {
        let mut c = Cursor::new(&[0x85, 0x01, 3, b'a', b'b', b'c']);
        assert_eq!(c.varint("t").unwrap(), 0x85 & 0x7F | (1 << 7));
        assert_eq!(c.string("t").unwrap(), "abc");
        assert_eq!(Cursor::new(&[0x80; 5]).varint("t"), Err(XnbError::BadVarInt));
    }

    #[test]
    fn char_reads_utf8() {
        let mut c = Cursor::new("a\u{e9}\u{20ac}".as_bytes());
        assert_eq!(c.char("t").unwrap(), 'a');
        assert_eq!(c.char("t").unwrap(), '\u{e9}');
        assert_eq!(c.char("t").unwrap(), '\u{20ac}');
        assert!(Cursor::new(&[0xE2, 0x82]).char("t").is_err());
    }

    #[test]
    fn texture_color() {
        let f = xnb(&[TEX], 0, &[&[1u8][..], &tex_body(0, 2, 1, &[1, 2, 3, 4, 5, 6, 7, 8])].concat());
        let x = read_xnb(&f).unwrap();
        assert_eq!((x.platform, x.version, x.compressed), (b'w', 5, false));
        let Asset::Texture2D(t) = &x.asset else { panic!("not a texture") };
        assert_eq!((t.format, t.width, t.height), (SurfaceFormat::Color, 2, 1));
        assert_eq!(t.top_mip_rgba8().unwrap(), vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn texture_dxt1_and_dxt5() {
        // DXT1: c0 = red (f800), c1 = blue (001f); index 0 for all pixels -> red.
        let mut dxt1 = vec![0x00, 0xF8, 0x1F, 0x00];
        dxt1.extend_from_slice(&[0; 4]);
        let t = Texture2D { format: SurfaceFormat::Dxt1, width: 4, height: 4, mips: vec![dxt1] };
        let rgba = t.top_mip_rgba8().unwrap();
        assert_eq!(&rgba[0..4], &[255, 0, 0, 255]);
        assert_eq!(rgba.len(), 64);

        // DXT5: alpha a0=255,a1=0, indices all 1 -> alpha 0; colour all c0 = blue.
        let mut dxt5 = vec![255, 0];
        dxt5.extend_from_slice(&[0x49, 0x92, 0x24, 0x49, 0x92, 0x24]);
        dxt5.extend_from_slice(&[0x1F, 0x00, 0x00, 0xF8, 0, 0, 0, 0]);
        let t = Texture2D { format: SurfaceFormat::Dxt5, width: 3, height: 2, mips: vec![dxt5] };
        let rgba = t.top_mip_rgba8().unwrap();
        assert_eq!(rgba.len(), 3 * 2 * 4);
        assert_eq!(&rgba[0..4], &[0, 0, 255, 0]);
    }

    #[test]
    fn texture_bad_data_and_formats() {
        let t = Texture2D { format: SurfaceFormat::Color, width: 2, height: 2, mips: vec![vec![0; 3]] };
        assert_eq!(t.top_mip_rgba8(), Err(XnbError::BadTextureData { expected: 16, actual: 3 }));
        let t = Texture2D { format: SurfaceFormat::Other(13), width: 1, height: 1, mips: vec![vec![0; 4]] };
        assert_eq!(t.top_mip_rgba8(), Err(XnbError::UnsupportedSurfaceFormat(13)));
        let t = Texture2D { format: SurfaceFormat::Color, width: 1, height: 1, mips: vec![] };
        assert_eq!(t.top_mip_rgba8(), Err(XnbError::NoMips));
        let t = Texture2D { format: SurfaceFormat::Bgr565, width: 1, height: 1, mips: vec![vec![0x00, 0xF8]] };
        assert_eq!(t.top_mip_rgba8().unwrap(), vec![255, 0, 0, 255]);
    }

    #[test]
    fn sound_effect() {
        let mut fmt = Vec::new();
        fmt.extend_from_slice(&1u16.to_le_bytes());
        fmt.extend_from_slice(&2u16.to_le_bytes());
        fmt.extend_from_slice(&44100u32.to_le_bytes());
        fmt.extend_from_slice(&176400u32.to_le_bytes());
        fmt.extend_from_slice(&4u16.to_le_bytes());
        fmt.extend_from_slice(&16u16.to_le_bytes());
        fmt.extend_from_slice(&0u16.to_le_bytes());
        let mut body = vec![1u8];
        body.extend_from_slice(&(fmt.len() as i32).to_le_bytes());
        body.extend_from_slice(&fmt);
        body.extend_from_slice(&4i32.to_le_bytes());
        body.extend_from_slice(&[9, 8, 7, 6]);
        for v in [0i32, 1, 250] {
            body.extend_from_slice(&v.to_le_bytes());
        }
        let x = read_xnb(&xnb(&[SND], 0, &body)).unwrap();
        let Asset::SoundEffect(s) = x.asset else { panic!("not a sound") };
        assert_eq!((s.format_tag, s.channels, s.sample_rate, s.block_align, s.bits_per_sample), (1, 2, 44100, 4, 16));
        assert_eq!(s.format_extra, vec![0, 0]);
        assert_eq!(s.data, vec![9, 8, 7, 6]);
        assert_eq!((s.loop_start, s.loop_length, s.duration_ms), (0, 1, 250));
    }

    #[test]
    fn effect() {
        let mut body = vec![1u8];
        body.extend_from_slice(&3i32.to_le_bytes());
        body.extend_from_slice(&[0xCF, 0x0B, 0xF0]);
        let x = read_xnb(&xnb(&[FX], 0, &body)).unwrap();
        assert_eq!(x.asset, Asset::Effect(Effect { bytecode: vec![0xCF, 0x0B, 0xF0] }));
    }

    fn font_lists(body: &mut Vec<u8>, ids: [u8; 4]) {
        // glyphs / padding: one rect each
        for _ in 0..2 {
            body.push(ids[0]);
            body.extend_from_slice(&1i32.to_le_bytes());
            for v in [1i32, 2, 3, 4] {
                body.extend_from_slice(&v.to_le_bytes());
            }
        }
        body.push(ids[1]);
        body.extend_from_slice(&1i32.to_le_bytes());
        body.extend_from_slice("\u{e9}".as_bytes());
        body.push(ids[2]);
        body.extend_from_slice(&1i32.to_le_bytes());
        for v in [0.5f32, 1.5, 2.5] {
            body.extend_from_slice(&v.to_le_bytes());
        }
        let _ = ids[3];
    }

    #[test]
    fn dynamic_sprite_font() {
        let mut body = vec![1u8]; // primary: reader 1 = DFONT
        body.extend_from_slice(&1.5f32.to_le_bytes());
        body.extend_from_slice(&30i32.to_le_bytes());
        body.push(b'?');
        body.extend_from_slice(&1i32.to_le_bytes());
        body.push(2); // texture reader
        body.extend_from_slice(&tex_body(0, 1, 1, &[1, 2, 3, 4]));
        font_lists(&mut body, [3, 4, 5, 0]);
        let x = read_xnb(&xnb(&[DFONT, TEX, LRECT, LCHAR, LVEC3], 0, &body)).unwrap();
        let Asset::DynamicSpriteFont(f) = x.asset else { panic!("not a font") };
        assert_eq!((f.spacing, f.line_spacing, f.default_character), (1.5, 30, '?'));
        let p = &f.pages[0];
        assert_eq!(p.glyphs, vec![Rect { x: 1, y: 2, w: 3, h: 4 }]);
        assert_eq!(p.padding.len(), 1);
        assert_eq!(p.characters, vec!['\u{e9}']);
        assert_eq!(p.kerning, vec![Vec3 { x: 0.5, y: 1.5, z: 2.5 }]);
        assert_eq!(p.texture.width, 1);
    }

    #[test]
    fn sprite_font() {
        let mut body = vec![1u8];
        body.push(2);
        body.extend_from_slice(&tex_body(0, 1, 1, &[1, 2, 3, 4]));
        // glyphs, cropping, chars, then line spacing, spacing, kerning, default char
        let mut lists = Vec::new();
        font_lists(&mut lists, [3, 4, 5, 0]);
        // font_lists emits rect,rect,char,vec3; reorder to rect,rect,char then scalars, vec3
        let rect_len = 1 + 4 + 16;
        let char_len = 1 + 4 + 2;
        body.extend_from_slice(&lists[..2 * rect_len + char_len]);
        body.extend_from_slice(&20i32.to_le_bytes());
        body.extend_from_slice(&2.0f32.to_le_bytes());
        body.extend_from_slice(&lists[2 * rect_len + char_len..]);
        body.push(1);
        body.push(b'*');
        let x = read_xnb(&xnb(&[SFONT, TEX, LRECT, LCHAR, LVEC3], 0, &body)).unwrap();
        let Asset::SpriteFont(f) = x.asset else { panic!("not a font") };
        assert_eq!((f.line_spacing, f.spacing, f.default_character), (20, 2.0, Some('*')));
        assert_eq!(f.cropping.len(), 1);
    }

    #[test]
    fn unknown_reader_does_not_fail() {
        let x = read_xnb(&xnb(&["Some.Other.Reader"], 0, &[1, 0xAA, 0xBB])).unwrap();
        assert_eq!(x.asset, Asset::Unknown { reader_name: "Some.Other.Reader".into() });
    }

    #[test]
    fn errors_not_panics() {
        assert_eq!(read_xnb(b""), Err(XnbError::Truncated { context: "magic", needed: 3, available: 0 }));
        assert_eq!(read_xnb(b"ABCDEFGHIJ"), Err(XnbError::BadMagic));
        let mut f = xnb(&[TEX], 0, &[&[1u8][..], &tex_body(0, 1, 1, &[1, 2, 3, 4])].concat());
        let full = f.clone();
        for n in 0..full.len() {
            // every strict prefix must be an error (file size field mismatch or truncation)
            assert!(read_xnb(&full[..n]).is_err(), "prefix {n}");
        }
        f[4] = 4;
        assert_eq!(read_xnb(&f), Err(XnbError::UnsupportedVersion(4)));
        f[4] = 5;
        f[5] = 0x40;
        assert_eq!(read_xnb(&f), Err(XnbError::UnsupportedFlags(0x40)));
        f[5] = 0;
        f.push(0);
        let len = f.len() as u32;
        f[6..10].copy_from_slice(&len.to_le_bytes());
        assert_eq!(read_xnb(&f), Err(XnbError::TrailingBytes { count: 1 }));
        // bad type id
        let g = xnb(&[TEX], 0, &[9]);
        assert_eq!(read_xnb(&g), Err(XnbError::InvalidTypeId { id: 9, reader_count: 1 }));
        // huge counts do not allocate or panic
        let g = xnb(&[TEX], 0, &[1, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0x7F]);
        assert!(matches!(read_xnb(&g), Err(XnbError::Truncated { .. })));
    }

    /// Hand-built LZX stream (XNA framed): one uncompressed block holding `data`.
    fn lzx_stored(data: &[u8]) -> Vec<u8> {
        // bit stream (16-bit LE words, MSB first): E8 flag 0, block type 3, 24-bit size,
        // pad to 16 bits, then R0..R2 (1,1,1) and data padded to an even length.
        let n = data.len() as u32;
        let w1 = 0x3000u16 | (n >> 12) as u16;
        let w2 = ((n & 0xFFF) << 4) as u16;
        let mut chunk = Vec::new();
        chunk.extend_from_slice(&w1.to_le_bytes());
        chunk.extend_from_slice(&w2.to_le_bytes());
        for _ in 0..3 {
            chunk.extend_from_slice(&1u32.to_le_bytes());
        }
        chunk.extend_from_slice(data);
        if data.len() % 2 == 1 {
            chunk.push(0);
        }
        let mut framed = vec![0xFF];
        framed.extend_from_slice(&(data.len() as u16).to_be_bytes());
        framed.extend_from_slice(&(chunk.len() as u16).to_be_bytes());
        framed.extend_from_slice(&chunk);
        framed
    }

    #[test]
    fn lzx_framing_with_marker() {
        let data = lzx_stored(b"hello");
        assert_eq!(lzx_decompress(&data, 5).unwrap(), b"hello");
    }

    #[test]
    fn lzx_framing_errors() {
        let data = lzx_stored(b"hello");
        assert_eq!(
            lzx_decompress(&data, 6),
            Err(XnbError::SizeMismatch { what: "decompressed size", expected: 6, actual: 5 })
        );
        assert!(matches!(lzx_decompress(&data[..data.len() - 3], 5), Err(XnbError::Truncated { .. })));
        assert!(matches!(lzx_decompress(&[0xFF, 0x00], 0), Err(XnbError::Truncated { .. })));
        assert!(matches!(lzx_decompress(&[0xFF, 0xFF, 0xFF, 0x00, 0x01, 0x00], 0), Err(XnbError::BadLzxFraming(_))));
        assert_eq!(lzx_decompress(&[0x00, 0x00, 0x12, 0x34], 0).unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn compressed_container() {
        let plain = xnb(&[FX], 0, &[&[1u8][..], &3i32.to_le_bytes(), &[7, 8, 9]].concat());
        let payload = &plain[10..];
        let framed = lzx_stored(payload);
        let mut f = b"XNBw".to_vec();
        f.extend_from_slice(&[5, FLAG_COMPRESSED]);
        f.extend_from_slice(&((14 + framed.len()) as u32).to_le_bytes());
        f.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        f.extend_from_slice(&framed);
        let x = read_xnb(&f).unwrap();
        assert!(x.compressed);
        assert_eq!(x.payload_size, payload.len());
        assert_eq!(x.asset, Asset::Effect(Effect { bytecode: vec![7, 8, 9] }));
        assert_eq!(read_xnb(&plain).unwrap().asset, x.asset);
    }
}

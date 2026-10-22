//! Decoders behind `NVorbis.VorbisReader` and `MP3Sharp.MP3Stream` (the game's resource-pack / console music path:
//! `Terraria.Audio.OGGAudioTrack`, `MP3AudioTrack`).
//!
//! Both are pure-Rust crates: `lewton` 0.10 (Vorbis; with its `ogg` and `tinyvec`) and `puremp3` 0.1 (MPEG-1/2/2.5
//! Layer III; with `bitstream-io`). The streams are read into memory when the decoder is built (a music file is a few
//! MB; the PCM is NOT kept: it is produced packet by packet / frame by frame as `ReadSamples` / `Read` ask).
//!
//! Not bit-exact with NVorbis / MP3Sharp (different floating-point IMDCT / polyphase code): sample counts, rate,
//! channels, tags and seek positions are the same, sample values agree to about 1e-4 of full scale (tests in
//! `audio.rs` compare against the dlls). Seeking is exact but linear: `DecodedPosition = n` rewinds to the start
//! and decodes forward, which for the game's use (loop points a few seconds into a track, once per loop) costs tens
//! of milliseconds.

use std::io::Cursor;
use std::sync::Arc;

use lewton::inside_ogg::OggStreamReader;
use lewton::samples::InterleavedSamples;
use terraria_gen_rt::rt::{self, io::Stream, Arr, Dictionary, Gc, List, Marker, Str, Trace};

use crate::gfx::states::throw;

/// `Arc<Vec<u8>>` as a cursor's backing store.
#[derive(Clone)]
struct Bytes(Arc<Vec<u8>>);
impl AsRef<[u8]> for Bytes {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

fn stream_bytes(stream: Gc<Stream>) -> Arc<Vec<u8>> {
    if stream.is_null() {
        throw("System.ArgumentNullException", "stream");
    }
    // SAFETY: checked non-null; a live runtime object.
    let s = unsafe { &mut *stream.ptr() };
    let mut bytes = vec![0u8; (s.length() - s.position()).max(0) as usize];
    let n = s.read_slice(&mut bytes);
    bytes.truncate(n);
    Arc::new(bytes)
}

// ------------------------------------------------------------------------------------------------ Vorbis

/// Total number of samples per channel: the granule position of the last Ogg page (what NVorbis reports as
/// `TotalSamples`).
pub fn ogg_total_samples(b: &[u8]) -> Option<i64> {
    let mut last = None;
    let mut o = 0;
    while o + 27 <= b.len() && &b[o..o + 4] == b"OggS" {
        let granule = i64::from_le_bytes(b[o + 6..o + 14].try_into().ok()?);
        let segs = b[o + 26] as usize;
        let table = b.get(o + 27..o + 27 + segs)?;
        let body: usize = table.iter().map(|x| *x as usize).sum();
        if granule >= 0 {
            last = Some(granule);
        }
        o += 27 + segs + body;
    }
    last
}

/// `NVorbis.VorbisReader`
pub struct VorbisReader {
    bytes: Arc<Vec<u8>>,
    rd: OggStreamReader<Cursor<Bytes>>,
    channels: i32,
    rate: i32,
    total: i64,
    /// frames returned to the caller so far (`DecodedPosition`)
    pos: i64,
    /// interleaved samples of the current packet not handed out yet
    pending: Vec<f32>,
    pending_off: usize,
    /// (UPPER-CASE key, values) in file order
    tags: Vec<(String, Vec<String>)>,
    eof: bool,
    disposed: bool,
}
// SAFETY: no handles inside.
unsafe impl Trace for VorbisReader {
    const NEEDS_TRACE: bool = false;
    unsafe fn trace(&self, _m: &mut Marker) {}
}

fn open_ogg(bytes: &Arc<Vec<u8>>) -> Result<OggStreamReader<Cursor<Bytes>>, String> {
    OggStreamReader::new(Cursor::new(Bytes(bytes.clone()))).map_err(|e| format!("{e:?}"))
}

impl VorbisReader {
    /// `new VorbisReader(stream)`: `ArgumentException` for data that is not Ogg Vorbis ("Could not determine
    /// container type" in NVorbis).
    pub fn new_gc(stream: Gc<Stream>) -> Gc<VorbisReader> {
        let bytes = stream_bytes(stream);
        let rd = match open_ogg(&bytes) {
            Ok(r) => r,
            Err(e) => rt::throw(rt::exc("System.ArgumentException", Str::new(&format!("Could not determine container type: {e}")))),
        };
        let mut tags: Vec<(String, Vec<String>)> = Vec::new();
        for (k, v) in &rd.comment_hdr.comment_list {
            let key = k.to_uppercase();
            match tags.iter_mut().find(|t| t.0 == key) {
                Some(t) => t.1.push(v.clone()),
                None => tags.push((key, vec![v.clone()])),
            }
        }
        let channels = rd.ident_hdr.audio_channels as i32;
        let rate = rd.ident_hdr.audio_sample_rate as i32;
        let total = ogg_total_samples(&bytes).unwrap_or(0);
        rt::alloc_init(VorbisReader { bytes, rd, channels, rate, total, pos: 0, pending: Vec::new(), pending_off: 0, tags, eof: false, disposed: false })
    }
    fn check(&self) {
        if self.disposed {
            throw("System.ObjectDisposedException", "Cannot access a disposed object. Object name: 'VorbisReader'.");
        }
    }
    pub fn channels(&self) -> i32 {
        self.channels
    }
    pub fn sample_rate(&self) -> i32 {
        self.rate
    }
    /// `TotalSamples` (per channel)
    pub fn total_samples(&self) -> i64 {
        self.total
    }
    /// `DecodedPosition` (per channel)
    pub fn decoded_position(&self) -> i64 {
        self.pos
    }
    pub fn is_end_of_stream(&self) -> bool {
        self.eof && self.pending_off >= self.pending.len()
    }
    /// `Tags.All`: keys are upper case, values in file order.
    pub fn tags_all(&self) -> Gc<Dictionary<Str, Gc<List<Str>>>> {
        let d = Dictionary::<Str, Gc<List<Str>>>::new_gc();
        for (k, vs) in &self.tags {
            let l = List::<Str>::new_gc();
            for v in vs {
                // SAFETY: fresh list
                unsafe { (*l.ptr()).add(Str::new(v)) };
            }
            // SAFETY: fresh dictionary
            unsafe { (*d.ptr()).set_item(Str::new(k), l) };
        }
        d
    }

    /// Decodes the next packet into `pending`; false at the end of the stream.
    fn fill(&mut self) -> bool {
        loop {
            match self.rd.read_dec_packet_generic::<InterleavedSamples<f32>>() {
                Ok(Some(p)) => {
                    if p.samples.is_empty() {
                        continue;
                    }
                    self.pending = p.samples;
                    self.pending_off = 0;
                    return true;
                }
                Ok(None) | Err(_) => {
                    self.eof = true;
                    return false;
                }
            }
        }
    }

    /// `ReadSamples(float[] buffer, int offset, int count)`: interleaved samples; returns how many floats were
    /// written (fewer than `count` only at the end of the stream).
    pub fn read_samples(&mut self, buffer: Arr<f32>, offset: i32, count: i32) -> i32 {
        self.check();
        if buffer.is_null() {
            throw("System.ArgumentNullException", "buffer");
        }
        if offset < 0 || count < 0 || offset as i64 + count as i64 > buffer.len() as i64 {
            throw("System.ArgumentOutOfRangeException", "offset");
        }
        let ch = self.channels.max(1) as usize;
        let mut written = 0usize;
        let want = (count as usize / ch) * ch;
        // SAFETY: bounds checked above; the array is a live object
        let out = unsafe { std::slice::from_raw_parts_mut(buffer.as_slice().as_ptr().add(offset as usize) as *mut f32, count as usize) };
        while written < want {
            if self.pending_off >= self.pending.len() && !self.fill() {
                break;
            }
            let n = (self.pending.len() - self.pending_off).min(want - written);
            out[written..written + n].copy_from_slice(&self.pending[self.pending_off..self.pending_off + n]);
            self.pending_off += n;
            written += n;
        }
        self.pos += (written / ch) as i64;
        written as i32
    }

    /// `DecodedPosition = frames` / `SeekTo(frames)` / `SamplePosition = frames`: exact.
    pub fn set_decoded_position(&mut self, frames: i64) {
        self.check();
        let frames = frames.clamp(0, self.total.max(0));
        if frames < self.pos {
            match open_ogg(&self.bytes) {
                Ok(r) => self.rd = r,
                Err(e) => throw_owned("System.InvalidOperationException", e),
            }
            self.pending.clear();
            self.pending_off = 0;
            self.pos = 0;
            self.eof = false;
        }
        // decode forward, discarding
        let ch = self.channels.max(1) as usize;
        while self.pos < frames {
            if self.pending_off >= self.pending.len() && !self.fill() {
                break;
            }
            let have = (self.pending.len() - self.pending_off) / ch;
            let skip = have.min((frames - self.pos) as usize);
            self.pending_off += skip * ch;
            self.pos += skip as i64;
        }
    }
    /// `SeekTo(position, origin)`: `SeekOrigin` Begin 0, Current 1, End 2 (frames).
    pub fn seek_to(&mut self, position: i64, origin: i32) {
        let target = match origin {
            0 => position,
            1 => self.pos + position,
            _ => self.total + position,
        };
        self.set_decoded_position(target);
    }
    /// `TotalTime`
    pub fn total_time(&self) -> terraria_gen_rt::rt::time::TimeSpan {
        terraria_gen_rt::rt::time::TimeSpan::from_seconds(self.total as f64 / self.rate.max(1) as f64)
    }
    pub fn dispose(&mut self) {
        self.disposed = true;
        self.pending = Vec::new();
    }
}

fn throw_owned(kind: &'static str, msg: String) -> ! {
    rt::throw(rt::exc(kind, Str::new(&msg)))
}

// ------------------------------------------------------------------------------------------------ MP3

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{Decoder, DecoderOptions};
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// `MP3Sharp.MP3Stream`: 16-bit little-endian stereo PCM (a mono file is duplicated to both channels, as MP3Sharp
/// does), every frame decoded, no gapless trimming (MP3Sharp returns the encoder delay and padding too).
/// Decoder: `symphonia` 0.5 (`symphonia-bundle-mp3`); the first choice, `puremp3`, decoded these files with
/// different waveforms (short blocks and MPEG-2 mono) and was dropped.
pub struct Mp3Stream {
    bytes: Arc<Vec<u8>>,
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,
    frequency: i32,
    pending: Vec<u8>,
    pending_off: usize,
    consumed: i64,
    eof: bool,
    disposed: bool,
}
// SAFETY: no handles inside.
unsafe impl Trace for Mp3Stream {
    const NEEDS_TRACE: bool = false;
    unsafe fn trace(&self, _m: &mut Marker) {}
}

type Mp3Parts = (Box<dyn FormatReader>, Box<dyn Decoder>, u32, i32);

/// MP3Sharp decodes the Xing / Info frame that LAME and others put first (it holds no audio, so it comes out as
/// silence); symphonia skips it. Returns the number of stereo frames of silence to put in front: the samples per
/// frame (1152 MPEG-1, 576 MPEG-2 / 2.5) when the first frame carries a `Xing` or `Info` tag, else 0.
fn xing_frame_samples(b: &[u8]) -> usize {
    let mut o = 0;
    if b.len() > 10 && &b[..3] == b"ID3" {
        o = 10 + (((b[6] as usize) & 0x7f) << 21 | ((b[7] as usize) & 0x7f) << 14 | ((b[8] as usize) & 0x7f) << 7 | ((b[9] as usize) & 0x7f));
    }
    while o + 4 <= b.len() {
        if b[o] == 0xFF && b[o + 1] & 0xE0 == 0xE0 && (b[o + 1] >> 3) & 3 != 1 && (b[o + 1] >> 1) & 3 == 1 {
            let mpeg1 = (b[o + 1] >> 3) & 3 == 3;
            let crc = b[o + 1] & 1 == 0;
            let mono = (b[o + 3] >> 6) & 3 == 3;
            let side = match (mpeg1, mono) {
                (true, true) => 17,
                (true, false) => 32,
                (false, true) => 9,
                (false, false) => 17,
            };
            let tag = o + 4 + if crc { 2 } else { 0 } + side;
            let is_info = b.get(tag..tag + 4).is_some_and(|t| t == b"Xing" || t == b"Info");
            return if is_info { if mpeg1 { 1152 } else { 576 } } else { 0 };
        }
        o += 1;
    }
    0
}

fn open_mp3(bytes: &Arc<Vec<u8>>) -> Result<Mp3Parts, String> {
    let mss = MediaSourceStream::new(Box::new(Cursor::new(Bytes(bytes.clone()))), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("mp3");
    let probed = symphonia::default::get_probe().format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default()).map_err(|e| e.to_string())?;
    let format = probed.format;
    let track = format.default_track().ok_or("no audio track")?;
    let rate = track.codec_params.sample_rate.ok_or("unknown sample rate")? as i32;
    let id = track.id;
    let decoder = symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default()).map_err(|e| e.to_string())?;
    Ok((format, decoder, id, rate))
}

impl Mp3Stream {
    /// `new MP3Stream(stream)`
    pub fn new_gc(stream: Gc<Stream>) -> Gc<Mp3Stream> {
        let bytes = stream_bytes(stream);
        let (format, decoder, track_id, frequency) = match open_mp3(&bytes) {
            Ok(x) => x,
            Err(e) => rt::throw(rt::exc("System.ArgumentException", Str::new(&format!("Not an MPEG audio stream: {e}")))),
        };
        let lead = xing_frame_samples(&bytes) * 4;
        rt::alloc_init(Mp3Stream { bytes, format, decoder, track_id, frequency, pending: vec![0; lead], pending_off: 0, consumed: 0, eof: false, disposed: false })
    }
    pub fn frequency(&self) -> i32 {
        self.frequency
    }

    /// Decodes the next packet into `pending` (stereo i16 bytes); false at the end.
    fn fill(&mut self) -> bool {
        loop {
            let packet = match self.format.next_packet() {
                Ok(p) => p,
                Err(_) => {
                    self.eof = true;
                    return false;
                }
            };
            if packet.track_id() != self.track_id {
                continue;
            }
            self.consumed += packet.data.len() as i64 + 4;
            let decoded = match self.decoder.decode(&packet) {
                Ok(d) => d,
                // a damaged frame is skipped, as the original does
                Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
                Err(_) => {
                    self.eof = true;
                    return false;
                }
            };
            let spec = *decoded.spec();
            let ch = spec.channels.count();
            let mut buf = SampleBuffer::<i16>::new(decoded.capacity() as u64, spec);
            buf.copy_interleaved_ref(decoded);
            let src = buf.samples();
            let mut out = Vec::with_capacity(src.len() / ch.max(1) * 4);
            for frame in src.chunks_exact(ch.max(1)) {
                let (l, r) = (frame[0], if ch > 1 { frame[1] } else { frame[0] });
                out.extend_from_slice(&l.to_le_bytes());
                out.extend_from_slice(&r.to_le_bytes());
            }
            if out.is_empty() {
                continue;
            }
            self.pending = out;
            self.pending_off = 0;
            return true;
        }
    }

    /// `Read(byte[] buffer, int offset, int count)`: bytes written, 0 at the end.
    pub fn read(&mut self, buffer: Arr<u8>, offset: i32, count: i32) -> i32 {
        if self.disposed {
            throw("System.ObjectDisposedException", "Cannot access a disposed object. Object name: 'MP3Stream'.");
        }
        if buffer.is_null() {
            throw("System.ArgumentNullException", "buffer");
        }
        if offset < 0 || count < 0 || offset as i64 + count as i64 > buffer.len() as i64 {
            throw("System.ArgumentOutOfRangeException", "offset");
        }
        // SAFETY: bounds checked above; the array is a live object
        let out = unsafe { std::slice::from_raw_parts_mut(buffer.as_slice().as_ptr().add(offset as usize) as *mut u8, count as usize) };
        let mut written = 0;
        while written < out.len() {
            if self.pending_off >= self.pending.len() && (self.eof || !self.fill()) {
                break;
            }
            let n = (self.pending.len() - self.pending_off).min(out.len() - written);
            out[written..written + n].copy_from_slice(&self.pending[self.pending_off..self.pending_off + n]);
            self.pending_off += n;
            written += n;
        }
        written as i32
    }
    /// `Position.get`: bytes of the file consumed (the size of the file at the end, as MP3Sharp reports).
    pub fn position(&self) -> i64 {
        if self.is_eof() {
            self.bytes.len() as i64
        } else {
            self.consumed.min(self.bytes.len() as i64)
        }
    }
    /// `Position.set`: only 0 (rewind) is supported.
    pub fn set_position(&mut self, p: i64) {
        if p != 0 {
            terraria_gen_rt::host::unimplemented("XPT.Core.Audio.MP3Sharp.MP3Stream.Position.set(non-zero): only rewinding to 0 is provided");
            return;
        }
        if let Ok((format, decoder, track_id, _)) = open_mp3(&self.bytes) {
            self.format = format;
            self.decoder = decoder;
            self.track_id = track_id;
        }
        self.pending = vec![0; xing_frame_samples(&self.bytes) * 4];
        self.pending_off = 0;
        self.consumed = 0;
        self.eof = false;
    }
    /// `Length`: the size of the file.
    pub fn length(&self) -> i64 {
        self.bytes.len() as i64
    }
    pub fn is_eof(&self) -> bool {
        self.eof && self.pending_off >= self.pending.len()
    }
    pub fn dispose(&mut self) {
        self.disposed = true;
        self.pending = Vec::new();
    }
}

//! `Microsoft.Xna.Framework.Audio`: `SoundEffect`, `SoundEffectInstance`, `DynamicSoundEffectInstance`, and the
//! XACT classes (`AudioEngine`, `WaveBank`, `SoundBank`, `Cue`, `AudioCategory`).
//!
//! Two layers, as in XNA 4. The types here are XNA's managed classes: their argument checks, exceptions and state
//! bookkeeping follow XNA's own managed code (decompiled from the real Microsoft.Xna.Framework.dll / .Xact.dll:
//! `SoundEffectInstance.Volume` checks 0..1 then calls the native SetVolume, `Play` submits the buffer once
//! (`isPacketSubmitted`), `State` maps the kernel's voice state, `Cue.IsPlaying` tests a status bit, ...). Below them
//! [`engine`] is the native side: XNA's `SoundEffectUnsafeNativeMethods` and the XACT `UnsafeNativeMethods`, a
//! deterministic model of the kernel / XACT engine (the same model the reference run of the original uses,
//! tools/refdump_draw/AudioRec.cs), which writes the audio trace ([`trace`]) and hands [`mixer`] its commands.
//!
//! Output is mixer-neutral: PCM buffers (`Arc<Sound>`: interleaved 16-bit samples) and an ordered queue of
//! [`AudioCommand`]s that a backend takes once per frame ([`take_commands`]). Nothing here opens an audio device.
//! Voice state is decided by the engine, not by the backend: the driver advances it by the frame's elapsed time
//! ([`advance`]); the backend mixes the same samples at the same rate, so the two agree to within a block.
//!
//! XACT (`TerrariaMusic.xgs`, `Wave Bank.xwb`, `Sound Bank.xsb`, the game's legacy music path): the wave bank is
//! parsed and its entries (all 104 are MS ADPCM, stereo) are decoded to PCM ([`WaveBank::decode`]); a cue's sound
//! (simple sounds only) plays its wave as an internal engine voice whose gain follows the .xgs (category volumes,
//! the `Volume` RPC curve). See README.md, "Audio".

pub mod codec;
pub mod engine;
pub mod mixer;
pub mod trace;
pub mod xact;

use std::cell::RefCell;
use std::sync::Arc;

use terraria_formats::xnb;
use terraria_gen_rt::host;
use terraria_gen_rt::rt::{self, io::Stream, time::TimeSpan, Arr, Gc, Marker, Str, Trace};

use crate::gfx::states::{plain_trace, throw};

/// `SoundState`
pub const STATE_PLAYING: i32 = 0;
pub const STATE_PAUSED: i32 = 1;
pub const STATE_STOPPED: i32 = 2;

/// Decoded audio: interleaved signed 16-bit samples.
#[derive(Debug, PartialEq)]
pub struct Sound {
    pub name: String,
    pub channels: u16,
    pub sample_rate: u32,
    pub samples: Vec<i16>,
    /// loop region in frames (start, length); the whole sound when length is 0
    pub loop_start: u32,
    pub loop_length: u32,
}
impl Sound {
    pub fn frames(&self) -> usize {
        self.samples.len() / self.channels.max(1) as usize
    }
    pub fn seconds(&self) -> f64 {
        self.frames() as f64 / self.sample_rate.max(1) as f64
    }
}

/// What the backend has to do, in order.
#[derive(Clone, Debug)]
pub enum AudioCommand {
    /// Start voice `voice` from the beginning of `sound`.
    Play { voice: u64, sound: Arc<Sound>, volume: f32, pan: f32, pitch: f32, looped: bool },
    Pause { voice: u64 },
    Resume { voice: u64 },
    /// Stop and forget the voice.
    Stop { voice: u64 },
    SetVolume { voice: u64, volume: f32 },
    SetPan { voice: u64, pan: f32 },
    SetPitch { voice: u64, pitch: f32 },
    /// `SoundEffect.MasterVolume`
    MasterVolume(f32),
    /// A streaming voice (`DynamicSoundEffectInstance`): created paused and empty.
    StreamOpen { voice: u64, channels: u16, sample_rate: u32 },
    /// Samples appended to a streaming voice (`SubmitBuffer`).
    StreamData { voice: u64, samples: Arc<[i16]> },
}

/// Backend: the commands recorded since the last call, oldest first.
pub fn take_commands() -> Vec<AudioCommand> {
    engine::take_commands()
}

/// Driver: `seconds` of real time passed (rounded to 100 ns ticks, as the frame's `GameTime`). Playing voices
/// advance; finished ones stop.
pub fn advance(seconds: f64) {
    engine::advance_ticks((seconds * 1e7).round() as i64);
}

/// Number of voices playing right now (tests, diagnostics).
pub fn active_voices() -> usize {
    engine::playing_voices()
}

/// XNA's `Helpers.ThrowExceptionFromErrorCode` for what the engine answers (only E_INVALIDARG here).
fn check_hr(hr: i32) {
    if hr < 0 {
        throw("System.ArgumentException", "Value does not fall within the expected range.");
    }
}

// ------------------------------------------------------------------------------------------------ SoundEffect

pub struct SoundEffect {
    pub sound: Arc<Sound>,
    /// the engine's handle (`SoundEffect.Handle`)
    pub handle: u32,
    /// `Loop` (start, length) in frames, as SubmitPacket gets it
    pub loop_start: i32,
    pub loop_length: i32,
    pub disposed: bool,
}
plain_trace!(SoundEffect);

thread_local! {
    /// `SoundEffect.currentVolume` (the static `MasterVolume`; 1 until set)
    static MASTER: RefCell<f32> = const { RefCell::new(1.0) };
}

/// 16-bit samples from PCM bytes of 8 or 16 bits per sample.
fn pcm_to_i16(data: &[u8], bits: u16) -> Option<Vec<i16>> {
    match bits {
        16 => Some(data.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect()),
        8 => Some(data.iter().map(|b| ((*b as i16) - 128) << 8).collect()),
        _ => None,
    }
}

/// MS ADPCM (WAVE format 2) -> 16-bit PCM. `block_align` bytes per block; the standard 7 coefficient pairs.
pub fn decode_ms_adpcm(data: &[u8], channels: usize, block_align: usize) -> Vec<i16> {
    const ADAPT: [i32; 16] = [230, 230, 230, 230, 307, 409, 512, 614, 768, 614, 512, 409, 307, 230, 230, 230];
    const C1: [i32; 7] = [256, 512, 0, 192, 240, 460, 392];
    const C2: [i32; 7] = [0, -256, 0, 64, 0, -208, -232];
    if channels == 0 || block_align < 7 * channels {
        return Vec::new();
    }
    // two samples per channel from the header, then two per byte
    let mut out = Vec::with_capacity(data.len().div_ceil(block_align) * (2 * channels + 2 * (block_align - 7 * channels)));
    for (i, block) in data.chunks(block_align).enumerate() {
        if block.len() < 7 * channels {
            break;
        }
        // the browser build: a music track (decoded whole when it first plays, ~0.1 s) gives the display frame back
        #[cfg(target_family = "wasm")]
        if i % 1024 == 1023 {
            crate::sys::threading::main_yield::check();
        }
        #[cfg(not(target_family = "wasm"))]
        let _ = i;
        let mut p = 0;
        let pred: Vec<usize> = (0..channels).map(|c| (block[c] as usize).min(6)).collect();
        p += channels;
        let rd = |p: usize| i16::from_le_bytes([block[p], block[p + 1]]) as i32;
        let mut delta: Vec<i32> = (0..channels).map(|c| rd(p + 2 * c)).collect();
        p += 2 * channels;
        let mut s1: Vec<i32> = (0..channels).map(|c| rd(p + 2 * c)).collect();
        p += 2 * channels;
        let mut s2: Vec<i32> = (0..channels).map(|c| rd(p + 2 * c)).collect();
        p += 2 * channels;
        for c in 0..channels {
            out.push(s2[c] as i16);
        }
        for c in 0..channels {
            out.push(s1[c] as i16);
        }
        let mut ch = 0;
        for byte in &block[p..] {
            for nibble in [byte >> 4, byte & 15] {
                let signed = if nibble & 8 != 0 { nibble as i32 - 16 } else { nibble as i32 };
                let predicted = (s1[ch] * C1[pred[ch]] + s2[ch] * C2[pred[ch]]) >> 8;
                let sample = (predicted + signed * delta[ch]).clamp(-32768, 32767);
                out.push(sample as i16);
                s2[ch] = s1[ch];
                s1[ch] = sample;
                delta[ch] = ((ADAPT[nibble as usize] * delta[ch]) >> 8).max(16);
                ch = (ch + 1) % channels;
            }
        }
    }
    out
}

/// The WAVEFORMATEX bytes of an XNB sound effect (the reader hands XNA's constructor the format as stored).
fn xnb_format_bytes(s: &xnb::SoundEffect) -> Vec<u8> {
    let mut f = Vec::with_capacity(16 + s.format_extra.len());
    f.extend_from_slice(&s.format_tag.to_le_bytes());
    f.extend_from_slice(&s.channels.to_le_bytes());
    f.extend_from_slice(&s.sample_rate.to_le_bytes());
    f.extend_from_slice(&s.avg_bytes_per_sec.to_le_bytes());
    f.extend_from_slice(&s.block_align.to_le_bytes());
    f.extend_from_slice(&s.bits_per_sample.to_le_bytes());
    f.extend_from_slice(&s.format_extra);
    f
}

impl SoundEffect {
    /// XNA's `SoundEffect.Create`: the engine's handle for the format and data, then `MasterVolume = currentVolume`.
    fn create(label: &str, format: &[u8], data: &[u8], sound: Sound, loop_start: i32, loop_length: i32) -> Gc<SoundEffect> {
        let sound = Arc::new(sound);
        let handle = engine::create_sound_effect(label, format, data, sound.clone());
        set_master_volume(master_volume());
        rt::alloc_init(SoundEffect { sound, handle, loop_start, loop_length, disposed: false })
    }

    /// The samples as the game may ask for them. (They were kept a second time in a field of their own: every
    /// sound effect of the game twice, ~300 MB the browser build's 4 GB address space cannot spare.)
    pub fn pcm(&self) -> &[i16] {
        &self.sound.samples
    }

    /// From a decoded XNB `SoundEffect` (PCM or MS ADPCM), `name` its asset name.
    pub fn from_xnb(name: &str, s: &xnb::SoundEffect) -> Gc<SoundEffect> {
        let samples = match s.format_tag {
            1 => pcm_to_i16(&s.data, s.bits_per_sample),
            2 => Some(decode_ms_adpcm(&s.data, s.channels as usize, s.block_align as usize)),
            _ => None,
        };
        let Some(samples) = samples else {
            return host::unimplemented_value("Microsoft.Xna.Framework.Audio.SoundEffect: wave format other than PCM 8/16 bit or MS ADPCM");
        };
        let sound = Sound { name: name.to_owned(), channels: s.channels, sample_rate: s.sample_rate, samples, loop_start: s.loop_start.max(0) as u32, loop_length: s.loop_length.max(0) as u32 };
        Self::create(&trace::asset_label(name), &xnb_format_bytes(s), &s.data, sound, s.loop_start, s.loop_length)
    }

    /// `SoundEffect.FromStream(stream)`: XNA's `WavFile` (RIFF WAVE, PCM 8 / 16 bit, mono / stereo, 8000 - 48000 Hz).
    pub fn from_stream(stream: Gc<Stream>) -> Gc<SoundEffect> {
        if stream.is_null() {
            throw("System.ArgumentNullException", "stream");
        }
        // SAFETY: checked non-null; a live runtime object.
        let s = unsafe { &mut *stream.ptr() };
        let mut bytes = vec![0u8; (s.length() - s.position()).max(0) as usize];
        let n = s.read_slice(&mut bytes);
        bytes.truncate(n);
        let w = parse_wav(&bytes);
        let samples = pcm_to_i16(&w.data, u16::from_le_bytes([w.format[14], w.format[15]])).unwrap_or_default();
        let channels = u16::from_le_bytes([w.format[2], w.format[3]]);
        let rate = u32::from_le_bytes([w.format[4], w.format[5], w.format[6], w.format[7]]);
        let sound = Sound { name: "stream".into(), channels, sample_rate: rate, samples, loop_start: w.loop_start as u32, loop_length: w.loop_length as u32 };
        Self::create("stream", &w.format, &w.data, sound, w.loop_start, w.loop_length)
    }

    pub fn duration(&self) -> TimeSpan {
        // XNA: TimeSpan.FromSeconds(samples / sampleRate): whole milliseconds (rt::time rounds like .NET Framework)
        TimeSpan::from_seconds(self.sound.seconds())
    }
    pub fn is_disposed(&self) -> bool {
        self.disposed
    }
    /// `Dispose()`: the engine's handle is released.
    pub fn dispose(&mut self) {
        if !self.disposed {
            self.disposed = true;
            engine::release_sound_effect(self.handle);
        }
    }
    fn check(&self) {
        if self.disposed {
            throw("System.ObjectDisposedException", "Cannot access a disposed object. Object name: 'SoundEffect'.");
        }
    }

    /// `Play()`: fire and forget at volume 1, pitch 0, pan 0.
    pub fn play(&self) -> bool {
        self.play_with(1.0, 0.0, 0.0)
    }
    /// `Play(volume, pitch, pan)`: a fire-and-forget instance (XNA keeps it until it stops; the game does not use this).
    pub fn play_with(&self, volume: f32, pitch: f32, pan: f32) -> bool {
        self.check();
        let i = self.create_instance();
        // SAFETY: just allocated
        let i = unsafe { &mut *i.ptr() };
        i.set_volume(volume);
        i.set_pitch(pitch);
        i.set_pan(pan);
        i.play();
        true
    }
    /// `CreateInstance()`: XNA's `SoundEffectInstance(parent, false)`: a voice, then Volume 1, Pitch 0, Pan 0.
    pub fn create_instance(&self) -> Gc<SoundEffectInstance> {
        self.check();
        let voice = match engine::create_instance(self.handle) {
            Ok(v) => v,
            Err(hr) => {
                check_hr(hr);
                unreachable!()
            }
        };
        let g = rt::alloc_init(SoundEffectInstance { voice, loop_start: self.loop_start, loop_length: self.loop_length, volume: 1.0, pitch: 0.0, pan: 0.0, looped: false, submitted: false, disposed: false, dynamic: None });
        // SAFETY: just allocated
        let i = unsafe { &mut *g.ptr() };
        i.set_volume(1.0);
        i.set_pitch(0.0);
        i.set_pan(0.0);
        g
    }
}

/// `SoundEffect.MasterVolume.get`: 1 until set.
pub fn master_volume() -> f32 {
    MASTER.with(|m| *m.borrow())
}
/// `SoundEffect.MasterVolume.set`: 0..1, then the engine's master volume.
pub fn set_master_volume(v: f32) {
    if !(v >= 0.0 && v <= 1.0) {
        throw("System.ArgumentOutOfRangeException", "value");
    }
    check_hr(engine::set_master_volume(v));
    MASTER.with(|m| *m.borrow_mut() = v);
}

/// What XNA's `WavFile` takes from a RIFF WAVE stream.
struct Wav {
    /// the fmt chunk, at least 18 bytes (`AudioHelper.LocalizeFormat`: a shorter chunk gets cbSize 0 added)
    format: Vec<u8>,
    /// the data chunk, whole blocks only
    data: Vec<u8>,
    loop_start: i32,
    loop_length: i32,
}

/// XNA's `WavFile(stream)`: the RIFF header must match the stream length (`InvalidOperationException`); chunks are
/// read until one fails; no valid fmt or data chunk is `ArgumentException`. Loop region: the whole data, or the
/// first usable loop of a `wsmp` / `smpl` chunk.
fn parse_wav(b: &[u8]) -> Wav {
    let bad = || -> ! { throw("System.ArgumentException", "Ensure that the specified stream contains valid PCM wave data.") };
    if b.is_empty() {
        throw("System.ArgumentNullException", "source");
    }
    let u32at = |o: usize| b.get(o..o + 4).map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]));
    if b.len() < 12 || &b[0..4] != b"RIFF" || u32at(4).map(|n| n as usize) != Some(b.len() - 8) || &b[8..12] != b"WAVE" {
        throw("System.InvalidOperationException", "Operation is not valid due to the current state of the object.");
    }
    let mut pos = 12;
    let (mut format, mut data): (Option<Vec<u8>>, Option<Vec<u8>>) = (None, None);
    let (mut smpl, mut wsmp): (Option<Vec<u8>>, Option<Vec<u8>>) = (None, None);
    while pos < b.len() {
        if pos % 2 == 1 || pos + 8 > b.len() {
            break;
        }
        let id = &b[pos..pos + 4];
        let len = u32at(pos + 4).unwrap_or(0) as usize;
        let body = &b[pos + 8..(pos + 8 + len).min(b.len())];
        if id == b"fmt " {
            if len < 16 || body.len() < 16 {
                break;
            }
            let u16f = |o: usize| u16::from_le_bytes([body[o], body[o + 1]]);
            let (tag, channels, rate, align, bits) = (u16f(0), u16f(2), u32::from_le_bytes([body[4], body[5], body[6], body[7]]), u16f(12), u16f(14));
            if tag != 1 || !(channels == 1 || channels == 2) || !(8000..=48000).contains(&rate) || !(bits == 8 || bits == 16) || align != channels * bits / 8 {
                format = None;
                break;
            }
            let mut f = body.to_vec();
            if f.len() < 18 {
                f.resize(18, 0);
            }
            format = Some(f);
        } else if id == b"data" {
            let Some(f) = &format else { break };
            let align = u16::from_le_bytes([f[12], f[13]]) as usize;
            if len < align || body.len() < len {
                break;
            }
            data = Some(body[..len - len % align].to_vec());
        } else if id == b"smpl" {
            smpl = Some(body.to_vec());
        } else if id == b"wsmp" {
            wsmp = Some(body.to_vec());
        }
        pos += 8 + len;
        if pos % 2 == 1 {
            pos += 1;
        }
    }
    let (Some(format), Some(data)) = (format, data) else { bad() };
    let align = u16::from_le_bytes([format[12], format[13]]) as i32;
    let samples = data.len() as i32 / align.max(1);
    let (mut loop_start, mut loop_length) = (0, samples);
    let i32at = |c: &[u8], o: usize| c.get(o..o + 4).map(|x| i32::from_le_bytes([x[0], x[1], x[2], x[3]]));
    if let Some(c) = wsmp {
        let n = i32at(&c, 16).unwrap_or(0);
        let mut o = 20;
        for _ in 0..n.max(0) {
            let kind = i32at(&c, o + 4).unwrap_or(-1);
            if kind == 0 || kind == 1 {
                let (s, l) = (i32at(&c, o + 8).unwrap_or(-1), i32at(&c, o + 12).unwrap_or(0));
                if s >= 0 && s + l <= samples {
                    (loop_start, loop_length) = (s, l);
                    break;
                }
                o += 16;
            } else {
                o += 8;
            }
        }
    } else if let Some(c) = smpl {
        let n = i32at(&c, 28).unwrap_or(0);
        let mut o = 36;
        for _ in 0..n.max(0) {
            let kind = i32at(&c, o + 4).unwrap_or(-1);
            if kind == 0 {
                let (s, e) = (i32at(&c, o + 8).unwrap_or(-1), i32at(&c, o + 12).unwrap_or(-1));
                if s >= 0 && s < samples && s < e && e <= samples {
                    (loop_start, loop_length) = (s, e - s + 1);
                }
                break;
            }
            o += 24;
        }
    }
    Wav { format, data, loop_start, loop_length }
}

// ------------------------------------------------------------------------------------------------ SoundEffectInstance

/// `SoundEffectInstance` and `DynamicSoundEffectInstance` (`dynamic` is `Some`): XNA's managed fields, the engine's
/// voice handle.
pub struct SoundEffectInstance {
    pub voice: u32,
    loop_start: i32,
    loop_length: i32,
    /// `currentVolume` / `currentPitch` / `currentPan`
    volume: f32,
    pitch: f32,
    pan: f32,
    looped: bool,
    /// `isPacketSubmitted`
    submitted: bool,
    pub disposed: bool,
    /// (channels, sample rate) of a dynamic instance
    dynamic: Option<(u16, u32)>,
}
plain_trace!(SoundEffectInstance);

impl SoundEffectInstance {
    /// `new DynamicSoundEffectInstance(sampleRate, channels)`; `AudioChannels`: Mono 1, Stereo 2. The base class's
    /// parameterless constructor (no Volume / Pitch / Pan calls), then a dynamic voice for a 16-bit PCM format.
    pub fn new_dynamic(sample_rate: i32, channels: i32) -> Gc<SoundEffectInstance> {
        if !(8000..=48000).contains(&sample_rate) {
            throw("System.ArgumentOutOfRangeException", "sampleRate");
        }
        if !(1..=2).contains(&channels) {
            throw("System.ArgumentOutOfRangeException", "channels");
        }
        // AudioHelper.MakeFormat(sampleRate, channels, 16)
        let mut f = Vec::with_capacity(18);
        f.extend_from_slice(&1u16.to_le_bytes());
        f.extend_from_slice(&(channels as u16).to_le_bytes());
        f.extend_from_slice(&(sample_rate as u32).to_le_bytes());
        f.extend_from_slice(&((sample_rate * 16 / 8 * channels) as u32).to_le_bytes());
        f.extend_from_slice(&((channels * 16 / 8) as u16).to_le_bytes());
        f.extend_from_slice(&16u16.to_le_bytes());
        f.extend_from_slice(&0u16.to_le_bytes());
        let voice = engine::create_dynamic(&f);
        rt::alloc_init(SoundEffectInstance { voice, loop_start: 0, loop_length: 0, volume: 1.0, pitch: 0.0, pan: 0.0, looped: false, submitted: false, disposed: false, dynamic: Some((channels as u16, sample_rate as u32)) })
    }

    fn check(&self) {
        if self.disposed {
            throw("System.ObjectDisposedException", "Cannot access a disposed object. Object name: 'SoundEffectInstance'.");
        }
    }
    pub fn is_disposed(&self) -> bool {
        self.disposed
    }
    pub fn volume(&self) -> f32 {
        self.volume
    }
    /// `Volume.set`: 0..1 (`ArgumentOutOfRangeException` otherwise, NaN included), then the engine.
    pub fn set_volume(&mut self, v: f32) {
        self.check();
        if !(v >= 0.0 && v <= 1.0) {
            throw("System.ArgumentOutOfRangeException", "value");
        }
        check_hr(engine::set_volume(self.voice, v));
        self.volume = v;
    }
    pub fn pitch(&self) -> f32 {
        self.pitch
    }
    /// `Pitch.set`: -1..1 octaves.
    pub fn set_pitch(&mut self, v: f32) {
        self.check();
        if !(v >= -1.0 && v <= 1.0) {
            throw("System.ArgumentOutOfRangeException", "value");
        }
        check_hr(engine::set_pitch(self.voice, v));
        self.pitch = v;
    }
    pub fn pan(&self) -> f32 {
        self.pan
    }
    /// `Pan.set`: -1..1 (3D positioning, which would make it an InvalidOperation, is not used by the game).
    pub fn set_pan(&mut self, v: f32) {
        self.check();
        if !(v >= -1.0 && v <= 1.0) {
            throw("System.ArgumentOutOfRangeException", "value");
        }
        check_hr(engine::set_pan(self.voice, v));
        self.pan = v;
    }
    pub fn is_looped(&self) -> bool {
        if self.dynamic.is_some() {
            self.check();
            return false;
        }
        self.looped
    }
    /// `IsLooped.set`: `InvalidOperationException` once the instance has been played (a dynamic instance: true is
    /// always invalid).
    pub fn set_is_looped(&mut self, v: bool) {
        self.check();
        if self.dynamic.is_some() {
            if v {
                throw("System.InvalidOperationException", "This method does not accept null for this parameter.");
            }
            return;
        }
        if self.submitted {
            throw("System.InvalidOperationException", "This method does not accept null for this parameter.");
        }
        self.looped = v;
    }

    /// `State`: the engine's voice state: Paused when its paused bit is set, Playing when Playing or Stopping.
    pub fn state(&self) -> i32 {
        self.check();
        let s = match engine::get_state(self.voice) {
            Ok(s) => s,
            Err(hr) => {
                check_hr(hr);
                unreachable!()
            }
        };
        let mut r = STATE_STOPPED;
        if s & engine::VS_PAUSED != 0 {
            r = STATE_PAUSED;
        }
        if s == engine::VS_PLAYING || s == engine::VS_STOPPING {
            r = STATE_PLAYING;
        }
        r
    }

    /// `Play()`: the buffer is submitted the first time (loop region, loop count -1 when looped), then played. A
    /// dynamic instance plays its queue.
    pub fn play(&mut self) {
        self.check();
        if self.dynamic.is_none() && !self.submitted {
            check_hr(engine::submit_packet(self.voice, self.loop_start, self.loop_length, if self.looped { -1 } else { 0 }));
            self.submitted = true;
        }
        check_hr(engine::play(self.voice));
        self.submitted = true;
    }
    /// `Pause()`
    pub fn pause(&mut self) {
        self.check();
        check_hr(engine::pause(self.voice));
    }
    /// `Resume()`: plays an instance that was never played; plays a paused or stopped one.
    pub fn resume(&mut self) {
        self.check();
        if !self.submitted {
            return self.play();
        }
        // `State == Paused || State == Stopped`: the property is read twice when the first test fails
        if self.state() == STATE_PAUSED || self.state() == STATE_STOPPED {
            check_hr(engine::play(self.voice));
        }
    }
    /// `Stop()` = `Stop(true)`.
    pub fn stop(&mut self) {
        self.stop_with(true);
    }
    /// `Stop(immediate)`: immediate stops and flushes the buffer; otherwise a loop ends after its current pass.
    pub fn stop_with(&mut self, immediate: bool) {
        self.check();
        check_hr(engine::stop(self.voice, immediate));
    }
    /// `Dispose()`: the voice is destroyed.
    pub fn dispose(&mut self) {
        if !self.disposed {
            self.disposed = true;
            engine::destroy_instance(self.voice);
            self.submitted = false;
        }
    }

    // ---- DynamicSoundEffectInstance

    /// `SubmitBuffer(byte[] buffer)` / `(buffer, offset, count)`: 16-bit PCM, block aligned (XNA's checks).
    pub fn submit_buffer(&mut self, buffer: Arr<u8>, offset: i32, count: i32) {
        self.check();
        let Some((channels, _)) = self.dynamic else { throw("System.InvalidOperationException", "SubmitBuffer is only available on a DynamicSoundEffectInstance.") };
        let align = 2 * channels as i64;
        if buffer.is_null() || buffer.len() == 0 || buffer.len() as i64 % align != 0 {
            throw("System.ArgumentException", "Ensure that the buffer length is non-zero and meets the block alignment requirements for the audio format.");
        }
        if offset < 0 || offset as i64 >= buffer.len() as i64 || offset as i64 % align != 0 {
            throw("System.ArgumentException", "The offset must be non-negative, less than the buffer length and block aligned.");
        }
        if count <= 0 || offset as i64 + count as i64 > buffer.len() as i64 || count as i64 % align != 0 {
            throw("System.ArgumentException", "Ensure that the play length is non-zero and meets the block alignment requirements for the audio format.");
        }
        let bytes = &buffer.as_slice()[offset as usize..(offset + count) as usize];
        if engine::submit_buffer(self.voice, bytes) != 0 {
            throw("System.InvalidOperationException", "There are too many buffers pending.");
        }
    }
    /// `PendingBufferCount`: buffers submitted and not played yet.
    pub fn pending_buffer_count(&self) -> i32 {
        self.check();
        match engine::pending_buffer_count(self.voice) {
            Ok(n) => n,
            Err(hr) => {
                check_hr(hr);
                0
            }
        }
    }
    /// Seconds of submitted audio not played yet (diagnostics).
    pub fn pending_seconds(&self) -> f64 {
        engine::pending_seconds(self.voice)
    }
}

// ------------------------------------------------------------------------------------------------ XACT

/// Everything the XACT classes share (`AudioEngine`, `SoundBank`, `WaveBank`, `Cue`, `AudioCategory` are separate
/// objects in XNA; XACT binds them by name inside the engine, as here).
#[derive(Default)]
struct XactState {
    xgs: Option<xact::Xgs>,
    /// user volume of each category (`AudioCategory.SetVolume`), linear
    category_volume: Vec<f32>,
    /// wave banks by their name inside the file
    banks: Vec<(String, Arc<WaveBankData>)>,
    /// decoded waves (bank name, entry): the most recently used few
    cache: Vec<((String, usize), Arc<Sound>)>,
    /// cues that are playing: re-gained when their category's volume changes
    live: Vec<LiveCue>,
    stats: XactStats,
}
struct LiveCue {
    voice: u32,
    category: usize,
    /// sound volume + RPC curves, decibels
    base_db: f32,
}

/// What the XACT path did (tests and the demo report it).
#[derive(Default, Clone, Debug)]
pub struct XactStats {
    pub waves_decoded: u64,
    pub pcm_bytes_decoded: u64,
    pub decode_micros: u64,
}

thread_local! {
    static XACT: RefCell<XactState> = RefCell::new(XactState::default());
}

/// Decoded waves kept after their cue is gone. A music track is 20 - 90 MB of PCM, so only a few stay.
const WAVE_CACHE: usize = 2;

pub fn xact_stats() -> XactStats {
    XACT.with(|x| x.borrow().stats.clone())
}

/// XNA's check of an XACT file's signature (the first 4 bytes): `ArgumentException` otherwise.
fn xact_file(file: Str, signature: &[u8; 4]) -> (std::path::PathBuf, Vec<u8>) {
    let Some(name) = file.get().filter(|n| !n.is_empty()) else { throw("System.ArgumentNullException", "filename") };
    let p = crate::content::install_dir().join(name);
    let bytes = match std::fs::read(&p) {
        Ok(b) => b,
        Err(_) => rt::throw(rt::exc("System.IO.FileNotFoundException", Str::new(&format!("Could not find file '{}'.", p.display())))),
    };
    if bytes.len() <= 4 || &bytes[..4] != signature {
        throw("System.ArgumentException", "The file is not an XACT file of the version this framework reads.");
    }
    (p, bytes)
}

/// `AudioEngine`: parses `TerrariaMusic.xgs` (categories, variables, RPC curves).
pub struct AudioEngine {
    pub settings_file: Str,
    pub disposed: bool,
}
// SAFETY: the one handle is reported.
unsafe impl Trace for AudioEngine {
    unsafe fn trace(&self, m: &mut Marker) {
        self.settings_file.trace(m)
    }
}
impl AudioEngine {
    /// `new AudioEngine(settingsFile)` = (settingsFile, 250 ms, "").
    pub fn new_gc(settings_file: Str) -> Gc<AudioEngine> {
        Self::new_with(settings_file, TimeSpan::from_milliseconds(250.0), Str::new(""))
    }
    /// `new AudioEngine(settingsFile, lookAheadTime, rendererId)`: `FileNotFoundException` when the file is missing,
    /// `ArgumentException` when it is not an XGS file.
    pub fn new_with(settings_file: Str, look_ahead: TimeSpan, renderer: Str) -> Gc<AudioEngine> {
        let (p, bytes) = xact_file(settings_file, b"XGSF");
        let lookahead_ms = (look_ahead.ticks as f64 / 10_000.0) as u32;
        engine::xact_engine_create(&p.to_string_lossy(), lookahead_ms, renderer.get().unwrap_or(""));
        match xact::Xgs::parse(&bytes) {
            Ok(x) => XACT.with(|s| {
                let mut s = s.borrow_mut();
                s.category_volume = vec![1.0; x.categories.len()];
                s.xgs = Some(x);
            }),
            Err(e) => rt::throw(rt::exc("System.InvalidOperationException", Str::new(&format!("{}: {e}", p.display())))),
        }
        rt::alloc_init(AudioEngine { settings_file, disposed: false })
    }
    /// `Update()`: XACT's periodic work (cue state is decided by the engine model).
    pub fn update(&mut self) {
        engine::xact_engine_update();
    }
    /// `GetCategory(name)`: `ArgumentException` for an unknown name.
    pub fn get_category(&self, name: Str) -> Gc<AudioCategory> {
        let n = name.get().unwrap_or_else(|| throw("System.ArgumentNullException", "name"));
        let idx = XACT.with(|s| s.borrow().xgs.as_ref().and_then(|x| x.category_index(n)));
        let engine_index = engine::xact_get_category(n);
        match idx {
            Some(index) => rt::alloc_init(AudioCategory { name, index, engine_index }),
            None => rt::throw(rt::exc("System.ArgumentException", Str::new(&format!("The category \"{n}\" is not in the audio engine's settings.")))),
        }
    }
    /// Category names of the loaded settings file.
    pub fn category_names() -> Vec<String> {
        XACT.with(|s| s.borrow().xgs.as_ref().map(|x| x.categories.iter().map(|c| c.name.clone()).collect()).unwrap_or_default())
    }
    pub fn dispose(&mut self) {
        if !self.disposed {
            self.disposed = true;
            engine::xact_engine_release();
        }
    }
}

/// `AudioCategory` (`AudioEngine.GetCategory(name)`).
pub struct AudioCategory {
    pub name: Str,
    pub index: usize,
    engine_index: u16,
}
// SAFETY: the one handle is reported.
unsafe impl Trace for AudioCategory {
    unsafe fn trace(&self, m: &mut Marker) {
        self.name.trace(m)
    }
}
impl AudioCategory {
    pub fn name(&self) -> Str {
        self.name
    }
    /// `SetVolume(volume)`: a linear factor on every cue of the category and its children (XNA: 1 is unchanged).
    pub fn set_volume(&self, volume: f32) {
        if volume < 0.0 {
            throw("System.ArgumentOutOfRangeException", "volume");
        }
        engine::xact_category_volume(self.engine_index, volume);
        let index = self.index;
        let changes: Vec<(u32, f32)> = XACT.with(|s| {
            let mut s = s.borrow_mut();
            s.category_volume[index] = volume;
            let s = &*s;
            s.live.iter().filter(|l| category_chain(s, l.category).contains(&index)).map(|l| (l.voice, cue_gain(s, l.category, l.base_db))).collect()
        });
        trace::internal(|| {
            for (voice, gain) in changes {
                if engine::voice_known(voice) {
                    engine::set_volume(voice, gain);
                }
            }
        });
    }
    /// `Stop(options)`: stops the cues of the category that are playing.
    pub fn stop(&self, options: i32) {
        engine::xact_category_stop(self.engine_index, options as u32);
        let index = self.index;
        let voices: Vec<u32> = XACT.with(|s| {
            let s = s.borrow();
            let in_cat = |c: usize| category_chain(&s, c).contains(&index);
            s.live.iter().filter(|l| in_cat(l.category)).map(|l| l.voice).collect()
        });
        trace::internal(|| {
            for voice in voices {
                engine::stop(voice, true);
            }
        });
    }
}

/// The category and its parents, innermost first.
fn category_chain(s: &XactState, mut c: usize) -> Vec<usize> {
    let mut v = Vec::new();
    let Some(x) = s.xgs.as_ref() else { return v };
    while c < x.categories.len() && !v.contains(&c) {
        v.push(c);
        let p = x.categories[c].parent;
        if p < 0 {
            break;
        }
        c = p as usize;
    }
    v
}

/// Linear gain of a cue: its own decibels, the categories' file volumes and the user volumes set by `SetVolume`.
fn cue_gain(s: &XactState, category: usize, base_db: f32) -> f32 {
    let chain = category_chain(s, category);
    let (mut db, mut user) = (base_db, 1.0f32);
    for c in chain {
        db += s.xgs.as_ref().map_or(0.0, |x| x.categories[c].volume_db);
        user *= s.category_volume.get(c).copied().unwrap_or(1.0);
    }
    xact::db_to_amplitude(db) * user
}

#[derive(Clone, Debug, PartialEq)]
pub struct WaveEntry {
    pub codec: u32,
    pub channels: u16,
    pub sample_rate: u32,
    pub block_align: u32,
    pub offset: u64,
    pub length: u64,
}

/// The table of a wave bank, shared with the cues that play from it.
#[derive(Debug)]
pub struct WaveBankData {
    pub name: String,
    pub path: std::path::PathBuf,
    pub entries: Vec<WaveEntry>,
}

/// `WaveBank` (`Wave Bank.xwb`, a streaming bank): the entry table; sample data is read on demand.
pub struct WaveBank {
    pub path: std::path::PathBuf,
    pub entries: Vec<WaveEntry>,
    pub name: String,
    pub disposed: bool,
}
plain_trace!(WaveBank);

impl WaveBankData {
    /// Entry `index` as PCM (codec 0) or decoded MS ADPCM (codec 2).
    pub fn decode(&self, index: usize) -> Result<Sound, String> {
        use std::io::{Read, Seek, SeekFrom};
        let e = self.entries.get(index).ok_or("wave index outside the bank")?;
        let mut f = std::fs::File::open(&self.path).map_err(|x| x.to_string())?;
        f.seek(SeekFrom::Start(e.offset)).map_err(|x| x.to_string())?;
        let mut data = vec![0u8; e.length as usize];
        f.read_exact(&mut data).map_err(|x| x.to_string())?;
        let samples = match e.codec {
            0 => pcm_to_i16(&data, 16).ok_or("PCM depth")?,
            2 => decode_ms_adpcm(&data, e.channels as usize, e.block_align as usize),
            c => return Err(format!("wave bank codec {c} (XMA / xWMA) is not decoded")),
        };
        Ok(Sound { name: format!("wave {index}"), channels: e.channels, sample_rate: e.sample_rate, samples, loop_start: 0, loop_length: 0 })
    }
}

impl WaveBank {
    /// Parses the header and entry table of an XACT 3 wave bank ("WBND", version 46).
    pub fn open(path: &std::path::Path) -> Result<WaveBank, String> {
        use std::io::Read;
        let mut f = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut head = vec![0u8; 4096];
        let n = f.read(&mut head).map_err(|e| e.to_string())?;
        head.truncate(n);
        let u32at = |b: &[u8], o: usize| b.get(o..o + 4).map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]])).ok_or("truncated wave bank header");
        if head.get(0..4) != Some(b"WBND") {
            return Err("not an XACT wave bank (no WBND signature)".into());
        }
        let seg = |i: usize| -> Result<(usize, usize), &str> { Ok((u32at(&head, 12 + 8 * i)? as usize, u32at(&head, 16 + 8 * i)? as usize)) };
        let (bank, _) = seg(0)?;
        let (meta, meta_len) = seg(1)?;
        let (data, _) = seg(4)?;
        let count = u32at(&head, bank + 4)? as usize;
        let name_bytes = head.get(bank + 8..bank + 72).ok_or("truncated wave bank header")?;
        let name = String::from_utf8_lossy(&name_bytes[..name_bytes.iter().position(|c| *c == 0).unwrap_or(64)]).into_owned();
        let entry_size = u32at(&head, bank + 72)? as usize;
        if entry_size < 24 || count * entry_size > meta_len {
            return Err(format!("unsupported wave bank entry layout (entry size {entry_size}, {count} entries)"));
        }
        let mut table = head.clone();
        if table.len() < meta + meta_len {
            use std::io::{Seek, SeekFrom};
            table = vec![0u8; meta + meta_len];
            f.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
            f.read_exact(&mut table).map_err(|e| e.to_string())?;
        }
        let mut entries = Vec::with_capacity(count);
        for i in 0..count {
            let e = meta + i * entry_size;
            let format = u32at(&table, e + 4)?;
            let (codec, channels, rate, align) = (format & 3, (format >> 2) & 7, (format >> 5) & 0x3FFFF, (format >> 23) & 0xFF);
            entries.push(WaveEntry {
                codec,
                channels: channels as u16,
                sample_rate: rate,
                // MS ADPCM: (align + 22) * channels bytes per block
                block_align: if codec == 2 { (align + 22) * channels } else { align },
                offset: data as u64 + u32at(&table, e + 8)? as u64,
                length: u32at(&table, e + 12)? as u64,
            });
        }
        Ok(WaveBank { path: path.to_owned(), entries, name, disposed: false })
    }

    fn register(filename: Str, streaming: Option<(i32, i16)>) -> Gc<WaveBank> {
        let (p, _) = xact_file(filename, b"WBND");
        match streaming {
            Some((offset, packet_size)) => engine::xact_wave_bank_streaming(&p.to_string_lossy(), offset as u32, packet_size),
            None => engine::xact_wave_bank(&p.to_string_lossy()),
        };
        match WaveBank::open(&p) {
            Ok(w) => {
                let data = Arc::new(WaveBankData { name: w.name.clone(), path: w.path.clone(), entries: w.entries.clone() });
                XACT.with(|s| {
                    let mut s = s.borrow_mut();
                    s.banks.retain(|(n, _)| *n != w.name);
                    s.banks.push((w.name.clone(), data));
                });
                rt::alloc_init(w)
            }
            Err(e) => rt::throw(rt::exc("System.InvalidOperationException", Str::new(&e))),
        }
    }
    /// `new WaveBank(audioEngine, filename)`: an in-memory bank; registered under the name written in the file,
    /// which is how sound banks refer to it.
    pub fn new_gc(filename: Str) -> Gc<WaveBank> {
        Self::register(filename, None)
    }
    /// `new WaveBank(audioEngine, filename, offset, packetsize)`: a streaming bank (the game's).
    pub fn new_streaming(filename: Str, offset: i32, packet_size: i16) -> Gc<WaveBank> {
        Self::register(filename, Some((offset, packet_size)))
    }
    /// `IsPrepared`: the status's Prepared bit (the engine model prepares a bank at once).
    pub fn is_prepared(&self) -> bool {
        engine::xact_wave_bank_status() & 4 != 0
    }
    /// Entry `index` as PCM (codec 0) or decoded MS ADPCM (codec 2).
    pub fn decode(&self, index: usize) -> Result<Sound, String> {
        WaveBankData { name: self.name.clone(), path: self.path.clone(), entries: self.entries.clone() }.decode(index)
    }
    pub fn dispose(&mut self) {
        if !self.disposed {
            self.disposed = true;
            engine::xact_wave_bank_release();
        }
    }
}

/// `SoundBank` (`Sound Bank.xsb`): cue names, sounds and their wave references.
pub struct SoundBank {
    pub data: Arc<xact::Xsb>,
    pub disposed: bool,
}
impl SoundBank {
    /// `new SoundBank(audioEngine, filename)`: `ArgumentException` when the file is not a sound bank.
    pub fn new_gc(filename: Str) -> Gc<SoundBank> {
        let (p, bytes) = xact_file(filename, b"SDBK");
        engine::xact_sound_bank(&p.to_string_lossy());
        match xact::Xsb::parse(&bytes) {
            Ok(x) => rt::alloc_init(SoundBank { data: Arc::new(x), disposed: false }),
            Err(e) => rt::throw(rt::exc("System.InvalidOperationException", Str::new(&format!("{}: {e}", p.display())))),
        }
    }
    /// `GetCue(name)`: a prepared cue; `ArgumentException` when the bank has no such cue.
    pub fn get_cue(&self, name: Str) -> Gc<Cue> {
        let Some(n) = name.get().filter(|n| !n.is_empty()) else { throw("System.ArgumentNullException", "name") };
        let Some(index) = self.data.cue_index(n) else {
            rt::throw(rt::exc("System.ArgumentException", Str::new(&format!("The cue \"{n}\" was not found in the sound bank."))));
        };
        let handle = engine::xact_get_cue(n);
        let vars: Vec<f32> = XACT.with(|s| s.borrow().xgs.as_ref().map(|x| x.variables.iter().map(|v| v.initial).collect()).unwrap_or_default());
        rt::alloc_init(Cue { name, bank: self.data.clone(), cue: index, handle, vars, voice: None, disposed: false })
    }
    /// `IsInUse`: the status's in-use bit (never set by the model).
    pub fn is_in_use(&self) -> bool {
        engine::xact_sound_bank_status() & 0x80 != 0
    }
    pub fn dispose(&mut self) {
        if !self.disposed {
            self.disposed = true;
            engine::xact_sound_bank_release();
        }
    }
    pub fn cue_names(&self) -> Vec<String> {
        self.data.cues.iter().map(|c| c.name.clone()).collect()
    }
}
// SAFETY: no handles inside.
unsafe impl Trace for SoundBank {
    const NEEDS_TRACE: bool = false;
    unsafe fn trace(&self, _m: &mut Marker) {}
}

/// `Cue`: one instance of a sound-bank cue. Its state is the engine's status bits; its wave (decoded when the cue
/// first plays) plays as an internal engine voice; `SetVariable` before `Play` is remembered.
pub struct Cue {
    pub name: Str,
    bank: Arc<xact::Xsb>,
    cue: usize,
    /// the engine's cue handle
    handle: u32,
    /// current value of every XGS variable (initial values until set)
    vars: Vec<f32>,
    /// the internal voice of the cue's wave, once it plays
    voice: Option<u32>,
    disposed: bool,
}
// SAFETY: the one handle is reported.
unsafe impl Trace for Cue {
    unsafe fn trace(&self, m: &mut Marker) {
        self.name.trace(m);
    }
}

/// The decoded wave for (bank name, entry): from the small cache or freshly decoded.
fn wave_for(bank_name: &str, index: usize) -> Result<Arc<Sound>, String> {
    let (data, hit) = XACT.with(|s| {
        let s = s.borrow();
        let hit = s.cache.iter().find(|(k, _)| k.0 == bank_name && k.1 == index).map(|(_, v)| v.clone());
        (s.banks.iter().find(|(n, _)| n == bank_name).map(|(_, d)| d.clone()), hit)
    });
    if let Some(h) = hit {
        return Ok(h);
    }
    let data = data.ok_or_else(|| format!("wave bank \"{bank_name}\" is not loaded (new WaveBank must come first)"))?;
    let t = std::time::Instant::now();
    let sound = Arc::new(data.decode(index)?);
    XACT.with(|s| {
        let mut s = s.borrow_mut();
        s.stats.waves_decoded += 1;
        s.stats.pcm_bytes_decoded += sound.samples.len() as u64 * 2;
        s.stats.decode_micros += t.elapsed().as_micros() as u64;
        s.cache.push(((bank_name.to_owned(), index), sound.clone()));
        while s.cache.len() > WAVE_CACHE {
            s.cache.remove(0);
        }
    });
    Ok(sound)
}

impl Cue {
    pub fn name(&self) -> Str {
        self.name
    }
    fn sound_def(&self) -> Result<&xact::SoundDef, String> {
        let c = &self.bank.cues[self.cue];
        let i = c.sound.ok_or_else(|| format!("cue \"{}\" has no simple sound (complex cues are not parsed)", c.name))?;
        let s = &self.bank.sounds[i];
        if s.complex {
            return Err(format!("cue \"{}\" uses a complex sound (several tracks / events), which is not parsed", c.name));
        }
        Ok(s)
    }
    /// The sound's decibels with the RPC curves for the current variable values.
    fn base_db(&self, s: &xact::SoundDef) -> f32 {
        XACT.with(|x| {
            let x = x.borrow();
            let mut db = s.volume_db;
            if let Some(g) = x.xgs.as_ref() {
                for code in &s.rpc_codes {
                    if let Ok(r) = g.rpc(*code) {
                        if r.parameter == 0 {
                            db += r.eval(self.vars.get(r.variable as usize).copied().unwrap_or(0.0)) / 100.0;
                        }
                    }
                }
            }
            db
        })
    }
    fn gain(&self, s: &xact::SoundDef) -> f32 {
        let base = self.base_db(s);
        XACT.with(|x| cue_gain(&x.borrow(), s.category as usize, base))
    }

    /// `Play()`: a prepared cue starts its wave; a cue plays once (XACT's invalid-usage error: Cue.Play throws).
    pub fn play(&mut self) {
        if !engine::xact_cue_play(self.handle) {
            throw("System.InvalidOperationException", "The cue has already been played; get a new cue from the sound bank.");
        }
        let s = match self.sound_def() {
            Ok(s) => s.clone(),
            Err(e) => rt::throw(rt::exc("System.InvalidOperationException", Str::new(&e))),
        };
        let (wave, bank_idx) = s.wave.map(|(w, b)| (w as usize, b as usize)).expect("simple sound has a wave");
        let bank_name = self.bank.wavebanks.get(bank_idx).cloned().unwrap_or_default();
        let sound = match wave_for(&bank_name, wave) {
            Ok(s) => s,
            Err(e) => rt::throw(rt::exc("System.InvalidOperationException", Str::new(&e))),
        };
        let gain = self.gain(&s);
        let pitch = (s.pitch as f32 / 1200.0).clamp(-1.0, 1.0);
        let frames = sound.frames() as i32;
        let voice = trace::internal(|| {
            let v = engine::create_internal_voice(sound);
            engine::set_volume(v, gain);
            if pitch != 0.0 {
                engine::set_pitch(v, pitch);
            }
            engine::submit_packet(v, 0, frames, 0);
            engine::play(v);
            v
        });
        engine::xact_cue_set_voice(self.handle, voice);
        let base_db = self.base_db(&s);
        XACT.with(|x| {
            let mut x = x.borrow_mut();
            // forget cues whose voice is gone
            x.live.retain(|l| engine::voice_known(l.voice));
            x.live.push(LiveCue { voice, category: s.category as usize, base_db });
        });
        self.voice = Some(voice);
    }
    pub fn pause(&mut self) {
        let bits = engine::xact_cue_pause(self.handle, true);
        if let (Some(v), true) = (self.voice, bits == engine::CUE_PAUSED) {
            trace::internal(|| engine::pause(v));
        }
    }
    pub fn resume(&mut self) {
        let bits = engine::xact_cue_pause(self.handle, false);
        if let (Some(v), true) = (self.voice, bits == engine::CUE_PLAYING) {
            trace::internal(|| engine::play(v));
        }
    }
    /// `Stop(AudioStopOptions)`: both options stop at once here (`AsAuthored` waits for an authored release, and
    /// Terraria's sounds have none).
    pub fn stop(&mut self, options: i32) {
        engine::xact_cue_stop(self.handle, options as u32);
        if let Some(v) = self.voice.take() {
            trace::internal(|| {
                engine::stop(v, true);
                engine::destroy_instance(v);
            });
            XACT.with(|x| x.borrow_mut().live.retain(|l| l.voice != v));
        }
    }
    fn status(&self) -> u32 {
        engine::xact_cue_status(self.handle)
    }
    pub fn is_playing(&self) -> bool {
        self.status() & engine::CUE_PLAYING != 0
    }
    pub fn is_paused(&self) -> bool {
        self.status() & engine::CUE_PAUSED != 0
    }
    pub fn is_stopped(&self) -> bool {
        self.status() & engine::CUE_STOPPED != 0
    }
    pub fn is_prepared(&self) -> bool {
        self.status() & engine::CUE_PREPARED != 0
    }
    pub fn is_created(&self) -> bool {
        self.status() & 1 != 0
    }
    /// `SetVariable(name, value)`: `ArgumentNullException` for an empty name; the engine takes the value
    /// (`ArgumentException` for a name the .xgs does not have); the gain of a playing cue follows (the value clamped to
    /// the variable's range, as XACT does).
    pub fn set_variable(&mut self, name: Str, value: f32) {
        let n = name.get().filter(|n| !n.is_empty()).unwrap_or_else(|| throw("System.ArgumentNullException", "name"));
        engine::xact_cue_set_variable(self.handle, n, value);
        let found = XACT.with(|x| x.borrow().xgs.as_ref().and_then(|g| g.variable_index(n).map(|i| (i, g.variables[i].min, g.variables[i].max))));
        let Some((i, min, max)) = found else { rt::throw(rt::exc("System.ArgumentException", Str::new(&format!("The variable \"{n}\" does not exist.")))) };
        if self.vars.len() <= i {
            self.vars.resize(i + 1, 0.0);
        }
        self.vars[i] = value.clamp(min, max);
        if let Some(voice) = self.voice {
            let Ok(s) = self.sound_def().cloned() else { return };
            let (gain, base_db) = (self.gain(&s), self.base_db(&s));
            trace::internal(|| engine::set_volume(voice, gain));
            XACT.with(|x| {
                if let Some(l) = x.borrow_mut().live.iter_mut().find(|l| l.voice == voice) {
                    l.base_db = base_db;
                }
            });
        }
    }
    /// `GetVariable(name)`
    pub fn get_variable(&self, name: Str) -> f32 {
        let n = name.get().filter(|n| !n.is_empty()).unwrap_or_else(|| throw("System.ArgumentNullException", "name"));
        engine::xact_cue_get_variable(self.handle, n);
        match XACT.with(|x| x.borrow().xgs.as_ref().and_then(|g| g.variable_index(n))) {
            Some(i) => self.vars.get(i).copied().unwrap_or(0.0),
            None => rt::throw(rt::exc("System.ArgumentException", Str::new(&format!("The variable \"{n}\" does not exist.")))),
        }
    }
    /// `Dispose()`: the engine's cue handle is released (a playing wave stops with it).
    pub fn dispose(&mut self) {
        if !self.disposed {
            self.disposed = true;
            if let Some(v) = self.voice.take() {
                trace::internal(|| {
                    engine::stop(v, true);
                    engine::destroy_instance(v);
                });
            }
            engine::xact_cue_release(self.handle);
        }
    }
}

#[cfg(test)]
mod xact_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(seconds: f64) -> Gc<SoundEffect> {
        let n = (22050.0 * seconds) as usize;
        let samples: Vec<i16> = (0..n).map(|i| ((i as f32 * 0.2).sin() * 8000.0) as i16).collect();
        let data: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        // PCM 16 bit mono 22050 Hz
        let mut format = Vec::new();
        for v in [1u16, 1] {
            format.extend_from_slice(&v.to_le_bytes());
        }
        format.extend_from_slice(&22050u32.to_le_bytes());
        format.extend_from_slice(&44100u32.to_le_bytes());
        for v in [2u16, 16, 0] {
            format.extend_from_slice(&v.to_le_bytes());
        }
        let sound = Sound { name: "tone".into(), channels: 1, sample_rate: 22050, samples, loop_start: 0, loop_length: 0 };
        SoundEffect::create("tone", &format, &data, sound, 0, n as i32)
    }
    const FRAME: f64 = 1.0 / 60.0;

    #[test]
    fn instance_state_machine() {
        unsafe {
            let fx = &*tone(1.0).ptr();
            assert_eq!(fx.duration().ticks, 10_000_000);
            let i = &mut *fx.create_instance().ptr();
            assert_eq!(i.state(), STATE_STOPPED);
            i.set_is_looped(false);
            i.set_volume(0.5);
            let _ = take_commands();
            i.play();
            assert_eq!(i.state(), STATE_PLAYING);
            let e = rt::try_(|| i.set_is_looped(true)).unwrap_err();
            assert!(rt::exc_is(e, "System.InvalidOperationException"));
            i.play();
            let cmds = take_commands();
            assert!(matches!(cmds.as_slice(), [AudioCommand::Play { volume, looped: false, .. }] if *volume == 0.5), "{cmds:?}");
            advance(0.4);
            i.pause();
            assert_eq!(i.state(), STATE_PAUSED);
            advance(5.0);
            assert_eq!(i.state(), STATE_PAUSED, "a paused voice does not run out");
            i.resume();
            assert_eq!(i.state(), STATE_PLAYING);
            // pitch +1 = twice as fast: 0.6 s of sound are over after 0.3 s
            i.set_pitch(1.0);
            advance(0.25);
            assert_eq!(i.state(), STATE_PLAYING);
            advance(0.1);
            assert_eq!(i.state(), STATE_STOPPED);
            i.play();
            assert_eq!(i.state(), STATE_PLAYING);
            // Stop with a queued buffer: Stopping (State reads Playing) until the next frame
            i.stop();
            assert_eq!(i.state(), STATE_PLAYING);
            advance(FRAME);
            assert_eq!(i.state(), STATE_STOPPED);
            let kinds: Vec<&str> = take_commands()
                .iter()
                .map(|c| match c {
                    AudioCommand::Pause { .. } => "pause",
                    AudioCommand::Resume { .. } => "resume",
                    AudioCommand::SetPitch { .. } => "pitch",
                    AudioCommand::Play { .. } => "play",
                    AudioCommand::Stop { .. } => "stop",
                    _ => "other",
                })
                .collect();
            assert_eq!(kinds, ["pause", "resume", "pitch", "play", "stop"]);
            for bad in [-0.1f32, 1.1, f32::NAN] {
                let e = rt::try_(|| i.set_volume(bad)).unwrap_err();
                assert!(rt::exc_is(e, "System.ArgumentOutOfRangeException"));
            }
            let e = rt::try_(|| i.set_pan(-1.5)).unwrap_err();
            assert!(rt::exc_is(e, "System.ArgumentOutOfRangeException"));
            i.dispose();
            let e = rt::try_(|| i.play()).unwrap_err();
            assert!(rt::exc_is(e, "System.ObjectDisposedException"));
        }
    }

    #[test]
    fn looping_master_volume_and_the_trace() {
        unsafe {
            trace::set_active(true);
            trace::set_full(true);
            let fx = &*tone(0.5).ptr();
            let l = &mut *fx.create_instance().ptr();
            l.set_is_looped(true);
            l.set_pan(-0.25);
            l.play();
            advance(100.0);
            assert_eq!(l.state(), STATE_PLAYING, "a looped voice never ends by itself");
            l.stop();
            assert_eq!(master_volume(), 1.0);
            set_master_volume(0.25);
            assert_eq!(master_volume(), 0.25);
            assert!(matches!(take_commands().last(), Some(AudioCommand::MasterVolume(v)) if *v == 0.25));
            trace::set_active(false);
            let out = String::from_utf8(trace::take_output()).unwrap();
            let lines: Vec<&str> = out.lines().collect();
            // XNA's sequence: the effect, its master volume, the instance with Volume 1 / Pitch 0 / Pan 0, ...
            assert!(lines[0].starts_with("se.create tone 18:"), "{out}");
            assert_eq!(lines[1], "se.master 3f800000");
            assert!(lines[2].starts_with("si.create si") && lines[2].ends_with(" tone"), "{out}");
            assert!(lines[3].ends_with(" 3f800000 v=3f800000"), "{out}");
            assert!(lines[4].ends_with(" 00000000 r=3f800000"), "{out}");
            assert!(lines[5].ends_with(" 00000000 m=3f800000,3f800000"), "{out}");
            // pan -0.25 on a mono voice: left 1 (clamped 1.25), right 0.75
            assert!(lines[6].ends_with(" be800000 m=3f800000,3f400000"), "{out}");
            assert!(lines[7].ends_with(" 0 11025 -1"), "{out}");
            assert!(lines[8].ends_with(" 1"), "{out}");
        }
    }

    #[test]
    fn wav_stream_and_dynamic_instance() {
        // RIFF/WAVE, PCM 16 bit mono 8000 Hz, 4 samples
        let mut wav = b"RIFF\x2c\0\0\0WAVEfmt \x10\0\0\0\x01\0\x01\0\x40\x1f\0\0\x80\x3e\0\0\x02\0\x10\0data\x08\0\0\0".to_vec();
        wav.extend_from_slice(&[1, 0, 2, 0, 0xFF, 0xFF, 0, 0x80]);
        unsafe {
            let fx = &*SoundEffect::from_stream(Stream::from_vec(wav)).ptr();
            assert_eq!((fx.sound.channels, fx.sound.sample_rate, &fx.sound.samples[..]), (1, 8000, &[1i16, 2, -1, -32768][..]));
            assert_eq!((fx.loop_start, fx.loop_length), (0, 4));
            let e = rt::try_(|| SoundEffect::from_stream(Stream::from_vec(vec![1, 2, 3]))).unwrap_err();
            assert!(rt::exc_is(e, "System.InvalidOperationException"));

            let d = &mut *SoundEffectInstance::new_dynamic(44100, 2).ptr();
            assert_eq!((d.state(), d.pending_buffer_count()), (STATE_STOPPED, 0));
            d.submit_buffer(Arr::from_slice(&[0u8; 44100 * 4]), 0, 44100 * 4);
            assert_eq!(d.pending_buffer_count(), 1);
            assert!((d.pending_seconds() - 1.0).abs() < 1e-9);
            d.play();
            assert_eq!(d.state(), STATE_PLAYING);
            advance(0.75);
            assert!((d.pending_seconds() - 0.25).abs() < 1e-6, "{}", d.pending_seconds());
            advance(0.5);
            assert_eq!(d.pending_buffer_count(), 0);
            let e = rt::try_(|| d.submit_buffer(Arr::from_slice(&[0u8; 6]), 0, 6)).unwrap_err();
            assert!(rt::exc_is(e, "System.ArgumentException"));
        }
        // one MS ADPCM mono block: predictor 0, delta 16, sample1 100, sample2 50, then nibbles 0x1, 0xF
        let block = [0u8, 16, 0, 100, 0, 50, 0, 0x1F];
        let pcm = decode_ms_adpcm(&block, 1, 8);
        // sample2, sample1; predicted (100*256 + 50*0) >> 8 = 100, + 1 * 16 = 116, delta = max(230*16 >> 8, 16) = 16;
        // then predicted 116, nibble 0xF = -1: 116 - 16 = 100
        assert_eq!(pcm, [50, 100, 116, 100]);
    }

    /// XNA's kernel pan law and frequency ratio (Microsoft.Xna.Framework.dll, KernelSoundEffectInstance::SetPan /
    /// SetPitch): the values the reference run recorded from the real kernel code.
    #[test]
    fn output_matrix_and_frequency_ratio_are_the_kernels() {
        use mixer::{frequency_ratio, output_matrix};
        let bits = |m: [f32; 4]| m.map(f32::to_bits);
        // stereo, pan 0: [1, -0 (0 * -0.5), 0, 1]
        assert_eq!(bits(output_matrix(2, 0.0)), [0x3f800000, 0x80000000, 0, 0x3f800000]);
        assert_eq!(output_matrix(2, -1.0), [0.5, 0.5, 0.0, 0.0]);
        assert_eq!(output_matrix(2, 1.0), [0.0, 0.0, 0.5, 0.5]);
        assert_eq!(output_matrix(1, 0.5), [0.5, 1.0, 0.0, 0.0]);
        assert_eq!(output_matrix(1, -1.0), [1.0, 0.0, 0.0, 0.0]);
        assert_eq!(frequency_ratio(0.0), 1.0);
        assert_eq!(frequency_ratio(1.0), 2.0);
        assert_eq!(frequency_ratio(-1.0), 0.5);
    }

    /// The game's wave bank (needs the install; skipped with a note otherwise).
    #[test]
    fn wave_bank_entries_decode() {
        let p = crate::content::install_dir().join("Content").join("Wave Bank.xwb");
        if !p.is_file() {
            eprintln!("skipped: no Terraria install");
            return;
        }
        let bank = WaveBank::open(&p).unwrap();
        assert_eq!(bank.entries.len(), 104);
        assert!(bank.entries.iter().all(|e| e.codec == 2 && e.channels == 2));
        let shortest = (0..bank.entries.len()).min_by_key(|i| bank.entries[*i].length).unwrap();
        let s = bank.decode(shortest).unwrap();
        let e = &bank.entries[shortest];
        // every block of N bytes holds 2 + (N - 14) * 2 / 2 frames (stereo): the decoded length follows from the data size
        let frames_per_block = 2 + (e.block_align as usize - 14);
        assert_eq!(s.frames(), (e.length as usize / e.block_align as usize) * frames_per_block);
        let peak = s.samples.iter().map(|v| v.unsigned_abs()).max().unwrap();
        assert!(peak > 1000, "decoded audio is not silence (peak {peak})");
        eprintln!("wave {shortest}: {} Hz, {:.1} s, peak {peak}", s.sample_rate, s.seconds());
    }
}

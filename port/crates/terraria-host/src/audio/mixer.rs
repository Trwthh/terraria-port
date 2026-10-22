//! The software mixer both builds use: executes the host's [`AudioCommand`]s in order and mixes 44.1 kHz stereo
//! float blocks. No thread, no device, no Bevy: the native client runs it on its mixer thread and hands the blocks
//! to `waveOut` (terraria-app `host/audio.rs`); the browser build runs it inside `tw_frame` and hands the frames to
//! the AudioWorklet (`tw.audio_push`, terraria-web). Moved here from terraria-app unchanged, except that the voices
//! are summed in voice-id order (a `BTreeMap`; the `HashMap` before summed them in a per-process random order, so
//! with three or more voices the low bits of the float sum differed from run to run and could not match between
//! the two builds).
//!
//! Per voice, the parameters XNA 4 hands XAudio2 (read from XNA's own kernel code, `<Module>.Microsoft.Xna.Framework.
//! Audio.KernelSoundEffectInstance.SetPan / SetPitch / SetVolume` in Microsoft.Xna.Framework.dll, a mixed-mode
//! assembly whose kernel is IL; and checked call by call against the real kernel by the audio gate, VERIFICATION.md
//! "Audio output"): the voice volume (= `Volume`), the output matrix of [`output_matrix`] (mono: left = clamp(1 - pan),
//! right = clamp(1 + pan); stereo: a cross-feed, the far channel's half moves to the near side) and the frequency ratio
//! of [`frequency_ratio`] (`(float)Math.Pow(2, pitch)`). Each output channel is the sum over the source channels of
//! sample * coefficient * volume. Looping: the buffer's loop region. What is NOT XAudio2's: the sample-rate converter
//! (linear interpolation between source frames here; XAudio2's SRC filter is not public) and the 10 ms processing
//! quantum (blocks of [`BLOCK_FRAMES`] here).
//!
//! Summing and full scale: the voices are summed in 32-bit float and not limited (the original's XAudio2 hands its
//! float mix to the system's audio engine); [`soft_limit`] is the 16-bit fallback's output stage. Blocks are
//! [`BLOCK_FRAMES`] frames: commands take effect at block boundaries, so a consumer that wants other sizes reads
//! through [`BlockReader`], which keeps the block structure (and so the samples) of the native thread.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};
use std::sync::Arc;

use super::{AudioCommand, Sound};

/// Output rate (frames per second).
pub const RATE: u32 = 44100;
/// Frames per mixed block.
pub const BLOCK_FRAMES: usize = 1024;

#[derive(Default)]
pub struct AudioStats {
    pub blocks_written: AtomicU64,
    /// largest sample of the float mix, in 16-bit units (32,767 = full scale; more than that: the sum of the voices
    /// exceeded full scale and the output stage limited it)
    pub peak: AtomicI32,
    pub voices_started: AtomicU64,
    /// sum of the squares of every sample of the mix (both channels, 16-bit units) and their number:
    /// RMS = sqrt(sum_sq / samples)
    pub sum_sq: AtomicU64,
    pub samples: AtomicU64,
    /// samples of the mix above full scale, and blocks that had at least one
    pub over_samples: AtomicU64,
    pub over_blocks: AtomicU64,
    /// most voices mixed into one block
    pub max_voices: AtomicI32,
    /// output format: 0 none (muted), 1 32-bit float, 2 16-bit PCM
    pub format: AtomicI32,
    /// 1: a device is open, 0: muted or no device
    pub device_open: AtomicI32,
}

impl AudioStats {
    /// RMS (in 16-bit units) of everything the mixer wrote so far, silence included.
    pub fn rms(&self) -> f64 {
        let n = self.samples.load(Ordering::Relaxed);
        if n == 0 {
            0.0
        } else {
            (self.sum_sq.load(Ordering::Relaxed) as f64 / n as f64).sqrt()
        }
    }
    /// One block of the float mix (1.0 = full scale), played by `voices` voices.
    pub fn account(&self, block: &[f32], voices: usize) {
        let (mut sq, mut peak, mut over) = (0f64, 0f32, 0u64);
        for s in block {
            let a = s.abs();
            sq += (a as f64 * 32767.0) * (a as f64 * 32767.0);
            peak = peak.max(a);
            over += (a > 1.0) as u64;
        }
        self.sum_sq.fetch_add(sq as u64, Ordering::Relaxed);
        self.samples.fetch_add(block.len() as u64, Ordering::Relaxed);
        self.peak.fetch_max((peak * 32767.0).min(i32::MAX as f32) as i32, Ordering::Relaxed);
        self.over_samples.fetch_add(over, Ordering::Relaxed);
        self.over_blocks.fetch_add((over > 0) as u64, Ordering::Relaxed);
        self.max_voices.fetch_max(voices as i32, Ordering::Relaxed);
        self.blocks_written.fetch_add(1, Ordering::Relaxed);
    }
    /// One line for a log.
    pub fn summary(&self) -> String {
        let l = |a: &AtomicU64| a.load(Ordering::Relaxed);
        let peak = self.peak.load(Ordering::Relaxed);
        let db = |x: f64| if x <= 0.0 { f64::NEG_INFINITY } else { 20.0 * (x / 32767.0).log10() };
        format!(
            "output {}, {} blocks written, {} voices started, at most {} at once; mix peak {peak} of 32767 ({:+.1} dBFS), RMS {:.1} ({:+.1} dBFS); {} of {} samples above full scale ({} blocks): limited by {}",
            match self.format.load(Ordering::Relaxed) {
                1 => "32-bit float",
                2 => "16-bit PCM",
                _ => "none (muted)",
            },
            l(&self.blocks_written),
            l(&self.voices_started),
            self.max_voices.load(Ordering::Relaxed),
            db(peak as f64),
            self.rms(),
            db(self.rms()),
            l(&self.over_samples),
            l(&self.samples),
            l(&self.over_blocks),
            match self.format.load(Ordering::Relaxed) {
                1 => "the system's audio engine, as the original's float mix",
                2 => "the mixer's soft knee",
                _ => "nothing (no output)",
            }
        )
    }
}

enum Source {
    Sound(Arc<Sound>),
    Stream { channels: u16, sample_rate: u32, samples: Vec<i16> },
}

struct Voice {
    source: Source,
    /// position in source frames
    pos: f64,
    volume: f32,
    pan: f32,
    pitch: f32,
    looped: bool,
    paused: bool,
}

/// XAudio2's output matrix for a voice of `channels` source channels at `pan`, as XNA 4's
/// `KernelSoundEffectInstance::SetPan` computes it, laid out as XAudio2 wants it: coefficient `[dst * src + s]`
/// (mono: `[L, R]`; stereo: `[L<-L, L<-R, R<-L, R<-R]`). The arithmetic is the kernel's: double for the clamped
/// mono terms and the stereo terms with 1.0, float for the rest (so `0 * -0.5` keeps its sign).
pub fn output_matrix(channels: u16, pan: f32) -> [f32; 4] {
    let clamp01 = |d: f64| if d < 0.0 { 0.0 } else if d > 1.0 { 1.0 } else { d as f32 };
    match channels {
        1 => [clamp01(1.0 - pan as f64), clamp01(pan as f64 + 1.0), 0.0, 0.0],
        2 => {
            if -1.0 <= pan as f64 && pan <= 0.0 {
                [(pan as f64 * 0.5 + 1.0) as f32, pan * -0.5, 0.0, pan + 1.0]
            } else {
                [(1.0 - pan as f64) as f32, 0.0, pan * 0.5, (1.0 - pan as f64 * 0.5) as f32]
            }
        }
        // the kernel leaves the matrix as it was for other channel counts (never the case for the game's sounds)
        _ => [1.0, 0.0, 0.0, 1.0],
    }
}

/// XAudio2's frequency ratio for `pitch` (octaves), as `KernelSoundEffectInstance::SetPitch` computes it:
/// `(float)Math.Pow(2.0, pitch * 12f / 12f)` (`pitch * 12 / 12` is exact in the JIT's double arithmetic).
pub fn frequency_ratio(pitch: f32) -> f32 {
    2f64.powf(pitch as f64 * 12.0 / 12.0) as f32
}

pub struct Mixer {
    voices: BTreeMap<u64, Voice>,
    master: f32,
    /// extra output gain (`TERRARIA_VOLUME`), 0..1
    pub gain: f32,
}

impl Mixer {
    pub fn new(gain: f32) -> Mixer {
        Mixer { voices: BTreeMap::new(), master: 1.0, gain }
    }

    pub fn apply(&mut self, c: AudioCommand, stats: &AudioStats) {
        match c {
            AudioCommand::Play { voice, sound, volume, pan, pitch, looped } => {
                stats.voices_started.fetch_add(1, Ordering::Relaxed);
                self.voices.insert(voice, Voice { source: Source::Sound(sound), pos: 0.0, volume, pan, pitch, looped, paused: false });
            }
            AudioCommand::Pause { voice } => {
                if let Some(v) = self.voices.get_mut(&voice) {
                    v.paused = true;
                }
            }
            AudioCommand::Resume { voice } => {
                if let Some(v) = self.voices.get_mut(&voice) {
                    v.paused = false;
                }
            }
            AudioCommand::Stop { voice } => {
                self.voices.remove(&voice);
            }
            AudioCommand::SetVolume { voice, volume } => {
                if let Some(v) = self.voices.get_mut(&voice) {
                    v.volume = volume;
                }
            }
            AudioCommand::SetPan { voice, pan } => {
                if let Some(v) = self.voices.get_mut(&voice) {
                    v.pan = pan;
                }
            }
            AudioCommand::SetPitch { voice, pitch } => {
                if let Some(v) = self.voices.get_mut(&voice) {
                    v.pitch = pitch;
                }
            }
            AudioCommand::MasterVolume(v) => self.master = v,
            AudioCommand::StreamOpen { voice, channels, sample_rate } => {
                self.voices.insert(voice, Voice { source: Source::Stream { channels, sample_rate, samples: Vec::new() }, pos: 0.0, volume: 1.0, pan: 0.0, pitch: 0.0, looped: false, paused: true });
            }
            AudioCommand::StreamData { voice, samples } => {
                if let Some(Voice { source: Source::Stream { samples: queue, .. }, .. }) = self.voices.get_mut(&voice) {
                    queue.extend_from_slice(&samples);
                }
            }
        }
    }

    /// One block of interleaved stereo samples, summed in float (1.0 = full scale, not limited). Returns the
    /// number of voices that played.
    pub fn mix(&mut self, acc: &mut [f32]) -> usize {
        acc.fill(0.0);
        let mut finished = Vec::new();
        let mut playing = 0;
        for (id, v) in self.voices.iter_mut() {
            if v.paused {
                continue;
            }
            playing += 1;
            let (data, channels, rate, loop_range): (&[i16], usize, u32, Option<(usize, usize)>) = match &v.source {
                Source::Sound(s) => {
                    let frames = s.frames();
                    let (ls, ll) = (s.loop_start as usize, s.loop_length as usize);
                    let range = if v.looped { Some(if ll > 0 && ls + ll <= frames { (ls, ls + ll) } else { (0, frames) }) } else { None };
                    (&s.samples, s.channels.max(1) as usize, s.sample_rate, range)
                }
                Source::Stream { channels, sample_rate, samples } => (samples, (*channels).max(1) as usize, *sample_rate, None),
            };
            let frames = data.len() / channels;
            let step = rate as f64 / RATE as f64 * frequency_ratio(v.pitch) as f64;
            let m = output_matrix(channels as u16, v.pan);
            let gain = v.volume * self.master * self.gain / 32768.0;
            let at = |f: usize, c: usize| data.get(f * channels + c.min(channels - 1)).copied().unwrap_or(0) as f32;
            for frame in acc.chunks_exact_mut(2) {
                let mut i = v.pos as usize;
                if let Some((start, end)) = loop_range {
                    if end > start && i >= end {
                        v.pos = start as f64 + (v.pos - end as f64) % (end - start) as f64;
                        i = v.pos as usize;
                    }
                }
                if i >= frames {
                    if matches!(v.source, Source::Sound(_)) {
                        finished.push(*id);
                    }
                    break;
                }
                let t = (v.pos - i as f64) as f32;
                let next = if i + 1 < frames { i + 1 } else { i };
                let l = at(i, 0) + (at(next, 0) - at(i, 0)) * t;
                if channels == 1 {
                    frame[0] += l * gain * m[0];
                    frame[1] += l * gain * m[1];
                } else {
                    let r = at(i, 1) + (at(next, 1) - at(i, 1)) * t;
                    frame[0] += (l * m[0] + r * m[1]) * gain;
                    frame[1] += (l * m[2] + r * m[3]) * gain;
                }
                v.pos += step;
            }
            // drop the consumed part of a stream so its queue does not grow without bound
            if let Source::Stream { samples, .. } = &mut v.source {
                let used = (v.pos as usize).min(samples.len() / channels);
                samples.drain(..used * channels);
                v.pos -= used as f64;
            }
        }
        for id in finished {
            self.voices.remove(&id);
        }
        playing
    }
}

/// Reads the mix in any number of frames while mixing in [`BLOCK_FRAMES`] blocks, as the native thread does: the
/// samples are the same whatever sizes the consumer asks for (commands applied between reads take effect at the
/// next block, as they do between the native thread's blocks).
pub struct BlockReader {
    pub mixer: Mixer,
    block: Vec<f32>,
    /// samples of `block` already read
    used: usize,
}

impl BlockReader {
    pub fn new(mixer: Mixer) -> BlockReader {
        BlockReader { mixer, block: vec![0f32; BLOCK_FRAMES * 2], used: BLOCK_FRAMES * 2 }
    }
    /// Fills `out` (interleaved stereo, any even length); every block mixed is accounted in `stats`.
    pub fn read(&mut self, out: &mut [f32], stats: &AudioStats) {
        let mut o = 0;
        while o < out.len() {
            if self.used == self.block.len() {
                let voices = self.mixer.mix(&mut self.block);
                stats.account(&self.block, voices);
                self.used = 0;
            }
            let n = (out.len() - o).min(self.block.len() - self.used);
            out[o..o + n].copy_from_slice(&self.block[self.used..self.used + n]);
            o += n;
            self.used += n;
        }
    }
}

/// The 16-bit fallback's output stage: linear up to -3 dBFS, then a knee that approaches full scale and never
/// reaches it (`x` and the result: 1.0 = full scale). A hard clamp would turn every overshoot into a square edge.
pub fn soft_limit(x: f32) -> f32 {
    const KNEE: f32 = 0.707_945_8;
    let a = x.abs();
    if a <= KNEE {
        return x;
    }
    let over = (a - KNEE) / (1.0 - KNEE);
    (KNEE + (1.0 - KNEE) * (over / (1.0 + over))).copysign(x)
}

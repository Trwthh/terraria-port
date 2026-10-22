//! Audio backend for `terraria_host::audio`: a small software mixer on its own thread, writing 44.1 kHz stereo
//! 16-bit blocks to the default output device through Windows' `waveOut` API (winmm, declared here: no audio crate
//! and no Bevy audio feature is needed).
//!
//! The mixing itself (commands, voices, the float sum, [`AudioStats`], [`soft_limit`]) is
//! `terraria_host::audio::mixer`, without a thread or a device, so that the browser build (terraria-web) mixes with
//! the same code; this file is the thread, the device and the environment variables.
//!
//! The mixer executes the host's [`AudioCommand`]s in order. Per voice: volume (linear), pitch (playback rate
//! 2^pitch, linear interpolation between source frames), pan and looping.
//! Pan law and pitch: XNA's own (its kernel's SetPan / SetPitch, `terraria_host::audio::mixer`; checked against the
//! real kernel by the audio gate). Before 2026-10-15: the one FNA uses for XNA's `SoundEffectInstance.Pan` (mono: left = 1 - max(pan, 0), right = 1 +
//! min(pan, 0); stereo: the opposite channel is attenuated the same way). XNA's own native mixer was not
//! decompiled for this; treat the law as unverified.
//!
//! Summing and full scale. The original mixes in XAudio2 2.7 (XNA 4's `SoundEffect` and XACT both sit on it):
//! voices are summed in 32-bit float and the mastering voice hands the float mix to the Windows audio engine,
//! whose limiter deals with what exceeds full scale. Nothing in XNA or in the game clamps or limits the sum; what
//! bounds the number of voices is the game's own code (`LegacySoundPlayer` keeps one `SoundEffectInstance` per sound
//! id and stops it before it plays again; `SoundPlayer` for tracked sounds), which is translated and runs here.
//! So this mixer sums in float and opens the device with a 32-bit float format: the same engine and the same
//! limiter treat the sum as they treat the original's. Only when the device refuses float is the mix written as
//! 16-bit PCM, through a soft knee above -3 dBFS instead of a hard clamp ([`soft_limit`]; the statistics say which).
//!
//! `TERRARIA_MUTE=1`: mix but do not open a device. `TERRARIA_VOLUME=<0..1>`: extra output gain.
//! `TERRARIA_VOLUME_FILE=<file>`: the output gain is read from this file five times a second (a number 0..1; no
//! file or no number: 0), so a test can open the device silent and let sound out for a bounded time.
//! The mixer reports what it did through [`AudioStats`] (blocks written, peak and RMS of the float mix, samples
//! above full scale, most voices at once), which is how a headless run shows what sound was produced.

use std::sync::atomic::Ordering;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

use terraria_host::audio::mixer::{Mixer, BLOCK_FRAMES, RATE};
use terraria_host::audio::AudioCommand;
// the mixer itself is in terraria-host (`audio::mixer`, no thread and no device: the browser build runs it too)
pub use terraria_host::audio::mixer::{soft_limit, AudioStats};

const BLOCKS: usize = 4;

pub struct AudioBackend {
    tx: Sender<AudioCommand>,
    pub stats: Arc<AudioStats>,
}

impl AudioBackend {
    pub fn start() -> AudioBackend {
        let (tx, rx) = channel();
        let stats = Arc::new(AudioStats::default());
        let s2 = stats.clone();
        std::thread::Builder::new().name("terraria audio mixer".into()).spawn(move || mixer_thread(rx, s2)).expect("spawn the mixer thread");
        AudioBackend { tx, stats }
    }
    pub fn submit(&self, commands: Vec<AudioCommand>) {
        for c in commands {
            let _ = self.tx.send(c);
        }
    }
}

#[repr(C)]
struct WaveFormatEx {
    format_tag: u16,
    channels: u16,
    samples_per_sec: u32,
    avg_bytes_per_sec: u32,
    block_align: u16,
    bits_per_sample: u16,
    cb_size: u16,
}

#[repr(C)]
struct WaveHdr {
    data: *mut u8,
    buffer_length: u32,
    bytes_recorded: u32,
    user: usize,
    flags: u32,
    loops: u32,
    next: *mut WaveHdr,
    reserved: usize,
}

const WHDR_DONE: u32 = 1;
const WAVE_MAPPER: u32 = 0xFFFF_FFFF;

#[cfg(windows)]
#[link(name = "winmm")]
extern "system" {
    fn waveOutOpen(handle: *mut isize, device: u32, format: *const WaveFormatEx, callback: usize, instance: usize, flags: u32) -> u32;
    fn waveOutPrepareHeader(handle: isize, header: *mut WaveHdr, size: u32) -> u32;
    fn waveOutWrite(handle: isize, header: *mut WaveHdr, size: u32) -> u32;
    fn waveOutReset(handle: isize) -> u32;
    fn waveOutClose(handle: isize) -> u32;
}

fn mixer_thread(rx: Receiver<AudioCommand>, stats: Arc<AudioStats>) {
    let gain = std::env::var("TERRARIA_VOLUME").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(1.0).clamp(0.0, 1.0);
    let volume_file = std::env::var_os("TERRARIA_VOLUME_FILE").filter(|v| !v.is_empty()).map(std::path::PathBuf::from);
    let gain = if volume_file.is_some() { 0.0 } else { gain };
    let mut volume_polled = std::time::Instant::now();
    let mut mixer = Mixer::new(gain);
    let muted = std::env::var_os("TERRARIA_MUTE").is_some();
    let mut handle: isize = 0;
    // 1: 32-bit float (WAVE_FORMAT_IEEE_FLOAT), 2: 16-bit PCM
    let mut format = 0;
    #[cfg(windows)]
    if !muted {
        let float = WaveFormatEx { format_tag: 3, channels: 2, samples_per_sec: RATE, avg_bytes_per_sec: RATE * 8, block_align: 8, bits_per_sample: 32, cb_size: 0 };
        let pcm = WaveFormatEx { format_tag: 1, channels: 2, samples_per_sec: RATE, avg_bytes_per_sec: RATE * 4, block_align: 4, bits_per_sample: 16, cb_size: 0 };
        let force_pcm = std::env::var_os("TERRARIA_AUDIO_PCM16").is_some_and(|v| !v.is_empty() && v != "0");
        // SAFETY: plain winmm calls with valid pointers; CALLBACK_NULL.
        unsafe {
            if !force_pcm && waveOutOpen(&mut handle, WAVE_MAPPER, &float, 0, 0, 0) == 0 {
                format = 1;
            } else {
                let r = waveOutOpen(&mut handle, WAVE_MAPPER, &pcm, 0, 0, 0);
                if r == 0 {
                    format = 2;
                } else {
                    eprintln!("[audio] waveOutOpen failed ({r}): no sound output");
                    handle = 0;
                }
            }
        }
    }
    stats.device_open.store((handle != 0) as i32, Ordering::Relaxed);
    stats.format.store(format, Ordering::Relaxed);
    let mut mix = vec![0f32; BLOCK_FRAMES * 2];
    // device buffers: 8 bytes per frame as float, 4 as PCM (u32 storage keeps them aligned for both)
    let bytes = BLOCK_FRAMES * if format == 1 { 8 } else { 4 };
    let mut buffers: Vec<Vec<u32>> = (0..BLOCKS).map(|_| vec![0u32; bytes / 4]).collect();
    let mut headers: Vec<WaveHdr> = buffers
        .iter_mut()
        .map(|b| WaveHdr { data: b.as_mut_ptr() as *mut u8, buffer_length: bytes as u32, bytes_recorded: 0, user: 0, flags: WHDR_DONE, loops: 0, next: std::ptr::null_mut(), reserved: 0 })
        .collect();
    let block_time = std::time::Duration::from_secs_f64(BLOCK_FRAMES as f64 / RATE as f64);
    let mut next_silent_block = std::time::Instant::now();
    'run: loop {
        loop {
            match rx.try_recv() {
                Ok(c) => mixer.apply(c, &stats),
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => break 'run,
            }
        }
        if let Some(p) = volume_file.as_ref() {
            if volume_polled.elapsed() >= std::time::Duration::from_millis(200) {
                volume_polled = std::time::Instant::now();
                let g = std::fs::read_to_string(p).ok().and_then(|t| t.trim().parse::<f32>().ok()).unwrap_or(0.0).clamp(0.0, 1.0);
                if g != mixer.gain {
                    eprintln!("[audio] TERRARIA_VOLUME_FILE: output gain {g}");
                    mixer.gain = g;
                }
            }
        }
        let mut wrote = false;
        if handle != 0 {
            #[cfg(windows)]
            for i in 0..BLOCKS {
                if headers[i].flags & WHDR_DONE == 0 {
                    continue;
                }
                let voices = mixer.mix(&mut mix);
                stats.account(&mix, voices);
                // SAFETY: the buffer holds `bytes` bytes: BLOCK_FRAMES * 2 samples of the open format.
                unsafe {
                    if format == 1 {
                        core::slice::from_raw_parts_mut(buffers[i].as_mut_ptr() as *mut f32, mix.len()).copy_from_slice(&mix);
                    } else {
                        for (o, a) in core::slice::from_raw_parts_mut(buffers[i].as_mut_ptr() as *mut i16, mix.len()).iter_mut().zip(&mix) {
                            *o = (soft_limit(*a) * 32767.0) as i16;
                        }
                    }
                }
                headers[i].flags = 0;
                // SAFETY: the header and its buffer live as long as this thread; the device was opened above.
                unsafe {
                    waveOutPrepareHeader(handle, &mut headers[i], size_of::<WaveHdr>() as u32);
                    waveOutWrite(handle, &mut headers[i], size_of::<WaveHdr>() as u32);
                }
                wrote = true;
            }
        } else if std::time::Instant::now() >= next_silent_block {
            // no device: keep consuming at real-time rate so voices end when they would
            let voices = mixer.mix(&mut mix);
            stats.account(&mix, voices);
            next_silent_block += block_time;
            wrote = true;
        }
        if !wrote {
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }
    #[cfg(windows)]
    if handle != 0 {
        // SAFETY: the handle is ours.
        unsafe {
            waveOutReset(handle);
            waveOutClose(handle);
        }
    }
    let _ = &mut headers;
}

#[cfg(test)]
mod tests {
    use super::*;
    use terraria_host::audio::Sound;

    #[test]
    fn soft_limit_is_linear_below_the_knee_and_never_reaches_full_scale() {
        for x in [0.0f32, 0.1, -0.5, 0.7] {
            assert_eq!(soft_limit(x), x);
        }
        let mut last = 0.7f32;
        for i in 0..2000 {
            let x = 0.71 + i as f32 * 0.01;
            let y = soft_limit(x);
            assert!(y > last && y < 1.0, "{x} -> {y}");
            assert_eq!(soft_limit(-x), -y);
            last = y;
        }
    }

    /// Two full-scale voices: the mix is their sum (2.0), not a clamped 1.0, and the statistics say so.
    #[test]
    fn the_mix_is_a_float_sum_and_the_statistics_count_what_exceeds_full_scale() {
        let sound = Arc::new(Sound { name: String::new(), samples: vec![i16::MAX; 4096], channels: 1, sample_rate: RATE, loop_start: 0, loop_length: 0 });
        let stats = AudioStats::default();
        let mut m = Mixer::new(1.0);
        for voice in 0..2 {
            m.apply(AudioCommand::Play { voice, sound: sound.clone(), volume: 1.0, pan: 0.0, pitch: 0.0, looped: false }, &stats);
        }
        let mut block = vec![0f32; 512];
        let voices = m.mix(&mut block);
        stats.account(&block, voices);
        assert_eq!(voices, 2);
        assert!(block.iter().all(|s| (*s - 2.0).abs() < 1e-3), "{:?}", &block[..4]);
        assert_eq!(stats.over_samples.load(Ordering::Relaxed), 512);
        assert_eq!(stats.max_voices.load(Ordering::Relaxed), 2);
        assert!(stats.peak.load(Ordering::Relaxed) > 65000);
        assert!(stats.summary().contains("512 of 512 samples above full scale"));
    }

    /// The mixer as it was in this file before it moved to terraria-host (`audio::mixer`), kept as the reference
    /// for the move. Copied unchanged except for the voice map: `BTreeMap` (voice-id order) as in the moved mixer,
    /// because the `HashMap` summed voices in a per-process random order (its seed), so not even two runs of the
    /// old code were bit-equal with three or more voices.
    mod reference {
        use std::collections::BTreeMap;
        use std::sync::atomic::Ordering;
        use std::sync::Arc;

        use super::{AudioStats, RATE};
        use terraria_host::audio::{AudioCommand, Sound};

        pub enum Source {
            Sound(Arc<Sound>),
            Stream { channels: u16, sample_rate: u32, samples: Vec<i16> },
        }

        pub struct Voice {
            source: Source,
            /// position in source frames
            pos: f64,
            volume: f32,
            pan: f32,
            pitch: f32,
            looped: bool,
            paused: bool,
        }

        pub struct Mixer {
            pub voices: BTreeMap<u64, Voice>,
            pub master: f32,
            pub gain: f32,
        }

        impl Mixer {
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
                    // since 2026-10-15 the mixer uses XNA's own pan law and frequency ratio (the kernel's,
                    // terraria_host::audio::mixer::output_matrix / frequency_ratio); this copy follows it so that it
                    // still proves the move and the browser build's block reading
                    let step = rate as f64 / RATE as f64 * terraria_host::audio::mixer::frequency_ratio(v.pitch) as f64;
                    let m = terraria_host::audio::mixer::output_matrix(channels as u16, v.pan);
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
    }

    /// Deterministic test material (an LCG; no files): `frames` frames of `channels` channels.
    fn material(name: &str, frames: usize, channels: u16, sample_rate: u32, seed: u32, loop_start: u32, loop_length: u32) -> Arc<Sound> {
        let mut x = seed;
        let mut samples = Vec::with_capacity(frames * channels as usize);
        for i in 0..frames * channels as usize {
            x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            // a tone plus noise, near full scale at times
            let tone = ((i as f32 * 0.031).sin() * 22000.0) as i32;
            samples.push((tone + ((x >> 16) as i32 - 32768) / 3).clamp(-32768, 32767) as i16);
        }
        Arc::new(Sound { name: name.into(), samples, channels, sample_rate, loop_start, loop_length })
    }

    /// The commands applied before block `k` of a fixed sequence that uses every command kind: looped and one-shot
    /// sounds, mono and stereo, 22,050 / 44,100 / 48,000 Hz sources, pitch, pan, volume, pause / resume, master
    /// volume, a stream fed in parts, stops, voices ending on their own, up to six voices at once.
    fn commands(k: usize) -> Vec<AudioCommand> {
        use AudioCommand::*;
        let music = material("music", 30_000, 2, 44100, 1, 4_000, 20_000);
        let mono = material("mono", 9_000, 1, 22050, 2, 0, 0);
        let hi = material("hi", 5_000, 1, 48000, 3, 0, 0);
        let st = material("st", 3_000, 2, 44100, 4, 0, 0);
        let stream: Arc<[i16]> = material("stream", 6_000, 2, 44100, 5, 0, 0).samples.clone().into();
        match k {
            0 => vec![MasterVolume(0.9), Play { voice: 1, sound: music, volume: 0.8, pan: 0.0, pitch: 0.0, looped: true }],
            3 => vec![Play { voice: 2, sound: mono.clone(), volume: 1.0, pan: -0.4, pitch: 0.0, looped: false }, Play { voice: 3, sound: hi.clone(), volume: 0.7, pan: 0.6, pitch: 0.25, looped: false }],
            4 => vec![StreamOpen { voice: 10, channels: 2, sample_rate: 44100 }, StreamData { voice: 10, samples: stream.clone() }, Resume { voice: 10 }],
            5 => vec![Play { voice: 4, sound: st.clone(), volume: 0.5, pan: 0.3, pitch: -0.5, looped: true }, Play { voice: 5, sound: mono.clone(), volume: 0.9, pan: 1.0, pitch: 0.7, looped: false }, SetPitch { voice: 1, pitch: 0.1 }],
            7 => vec![SetVolume { voice: 4, volume: 1.0 }, SetPan { voice: 1, pan: -1.0 }, Pause { voice: 3 }, StreamData { voice: 10, samples: stream.clone() }],
            9 => vec![Resume { voice: 3 }, Play { voice: 6, sound: hi, volume: 1.0, pan: 0.0, pitch: -1.0, looped: true }, MasterVolume(0.6)],
            12 => vec![Stop { voice: 4 }, Pause { voice: 10 }, Play { voice: 2, sound: st, volume: 1.0, pan: 0.0, pitch: 0.0, looped: false }],
            14 => vec![Resume { voice: 10 }, StreamData { voice: 10, samples: stream }, SetPitch { voice: 6, pitch: 0.9 }],
            20 => vec![Stop { voice: 6 }, Stop { voice: 99 }, SetVolume { voice: 1, volume: 0.3 }],
            _ => Vec::new(),
        }
    }

    /// The moved mixer gives the old one's samples bit for bit, block by block, and so does the browser build's way
    /// of reading it ([`BlockReader`] in sizes that are not blocks); the statistics agree too.
    #[test]
    fn the_moved_mixer_is_bit_equal_to_the_native_one() {
        use terraria_host::audio::mixer::BlockReader;
        const N: usize = 48;
        let (s_old, s_new, s_web) = (AudioStats::default(), AudioStats::default(), AudioStats::default());
        let mut old = reference::Mixer { voices: std::collections::BTreeMap::new(), master: 1.0, gain: 0.8 };
        let mut new = Mixer::new(0.8);
        let mut web = BlockReader::new(Mixer::new(0.8));
        let (mut a, mut b) = (vec![0f32; BLOCK_FRAMES * 2], vec![0f32; BLOCK_FRAMES * 2]);
        let (mut all_old, mut all_web) = (Vec::new(), vec![0f32; N * BLOCK_FRAMES * 2]);
        let sizes = [735usize, 128, 1, 160];
        let mut at = 0;
        let mut size = 0;
        for k in 0..N {
            for c in commands(k) {
                old.apply(c.clone(), &s_old);
                new.apply(c.clone(), &s_new);
                web.mixer.apply(c, &s_web);
            }
            let (vo, vn) = (old.mix(&mut a), new.mix(&mut b));
            s_old.account(&a, vo);
            s_new.account(&b, vn);
            assert_eq!(vo, vn, "block {k}: voices");
            for (i, (x, y)) in a.iter().zip(&b).enumerate() {
                assert_eq!(x.to_bits(), y.to_bits(), "block {k} sample {i}: {x} vs {y}");
            }
            all_old.extend_from_slice(&a);
            // the browser side: block k's frames in odd-sized reads (the commands of block k + 1 come after all of them)
            let end = (k + 1) * BLOCK_FRAMES * 2;
            while at < end {
                let n = (sizes[size % sizes.len()] * 2).min(end - at);
                size += 1;
                web.read(&mut all_web[at..at + n], &s_web);
                at += n;
            }
        }
        assert!(all_old.iter().any(|s| s.abs() > 0.5), "the sequence is not silent");
        for (i, (x, y)) in all_old.iter().zip(&all_web).enumerate() {
            assert_eq!(x.to_bits(), y.to_bits(), "read sample {i}: {x} vs {y}");
        }
        assert_eq!(s_old.summary(), s_new.summary());
        assert_eq!(s_old.summary(), s_web.summary());
        assert!(s_old.max_voices.load(Ordering::Relaxed) >= 5, "{}", s_old.summary());
    }
}

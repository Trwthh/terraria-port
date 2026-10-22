//! The audio engine behind XNA's managed audio classes: the counterpart of XNA 4's `SoundEffectUnsafeNativeMethods`
//! (Microsoft.Xna.Framework.dll, the calls that reach the `Kernel*` classes and XAudio2) and of the XACT
//! `UnsafeNativeMethods.{Engine, SoundBank, WaveBank, Cue}` (Microsoft.Xna.Framework.Xact.dll, the calls that reach
//! xactengine3_7). The managed layer above it (`super::SoundEffect`, `SoundEffectInstance`, `AudioEngine`, `Cue`, ...)
//! follows XNA's managed code (argument checks, exceptions, `isPacketSubmitted`, the `State` mapping); this layer
//! keeps the engine state, writes the trace line of every call ([`super::trace`]) and hands the mixer its commands.
//!
//! The model is the one `tools/refdump_draw/AudioRec.cs` replaces XNA's native layer with in the reference run, rule
//! for rule (so a run of the original and a run of the port can be compared call by call):
//!   - voice states of the XNA kernel (`KernelSoundEffectInstance`): Playing 1, Stopping 2, Stopped 4, Paused 8. A new
//!     voice is Stopped. Play from Stopped resubmits the buffer when none is queued; Play from Paused continues; Play
//!     from Playing / Stopping does nothing. Stop(immediate) with a queued buffer is Stopping until the next frame
//!     ([`advance_ticks`]), then Stopped with the buffer flushed; without one it is Stopped at once. Pause of a playing
//!     voice is Paused.
//!   - time: every frame a playing voice consumes `ticks * frequency ratio` of source time. A buffer with loop count 0
//!     ends when its frames are consumed (exact integers: sum of ratio * 2^24 * ticks * sample rate against
//!     frames * 10^7 * 2^24). Frames of a buffer: PCM bytes / block align; MS ADPCM (bytes / block align) * samples
//!     per block, as XAudio2 plays whole blocks.
//!   - XACT: a cue from GetCue is Prepared (status 4), Play makes it Playing (8), Pause(1) of a playing cue Paused
//!     (0x40), Pause(0) of a paused cue Playing, Stop Stopped (0x20). A wave bank is Prepared (4). A cue's wave
//!     plays as an internal voice (not traced); when it runs out the cue reads Stopped here, and the run counts it
//!     ([`cues_ran_out`]): the reference model never ends a cue, so the comparison requires that count to stay 0.
//!
//! Derived XAudio2 parameters (written on the trace line, used by the mixer): the voice volume (= Volume), the output
//! matrix ([`super::mixer::output_matrix`]: XNA's `KernelSoundEffectInstance::SetPan`) and the frequency ratio
//! ([`super::mixer::frequency_ratio`]: `KernelSoundEffectInstance::SetPitch`). The reference writes the values XNA's
//! real kernel functions computed, so equal lines mean the mixer gets XNA's parameters.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::Arc;

use super::{mixer, trace, AudioCommand, Sound};

pub const VS_PLAYING: i32 = 1;
pub const VS_STOPPING: i32 = 2;
pub const VS_STOPPED: i32 = 4;
pub const VS_PAUSED: i32 = 8;
/// XACT cue status bits (`XACT_CUESTATE_*`)
pub const CUE_PREPARED: u32 = 4;
pub const CUE_PLAYING: u32 = 8;
pub const CUE_STOPPED: u32 = 0x20;
pub const CUE_PAUSED: u32 = 0x40;
/// `E_INVALIDARG`, what the native layer answers for an unknown handle
pub const E_INVALIDARG: i32 = -2147024809;

const TICKS_SCALE: u128 = 10_000_000u128 << 24;

struct Effect {
    label: String,
    sound: Arc<Sound>,
    frames: u64,
}

struct Voice {
    /// trace index (`si<k>`; creation order of the instances the game made), None for internal voices
    k: Option<u32>,
    sound: Option<Arc<Sound>>,
    channels: u16,
    rate: u32,
    frames: u64,
    state: i32,
    queued: bool,
    consumed: u128,
    loop_count: i32,
    ratio24: u64,
    dynamic: bool,
    buffers: VecDeque<u64>,
    volume: f32,
    pan: f32,
    pitch: f32,
}

struct CueM {
    k: u32,
    bits: u32,
    vars: HashMap<String, f32>,
    /// the internal voice that plays the cue's wave (the managed layer sets it)
    voice: Option<u32>,
}

#[derive(Default)]
struct Engine {
    effects: HashMap<u32, Effect>,
    voices: BTreeMap<u32, Voice>,
    cues: HashMap<u32, CueM>,
    next_handle: u32,
    next_voice: u32,
    next_cue: u32,
    master: Option<f32>,
    categories: Vec<String>,
    globals: HashMap<String, f32>,
    cues_ran_out: u64,
    commands: Vec<AudioCommand>,
}

thread_local! {
    static E: RefCell<Engine> = RefCell::new(Engine::default());
}

fn with<R>(f: impl FnOnce(&mut Engine) -> R) -> R {
    E.with(|e| f(&mut e.borrow_mut()))
}
fn handle(e: &mut Engine) -> u32 {
    e.next_handle += 1;
    e.next_handle
}
fn push(e: &mut Engine, c: AudioCommand) {
    e.commands.push(c);
}
fn si(k: Option<u32>) -> String {
    match k {
        Some(k) => format!("si{k}"),
        None => "si?".into(),
    }
}

/// Backend: the mixer commands recorded since the last call, oldest first.
pub fn take_commands() -> Vec<AudioCommand> {
    with(|e| std::mem::take(&mut e.commands))
}
/// Voices that are playing (tests, diagnostics).
pub fn playing_voices() -> usize {
    with(|e| e.voices.values().filter(|v| v.state == VS_PLAYING).count())
}
/// XACT cues whose wave ran out by itself (the reference model has no such event; see the module docs).
pub fn cues_ran_out() -> u64 {
    with(|e| e.cues_ran_out)
}

/// Frames XAudio2 plays from `data_len` bytes of a buffer in `format` (a WAVEFORMATEX).
pub fn frames_of(format: &[u8], data_len: usize) -> u64 {
    let u16at = |o: usize| format.get(o..o + 2).map_or(0, |b| u16::from_le_bytes([b[0], b[1]]) as u64);
    let (tag, align) = (u16at(0), u16at(12));
    if align == 0 {
        return 0;
    }
    if tag == 2 && format.len() >= 20 {
        return (data_len as u64 / align) * u16at(18);
    }
    data_len as u64 / align
}

/// One frame of time (`ticks` of 100 ns) for every voice (AudioRec.cs `Advance`).
pub fn advance_ticks(ticks: i64) {
    let ticks = ticks.max(0) as u128;
    with(|e| {
        for v in e.voices.values_mut() {
            if v.state == VS_STOPPING {
                v.state = VS_STOPPED;
                v.queued = false;
                v.consumed = 0;
                v.buffers.clear();
                continue;
            }
            if v.state != VS_PLAYING {
                continue;
            }
            if v.dynamic {
                if v.buffers.is_empty() {
                    continue;
                }
                v.consumed += v.ratio24 as u128 * ticks * v.rate as u128;
                while let Some(&b) = v.buffers.front() {
                    if v.consumed < b as u128 * TICKS_SCALE {
                        break;
                    }
                    v.consumed -= b as u128 * TICKS_SCALE;
                    v.buffers.pop_front();
                }
                if v.buffers.is_empty() {
                    v.consumed = 0;
                }
                continue;
            }
            if !v.queued || v.loop_count != 0 || v.sound.is_none() {
                continue;
            }
            v.consumed += v.ratio24 as u128 * ticks * v.rate as u128;
            if v.consumed >= v.frames as u128 * TICKS_SCALE {
                v.state = VS_STOPPED;
                v.queued = false;
                v.consumed = 0;
            }
        }
    });
}

// ------------------------------------------------------------------------------------------------ SoundEffectUnsafeNativeMethods

/// `CreateSoundEffectHandle(format, data)`: `label` is the content asset below Content/ or "stream".
pub fn create_sound_effect(label: &str, format: &[u8], data: &[u8], sound: Arc<Sound>) -> u32 {
    let frames = frames_of(format, data.len());
    trace::line(|| format!("se.create {label} {} {} {frames}", trace::bytes(format), trace::bytes(data)));
    with(|e| {
        let h = handle(e);
        e.effects.insert(h, Effect { label: label.to_owned(), sound, frames });
        h
    })
}
pub fn release_sound_effect(h: u32) -> i32 {
    let Some(label) = with(|e| e.effects.remove(&h).map(|x| x.label)) else { return E_INVALIDARG };
    trace::line(|| format!("se.release {label}"));
    0
}
pub fn set_master_volume(v: f32) -> i32 {
    trace::line(|| format!("se.master {}", trace::f(v)));
    with(|e| {
        e.master = Some(v);
        push(e, AudioCommand::MasterVolume(v));
    });
    0
}
pub fn get_master_volume() -> f32 {
    trace::line(|| "se.getmaster".into());
    with(|e| e.master.unwrap_or(1.0))
}

/// `CreateSoundEffectInstance(soundEffect)`: a Stopped voice of the effect's sound.
pub fn create_instance(fx: u32) -> Result<u32, i32> {
    let internal = trace::is_internal();
    let r: Result<(u32, Option<u32>, String), i32> = with(|e| {
        let (label, sound, frames) = {
            let x = e.effects.get(&fx).ok_or(E_INVALIDARG)?;
            (x.label.clone(), x.sound.clone(), x.frames)
        };
        let k = if internal {
            None
        } else {
            e.next_voice += 1;
            Some(e.next_voice - 1)
        };
        let h = handle(e);
        e.voices.insert(h, new_voice(k, Some(sound.clone()), sound.channels, sound.sample_rate, frames));
        Ok((h, k, label))
    });
    let (h, k, label) = r?;
    trace::line(|| format!("si.create {} {label}", si(k)));
    Ok(h)
}
/// A voice of a sound no effect object owns (the waves of XACT cues): internal, not traced.
pub fn create_internal_voice(sound: Arc<Sound>) -> u32 {
    let frames = sound.frames() as u64;
    with(|e| {
        let h = handle(e);
        e.voices.insert(h, new_voice(None, Some(sound.clone()), sound.channels, sound.sample_rate, frames));
        h
    })
}
fn new_voice(k: Option<u32>, sound: Option<Arc<Sound>>, channels: u16, rate: u32, frames: u64) -> Voice {
    Voice { k, sound, channels, rate, frames, state: VS_STOPPED, queued: false, consumed: 0, loop_count: 0, ratio24: 1 << 24, dynamic: false, buffers: VecDeque::new(), volume: 1.0, pan: 0.0, pitch: 0.0 }
}
fn voice_k(h: u32) -> Option<Option<u32>> {
    with(|e| e.voices.get(&h).map(|v| v.k))
}

pub fn destroy_instance(h: u32) -> i32 {
    let Some(v) = with(|e| {
        let v = e.voices.remove(&h);
        if v.is_some() {
            push(e, AudioCommand::Stop { voice: h as u64 });
        }
        v
    }) else {
        return E_INVALIDARG;
    };
    trace::line(|| format!("si.destroy {}", si(v.k)));
    0
}

/// `SubmitPacket(loopStart, loopLength, loopCount)`: the buffer is queued (loop count -1 is XAUDIO2_LOOP_INFINITE).
pub fn submit_packet(h: u32, loop_start: i32, loop_length: i32, loop_count: i32) -> i32 {
    let Some(k) = with(|e| {
        let v = e.voices.get_mut(&h)?;
        v.loop_count = if loop_count == -1 { 255 } else { loop_count };
        v.queued = true;
        v.consumed = 0;
        Some(v.k)
    }) else {
        return E_INVALIDARG;
    };
    trace::line(|| format!("si.submit {} {loop_start} {loop_length} {loop_count}", si(k)));
    0
}

pub fn play(h: u32) -> i32 {
    let Some((k, state)) = with(|e| {
        let v = e.voices.get_mut(&h)?;
        let mut cmd = None;
        if v.state == VS_STOPPED || v.state == VS_PAUSED {
            if v.dynamic || v.state == VS_PAUSED {
                cmd = Some(AudioCommand::Resume { voice: h as u64 });
            } else {
                if !v.queued {
                    v.queued = true;
                    v.consumed = 0;
                }
                if let Some(sound) = v.sound.clone() {
                    cmd = Some(AudioCommand::Play { voice: h as u64, sound, volume: v.volume, pan: v.pan, pitch: v.pitch, looped: v.loop_count != 0 });
                }
            }
            v.state = VS_PLAYING;
        }
        let out = (v.k, v.state);
        if let Some(c) = cmd {
            push(e, c);
        }
        Some(out)
    }) else {
        return E_INVALIDARG;
    };
    trace::line(|| format!("si.play {} {state}", si(k)));
    0
}

pub fn stop(h: u32, immediate: bool) -> i32 {
    let Some((k, state)) = with(|e| {
        let v = e.voices.get_mut(&h)?;
        let mut stop = false;
        if immediate {
            let queued = if v.dynamic { !v.buffers.is_empty() } else { v.queued };
            v.state = if queued { VS_STOPPING } else { VS_STOPPED };
            stop = true;
        } else if v.loop_count != 0 {
            // ExitLoop: the voice plays its buffer to the end once more (not used by the game)
            v.loop_count = 0;
            v.consumed = 0;
        }
        let (out, dynamic, channels, rate) = ((v.k, v.state), v.dynamic, v.channels, v.rate);
        if stop {
            push(e, AudioCommand::Stop { voice: h as u64 });
            if dynamic {
                // a dynamic voice keeps existing: a new, empty, paused stream in the mixer
                push(e, AudioCommand::StreamOpen { voice: h as u64, channels, sample_rate: rate });
            }
        }
        Some(out)
    }) else {
        return E_INVALIDARG;
    };
    trace::line(|| format!("si.stop {} {} {state}", si(k), immediate as i32));
    0
}

pub fn pause(h: u32) -> i32 {
    let Some((k, state)) = with(|e| {
        let v = e.voices.get_mut(&h)?;
        let paused = v.state == VS_PLAYING;
        if paused {
            v.state = VS_PAUSED;
        }
        let out = (v.k, v.state);
        if paused {
            push(e, AudioCommand::Pause { voice: h as u64 });
        }
        Some(out)
    }) else {
        return E_INVALIDARG;
    };
    trace::line(|| format!("si.pause {} {state}", si(k)));
    0
}

/// `SetVolume`: the XAudio2 voice volume is the value itself (`KernelSoundEffectInstance::SetVolume`).
pub fn set_volume(h: u32, volume: f32) -> i32 {
    let Some(k) = with(|e| {
        let v = e.voices.get_mut(&h)?;
        v.volume = volume;
        let k = v.k;
        push(e, AudioCommand::SetVolume { voice: h as u64, volume });
        Some(k)
    }) else {
        return E_INVALIDARG;
    };
    trace::line(|| format!("si.volume {} {} v={}", si(k), trace::f(volume), trace::f(volume)));
    0
}

pub fn set_pitch(h: u32, pitch: f32) -> i32 {
    let ratio = mixer::frequency_ratio(pitch);
    let Some(k) = with(|e| {
        let v = e.voices.get_mut(&h)?;
        v.pitch = pitch;
        v.ratio24 = (ratio as f64 * (1u64 << 24) as f64) as u64;
        let k = v.k;
        push(e, AudioCommand::SetPitch { voice: h as u64, pitch });
        Some(k)
    }) else {
        return E_INVALIDARG;
    };
    trace::line(|| format!("si.pitch {} {} r={}", si(k), trace::f(pitch), trace::f(ratio)));
    0
}

pub fn set_pan(h: u32, pan: f32) -> i32 {
    let Some((k, channels)) = with(|e| {
        let v = e.voices.get_mut(&h)?;
        v.pan = pan;
        let out = (v.k, v.channels);
        push(e, AudioCommand::SetPan { voice: h as u64, pan });
        Some(out)
    }) else {
        return E_INVALIDARG;
    };
    trace::line(|| {
        let m = mixer::output_matrix(channels, pan);
        let n = if channels == 2 { 4 } else { 2 };
        format!("si.pan {} {} m={}", si(k), trace::f(pan), m[..n].iter().map(|x| trace::f(*x)).collect::<Vec<_>>().join(","))
    });
    0
}

/// `GetState`: the kernel's voice state.
pub fn get_state(h: u32) -> Result<i32, i32> {
    let (k, state) = with(|e| e.voices.get(&h).map(|v| (v.k, v.state))).ok_or(E_INVALIDARG)?;
    trace::line(|| format!("si.state {} {state}", si(k)));
    Ok(state)
}

/// `CreateDynamicSoundEffectInstance(format)`: a streaming voice (16-bit PCM).
pub fn create_dynamic(format: &[u8]) -> u32 {
    let u16at = |o: usize| u16::from_le_bytes([format[o], format[o + 1]]);
    let (channels, rate) = (u16at(2), u32::from_le_bytes([format[4], format[5], format[6], format[7]]));
    let internal = trace::is_internal();
    let (h, k) = with(|e| {
        let k = if internal {
            None
        } else {
            e.next_voice += 1;
            Some(e.next_voice - 1)
        };
        let h = handle(e);
        let mut v = new_voice(k, None, channels, rate, 0);
        v.dynamic = true;
        e.voices.insert(h, v);
        push(e, AudioCommand::StreamOpen { voice: h as u64, channels, sample_rate: rate });
        (h, k)
    });
    trace::line(|| format!("sd.create {} {}", si(k), trace::bytes(format)));
    h
}
/// `SubmitBuffer(data)`: 16-bit little-endian PCM appended to the stream.
pub fn submit_buffer(h: u32, data: &[u8]) -> i32 {
    let Some(k) = with(|e| {
        let v = e.voices.get_mut(&h)?;
        v.buffers.push_back(data.len() as u64 / (2 * v.channels.max(1) as u64));
        let k = v.k;
        let samples: Vec<i16> = data.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect();
        push(e, AudioCommand::StreamData { voice: h as u64, samples: samples.into() });
        Some(k)
    }) else {
        return E_INVALIDARG;
    };
    trace::line(|| format!("sd.submit {} {}", si(k), trace::bytes(data)));
    0
}
pub fn clear_pending(h: u32) -> i32 {
    let Some(k) = with(|e| {
        let v = e.voices.get_mut(&h)?;
        v.buffers.clear();
        v.consumed = 0;
        let (k, channels, rate) = (v.k, v.channels, v.rate);
        push(e, AudioCommand::Stop { voice: h as u64 });
        push(e, AudioCommand::StreamOpen { voice: h as u64, channels, sample_rate: rate });
        Some(k)
    }) else {
        return E_INVALIDARG;
    };
    trace::line(|| format!("sd.clear {}", si(k)));
    0
}
pub fn pending_buffer_count(h: u32) -> Result<i32, i32> {
    let (k, n) = with(|e| e.voices.get(&h).map(|v| (v.k, v.buffers.len() as i32))).ok_or(E_INVALIDARG)?;
    trace::line(|| format!("sd.pending {} {n}", si(k)));
    Ok(n)
}
/// Seconds of submitted, unplayed audio of a dynamic voice (diagnostics).
pub fn pending_seconds(h: u32) -> f64 {
    with(|e| {
        e.voices.get(&h).map_or(0.0, |v| {
            let frames: u64 = v.buffers.iter().sum();
            let done = v.consumed as f64 / TICKS_SCALE as f64;
            (frames as f64 - done).max(0.0) / v.rate.max(1) as f64
        })
    })
}
pub fn voice_known(h: u32) -> bool {
    voice_k(h).is_some()
}

// ------------------------------------------------------------------------------------------------ XACT

pub fn xact_engine_create(file: &str, lookahead_ms: u32, renderer: &str) -> u32 {
    trace::line(|| format!("xe.create {} {lookahead_ms} {}", trace::asset_label(file), if renderer.is_empty() { "-" } else { renderer }));
    with(handle)
}
pub fn xact_engine_update() {
    trace::line(|| "xe.update".into());
}
pub fn xact_engine_release() {
    trace::line(|| "xe.release".into());
}
pub fn xact_get_category(name: &str) -> u16 {
    let i = with(|e| match e.categories.iter().position(|c| c == name) {
        Some(i) => i,
        None => {
            e.categories.push(name.to_owned());
            e.categories.len() - 1
        }
    });
    trace::line(|| format!("xe.category {name} {i}"));
    i as u16
}
pub fn xact_category_volume(index: u16, v: f32) {
    trace::line(|| format!("xe.catvolume {index} {}", trace::f(v)));
}
pub fn xact_category_pause(index: u16, pause: bool) {
    trace::line(|| format!("xe.catpause {index} {}", pause as i32));
}
pub fn xact_category_stop(index: u16, options: u32) {
    trace::line(|| format!("xe.catstop {index} {options}"));
}
pub fn xact_set_global(name: &str, v: f32) {
    with(|e| e.globals.insert(name.to_owned(), v));
    trace::line(|| format!("xe.setglobal {name} {}", trace::f(v)));
}
pub fn xact_get_global(name: &str) -> f32 {
    trace::line(|| format!("xe.getglobal {name}"));
    with(|e| e.globals.get(name).copied().unwrap_or(0.0))
}
pub fn xact_wave_bank(file: &str) -> u32 {
    trace::line(|| format!("xw.create {}", trace::asset_label(file)));
    with(handle)
}
pub fn xact_wave_bank_streaming(file: &str, offset: u32, packet_size: i16) -> u32 {
    trace::line(|| format!("xw.stream {} {offset} {packet_size}", trace::asset_label(file)));
    with(handle)
}
pub fn xact_wave_bank_status() -> u32 {
    trace::line(|| "xw.status 4".into());
    CUE_PREPARED
}
pub fn xact_wave_bank_release() {
    trace::line(|| "xw.release".into());
}
pub fn xact_sound_bank(file: &str) -> u32 {
    trace::line(|| format!("xs.create {}", trace::asset_label(file)));
    with(handle)
}
pub fn xact_sound_bank_status() -> u32 {
    trace::line(|| "xs.status 0".into());
    0
}
pub fn xact_sound_bank_release() {
    trace::line(|| "xs.release".into());
}
/// `SoundBank.GetCue`: a Prepared cue.
pub fn xact_get_cue(name: &str) -> u32 {
    let (h, k) = with(|e| {
        let h = handle(e);
        let k = e.next_cue;
        e.next_cue += 1;
        e.cues.insert(h, CueM { k, bits: CUE_PREPARED, vars: HashMap::new(), voice: None });
        (h, k)
    });
    trace::line(|| format!("xs.getcue {name} xc{k}"));
    h
}
fn cue_k(h: u32) -> Option<u32> {
    with(|e| e.cues.get(&h).map(|c| c.k))
}
/// `Cue.GetStatus`: the status bits. A cue whose internal voice ran out reads Stopped (and is counted).
pub fn xact_cue_status(h: u32) -> u32 {
    let r = with(|e| {
        let (k, bits, voice) = {
            let c = e.cues.get(&h)?;
            (c.k, c.bits, c.voice)
        };
        let mut bits = bits;
        if bits == CUE_PLAYING {
            let ended = voice.is_some_and(|v| e.voices.get(&v).is_none_or(|x| x.state == VS_STOPPED));
            if ended {
                bits = CUE_STOPPED;
                e.cues_ran_out += 1;
                if let Some(c) = e.cues.get_mut(&h) {
                    c.bits = CUE_STOPPED;
                }
            }
        }
        Some((k, bits))
    });
    let Some((k, bits)) = r else { return 0 };
    trace::line(|| format!("xc.status xc{k} {bits}"));
    bits
}
/// `Cue.Play`: Prepared -> Playing. True when the cue starts now (the managed layer then starts its voice).
pub fn xact_cue_play(h: u32) -> bool {
    let Some((k, started)) = with(|e| {
        let c = e.cues.get_mut(&h)?;
        let started = c.bits == CUE_PREPARED;
        if started {
            c.bits = CUE_PLAYING;
        }
        Some((c.k, started))
    }) else {
        return false;
    };
    trace::line(|| format!("xc.play xc{k}"));
    started
}
/// The internal voice that plays a cue's wave.
pub fn xact_cue_set_voice(h: u32, voice: u32) {
    with(|e| {
        if let Some(c) = e.cues.get_mut(&h) {
            c.voice = Some(voice);
        }
    });
}
/// `Cue.Pause(pause)`: returns the new status bits.
pub fn xact_cue_pause(h: u32, pause: bool) -> u32 {
    let Some((k, bits)) = with(|e| {
        let c = e.cues.get_mut(&h)?;
        if pause && c.bits == CUE_PLAYING {
            c.bits = CUE_PAUSED;
        } else if !pause && c.bits == CUE_PAUSED {
            c.bits = CUE_PLAYING;
        }
        Some((c.k, c.bits))
    }) else {
        return 0;
    };
    trace::line(|| format!("xc.pause xc{k} {}", pause as i32));
    bits
}
pub fn xact_cue_stop(h: u32, options: u32) {
    let Some(k) = with(|e| {
        let c = e.cues.get_mut(&h)?;
        c.bits = CUE_STOPPED;
        Some(c.k)
    }) else {
        return;
    };
    trace::line(|| format!("xc.stop xc{k} {options}"));
}
pub fn xact_cue_set_variable(h: u32, name: &str, value: f32) {
    let Some(k) = with(|e| {
        let c = e.cues.get_mut(&h)?;
        c.vars.insert(name.to_owned(), value);
        Some(c.k)
    }) else {
        return;
    };
    trace::line(|| format!("xc.setvar xc{k} {name} {}", trace::f(value)));
}
pub fn xact_cue_get_variable(h: u32, name: &str) {
    if let Some(k) = cue_k(h) {
        trace::line(|| format!("xc.getvar xc{k} {name}"));
    }
}
pub fn xact_cue_release(h: u32) {
    let Some(k) = with(|e| e.cues.remove(&h).map(|c| c.k)) else { return };
    trace::line(|| format!("xc.release xc{k}"));
}

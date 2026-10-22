//! Plays one cue of the real `Sound Bank.xsb` / `Wave Bank.xwb` through the host's XACT path and the app's mixer
//! (the mixer source file of terraria-app is included as a module: it needs no Bevy) and reports peak and RMS.
//!
//!     host_audio_demo [cue name = Music_1] [seconds = 5]
//!
//! `TERRARIA_MUTE=1` mixes without opening a device (the measurement run); without it the default output device is
//! opened. `TERRARIA_VOLUME=0.03` scales the device output to 3 % (the mixer's own gain; the cue's XACT volume is
//! untouched). The run ends after the given number of seconds (at most 5 are accepted).

#[path = "../../terraria-app/src/host/audio.rs"]
mod mixer;

use std::time::{Duration, Instant};

use terraria_gen_rt::rt::{self, Str};
use terraria_host::audio::{self, AudioEngine, SoundBank, WaveBank};

fn main() {
    let cue_name = std::env::args().nth(1).unwrap_or_else(|| "Music_1".into());
    let seconds: f64 = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(5.0_f64).clamp(0.5, 5.0);
    if std::env::var_os("TERRARIA_MUTE").is_none() && std::env::var("TERRARIA_VOLUME").ok().and_then(|v| v.parse::<f32>().ok()).is_none_or(|v| v > 0.03) {
        eprintln!("refusing to open the real device without TERRARIA_VOLUME<=0.03 (set TERRARIA_MUTE=1 for the silent measurement run)");
        std::process::exit(2);
    }
    let backend = mixer::AudioBackend::start();

    let t = Instant::now();
    let _engine = AudioEngine::new_gc(Str::new("Content\\TerrariaMusic.xgs"));
    let sound_bank = SoundBank::new_gc(Str::new("Content\\Sound Bank.xsb"));
    let _wave_bank = WaveBank::new_gc(Str::new("Content\\Wave Bank.xwb"));
    let cue = rt::run(|| sound_bank_cue(sound_bank, &cue_name)).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1);
    });
    println!("xact loaded in {} ms: {} cues, categories {:?}", t.elapsed().as_millis(), unsafe { (*sound_bank.ptr()).cue_names().len() }, AudioEngine::category_names());

    // LegacyAudioSystem.UpdateCommonTrack: SetVariable("Volume", ..) then Play()
    let cue_ref = unsafe { &mut *cue.ptr() };
    cue_ref.set_variable(Str::new("Volume"), 1.0);
    let t = Instant::now();
    cue_ref.play();
    println!("Play() of {cue_name}: {} ms (wave decoded: {:?})", t.elapsed().as_millis(), audio::xact_stats());
    backend.submit(audio::take_commands());

    let start = Instant::now();
    let mut last = start;
    while start.elapsed().as_secs_f64() < seconds {
        std::thread::sleep(Duration::from_millis(20));
        let now = Instant::now();
        audio::advance((now - last).as_secs_f64());
        last = now;
        backend.submit(audio::take_commands());
    }
    println!("cue state after {seconds} s: playing={} paused={} stopped={}", cue_ref.is_playing(), cue_ref.is_paused(), cue_ref.is_stopped());
    cue_ref.stop(1);
    backend.submit(audio::take_commands());
    std::thread::sleep(Duration::from_millis(100));
    use std::sync::atomic::Ordering::Relaxed;
    let st = &backend.stats;
    println!(
        "device_open={} blocks={} voices_started={} peak={} ({:.1} % of full scale) rms={:.1} ({:.2} % of full scale) cue stopped={}",
        st.device_open.load(Relaxed),
        st.blocks_written.load(Relaxed),
        st.voices_started.load(Relaxed),
        st.peak.load(Relaxed),
        st.peak.load(Relaxed) as f64 / 327.68,
        st.rms(),
        st.rms() / 327.68,
        cue_ref.is_stopped()
    );
}

fn sound_bank_cue(sb: rt::Gc<SoundBank>, name: &str) -> rt::Gc<audio::Cue> {
    // SAFETY: a live object
    unsafe { (*sb.ptr()).get_cue(Str::new(name)) }
}

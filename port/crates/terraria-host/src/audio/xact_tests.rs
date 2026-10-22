//! Tests of the XACT path (real files of the player's install, skipped with a note when absent) and of the Vorbis /
//! MP3 decoders (fixtures in tests/data, expected values from the game's NVorbis.dll and MP3Sharp.dll).

use super::*;
use super::codec::{Mp3Stream, VorbisReader};
use terraria_gen_rt::rt::{Arr, Dictionary, Str};

fn install() -> Option<std::path::PathBuf> {
    let p = crate::content::install_dir().join("Content");
    if p.join("Sound Bank.xsb").is_file() && p.join("Wave Bank.xwb").is_file() && p.join("TerrariaMusic.xgs").is_file() {
        Some(p)
    } else {
        eprintln!("skipped: the XACT files are not in {}", p.display());
        None
    }
}
fn s(x: &str) -> Str {
    Str::new(x)
}
fn load_xact() -> (Gc<AudioEngine>, Gc<WaveBank>, Gc<SoundBank>) {
    let e = AudioEngine::new_gc(s("Content\\TerrariaMusic.xgs"));
    let sb = SoundBank::new_gc(s("Content\\Sound Bank.xsb"));
    let wb = WaveBank::new_gc(s("Content\\Wave Bank.xwb"));
    (e, wb, sb)
}

#[test]
fn xgs_and_xsb_are_understood() {
    let Some(dir) = install() else { return };
    let xgs = xact::Xgs::parse(&std::fs::read(dir.join("TerrariaMusic.xgs")).unwrap()).unwrap();
    let cats: Vec<&str> = xgs.categories.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(cats, ["Global", "Default", "Music"]);
    assert_eq!(xgs.categories[2].parent, 0);
    assert!(xgs.categories.iter().all(|c| c.volume_db.abs() < 0.01), "all categories are at 0 dB");
    let vars: Vec<&str> = xgs.variables.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(vars, ["NumCueInstances", "AttackTime", "ReleaseTime", "OrientationAngle", "DopplerPitchScalar", "SpeedOfSound", "Distance", "Volume"]);
    println!("XGS variables (min, max, initial): {:?}", xgs.variables.iter().map(|v| (v.name.as_str(), v.min, v.max, v.initial)).collect::<Vec<_>>());

    let xsb = xact::Xsb::parse(&std::fs::read(dir.join("Sound Bank.xsb")).unwrap()).unwrap();
    assert_eq!((xsb.cues.len(), xsb.sounds.len()), (104, 104));
    assert_eq!(xsb.wavebanks, ["Wave Bank"]);
    assert!(xsb.cues.iter().all(|c| c.name.starts_with("Music_") && c.sound.is_some()), "104 named simple cues");
    let mut names: Vec<&str> = xsb.cues.iter().map(|c| c.name.as_str()).collect();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), 104, "cue names are unique");
    println!("cues: {}", xsb.cues.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(" "));
    // every sound: one wave of bank 0, category Music, -11.94 dB, one RPC: the Volume curve
    for snd in &xsb.sounds {
        assert!(!snd.complex && snd.wave.is_some() && snd.wave.unwrap().1 == 0);
        assert_eq!(snd.category, 1, "category of the sounds: Default (index 1)");
        assert!((snd.volume_db + 11.94).abs() < 0.02, "volume {} dB", snd.volume_db);
        assert_eq!(snd.rpc_codes.len(), 1);
    }
    let rpc = xgs.rpc(xsb.sounds[0].rpc_codes[0]).unwrap();
    assert_eq!((rpc.variable, rpc.parameter), (7, 0), "RPC of the variable Volume controlling the volume");
    assert_eq!(rpc.points.len(), 2);
    // Terraria's own emulation (ASoundEffectBasedAudioTrack.ReMapVolumeToMatchXact): dB = 31 v - 25 - 11.94
    for v in [0.0f32, 0.25, 0.5, 1.0] {
        let db = xsb.sounds[0].volume_db + rpc.eval(v) / 100.0;
        assert!((db - (31.0 * v - 25.0 - 11.94)).abs() < 0.02, "Volume {v}: {db} dB");
    }
}

#[test]
fn wave_bank_entries_cover_every_cue() {
    let Some(dir) = install() else { return };
    let xsb = xact::Xsb::parse(&std::fs::read(dir.join("Sound Bank.xsb")).unwrap()).unwrap();
    let wb = WaveBank::open(&dir.join("Wave Bank.xwb")).unwrap();
    assert_eq!(wb.name, "Wave Bank");
    let mut used = std::collections::HashSet::new();
    for c in &xsb.cues {
        let (w, _) = xsb.sounds[c.sound.unwrap()].wave.unwrap();
        assert!((w as usize) < wb.entries.len(), "{} -> wave {w}", c.name);
        used.insert(w);
    }
    println!("{} cues use {} distinct waves of {}", xsb.cues.len(), used.len(), wb.entries.len());
    assert_eq!(used.len(), 104);
}

/// Every cue decodes to non-silent PCM of a plausible length; reports time and memory (the streaming decision).
#[test]
fn every_cue_decodes_to_music() {
    let Some(_) = install() else { return };
    let (_e, _wb, sb) = load_xact();
    let sb = unsafe { &*sb.ptr() };
    let t = std::time::Instant::now();
    let (mut total_secs, mut max_bytes, mut max_secs, mut min_secs, mut total_bytes) = (0.0f64, 0usize, 0.0f64, f64::MAX, 0usize);
    let mut slowest = (0u128, String::new());
    for name in sb.cue_names() {
        let t1 = std::time::Instant::now();
        let def = sb.data.cues.iter().find(|c| c.name == name).unwrap();
        let (wave, _) = sb.data.sounds[def.sound.unwrap()].wave.unwrap();
        let sound = wave_for("Wave Bank", wave as usize).unwrap();
        let ms = t1.elapsed().as_millis();
        if ms > slowest.0 {
            slowest = (ms, name.clone());
        }
        let secs = sound.seconds();
        assert!(secs > 5.0 && secs < 1200.0, "{name}: {secs} s");
        assert!(sound.channels == 2 && (44_000..=48_200).contains(&sound.sample_rate), "{name}: {} ch, {} Hz", sound.channels, sound.sample_rate);
        // not silent: some second of the track has real energy
        let step = sound.sample_rate as usize * 2;
        let loud = sound.samples.chunks(step).any(|c| (c.iter().map(|x| (*x as f64).powi(2)).sum::<f64>() / c.len() as f64).sqrt() > 300.0);
        assert!(loud, "{name} is silent");
        total_secs += secs;
        min_secs = min_secs.min(secs);
        max_secs = max_secs.max(secs);
        max_bytes = max_bytes.max(sound.samples.len() * 2);
        total_bytes += sound.samples.len() * 2;
    }
    println!("104 cues: {:.1} min of music in total ({:.0} s to {:.0} s per track); PCM of all tracks {:.0} MB, largest {:.1} MB; all decoded in {:.1} s (slowest {} ms: {}); decoded waves kept in the cache: {}", total_secs / 60.0, min_secs, max_secs, total_bytes as f64 / 1e6, max_bytes as f64 / 1e6, t.elapsed().as_secs_f64(), slowest.0, slowest.1, WAVE_CACHE);
    assert!(total_secs > 3600.0, "an hour or more of music");
}

fn drain_play(cmds: &[AudioCommand]) -> Option<(u64, f32, f32)> {
    cmds.iter().find_map(|c| match c {
        AudioCommand::Play { voice, volume, pan, .. } => Some((*voice, *volume, *pan)),
        _ => None,
    })
}

#[test]
fn cue_lifecycle_and_volume_variable() {
    let Some(_) = install() else { return };
    let (_e, _wb, sb) = load_xact();
    take_commands();
    unsafe {
        let sb = &*sb.ptr();
        // unknown cue
        assert!(rt::run(|| sb.get_cue(s("No_Such_Cue"))).is_err());
        let cue = &mut *sb.get_cue(s("Music_1")).ptr();
        assert!(cue.is_prepared() && !cue.is_playing() && !cue.is_stopped() && !cue.is_paused());
        // the game sets Volume before Play (LegacyAudioSystem.UpdateCommonTrack)
        cue.set_variable(s("Volume"), 0.5);
        assert!(take_commands().is_empty(), "nothing reaches the mixer before Play");
        assert_eq!(cue.get_variable(s("Volume")), 0.5);
        cue.play();
        assert!(cue.is_playing() && !cue.is_stopped() && !cue.is_prepared());
        let cmds = take_commands();
        let (voice, volume, _) = drain_play(&cmds).expect("a Play command");
        // dB = -11.94 (sound) + 31 * 0.5 - 25 (RPC) = -21.44 dB
        let expect = 10f32.powf((-11.94 + 31.0 * 0.5 - 25.0) / 20.0);
        assert!((volume - expect).abs() < 1e-3, "volume {volume} vs {expect}");
        // Volume changes while playing
        cue.set_variable(s("Volume"), 1.0);
        let cmds = take_commands();
        let vol = cmds.iter().find_map(|c| if let AudioCommand::SetVolume { voice: v, volume } = c { (*v == voice).then_some(*volume) } else { None }).expect("SetVolume");
        assert!((vol - 10f32.powf((-11.94 + 6.0) / 20.0)).abs() < 1e-3);
        // values outside the variable's range are clamped; an unknown variable throws
        cue.set_variable(s("Volume"), 5.0);
        assert_eq!(cue.get_variable(s("Volume")), 1.0);
        assert!(rt::run(|| cue.set_variable(s("Nope"), 1.0)).is_err());
        cue.pause();
        assert!(cue.is_paused() && !cue.is_playing());
        cue.resume();
        assert!(cue.is_playing());
        cue.stop(1);
        assert!(cue.is_stopped() && !cue.is_playing());
        assert!(take_commands().iter().any(|c| matches!(c, AudioCommand::Stop { voice: v } if *v == voice)));
        // a stopped cue cannot be replayed; Reuse() gets a new one
        assert!(rt::run(|| cue.play()).is_err());
        let again = &mut *sb.get_cue(s("Music_1")).ptr();
        again.play();
        assert!(again.is_playing());
        again.stop(0);
        take_commands();
    }
}

#[test]
fn cue_ends_by_itself_and_category_volume_applies() {
    let Some(_) = install() else { return };
    let (engine, _wb, sb) = load_xact();
    take_commands();
    unsafe {
        let sb = &*sb.ptr();
        let cue = &mut *sb.get_cue(s("Music_2")).ptr();
        cue.set_variable(s("Volume"), 1.0);
        cue.play();
        let cmds = take_commands();
        let (voice, volume, _) = drain_play(&cmds).unwrap();
        let cat = (*engine.ptr()).get_category(s("Music"));
        assert!(rt::run(|| (*engine.ptr()).get_category(s("Nope"))).is_err());
        // Music's parent is Global; the cue's own category is Default: SetVolume("Music") does not touch it, "Default" does
        (*cat.ptr()).set_volume(0.5);
        assert!(take_commands().iter().all(|c| !matches!(c, AudioCommand::SetVolume { voice: v, .. } if *v == voice)));
        let def = (*engine.ptr()).get_category(s("Default"));
        (*def.ptr()).set_volume(0.5);
        let vol = take_commands().iter().find_map(|c| if let AudioCommand::SetVolume { voice: v, volume } = c { (*v == voice).then_some(*volume) } else { None }).expect("category SetVolume re-gains the cue");
        assert!((vol - volume * 0.5).abs() < 1e-4);
        (*def.ptr()).set_volume(1.0);
        (*cat.ptr()).set_volume(1.0);
        take_commands();
        // the host clock decides the end of the voice
        advance(10_000.0);
        assert!(cue.is_stopped() && !cue.is_playing());
    }
}

// ------------------------------------------------------------------------------------------------ codecs

fn data(file: &str) -> Vec<u8> {
    std::fs::read(crate::manifest_dir().join("tests").join("data").join(file)).expect("fixture")
}
fn expected() -> std::collections::HashMap<(String, String), String> {
    let t = std::fs::read_to_string(crate::manifest_dir().join("tests").join("dotnet").join("codec.expected")).expect("codec.expected");
    t.lines().map(|l| { let p: Vec<&str> = l.split('\t').collect(); ((p[0].to_owned(), p[1].to_owned()), p.get(2).unwrap_or(&"").to_string()) }).collect()
}
fn num(x: &str) -> f64 {
    x.parse().expect("number")
}

#[test]
fn vorbis_reader_matches_nvorbis() {
    let exp = expected();
    for f in ["tone_stereo.ogg", "tone_mono.ogg"] {
        let e = |k: &str| exp[&(f.to_owned(), k.to_owned())].clone();
        let r = unsafe { &mut *VorbisReader::new_gc(Stream::from_vec(data(f))).ptr() };
        assert_eq!(r.channels().to_string(), e("channels"), "{f}");
        assert_eq!(r.sample_rate().to_string(), e("rate"), "{f}");
        assert_eq!(r.total_samples().to_string(), e("total"), "{f}");
        // tags: LOOPSTART / LOOPEND as the game reads them (keys upper case)
        let tags = r.tags_all();
        let mut got: Vec<String> = Dictionary::as_seq(tags).to_vec().iter().filter(|kv| kv.key.as_str() != "ENCODER").map(|kv| format!("{}={}", kv.key.as_str(), unsafe { (*kv.value.ptr()).get_item(0).as_str() })).collect();
        got.sort();
        assert_eq!(got.join(";"), e("tags"), "{f}");
        // all samples, in blocks of 2048 floats like OGGAudioTrack
        let buf = Arr::<f32>::new(2048);
        let mut all: Vec<f32> = Vec::new();
        loop {
            let n = r.read_samples(buf, 0, 2048);
            if n <= 0 {
                break;
            }
            all.extend_from_slice(&buf.as_slice()[..n as usize]);
        }
        assert_eq!(all.len().to_string(), e("floats"), "{f}: ReadSamples total");
        assert_eq!(r.decoded_position().to_string(), e("position_at_end"), "{f}");
        let rms = (all.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / all.len() as f64).sqrt();
        let peak = all.iter().fold(0f32, |m, v| m.max(v.abs())) as f64;
        assert!((rms - num(&e("rms"))).abs() < 1e-4, "{f} rms {rms} vs {}", e("rms"));
        assert!((peak - num(&e("peak"))).abs() < 1e-3, "{f} peak {peak} vs {}", e("peak"));
        for i in [0usize, 1, 1000, 20001, 20002] {
            if let Some(v) = all.get(i) {
                assert!((*v as f64 - num(&e(&format!("sample {i}")))).abs() < 2e-3, "{f} sample {i}: {v} vs {}", e(&format!("sample {i}")));
            }
        }
        // seeks, as the game uses them
        let check = |r: &mut VorbisReader, label: &str, n: i32| {
            let got = r.read_samples(buf, 0, n);
            let b = buf.as_slice();
            let exp_line: Vec<String> = e(label).split(' ').map(str::to_owned).collect();
            assert_eq!(got.to_string(), exp_line[0], "{f} {label}: floats read");
            assert_eq!(r.decoded_position().to_string(), exp_line[1], "{f} {label}: position");
            assert!((b[0] as f64 - num(&exp_line[2])).abs() < 2e-3, "{f} {label}: first {} vs {}", b[0], exp_line[2]);
        };
        r.set_decoded_position(5000);
        check(r, "seek5000", 8);
        r.set_decoded_position(10000);
        check(r, "seek10000", 4);
        r.set_decoded_position(0);
        check(r, "seek0", 4);
        // the loop in OGGAudioTrack: position back to LOOPSTART after reading past LOOPEND
        r.set_decoded_position(r.total_samples());
        assert_eq!(r.read_samples(buf, 0, 16), 0);
        r.set_decoded_position(10000);
        assert_eq!(r.read_samples(buf, 0, 16), 16);
        r.dispose();
    }
    // not an Ogg stream
    assert!(rt::run(|| VorbisReader::new_gc(Stream::from_vec(vec![1, 2, 3, 4, 5, 6, 7, 8]))).is_err());
}

#[test]
fn mp3_stream_matches_mp3sharp() {
    let exp = expected();
    for f in ["tone_stereo.mp3", "tone_mono.mp3"] {
        let e = |k: &str| exp[&(f.to_owned(), k.to_owned())].clone();
        let m = unsafe { &mut *Mp3Stream::new_gc(Stream::from_vec(data(f))).ptr() };
        assert_eq!(m.frequency().to_string(), e("frequency"), "{f}");
        assert_eq!(m.length().to_string(), e("length"), "{f}");
        let buf = Arr::<u8>::new(4096);
        let mut all: Vec<u8> = Vec::new();
        loop {
            let n = m.read(buf, 0, 4096);
            if n <= 0 {
                break;
            }
            all.extend_from_slice(&buf.as_slice()[..n as usize]);
        }
        assert_eq!(all.len().to_string(), e("bytes"), "{f}: bytes read (every frame, no gapless trimming)");
        assert_eq!(m.position().to_string(), e("position_at_end"), "{f}");
        let pcm: Vec<i16> = all.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect();
        let rms = (pcm.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / pcm.len() as f64).sqrt();
        let peak = pcm.iter().map(|v| v.unsigned_abs()).max().unwrap() as f64;
        assert!((rms / num(&e("rms")) - 1.0).abs() < 0.005, "{f} rms {rms} vs {}", e("rms"));
        assert!((peak - num(&e("peak"))).abs() < 12.0, "{f} peak {peak} vs {}", e("peak"));
        for i in [4000usize, 20000, 40001, 45000] {
            assert!((pcm[i] as f64 - num(&e(&format!("sample {i}")))).abs() < 12.0, "{f} sample {i}: {} vs {}", pcm[i], e(&format!("sample {i}")));
        }
        m.set_position(0);
        assert_eq!(m.read(buf, 0, 16), 16);
        assert!((i16::from_le_bytes([buf.as_slice()[0], buf.as_slice()[1]]) as i32).abs() < 5, "{f} rewind");
    }
}

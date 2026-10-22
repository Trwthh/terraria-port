//! Records what the default output device plays (WASAPI loopback: an input stream opened on a render endpoint) into a
//! 32-bit float WAV file. Nothing is played and no device setting is changed. `loopback_capture <seconds> <out.wav>`
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::io::Write;
use std::sync::{Arc, Mutex};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seconds: f32 = args.get(1).and_then(|v| v.parse().ok()).unwrap_or(5.0);
    let out = args.get(2).cloned().unwrap_or_else(|| "loopback.wav".to_owned());
    let host = cpal::default_host();
    let device = host.default_output_device().expect("no default output device");
    let config = device.default_output_config().expect("no output format");
    let (rate, channels) = (config.sample_rate(), config.channels());
    eprintln!("loopback: default render endpoint, {} Hz, {} channels, {:?}", u32::from(rate), channels, config.sample_format());
    let samples: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = samples.clone();
    let started = std::time::Instant::now();
    let first: Arc<Mutex<Option<f32>>> = Arc::new(Mutex::new(None));
    let first2 = first.clone();
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &config.config(),
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                first2.lock().unwrap().get_or_insert(started.elapsed().as_secs_f32());
                sink.lock().unwrap().extend_from_slice(data)
            },
            |e| eprintln!("loopback: stream error {e}"),
            None,
        ),
        cpal::SampleFormat::I16 => device.build_input_stream(
            &config.config(),
            move |data: &[i16], _: &cpal::InputCallbackInfo| {
                first2.lock().unwrap().get_or_insert(started.elapsed().as_secs_f32());
                sink.lock().unwrap().extend(data.iter().map(|s| *s as f32 / 32768.0))
            },
            |e| eprintln!("loopback: stream error {e}"),
            None,
        ),
        f => panic!("sample format {f:?} is not handled"),
    }
    .expect("the loopback stream could not be opened");
    stream.play().expect("the loopback stream could not be started");
    std::thread::sleep(std::time::Duration::from_secs_f32(seconds));
    drop(stream);
    let data = samples.lock().unwrap();
    // WASAPI loopback delivers nothing while the endpoint is silent: the file then is shorter than the time asked for
    eprintln!("loopback: {} frames in {seconds} s (first packet after {:?} s)", data.len() / channels as usize, *first.lock().unwrap());
    // 32-bit float samples (WAVE_FORMAT_IEEE_FLOAT): a capture at 3 % volume would be a few steps of 16 bits
    let bytes = (data.len() * 4) as u32;
    let rate = u32::from(rate);
    let mut f = std::io::BufWriter::new(std::fs::File::create(&out).expect("cannot create the output file"));
    f.write_all(b"RIFF").unwrap();
    f.write_all(&(36 + bytes).to_le_bytes()).unwrap();
    f.write_all(b"WAVEfmt ").unwrap();
    f.write_all(&16u32.to_le_bytes()).unwrap();
    f.write_all(&3u16.to_le_bytes()).unwrap();
    f.write_all(&channels.to_le_bytes()).unwrap();
    f.write_all(&rate.to_le_bytes()).unwrap();
    f.write_all(&(rate * channels as u32 * 4).to_le_bytes()).unwrap();
    f.write_all(&(channels * 4).to_le_bytes()).unwrap();
    f.write_all(&32u16.to_le_bytes()).unwrap();
    f.write_all(b"data").unwrap();
    f.write_all(&bytes.to_le_bytes()).unwrap();
    for s in data.iter() {
        f.write_all(&s.to_le_bytes()).unwrap();
    }
}

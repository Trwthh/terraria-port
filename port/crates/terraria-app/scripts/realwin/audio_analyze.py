"""Analysis of a loopback capture (tools/loopback_capture): python audio_analyze.py <file.wav> [event_time ...]
Prints: duration, non-silence, RMS per second (dBFS), spectral centroid and flatness (music is tonal: low flatness;
noise: flatness near 1), the strongest periodicity of the loudness envelope (a beat), pitch stability, and the
onsets (sudden rises of short-time energy) next to the given event times (seconds from the start of the capture)."""
import sys
import wave

import numpy as np


def load(path):
    """16-bit PCM or 32-bit float WAV (the recorder writes float: a capture at 3 % volume is a few LSB of 16 bits).
    Returns the mean of the channels that carry signal (a stereo stream on a 7.1 endpoint fills two of eight)."""
    raw = open(path, "rb").read()
    import struct
    tag, ch, rate = struct.unpack_from("<HHI", raw, 20)
    bits = struct.unpack_from("<H", raw, 34)[0]
    data = raw[44:]
    if tag == 3 and bits == 32:
        x = np.frombuffer(data[:len(data) // 4 * 4], dtype="<f4").astype(np.float64)
    else:
        x = np.frombuffer(data[:len(data) // 2 * 2], dtype="<i2").astype(np.float64) / 32768.0
    x = x[:len(x) // ch * ch].reshape(-1, ch)
    if len(x) == 0:
        return x.reshape(-1), rate, ch
    power = (x ** 2).mean(axis=0)
    used = [i for i in range(ch) if power[i] > power.max() * 1e-4] or list(range(ch))
    return x[:, used].mean(axis=1), rate, f"{ch} ({len(used)} with signal)"


def db(v):
    return 20 * np.log10(max(v, 1e-9))


def main():
    path = sys.argv[1]
    events = [float(v) for v in sys.argv[2:]]
    x, rate, ch = load(path)
    dur = len(x) / rate
    print(f"file {path}: {dur:.2f} s, {rate} Hz, {ch} channels")
    if len(x) < rate // 2:
        print("result: SILENT (the endpoint delivered no audio packets)")
        return
    peak = np.abs(x).max()
    rms = np.sqrt((x ** 2).mean())
    print(f"overall: peak {peak:.5f} ({db(peak):.1f} dBFS), RMS {rms:.5f} ({db(rms):.1f} dBFS)")
    hop = rate // 100  # 10 ms
    frames = len(x) // hop
    e = np.sqrt((x[:frames * hop].reshape(frames, hop) ** 2).mean(axis=1))
    floor = 10 ** (-80 / 20)
    print(f"non-silence: {100 * (e > floor).mean():.1f} % of 10 ms frames above -80 dBFS")
    per_s = [db(np.sqrt((x[i * rate:(i + 1) * rate] ** 2).mean())) for i in range(int(dur))]
    print("RMS per second (dBFS):", " ".join(f"{v:.1f}" for v in per_s))
    # spectrum: 4096-point frames
    n = 4096
    win = np.hanning(n)
    cents, flats, pitches = [], [], []
    freqs = np.fft.rfftfreq(n, 1 / rate)
    for i in range(0, len(x) - n, n // 2):
        seg = x[i:i + n]
        if np.sqrt((seg ** 2).mean()) < floor:
            continue
        mag = np.abs(np.fft.rfft(seg * win)) + 1e-12
        p = mag ** 2
        cents.append((freqs * p).sum() / p.sum())
        band = p[(freqs > 50) & (freqs < 8000)]
        flats.append(np.exp(np.log(band).mean()) / band.mean())
        pitches.append(freqs[np.argmax(mag[1:]) + 1])
    if cents:
        print(f"spectral centroid: median {np.median(cents):.0f} Hz (10 % {np.percentile(cents, 10):.0f}, 90 % {np.percentile(cents, 90):.0f})")
        print(f"spectral flatness 50 Hz - 8 kHz: median {np.median(flats):.4f} (white noise is about 0.56 with this window; tonal music is far below 0.1)")
        uniq = len(set(int(round(12 * np.log2(max(f, 20) / 440.0))) for f in pitches))
        print(f"strongest partial per frame: {uniq} distinct semitones over {len(pitches)} frames (a melody moves between a limited set of notes)")
    # beat: autocorrelation of the loudness envelope, lags 0.2 .. 2 s
    env = e - e.mean()
    if env.std() > 0:
        ac = np.correlate(env, env, mode="full")[len(env) - 1:]
        ac = ac / ac[0]
        # the first local maximum after the first minimum, lags 0.15 .. 2 s (a monotonic decay has none: no beat)
        hi = min(200, len(ac) - 2)
        peaks = [k for k in range(15, hi) if ac[k] > ac[k - 1] and ac[k] >= ac[k + 1] and ac[k] > 0.05 and ac[:k].min() < ac[k] - 0.05]
        if peaks:
            best = max(peaks, key=lambda k: ac[k])
            print(f"loudness envelope autocorrelation: {len(peaks)} peaks between 0.15 and 2 s, the strongest at {best * 10} ms (= {60000 / (best * 10):.0f} per minute) with value {ac[best]:.2f}; first at {peaks[0] * 10} ms (noise or a steady tone has no such peaks)")
        else:
            print("loudness envelope autocorrelation: no peak between 0.15 and 2 s (no regular beat in the loudness)")
    # transients: energy above 4 kHz in 10 ms frames (a click or a hit is broadband; the music sits lower)
    from numpy.fft import rfft, rfftfreq
    hb = np.zeros(frames)
    fr = rfftfreq(hop, 1 / rate)
    for i in range(frames):
        m = np.abs(rfft(x[i * hop:(i + 1) * hop] * np.hanning(hop)))
        hb[i] = np.sqrt((m[fr > 4000] ** 2).sum())
    hmed = np.median(hb[hb > 0]) if (hb > 0).any() else 0.0
    hits = [i for i in range(2, frames) if hb[i] > 6 * max(hmed, 1e-12) and hb[i] > 3 * hb[i - 2] and hb[i] >= hb[i - 1]]
    hits = [h for k, h in enumerate(hits) if k == 0 or h - hits[k - 1] > 6]
    print(f"transients (energy above 4 kHz, 6x the median and 3x the level 20 ms before): {len(hits)}:", " ".join(f"{i / 100:.2f}" for i in hits[:80]))
    # onsets: short-time energy at least 6 dB above the mean of the previous 150 ms, 80 ms apart
    onsets = []
    for i in range(15, frames):
        prev = e[i - 15:i].mean()
        if e[i] > floor * 3 and e[i] > 2.0 * prev and (not onsets or i - onsets[-1] > 8):
            onsets.append(i)
    print(f"onsets (energy rises of 6 dB or more): {len(onsets)}:", " ".join(f"{i / 100:.2f}" for i in onsets[:60]))
    for t in events:
        near = [i / 100 for i in onsets if -0.05 <= i / 100 - t <= 0.45]
        a = int(max(0, (t - 0.3)) * 100)
        b = int(min(frames, (t + 0.5) * 100))
        before = db(e[a:int(t * 100)].mean()) if int(t * 100) > a else float("nan")
        after = db(e[int(t * 100):b].max()) if b > int(t * 100) else float("nan")
        tr = [i / 100 for i in hits if -0.10 <= i / 100 - t <= 0.50]
        print(f"event at {t:.2f} s: onsets within -0.05 .. +0.45 s: {near or 'none'}; transients above 4 kHz within -0.10 .. +0.50 s: {tr or 'none'}; level before {before:.1f} dBFS, peak after {after:.1f} dBFS")
    if events:
        # control: how often does a transient fall into a window of that length by chance
        span = frames / 100
        window = 0.6
        print(f"control: {len(hits)} transients in {span:.1f} s: a random {window} s window holds one with probability about {min(1.0, len(hits) * window / max(span, 1e-9)):.2f}")


if __name__ == "__main__":
    main()

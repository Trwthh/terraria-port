"""python pacing_report.py <frames.tsv> [last_n_frames=3600] : where the time of a frame goes (TERRARIA_FRAME_LOG).
spacing   = main-thread time between the ends of two shell ticks (what a player sees as the frame interval)
tick      = the whole shell tick (input, Update, Draw, the game's EndDraw wait, outputs, collector slice)
end_draw  = the game's own wait in Main.EndDraw (it waits until 1/60 s after the previous frame)
outside   = spacing - tick: Bevy between two ticks (extract, waiting for the render thread = present, message pump)"""
import sys


def q(v, f):
    return v[int((len(v) - 1) * f)]


def line(name, v):
    v = sorted(v)
    return f"{name}: mean {sum(v) / len(v):7.3f}  p1 {q(v, 0.01):7.3f}  p50 {q(v, 0.5):7.3f}  p99 {q(v, 0.99):7.3f}  max {v[-1]:8.3f} ms"


def main():
    rows = [l.split("\t") for l in open(sys.argv[1]).read().splitlines()[1:]]
    rows = [r for r in rows if len(r) >= 11]
    n = int(sys.argv[2]) if len(sys.argv) > 2 else 3600
    rows = rows[-n:]
    t = [float(r[1]) for r in rows]
    spacing = [b - a for a, b in zip(t, t[1:])]
    tick = [float(r[10]) for r in rows[1:]]
    end_draw = [float(r[9]) for r in rows[1:]]
    outside = [s - k for s, k in zip(spacing, tick)]
    secs = (t[-1] - t[0]) / 1000
    good = sum(1 for v in spacing if 15.67 <= v <= 17.67)
    print(f"{len(spacing)} frames in {secs:.1f} s = {len(spacing) / secs:.2f} frames/s; ticks {rows[0][0]}..{rows[-1][0]}; {100 * good / len(spacing):.1f} % of spacings within 16.67 +- 1 ms; {sum(1 for v in spacing if v > 25)} above 25 ms, {sum(1 for v in spacing if v > 33.4)} above 33.4 ms")
    print(line("spacing ", spacing))
    print(line("tick    ", tick))
    print(line("end_draw", end_draw))
    print(line("work    ", [k - e for k, e in zip(tick, end_draw)]))
    print(line("outside ", outside))
    late = [o for o, e in zip(outside, end_draw) if e < 0.05]
    print(f"frames in which EndDraw did not have to wait (the frame came late): {len(late)} of {len(spacing)}" + (f"; time outside the tick in those: mean {sum(late) / len(late):.2f} ms" if late else ""))


if __name__ == "__main__":
    main()

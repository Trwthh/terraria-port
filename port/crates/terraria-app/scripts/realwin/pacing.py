"""Frame pacing of the presenting window: python pacing.py [seconds=60] [tag]
In a world, focus on. Half of the time idle, half with input (a held key with repeat messages and mouse moves).
Reads the shell's TERRARIA_FRAME_LOG (interval between shell ticks = Bevy frames, one draw each) and the game's own
update counter (Main.GameUpdateCount) through the command file; prints mean / p50 / p99 / max and the rates."""
import ctypes
import ctypes.wintypes as wt
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import rw  # noqa: E402
import rwlib  # noqa: E402


def cpu_seconds(pid):
    h = rw.k32.OpenProcess(0x1000, False, pid)
    c, e, k, u = wt.FILETIME(), wt.FILETIME(), wt.FILETIME(), wt.FILETIME()
    rw.k32.GetProcessTimes(h, ctypes.byref(c), ctypes.byref(e), ctypes.byref(k), ctypes.byref(u))
    rw.k32.CloseHandle(h)
    f = lambda t: ((t.dwHighDateTime << 32) | t.dwLowDateTime) / 1e7
    return f(k) + f(u)


def frames(path):
    rows = []
    for l in open(path).read().splitlines()[1:]:
        p = l.split("\t")
        if len(p) >= 9:
            rows.append((int(p[0]), float(p[1]), float(p[2]), int(p[3]), int(p[4]), int(p[5])))
    return rows


def stats(rows, a, b, label, u0, u1, cpu):
    sel = [r for r in rows if a <= r[0] <= b]
    iv = sorted(r[2] for r in sel)
    n = len(iv)
    secs = (sel[-1][1] - sel[0][1]) / 1000.0
    q = lambda f: iv[int((n - 1) * f)]
    over = sum(1 for v in iv if v > 25.0)
    print(f"{label}: {n} frames in {secs:.1f} s = {n / secs:.2f} frames/s; interval mean {sum(iv) / n:.3f} ms, p50 {q(0.5):.3f}, p99 {q(0.99):.3f}, max {iv[-1]:.3f}; {over} intervals above 25 ms; "
          f"draws {sum(r[4] for r in sel) / secs:.2f}/s; game updates {(u1 - u0) / secs:.2f}/s; process CPU {cpu / secs * 100:.0f} % of one core")


def main():
    secs = float(sys.argv[1]) if len(sys.argv) > 1 else 60.0
    st, win = rw.window_or_exit()
    S = rwlib.state
    path = os.path.join(rw.LOGS, f"rw_{st['name']}.frames.tsv")
    half = secs / 2

    def mark():
        w = S("dump world")
        time.sleep(0.6)  # the frame log is flushed every 30 ticks
        return frames(path)[-1][0], w["gameUpdateCount"], cpu_seconds(st["pid"])

    a = mark()
    time.sleep(half)
    b = mark()
    t0 = time.time()
    k = "d"
    while time.time() - t0 < half:
        # what a player does: a direction held (repeat messages), the mouse moving all the time
        win.keydown(k)
        t1 = time.time()
        nxt = t1 + 0.5
        i = 0
        while time.time() - t1 < 2.0:
            if time.time() >= nxt:
                win.keydown(k, repeat=True)
                nxt += 1 / 30
            i += 1
            win.move(400 + (i * 7) % 500, 250 + (i * 3) % 200)
            time.sleep(0.008)
        win.keyup(k)
        k = "a" if k == "d" else "d"
    c = mark()
    rows = frames(path)
    stats(rows, a[0], b[0], "idle     ", a[1], b[1], b[2] - a[2])
    stats(rows, b[0], c[0], "input    ", b[1], c[1], c[2] - b[2])
    stats(rows, a[0], c[0], "all      ", a[1], c[1], c[2] - a[2])


if __name__ == "__main__":
    main()

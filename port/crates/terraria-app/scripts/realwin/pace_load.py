"""Frame pacing with and without a synthetic CPU load: python pace_load.py [seconds=60] [tag]

Needs a client started by `rw.py launch`, in a world, with the message-level focus on. Two phases of `seconds`
each, both with player-like input (a direction key held with repeat messages, the mouse moving):
  quiet   nothing else is started (the machine as it is: its overall CPU use is measured and printed)
  loaded  N-1 busy-loop processes at normal priority (N = logical processors), started here and killed by PID
          afterwards (verified)
Per phase, from the shell's TERRARIA_FRAME_LOG: the spacing of the shell ticks (mean / p50 / p99 / max, share within
16.67 +- 1 ms, counts above 25 and 33.4 ms), the game's own update rate (Main.GameUpdateCount through the command
file), the time of the tick spent in the game's EndDraw wait and outside the tick, the client's CPU use and the CPU
use of the whole machine (GetSystemTimes). Nothing here touches the mouse, the keyboard focus or the screen.
"""
import ctypes
import ctypes.wintypes as wt
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import rw  # noqa: E402
import rwlib  # noqa: E402
from pacing import cpu_seconds  # noqa: E402

k32 = rw.k32


def system_times():
    i, k, u = wt.FILETIME(), wt.FILETIME(), wt.FILETIME()
    k32.GetSystemTimes(ctypes.byref(i), ctypes.byref(k), ctypes.byref(u))
    f = lambda t: ((t.dwHighDateTime << 32) | t.dwLowDateTime) / 1e7
    return f(i), f(k) + f(u)  # idle, kernel (includes idle) + user


def rows_of(path):
    out = []
    for l in open(path).read().splitlines()[1:]:
        p = l.split("\t")
        if len(p) >= 11:
            out.append((int(p[0]), float(p[1]), int(p[3]), int(p[4]), float(p[9]), float(p[10]), int(p[11]) if len(p) > 11 else 0))
    return out


def play(win, seconds):
    t0 = time.time()
    k = "d"
    while time.time() - t0 < seconds:
        win.keydown(k)
        t1 = time.time()
        nxt = t1 + 0.5
        i = 0
        while time.time() - t1 < 2.0 and time.time() - t0 < seconds:
            if time.time() >= nxt:
                win.keydown(k, repeat=True)
                nxt += 1 / 30
            i += 1
            win.move(400 + (i * 7) % 500, 250 + (i * 3) % 200)
            time.sleep(0.008)
        win.keyup(k)
        k = "a" if k == "d" else "d"


def report(label, rows, a, b, out):
    sel = [r for r in rows if a["tick"] <= r[0] <= b["tick"]]
    t = [r[1] for r in sel]
    sp = sorted(y - x for x, y in zip(t, t[1:]))
    n = len(sp)
    secs = (t[-1] - t[0]) / 1000.0
    q = lambda f: sp[int((n - 1) * f)]
    good = sum(1 for v in sp if 15.67 <= v <= 17.67)
    end_draw = [r[4] for r in sel[1:]]
    tick = [r[5] for r in sel[1:]]
    spacing = [y - x for x, y in zip(t, t[1:])]
    outside = sorted(s - k for s, k in zip(spacing, tick))
    work = sorted(k - e for k, e in zip(tick, end_draw))
    late = sum(1 for e in end_draw if e < 0.05)
    idle = b["sys"][0] - a["sys"][0]
    total = b["sys"][1] - a["sys"][1]
    line = (f"{label}: {n} frames in {secs:.1f} s; spacing mean {sum(sp) / n:.3f} ms, p50 {q(0.5):.3f}, p99 {q(0.99):.3f}, max {sp[-1]:.3f}; "
            f"{100 * good / n:.1f} % within 16.67 +- 1 ms; {sum(1 for v in sp if v > 25)} above 25 ms, {sum(1 for v in sp if v > 33.4)} above 33.4 ms; "
            f"game updates {(b['updates'] - a['updates']) / secs:.2f}/s, draws {sum(r[3] for r in sel) / secs:.2f}/s; "
            f"work in the tick (without the EndDraw wait) p50 {work[len(work) // 2]:.2f} p99 {work[int((len(work) - 1) * 0.99)]:.2f} ms; "
            f"outside the tick p50 {outside[len(outside) // 2]:.2f} p99 {outside[int((len(outside) - 1) * 0.99)]:.2f} ms; "
            f"frames that came late (EndDraw did not wait) {late}; client CPU {(b['cpu'] - a['cpu']) / secs * 100:.0f} % of one core; "
            f"machine CPU {100 * (1 - idle / total):.0f} % of {os.cpu_count()} logical processors")
    print(line)
    out.write(line + "\n")
    # a frame log with a 12th column that changes in blocks of 600 ticks (an experiment build that alternates
    # between two variants, so both see the same machine load): the same numbers per variant; the first 60 ticks
    # of a block are left out (the switch itself)
    modes = sorted(set(r[6] for r in sel))
    if len(modes) > 1:
        for m in modes:
            sp_m, upd, secs_m = [], 0, 0.0
            for x, y in zip(sel, sel[1:]):
                if x[6] == m and y[6] == m and x[0] % 600 >= 60:
                    sp_m.append(y[1] - x[1])
                    upd += y[2]
                    secs_m += (y[1] - x[1]) / 1000.0
            sp_m.sort()
            k = len(sp_m)
            if k:
                line = (f"    {label.strip()} / variant {m}: {k} frames in {secs_m:.1f} s; spacing mean {sum(sp_m) / k:.3f} ms, p50 {sp_m[k // 2]:.3f}, p99 {sp_m[int((k - 1) * 0.99)]:.3f}, max {sp_m[-1]:.3f}; "
                        f"{100 * sum(1 for v in sp_m if 15.67 <= v <= 17.67) / k:.1f} % within 16.67 +- 1 ms; {sum(1 for v in sp_m if v > 25)} above 25 ms, {sum(1 for v in sp_m if v > 33.4)} above 33.4 ms; updates {upd / secs_m:.2f}/s")
                print(line)
                out.write(line + "\n")


def main():
    secs = float(sys.argv[1]) if len(sys.argv) > 1 else 60.0
    tag = sys.argv[2] if len(sys.argv) > 2 else "in_pacing"
    st, win = rw.window_or_exit()
    path = os.path.join(rw.LOGS, f"rw_{st['name']}.frames.tsv")
    out = open(os.path.join(rw.LOGS, tag + ".txt"), "a", encoding="utf-8")
    out.write(f"# {time.strftime('%Y-%m-%d %H:%M:%S')} client pid {st['pid']} ({st['name']}), {secs:.0f} s per phase\n")

    def mark():
        w = rwlib.state("dump world")
        time.sleep(0.6)  # the frame log is flushed every 30 ticks
        return {"tick": rows_of(path)[-1][0], "updates": w["gameUpdateCount"], "cpu": cpu_seconds(st["pid"]), "sys": system_times()}

    a = mark()
    play(win, secs)
    b = mark()
    n = max(1, (os.cpu_count() or 2) - 1)
    burners = [subprocess.Popen([sys.executable, "-c", "while True: pass"], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                                creationflags=subprocess.CREATE_NO_WINDOW) for _ in range(n)]
    try:
        time.sleep(1.0)
        c = mark()
        play(win, secs)
        d = mark()
    finally:
        for p in burners:
            p.kill()
        for p in burners:
            p.wait()
    alive = [p.pid for p in burners if p.poll() is None]
    print(f"load: {n} busy-loop processes at normal priority, pids {[p.pid for p in burners]}; still running after kill: {alive or 'none'}")
    rows = rows_of(path)
    report("quiet ", rows, a, b, out)
    report("loaded", rows, c, d, out)
    out.close()


if __name__ == "__main__":
    main()

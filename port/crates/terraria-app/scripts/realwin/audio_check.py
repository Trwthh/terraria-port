"""The one audio check on the real output device (work/realwin/REPORT.md, "Audio").

    python audio_check.py title      the title menu: music, plus hovering menu entries (menu tick)
    python audio_check.py ingame     in a world (music volume 0 in the game's config): digging with the pickaxe

The client must have been launched with `rw.py launch --volume-file` (device open, output gain 0). This script
records the default render endpoint with tools/loopback_capture, sets the gain to 0.03 for the audible part
(title: 9 s, ingame: 10 s; 19 s together, the limit is 20), acts with window messages at known times, sets the
gain back to 0 and prints the analysis (audio_analyze.py). Nobody listens: the capture is the evidence.
"""
import os
import re
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import rw  # noqa: E402
import rwlib  # noqa: E402

REC = os.path.join(rw.PROJECT, "tools", "loopback_capture", "target", "release", "loopback_capture.exe")


def main():
    phase = sys.argv[1]
    win = rwlib.attach()
    S = rwlib.state
    wav = os.path.join(rw.LOGS, f"rw_audio_{phase}.wav")
    audible = 9.0 if phase == "title" else 10.0
    total = audible + 4.0
    events = []
    if phase == "ingame":
        win.key("2")  # the pickaxe
        time.sleep(0.3)
        p = rwlib.player()
        fx, fy = rwlib.feet_tile(p)
        targets = [(fx + dx, fy) for dx in (2, -2, 1, -1, 3) if rwlib.tile(fx + dx, fy)["active"]][:2]
        aims = [rwlib.aim_tile(*t) for t in targets]
        print("dig targets", targets, "at", aims)
    rec = subprocess.Popen([REC, str(total), wav], stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
                           creationflags=subprocess.CREATE_NO_WINDOW)
    t_spawn = time.time()

    def at(t):  # wait until t seconds after the recorder was started
        d = t_spawn + t - time.time()
        if d > 0:
            time.sleep(d)

    def mark(name):
        events.append((time.time() - t_spawn, name))

    at(1.5)
    rw.run(win, "volume 0.03")
    mark("gain 0.03")
    if phase == "title":
        items = S("dump menu")["items"]
        for k, t in ((1, 4.0), (3, 5.5), (5, 7.0), (2, 8.5)):
            at(t)
            win.move(int(items[k][0]), int(items[k][1]))  # one WM_MOUSEMOVE onto another menu entry
            mark(f"hover menu entry {k}")
    else:
        # sound effects only (the music and ambient volumes are 0 in the game's config for this run)
        for key, t, name in (("3", 2.5, "key 3: hotbar slot change (menu tick)"), ("2", 3.3, "key 2: hotbar slot change (menu tick)"),
                             ("escape", 4.1, "Esc: inventory opens (menu open sound)"), ("escape", 5.1, "Esc: inventory closes (menu close sound)")):
            at(t)
            win.keydown(key)
            mark(name)
            time.sleep(0.07)
            win.keyup(key)
        for (x, y), t in zip(aims, (6.3, 8.9)):
            at(t - 0.3)
            win.move(x, y)
            at(t)
            win.down()
            mark("left button down on a tile (pickaxe: swing and dig sounds while held)")
            time.sleep(1.3)
            win.up()
            mark("left button up")
    at(1.5 + audible)
    rw.run(win, "volume 0")
    mark("gain 0")
    out = rec.communicate(timeout=60)[0]
    print(out.strip())
    m = re.search(r"first packet after Some\(([0-9.]+)\)", out)
    lag = float(m.group(1)) if m else 0.0
    print(f"audible for {audible} s at gain 0.03; recorder start-up lag {lag:.2f} s")
    for t, name in events:
        print(f"  {t - lag:6.2f} s  {name}")
    times = [f"{t - lag:.2f}" for t, name in events if "gain" not in name and "button up" not in name]
    subprocess.run([sys.executable, os.path.join(os.path.dirname(os.path.abspath(__file__)), "audio_analyze.py"), wav] + times)


if __name__ == "__main__":
    main()

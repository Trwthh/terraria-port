"""The game's window modes against a fake monitor (TERRARIA_FAKE_MONITOR), with real window messages only.

    python window_modes.py title      a fresh save root: the default borderless start, Alt+Enter, the video menu's
                                      borderless / resolution / fullscreen switches, then into the world for the HUD
    python window_modes.py maximized  second start on the same save root with WindowMaximized in config.json: the
                                      maximised window, then the switch to borderless (the game restores it first)

The client is started by this script through rw.py (no activation, outside the desktop, muted, loopback only, the
guard of rw.py for the whole run) with RW_SAVE = work\\port-save-in-window and TERRARIA_FAKE_MONITOR=auto: the game is
told about ONE monitor of 1920 x 1080 that lies below and right of the real desktop. At every step the window is
asked what it is (GetWindowRect / GetClientRect / GetWindowLong through `rw.style_of`), the game is asked what it
thinks (`dump input`), and a screenshot is taken. One JSON line per step goes to work\\logs\\in_window_<phase>.jsonl;
the script fails (exit code 1) when the window ever touches the desktop or becomes the foreground window, and
prints which expectations did not hold.
"""
import json
import os
import shutil
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
PROJECT = os.path.abspath(os.path.join(HERE, "..", "..", "..", "..", ".."))
SAVE = os.path.join(PROJECT, "work", "port-save-in-window")
os.environ["RW_SAVE"] = SAVE
os.environ.setdefault("PYTHONIOENCODING", "utf-8")
sys.path.insert(0, HERE)
import rw  # noqa: E402
import rwlib  # noqa: E402

S = rwlib.state
PROBLEMS = []
MONITOR = None  # the fake monitor: x, y, w, h


def launch(name):
    out = subprocess.run([sys.executable, os.path.join(HERE, "rw.py"), "launch", "--name", name, "TERRARIA_NO_GAMEPAD=1", "TERRARIA_FAKE_MONITOR=auto"], capture_output=True, text=True)
    line = [l for l in out.stdout.splitlines() if l.startswith("{")]
    if not line:
        sys.exit(f"launch failed: {out.stdout} {out.stderr}")
    info = json.loads(line[-1])
    vs = info["virtual_screen"]
    global MONITOR
    MONITOR = (vs[0] + vs[2] + 240, vs[1] + vs[3] + 160, 1920, 1080)
    return info


def step(log, win, tag, note, expect=None):
    """One record: the window as the system sees it, the game's own view, a screenshot. `expect`: checks by name."""
    time.sleep(1.2)
    st = rw.style_of(win.h)
    g = S("dump input")
    rw.shot(win.h, f"in_win_{tag}", 2)
    rec = {"step": tag, "note": note, "window": st["window"], "client": st["client"], "borderless": st["borderless"], "style": st["set"], "iconic": st["iconic"],
           "touches_desktop": st["touches_desktop"], "foreground_is_client": st["foreground_is_client"],
           "game": {k: g.get(k) for k in ("backBuffer", "screen", "display", "isFullScreen", "screenBorderless", "screenMaximized", "formWindowState", "formBorderStyle", "formBounds", "uiScale", "gameIsActive")}}
    checks = {}
    for name, want in (expect or {}).items():
        # the back buffer is the game's resolution (Main.screenWidth / Height are divided by the UI scale while a menu is drawn)
        got = {"window": st["window"], "client": st["client"], "borderless": st["borderless"], "screen": g.get("backBuffer"), "isFullScreen": g.get("isFullScreen"),
               "screenBorderless": g.get("screenBorderless"), "formWindowState": g.get("formWindowState"), "formBorderStyle": g.get("formBorderStyle"), "outer": st["window"][2:]}[name]
        # the outer rectangle may differ by a pixel (the frame the system gives a window is not always the one measured before)
        ok = got == want or (name == "window" and all(abs(a - b) <= 1 for a, b in zip(got, want)))
        checks[name] = {"want": want, "got": got, "ok": ok}
        if not ok:
            PROBLEMS.append(f"{tag}: {name} is {got}, expected {want}")
    rec["checks"] = checks
    if st["touches_desktop"]:
        PROBLEMS.append(f"{tag}: THE WINDOW TOUCHES THE DESKTOP: {st['window']}")
    if st["foreground_is_client"]:
        PROBLEMS.append(f"{tag}: THE CLIENT IS THE FOREGROUND WINDOW")
    log.write(json.dumps(rec) + "\n")
    log.flush()
    print(f"{tag}: window {st['window']} client {st['client']} borderless={st['borderless']} | game back buffer {g.get('backBuffer')} fullScreen={g.get('isFullScreen')} "
          f"borderless={g.get('screenBorderless')} formState={g.get('formWindowState')} | {note}" + ("" if all(c["ok"] for c in checks.values()) else "  <-- NOT AS EXPECTED"))
    return rec


def menu_click(win, k, wait=0.9):
    m = S(f"aim menu {k}")
    x, y = m["mouse"]
    win.click(int(x), int(y))
    time.sleep(wait)


def menu_mode():
    return S("dump menu")["menuMode"]


def wait_mode(mode, timeout=20):
    t0 = time.time()
    while time.time() - t0 < timeout:
        if menu_mode() == mode:
            return True
        time.sleep(0.3)
    PROBLEMS.append(f"menu mode {mode} was not reached (now {menu_mode()})")
    return False


def alt_enter(win):
    win.keydown("alt")
    time.sleep(0.05)
    win.keydown("enter")
    time.sleep(0.12)
    win.keyup("enter")
    time.sleep(0.05)
    win.keyup("alt")
    time.sleep(0.3)


def to_resolution_menu(win):
    """Title -> Settings -> Video -> Resolution (menu modes 0 -> 11 -> 1111 -> 111)."""
    if menu_mode() == 0:
        menu_click(win, 4)
        wait_mode(11)
    if menu_mode() == 11:
        menu_click(win, 2)
        wait_mode(1111)
    if menu_mode() == 1111:
        menu_click(win, 0)
        wait_mode(111)


def finish(log, name):
    out = subprocess.run([sys.executable, os.path.join(HERE, "rw.py"), "kill"], capture_output=True, text=True).stdout.strip()
    print(out)
    time.sleep(1.0)
    fg = open(os.path.join(rw.LOGS, f"rw_{name}.foreground.log"), encoding="utf-8").read().strip().splitlines()
    print(fg[-1])
    log.write(json.dumps({"step": "end", "kill": out, "foreground_log_end": fg[-1], "problems": PROBLEMS}) + "\n")
    log.close()
    if PROBLEMS:
        print("NOT AS EXPECTED:")
        for p in PROBLEMS:
            print("  " + p)
    sys.exit(1 if any("DESKTOP" in p or "FOREGROUND" in p for p in PROBLEMS) else 0)


def phase_title():
    old = os.path.join(PROJECT, "work", "port-save-runs", "_old")
    os.makedirs(old, exist_ok=True)
    if os.path.isdir(SAVE):
        shutil.move(SAVE, os.path.join(old, "in-window-" + time.strftime("%Y%m%d-%H%M%S")))
    os.makedirs(os.path.join(SAVE, "Worlds"))
    shutil.copy(os.path.join(PROJECT, "work", "refsave", "Worlds", "ref_small.wld"), os.path.join(SAVE, "Worlds", "ref_small.wld"))
    info = launch("in_window")
    mx, my, mw, mh = MONITOR
    log = open(os.path.join(rw.LOGS, "in_window_title.jsonl"), "w", encoding="utf-8")
    log.write(json.dumps({"step": "launch", "info": info, "fake_monitor": MONITOR}) + "\n")
    st, win = rw.window_or_exit()
    win.focus(True)
    time.sleep(1.5)
    full = {"window": [mx, my, mw, mh], "client": [mw, mh], "borderless": True, "screen": [mw, mh]}
    step(log, win, "01_default_start", "first start: the game's default is a borderless window with the monitor's size (language select)", dict(full, isFullScreen=False, screenBorderless=True))
    menu_click(win, 0, 1.5)  # English
    wait_mode(0)
    step(log, win, "02_title_borderless", "title menu, borderless over the fake monitor", dict(full, isFullScreen=False))
    alt_enter(win)
    step(log, win, "03_alt_enter_fullscreen", "Alt+Enter: full screen = a borderless window over the monitor", dict(full, isFullScreen=True))
    alt_enter(win)
    step(log, win, "04_alt_enter_back", "Alt+Enter again: back to the borderless window", dict(full, isFullScreen=False, screenBorderless=True))
    # the video menu: borderless off, Apply -> a framed window
    to_resolution_menu(win)
    step(log, win, "05_resolution_menu", "Settings > Video > Resolution")
    menu_click(win, 1)  # Borderless: toggles the pending state
    menu_click(win, 3)  # Apply (back to the video menu)
    wait_mode(1111)
    step(log, win, "06_windowed_framed", "borderless off + Apply: a framed window (the game picks its first display mode)", {"borderless": False, "screenBorderless": False, "isFullScreen": False, "formBorderStyle": 4})
    # another resolution: cycle the entry until 1280x720 is pending, Apply
    menu_click(win, 0)
    wait_mode(111)
    for _ in range(40):
        g = S("dump menu")
        w = S("dump input")
        # the pending resolution is not in a dump: cycle and apply, then look at the screen size
        menu_click(win, 0, 0.35)
        menu_click(win, 3, 0.9)
        wait_mode(1111)
        scr = S("dump input")["screen"]
        if scr == [1280, 720]:
            break
        menu_click(win, 0)
        wait_mode(111)
    step(log, win, "07_windowed_1280x720", "resolution 1280x720 + Apply: a framed window with that client size", {"client": [1280, 720], "borderless": False, "screen": [1280, 720]})
    # fullscreen from the menu, and back
    menu_click(win, 0)
    wait_mode(111)
    menu_click(win, 2, 1.5)  # Go Fullscreen
    step(log, win, "08_menu_fullscreen", "the menu's fullscreen switch: a borderless window over the monitor", dict(full, isFullScreen=True))
    menu_click(win, 2, 1.5)  # Go Windowed
    step(log, win, "09_menu_windowed_again", "back to windowed: the framed 1280x720 window again", {"client": [1280, 720], "borderless": False, "isFullScreen": False, "screen": [1280, 720]})
    # borderless on again
    if menu_mode() != 111:
        to_resolution_menu(win)
    menu_click(win, 1)
    menu_click(win, 3)
    wait_mode(1111)
    step(log, win, "10_borderless_again", "borderless on + Apply: over the monitor again", dict(full, screenBorderless=True))
    # back to the title, create a player, enter the world: the HUD at the monitor's size
    win.key("escape")
    time.sleep(0.6)
    win.key("escape")
    time.sleep(0.6)
    wait_mode(0)
    menu_click(win, 0, 1.5)  # Single Player
    ui = S("dump ui")
    scale = ui.get("uiScale", 1.0)
    new = [p for p in ui.get("snapPoints", []) if p["name"] == "New"]
    # the "New" button is the right one of the two at the bottom (the snap point named New is the left, "Back")
    els = [e for e in ui.get("elements", []) if e[0] == 2 and e[5] == 50.0]
    bx = max(els, key=lambda e: e[2])
    win.click(int((bx[2] + bx[4] / 2) * scale), int((bx[3] + bx[5] / 2) * scale))
    time.sleep(1.5)
    ui = S("dump ui")
    step(log, win, "11_character_creation", f"character creation at the monitor's size ({ui.get('state')})")
    # "Create" is the right button of the bottom row of the creation panel
    row = [e for e in ui.get("elements", []) if e[4] > 100 and e[5] >= 40]
    low = max(e[3] for e in row)
    create = max((e for e in row if abs(e[3] - low) < 2), key=lambda e: e[2])
    win.click(int((create[2] + create[4] / 2) * scale), int((create[3] + create[5] / 2) * scale))
    time.sleep(1.2)
    if S("dump ui").get("state") != "UIVirtualKeyboard":
        PROBLEMS.append("the name prompt did not open")
    win.text("Winda")
    time.sleep(0.4)
    win.key("enter")
    time.sleep(2.0)
    ui = S("dump ui")
    play = [p for p in ui.get("snapPoints", []) if p["name"] == "Play"]
    win.click(int(play[0]["at"][0] * scale), int(play[0]["at"][1] * scale))
    time.sleep(1.5)
    ui = S("dump ui")
    play = [p for p in ui.get("snapPoints", []) if p["name"] == "Play"]
    win.click(int(play[0]["at"][0] * scale), int(play[0]["at"][1] * scale))
    t0 = time.time()
    while time.time() - t0 < 60 and S("dump world").get("gameMenu", True):
        time.sleep(0.5)
    time.sleep(3.0)
    step(log, win, "12_world_borderless_1920x1080", "in the world: the HUD laid out for the monitor's size", dict(full))
    win.key("escape")  # inventory open: the whole HUD
    step(log, win, "13_world_inventory", "inventory, equipment column, minimap at 1920x1080")
    win.key("escape")
    time.sleep(0.4)
    alt_enter(win)
    step(log, win, "14_world_fullscreen", "in the world, Alt+Enter: full screen", dict(full, isFullScreen=True))
    alt_enter(win)
    step(log, win, "15_world_back", "in the world, Alt+Enter again", dict(full, isFullScreen=False))
    finish(log, "in_window")


def phase_maximized():
    cfg = os.path.join(SAVE, "config.json")
    data = json.load(open(cfg, encoding="utf-8-sig"))
    before = {k: data.get(k) for k in ("WindowMaximized", "WindowBorderless", "Fullscreen", "DisplayWidth", "DisplayHeight")}
    data.update({"WindowMaximized": True, "WindowBorderless": False, "Fullscreen": False})
    json.dump(data, open(cfg, "w", encoding="utf-8"), indent=2)
    info = launch("in_window_max")
    mx, my, mw, mh = MONITOR
    log = open(os.path.join(rw.LOGS, "in_window_maximized.jsonl"), "w", encoding="utf-8")
    log.write(json.dumps({"step": "launch", "info": info, "fake_monitor": MONITOR, "config_before": before, "config_set": {"WindowMaximized": True, "WindowBorderless": False, "Fullscreen": False}}) + "\n")
    st, win = rw.window_or_exit()
    win.focus(True)
    time.sleep(1.5)
    wait_mode(0)
    step(log, win, "20_maximized_start", "WindowMaximized in config.json: the framed window fills the monitor (outer rectangle = monitor)", {"window": [mx, my, mw, mh], "borderless": False, "formWindowState": 2, "screenBorderless": False})
    to_resolution_menu(win)
    menu_click(win, 1)  # Borderless on
    menu_click(win, 3)  # Apply: SetDisplayMode restores the maximised form first, then goes borderless
    wait_mode(1111)
    step(log, win, "21_maximized_to_borderless", "borderless on + Apply from the maximised window", {"window": [mx, my, mw, mh], "client": [mw, mh], "borderless": True, "formWindowState": 0, "screenBorderless": True})
    menu_click(win, 0)
    wait_mode(111)
    menu_click(win, 1)  # Borderless off
    menu_click(win, 3)
    wait_mode(1111)
    step(log, win, "22_framed_after_restore", "borderless off + Apply: a framed, not maximised window", {"borderless": False, "formWindowState": 0})
    # into the world with the player of the first phase: the HUD in the small framed window, then after a resize
    win.key("escape")
    time.sleep(0.6)
    win.key("escape")
    time.sleep(0.6)
    wait_mode(0)
    menu_click(win, 0, 1.5)  # Single Player
    for _ in range(2):
        ui = S("dump ui")
        play = [q for q in ui.get("snapPoints", []) if q["name"] == "Play"]
        if not play:
            PROBLEMS.append(f"no Play button in {ui.get('state')}")
            break
        win.click(int(play[0]["at"][0] * ui.get("uiScale", 1.0)), int(play[0]["at"][1] * ui.get("uiScale", 1.0)))
        time.sleep(1.5)
    t0 = time.time()
    while time.time() - t0 < 60 and S("dump world").get("gameMenu", True):
        time.sleep(0.5)
    time.sleep(3.0)
    win.key("escape")  # the inventory: the whole HUD
    step(log, win, "23_world_800x600", "in the world, framed 800x600 window: the HUD with the inventory open", {"client": [800, 600], "borderless": False, "screen": [800, 600]})
    win.resize(1280, 720)  # a user's resize (SetWindowPos, no activation, no move)
    step(log, win, "24_world_1280x720", "the same after a resize of the window to 1280x720", {"client": [1280, 720], "borderless": False, "screen": [1280, 720]})
    finish(log, "in_window_max")


if __name__ == "__main__":
    {"title": phase_title, "maximized": phase_maximized}[sys.argv[1] if len(sys.argv) > 1 else "title"]()

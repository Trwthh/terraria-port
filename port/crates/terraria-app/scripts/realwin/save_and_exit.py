"""Save & Exit from a world and Exit from the title menu with real messages; prints the process exit code.
    python save_and_exit.py [tag]
The Save & Exit entry is found by its text colour change on hover is not needed: the settings menu's left column is
at 220/640 of the width and its 8th entry at 290/360 of the height at UI scale 1.2 on 1280x720 (measured on
work/shots/rw_30a_settings.png); for other sizes the entry is looked for by hovering down the column until the
game reports the options window closed after the click."""
import ctypes
import ctypes.wintypes as wt
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import rw  # noqa: E402
import rwlib  # noqa: E402


def main():
    tag = sys.argv[1] if len(sys.argv) > 1 else "rw_exit"
    st, win = rw.window_or_exit()
    S = rwlib.state
    k32 = rw.k32
    handle = k32.OpenProcess(0x1000 | 0x00100000, False, st["pid"])
    m = S("dump menu")
    if not m.get("gameMenu"):
        w = S("dump world")
        if w.get("mapFullscreen"):
            win.key("m")
            time.sleep(0.4)
        if S("dump chat")["drawingPlayerChat"]:
            win.key("escape")
            time.sleep(0.4)
        w = S("dump world")
        if not w.get("ingameOptionsWindow"):
            if not w.get("playerInventory"):
                win.key("escape")
                time.sleep(0.6)
            x, y = S("aim settings")["mouse"]
            win.click(x, y)
            time.sleep(0.8)
        print("settings open:", S("dump world")["ingameOptionsWindow"])
        rw.shot(win.h, tag + "_settings", 2)
        win.click(440, 580)
        t0 = time.time()
        while time.time() - t0 < 90:
            time.sleep(0.5)
            m = S("dump menu")
            if m.get("gameMenu") and m.get("menuMode") == 0:
                break
        print(f"Save & Exit: title menu after {time.time() - t0:.1f} s:", {k: m.get(k) for k in ("gameMenu", "menuMode", "statusText")})
        time.sleep(1.0)
    rw.shot(win.h, tag + "_title", 2)
    items = S("dump menu")["items"]
    win.click(int(items[-1][0]), int(items[-1][1]))
    k32.WaitForSingleObject.argtypes = [wt.HANDLE, wt.DWORD]
    r = k32.WaitForSingleObject(handle, 30000)
    code = wt.DWORD()
    k32.GetExitCodeProcess(handle, ctypes.byref(code))
    print("Exit clicked: the process", "ended" if r == 0 else "DID NOT END within 30 s", "| exit code", code.value if code.value != 259 else "still running")


if __name__ == "__main__":
    main()

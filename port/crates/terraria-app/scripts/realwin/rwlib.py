"""Helpers on top of rw.py for composite steps (read state through the command file, then act with messages)."""
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import rw  # noqa: E402


def attach():
    st, win = rw.window_or_exit()
    return win


def state(cmd):
    """The `state` object of the last JSON line a dump / aim command wrote."""
    out = rw.dump(cmd)
    for l in reversed(out.splitlines()):
        if l.startswith("{"):
            d = json.loads(l)
            return d.get("state", d)
    return {}


def player():
    return state("dump player")


def aim_tile(x, y):
    return tuple(state(f"aim tile {x} {y}")["mouse"])


def tile(x, y):
    return state(f"dump tile {x} {y}")


def feet_tile(p=None):
    p = p or player()
    return int((p["position"][0] + p["width"] / 2) // 16), int((p["position"][1] + p["height"]) // 16)


def hold_left_on_tile(win, x, y, seconds, button="left"):
    """Left button held on a tile; the aim is refreshed with WM_MOUSEMOVE while it is held (the camera follows the player)."""
    mx, my = aim_tile(x, y)
    win.glide(mx, my)
    time.sleep(0.1)
    win.down(button)
    t0 = time.time()
    while time.time() - t0 < seconds:
        time.sleep(0.25)
        mx, my = aim_tile(x, y)
        win.move(mx, my)
        if not tile(x, y)["active"]:
            break
    win.up(button)
    return time.time() - t0

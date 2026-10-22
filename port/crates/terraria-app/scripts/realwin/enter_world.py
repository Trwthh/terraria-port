"""From the title menu into the first world with the first player, by window messages: python enter_world.py
Needs a client started by `rw.py launch` (save root with a player and a world). Posts the activation messages
first (`focus on`), then clicks Single Player, the first player's Play button and the first world's Play button
(positions from the game's own snap points), and waits until the world is loaded."""
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import rw  # noqa: E402
import rwlib  # noqa: E402

S = rwlib.state


def play_button(win):
    ui = S("dump ui")
    scale = ui.get("uiScale", 1.0)
    play = [p for p in ui.get("snapPoints", []) if p["name"] == "Play"]
    if not play:
        sys.exit(f"no Play button in {ui.get('state')}")
    win.click(int(play[0]["at"][0] * scale), int(play[0]["at"][1] * scale))


def main():
    st, win = rw.window_or_exit()
    win.focus(True)
    time.sleep(1.0)
    # the title menu is up when it has recorded where its entries are (the splash comes first)
    t0 = time.time()
    while time.time() - t0 < 60:
        d = S("dump menu")
        if d.get("menuMode") == 0 and d.get("items"):
            break
        time.sleep(0.5)
    time.sleep(1.0)
    m = S("aim menu 0")["mouse"]
    win.click(int(m[0]), int(m[1]))
    time.sleep(1.5)
    play_button(win)
    time.sleep(1.5)
    play_button(win)
    t0 = time.time()
    while time.time() - t0 < 90:
        w = S("dump world")
        if not w.get("gameMenu", True):
            time.sleep(3.0)
            print(f"in the world {w.get('worldName')!r} after {time.time() - t0:.1f} s")
            return
        time.sleep(0.5)
    sys.exit("the world did not load within 90 s")


if __name__ == "__main__":
    main()

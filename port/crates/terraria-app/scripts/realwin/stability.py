"""Mixed play through window messages for N seconds, then Save & Exit and Exit from the title menu.

    python stability.py [seconds=600] [tag=rw_stab]

Needs a client started by `rw.py launch`, in a world, with the message-level focus on. Every action is a window
message (rw.py); the game's state is read through TERRARIA_COMMAND_FILE (dump lines) to choose targets and to log.
Writes work/logs/<tag>.jsonl (one line per 15 s: time, game updates, position, life, deaths, inventory size),
takes a screenshot every 2 minutes, and prints the exit code of the process at the end.
"""
import ctypes
import ctypes.wintypes as wt
import json
import os
import random
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import rw  # noqa: E402
import rwlib  # noqa: E402

S = rwlib.state


def main():
    seconds = float(sys.argv[1]) if len(sys.argv) > 1 else 600.0
    tag = sys.argv[2] if len(sys.argv) > 2 else "rw_stab"
    st, win = rw.window_or_exit()
    pid = st["pid"]
    k32 = rw.k32
    handle = k32.OpenProcess(0x1000 | 0x00100000, False, pid)  # QUERY_LIMITED_INFORMATION | SYNCHRONIZE
    rnd = random.Random(20261012)
    log = open(os.path.join(rw.LOGS, tag + ".jsonl"), "w", encoding="utf-8")
    t0 = time.time()
    w0 = S("dump world")
    counts = {}
    deaths = 0
    was_dead = False
    next_log = 0.0
    next_shot = 0.0
    fg_ours = 0

    def note(name):
        counts[name] = counts.get(name, 0) + 1

    def slot(n):
        return tuple(S(f"aim slot {n}")["mouse"])

    def close_everything():
        w = S("dump world")
        if w.get("mapFullscreen"):
            win.key("m")
            time.sleep(0.3)
        if S("dump chat")["drawingPlayerChat"]:
            win.key("escape")
            time.sleep(0.3)
        w = S("dump world")
        if w.get("ingameOptionsWindow") or w.get("playerInventory"):
            win.key("escape")
            time.sleep(0.3)

    while time.time() - t0 < seconds:
        if rw.exit_code(pid) is not None:
            print(f"THE CLIENT ENDED after {time.time() - t0:.0f} s with exit code {rw.exit_code(pid)}")
            break
        now = time.time() - t0
        p = rwlib.player()
        if p.get("dead") and not was_dead:
            deaths += 1
        was_dead = bool(p.get("dead"))
        if rw.fg_info()["pid"] == pid:
            fg_ours += 1
        if now >= next_log:
            w = S("dump world")
            line = {"t": round(now, 1), "gameUpdateCount": w["gameUpdateCount"], "updates_per_s": round((w["gameUpdateCount"] - w0["gameUpdateCount"]) / max(now, 1e-3), 2) if now > 1 else None,
                    "position": p["position"], "life": p["statLife"], "dead": p["dead"], "deaths": deaths, "items": len(p["inventory"]), "actions": dict(counts)}
            log.write(json.dumps(line) + "\n")
            log.flush()
            next_log += 15.0
        if now >= next_shot:
            close_everything()
            rw.shot(win.h, f"{tag}_{int(now) // 60:02d}min", 2)
            next_shot += 120.0
        if p.get("dead"):
            # mouse moves and a key while dead, then wait for the respawn
            win.glide(rnd.randint(50, 1230), rnd.randint(50, 670))
            time.sleep(1.0)
            continue
        a = rnd.choice(["walk", "walk", "walk", "jump", "hotbar_key", "hotbar_wheel", "dig", "dig", "sword", "sword", "inventory", "map", "chat", "mouse", "mouse", "right_click", "walk_jump"])
        note(a)
        if a == "walk":
            win.hold(rnd.choice("ad"), rnd.uniform(0.3, 2.0))
        elif a == "walk_jump":
            k = rnd.choice("ad")
            win.keydown(k)
            for _ in range(rnd.randint(1, 3)):
                time.sleep(0.25)
                win.keydown("space")
                time.sleep(0.2)
                win.keyup("space")
            win.keyup(k)
        elif a == "jump":
            win.keydown("space")
            time.sleep(rnd.uniform(0.1, 0.4))
            win.keyup("space")
        elif a == "hotbar_key":
            win.key(rnd.choice("1234567890"))
        elif a == "hotbar_wheel":
            win.wheel(rnd.choice((-2, -1, 1, 2)))
        elif a == "dig":
            win.key("2")
            time.sleep(0.15)
            fx, fy = rwlib.feet_tile(p)
            tx, ty = fx + rnd.choice((-2, -1, 1, 2)), fy + rnd.choice((0, 0, 1))
            rwlib.hold_left_on_tile(win, tx, ty, rnd.uniform(0.6, 2.0))
        elif a == "sword":
            win.key("1")
            time.sleep(0.1)
            e = S("aim enemy")
            if e.get("npc") is not None and "mouse" in e:
                win.glide(*e["mouse"])
            else:
                win.glide(640 + rnd.choice((-80, 80)), 340)
            win.down()
            time.sleep(rnd.uniform(0.5, 1.5))
            win.up()
        elif a == "inventory":
            win.key("escape")
            time.sleep(0.4)
            if S("dump world").get("playerInventory"):
                # pick an item up and put it back two slots further (or swap)
                items = [i["slot"] for i in rwlib.player()["inventory"] if 10 <= i["slot"] < 50]
                if items:
                    src = rnd.choice(items)
                    win.click(*slot(src))
                    time.sleep(0.2)
                    win.click(*slot(10 + (src - 10 + 2) % 40))
                    time.sleep(0.2)
                    if rwlib.player()["mouseItem"].get("type"):
                        win.click(*slot(src))  # a swap left the other item on the cursor: back to the source slot
                        time.sleep(0.2)
                for _ in range(3):
                    win.glide(rnd.randint(20, 560), rnd.randint(20, 300))
                    time.sleep(0.1)
            close_everything()
        elif a == "map":
            win.key("m")
            time.sleep(0.5)
            win.wheel(rnd.choice((-2, 2)), rnd.randint(300, 900), rnd.randint(200, 500))
            time.sleep(0.3)
            win.key("m")
            time.sleep(0.3)
            close_everything()
        elif a == "chat":
            win.key("enter")
            time.sleep(0.3)
            if S("dump chat")["drawingPlayerChat"]:
                win.text(rnd.choice(["stability run", "still here", "/playing", "tick tock", "abc 123"]), dt=0.03)
                win.key("backspace")
                win.key(rnd.choice(("enter", "numenter")))
                time.sleep(0.3)
            close_everything()
        elif a == "mouse":
            for _ in range(rnd.randint(3, 10)):
                win.glide(rnd.randint(0, 1279), rnd.randint(0, 719), steps=8)
            if rnd.random() < 0.3:
                rw.run(win, "leave", quiet=True)
        elif a == "right_click":
            win.click(rnd.randint(400, 880), rnd.randint(250, 400), "right")
        time.sleep(rnd.uniform(0.05, 0.3))

    played = time.time() - t0
    if rw.exit_code(pid) is None:
        close_everything()
        w1 = S("dump world")
        p = rwlib.player()
        print(f"played {played:.0f} s: {w1['gameUpdateCount'] - w0['gameUpdateCount']} game updates = {(w1['gameUpdateCount'] - w0['gameUpdateCount']) / played:.2f}/s; deaths {deaths}; actions {counts}; foreground samples that were the client: {fg_ours}")
        print("inventory before Save & Exit:", [(i["slot"], i["item"]["name"], i["item"]["stack"]) for i in p["inventory"]])
        rw.shot(win.h, f"{tag}_end", 2)
        # Save & Exit: Esc (inventory), the Settings button, the Save & Exit entry of the settings menu
        win.key("escape")
        time.sleep(0.6)
        x, y = S("aim settings")["mouse"]
        win.click(x, y)
        time.sleep(0.8)
        w = S("dump world")
        # IngameOptions.Draw: left column anchored at (W/2 - 335 + 167.5, H/2 - 240 + 50) in UI pixels, 47.5 per entry, Save & Exit is the 8th
        us = w["uiScale"]
        sx, sy = (w["screenWidth"] / 2 - 335 + 167.5) * us, (w["screenHeight"] / 2 - 240 + 50 + 47.5 * 7) * us
        print("settings open:", w["ingameOptionsWindow"], "| Save & Exit at", (round(sx), round(sy)))
        win.click(int(sx), int(sy))
        for _ in range(120):
            time.sleep(0.5)
            m = S("dump menu")
            if m.get("gameMenu") and m.get("menuMode") == 0:
                break
        print("after Save & Exit:", {k: m.get(k) for k in ("gameMenu", "menuMode", "statusText")})
        time.sleep(1.0)
        rw.shot(win.h, f"{tag}_title", 2)
        # Exit: the last entry of the title menu
        items = S("dump menu")["items"]
        win.click(int(items[-1][0]), int(items[-1][1]))
        k32.WaitForSingleObject.argtypes = [wt.HANDLE, wt.DWORD]
        r = k32.WaitForSingleObject(handle, 30000)
        code = wt.DWORD()
        k32.GetExitCodeProcess(handle, ctypes.byref(code))
        print("Exit clicked: the process", "ended" if r == 0 else "DID NOT END within 30 s", "| exit code", code.value if code.value != 259 else "still running")
    log.close()


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Writes the scripted play sessions of the translated client (stage B of the in-game work).

    python make_play_script.py            writes g7_play.input next to this file

The session starts like g6_enter_world.input (first run: language, title, new player "Porter", ref_small) and then
plays: walk, jump, dig, place, chop a tree, inventory, crafting, night + combat, map, settings, save and exit.
Tile coordinates are those of work/refsave/Worlds/ref_small.wld (spawn tile 2095,232; the tree at x 2128).
Ticks are shell ticks; the world is loaded at tick 2052.

Every step writes state dumps (work/logs/<name>.state.jsonl); tools/check_play.py reads them and says what
happened. Change a step here, never in the generated .input file.
"""
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
SHOTS = "../work/shots"


def menu_prefix():
    """g6_enter_world.input up to the click that loads the world (its screenshots and its quit left out)."""
    out = []
    for line in open(os.path.join(HERE, "g6_enter_world.input"), encoding="utf-8").read().splitlines():
        if line.startswith("frame") and (" shot" in line or " quit" in line or " dump " in line):
            continue
        out.append(line)
        if line.startswith("frame 2120 "):
            break
    return out


class Script:
    def __init__(self):
        self.lines = menu_prefix()
        self.t = 2300

    def at(self, dt, cmd):
        self.lines.append(f"frame {self.t + dt} {cmd}")

    def note(self, text):
        self.lines.append(f"# {text}")

    def advance(self, dt):
        self.t += dt

    def shot(self, dt, name):
        self.at(dt, f"shot {SHOTS}/{name}.png")


def play(stop_after=None):
    s = Script()

    def done(stage):
        return stop_after == stage

    s.note("---- in the world (loaded at tick 2052): state at the spawn point")
    s.at(0, "dump world")
    s.at(0, "dump player")
    s.at(0, "dump npcs")
    s.advance(10)

    s.note("---- walk right for 120 ticks")
    s.at(0, "key D down")
    s.shot(115, "g6_walk")
    s.at(115, "dump player")
    s.at(120, "key D up")
    s.at(125, "dump player")
    s.advance(130)

    s.note("---- jump (Space held for 15 ticks)")
    s.at(0, "key Space down")
    s.shot(12, "g6b_jump")
    s.at(12, "dump player")
    s.at(15, "key Space up")
    s.at(60, "dump player")
    s.at(60, "dump tiles rel -4 -2 8 3")
    s.advance(70)
    if done("walk"):
        return s

    s.note("---- dig: copper pickaxe (hotbar 2), the mouse held on grass tile 2122,233 until it breaks")
    s.at(0, "key D2 press")
    s.at(10, "aim tile 2122 233")
    s.at(10, "dump tile 2122 233")
    s.at(12, "button left down")
    s.at(250, "button left up")
    s.at(252, "dump tile 2122 233")
    s.at(252, "dump items")
    s.at(310, "dump items")
    s.at(310, "dump player")
    s.shot(312, "g7_dig")
    s.advance(320)

    s.note("---- place the dirt block back (inventory slot 4 = hotbar 5; slot 3 holds the mushrooms the swing cut)")
    s.at(0, "key D5 press")
    s.at(10, "aim tile 2122 233")
    s.at(12, "button left down")
    s.at(20, "button left up")
    s.at(30, "dump tile 2122 233")
    s.at(30, "dump player")
    s.shot(40, "g7b_place")
    s.advance(50)
    if done("dig"):
        return s

    s.note("---- chop the tree at x 2128: walk closer, copper axe (hotbar 3), the mouse held on the foot of the trunk")
    s.at(0, "key D down")
    s.at(45, "key D up")
    s.at(60, "dump player")
    s.at(70, "key D3 press")
    s.at(80, "aim tile 2128 231")
    s.at(80, "dump tile 2128 231")
    s.at(82, "button left down")
    s.at(700, "button left up")
    s.at(702, "dump tile 2128 229")
    s.at(702, "dump tile 2128 225")
    s.at(702, "dump items")
    s.at(800, "dump items")
    s.at(800, "dump player")
    s.shot(802, "g7c_tree")
    s.advance(810)
    if done("tree"):
        return s

    s.note("---- inventory: open (Escape), move the pickaxe from slot 1 to slot 12 with the mouse")
    s.at(0, "key Escape press")
    s.at(30, "dump world")
    s.at(30, "dump recipes")
    s.at(40, "aim slot 1")
    s.at(50, "click left")
    s.at(60, "dump player")
    s.at(70, "aim slot 12")
    s.at(80, "click left")
    s.at(95, "dump item 1")
    s.at(95, "dump item 12")
    s.shot(100, "g8_inventory")
    s.advance(110)
    if done("inventory"):
        return s

    s.note("---- crafting: put the pickaxe back, focus the Work Bench in the crafting list, craft it, put it in hotbar 6")
    s.at(0, "aim slot 12")
    s.at(10, "click left")
    s.at(20, "aim slot 1")
    s.at(30, "click left")
    s.at(40, "dump recipes")
    s.at(40, "dump player")
    # the recipe list is sorted by recipe number; RECIPE_STEPS clicks on the entry below the focused one bring the
    # Work Bench into focus (the count comes from the `dump recipes` line of a survey run)
    t = 50
    for k in range(RECIPE_STEPS):
        s.at(t, f"aim recipe {RECIPE_FOCUS_START + k + 1}")
        s.at(t + 5, "click left")
        t += 25
    s.at(t, "dump recipes")
    s.at(t + 5, f"aim recipe {RECIPE_FOCUS_START + RECIPE_STEPS}")
    s.at(t + 15, "click left")
    s.at(t + 30, "dump player")
    s.at(t + 40, "aim slot 5")
    s.at(t + 50, "click left")
    s.at(t + 60, "dump player")
    s.shot(t + 65, "g8b_craft_list")
    s.at(t + 70, "key Escape press")
    s.advance(t + 100)
    s.note("---- place the work bench (hotbar 6) on the grass at 2129,231 (2131,232 is a slope: no anchor there)")
    s.at(0, "key D6 press")
    s.at(10, "aim tile 2129 231")
    s.at(10, "dump tiles 2126 229 2134 233")
    s.at(12, "button left down")
    s.at(20, "button left up")
    s.at(40, "dump tiles 2126 229 2134 233")
    s.at(40, "dump tile 2129 231")
    s.at(40, "dump tile 2130 231")
    s.at(40, "dump player")
    s.shot(45, "g8b_craft")
    s.advance(60)
    if done("craft"):
        return s

    s.note("---- night: the game's own Main.Moondialing() (time runs 60x until dusk), then the first enemies")
    s.at(0, "key D1 press")
    s.at(5, "game moondial")
    s.at(400, "dump world")
    s.advance(700)
    s.at(0, "dump world")
    s.at(0, "dump npcs")
    # sword swings: a click every 20 ticks at the nearest hostile NPC (a shortsword stabs towards the cursor);
    # `aim enemy` only moves the scripted cursor, the swing itself is the game's input path
    span = COMBAT_TICKS
    for k in range(0, span, 20):
        s.at(k, "aim enemy")
        s.at(k + 1, "click left")
        if k % 240 == 0:
            s.at(k + 2, "dump npcs")
            s.at(k + 2, "dump combat")
            s.at(k + 2, "dump player")
            s.at(k + 2, "dump items")
        if k % 240 == 0:
            s.shot(k + 3, f"g9_combat_{k // 240:02d}")
    s.advance(span)
    s.at(0, "dump world")
    s.at(0, "dump npcs")
    s.at(0, "dump player")
    s.advance(10)
    if done("combat"):
        return s

    s.note("---- the map (M) open and closed")
    s.at(0, "key M press")
    s.at(40, "dump world")
    s.shot(45, "g10_map")
    s.at(60, "key M press")
    s.at(90, "dump world")
    s.advance(100)

    s.note("---- settings: inventory (Escape), the Settings button, then Save & Exit (IngameOptions left column, entry 7)")
    s.at(0, "key Escape press")
    s.at(30, "aim settings")
    s.at(40, "click left")
    s.at(70, "dump world")
    s.shot(75, "g10b_settings")
    # IngameOptions.Draw: anchor (W/2 - 335 + 167.5, H/2 - 240 + 50), 47.5 per entry, Save & Exit is entry 7 (8th)
    s.at(80, "aim ui 414 517")
    s.at(90, "click left")
    s.at(200, "dump world")
    s.shot(260, "g10c_title_after_exit")
    s.advance(260)
    return s


# set from a survey run (tools/cs2rs/p1.sh --client): see the `dump recipes` lines
RECIPE_FOCUS_START = 0
RECIPE_STEPS = 1
COMBAT_TICKS = 1200


def main():
    stop = sys.argv[1] if len(sys.argv) > 1 else None
    if stop == "stability":
        stability(int(sys.argv[2]) if len(sys.argv) > 2 else 10000)
        return
    s = play(stop)
    s.at(60, "dump world")
    s.at(120, "quit")
    name = "g7_play.input" if stop is None else f"g7_play_{stop}.input"
    path = os.path.join(HERE, name)
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write("# GENERATED by make_play_script.py: change the generator, not this file.\n")
        f.write("\n".join(s.lines) + "\n")
    print(path, len(s.lines), "lines, last tick", s.t + 120)



def stability(ticks=10000):
    """g12_stability.input: `ticks` in-game ticks of moving, digging and fighting (stage D)."""
    s = Script()
    s.note(f"---- stability: {ticks} in-game ticks; a 500-tick cycle of walking, jumping, digging and sword swings")
    s.at(0, "dump world")
    s.at(0, "dump player")
    cycle = 0
    while cycle * 500 < ticks:
        b = cycle * 500
        s.at(b + 0, "key D1 press")
        s.at(b + 2, "key D down")
        s.at(b + 80, "key D up")
        s.at(b + 90, "key A down")
        s.at(b + 170, "key A up")
        s.at(b + 180, "key Space down")
        s.at(b + 195, "key Space up")
        s.at(b + 200, "key D2 press")
        s.at(b + 210, "aim rel 2 2")
        s.at(b + 212, "button left down")
        s.at(b + 290, "button left up")
        s.at(b + 300, "key D1 press")
        for k in range(300, 480, 20):
            s.at(b + k, "aim enemy")
            s.at(b + k + 1, "click left")
        if cycle == 3:
            s.note("night falls (Main.Moondialing): zombies and demon eyes join the slimes")
            s.at(b + 485, "game moondial")
        if cycle % 2 == 0:
            s.at(b + 490, "dump player")
            s.at(b + 490, "dump world")
            s.at(b + 490, "dump npcs")
        if cycle in (9, 19):
            s.shot(b + 495, f"g12_stability_{(cycle + 1) * 500}")
        cycle += 1
    s.advance(cycle * 500)
    s.at(0, "dump player")
    s.at(0, "dump world")
    s.at(0, "dump npcs")
    s.shot(5, "g12_stability_end")
    s.at(30, "quit")
    path = os.path.join(HERE, "g12_stability.input")
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write("# GENERATED by make_play_script.py stability: change the generator, not this file.\n")
        f.write("\n".join(s.lines) + "\n")
    print(path, len(s.lines), "lines, last tick", s.t + 30)


if __name__ == "__main__":
    main()

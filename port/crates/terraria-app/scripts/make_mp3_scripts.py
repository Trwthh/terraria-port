"""Writes the three-player, password multiplayer scripts (mp3_host / mp3_join_b / mp3_join_c) from the two-player
ones (mp_host / mp_join), so that the two scenarios stay in step. Run from this folder: python make_mp3_scripts.py"""
import os

os.chdir(os.path.dirname(os.path.abspath(__file__)))
host = open('mp_host.input', encoding='utf-8').read()
join = open('mp_join.input', encoding='utf-8').read()
NL = '\\n'  # the script language's typed Enter is the two characters backslash, n


def rep(s, a, b):
    assert s.count(a) == 1, a
    return s.replace(a, b)


h = host
h = rep(h, "# terraria (translated client), process A of the multiplayer proof:",
        "# THREE-PLAYER, PASSWORD variant (written by make_mp3_scripts.py from mp_host.input): the server password is\n"
        "# potato7; B (mp3_join_b) and C (mp3_join_c) join and type it at the password prompt (Main.menuMode 31); A waits\n"
        "# for three players, for B's tile right of the spawn and C's tile left of it, sends the chat line, waits until\n"
        "# both left. Run: sh crates/terraria-app/scripts/run_multiplayer3.sh\n"
        "# terraria (translated client), process A of the multiplayer proof:")
h = rep(h, "# the server password prompt: empty, Enter -> Main.HostAndPlay()\nframe 1940 text " + NL,
        "# the server password prompt: potato7, Enter -> Main.HostAndPlay()\nframe 1940 text potato7\nframe 1960 text " + NL)
h = rep(h, "frame 2080 wait 600 players 2", "frame 2080 wait 900 players 3")
h = rep(h, "frame 2170 wait 300 tile rel 2 2 inactive\nframe 2180 dump tile rel 2 2",
        "frame 2170 wait 300 tile rel 2 2 inactive\nframe 2175 wait 300 tile rel -2 2 inactive\n"
        "frame 2180 dump tile rel 2 2\nframe 2181 dump tile rel -2 2")
h = rep(h, "# B leaves", "# B and C leave")
h = h.replace("mp_a3_two_players", "mp_a3_three_players").replace("../work/shots/mp_a", "../work/shots/mp3_a")
open('mp3_host.input', 'w', encoding='utf-8', newline='\n').write(h)


def joiner(name, dx, walk_key, shot):
    j = join
    j = rep(j, "# terraria (translated client), process B of the multiplayer proof:",
            f"# THREE-PLAYER, PASSWORD variant (written by make_mp3_scripts.py from mp_join.input): player {name}, types\n"
            f"# the server password potato7 at the password prompt, digs the tile {dx} 2 relative to the spawn.\n"
            "# terraria (translated client), process B of the multiplayer proof:")
    j = rep(j, "frame 1590 text Joiner", f"frame 1590 text {name}")
    j = rep(j, "frame 2030 text " + NL + "\nframe 2060 dump net",
            "frame 2030 text " + NL + "\n"
            "# the server asks for its password (NetMessage 37 -> Main.menuMode 31): typed, Enter\n"
            "frame 2040 wait 600 menumode 31\nframe 2045 dump ui\n"
            f"frame 2050 shot ../work/shots/{shot}_password.png\n"
            "frame 2055 text potato7\nframe 2058 text " + NL + "\nframe 2060 dump net")
    j = rep(j, "frame 2150 wait 300 players 2", "frame 2150 wait 600 players 3")
    j = j.replace("tile rel 2 2", f"tile rel {dx} 2").replace("aim rel 2 2", f"aim rel {dx} 2")
    j = rep(j, "frame 2300 key D down\nframe 2480 key D up", f"frame 2300 key {walk_key} down\nframe 2480 key {walk_key} up")
    j = j.replace("../work/shots/mp_b", f"../work/shots/{shot}")
    return j


open('mp3_join_b.input', 'w', encoding='utf-8', newline='\n').write(joiner("Joiner", 2, "D", "mp3_b"))
open('mp3_join_c.input', 'w', encoding='utf-8', newline='\n').write(joiner("Third", -2, "A", "mp3_c"))
print("wrote mp3_host.input, mp3_join_b.input, mp3_join_c.input")

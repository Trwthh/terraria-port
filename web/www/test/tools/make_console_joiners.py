"""Native port clients (port/target-app/client/terraria.exe, hidden, scripted) for the server console tests
(web/STATUS.md 2026-10-14 "Server console: every command"): each joins 127.0.0.1:<port> as "Joiner" and checks
what the server did to it. Used against the browser's Host & Play server (run-host-console-mp.sh, port 47800
through tools/ws-bridge) and against the ORIGINAL TerrariaServer.exe (orig_console.py, port 47801).

    python make_console_joiners.py <port> <out dir>     -> <out dir>/console_{pwwrong,kick,ban,banned}.input

  pwwrong  the server asks for its password (NetMessage 37 -> Main.menuMode 31): types "wrong" -> refused
           (the client's status "Incorrect password")
  kick     types "secret" -> in the world, the MOTD in its chat, waits to be kicked ("Kicked from server.")
  ban      the same, waits to be banned ("Banned from server.")
  banned   connects again after the ban: refused before the password ("You are banned from this server.")
"""
import os
import sys

PORT = sys.argv[1]
OUT = sys.argv[2]
MOTD = 'Console MOTD for joiners'
os.makedirs(OUT, exist_ok=True)

# title -> Multiplayer -> Join via IP -> new player "Joiner" -> 127.0.0.1 -> <port> (mp_join_original.input)
JOIN = f"""frame 900 mouse 640 360
frame 1010 mouse 640 247
frame 1030 click left
frame 1100 mouse 900 600
frame 1210 aim menu 1
frame 1230 click left
frame 1300 aim menu 0
frame 1320 click left
frame 1400 mouse 807 650
frame 1420 click left
frame 1500 mouse 770 533
frame 1520 click left
frame 1590 text Joiner
frame 1630 text \\n
frame 1740 aim snap Play 0
frame 1760 click left
frame 1870 text 127.0.0.1
frame 1900 text \\n
frame 1970 key Back down
frame 1973 key Back up
frame 1976 key Back down
frame 1979 key Back up
frame 1982 key Back down
frame 1985 key Back up
frame 1988 key Back down
frame 1991 key Back up
frame 2000 text {PORT}
frame 2030 text \\n
frame 2060 dump net
"""

PASSWORD = """frame 2040 wait 600 menumode 31
frame 2045 dump ui
frame 2055 text {pw}
frame 2058 text \\n
"""

IN_WORLD = f"""frame 2070 wait 600 ingame
frame 2080 dump players
frame 2090 wait 120 chat {MOTD}
frame 2095 dump chat
"""


def write(name, body):
    path = os.path.join(OUT, f'console_{name}.input')
    with open(path, 'w', encoding='utf-8', newline='\n') as f:
        f.write(f'# written by web/www/test/tools/make_console_joiners.py ({name}, port {PORT})\n' + body)
    print(path)


write('pwwrong', JOIN + PASSWORD.format(pw='wrong') + """frame 2070 wait 300 status Incorrect password
frame 2075 dump ui
frame 2100 quit
""")
for name, why in (('kick', 'Kicked from server.'), ('ban', 'Banned from server.')):
    write(name, JOIN + PASSWORD.format(pw='secret') + IN_WORLD + f"""frame 2100 wait 600 status {why}
frame 2105 wait 120 menu
frame 2110 dump ui
frame 2120 quit
""")
write('banned', JOIN + """frame 2070 wait 300 status You are banned from this server.
frame 2075 dump ui
frame 2100 quit
""")

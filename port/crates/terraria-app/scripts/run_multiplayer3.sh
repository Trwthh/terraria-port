#!/bin/sh
# Three hidden, muted, scripted processes of the port with a server password (potato7): A hosts (mp3_host.input),
# B and C join via IP and type the password (mp3_join_b.input, mp3_join_c.input; C starts 20 s after B).
# Scripts are written by make_mp3_scripts.py from the two-player ones.
#   sh crates/terraria-app/scripts/run_multiplayer3.sh [seconds]      (from port/; default limit 900)
cd "$(dirname "$0")"
HOST_SCRIPT=mp3_host.input JOIN_SCRIPT=mp3_join_b.input JOIN_SCRIPT_C=mp3_join_c.input \
NAME_A="${NAME_A:-mp3_a}" NAME_B="${NAME_B:-mp3_b}" NAME_C="${NAME_C:-mp3_c}" exec sh ./run_multiplayer.sh "$@"

#!/bin/sh
# The draw reference again: what the original client draws in the scenes of RefDumpDraw.cs, and the machine code of
# every method that ran in that process (the client JIT facts, jit_calls.Dirs.fill). With --audio: the audio run
# (RefDumpDraw.cs audio mode: sound on, menu clicks, the audio scene; refdata/audio, work/jit/client/audio) instead.
#
#   sh tools/refdump_draw/run_draw.sh [--build] [--refdata] [--audio]
#
#   --build    tools\refdump_draw\build.cmd first (stub generator, XNA stand-in, RefDumpDraw.exe, RefJitDraw.exe)
#   --refdata  also write refdata/draw (refdata/audio) itself (RefDumpDraw.exe); without it the run only goes to work/
#              and is compared with refdata/draw (refdata/audio)
#   --audio    the audio run
#   --scenes   the scenes run (RefDumpDraw.cs scenes mode: menus, biomes, events, UI screens, mounts / pets, lighting
#              modes, every boss; refdata/scenes, work/jit/client/scenes)
#   --interact the interaction run (Interact.cs: every held / channelled weapon used, wiring, chests, the Old One's
#              Army, shimmer, town NPCs arriving, player states; refdata/interact, work/jit/client/interact)
#   --cover    the coverage run (Cover.cs: display dolls, doors, a Christmas tree, teleporters, inventory pages, the map
#              saved and loaded, every accessory and mount, three skies, the Wall of Flesh; refdata/cover, work/jit/client/cover)
#   --probe    the probe run (Probe*.cs: the float methods no other run reached, each by the state that makes the game
#              call it or by a direct call; probe lines and direct draw scenes; refdata/probe, work/jit/client/probe)
#   --gallery  the gallery run (Gallery.cs: old backgrounds, every particle / ambient sky entity / projectile / NPC type,
#              world events, the player's death; refdata/gallery, work/jit/client/gallery)
#
# Output: work/jit/client/<draw|audio>/ (the previous one moved to work/_archive/jit_client/; jit_code.bin, jit_code.json,
# jit_debug.bin, jit_callees.json, jit_method_callees.json, jit_cells.json, jit_targets.json; data/ = what that run
# recorded), log work/jit/client/<draw|audio>.log. Then the proof that the JIT dump is of the run that wrote the
# reference data: data/ byte for byte against refdata/<draw|audio>.
# Headless: the game's form is hidden on a desktop of its own (RefDumpDraw.cs); no socket; the audio engine is AudioRec.cs's
# model (nothing is played). One process at a time, each with an in-process watchdog and killed by PID here when still
# alive after the limit.
# After a new dump: python tools/cs2rs/gen_jit_refcalls.py && python tools/cs2rs/gen_jit_float_alloc.py &&
# python tools/cs2rs/gen_jit_vec_temps.py && python tools/cs2rs/gen_jit_vec_ctx.py && python tools/cs2rs/gen_jit_bodies.py &&
# python tools/cs2rs/gen_jit_calls.py, then sh tools/cs2rs/run.sh.
cd "$(dirname "$0")/../.."
B=tools/refdump_draw/bin
J=work/jit/client
USER_LIMIT=$LIMIT
LIMIT=${LIMIT:-1500}
KIND=draw
MODE=""
for a in "$@"; do
  case "$a" in
    --build) powershell -NoProfile -Command "& '$(cygpath -w "$PWD/tools/refdump_draw/build.cmd")'" || exit 1 ;;
    --refdata) REFDATA=1 ;;
    --audio) KIND=audio; MODE=audio ;;
    --scenes) KIND=scenes; MODE=scenes ;;
    --gallery) KIND=gallery; MODE=gallery; LIMIT=${USER_LIMIT:-7200}; export REFDRAW_WATCHDOG=${REFDRAW_WATCHDOG:-7000} ;;
    --interact) KIND=interact; MODE=interact; LIMIT=${USER_LIMIT:-7200}; export REFDRAW_WATCHDOG=${REFDRAW_WATCHDOG:-7000} ;;
    --cover) KIND=cover; MODE=cover; LIMIT=${USER_LIMIT:-7200}; export REFDRAW_WATCHDOG=${REFDRAW_WATCHDOG:-7000} ;;
    --probe) KIND=probe; MODE=probe; LIMIT=${USER_LIMIT:-7200}; export REFDRAW_WATCHDOG=${REFDRAW_WATCHDOG:-7000} ;;
    *) echo "unknown option $a"; exit 2 ;;
  esac
done

run() {   # name, then the command
  name=$1; shift
  log="$J/$name.log"
  t0=$(date +%s)
  "$@" > "$log" 2>&1 < /dev/null &
  pid=$!
  t=0
  while kill -0 "$pid" 2>/dev/null; do
    sleep 2; t=$((t + 2))
    if [ "$t" -ge "$LIMIT" ]; then echo "$name: still running after $t s: killing PID $pid"; kill -9 "$pid" 2>/dev/null; break; fi
  done
  wait "$pid" 2>/dev/null
  rc=$?
  echo "$name: exit $rc after $(($(date +%s) - t0)) s; $(grep -E '^dumpran: [0-9]+ of|^done:|FATAL|WATCHDOG' "$log" | head -2 | tr '\n' ' ')"
  return $rc
}

mkdir -p "$J"
if [ -n "$REFDATA" ]; then
  mkdir -p refdata/$KIND
  run refdata_$KIND "$B/RefDumpDraw.exe" "$(cygpath -w "$PWD/refdata/$KIND")" $MODE || exit 1
fi
# the previous dump is kept: moved to work/_archive (nothing is deleted)
if [ -d "$J/$KIND" ]; then mkdir -p work/_archive/jit_client; mv "$J/$KIND" "work/_archive/jit_client/${KIND}_$(date +%Y%m%d-%H%M%S)"; fi
mkdir -p "$J/$KIND/data"
run $KIND "$B/RefJitDraw.exe" "$(cygpath -w "$PWD/$J/$KIND")" "$(cygpath -w "$PWD/$J/$KIND/data")" $MODE
tasklist //FO CSV 2>/dev/null | grep -iE "RefJitDraw|RefDumpDraw" && echo "WARNING: a harness process is still running"
same=0; diff=0
for f in refdata/$KIND/*; do
  n=$(basename "$f")
  if cmp -s "$f" "$J/$KIND/data/$n"; then same=$((same + 1)); else diff=$((diff + 1)); echo "differs: $n"; fi
done
echo "$KIND: $same files equal to refdata/$KIND, $diff different"
[ "$diff" -eq 0 ]

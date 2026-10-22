#!/bin/sh
# The multiplayer proof: two hidden, muted, scripted processes of the port on this machine.
#   A (mp_host.input): Multiplayer -> Host & Play with ref_small; the game starts its dedicated server (this
#     executable with -server) and joins it;  B (mp_join.input): Multiplayer -> Join via IP 127.0.0.1:47800.
#
#   sh crates/terraria-app/scripts/run_multiplayer.sh [seconds]        (default limit 900; PROFILE=client-opt)
#
# - TERRARIA_NET_LOOPBACK_ONLY=1: every listener is on 127.0.0.1 only and the LAN announcement is not sent;
#   TERRARIA_HOSTPLAY_PORT=47800: the server Host & Play starts listens on 47800 instead of the fixed 7777.
# - fresh save roots work/port-save-runs/mp_a (with ref_small.wld) and mp_b (empty); logs work/logs/mp_a.log,
#   mp_b.log, state dumps mp_a.state.jsonl / mp_b.state.jsonl, the console of the server in
#   work/port-save-runs/mp_a/server-last-run.log
# - every process is killed by PID if it overruns, the server too (its PID is in the log of A); nothing is
#   left running.
# HOST_SCRIPT=<file> (default mp_host.input) for A; JOIN_SCRIPT_C=<file> [NAME_C=mp_c] [C_DELAY=20]: a third process
#   C, started C_DELAY seconds after B (run_multiplayer3.sh: three players and a server password).
# ONLY=join SERVER=<host:port is typed by the script>: start B alone (against a server started elsewhere, e.g.
#   the original dedicated server: run_original_server.sh).
set -u
cd "$(dirname "$0")/../../.."          # port/
LIMIT="${1:-900}"
PROFILE="${PROFILE:-client}"
PROJECT="$(cd .. && pwd -W 2>/dev/null || (cd .. && pwd))"
EXE="./target-app/$PROFILE/terraria.exe"
SCRIPTS="crates/terraria-app/scripts"
NAME_A="${NAME_A:-mp_a}"
NAME_B="${NAME_B:-mp_b}"
JOIN_SCRIPT="${JOIN_SCRIPT:-mp_join.input}"
[ -x "$EXE" ] || { echo "missing $EXE"; exit 2; }
mkdir -p "$PROJECT/work/logs" "$PROJECT/work/shots" "$PROJECT/work/port-save-runs/_old"
fresh() {
  S="$PROJECT/work/port-save-runs/$1"
  [ -d "$S" ] && mv "$S" "$PROJECT/work/port-save-runs/_old/$1-$(date +%Y%m%d-%H%M%S)"
  mkdir -p "$S/Worlds"
}
export TERRARIA_NET_LOOPBACK_ONLY=1 TERRARIA_HOSTPLAY_PORT="${TERRARIA_HOSTPLAY_PORT:-47800}" TERRARIA_MUTE=1 TERRARIA_HIDDEN=1 TERRARIA_FRAME_TIMES=1 TERRARIA_HOST_REPORT=1
start() {  # name script
  TERRARIA_SAVE_DIR="$PROJECT/work/port-save-runs/$1" TERRARIA_INPUT_SCRIPT="$SCRIPTS/$2" TERRARIA_STATE_LOG="$PROJECT/work/logs/$1.state.jsonl" \
    "$EXE" > "$PROJECT/work/logs/$1.log" 2>&1 &
}
# (started in THIS shell, not in a command substitution, so that `wait` gets the exit codes)
PA=""
if [ "${ONLY:-}" != join ]; then
  fresh "$NAME_A"
  cp "$PROJECT/work/refsave/Worlds/ref_small.wld" "$PROJECT/work/port-save-runs/$NAME_A/Worlds/ref_small.wld"
  start "$NAME_A" "${HOST_SCRIPT:-mp_host.input}"
  PA=$!
  # B types the address about as fast as A gets to Host & Play: start B only when the server of A listens (a
  # player would get "connection refused" otherwise, as in the original)
  W0=$(date +%s)
  until grep -q "Server started" "$PROJECT/work/port-save-runs/$NAME_A/server-last-run.log" 2>/dev/null; do
    sleep 1
    if ! kill -0 "$PA" 2>/dev/null; then echo "run_multiplayer: A ended before its server listened"; grep -E "ERROR|panicked|stopped" "$PROJECT/work/logs/$NAME_A.log" | cut -c1-200 | tail -5; exit 1; fi
    if [ $(( $(date +%s) - W0 )) -ge 300 ]; then echo "run_multiplayer: the server of A did not listen within 300 s: killing A and its server"; kill -9 "$PA" 2>/dev/null
      SP=$(grep -o "started the dedicated server: pid [0-9]*" "$PROJECT/work/logs/$NAME_A.log" | grep -o "[0-9]*$" | head -1)
      [ -n "${SP:-}" ] && taskkill //PID "$SP" //F > /dev/null 2>&1
      exit 1
    fi
  done
  echo "run_multiplayer: the server of A listens after $(( $(date +%s) - W0 )) s"
fi
fresh "$NAME_B"
start "$NAME_B" "$JOIN_SCRIPT"
PB=$!
PC=""
NAME_C="${NAME_C:-mp_c}"
if [ -n "${JOIN_SCRIPT_C:-}" ]; then
  sleep "${C_DELAY:-20}"
  fresh "$NAME_C"
  start "$NAME_C" "$JOIN_SCRIPT_C"
  PC=$!
fi
# the Windows process ids too: the limit and the final check do not depend on this shell's process table
WA=""; [ -n "$PA" ] && WA=$(cat "/proc/$PA/winpid" 2>/dev/null)
WB=$(cat "/proc/$PB/winpid" 2>/dev/null)
WC=""; [ -n "$PC" ] && WC=$(cat "/proc/$PC/winpid" 2>/dev/null)
echo "run_multiplayer: A pid ${PA:-none} (windows ${WA:-?}), B pid $PB (windows ${WB:-?}), C pid ${PC:-none} (windows ${WC:-?})"
T0=$(date +%s)
T=0
alive() { [ -n "$1" ] && kill -0 "$1" 2>/dev/null; }
while alive "$PA" || alive "$PB" || alive "$PC"; do
  sleep 1
  T=$(( $(date +%s) - T0 ))
  if [ "$T" -ge "$LIMIT" ]; then
    echo "run_multiplayer: still running after $LIMIT s: killing ${PA:-} $PB ${PC:-}"
    kill -9 $PA $PB $PC 2>/dev/null
    break
  fi
done
T=$(( $(date +%s) - T0 ))
RA=0
[ -n "$PA" ] && { wait "$PA" 2>/dev/null; RA=$?; }
wait "$PB" 2>/dev/null; RB=$?
RC=0
[ -n "$PC" ] && { wait "$PC" 2>/dev/null; RC=$?; }
# the server A started: it shuts down by itself when its host leaves (-autoshutdown); make sure
if [ -n "$PA" ]; then
  SP=$(grep -o "started the dedicated server: pid [0-9]*" "$PROJECT/work/logs/$NAME_A.log" | grep -o "[0-9]*$" | head -1)
  if [ -n "${SP:-}" ]; then
    W=0
    while tasklist //FI "PID eq $SP" 2>/dev/null | grep -q " $SP " && [ "$W" -lt 30 ]; do sleep 1; W=$((W + 1)); done
    if tasklist //FI "PID eq $SP" 2>/dev/null | grep -q " $SP "; then
      echo "run_multiplayer: the server (pid $SP) is still running 30 s after its host left: killing it"
      taskkill //PID "$SP" //F > /dev/null 2>&1
    else
      echo "run_multiplayer: the server (pid $SP) ended by itself within $W s"
    fi
  fi
fi
echo "run_multiplayer: A exit $RA, B exit $RB$([ -n "$PC" ] && echo ", C exit $RC") after $T s"
for n in $([ -n "$PA" ] && echo "$NAME_A") "$NAME_B" $([ -n "$PC" ] && echo "$NAME_C"); do
  echo "---- $n"
  grep -E "^\[terraria\] tick|^\[shell\] (wait|script|stopped|unhandled)|^\[host\] (net|started|not implemented)|panicked|^error" "$PROJECT/work/logs/$n.log" | cut -c1-${WIDTH:-300} | tail -${LINES_MAX:-25}
done
if [ -n "$PA" ] && [ -f "$PROJECT/work/port-save-runs/$NAME_A/server-last-run.log" ]; then
  echo "---- server"
  grep -v "inert " "$PROJECT/work/port-save-runs/$NAME_A/server-last-run.log" | cut -c1-${WIDTH:-300} | tail -${LINES_MAX:-25}
fi
for w in $WA $WB $WC; do
  if tasklist //FI "PID eq $w" 2>/dev/null | grep -q " $w "; then
    echo "run_multiplayer: process $w outlived this script's bookkeeping: killing it"
    taskkill //PID "$w" //F > /dev/null 2>&1
    RA=1
  fi
done
tasklist //FO CSV 2>/dev/null | grep -i "terraria.exe" && echo "run_multiplayer: WARNING: a terraria.exe process is still running"
[ "$RA" = 0 ] && [ "$RB" = 0 ] && [ "$RC" = 0 ]

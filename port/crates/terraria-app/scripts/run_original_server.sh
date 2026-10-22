#!/bin/sh
# Interoperability: the port's client against the ORIGINAL dedicated server.
#
#   sh crates/terraria-app/scripts/run_original_server.sh [seconds]      (default limit 600; PROFILE=client-opt)
#
# Server: work/refserver/TerrariaServer.exe (the private copy of the real game's server; nothing in it is written),
#   started without a window, console output to work/logs/mpo_server.log, with
#   -world <scratch copy of ref_small.wld> -port 47801 -ip 127.0.0.1 -password "" -autoshutdown -noupnp
#   -savedirectory <scratch>
#   * -ip is read by the game's TcpSocket.StartListening (TcpSocket.cs:160-164): the listener is on 127.0.0.1 only;
#   * -savedirectory (Program.cs: LaunchParameters "-savedirectory") keeps every file the server writes under
#     work/interop-scratch, as does its working directory; the user's Documents\My Games\Terraria is not touched;
#   * -autoshutdown: no console input thread, and the server ends by itself when the last local player leaves
#     (NetMessage.EnsureLocalPlayerIsPresent).
#   NOT avoidable: the original announces a running server on the LAN (Netplay.BroadcastThread: one UDP datagram
#   per second to 255.255.255.255:8888 with the world name, the machine name and the port) for as long as it runs.
# Client: the port, hidden, muted, scripted (mp_join_original.input), TERRARIA_NET_LOOPBACK_ONLY=1.
# Both are killed by PID when they overrun; the server is killed in any case if it is still alive at the end.
set -u
cd "$(dirname "$0")/../../.."          # port/
LIMIT="${1:-600}"
PROFILE="${PROFILE:-client}"
PROJECT="$(cd .. && pwd -W 2>/dev/null || (cd .. && pwd))"
EXE="./target-app/$PROFILE/terraria.exe"
SERVER="$PROJECT/work/refserver/TerrariaServer.exe"
SCRATCH="$PROJECT/work/interop-scratch"
[ -x "$EXE" ] || { echo "missing $EXE"; exit 2; }
[ -f "$SERVER" ] || { echo "missing $SERVER"; exit 2; }
mkdir -p "$PROJECT/work/logs" "$PROJECT/work/shots" "$PROJECT/work/port-save-runs/_old"
[ -d "$SCRATCH" ] && mv "$SCRATCH" "$PROJECT/work/port-save-runs/_old/interop-scratch-$(date +%Y%m%d-%H%M%S)"
mkdir -p "$SCRATCH/save" "$SCRATCH/Worlds"
cp "$PROJECT/work/refsave/Worlds/ref_small.wld" "$SCRATCH/Worlds/ref_small.wld"
SLOG="$PROJECT/work/logs/mpo_server.log"
W_WORLD="$(cygpath -w "$SCRATCH/Worlds/ref_small.wld")"
W_SAVE="$(cygpath -w "$SCRATCH/save")"
# the server: its own process, no window (stdin from nul, output to the log)
( cd "$SCRATCH" && exec "$SERVER" -world "$W_WORLD" -port 47801 -ip 127.0.0.1 -password "" -autoshutdown -noupnp -savedirectory "$W_SAVE" < /dev/null > "$SLOG" 2>&1 ) &
PS=$!
sleep 2
WS=$(cat "/proc/$PS/winpid" 2>/dev/null)
echo "run_original_server: server pid $PS (windows ${WS:-?}); log $SLOG"
killserver() { kill -9 "$PS" 2>/dev/null; [ -n "${WS:-}" ] && taskkill //PID "$WS" //F > /dev/null 2>&1; }
# wait for the listener (the world loads first)
T=0
until grep -q "Listening on port\|Server started" "$SLOG" 2>/dev/null; do
  sleep 1; T=$((T + 1))
  if ! kill -0 "$PS" 2>/dev/null; then echo "run_original_server: the server ended before it listened"; tail -20 "$SLOG"; exit 1; fi
  if [ "$T" -ge 180 ]; then echo "run_original_server: the server did not start listening in 180 s"; killserver; exit 1; fi
done
echo "run_original_server: the server listens after $T s"
netstat -an 2>/dev/null | grep ":47801 " | head -3
# the client
S="$PROJECT/work/port-save-runs/mpo_client"
[ -d "$S" ] && mv "$S" "$PROJECT/work/port-save-runs/_old/mpo_client-$(date +%Y%m%d-%H%M%S)"
mkdir -p "$S/Worlds"
TERRARIA_NET_LOOPBACK_ONLY=1 TERRARIA_MUTE=1 TERRARIA_HIDDEN=1 TERRARIA_FRAME_TIMES=1 TERRARIA_HOST_REPORT=1 TERRARIA_SAVE_DIR="$S" \
  TERRARIA_INPUT_SCRIPT="crates/terraria-app/scripts/mp_join_original.input" TERRARIA_STATE_LOG="$PROJECT/work/logs/mpo_client.state.jsonl" \
  "$EXE" > "$PROJECT/work/logs/mpo_client.log" 2>&1 &
PC=$!
WC=$(cat "/proc/$PC/winpid" 2>/dev/null)
T0=$(date +%s)
T=0
while kill -0 "$PC" 2>/dev/null; do
  sleep 1; T=$(( $(date +%s) - T0 ))
  if [ "$T" -ge "$LIMIT" ]; then echo "run_original_server: the client is still running after $LIMIT s: killing $PC"; kill -9 "$PC" 2>/dev/null; break; fi
done
wait "$PC" 2>/dev/null; RC=$?
# the server ends by itself (-autoshutdown) once its local player left; make sure
W=0
while kill -0 "$PS" 2>/dev/null && [ "$W" -lt 40 ]; do sleep 1; W=$((W + 1)); done
if kill -0 "$PS" 2>/dev/null; then
  echo "run_original_server: the server is still running 40 s after the client left: killing it"
  killserver
else
  echo "run_original_server: the server ended by itself within $W s"
fi
# whatever is left of it (the shell above execs the server, so normally nothing)
for w in ${WS:-} ${WC:-}; do
  if tasklist //FI "PID eq $w" 2>/dev/null | grep -q " $w "; then
    echo "run_original_server: process $w outlived this script's bookkeeping: killing it"
    taskkill //PID "$w" //F > /dev/null 2>&1
  fi
done
tasklist //FO CSV 2>/dev/null | grep -i "TerrariaServer.exe" && echo "run_original_server: WARNING: a TerrariaServer.exe process is running (not started by this script, or not killable by PID)"
echo "run_original_server: client exit $RC after $T s"
echo "---- client"
grep -E "^\[terraria\] tick|^\[shell\] (wait|script|stopped|unhandled)|^\[host\] (net|not implemented)|panicked|^error" "$PROJECT/work/logs/mpo_client.log" | cut -c1-300 | tail -20
echo "---- server console"
cut -c1-300 "$SLOG" | grep -v "^[A-Za-z ]*[0-9]*%$" | tail -40
[ "$RC" = 0 ]

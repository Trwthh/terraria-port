#!/bin/sh
# Multiplayer in the browser, end to end: a dedicated server on 127.0.0.1:47800, the WebSocket bridge, and the web
# build in hidden headless Chromium joining it (web/www/test/actions/mp_join.txt: Join via IP -> in the world -> dig
# -> walk -> chat "hello from web" -> leave).
#
#   sh tools/ws-bridge/run_mp_web.sh [port|original] [limit seconds, default 1500]
#   MP_ACTIONS=test/actions/mp_join_long.txt: the same with about 3,300 frames connected (collector cycles)
#
# port:     the port's own dedicated server (port/target-app/client/terraria.exe -server), TERRARIA_NET_LOOPBACK_ONLY=1
# original: the ORIGINAL work/refserver/TerrariaServer.exe (as run_original_server.sh: -ip 127.0.0.1, its files under
#           work/interop-scratch-web). NOT avoidable: while it runs it announces itself on the LAN (UDP broadcast
#           to 255.255.255.255:8888 once a second); this script keeps it running only for the run and kills it.
# Both: fresh copy of work/refsave/Worlds/ref_small.wld, -autoshutdown (the server ends and saves when its local
# player, the browser through the bridge on 127.0.0.1, leaves). Logs: work/logs/mpw_<mode>_server.log,
# mpw_<mode>_bridge.log, the page's log work/logs/web_game_run.txt; shots work/shots/mpw_*.png.
# Every process started here (server, bridge, the runner and its browser) is stopped by PID at the end.
set -u
cd "$(dirname "$0")/../.."
PROJECT="$(pwd)"
MODE="${1:-port}"
LIMIT="${2:-1500}"
PORTN=47800
BPORT="${BRIDGE_PORT:-7780}"
SLOG="$PROJECT/work/logs/mpw_${MODE}_server.log"
BLOG="$PROJECT/work/logs/mpw_${MODE}_bridge.log"
SCRATCH="$PROJECT/work/port-save-runs/mpw_$MODE"
mkdir -p "$PROJECT/work/logs" "$PROJECT/work/port-save-runs/_old"
[ -d "$SCRATCH" ] && mv "$SCRATCH" "$PROJECT/work/port-save-runs/_old/mpw_$MODE-$(date +%Y%m%d-%H%M%S)"
mkdir -p "$SCRATCH/Worlds" "$SCRATCH/save"
cp "$PROJECT/work/refsave/Worlds/ref_small.wld" "$SCRATCH/Worlds/ref_small.wld"
W_WORLD="$(cygpath -w "$SCRATCH/Worlds/ref_small.wld")"
: > "$BLOG"
case "$MODE" in
  port)
    EXE="$PROJECT/port/target-app/client/terraria.exe"
    [ -x "$EXE" ] || { echo "missing $EXE"; exit 2; }
    ( cd "$PROJECT/port" && TERRARIA_NET_LOOPBACK_ONLY=1 TERRARIA_MUTE=1 TERRARIA_HIDDEN=1 TERRARIA_SAVE_DIR="$SCRATCH/save" \
        exec "$EXE" -server -world "$W_WORLD" -port $PORTN -ip 127.0.0.1 -password "" -autoshutdown < /dev/null > "$SLOG" 2>&1 ) &
    ;;
  original)
    EXE="$PROJECT/work/refserver/TerrariaServer.exe"
    [ -f "$EXE" ] || { echo "missing $EXE"; exit 2; }
    W_SAVE="$(cygpath -w "$SCRATCH/save")"
    ( cd "$SCRATCH" && exec "$EXE" -world "$W_WORLD" -port $PORTN -ip 127.0.0.1 -password "" -autoshutdown -noupnp -savedirectory "$W_SAVE" < /dev/null > "$SLOG" 2>&1 ) &
    ;;
  *) echo "run_mp_web: mode port or original"; exit 2 ;;
esac
PS=$!
sleep 2
WS=$(cat "/proc/$PS/winpid" 2>/dev/null)
echo "run_mp_web: $MODE server pid $PS (windows ${WS:-?}), log $SLOG"
killserver() { kill -9 "$PS" 2>/dev/null; [ -n "${WS:-}" ] && taskkill //PID "$WS" //F > /dev/null 2>&1; }
T=0
until grep -q "Server started" "$SLOG" 2>/dev/null; do
  sleep 1; T=$((T + 1))
  if ! kill -0 "$PS" 2>/dev/null; then echo "run_mp_web: the server ended before it listened"; tail -20 "$SLOG"; exit 1; fi
  if [ "$T" -ge 240 ]; then echo "run_mp_web: the server did not listen within 240 s"; killserver; exit 1; fi
done
echo "run_mp_web: the server listens after $T s"
netstat -an 2>/dev/null | grep ":$PORTN " | grep LISTEN | head -2
python "$PROJECT/tools/ws-bridge/ws_bridge.py" --port "$BPORT" --trace-terraria --log "$(cygpath -w "$BLOG")" > /dev/null 2>&1 &
PB=$!
sleep 1
WB=$(cat "/proc/$PB/winpid" 2>/dev/null)
echo "run_mp_web: bridge pid $PB (windows ${WB:-?}) on ws://127.0.0.1:$BPORT"
( cd "$PROJECT/web/www" && exec node test/run-game-headless.mjs --actions "${MP_ACTIONS:-test/actions/mp_join.txt}" --bridge "ws://127.0.0.1:$BPORT" ) > "$PROJECT/work/logs/mpw_${MODE}_runner.txt" 2>&1 &
PR=$!
WR=$(cat "/proc/$PR/winpid" 2>/dev/null)
T0=$(date +%s)
while kill -0 "$PR" 2>/dev/null; do
  sleep 2
  if [ $(( $(date +%s) - T0 )) -ge "$LIMIT" ]; then echo "run_mp_web: the browser run is still going after $LIMIT s: killing it"; kill "$PR" 2>/dev/null; sleep 5; kill -9 "$PR" 2>/dev/null; [ -n "${WR:-}" ] && taskkill //PID "$WR" //T //F > /dev/null 2>&1; break; fi
done
wait "$PR" 2>/dev/null; RR=$?
echo "run_mp_web: browser run exit $RR after $(( $(date +%s) - T0 )) s"
# -autoshutdown: the server ends by itself once the browser's player left; make sure
W=0
while kill -0 "$PS" 2>/dev/null && [ "$W" -lt 40 ]; do sleep 1; W=$((W + 1)); done
if kill -0 "$PS" 2>/dev/null; then echo "run_mp_web: the server is still running 40 s after the run: killing it"; killserver; else echo "run_mp_web: the server ended by itself within $W s"; fi
kill "$PB" 2>/dev/null; sleep 1; kill -9 "$PB" 2>/dev/null; [ -n "${WB:-}" ] && taskkill //PID "$WB" //F > /dev/null 2>&1
for w in ${WS:-} ${WB:-}; do
  if tasklist //FI "PID eq $w" 2>/dev/null | grep -q " $w "; then echo "run_mp_web: process $w outlived the bookkeeping: killing it"; taskkill //PID "$w" //F > /dev/null 2>&1; fi
done
echo "---- runner"
cat "$PROJECT/work/logs/mpw_${MODE}_runner.txt" | cut -c1-300
echo "---- bridge"
cut -c1-300 "$BLOG"
echo "---- server"
grep -v "%$" "$SLOG" | grep -v "inert \|^\[server\] gc profile" | cut -c1-300 | tail -40
[ "$RR" = 0 ]

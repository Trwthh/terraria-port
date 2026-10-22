#!/bin/sh
# Host & Play in the browser, end to end (web/STATUS.md 2026-10-14): a hidden headless page hosts (the dedicated
# server in a Worker of the page, the page's client plays on it), the bridge in reverse mode lets others in, and two
# other players join and leave one after the other:
#   1. the page (web/www/test/actions/mp_host.txt): import ref_small.wld -> Multiplayer -> Host & Play -> "Hoster";
#   2. the NATIVE port client (port/target-app/client/terraria.exe, hidden, muted, scripted with
#      port/crates/terraria-app/scripts/mp_join.input: Join via IP 127.0.0.1:47800 -> "Joiner" -> digs, walks, waits
#      for the host's chat "hello from A", leaves) once the page marks "hosting";
#   3. a SECOND headless page (test/actions/mp_join_page.txt: "WebJoiner", Join via IP 127.0.0.1:47800 through the
#      bridge's forward mode, digs, walks, chats "hello from web", leaves) once the page marks "native_done";
#   4. the host leaves (Save & Exit); its server ends by itself (-autoshutdown).
#
#   sh tools/ws-bridge/run_hostplay_web.sh [limit seconds, default 1500]
#   HOST_ACTIONS=test/actions/mp_host_long.txt HOST_LOG=hpl_host_page.txt: the page hosts alone for about 6 minutes
#
# The bridge: ws_bridge.py --port 7780 --listen 47800 --to-page --trace-terraria (127.0.0.1 only). The second page
# uses a browser profile of its own (work/web-profile-2, a copy of work/web-profile made once: the same OPFS import)
# and the first runner's serve.py (one origin). Every process started here is stopped by PID at the end.
# Logs: work/logs/hpw_{host_page,join_page,bridge,native}*.txt; shots work/shots/hpw_*, mpw_* (page 2), mp_b* (native).
set -u
cd "$(dirname "$0")/../.."
PROJECT="$(pwd)"
LIMIT="${1:-1500}"
BPORT=7780
TPORT=47800
STAMP="$(date +%Y%m%d-%H%M%S)"
MARKS="work/logs/marks/hpw-$STAMP"
LOGS="$PROJECT/work/logs"
mkdir -p "$LOGS" "$PROJECT/$MARKS" "$PROJECT/work/port-save-runs/_old"
EXE="$PROJECT/port/target-app/client/terraria.exe"
[ -x "$EXE" ] || { echo "missing $EXE"; exit 2; }
# the second page's browser profile: a copy of the first one's (made once, while no browser uses it)
if [ ! -d "$PROJECT/work/web-profile-2" ]; then
  echo "run_hostplay_web: copying work/web-profile to work/web-profile-2 (once)"
  cp -r "$PROJECT/work/web-profile" "$PROJECT/work/web-profile-2" || exit 2
fi
BLOG="$LOGS/hpw_bridge.log"
: > "$BLOG"
python "$PROJECT/tools/ws-bridge/ws_bridge.py" --port "$BPORT" --listen "$TPORT" --to-page --trace-terraria --log "$(cygpath -w "$BLOG")" > /dev/null 2>&1 &
PB=$!
sleep 1
WB=$(cat "/proc/$PB/winpid" 2>/dev/null)
echo "run_hostplay_web: bridge pid $PB (windows ${WB:-?}): ws://127.0.0.1:$BPORT, reverse tcp 127.0.0.1:$TPORT"
winpid() { cat "/proc/$1/winpid" 2>/dev/null; }
alive() { [ -n "$1" ] && kill -0 "$1" 2>/dev/null; }
( cd "$PROJECT/web/www" && MARKS_DIR="$MARKS" exec node test/run-game-headless.mjs --actions "${HOST_ACTIONS:-test/actions/mp_host.txt}" --bridge "ws://127.0.0.1:$BPORT" --log "${HOST_LOG:-hpw_host_page.txt}" ) > "$LOGS/hpw_host_runner.txt" 2>&1 &
P1=$!
W1=$(winpid $P1)
echo "run_hostplay_web: host page runner pid $P1 (windows ${W1:-?})"
PN=""; WN=""; P2=""; W2=""
T0=$(date +%s)
while alive "$P1"; do
  sleep 2
  T=$(( $(date +%s) - T0 ))
  if [ -z "$PN" ] && [ -f "$PROJECT/$MARKS/hosting" ]; then
    S="$PROJECT/work/port-save-runs/hpw_native"
    [ -d "$S" ] && mv "$S" "$PROJECT/work/port-save-runs/_old/hpw_native-$STAMP"
    mkdir -p "$S"
    ( cd "$PROJECT/port" && TERRARIA_NET_LOOPBACK_ONLY=1 TERRARIA_MUTE=1 TERRARIA_HIDDEN=1 TERRARIA_FRAME_TIMES=1 TERRARIA_HOST_REPORT=1 \
        TERRARIA_SAVE_DIR="$S" TERRARIA_INPUT_SCRIPT=crates/terraria-app/scripts/mp_join.input TERRARIA_STATE_LOG="$LOGS/hpw_native.state.jsonl" \
        exec "$EXE" ) > "$LOGS/hpw_native.log" 2>&1 &
    PN=$!
    WN=$(winpid $PN)
    echo "run_hostplay_web: [$T s] the page hosts: native client pid $PN (windows ${WN:-?}) joins 127.0.0.1:$TPORT"
  fi
  if [ -z "$P2" ] && [ -f "$PROJECT/$MARKS/native_done" ]; then
    ( cd "$PROJECT/web/www" && exec node test/run-game-headless.mjs --actions test/actions/mp_join_page.txt --bridge "ws://127.0.0.1:$BPORT" --no-serve --port 8147 --profile-dir work/web-profile-2 --log hpw_join_page.txt ) > "$LOGS/hpw_join_runner.txt" 2>&1 &
    P2=$!
    W2=$(winpid $P2)
    echo "run_hostplay_web: [$T s] the native client left: second page runner pid $P2 (windows ${W2:-?}) joins"
  fi
  if [ "$T" -ge "$LIMIT" ]; then
    echo "run_hostplay_web: still running after $LIMIT s: stopping everything"
    kill $P1 $P2 2>/dev/null
    sleep 5
    break
  fi
done
wait "$P1" 2>/dev/null; R1=$?
R2=0; [ -n "$P2" ] && { W=0; while alive "$P2" && [ "$W" -lt 120 ]; do sleep 1; W=$((W + 1)); done; wait "$P2" 2>/dev/null; R2=$?; }
RN=0; [ -n "$PN" ] && { W=0; while alive "$PN" && [ "$W" -lt 30 ]; do sleep 1; W=$((W + 1)); done; alive "$PN" && kill -9 "$PN" 2>/dev/null; wait "$PN" 2>/dev/null; RN=$?; }
kill "$PB" 2>/dev/null; sleep 1; kill -9 "$PB" 2>/dev/null
for w in ${WB:-} ${W1:-} ${W2:-} ${WN:-}; do
  if tasklist //FI "PID eq $w" 2>/dev/null | grep -q " $w "; then echo "run_hostplay_web: process $w outlived the bookkeeping: killing it"; taskkill //PID "$w" //T //F > /dev/null 2>&1; fi
done
echo "run_hostplay_web: host page exit $R1, native client exit $RN${PN:+ (pid $PN)}, second page exit $R2${P2:+ (pid $P2)} after $(( $(date +%s) - T0 )) s"
echo "---- host page runner"; cut -c1-300 "$LOGS/hpw_host_runner.txt" | tail -40
[ -n "$P2" ] && { echo "---- second page runner"; cut -c1-300 "$LOGS/hpw_join_runner.txt" | tail -25; }
[ -n "$PN" ] && { echo "---- native client"; grep -E "^\[shell\] (wait|script|stopped|unhandled)|^\[host\] (net|not implemented)|panicked|^error" "$LOGS/hpw_native.log" | cut -c1-250 | tail -25; }
echo "---- bridge"; cut -c1-250 "$BLOG" | grep -v "PlayerControls\|TileSection " | tail -40
echo "---- server (from the host page's log)"; grep -E "\[server\]" "$LOGS/${HOST_LOG:-hpw_host_page.txt}" | grep -v "work fiber ended\|Resetting\|Loading world\|Settling\|%$" | cut -c1-300 | tail -60
[ "$R1" = 0 ] && [ "$RN" = 0 ] && [ "$R2" = 0 ]

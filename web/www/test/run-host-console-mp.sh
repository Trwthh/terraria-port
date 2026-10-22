#!/bin/sh
# Every command of the dedicated server's console in the browser's Host & Play, with joiners (web/STATUS.md
# 2026-10-14 "Server console: every command"). A hidden headless page hosts (test/actions/host_console_mp.txt) and
# types the commands into its server console box; tools/ws-bridge (reverse mode, TCP 127.0.0.1:47800) lets the
# NATIVE port client (port/target-app/client/terraria.exe, hidden, muted, loopback only) in, started at each mark
# of the page with work/console-joiners/web/console_<mark>.input (test/tools/make_console_joiners.py):
#   join_pwwrong (wrong password: refused), join_kick (password, MOTD, kicked), join_ban (banned), join_banned
#   (refused: banned).
#   sh web/www/test/run-host-console-mp.sh [limit seconds, default 1500]
# Exit 0 when the page's checks all held and every joiner's script ended with exit 0. Logs work/logs/hpcm_*,
# console dumps work/logs/hpcm_console_{1,2}.txt (test/tools/console_diff.py compares them with the original's).
# Every process started here is stopped by PID at the end.
set -u
cd "$(dirname "$0")/../../.."
PROJECT="$(pwd)"
LIMIT="${1:-1500}"
BPORT=7780
TPORT=47800
STAMP="$(date +%Y%m%d-%H%M%S)"
MARKS="work/logs/marks/hpcm-$STAMP"
LOGS="$PROJECT/work/logs"
mkdir -p "$LOGS" "$PROJECT/$MARKS" "$PROJECT/work/port-save-runs/_old"
EXE="$PROJECT/port/target-app/client/terraria.exe"
[ -x "$EXE" ] || { echo "missing $EXE"; exit 2; }
python "$PROJECT/web/www/test/tools/make_console_joiners.py" "$TPORT" "$PROJECT/work/console-joiners/web" > /dev/null || exit 2
BLOG="$LOGS/hpcm_bridge.log"
: > "$BLOG"
python "$PROJECT/tools/ws-bridge/ws_bridge.py" --port "$BPORT" --listen "$TPORT" --to-page --trace-terraria --log "$(cygpath -w "$BLOG")" > /dev/null 2>&1 &
PB=$!
sleep 1
winpid() { cat "/proc/$1/winpid" 2>/dev/null; }
alive() { [ -n "$1" ] && kill -0 "$1" 2>/dev/null; }
WB=$(winpid $PB)
echo "run-host-console-mp: bridge pid $PB (windows ${WB:-?})"
( cd "$PROJECT/web/www" && MARKS_DIR="$MARKS" exec node test/run-game-headless.mjs --actions test/actions/host_console_mp.txt --bridge "ws://127.0.0.1:$BPORT" --log hpcm_host_page.txt ) > "$LOGS/hpcm_host_runner.txt" 2>&1 &
P1=$!
W1=$(winpid $P1)
echo "run-host-console-mp: host page runner pid $P1 (windows ${W1:-?})"
JOINERS="pwwrong kick ban banned"
PIDS=""
WPIDS=""
RJ=0
T0=$(date +%s)
start_joiner() { # $1 = name
  S="$PROJECT/work/port-save-runs/hpcm_$1"
  [ -d "$S" ] && mv "$S" "$PROJECT/work/port-save-runs/_old/hpcm_$1-$STAMP"
  mkdir -p "$S/Worlds"
  ( cd "$PROJECT/port" && TERRARIA_NET_LOOPBACK_ONLY=1 TERRARIA_MUTE=1 TERRARIA_HIDDEN=1 TERRARIA_HOST_REPORT=1 TERRARIA_SAVE_DIR="$S" \
      TERRARIA_INPUT_SCRIPT="$(cygpath -w "$PROJECT/work/console-joiners/web/console_$1.input")" TERRARIA_STATE_LOG="$LOGS/hpcm_join_$1.state.jsonl" \
      exec "$EXE" ) > "$LOGS/hpcm_join_$1.log" 2>&1 &
  eval "PJ_$1=$!"
  PIDS="$PIDS $!"
  WPIDS="$WPIDS $(winpid $!)"
  echo "run-host-console-mp: [$(( $(date +%s) - T0 )) s] mark join_$1: native client pid $! joins 127.0.0.1:$TPORT"
}
while alive "$P1"; do
  sleep 1
  for j in $JOINERS; do
    eval "started=\${PJ_$j:-}"
    [ -z "$started" ] && [ -f "$PROJECT/$MARKS/join_$j" ] && start_joiner "$j"
    # the joiner saw the MOTD in its chat: the page may kick / ban it now (waitmark motd_<joiner>)
    [ -n "$started" ] && [ ! -f "$PROJECT/$MARKS/motd_$j" ] && grep -q 'wait "chat Console MOTD for joiners": holds' "$LOGS/hpcm_join_$j.log" 2>/dev/null &&       echo "$(( $(date +%s) - T0 ))" > "$PROJECT/$MARKS/motd_$j"
  done
  if [ $(( $(date +%s) - T0 )) -ge "$LIMIT" ]; then
    echo "run-host-console-mp: still running after $LIMIT s: stopping everything"
    kill $P1 2>/dev/null
    sleep 5
    break
  fi
done
wait "$P1" 2>/dev/null; R1=$?
for j in $JOINERS; do
  eval "p=\${PJ_$j:-}"
  [ -z "$p" ] && { echo "run-host-console-mp: joiner $j was never started"; RJ=1; continue; }
  W=0; while alive "$p" && [ "$W" -lt 60 ]; do sleep 1; W=$((W + 1)); done
  alive "$p" && { echo "run-host-console-mp: joiner $j still running: killing it"; kill -9 "$p" 2>/dev/null; }
  wait "$p" 2>/dev/null; r=$?
  echo "run-host-console-mp: joiner $j exit $r ($(grep -E "^\[shell\] (script|stopped|unhandled)|wait .*(timed out|failed)" "$LOGS/hpcm_join_$j.log" | tail -2 | tr '\n' ' ' | cut -c1-250))"
  [ "$r" = 0 ] || RJ=1
done
kill "$PB" 2>/dev/null; sleep 1; kill -9 "$PB" 2>/dev/null
for w in ${WB:-} ${W1:-} $WPIDS; do
  if tasklist //FI "PID eq $w" 2>/dev/null | grep -q " $w "; then echo "run-host-console-mp: process $w outlived the bookkeeping: killing it"; taskkill //PID "$w" //T //F > /dev/null 2>&1; fi
done
echo "run-host-console-mp: host page exit $R1, joiners $([ "$RJ" = 0 ] && echo ok || echo FAILED) after $(( $(date +%s) - T0 )) s"
echo "---- host page runner"; cut -c1-300 "$LOGS/hpcm_host_runner.txt" | tail -25
echo "---- server (from the host page's log)"; grep -E "\[server\]" "$LOGS/hpcm_host_page.txt" | grep -v "work fiber ended\|Resetting\|Loading world\|Settling\|%$" | cut -c1-300 | tail -40
[ "$R1" = 0 ] && [ "$RJ" = 0 ]

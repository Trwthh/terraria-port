#!/bin/sh
# One hidden, muted, scripted session of the translated client. Nothing touches the real keyboard, mouse or screen.
#
#   sh crates/terraria-app/scripts/run_hidden.sh <script.input> [name] [seconds]
#
# - a FRESH save root work/port-save-runs/<name> (a previous one is moved to work/port-save-runs/_old/, never
#   deleted), with a copy of work/refsave/Worlds/ref_small.wld in its Worlds folder (the original is not touched);
#   KEEP_SAVE=1 keeps the existing save root instead (a second session on the same saves);
#   NO_WORLD=1 starts without the copied world (world creation from the menu)
# - log: work/logs/<name>.log; state dumps of the script: work/logs/<name>.state.jsonl
# - the process is killed (by PID) after [seconds] (default 900) if it has not ended on its own
# - PROFILE=client-opt (default client) picks the build; other TERRARIA_* variables pass through (TERRARIA_KEEP_GOING=1 ...)
# - AUDIO_DEVICE=1: not muted, the real output device at TERRARIA_VOLUME (default 0.03); otherwise TERRARIA_MUTE=1
set -u
cd "$(dirname "$0")/../../.."          # port/
SCRIPT="$1"
NAME="${2:-$(basename "$SCRIPT" .input)}"
LIMIT="${3:-900}"
PROFILE="${PROFILE:-client}"
PROJECT="$(cd .. && pwd -W 2>/dev/null || (cd .. && pwd))"
SAVE="$PROJECT/work/port-save-runs/$NAME"
EXE="./target-app/$PROFILE/terraria.exe"
[ -x "$EXE" ] || { echo "missing $EXE (build: CARGO_TARGET_DIR=target-app cargo build -p terraria-app --bin terraria --features client --profile $PROFILE -j 4)"; exit 2; }
mkdir -p "$PROJECT/work/logs" "$PROJECT/work/shots" "$PROJECT/work/port-save-runs/_old"
if [ "${KEEP_SAVE:-0}" != 1 ]; then
  [ -d "$SAVE" ] && mv "$SAVE" "$PROJECT/work/port-save-runs/_old/$NAME-$(date +%Y%m%d-%H%M%S)"
  mkdir -p "$SAVE/Worlds"
  # NO_WORLD=1: an empty Worlds folder (a session that creates its world from the menu)
  [ "${NO_WORLD:-0}" = 1 ] || cp "$PROJECT/work/refsave/Worlds/ref_small.wld" "$SAVE/Worlds/ref_small.wld"
fi
LOG="$PROJECT/work/logs/$NAME.log"
# muted unless AUDIO_DEVICE=1: then the real output device is opened at TERRARIA_VOLUME (default 0.03, the limit for
# test runs), for the one listening check; the mixer's peak / RMS are printed at exit either way
if [ "${AUDIO_DEVICE:-0}" = 1 ]; then
  unset TERRARIA_MUTE
  export TERRARIA_VOLUME="${TERRARIA_VOLUME:-0.03}"
else
  export TERRARIA_MUTE=1
fi
TERRARIA_SAVE_DIR="$SAVE" TERRARIA_HIDDEN=1 TERRARIA_INPUT_SCRIPT="$SCRIPT" \
TERRARIA_STATE_LOG="$PROJECT/work/logs/$NAME.state.jsonl" TERRARIA_FRAME_TIMES=1 TERRARIA_HOST_REPORT=1 \
  "$EXE" > "$LOG" 2>&1 &
PID=$!
T=0
while kill -0 "$PID" 2>/dev/null; do
  sleep 1
  T=$((T + 1))
  if [ "$T" -ge "$LIMIT" ]; then
    echo "run_hidden: still running after $LIMIT s: killing PID $PID" | tee -a "$LOG"
    kill -9 "$PID" 2>/dev/null
    break
  fi
done
wait "$PID" 2>/dev/null
RC=$?
echo "run_hidden: $NAME exit code $RC after $T s; log $LOG"
grep -E "^\[terraria\]|^\[shell\]|^\[host\]|^\[unported\]|panicked|^error" "$LOG" | cut -c1-${WIDTH:-400} | tail -${LINES_MAX:-40}
exit $RC

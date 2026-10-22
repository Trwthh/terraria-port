#!/bin/sh
# Frame pacing, quiet and under a synthetic CPU load, for one or more variants of the same executable:
#   sh pace_ab.sh <tag> [seconds=60] -- <name>[:KEY=VALUE,KEY=VALUE..] ...
# e.g. sh pace_ab.sh in_pacing 60 -- old:TERRARIA_BEVY_MT=1,TERRARIA_COARSE_TIMER=1 new
# Each variant: rw.py launch (no activation, outside the desktop, muted, no game pad), enter_world.py, pace_load.py
# (which starts and kills its own busy-loop processes), rw.py kill. Results: work/logs/<tag>.txt (appended), the
# frame logs work/logs/rw_<tag>_<name>.frames.tsv, the foreground logs rw_<tag>_<name>.foreground.log.
# RW_EXE picks the executable (default: the client-opt build).
cd "$(dirname "$0")"
TAG="$1"; SECS="${2:-60}"
shift 2
[ "$1" = "--" ] && shift
export PYTHONIOENCODING=utf-8
for v in "$@"; do
  name="${v%%:*}"
  vars=""
  case "$v" in *:*) vars="$(echo "${v#*:}" | tr ',' ' ')" ;; esac
  echo "== $name ($vars)"
  echo "## variant $name: ${vars:-defaults}; exe ${RW_EXE:-client-opt}" >> ../../../../../work/logs/$TAG.txt
  # shellcheck disable=SC2086
  python rw.py launch --name "${TAG}_$name" TERRARIA_NO_GAMEPAD=1 $vars | tail -1 | cut -c1-120
  python enter_world.py || { python rw.py kill; continue; }
  python pace_load.py "$SECS" "$TAG"
  python rw.py kill
  tail -1 "../../../../../work/logs/rw_${TAG}_$name.foreground.log"
done

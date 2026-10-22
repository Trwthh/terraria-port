#!/bin/sh
# The dedicated server's console in the browser, end to end (web/STATUS.md 2026-10-14 "Server console"): a hidden
# headless page hosts (Host & Play) and types help, say, time, noon, dawn, playing, save and exit into the page's
# server console box (test/actions/host_console.txt). Exit 0 when every check of the actions file held.
#   sh web/www/test/run-host-console.sh
# Log work/logs/hpc_host_page.txt, shots work/shots/hpc_*. The runner starts serve.py and Chromium itself and stops
# both at the end (test/run-game-headless.mjs).
cd "$(dirname "$0")/.." || exit 2
node test/run-game-headless.mjs --actions test/actions/host_console.txt --log hpc_host_page.txt
R=$?
echo "run-host-console: exit $R (log work/logs/hpc_host_page.txt)"
exit $R

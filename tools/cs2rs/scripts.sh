#!/bin/sh
# The in-game script regression suite: every port/crates/terraria-app/scripts/g*.input through the headless survey
# (tests/client.rs with CLIENT_SCRIPT: the app's call sequence without a GPU or a window, every call caught on its
# own), as ONE command. A script fails when the survey reports a stop (something the port does not have, or an
# uncaught C# exception), a failed script command, an unimplemented void host member or a gap. Exit code 1 then.
#
#   sh tools/cs2rs/scripts.sh [name-pattern]        e.g. sh tools/cs2rs/scripts.sh g1    (default: all)
#
# - a script that needs a world gets work/refsave/Worlds/ref_small.wld in its scratch save root (CLIENT_WORLD)
# - g11_reload.input (a second start on existing saves) runs on the save root the suite's g7_play left
#   (CLIENT_SAVE_FROM), so g7_play runs before it whatever the pattern order
# - one build (target-gen/fast, as p1.sh), then one run per script; logs: port/target-gen/gates/script-<name>.txt
# - opaque objects that are only created (TcpClient..ctor, as in Proof 1) are not failures
cd "$(dirname "$0")"
PROJECT="$(cd ../.. && pwd -W 2>/dev/null || (cd ../.. && pwd))"
PORT="$PROJECT/port"
SCRIPTS="$PORT/crates/terraria-app/scripts"
OUT="$PORT/target-gen/gates"
mkdir -p "$OUT"
PATTERN="${1:-g}"
python mk_client_survey.py > /dev/null
cd "$PORT" || exit 2
export CARGO_PROFILE_DEV_OPT_LEVEL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=target-gen/fast
# build once; the runs below only start the test binary
cargo test -p terraria-gen-game -j 4 --test client --no-run > "$OUT/script-build.txt" 2>&1 || { echo "script suite: the survey does not build, see $OUT/script-build.txt"; exit 1; }
SAVE_OF_PLAY="$PORT/target-gen/fast/tmp/client-save-g7_play"
FAILED=0
# g7_play first (g11_reload continues on its save), then name order
LIST="$(ls "$SCRIPTS"/g*.input | sed 's/.*\///' | grep "$PATTERN" | sort -V)"
ORDERED="$(echo "$LIST" | grep '^g7_play' ; echo "$LIST" | grep -v '^g7_play')"
for f in $ORDERED; do
  name="${f%.input}"
  log="$OUT/script-$name.txt"
  t0=$(date +%s)
  unset CLIENT_SAVE_FROM
  export CLIENT_WORLD="$PROJECT/work/refsave/Worlds/ref_small.wld"
  if [ "$name" = g11_reload ]; then
    if [ ! -d "$SAVE_OF_PLAY" ]; then
      echo "script $name: FAIL (needs the save root of g7_play: run the suite with a pattern that includes it)"
      FAILED=1
      continue
    fi
    export CLIENT_SAVE_FROM="$SAVE_OF_PLAY"
    unset CLIENT_WORLD
  fi
  CLIENT_SCRIPT="$SCRIPTS/$f" cargo test -p terraria-gen-game -j 4 --test client -- --nocapture > "$log" 2>&1
  t1=$(date +%s)
  if [ "$name" = g7_play ]; then
    rm -rf "$SAVE_OF_PLAY"
    cp -r "$PORT/target-gen/fast/tmp/client-save" "$SAVE_OF_PLAY" 2>/dev/null
  fi
  stops=$(grep "^client script: " "$log" | sed 's/^client script: \([0-9]*\) distinct.*/\1/')
  cmdfail=$(grep -c "^state tick .*FAILED" "$log")
  gaps=$(grep "^gaps reached: " "$log" | sed 's/.*: //')
  # unimplemented members other than constructors of opaque objects
  unimpl=$(grep "^    unimpl " "$log" | grep -vc "\.\.ctor")
  if [ "$stops" = 0 ] && [ "$cmdfail" = 0 ] && [ "$gaps" = 0 ] && [ "$unimpl" = 0 ] && grep -q "^test result: ok" "$log"; then
    echo "script $name: ok ($((t1 - t0)) s; $(grep '^client script: ' "$log" | sed 's/^client script: 0 distinct stop(s); //'))"
  else
    echo "script $name: FAIL ($((t1 - t0)) s; stops ${stops:-no report}, failed commands $cmdfail, gaps ${gaps:-?}, unimplemented void members $unimpl; see $log)"
    grep -E "^STOP|^state tick .*FAILED|^    gap |^    unimpl " "$log" | cut -c1-300 | head -8
    FAILED=1
  fi
done
exit $FAILED

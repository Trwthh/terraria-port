#!/bin/sh
# One of the whole game's own proofs (port/crates/terraria-gen-game/tests/<test>.rs) as a wasm32-wasip1 binary under
# Node's WASI: the same reference data (refdata, the player's install) as the native gate, so the comparison of the
# translated game with the real game is repeated with the game compiled to WebAssembly.
#
#   sh web/ci/wasm_game_test.sh <test> [extra test args]      e.g. proof1, sim, ticks, worldgen
#
# Build: nightly, -Zbuild-std, wasm exception handling, profile client-opt (the generated crate at opt-level 2: its
# largest functions would exceed V8's limits on locals at opt-level 0), a 1 GB wasm stack (the native tests run on a
# 256 MB - 1 GB thread; terraria_host::run_with_stack calls directly on wasm). Log: work/logs/wasm_<test>.txt.
set -u
TEST="$1"; shift
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LOG="$ROOT/work/logs/wasm_$TEST.txt"
cd "$ROOT/port"
export CARGO_TARGET_DIR=target-wasm
export RUSTFLAGS="-C panic=unwind -C target-feature=+exception-handling -C link-arg=-zstack-size=1073741824"
T0=$(date +%s)
EXE="$(cargo +nightly-x86_64-pc-windows-gnu test -p terraria-gen-game --test "$TEST" --no-run --target wasm32-wasip1 --profile client-opt -j 2 \
  -Zbuild-std=std,panic_unwind,test --message-format=json 2>"$LOG.build" | grep -o '"executable":"[^"]*\.wasm"' | sed 's/"executable":"//;s/"$//;s/\\\\/\//g' | head -1)"
T1=$(date +%s)
if [ -z "$EXE" ]; then echo "wasm_game_test: build failed ($((T1 - T0)) s)"; grep -E "^error" -A6 "$LOG.build" | head -30; exit 2; fi
echo "wasm_game_test: built in $((T1 - T0)) s: $(stat -c %s "$EXE") bytes"
node --no-warnings "$ROOT/web/ci/wasi-run.mjs" "$(cygpath -w "$ROOT")" "$(cygpath -w "$EXE")" --nocapture --test-threads 1 "$@" > "$LOG" 2>&1
RC=$?
echo "wasm_game_test: $TEST exit $RC after $(( $(date +%s) - T1 )) s; log $LOG"
grep -E "test result|proof 1|passed|FAILED|panicked|RuntimeError" "$LOG" | tail -8
exit $RC

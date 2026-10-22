#!/bin/sh
# Stage 0 of the browser port (web/WEB_PORT.md section 9): a crate's own test suites (every test binary: the library's
# unit tests and each integration test) on wasm32-wasip1, under Node's WASI, one fresh instance per test (a test that
# traps cannot take the others with it).
#
#   sh web/ci/wasm_tests.sh <crate>      (from anywhere, Git Bash)
#
# Build: the pinned nightly with `-Zbuild-std` and wasm exception handling (C# `try` is `catch_unwind`; stable traps
# on wasm32, WEB_PORT.md section 1.6). Output in port/target-wasm. The native build is untouched.
# The runner preopens the project root as `/w` (tests that need a scratch folder use work/wasm-tmp there).
# Writes web/logs/stage0-<crate>.txt; exit 1 when a test fails (ignored tests are skipped, as natively).
set -u
CRATE="$1"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LOG="$ROOT/web/logs/stage0-$CRATE.txt"
cd "$ROOT/port"
export CARGO_TARGET_DIR=target-wasm
export RUSTFLAGS="-C panic=unwind -C target-feature=+exception-handling"
OUT="$(cargo +nightly-x86_64-pc-windows-gnu test -p "$CRATE" --target wasm32-wasip1 --no-run -j 2 \
  -Zbuild-std=std,panic_unwind,test --message-format=json 2>/tmp/wasm_build_$CRATE.txt)"
RC=$?
WASMS="$(echo "$OUT" | grep -o '"executable":"[^"]*\.wasm"' | sed 's/"executable":"//;s/"$//;s/\\\\/\//g' | sort -u)"
if [ $RC -ne 0 ] || [ -z "$WASMS" ]; then
  echo "wasm_tests: the build failed" | tee "$LOG"; grep -E "^error" -A5 /tmp/wasm_build_$CRATE.txt | head -40 | tee -a "$LOG"; exit 2
fi
WROOT="$(cygpath -w "$ROOT")"
pass=0; fail=0; skip=0; partial=0; bins=0; : > "$LOG"
echo "$WASMS" | grep -v '/examples/' > /tmp/wasm_bins_$CRATE.txt
while IFS= read -r WASM; do
  bins=$((bins + 1))
  WWASM="$(cygpath -w "$WASM")"
  run() { node --no-warnings "$ROOT/web/ci/wasi-run.mjs" "$WROOT" "$WWASM" "$@"; }
  echo "== $(basename "$WASM") ($(stat -c %s "$WASM") bytes)" >> "$LOG"
  for t in $(run --list --format terse 2>/dev/null | grep ': test$' | sed 's/: test$//'); do
    if run --exact "$t" --test-threads 1 --nocapture > /tmp/wasm_one.txt 2>&1 && grep -q "test result: ok. 1 passed" /tmp/wasm_one.txt; then
      if grep -q "WASM-PARTIAL:" /tmp/wasm_one.txt; then
        echo "ok   $t (partial: $(grep -o 'WASM-PARTIAL: .*' /tmp/wasm_one.txt | head -1 | cut -c15-))" >> "$LOG"; partial=$((partial + 1))
      else
        echo "ok   $t" >> "$LOG"
      fi
      pass=$((pass + 1))
    elif grep -q "test result: ok. 0 passed; 0 failed; 1 ignored" /tmp/wasm_one.txt; then
      echo "skip $t (ignored, as natively)" >> "$LOG"; skip=$((skip + 1))
    else
      echo "FAIL $t: $(grep -E 'panicked|RuntimeError|rror' /tmp/wasm_one.txt | head -2 | tr '\n' ' ' | cut -c1-240)" >> "$LOG"; fail=$((fail + 1))
    fi
  done
done < /tmp/wasm_bins_$CRATE.txt
echo "wasm_tests: $CRATE on wasm32-wasip1: $pass passed ($partial partly: host limits), $fail failed, $skip ignored, in $bins test binaries" | tee -a "$LOG"
[ "$fail" = 0 ]

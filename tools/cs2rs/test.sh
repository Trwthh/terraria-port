#!/bin/sh
# Regenerate, then run the runtime tests, the snippets and the comparison with the real game.
# Usage: sh test.sh            (CS2RS_ONLY=collision,lighting sh test.sh for a subset of the reference sections)
set -e
cd "$(dirname "$0")"
sh run.sh
cd ../../port
cargo test -p terraria-gen-rt -j 4 --message-format short 2>&1 | grep -E "^error|test result|FAILED|panicked" | cut -c1-300
cargo test -p terraria-gen -j 4 --test reference --message-format short -- --nocapture 2>&1 | grep -v "^   Compiling\|^\[unported\]\|^\[host\]" | cut -c1-400 | tail -${LINES_OUT:-200}

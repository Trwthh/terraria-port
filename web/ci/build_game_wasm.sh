#!/bin/sh
# Builds the browser module: port/crates/terraria-web as a wasm32-wasip1 reactor -> web/www/game.wasm.
#   sh web/ci/build_game_wasm.sh            (Git Bash; about 15 minutes after a change to the generated crate)
# nightly + -Zbuild-std + wasm exception handling (C# try = catch_unwind), profile client-opt (the generated crate at
# opt-level 2), a 64 MB wasm shadow stack for the main stack (the game recurses deeply; the native client thread is
# 512 MB; work fibers get 32 MB shadow stacks of their own, threading.rs wasm_fibers), exports _initialize / tw_init / tw_frame, and for the work
# fibers tw_fiber_entry + __stack_pointer (the page swaps it at every fiber switch), and the dedicated server of Host & Play
# tw_server_init + tw_server_tick (a second Worker runs them on its own instance) (HOST_ABI.md).
set -eu
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT/port"
export CARGO_TARGET_DIR=target-wasm
export RUSTFLAGS="-C panic=unwind -C target-feature=+exception-handling -C link-arg=-zstack-size=67108864 -C link-arg=--export=tw_init -C link-arg=--export=tw_frame -C link-arg=--export=tw_fiber_entry -C link-arg=--export=__stack_pointer -C link-arg=--export=tw_server_init -C link-arg=--export=tw_server_tick"
T0=$(date +%s)
# a failed build stops here (before, the grep pipeline hid cargo's status and the old module was copied as new)
LOG="$ROOT/port/target-wasm/build_game_wasm.log"
if ! cargo +nightly-x86_64-pc-windows-gnu build -p terraria-web --lib --target wasm32-wasip1 --profile client-opt -j 2 -Zbuild-std=std,panic_unwind > "$LOG" 2>&1; then
  grep -E "^(error|warning: unused)|^ *-->" "$LOG" | head -40
  echo "build_game_wasm: cargo failed (log $LOG); web/www/game.wasm NOT replaced"
  exit 1
fi
grep -E "^(error|warning: unused)|Finished" "$LOG" | head -20
cp target-wasm/wasm32-wasip1/client-opt/terraria_web.wasm "$ROOT/web/www/game.wasm"
echo "build_game_wasm: $(stat -c %s "$ROOT/web/www/game.wasm") bytes in $(( $(date +%s) - T0 )) s -> web/www/game.wasm"
node --no-warnings -e "
const fs=require('fs');const m=new WebAssembly.Module(fs.readFileSync(process.argv[1]));
const imp={};for(const i of WebAssembly.Module.imports(m))(imp[i.module]??=[]).push(i.name);
console.log('imports:',Object.entries(imp).map(([k,v])=>k+' ('+v.length+'): '+v.join(' ')).join(' | '));
console.log('exports:',WebAssembly.Module.exports(m).map(e=>e.name).filter(n=>!n.startsWith('__')).join(' '));" "$(cygpath -w "$ROOT/web/www/game.wasm")"

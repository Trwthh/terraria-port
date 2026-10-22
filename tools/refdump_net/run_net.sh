#!/bin/sh
# The multiplayer reference run: the original server (RefDumpNet.exe server, NetServer.cs: TerrariaServer.exe in-process)
# and the original client (RefDumpDraw.exe net, tools/refdump_draw/NetClient.cs: Terraria.exe in-process, hidden desktop)
# in lock step over a named pipe of this machine; their network is virtual (NetVirt.cs): no socket is opened, nothing
# listens, nothing is broadcast. Then the JIT twins (RefJitNet.exe, RefJitDraw.exe net) run the same and dump the machine
# code of every method that ran: work/jit/ref/net_server, work/jit/client/net; their data must equal refdata/net.
#
#   sh tools/refdump_net/run_net.sh [--build] [--refdata] [--nojit]
#
#   --build    tools/refdump_net/build.cmd and tools/refdump_draw/build.cmd first
#   --refdata  write refdata/net (the plain harnesses); without it only the twins run (into work/jit/*/data) and their
#              data is compared with refdata/net
#   --nojit    no twin run
# Both processes are time-boxed (watchdogs inside, and killed by PID here after LIMIT seconds); none is left running.
cd "$(dirname "$0")/../.."
LIMIT=${LIMIT:-5400}
PIPE=VaelRefNet
for a in "$@"; do
  case "$a" in
    --build) cmd //c "tools\\refdump_net\\build.cmd" || exit 1; cmd //c "tools\\refdump_draw\\build.cmd" || exit 1 ;;
    --refdata) REFDATA=1 ;;
    --nojit) NOJIT=1 ;;
    *) echo "unknown option $a"; exit 2 ;;
  esac
done
mkdir -p work/logs

# pair <server command...> -- <client command...>: both started, both waited for, killed after LIMIT
pair() {
  name=$1; shift
  srv=""; while [ "$1" != "--" ]; do srv="$srv \"$1\""; shift; done; shift
  t0=$(date +%s)
  eval "$srv" > "work/logs/net_${name}_server.log" 2>&1 < /dev/null &
  sp=$!
  "$@" > "work/logs/net_${name}_client.log" 2>&1 < /dev/null &
  cp=$!
  t=0
  while kill -0 "$sp" 2>/dev/null || kill -0 "$cp" 2>/dev/null; do
    sleep 2; t=$((t + 2))
    if [ "$t" -ge "$LIMIT" ]; then echo "$name: still running after $t s: killing PIDs $sp $cp"; kill -9 "$sp" "$cp" 2>/dev/null; break; fi
  done
  wait "$sp" 2>/dev/null; src=$?
  wait "$cp" 2>/dev/null; crc=$?
  echo "$name: server exit $src, client exit $crc after $(($(date +%s) - t0)) s; $(grep -E '^done:|FATAL|WATCHDOG' work/logs/net_${name}_server.log | head -2 | tr '\n' ' ') $(grep -E '^done:|FAILED|WATCHDOG|joined' work/logs/net_${name}_client.log | head -3 | tr '\n' ' ')"
  [ "$src" = 0 ] && [ "$crc" = 0 ]
}

W() { cygpath -w "$PWD/$1"; }
RC=0
if [ -n "$REFDATA" ]; then
  if [ -d refdata/net ]; then mkdir -p work/_archive/refdata_net; mv refdata/net "work/_archive/refdata_net/net_$(date +%Y%m%d-%H%M%S)"; fi
  mkdir -p refdata/net
  pair refdata tools/refdump_net/bin/RefDumpNet.exe server "$(W refdata/net)" $PIPE 5000 -- \
       env REFDRAW_WATCHDOG=5000 tools/refdump_draw/bin/RefDumpDraw.exe "$(W refdata/net)" net $PIPE || RC=1
fi
if [ -z "$NOJIT" ] && [ "$RC" = 0 ]; then
  for d in work/jit/ref/net_server work/jit/client/net; do
    if [ -d "$d" ]; then mkdir -p work/_archive/jit_net; mv "$d" "work/_archive/jit_net/$(basename "$d")_$(date +%Y%m%d-%H%M%S)"; fi
  done
  mkdir -p work/jit/ref/net_server work/jit/client/net/data
  pair jit tools/refdump_net/bin/RefJitNet.exe "$(W work/jit/ref/net_server)" server "$(W work/jit/client/net/data)" $PIPE 5000 -- \
       env REFDRAW_WATCHDOG=5000 tools/refdump_draw/bin/RefJitDraw.exe "$(W work/jit/client/net)" "$(W work/jit/client/net/data)" net $PIPE || RC=1
  same=0; diff=0
  for f in refdata/net/*; do
    n=$(basename "$f")
    case "$n" in *timing*) continue ;; esac
    if cmp -s "$f" "work/jit/client/net/data/$n"; then same=$((same + 1)); else diff=$((diff + 1)); echo "differs: $n"; fi
  done
  echo "net: $same files of the twins equal to refdata/net, $diff different"
  [ "$diff" -eq 0 ] || RC=1
fi
tasklist //FO CSV 2>/dev/null | grep -iE "RefDumpNet|RefJitNet|RefDumpDraw|RefJitDraw" && echo "WARNING: a harness process is still running"
exit $RC

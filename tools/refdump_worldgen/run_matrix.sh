#!/bin/sh
# The world generation reference matrix: the real game (RefDumpWorldgenX.exe genx, GenX.cs) generates each world of
# the table below and writes refdata/worldgen/matrix/<tag>.{passes.jsonl,columns.bin,summary.json,wld}.
#
#   sh tools/refdump_worldgen/run_matrix.sh [tag ...]          (default: every world of the table)
#   SNAPS=default|i,j,.. sh tools/refdump_worldgen/run_matrix.sh <tag>     also tile snapshots (diagnosis; large files)
#   OUT=<folder> ...                                            another output folder (a check run)
#
# Headless: no window, no socket, no console input. Each run is its own process with a watchdog inside (third
# column, seconds) and is killed by PID here if it is still alive 60 s after that. Scratch worlds and save roots:
# work/worldgen-scratch/<tag>/. Nothing is written below work/refserver or the game folder (the harness refuses).
# The existing files of the "12345" / small scenario (refdata/worldgen/ref_small.*) are not touched: `check` reruns
# that scenario through this harness into a scratch folder and compares the hashes.
cd "$(dirname "$0")/../.."
ROOT="$(pwd -W 2>/dev/null || pwd)"
EXE="tools/refdump_worldgen/bin/RefDumpWorldgenX.exe"
OUT="${OUT:-$ROOT/refdata/worldgen/matrix}"
SNAPS="${SNAPS:-none}"
[ -x "$EXE" ] || cmd //c "tools\\refdump_worldgen\\build_x.cmd" || exit 1
mkdir -p "$OUT" work/logs

# tag            watchdog  seed text   size    game mode  evil        flags
MATRIX='
ref_medium       1500      2718281     medium  expert     corruption  -
ref_large        3000      31415926    large   classic    random      -
ref_bees         900       404         small   classic    random      notthebees
ref_drunk        900       5162        small   classic    random      drunk
ref_worthy       900       1337        small   master     crimson     fortheworthy
ref_celebration  900       3001        small   classic    random      celebration
ref_constant     900       3002        small   classic    random      theconstant
ref_notraps      900       3003        small   classic    random      notraps
ref_remix        900       3004        small   classic    random      remix
ref_zenith       1200      3005        small   classic    random      zenith
ref_skyblock     900       3006        small   classic    random      skyblock
ref_sec_error    900       2001        small   classic    random      secret.errorWorld
ref_sec_desert   900       2002        small   classic    random      secret.surfaceIsDesert,secret.extraLiquid,secret.rainbowStuff,secret.roundLandmasses,secret.digExtraHoles,secret.pooEverywhere,secret.actuallyNoTraps,secret.portalGunInChests,secret.randomSpawn,secret.teamBasedSpawns
ref_sec_frozen   900       2003        small   classic    random      secret.worldIsFrozen,secret.dualDungeons,secret.startInHardmode,secret.hallowOnTheSurface,secret.worldIsInfected,secret.biggerAbandonedHouses,secret.addTeleporters,secret.extraLivingTrees,secret.extraFloatingIslands
ref_sec_mush     900       2004        small   classic    random      secret.surfaceIsMushrooms,secret.noSurface,secret.surfaceIsInSpace,secret.paintEverythingGray,secret.coatEverythingEcho,secret.noInfection,secret.noSpiderCaves,secret.graveyardBloodmoonStart,secret.halloweenGen
ref_sec_liquid   900       2005        small   classic    random      secret.extraLiquid,secret.rainsForAYear,secret.paintEverythingNegative,secret.dualDungeons
ref_trapsworthy  900       2006        small   classic    random      notraps,fortheworthy
ref_shimmerocean 900       2007        small   classic    random      drunk,celebration
ref_dual_a       900       2008        small   classic    random      secret.dualDungeons
'

run_one() {
  tag="$1"; limit="$2"; seed="$3"; size="$4"; mode="$5"; evil="$6"; flags="$7"
  log="work/logs/worldgen-ref-$tag.log"
  t0=$(date +%s)
  "$EXE" genx "$OUT" "$tag" "$seed" "$size" "$mode" "$evil" "$flags" passes "$SNAPS" "$limit" > "$log" 2>&1 &
  pid=$!
  t=0
  while kill -0 "$pid" 2>/dev/null; do
    sleep 2; t=$((t + 2))
    if [ "$t" -ge $((limit + 60)) ]; then echo "$tag: still running after $t s: killing PID $pid"; kill -9 "$pid" 2>/dev/null; break; fi
  done
  wait "$pid" 2>/dev/null
  rc=$?
  t1=$(date +%s)
  echo "$tag: exit $rc after $((t1 - t0)) s; $(grep -E '^generation |^FATAL|WATCHDOG' "$log" | head -2 | tr '\n' ' ')"
  return $rc
}

if [ "$1" = check ]; then
  # the reference scenario through the new harness must give the reference files, byte for byte
  OUT="$ROOT/work/worldgen-scratch/_xcheck"
  mkdir -p "$OUT"
  run_one ref_small 900 12345 small classic random - || exit 1
  a=$(sha256sum "$OUT/ref_small.passes.jsonl" | cut -d' ' -f1); b=$(sha256sum refdata/worldgen/ref_small.passes.jsonl | cut -d' ' -f1)
  c=$(sha256sum "$OUT/ref_small.columns.bin" | cut -d' ' -f1); d=$(sha256sum refdata/worldgen/ref_small.columns.bin | cut -d' ' -f1)
  if [ "$a" = "$b" ] && [ "$c" = "$d" ]; then echo "check: ref_small.passes.jsonl and ref_small.columns.bin identical to refdata/worldgen ($a, $c)"; exit 0; fi
  echo "check: DIFFERENT from refdata/worldgen"; exit 1
fi

FAILED=0
WANT=" $* "
while read -r tag limit seed size mode evil flags; do
  [ -z "$tag" ] && continue
  if [ "$WANT" != "  " ]; then case "$WANT" in *" $tag "*) ;; *) continue ;; esac; fi
  run_one "$tag" "$limit" "$seed" "$size" "$mode" "$evil" "$flags" < /dev/null || FAILED=1
done <<EOF
$MATRIX
EOF
# no harness process may be left behind
tasklist //FO CSV 2>/dev/null | grep -i "RefDumpWorldgenX" && echo "WARNING: a harness process is still running"
exit $FAILED

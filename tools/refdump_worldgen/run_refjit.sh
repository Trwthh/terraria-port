#!/bin/sh
# Every reference run again, each followed by a dump of the machine code of every game method that ran in it
# (JitDump.cs DumpRanGame), and the proof that each run is the one that wrote its refdata (refjit_compare.py).
#
#   sh tools/refdump_worldgen/run_refjit.sh [scenario ...]        (default: all of the table below)
#
# Output: work/jit/ref/<scenario>/ (jit_code.bin, jit_code.json, jit_callees.json, jit_method_callees.json,
# jit_cells.json, jit_targets.json; data/ = what the harness wrote, never refdata), logs work/jit/ref/<scenario>.log,
# work/jit/ref/compare.txt. Headless: no window, no socket; one process at a time, each with an in-process watchdog and
# killed by PID here when it is still alive after the limit of the table. Then:
#   python tools/cs2rs/gen_jit_refcalls.py && python tools/cs2rs/gen_jit_calls.py && python tools/cs2rs/gen_jit_bodies.py
cd "$(dirname "$0")/../.."
B=tools/refdump_worldgen/bin
J=work/jit/ref
W=work/refsave/Worlds/ref_small.wld
mkdir -p "$J"
[ -x "$B/RefJitEntities.exe" ] || cmd //c "tools\\refdump_worldgen\\build_refjit.cmd" || exit 1

# scenario     limit(s)  command (JIT dir first where the wrapper takes it)
TABLE='
p1               600     RefJitP1.exe          $J/p1 p1 $J/p1/data
framed           900     RefJitP1.exe          $J/framed framed $W $J/framed/data
framed_scrambled 900     RefJitP1.exe          $J/framed_scrambled framed $W $J/framed_scrambled/data scramble
collision        1500    RefDumpCollisionJit.exe $J/collision/data $J/collision
light            900     RefDumpLightJit.exe   $J/light/data $J/light
sky              600     RefJitSky.exe         $J/sky sky $W $J/sky/data
liquid_settle_trace  600 RefJitLiquid.exe      $J/liquid_settle_trace liquid $W $J/liquid_settle_trace/data settle_trace
liquid_lake_drain    600 RefJitLiquid.exe      $J/liquid_lake_drain liquid $W $J/liquid_lake_drain/data lake_drain
liquid_surface_fill  600 RefJitLiquid.exe      $J/liquid_surface_fill liquid $W $J/liquid_surface_fill/data surface_fill
liquid_shimmer_drain 600 RefJitLiquid.exe      $J/liquid_shimmer_drain liquid $W $J/liquid_shimmer_drain/data shimmer_drain
liquid_honey_drain   600 RefJitLiquid.exe      $J/liquid_honey_drain liquid $W $J/liquid_honey_drain/data honey_drain
liquid_lava_open     600 RefJitLiquid.exe      $J/liquid_lava_open liquid $W $J/liquid_lava_open/data lava_open
ticks_world      1200    RefJitTicks.exe       $J/ticks_world world $J/ticks_world/data 3600
ticks_player     900     RefJitTicks.exe       $J/ticks_player player $J/ticks_player/data 600
entities_npc     1800    RefJitEntities.exe    $J/entities_npc npc $J/entities_npc/data 120
entities_proj    1800    RefJitEntities.exe    $J/entities_proj proj $J/entities_proj/data 120
entities_item    2400    RefJitEntities.exe    $J/entities_item item $J/entities_item/data 120
entities_citem   7200    RefJitEntities.exe    $J/entities_citem citem $J/entities_citem/data 60
worldgen_small   1500    RefJitWorldgen.exe    $J/worldgen_small $J/worldgen_small/data main passes task
wg_ref_medium    2400    RefJitWorldgenX.exe   $J/wg_ref_medium genx $J/wg_ref_medium/data ref_medium 2718281 medium expert corruption - passes none 2300
wg_ref_large     4000    RefJitWorldgenX.exe   $J/wg_ref_large genx $J/wg_ref_large/data ref_large 31415926 large classic random - passes none 3900
wg_ref_bees      1500    RefJitWorldgenX.exe   $J/wg_ref_bees genx $J/wg_ref_bees/data ref_bees 404 small classic random notthebees passes none 1400
wg_ref_drunk     1500    RefJitWorldgenX.exe   $J/wg_ref_drunk genx $J/wg_ref_drunk/data ref_drunk 5162 small classic random drunk passes none 1400
wg_ref_worthy    1500    RefJitWorldgenX.exe   $J/wg_ref_worthy genx $J/wg_ref_worthy/data ref_worthy 1337 small master crimson fortheworthy passes none 1400
wg_ref_celebration 1500    RefJitWorldgenX.exe   $J/wg_ref_celebration genx $J/wg_ref_celebration/data ref_celebration 3001 small classic random celebration passes none 1400
wg_ref_constant   1500    RefJitWorldgenX.exe   $J/wg_ref_constant genx $J/wg_ref_constant/data ref_constant 3002 small classic random theconstant passes none 1400
wg_ref_notraps    1500    RefJitWorldgenX.exe   $J/wg_ref_notraps genx $J/wg_ref_notraps/data ref_notraps 3003 small classic random notraps passes none 1400
wg_ref_remix      1500    RefJitWorldgenX.exe   $J/wg_ref_remix genx $J/wg_ref_remix/data ref_remix 3004 small classic random remix passes none 1400
wg_ref_zenith     1800    RefJitWorldgenX.exe   $J/wg_ref_zenith genx $J/wg_ref_zenith/data ref_zenith 3005 small classic random zenith passes none 1400
wg_ref_skyblock   1500    RefJitWorldgenX.exe   $J/wg_ref_skyblock genx $J/wg_ref_skyblock/data ref_skyblock 3006 small classic random skyblock passes none 1400
wg_ref_sec_error  1500    RefJitWorldgenX.exe   $J/wg_ref_sec_error genx $J/wg_ref_sec_error/data ref_sec_error 2001 small classic random secret.errorWorld passes none 1400
wg_ref_sec_desert 1500    RefJitWorldgenX.exe   $J/wg_ref_sec_desert genx $J/wg_ref_sec_desert/data ref_sec_desert 2002 small classic random secret.surfaceIsDesert,secret.extraLiquid,secret.rainbowStuff,secret.roundLandmasses,secret.digExtraHoles,secret.pooEverywhere,secret.actuallyNoTraps,secret.portalGunInChests,secret.randomSpawn,secret.teamBasedSpawns passes none 1400
wg_ref_sec_frozen 1500    RefJitWorldgenX.exe   $J/wg_ref_sec_frozen genx $J/wg_ref_sec_frozen/data ref_sec_frozen 2003 small classic random secret.worldIsFrozen,secret.dualDungeons,secret.startInHardmode,secret.hallowOnTheSurface,secret.worldIsInfected,secret.biggerAbandonedHouses,secret.addTeleporters,secret.extraLivingTrees,secret.extraFloatingIslands passes none 1400
wg_ref_sec_mush   1500    RefJitWorldgenX.exe   $J/wg_ref_sec_mush genx $J/wg_ref_sec_mush/data ref_sec_mush 2004 small classic random secret.surfaceIsMushrooms,secret.noSurface,secret.surfaceIsInSpace,secret.paintEverythingGray,secret.coatEverythingEcho,secret.noInfection,secret.noSpiderCaves,secret.graveyardBloodmoonStart,secret.halloweenGen passes none 1400
wg_ref_sec_liquid 1500    RefJitWorldgenX.exe   $J/wg_ref_sec_liquid genx $J/wg_ref_sec_liquid/data ref_sec_liquid 2005 small classic random secret.extraLiquid,secret.rainsForAYear,secret.paintEverythingNegative,secret.dualDungeons passes none 1400
wg_ref_trapsworthy 1500   RefJitWorldgenX.exe   $J/wg_ref_trapsworthy genx $J/wg_ref_trapsworthy/data ref_trapsworthy 2006 small classic random notraps,fortheworthy passes none 1400
wg_ref_shimmerocean 1500  RefJitWorldgenX.exe   $J/wg_ref_shimmerocean genx $J/wg_ref_shimmerocean/data ref_shimmerocean 2007 small classic random drunk,celebration passes none 1400
wg_ref_dual_a     1500    RefJitWorldgenX.exe   $J/wg_ref_dual_a genx $J/wg_ref_dual_a/data ref_dual_a 2008 small classic random secret.dualDungeons passes none 1400
'

FAILED=0
WANT=" $* "
echo "$TABLE" | while read -r name limit exe rest; do
  [ -z "$name" ] && continue
  if [ "$WANT" != "  " ]; then case "$WANT" in *" $name "*) ;; *) continue ;; esac; fi
  args=$(eval echo "$rest")
  # an earlier dump of this scenario is moved to work/_archive/jit_ref/<date>/ (never deleted)
  if [ -e "$J/$name" ]; then a="work/_archive/jit_ref/$(date +%Y%m%d-%H%M%S)"; mkdir -p "$a"; mv "$J/$name" "$a/"; [ -e "$J/$name.log" ] && mv "$J/$name.log" "$a/"; fi
  mkdir -p "$J/$name"
  log="$J/$name.log"
  t0=$(date +%s)
  # shellcheck disable=SC2086
  "$B/$exe" $args > "$log" 2>&1 < /dev/null &
  pid=$!
  t=0
  while kill -0 "$pid" 2>/dev/null; do
    sleep 2; t=$((t + 2))
    if [ "$t" -ge "$limit" ]; then echo "$name: still running after $t s: killing PID $pid"; kill -9 "$pid" 2>/dev/null; break; fi
  done
  wait "$pid" 2>/dev/null
  rc=$?
  t1=$(date +%s)
  echo "$name: exit $rc after $((t1 - t0)) s; $(grep -E '^dumpran: [0-9]+ of|FATAL|WATCHDOG' "$log" | head -2 | tr '\n' ' ')"
done
tasklist //FO CSV 2>/dev/null | grep -iE "RefJit|RefDump" && echo "WARNING: a harness process is still running"
python tools/refdump_worldgen/refjit_compare.py | tee "$J/compare.txt"

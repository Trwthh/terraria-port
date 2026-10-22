#!/bin/sh
# Every gate of the port, one after the other, and ONE summary table at the end. Exit code 1 when a gate failed.
#
#   sh tools/cs2rs/gates.sh [--nogen] [--slow] [--only <name>[,<name>..]] [--skip <name>[,..]]
#
# Gates (names for --only / --skip):
#   p1            regenerate (unless --nogen) + Proof 1 (tests/proof1.rs: 3,911,380 values against the real game)
#   client-whole  the client start-up as single calls (tests/client.rs, CLIENT_WHOLE=1)
#   rt            cargo test -p terraria-gen-rt (runtime, collector, snippets)
#   sim           tests/sim.rs (collision, framing, lighting against the real game)
#   ticks         tests/ticks.rs, both scenarios (server ticks against the real server)
#   host          cargo test -p terraria-host
#   app           cargo test -p terraria-app (shell runs, hidden)
#   viewer        prototype viewer regression screenshot (SHA-1 35b0971b...); builds the viewer on demand
#                 (port/target-app/debug is a regenerable cache: when it was deleted this gate rebuilds it)
#   scripts       every crates/terraria-app/scripts/g*.input through the headless survey (no GPU, no window): fails
#                 on any stop, exception, failed script command, unimplemented void host member or gap. Among them
#                 g15_create_world (world creation from the menu) and g16_language (the language switch)
#   draw          tests/draw.rs: what the client DRAWS, call by call against the original client (refdata/draw,
#                 tools/refdump_draw: the real Terraria.exe on a recording XNA stand-in, no GPU, no window): title,
#                 character creation, in the world by day / night, ocean, cave, underworld, boss + projectiles,
#                 inventory, full-screen map; every SpriteBatch / device / effect call of the recorded frames bit for
#                 bit and the per-frame digest of all 730 frames of the run
#   audio         tests/draw.rs with DRAW_MODE=audio: the audio run (refdata/audio, RefDumpDraw.exe audio: sound / music /
#                 ambience on, menu clicks, walking on grass, mining, an enemy hit, item use, music across day / night /
#                 space / ocean / underworld): every call the game's audio makes at XNA's engine boundary (SoundEffect /
#                 instance / XACT, with the XAudio2 voice volume, output matrix and frequency ratio from XNA's own kernel
#                 code), all frames, bit for bit (tools/refdump_draw/AudioRec.cs, terraria_host::audio::trace); the draw
#                 scenes of that run and the digest of all its frames; no XACT cue may run out by itself
#   scenes        tests/draw.rs with DRAW_MODE=scenes: the scenes run (refdata/scenes, RefDumpDraw.exe scenes): menus
#                 (multiplayer, characters, worlds, world creation), nine biomes, six events, the settings pages, crafting,
#                 housing, NPC chat / shop, bestiary, emotes, a chest, camera mode, a mount, pets / a minion, the smart
#                 cursor, three lighting modes and all 18 bosses: every recorded scene call by call, the digest of all
#                 frames, and per scene the state line (Main.rand, the player, every NPC and projectile)
#   gallery       tests/draw.rs with DRAW_MODE=gallery: the gallery run (refdata/gallery, RefDumpDraw.exe gallery,
#                 tools/refdump_draw/Gallery.cs): the old background drawing, every particle, every ambient sky entity,
#                 every projectile and NPC type drawn next to the player, world events (meteor, a shaken tree, hardmode,
#                 a demon altar, the lunar pillars), the player's death and respawn; scenes, digests, state lines
#   interact      tests/draw.rs with DRAW_MODE=interact: the interaction run (refdata/interact, RefDumpDraw.exe
#                 interact, tools/refdump_draw/Interact.cs): every held / channelled weapon used (whips, kites,
#                 yo-yos, flails, spears, drills, fishing poles), a wired contraption, chest operations, the Old
#                 One's Army at difficulty 1 - 3, shimmer, town NPCs arriving, player states, UI details, fishing
#   cover         tests/draw.rs with DRAW_MODE=cover: the coverage run (refdata/cover, RefDumpDraw.exe cover,
#                 tools/refdump_draw/Cover.cs): display dolls holding every kind of weapon in every pose, hat racks,
#                 doors, a trapdoor, a Christmas tree, minecart tracks, teleporters, a logic gate, the inventory's
#                 banner / recipe / housing pages, the map saved and loaded, every accessory, every mount, three skies,
#                 the Wall of Flesh summoned; scenes, digests, state lines
#   probe         tests/draw.rs with DRAW_MODE=probe: the probe run (refdata/probe, RefDumpDraw.exe probe,
#                 tools/refdump_draw/Probe*.cs): the float methods no other run reached, each called or brought about
#                 with the state that makes it work (players, NPCs, projectiles, world generation helpers, UI states,
#                 menus, effects, capture, pylons), with "probe" lines of their results; scenes, digests, state lines
#   net           the multiplayer reference run (refdata/net, tools/refdump_net: the original server and the original
#                 client in lock step, virtual connections): tests/net.rs replays the client's bytes into the translated
#                 server and compares every byte it sends and its state per tick; tests/draw.rs DRAW_MODE=net replays
#                 the server's bytes into the translated client and compares every byte it sends per frame, the scenes,
#                 the digests and the state lines
# Slow gates, only with --slow or when named with --only:
#   worldgen      Proof 2 on every world: the reference scenario (108 rows required) and the fifteen worlds of
#                 refdata/worldgen/matrix (medium, large, not the bees, drunk, for the worthy, celebration, the constant,
#                 no traps, remix, zenith, skyblock, and four of secret seeds: error world; desert surface and nine
#                 more; frozen / dual dungeons / hardmode start and six more; mushroom surface / no surface / space and
#                 six more), every row required; about 65 minutes, the large world needs 3 GB
#   entities      tests/entities.rs, npc, proj and item (every NPC / projectile type and every usable item of
#                 refdata/entities, 120 ticks, bit for
#                 bit against the real server; tools/refdump_entities): fails when a type differs that
#                 tools/cs2rs/entities_allow.txt does not list (each listed one with its site), or a listed one is
#                 exact; about 10 minutes
#   client-items  tests/entities.rs with ENTITIES_KIND=citem: every usable item used by the OWNING player of a single
#                 player game for 60 ticks at a fixed target (shooting, placing / digging, consumables, mana, ammo;
#                 refdata/entities/citem.*, tools/refdump_entities/ClientItems.cs), bit for bit, with the tiles around the
#                 player per tick; the same allow file; about 7 minutes
#   multiplayer   builds the client executable (profile client) and runs the two-process proof
#                 (crates/terraria-app/scripts/run_multiplayer.sh: Host & Play + Join via IP on 127.0.0.1:47800,
#                 hidden, TERRARIA_NET_LOOPBACK_ONLY=1); fails unless both processes end with exit code 0
#   window-modes  builds the client executable (profile client) and runs the two real-window sessions of
#                 crates/terraria-app/scripts/realwin/window_modes.py (the game's borderless / fullscreen /
#                 maximise / resolution switches applied against TERRARIA_FAKE_MONITOR). The ONE gate with a
#                 visible window: it is created without activation, outside the desktop, driven by window
#                 messages, and rw.py's guard ends the process if it ever shows up on the desktop or in front.
#                 Fails when an expectation of the script does not hold
#
# No other gate opens a visible window; none touches the real keyboard / mouse. Logs: port/target-gen/gates/<name>.txt.
# Build directories as everywhere: target-gen/fast (generated crate, no optimisation), target-host2, target-app
# (target-app/debug without debug info here: CARGO_PROFILE_DEV_DEBUG=0; all of them are regenerable caches).
cd "$(dirname "$0")"
TOOLS="$(pwd)"
PORT="$(cd ../../port && pwd)"
PROJECT="$(cd ../.. && pwd)"
NOGEN=""
ONLY=""
SKIP=""
SLOW=""
while [ $# -gt 0 ]; do
  case "$1" in
    --nogen) NOGEN="--nogen" ;;
    --slow) SLOW=1 ;;
    --only) ONLY=",$2,"; shift ;;
    --skip) SKIP=",$2,"; shift ;;
    *) echo "gates.sh: unknown argument $1"; exit 2 ;;
  esac
  shift
done
OUT="$PORT/target-gen/gates"
mkdir -p "$OUT"
SUMMARY="$OUT/summary.txt"
: > "$SUMMARY"
FAILED=0

wanted() {
  [ -n "$ONLY" ] && { case "$ONLY" in *",$1,"*) ;; *) return 1 ;; esac; }
  case "$SKIP" in *",$1,"*) return 1 ;; esac
  return 0
}
# a slow gate runs with --slow, or when --only names it
wanted_slow() {
  case "$SKIP" in *",$1,"*) return 1 ;; esac
  [ -n "$ONLY" ] && { case "$ONLY" in *",$1,"*) return 0 ;; *) return 1 ;; esac; }
  [ -n "$SLOW" ]
}
# record <name> <ok|FAIL> <seconds> <detail>
record() {
  printf '| %-13s | %-4s | %5s s | %s |\n' "$1" "$2" "$3" "$4" >> "$SUMMARY"
  [ "$2" = ok ] || FAILED=1
  echo "gate $1: $2 ($3 s) $4"
}
# cargo_gate <name> <log> <command...>: ok when every "test result" line is ok and there is at least one
cargo_gate() {
  name="$1"; log="$2"; shift 2
  t0=$(date +%s)
  "$@" > "$log" 2>&1
  t1=$(date +%s)
  results=$(grep -c "^test result:" "$log")
  bad=$(grep "^test result:" "$log" | grep -vc "^test result: ok")
  passed=$(grep "^test result:" "$log" | sed 's/.*ok\. \([0-9]*\) passed.*/\1/' | awk '{s+=$1} END {print s+0}')
  if [ "$results" -gt 0 ] && [ "$bad" = 0 ] && ! grep -q "^error" "$log"; then
    record "$name" ok $((t1 - t0)) "$passed tests passed"
  else
    record "$name" FAIL $((t1 - t0)) "see $log: $(grep -E "^error|FAILED|panicked" "$log" | head -1 | cut -c1-120)"
  fi
}
FAST="env CARGO_PROFILE_DEV_OPT_LEVEL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=target-gen/fast"

if wanted p1; then
  t0=$(date +%s)
  sh "$TOOLS/p1.sh" $NOGEN > "$OUT/p1.txt" 2>&1
  t1=$(date +%s)
  if grep -q "^proof 1 passed" "$OUT/p1.txt"; then
    record p1 ok $((t1 - t0)) "$(grep -c 'values compared' "$OUT/p1.txt") dumps, $(grep 'not exact' "$OUT/p1.txt" | sed 's/.*compared, \([0-9]*\) exact.*/\1/' | awk '{s+=$1} END {print s}') values exact"
  else
    record p1 FAIL $((t1 - t0)) "see $OUT/p1.txt"
  fi
fi
if wanted client-whole; then
  t0=$(date +%s)
  sh "$TOOLS/p1.sh" --client-whole --nogen > "$OUT/client-whole.txt" 2>&1
  t1=$(date +%s)
  if grep -q "^test result: ok" "$OUT/client-whole.txt"; then
    record client-whole ok $((t1 - t0)) "start-up as single calls"
  else
    record client-whole FAIL $((t1 - t0)) "see $OUT/client-whole.txt"
  fi
fi
cd "$PORT" || exit 2
wanted rt && cargo_gate rt "$OUT/rt.txt" cargo test -p terraria-gen-rt -j 4
wanted sim && cargo_gate sim "$OUT/sim.txt" $FAST cargo test -p terraria-gen-game -j 4 --test sim -- --nocapture
if wanted ticks; then
  cargo_gate ticks-player "$OUT/ticks-player.txt" $FAST cargo test -p terraria-gen-game -j 4 --test ticks -- --nocapture
  cargo_gate ticks-world "$OUT/ticks-world.txt" env TICKS_SCENARIO=world $FAST cargo test -p terraria-gen-game -j 4 --test ticks -- --nocapture
fi
wanted host && cargo_gate host "$OUT/host.txt" env CARGO_TARGET_DIR=target-host2 cargo test -p terraria-host -j 3
# the app gates build without debug info: with it every exe of the app is 1.8 GB and there are eleven of them
# (five binaries, their unit tests, the integration test): 24 GB instead of about 5
wanted app && cargo_gate app "$OUT/app.txt" env CARGO_PROFILE_DEV_DEBUG=0 CARGO_TARGET_DIR=target-app cargo test -p terraria-app -j 3
if wanted viewer; then
  t0=$(date +%s)
  PNG="$OUT/viewer.png"
  rm -f "$PNG"
  CARGO_PROFILE_DEV_DEBUG=0 CARGO_TARGET_DIR=target-app cargo build -p terraria-app -j 3 > "$OUT/viewer.txt" 2>&1
  # hidden window; killed by PID if it does not end on its own
  TERRARIA_WORLD=../work/refsave/Worlds/ref_small.wld TERRARIA_VIEW_TILE=2100,600 TERRARIA_UNLIT=1 TERRARIA_HIDDEN=1 \
    TERRARIA_SCREENSHOT="$PNG" TERRARIA_SCREENSHOT_FRAME=60 ./target-app/debug/terraria-app.exe >> "$OUT/viewer.txt" 2>&1 &
  PID=$!
  T=0
  while kill -0 "$PID" 2>/dev/null; do
    sleep 1; T=$((T + 1))
    [ "$T" -ge 180 ] && { kill -9 "$PID" 2>/dev/null; break; }
  done
  wait "$PID" 2>/dev/null
  t1=$(date +%s)
  SHA=$(sha1sum "$PNG" 2>/dev/null | cut -d' ' -f1)
  if [ "$SHA" = 35b0971bcf5e7ddfc1bba3d86445de457c79a41a ]; then
    record viewer ok $((t1 - t0)) "SHA-1 $SHA"
  else
    record viewer FAIL $((t1 - t0)) "SHA-1 ${SHA:-no screenshot}, expected 35b0971bcf5e7ddfc1bba3d86445de457c79a41a"
  fi
fi
if wanted scripts; then
  t0=$(date +%s)
  sh "$TOOLS/scripts.sh" > "$OUT/scripts.txt" 2>&1
  rc=$?
  t1=$(date +%s)
  detail="$(grep -c '^script .*: ok' "$OUT/scripts.txt") of $(grep -c '^script ' "$OUT/scripts.txt") scripts without a stop"
  if [ "$rc" = 0 ]; then record scripts ok $((t1 - t0)) "$detail"; else record scripts FAIL $((t1 - t0)) "$detail; $(grep '^script .*FAIL' "$OUT/scripts.txt" | head -3 | tr '\n' ' ' | cut -c1-200)"; fi
fi
if wanted draw; then
  t0=$(date +%s)
  log="$OUT/draw.txt"
  $FAST cargo test -p terraria-gen-game -j 4 --test draw -- --nocapture > "$log" 2>&1
  t1=$(date +%s)
  calls=$(grep '^==== draw ' "$log" | sed 's/^==== draw [a-z_]*: \([0-9]*\) of \([0-9]*\) calls.*/\1 \2/' | awk '{e+=$1; n+=$2} END {print e+0 " of " n+0}')
  detail="$(grep -c '^==== draw ' "$log") scenes, $calls calls exact; $(grep '^==== digest' "$log" | sed 's/^==== digest: //')"
  if grep -q "^test result: ok" "$log"; then
    record draw ok $((t1 - t0)) "$detail"
  else
    record draw FAIL $((t1 - t0)) "$detail; $(grep -E '^==== draw .*first difference|panicked' "$log" | head -1 | cut -c1-140); see $log"
  fi
fi
if wanted audio; then
  t0=$(date +%s)
  log="$OUT/audio.txt"
  env DRAW_MODE=audio $FAST cargo test -p terraria-gen-game -j 4 --test draw -- --nocapture > "$log" 2>&1
  t1=$(date +%s)
  calls=$(grep '^==== audio: [0-9]* of' "$log" | sed 's/^==== audio: \([0-9]* of [0-9]*\) calls.*/\1/')
  scenes=$(grep '^==== draw ' "$log" | sed 's/^==== draw [a-z_]*: \([0-9]*\) of \([0-9]*\) calls.*/\1 \2/' | awk '{e+=$1; n+=$2} END {print e+0 " of " n+0}')
  detail="audio $calls calls exact; draw $(grep -c '^==== draw ' "$log") scenes, $scenes calls; $(grep '^==== digest' "$log" | sed 's/^==== digest: //'); cues run out: $(grep '^==== audio: XACT cues' "$log" | sed 's/.*: //')"
  if grep -q "^test result: ok" "$log"; then
    record audio ok $((t1 - t0)) "$detail"
  else
    record audio FAIL $((t1 - t0)) "$detail; $(grep -E '^==== (audio|draw) .*first divergence|panicked' "$log" | head -1 | cut -c1-140); see $log"
  fi
fi
if wanted scenes; then
  t0=$(date +%s)
  log="$OUT/scenes.txt"
  env DRAW_MODE=scenes $FAST cargo test -p terraria-gen-game -j 4 --test draw -- --nocapture > "$log" 2>&1
  t1=$(date +%s)
  calls=$(grep '^==== draw ' "$log" | sed 's/^==== draw [a-z0-9_]*: \([0-9]*\) of \([0-9]*\) calls.*/\1 \2/' | awk '{e+=$1; n+=$2} END {print e+0 " of " n+0}')
  detail="$(grep -c '^==== draw ' "$log") scenes, $calls calls exact; $(grep '^==== digest' "$log" | sed 's/^==== digest: //'); state $(grep '^==== state' "$log" | sed 's/^==== state: //' | cut -c1-60)"
  if grep -q "^test result: ok" "$log"; then
    record scenes ok $((t1 - t0)) "$detail"
  else
    record scenes FAIL $((t1 - t0)) "$detail; $(grep -E '^==== draw .*first divergence|panicked' "$log" | head -1 | cut -c1-140); see $log"
  fi
fi
if wanted gallery; then
  t0=$(date +%s)
  log="$OUT/gallery.txt"
  env DRAW_MODE=gallery $FAST cargo test -p terraria-gen-game -j 4 --test draw -- --nocapture > "$log" 2>&1
  t1=$(date +%s)
  calls=$(grep '^==== draw ' "$log" | sed 's/^==== draw [a-z0-9_]*: \([0-9]*\) of \([0-9]*\) calls.*/\1 \2/' | awk '{e+=$1; n+=$2} END {print e+0 " of " n+0}')
  detail="$(grep -c '^==== draw ' "$log") scenes, $calls calls exact; $(grep '^==== digest' "$log" | sed 's/^==== digest: //'); state $(grep '^==== state' "$log" | sed 's/^==== state: //' | cut -c1-60)"
  if grep -q "^test result: ok" "$log"; then
    record gallery ok $((t1 - t0)) "$detail"
  else
    record gallery FAIL $((t1 - t0)) "$detail; $(grep -E '^==== draw .*first divergence|^==== digest: first|panicked' "$log" | head -1 | cut -c1-140); see $log"
  fi
fi
if wanted interact; then
  t0=$(date +%s)
  log="$OUT/interact.txt"
  env DRAW_MODE=interact $FAST cargo test -p terraria-gen-game -j 4 --test draw -- --nocapture > "$log" 2>&1
  t1=$(date +%s)
  calls=$(grep '^==== draw ' "$log" | sed 's/^==== draw [a-z0-9_]*: \([0-9]*\) of \([0-9]*\) calls.*/\1 \2/' | awk '{e+=$1; n+=$2} END {print e+0 " of " n+0}')
  detail="$(grep -c '^==== draw ' "$log") scenes, $calls calls exact; $(grep '^==== digest' "$log" | sed 's/^==== digest: //'); state $(grep '^==== state' "$log" | sed 's/^==== state: //' | cut -c1-60)"
  if grep -q "^test result: ok" "$log"; then
    record interact ok $((t1 - t0)) "$detail"
  else
    record interact FAIL $((t1 - t0)) "$detail; $(grep -E '^==== draw .*first divergence|^==== digest: first|panicked' "$log" | head -1 | cut -c1-140); see $log"
  fi
fi
if wanted cover; then
  t0=$(date +%s)
  log="$OUT/cover.txt"
  env DRAW_MODE=cover $FAST cargo test -p terraria-gen-game -j 4 --test draw -- --nocapture > "$log" 2>&1
  t1=$(date +%s)
  calls=$(grep '^==== draw ' "$log" | sed 's/^==== draw [a-z0-9_]*: \([0-9]*\) of \([0-9]*\) calls.*/\1 \2/' | awk '{e+=$1; n+=$2} END {print e+0 " of " n+0}')
  detail="$(grep -c '^==== draw ' "$log") scenes, $calls calls exact; $(grep '^==== digest' "$log" | sed 's/^==== digest: //'); state $(grep '^==== state' "$log" | sed 's/^==== state: //' | cut -c1-60)"
  if grep -q "^test result: ok" "$log"; then
    record cover ok $((t1 - t0)) "$detail"
  else
    record cover FAIL $((t1 - t0)) "$detail; $(grep -E '^==== draw .*first divergence|^==== digest: first|panicked' "$log" | head -1 | cut -c1-140); see $log"
  fi
fi
if wanted probe; then
  t0=$(date +%s)
  log="$OUT/probe.txt"
  env DRAW_MODE=probe $FAST cargo test -p terraria-gen-game -j 4 --test draw -- --nocapture > "$log" 2>&1
  t1=$(date +%s)
  calls=$(grep '^==== draw ' "$log" | sed 's/^==== draw [a-z0-9_]*: \([0-9]*\) of \([0-9]*\) calls.*/\1 \2/' | awk '{e+=$1; n+=$2} END {print e+0 " of " n+0}')
  detail="$(grep -c '^==== draw ' "$log") scenes, $calls calls exact; $(grep '^==== digest' "$log" | sed 's/^==== digest: //'); state $(grep '^==== state' "$log" | sed 's/^==== state: //' | cut -c1-60)"
  if grep -q "^test result: ok" "$log"; then
    record probe ok $((t1 - t0)) "$detail"
  else
    record probe FAIL $((t1 - t0)) "$detail; $(grep -E '^==== draw .*first divergence|^==== digest: first|panicked' "$log" | head -1 | cut -c1-140); see $log"
  fi
fi
if wanted net; then
  t0=$(date +%s)
  slog="$OUT/net-server.txt"; clog="$OUT/net-client.txt"
  $FAST cargo test -p terraria-gen-game -j 4 --test net -- --nocapture > "$slog" 2>&1
  src=$?
  env DRAW_MODE=net $FAST cargo test -p terraria-gen-game -j 4 --test draw -- --nocapture > "$clog" 2>&1
  t1=$(date +%s)
  detail="server: $(grep '^==== net server: [0-9]* of' "$slog" | sed 's/^==== net server: //' | cut -c1-90); client: $(grep '^==== net client: [0-9]* of' "$clog" | sed 's/^==== net client: //' | cut -c1-70); $(grep -c '^==== draw ' "$clog") scenes, $(grep '^==== digest' "$clog" | sed 's/^==== digest: //')"
  if grep -q "^test result: ok" "$slog" && grep -q "^test result: ok" "$clog"; then
    record net ok $((t1 - t0)) "$detail"
  else
    record net FAIL $((t1 - t0)) "$detail; $(grep -hE '^==== net (server|client): first|^==== draw .*first divergence|panicked' "$slog" "$clog" | head -1 | cut -c1-160); see $slog, $clog"
  fi
fi
if wanted_slow worldgen; then
  # the reference scenario, then the matrix; one process per world (each needs a fresh game state)
  for w in ref_small ref_medium ref_large ref_bees ref_drunk ref_worthy ref_celebration ref_constant ref_notraps ref_remix ref_zenith ref_skyblock \
           ref_sec_error ref_sec_desert ref_sec_frozen ref_sec_mush ref_sec_liquid ref_trapsworthy ref_shimmerocean ref_dual_a; do
    t0=$(date +%s)
    log="$OUT/worldgen-$w.txt"
    if [ "$w" = ref_small ]; then
      env WORLDGEN_REQUIRE=108 $FAST cargo test -p terraria-gen-game -j 4 --test worldgen -- --nocapture > "$log" 2>&1
    else
      env WORLDGEN_WORLD=$w WORLDGEN_REQUIRE=all $FAST cargo test -p terraria-gen-game -j 4 --test worldgen -- --nocapture > "$log" 2>&1
    fi
    t1=$(date +%s)
    detail="$(grep '^==== Proof 2' "$log" | sed 's/^==== Proof 2[^:]*: //' | cut -c1-110)"
    if grep -q "^test result: ok" "$log" && grep -q "^==== no difference" "$log"; then
      record "worldgen-$w" ok $((t1 - t0)) "$detail"
    else
      record "worldgen-$w" FAIL $((t1 - t0)) "$detail; $(grep -E '^==== first difference|^==== stopped|panicked' "$log" | head -1 | cut -c1-160); see $log"
    fi
  done
fi
if wanted_slow entities; then
  for k in npc proj item; do
    t0=$(date +%s)
    log="$OUT/entities-$k.txt"
    env ENTITIES_KIND=$k ENTITIES_ALLOW="$(cygpath -w "$TOOLS/entities_allow.txt")" $FAST cargo test -p terraria-gen-game -j 4 --test entities -- --nocapture > "$log" 2>&1
    t1=$(date +%s)
    detail="$(grep "^==== $k: types run" "$log" | sed "s/^==== $k: //; s/; table.*//")"
    if grep -q "^test result: ok" "$log"; then
      record "entities-$k" ok $((t1 - t0)) "$detail"
    else
      record "entities-$k" FAIL $((t1 - t0)) "$detail; $(grep -E '^FAIL|panicked' "$log" | head -1 | cut -c1-140); see $log"
    fi
  done
fi
if wanted_slow client-items; then
  t0=$(date +%s)
  log="$OUT/client-items.txt"
  env ENTITIES_KIND=citem ENTITIES_ALLOW="$(cygpath -w "$TOOLS/entities_allow.txt")" $FAST cargo test -p terraria-gen-game -j 4 --test entities -- --nocapture > "$log" 2>&1
  t1=$(date +%s)
  detail="$(grep "^==== citem: types run" "$log" | sed "s/^==== citem: //; s/; table.*//")"
  if grep -q "^test result: ok" "$log"; then
    record client-items ok $((t1 - t0)) "$detail"
  else
    record client-items FAIL $((t1 - t0)) "$detail; $(grep -E '^FAIL|panicked' "$log" | head -1 | cut -c1-140); see $log"
  fi
fi
if wanted_slow multiplayer; then
  t0=$(date +%s)
  CARGO_TARGET_DIR=target-app cargo build -p terraria-app --bin terraria --features client --profile client -j 4 > "$OUT/multiplayer-build.txt" 2>&1
  if [ $? != 0 ]; then
    record multiplayer FAIL $(( $(date +%s) - t0 )) "the client does not build, see $OUT/multiplayer-build.txt"
  else
    sh crates/terraria-app/scripts/run_multiplayer.sh 900 > "$OUT/multiplayer.txt" 2>&1
    rc=$?
    t1=$(date +%s)
    detail="$(grep '^run_multiplayer: A exit' "$OUT/multiplayer.txt" | sed 's/^run_multiplayer: //')"
    if [ "$rc" = 0 ]; then record multiplayer ok $((t1 - t0)) "$detail"; else record multiplayer FAIL $((t1 - t0)) "$detail; see $OUT/multiplayer.txt and work/logs/mp_a.log, mp_b.log"; fi
  fi
fi

if wanted_slow window-modes; then
  t0=$(date +%s)
  CARGO_TARGET_DIR=target-app cargo build -p terraria-app --bin terraria --features client --profile client -j 4 > "$OUT/window-modes-build.txt" 2>&1
  if [ $? != 0 ]; then
    record window-modes FAIL $(( $(date +%s) - t0 )) "the client does not build, see $OUT/window-modes-build.txt"
  else
    RW="$PORT/crates/terraria-app/scripts/realwin"
    ( export RW_EXE="$PORT/target-app/client/terraria.exe" PYTHONIOENCODING=utf-8
      python "$RW/window_modes.py" title && python "$RW/window_modes.py" maximized ) > "$OUT/window-modes.txt" 2>&1
    rc=$?
    t1=$(date +%s)
    bad=$(grep -c "NOT AS EXPECTED" "$OUT/window-modes.txt")
    steps=$(grep -c "^[0-9][0-9]_" "$OUT/window-modes.txt")
    guard=$(grep -c "the guard fired 0 times" "$OUT/window-modes.txt")
    if [ "$rc" = 0 ] && [ "$bad" = 0 ] && [ "$guard" = 2 ]; then record window-modes ok $((t1 - t0)) "$steps steps as expected; never on the desktop, never in front"; else record window-modes FAIL $((t1 - t0)) "$steps steps, $bad lines not as expected; see $OUT/window-modes.txt"; fi
  fi
fi

echo
echo "| gate          | state | time    | detail |"
echo "|---|---|---|---|"
cat "$SUMMARY"
exit $FAILED

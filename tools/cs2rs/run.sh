#!/bin/sh
# Rebuild the translator and regenerate everything it writes:
#   port/crates/terraria-gen-rt/src/snip        the snippet tests (tests/*.cs)
#   port/crates/terraria-gen/src/gen            the core set (core.json): the crate of the reference test
#   port/crates/terraria-gen-game/src/gen       the WHOLE game + ReLogic (game.json), with the host crate's tables
#   SCALE_REPORT.md / HOST_API.md (core), GAME_REPORT.md / HOST_API_GAME.md (whole game)
# Deterministic: a second run reports "(0 changed)" three times. Usage: sh run.sh [--diagnostics] [--reach <rust::path>]
# The snippets and the core set are generated without native.d/host_*.json (--skip-native host_): terraria-gen-rt and
# terraria-gen do not depend on terraria-host.
set -e
cd "$(dirname "$0")"
python gen_native_xna.py | tail -1
python gen_il_sizes.py | tail -1
# a failed translator build stops here (the old cs2rs.dll would otherwise regenerate everything unchanged)
if ! dotnet build -c Release -nologo -v q > obj/build.log 2>&1; then grep -E " error " obj/build.log | sed 's/\[.*//' | sort -u; echo "run.sh: the translator does not build"; exit 1; fi
grep -E "rror\(s\)" obj/build.log
dotnet bin/Release/net8.0/cs2rs.dll --skip-native host_ --config tests/snippets.json --snippets tests --out ../../port/crates/terraria-gen-rt/src/snip | tail -2
grep "^failed\|^skipped\|^unported" ../../port/crates/terraria-gen-rt/src/snip/UNPORTED.txt | cut -c1-260 || true
dotnet bin/Release/net8.0/cs2rs.dll --skip-native host_ --config core.json --out ../../port/crates/terraria-gen/src/gen --report SCALE_REPORT.md --host-api HOST_API.md "$@" | tail -1
dotnet bin/Release/net8.0/cs2rs.dll --config game.json --out ../../port/crates/terraria-gen-game/src/gen --report GAME_REPORT.md --host-api HOST_API_GAME.md "$@" | tail -1

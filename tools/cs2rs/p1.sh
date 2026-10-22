#!/bin/sh
# Proof 1 loop: regenerate the whole game, build the harness with a fast profile (no optimisation, no debug info;
# a separate target directory) and run it. Prints the steps reached and where it stopped.
# Usage: sh p1.sh [--nogen] [--survey | --client | --client-whole]
#   --client        tests/client.rs, survey of the CLIENT start-up (DIAGNOSTIC: every statement caught on its own,
#                   then CLIENT_FRAMES ticks of Update / Draw); --client-whole: the same sequence as single calls,
#                   fails on any stop / unimplemented void member / gap
#   --survey  run tests/survey.rs instead (DIAGNOSTIC: every statement of Main.Initialize caught on its own, so one
#             build lists the first stop of each sub-initialiser; proves nothing)
set -e
cd "$(dirname "$0")"
TEST=proof1
NOGEN=0
for a in "$@"; do
  [ "$a" = "--survey" ] && TEST=survey
  [ "$a" = "--client" ] && TEST=client
  [ "$a" = "--client-whole" ] && TEST=client && export CLIENT_WHOLE=1
  [ "$a" = "--nogen" ] && NOGEN=1
done
if [ "$NOGEN" = 0 ]; then
  dotnet build -c Release -nologo -v q 2>&1 | grep -E " error |rror\(s\)" | sed 's/\[.*//' | sort -u
  dotnet bin/Release/net8.0/cs2rs.dll --config game.json --out ../../port/crates/terraria-gen-game/src/gen --report GAME_REPORT.md --host-api HOST_API_GAME.md | tail -1
fi
[ "$TEST" = client ] && python mk_client_survey.py
cd ../../port
export CARGO_PROFILE_DEV_OPT_LEVEL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=target-gen/fast
cargo test -p terraria-gen-game -j 4 --test $TEST --message-format short -- --nocapture > target-gen/p1.txt 2>&1 || true
grep -E "^ok |^FAIL |stopped|threw|: error|^error|test result|^proof|^survey|^client|^====|^STOP|^\[throw|^\[host\] |^\[survey\] |^state |^    |Finished" target-gen/p1.txt | cut -c1-${P1_WIDTH:-700} | tail -${P1_LINES:-25}

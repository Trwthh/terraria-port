#!/bin/sh
# Translate tests/*.cs into port/crates/terraria-gen/src/snip (expected output = the same C# run on .NET).
set -e
cd "$(dirname "$0")"
dotnet build -c Release -nologo -v q 2>&1 | grep -E " error |rror\(s\)" | sed 's/\[.*//' | sort -u
dotnet bin/Release/net8.0/cs2rs.dll --skip-native host_ --config tests/snippets.json --snippets tests --out ../../port/crates/terraria-gen-rt/src/snip | tail -2
grep "^failed\|^skipped\|^unported" ../../port/crates/terraria-gen-rt/src/snip/UNPORTED.txt | cut -c1-260 || true

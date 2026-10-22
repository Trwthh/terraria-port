#!/bin/sh
# Regenerates the three generated Rust files of the sky / background port (see cs_bg.py).
cd "$(dirname "$0")"
CRATES="../../port/crates"
MODE=sim FILTER=Main python cs_bg.py cs_bg_draw.py > "$CRATES/terraria-sim/src/sky_gen.rs" &&
MODE=sim FILTER=WorldGen python cs_bg.py cs_bg_draw.py > "$CRATES/terraria-sim/src/world_gen_bg.rs" &&
MODE=draw python cs_bg.py cs_bg_draw.py > "$CRATES/terraria-app/src/main_backgrounds_gen.rs"

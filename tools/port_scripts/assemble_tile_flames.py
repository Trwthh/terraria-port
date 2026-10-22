#!/usr/bin/env python3
"""Builds `port/crates/terraria-app/src/game_content/drawing/tile_drawing_flames.rs` from its hand-written head and the two
bodies `cs_tile_flames.py` generates from `TileDrawing.cs` (the `switch (num)` of `GetTileFlameData`, lines 1946-2700, and the body
of `DrawSingleTile_Flames`, lines 2709-3513).

    python tools/port_scripts/assemble_tile_flames.py

The output is a first draft; the checked-in `.rs` is the source of truth and carries hand fixes (do not regenerate over it).
"""
import subprocess
import sys
from pathlib import Path

root = Path(__file__).resolve().parents[2]
cs = root / 'decompile/Terraria/Terraria/GameContent/Drawing/TileDrawing.cs'
conv = root / 'tools/port_scripts/cs_tile_flames.py'


def gen(first, last, base):
    return subprocess.run([sys.executable, '-I', str(conv), str(cs), str(first), str(last), str(base)], check=True, capture_output=True, text=True).stdout


head = (root / 'tools/port_scripts/tile_drawing_flames.head.rs').read_text(encoding='utf-8')
out = head.replace('//@FLAME_DATA_SWITCH@', gen(1946, 2700, 8).rstrip()).replace('//@DRAW_FLAMES_BODY@', gen(2709, 3513, 8).rstrip())
target = root / 'port/crates/terraria-app/src/game_content/drawing/tile_drawing_flames.rs'
target.write_text(out, encoding='utf-8')
print('wrote', target)

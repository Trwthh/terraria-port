#!/usr/bin/env python3
"""Builds `port/crates/terraria-app/src/main_animate.rs` from `main_animate.head.rs` and the four bodies `cs_main_anim.py` generates
from `Main.cs` (`DoUpdate_AnimateWalls` 19549-19758, `AnimateTiles` 18734-19519, `AnimateTiles_WeatherVane` 19524-19544,
`AnimateTiles_CritterCages` 13722-16804).

    python tools/port_scripts/assemble_main_animate.py

The output is a first draft; the checked-in `.rs` is the source of truth and carries hand fixes (do not regenerate over it).
"""
import subprocess
import sys
from pathlib import Path

root = Path(__file__).resolve().parents[2]
cs = root / 'decompile/Terraria/Terraria/Main.cs'
conv = root / 'tools/port_scripts/cs_main_anim.py'


def gen(first, last):
    return subprocess.run([sys.executable, '-I', str(conv), str(cs), str(first), str(last), '4'], check=True, capture_output=True, text=True).stdout.rstrip()


text = (root / 'tools/port_scripts/main_animate.head.rs').read_text(encoding='utf-8')
text = text.replace('//@WALLS@', gen(19549, 19758)).replace('//@TILES@', gen(18734, 19519)).replace('//@VANE@', gen(19524, 19544)).replace('//@CAGES@', gen(13722, 16804))
target = root / 'port/crates/terraria-app/src/main_animate.rs'
target.write_text(text, encoding='utf-8')
print('wrote', target)

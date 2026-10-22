#!/usr/bin/env python3
"""Converts the tile / wall / cage animation code of `Main.cs` to Rust: `AnimateTiles` (18732-19520), `AnimateTiles_WeatherVane`
(19522-19545), `DoUpdate_AnimateWalls` (19547-19759) and `AnimateTiles_CritterCages` (13720-16805).

    python tools/port_scripts/cs_main_anim.py <Main.cs> <first line> <last line> [base indent in spaces]

Prints the Rust statements for the statements between the method's braces (give the line range of the body, without the
braces). Reuses the statement parser and emitter of `cs_tile_flames.py`; only the expression vocabulary differs (the `Main`
animation arrays, `rand`, the wind). The output is a first draft; the checked-in `.rs` (`port/crates/terraria-app/src/main_animate.rs`)
is the source of truth and carries the hand fixes.
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import cs_tile_flames as base  # noqa: E402

snake = base.snake

ARRAYS = {
    'tileFrameCounter': ('ctx.state.tile_frame_counter', 'i32'),
    'tileFrame': ('ctx.state.tile_frame', 'i32'),
    'wallFrameCounter': ('ctx.state.wall_frame_counter', 'u8'),
    'wallFrame': ('ctx.state.wall_frame', 'u8'),
}

IDENT_RE = re.compile(r'[A-Za-z_][A-Za-z_0-9]*')


def find_arrays(s):
    """Yield (start, end, rust_array, elem_type, index_exprs) for every `name[...]` of a known array in s."""
    out = []
    for m in re.finditer(r'(?<![\w.])([A-Za-z_][A-Za-z_0-9]*)\[', s):
        name = m.group(1)
        is_cage = re.fullmatch(r'\w+(CageFrame|CageFrameCounter|FrameCounter|CageFrameOrange|CageFrameCounterOrange|CageMode|FishBowlFrame|FishBowlFrameCounter|FishBowlFrameMode|JarFrame|JarFrameCounter|BowlFrame|BowlFrameCounter)', name) or name in (
            'fishBowlFrame', 'fishBowlFrameCounter', 'fishBowlFrameMode', 'lavaFishBowlFrame', 'lavaFishBowlFrameCounter', 'fairyJarFrame', 'fairyJarFrameCounter',
            'dragonflyJarFrame', 'dragonflyJarFrameCounter')
        if name not in ARRAYS and not is_cage:
            continue
        open_i = m.end() - 1
        close_i = base.match_paren(s, open_i)
        out.append((m.start(), close_i + 1, name, base.split_args(s[open_i + 1:close_i])))
    return out


def array_path(name):
    if name in ARRAYS:
        return ARRAYS[name]
    if name.startswith('jellyfishCageMode'):
        return ('ctx.g.main.' + snake(name), 'u8')
    if name.startswith('butterflyCageMode') or name == 'fishBowlFrameMode':
        return ('ctx.state.' + snake(name), 'u8')
    return ('ctx.state.' + snake(name), 'i32')


def rewrite_arrays(s, as_lvalue=False):
    """Replace array element reads: `a[i]` -> `(ctx.state.a[i as usize] as i32)` for u8 arrays, plain for i32."""
    while True:
        found = find_arrays(s)
        if not found:
            return s
        # innermost first: take the last match start so nested indexers are handled from the inside out
        start, end, name, idx = found[-1]
        path, ty = array_path(name)
        idx_rs = ''.join('[(%s) as usize]' % rewrite_expr(i) for i in idx)
        elem = '%s%s' % (path, idx_rs)
        if ty == 'u8' and not as_lvalue:
            elem = '(%s as i32)' % elem
        s = s[:start] + '\0' + elem.replace('[', '\1').replace(']', '\2') + s[end:]
        # keep marker characters out of the next scan
        s = s.replace('\0', '')
        s = s.replace('\1', '\3').replace('\2', '\4')
    return s


def restore(s):
    return s.replace('\3', '[').replace('\4', ']')


def rewrite_expr(s):
    s = s.strip()
    s = re.sub(r'\+\+(\w+(?:\[[^\]]*\])?)', r'{ \1 += 1; \1 }', s) if s.count('++') and not s.endswith('++') else s
    s = rewrite_arrays(s)
    s = restore(s)
    s = base.rewrite_casts(s)
    s = s.replace('WindForVisuals', 'ctx.g.main.wind_for_visuals()')
    s = s.replace('weatherVaneBobframe', 'ctx.state.weather_vane_bobframe')
    s = s.replace('WorldGen.isGeneratingOrLoadingWorld', 'ctx.g.world_gen.generating_world')
    s = s.replace('critterCage', 'ctx.state.critter_cage').replace('cageFrames', 'ctx.state.cage_frames')
    s = re.sub(r'rand\.NextFloat\(\)', 'ctx.g.main.rand.next_float()', s)
    s = re.sub(r'rand\.Next\(([^()]*)\)', lambda m: 'ctx.g.main.rand.next(%s)' % m.group(1) if ',' not in m.group(1) else 'ctx.g.main.rand.next_range(%s)' % m.group(1), s)
    s = s.replace('Math.Floor(', 'math_floor_f32(')
    s = s.replace('Math.Sign(', 'math_sign(')
    s = s.replace('Math.Abs(', 'f32::abs(').replace('MathHelper.Clamp(', 'terraria_core::xna::math_helper::clamp(')
    s = re.sub(r'(?<![\w.])(\d+\.\d+)f\b', r'\1_f32', s)
    s = re.sub(r'(?<![\w.])(\d+)f\b', r'\1.0_f32', s)
    s = re.sub(r'\[\((\d+)\) as usize\]', r'[\1]', s)
    s = snake_idents(s)
    return s


KEEP = {'ctx', 'self', 'as', 'f32', 'i32', 'u8', 'usize', 'math_floor_f32', 'math_sign', 'terraria_core', 'xna', 'math_helper', 'clamp', 'abs'}
IDENT = re.compile(r'(?<![\w.:])([a-z][A-Za-z0-9_]*)((?:\.[A-Za-z_][A-Za-z0-9_]*(?:\(\))?)*)')


def snake_idents(s):
    out = ''
    i = 0
    while i < len(s):
        m = IDENT.match(s, i)
        if m:
            head, tail = m.group(1), m.group(2)
            if head in KEEP or head in ('ctx',):
                out += head + tail
            else:
                out += snake(head) + tail
            i = m.end()
        else:
            out += s[i]
            i += 1
    return out


DECL = re.compile(r'^(int|float|bool|byte|short|double)\s+(\w+)\s*=\s*(.*);$')
RT = {'int': 'i32', 'float': 'f32', 'bool': 'bool', 'byte': 'u8', 'short': 'i16', 'double': 'f64'}


def lvalue_stmt(t):
    """`arr[i]++;`, `arr[i] = e;`, `arr[i] += e;` for the byte arrays need casts."""
    m = re.match(r'^(\w+\[[^=]*?\])(\+\+|--)\s*;$', t)
    if m:
        lv = rewrite_arrays(m.group(1), as_lvalue=True)
        return '%s %s= 1;' % (restore(lv), '+' if m.group(2) == '++' else '-')
    m = re.match(r'^(\w+\[[^=]*?\])\s*(=|\+=|-=|%=|\*=)\s*(.*);$', t)
    if m:
        lv_src = m.group(1)
        name = lv_src.split('[')[0]
        lv = restore(rewrite_arrays(lv_src, as_lvalue=True))
        rhs = rewrite_expr(m.group(3))
        ty = array_path(name)[1]
        if ty == 'u8':
            if m.group(2) == '=':
                return '%s = (%s) as u8;' % (lv, rhs)
            return '%s = ((%s as i32) %s (%s)) as u8;' % (lv, lv, m.group(2)[0], rhs)
        return '%s %s %s;' % (lv, m.group(2), rhs)
    return None


def emit_stmt(o, depth, t):
    if t == 'break;':
        return
    if t == 'continue;':
        o.w(depth, 'continue;')
        return
    if t == 'return;':
        o.w(depth, 'return;')
        return
    r = lvalue_stmt(t)
    if r:
        o.w(depth, r)
        return
    m = DECL.match(t)
    if m:
        ty, name, expr = m.groups()
        o.w(depth, 'let mut %s: %s = %s;' % (snake(name), RT[ty], rewrite_expr(expr)))
        return
    m = re.match(r'^(Animate\w+)\(\);$', t)
    if m:
        o.w(depth, '%s(ctx);' % snake(m.group(1)))
        return
    m = re.match(r'^(\w+)\s*(\+\+|--);$', t)
    if m:
        o.w(depth, '%s %s= 1;' % (snake(m.group(1)), '+' if m.group(2) == '++' else '-'))
        return
    o.w(depth, rewrite_expr(t.rstrip(';')) + ';')


base.rewrite_expr = rewrite_expr
base.emit_stmt = emit_stmt


def main():
    path, first, last = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
    indent = int(sys.argv[4]) if len(sys.argv) > 4 else 4
    lines = open(path, encoding='utf-8').read().split('\n')[first - 1:last]
    nodes, _ = base.parse(lines + ['}'], 0)
    o = base.Out(indent)
    base.emit(o, nodes, 0)
    print('\n'.join(o.lines))


if __name__ == '__main__':
    main()

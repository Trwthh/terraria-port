#!/usr/bin/env python3
"""Mechanical C# -> Rust translation of the body of `WorldGen.TileFrameCosmetic` (WorldGen.cs:82301-85749).

Why a script: the method is 3,450 lines of one regular shape (if / else if / switch (num6) / assignments of
`rectangle.X` and `rectangle.Y`). Typing it by hand would put typos into data (frame cells) that no reader can
spot. The script reads the decompiled source, so every constant comes straight from the original, and keeps
the statement order. The generated file is committed and is the source of truth from then on; the script is kept
so the translation can be audited or redone.

Usage:  python gen_tile_frame_cosmetic.py <WorldGen.cs> <out.rs> [tile_frame_cosmetic.rs.tpl]
Only the statement shapes that occur in this one method are supported; anything else raises.
"""
import re
import sys

SRC_FIRST = 82327   # `if (Main.tileStone[num])`  (first statement inside `default: {` of the inner switch)
SRC_LAST = 85728    # the `return;` that ends that block


def snake(name):
    s = re.sub(r'(?<=[a-z0-9])(?=[A-Z])', '_', name)
    s = re.sub(r'(?<=[A-Z])(?=[A-Z][a-z])', '_', s)
    return s.lower()


LOCAL_RENAME = {
    'upLeft': 'up_left', 'upRight': 'up_right', 'downLeft': 'down_left', 'downRight': 'down_right',
    'tileMergeCulling': 'tile_merge_culling', 'mergesWithDirtInASpecialWay': 'merges_with_dirt_in_a_special_way',
    'frameX': 'frame_x', 'frameY': 'frame_y', 'resetFrame': 'reset_frame',
}
TILE_VARS = {'tile': 'tile', 'tile2': 'tile2', 'tile3': 'tile3', 'tile4': 'tile4', 'tile5': 'tile5',
             'tile6': 'tile6', 'tile7': 'tile7', 'tile8': 'tile8', 'tileCache': 'tile_cache'}
TILE_METHODS = {
    'slope': ('slope', None), 'active': ('active', None), 'halfBrick': ('half_brick', None),
    'topSlope': ('top_slope', None), 'leftSlope': ('left_slope', None), 'rightSlope': ('right_slope', None),
    'bottomSlope': ('bottom_slope', None), 'blockType': ('block_type', None),
    'frameNumber': ('frame_number', 'i32'),
}
WORLDGEN_STATICS = {'mergeUp': 'g.world_gen.merge_up', 'mergeDown': 'g.world_gen.merge_down',
                    'mergeLeft': 'g.world_gen.merge_left', 'mergeRight': 'g.world_gen.merge_right',
                    'tileReframeCount': 'g.world_gen.tile_reframe_count'}


def split_top(s, sep):
    """Split on `sep` at bracket depth 0."""
    out, depth, cur = [], 0, ''
    for ch in s:
        if ch in '([{':
            depth += 1
        elif ch in ')]}':
            depth -= 1
        if ch == sep and depth == 0:
            out.append(cur)
            cur = ''
        else:
            cur += ch
    out.append(cur)
    return out


def strip_outer_parens(s):
    s = s.strip()
    while s.startswith('(') and s.endswith(')'):
        depth = 0
        ok = True
        for k, ch in enumerate(s):
            if ch == '(':
                depth += 1
            elif ch == ')':
                depth -= 1
                if depth == 0 and k != len(s) - 1:
                    ok = False
                    break
        if not ok:
            break
        s = s[1:-1].strip()
    return s


def table_path(expr):
    """`TileID.Sets.Conversion.Grass` -> `tile_id::sets::conversion::GRASS`; `Main.tileBrick` -> `g.main.tile_brick`."""
    if expr.startswith('TileID.Sets.'):
        parts = expr[len('TileID.Sets.'):].split('.')
        mods = [snake(p) for p in parts[:-1]]
        return 'tile_id::sets::' + ''.join(m + '::' for m in mods) + snake(parts[-1]).upper()
    if expr.startswith('Main.'):
        return 'g.main.' + snake(expr[len('Main.'):])
    raise ValueError(expr)


def is_table(expr):
    e = expr.strip()
    return bool(re.match(r'^(TileID\.Sets\.[\w\.]+|Main\.tile\w+(\[\w+\])?|mergesWithDirtInASpecialWay)$', e))


def table_ref(expr):
    e = expr.strip()
    if e == 'mergesWithDirtInASpecialWay':
        return '&merges_with_dirt_in_a_special_way'
    m = re.match(r'^Main\.tileMerge\[(\w+)\]$', e)
    if m:
        return '&g.main.tile_merge[%s as usize]' % ex(m.group(1))
    return '&' + table_path(e)


def ex(s):
    """Translate one C# expression."""
    s = s.strip()
    # null checks: tiles are never null in the port (PORTING.md)
    s = re.sub(r'\btile\w*\s*!=\s*null\s*&&\s*', '', s)
    s = re.sub(r'\b(tile\d?)\?\.frameY \?\? \(-1\)', r'\1.frameY', s)
    if 'null' in s or '?.' in s:
        raise ValueError('unhandled null/?. in: ' + s)
    # ternary at top level
    inner = strip_outer_parens(s)
    q = split_top(inner, '?')
    if len(q) == 2:
        c = split_top(q[1], ':')
        assert len(c) == 2, s
        return 'if %s { %s } else { %s }' % (strip_outer_parens(ex(q[0])), strip_outer_parens(ex(c[0])), strip_outer_parens(ex(c[1])))
    # (cond).ToInt()
    s = re.sub(r'\b(flag\d+)\.ToInt\(\)', r'(\1 as i32)', s)
    while '.ToInt()' in s:
        k = s.index('.ToInt()')
        assert s[k - 1] == ')'
        depth, p = 0, k - 1
        while True:
            if s[p] == ')':
                depth += 1
            elif s[p] == '(':
                depth -= 1
                if depth == 0:
                    break
            p -= 1
        s = s[:p] + '((' + ex(s[p + 1:k - 1]) + ') as i32)' + s[k + len('.ToInt()'):]
    s = re.sub(r'\bIsRope\((\w+), ([^()]+)\)', lambda m: 'is_rope(g, %s, %s, 5)' % (m.group(1), m.group(2)), s)
    s = re.sub(r'\bgenRand\.Next\((\d+), (\d+)\)', lambda m: 'g.main.rand.next_range(%s, %s)' % (m.group(1), m.group(2)), s)
    # casts
    s = re.sub(r'\(double\)(\w+)', r'(\1 as f64)', s)

    # tile accessors
    def tile_member(m):
        var, member = m.group(1), m.group(2)
        t = TILE_VARS[var] + '!()'
        if member == 'type':
            return '(%s.type_ as i32)' % t
        if member in ('frameX', 'frameY'):
            return '(%s.%s as i32)' % (t, snake(member))
        if member in TILE_METHODS:
            name, cast = TILE_METHODS[member]
            if cast:
                return '(%s.%s() as %s)' % (t, name, cast)
            return '%s.%s()' % (t, name)
        raise ValueError('tile member ' + member)
    s = re.sub(r'\b(tileCache|tile\d?)\.(\w+)(?:\(\))?', tile_member, s)
    # statics
    s = s.replace('TileID.Count', '(terraria_data::ids::tile_id::COUNT)')
    s = re.sub(r'\bMain\.rockLayer\b', 'g.main.rock_layer', s)
    for k, v in WORLDGEN_STATICS.items():
        s = re.sub(r'(?<![\w\.])%s\b' % k, v, s)

    # tables with indexers: NAME[expr] -> NAME[(expr) as usize]
    def conv_tables(s):
        out, k = '', 0
        pat = re.compile(r'(TileID\.Sets\.[\w\.]+|Main\.tile\w+|mergesWithDirtInASpecialWay)\[')
        while True:
            m = pat.search(s, k)
            if not m:
                out += s[k:]
                return out
            out += s[k:m.start()]
            name = m.group(1)
            depth, p = 1, m.end()
            while depth:
                if s[p] == '[':
                    depth += 1
                elif s[p] == ']':
                    depth -= 1
                p += 1
            idx = conv_tables(s[m.end():p - 1]).strip()
            if re.fullmatch(r'\w+', idx):
                idx_s = '%s as usize' % idx
            else:
                idx_s = '(%s) as usize' % strip_outer_parens(idx)
            ref = 'merges_with_dirt_in_a_special_way' if name == 'mergesWithDirtInASpecialWay' else table_path(name)
            out += '%s[%s]' % (ref, idx_s)
            k = p
    s = conv_tables(s)
    for k, v in LOCAL_RENAME.items():
        s = re.sub(r'(?<![\w\.])%s\b' % k, v, s)
    s = re.sub(r'\brectangle\.X\b', 'rectangle.x', s)
    s = re.sub(r'\brectangle\.Y\b', 'rectangle.y', s)
    return s


# ------------------------------------------------------------------ statements
class Node:
    pass


def parse_block(lines, i):
    """Parse statements from lines[i] until a closing `}` / case label; returns (stmts, i)."""
    stmts = []
    while True:
        if i >= len(lines):
            for n in stmts:
                n.blk_end = lines[-1][0] + 1
            return stmts, i
        ln, text = lines[i]
        if text == '}' or text.startswith('case ') or text == 'default:':
            for n in stmts:
                n.blk_end = ln
            return stmts, i
        if text == '{':
            raise ValueError('bare block at %d' % ln)
        m = re.match(r'^if \((.*)\)$', text)
        if m:
            chain = []
            cond = m.group(1)
            ln0 = ln
            i += 1
            assert lines[i][1] == '{'
            body, i = parse_block(lines, i + 1)
            assert lines[i][1] == '}'
            i += 1
            chain.append((ln0, cond, body))
            els = None
            while True:
                t = lines[i][1]
                mm = re.match(r'^else if \((.*)\)$', t)
                if mm:
                    ln1 = lines[i][0]
                    i += 1
                    assert lines[i][1] == '{'
                    b, i = parse_block(lines, i + 1)
                    assert lines[i][1] == '}'
                    i += 1
                    chain.append((ln1, mm.group(1), b))
                    continue
                if t == 'else':
                    i += 1
                    assert lines[i][1] == '{'
                    els, i = parse_block(lines, i + 1)
                    assert lines[i][1] == '}'
                    i += 1
                break
            n = Node()
            n.kind = 'if'
            n.chain = chain
            n.els = els
            stmts.append(n)
            continue
        m = re.match(r'^switch \((.*)\)$', text)
        if m:
            n = Node()
            n.kind = 'switch'
            n.ln = ln
            n.expr = m.group(1)
            n.arms = []
            i += 1
            assert lines[i][1] == '{'
            i += 1
            while lines[i][1] != '}':
                labels = []
                while True:
                    t = lines[i][1]
                    mc = re.match(r'^case (-?\d+):$', t)
                    if mc:
                        labels.append(int(mc.group(1)))
                        i += 1
                    elif t == 'default:':
                        labels.append(None)
                        i += 1
                    else:
                        break
                body, i = parse_block(lines, i)
                if body and body[-1].kind == 'simple' and body[-1].text == 'break;':
                    body.pop()
                elif body and body[-1].kind == 'simple' and body[-1].text == 'return;':
                    pass
                else:
                    raise ValueError('case without break at line %d' % lines[i][0])
                n.arms.append((labels, body))
            i += 1
            stmts.append(n)
            continue
        # multi-line array literal: `num6 = (new int[4, 3] { {..}, .. })[num7, num8] - 1;`
        if text.startswith('num6 = (new int[4, 3]'):
            rows = []
            while not lines[i][1].startswith('})[num7, num8]'):
                mr = re.match(r'^\{ (\d+), (\d+), (\d+) \},?$', lines[i][1])
                if mr:
                    rows.append('[%s, %s, %s]' % mr.groups())
                i += 1
            i += 1
            n = Node()
            n.kind = 'raw'
            n.text = 'num6 = [%s][num7 as usize][num8 as usize] - 1;' % ', '.join(rows)
            n.ln = ln
            stmts.append(n)
            continue
        n = Node()
        n.kind = 'simple'
        n.text = text
        n.ln = ln
        stmts.append(n)
        i += 1


def mutated(name, lines_after):
    pat = re.compile(r'((?<![\w\.])%s\s*(=(?!=)|\+=|-=|\+\+|--)|\bref %s\b)' % (name, name))
    return any(pat.search(t) for _, t in lines_after)


OUT = []


def emit(depth, s):
    OUT.append('    ' * depth + s)


def call_args(s):
    m = re.match(r'^(\w+(?:\.\w+)*)\((.*)\);$', s)
    return m.group(1), [a.strip() for a in split_top(m.group(2), ',')]


def refs_of(args):
    refs = [a for a in args if a.startswith('ref ')]
    pre = [a for a in args if not a.startswith('ref ')]
    return pre, ['&mut ' + LOCAL_RENAME.get(a[4:], a[4:]) for a in refs]


def tr_simple(text, ln, decl_mut):
    t = text
    # neighbour tiles: Tile is a reference type in C#, so later reads see later writes; a macro re-reads the live tile
    m = re.match(r'^Tile (tile\d?) = Main\.tile\[(.*), (.*)\];$', t)
    if m:
        return ['macro_rules! %s { () => { *g.main.tile.get(%s, %s) }; }' % (m.group(1), m.group(2), m.group(3))]
    if t == 'Rectangle rectangle = new Rectangle(-1, -1, 0, 0);':
        return ['let mut rectangle = Rectangle { x: -1, y: -1, width: 0, height: 0 };']
    m = re.match(r'^TileMergeCullCache tileMergeCulling = GetTileMergeCulling\((.*)\);$', t)
    if m:
        args = [a.strip() for a in m.group(1).split(',')]
        conv = [TILE_VARS[a] + '!()' for a in args]
        return ['let tile_merge_culling = get_tile_merge_culling(g, %s);' % ', '.join(conv)]
    m = re.match(r'^bool\[\] mergesWithDirtInASpecialWay = (.*);$', t)
    if m:
        return ['let merges_with_dirt_in_a_special_way = &%s;' % table_path(m.group(1))]
    m = re.match(r'^(int|bool|byte|short|ushort) (\w+) = (.*);$', t)
    if m:
        ty = {'int': 'i32', 'bool': 'bool', 'byte': 'u8', 'short': 'i16', 'ushort': 'u16'}[m.group(1)]
        name = LOCAL_RENAME.get(m.group(2), m.group(2))
        mu = 'mut ' if decl_mut else ''
        return ['let %s%s: %s = %s;' % (mu, name, ty, strip_outer_parens(ex(strip_outer_parens(m.group(3)))))]
    # writes to the tile
    m = re.match(r'^tileCache\.frameNumber\(\(byte\)(\w+)\);$', t)
    if m:
        return ['g.main.tile.get_mut(i, j).set_frame_number(%s as u8);' % m.group(1)]
    m = re.match(r'^tileCache\.frame([XY]) = \(short\)rectangle\.([XY]);$', t)
    if m:
        return ['g.main.tile.get_mut(i, j).frame_%s = rectangle.%s as i16;' % (m.group(1).lower(), m.group(2).lower())]
    if t == 'return;':
        return ['return;']
    if t.startswith('goto '):
        raise ValueError(t)
    # calls
    m = re.match(r'^tileMergeCulling\.Cull\((.*)\);$', t)
    if m:
        _, refs = refs_of([a.strip() for a in m.group(1).split(',')])
        return ['tile_merge_culling.cull(%s);' % ', '.join(refs)]
    m = re.match(r'^TileFrame\((.*), (.*)\);$', t)
    if m:
        return ['tile_frame(g, %s, %s, false, false);' % (ex(m.group(1)), ex(m.group(2)))]
    m = re.match(r'^TileFrameCosmetic\((.*), (.*), (tile\d?)\);$', t)
    if m:
        return ['tile_frame_cosmetic(g, %s, %s, false); // the Tile argument is Main.tile[x, y] of the same coordinates' %
                (ex(m.group(1)), ex(m.group(2)))]
    if re.match(r'^(TileMergeAttempt|TileMergeAttemptFrametest|TileMergeAttemptWeird)\(.*\);$', t):
        name, args = call_args(t)
        pre, refs = refs_of(args)
        assert len(refs) in (4, 8), t
        suffix = '_4' if len(refs) == 4 else ''
        if name == 'TileMergeAttemptWeird':
            assert len(pre) == 3
            return ['tile_merge_attempt_weird(%s, %s, %s, %s);' % (ex(pre[0]), ex(pre[1]), table_ref(pre[2]), ', '.join(refs))]
        if name == 'TileMergeAttemptFrametest':
            assert len(pre) == 4 and len(refs) == 8
            tbl = is_table(pre[3])
            fn = 'tile_merge_attempt_frametest_tbl' if tbl else 'tile_merge_attempt_frametest'
            last = table_ref(pre[3]) if tbl else ex(pre[3])
            return ['%s(g, %s, %s, %s, %s, %s);' % (fn, ex(pre[0]), ex(pre[1]), ex(pre[2]), last, ', '.join(refs))]
        if len(pre) == 2:
            if is_table(pre[1]):
                return ['tile_merge_attempt_tbl%s(%s, %s, %s);' % (suffix, ex(pre[0]), table_ref(pre[1]), ', '.join(refs))]
            return ['tile_merge_attempt%s(%s, %s, %s);' % (suffix, ex(pre[0]), ex(pre[1]), ', '.join(refs))]
        assert len(pre) == 3 and len(refs) == 8, t
        if is_table(pre[2]):
            return ['tile_merge_attempt_tbl_exclude_tbl(%s, %s, %s, %s);' % (ex(pre[0]), table_ref(pre[1]), table_ref(pre[2]), ', '.join(refs))]
        return ['tile_merge_attempt_tbl_exclude_type(%s, %s, %s, %s);' % (ex(pre[0]), table_ref(pre[1]), ex(pre[2]), ', '.join(refs))]
    # assignments
    m = re.match(r'^(\w+(?:\.[XY])?) (=|\+=|-=) (.*);$', t)
    if m:
        lhs, op, rhs = m.groups()
        lhs_r = ex(lhs) if '.' in lhs else (WORLDGEN_STATICS.get(lhs) or LOCAL_RENAME.get(lhs, lhs))
        return ['%s %s %s;' % (lhs_r, op, strip_outer_parens(ex(strip_outer_parens(rhs))))]
    m = re.match(r'^(\w+)(\+\+|--);$', t)
    if m:
        v = WORLDGEN_STATICS.get(m.group(1)) or m.group(1)
        return ['%s %s= 1;' % (v, m.group(2)[0])]
    raise ValueError('unhandled statement at %d: %s' % (ln, t))


def gen_block(stmts, depth, lines):
    for n in stmts:
        if n.kind == 'raw':
            emit(depth, n.text)
        elif n.kind == 'simple':
            mu = False
            m = re.match(r'^(?:int|bool|byte|short|ushort) (\w+) = ', n.text)
            if m:
                after = [(l, t) for l, t in lines if n.ln < l < n.blk_end]
                mu = mutated(m.group(1), after)
            for o in tr_simple(n.text, n.ln, mu):
                emit(depth, o)
        elif n.kind == 'if':
            for idx, (ln, cond, body) in enumerate(n.chain):
                head = ('if ' if idx == 0 else '} else if ') + strip_outer_parens(ex(cond)) + ' {'
                emit(depth, head + '  // WorldGen.cs:%d' % ln)
                gen_block(body, depth + 1, lines)
            if n.els is not None:
                emit(depth, '} else {')
                gen_block(n.els, depth + 1, lines)
            emit(depth, '}')
        elif n.kind == 'switch':
            # Rust needs the wildcard arm last; C# `default` may be anywhere and only matches when no case does
            arms = sorted(n.arms, key=lambda a: (None in a[0]))
            has_default = any(None in a[0] for a in arms)
            emit(depth, 'match %s {  // WorldGen.cs:%d' % (ex(n.expr), n.ln))
            for labels, body in arms:
                if None in labels:
                    assert all(x is None for x in labels), 'case labels sharing default'
                    pat = '_'
                else:
                    pat = ' | '.join(str(x) for x in labels)
                emit(depth + 1, pat + ' => {')
                gen_block(body, depth + 2, lines)
                emit(depth + 1, '}')
            if not has_default:
                emit(depth + 1, '_ => {}')
            emit(depth, '}')
        else:
            raise ValueError(n.kind)


def main():
    src, out = sys.argv[1], sys.argv[2]
    with open(src, encoding='utf-8') as f:
        raw = f.read().split('\n')
    lines = [(n, raw[n - 1].strip()) for n in range(SRC_FIRST, SRC_LAST + 1)]
    lines = [(n, t) for n, t in lines if t != '']
    stmts, i = parse_block(lines, 0)
    assert i == len(lines), (i, len(lines))
    emit(0, '// @generated by tools/refdump/gen_tile_frame_cosmetic.py from decompile/Terraria/Terraria/WorldGen.cs:%d-%d.' % (SRC_FIRST, SRC_LAST))
    emit(0, '// This is the body of the `default:` block of the inner switch of `WorldGen.TileFrameCosmetic`; the')
    emit(0, '// surrounding switches are written by hand in tile_frame_cosmetic.rs. Each `// WorldGen.cs:N` comment gives the')
    emit(0, '// source line of an `if` / `else if` / `switch`.')
    gen_block(stmts, 0, lines)
    if len(sys.argv) > 3:
        with open(sys.argv[3], encoding='utf-8') as f:
            tpl = f.read().split('\n')
        k = next(n for n, t in enumerate(tpl) if t.strip() == '//@@BODY@@')
        indent = tpl[k][:len(tpl[k]) - len(tpl[k].lstrip())]
        OUT[:] = tpl[:k] + [(indent + t) if t else t for t in OUT] + tpl[k + 1:]
        text = '\n'.join(OUT)
    else:
        text = '\n'.join(OUT) + '\n'
    with open(out, 'w', encoding='utf-8', newline='\n') as f:
        f.write(text)
    print('wrote', out, len(OUT), 'lines')


if __name__ == '__main__':
    main()

#!/usr/bin/env python3
"""Converts the flame code of `TileDrawing.cs` to Rust: `GetTileFlameData` (the `switch (num)` over the flame texture index,
lines 2001-2704) and `DrawSingleTile_Flames` (lines 2707-3514).

    python tools/port_scripts/cs_tile_flames.py <TileDrawing.cs> <first line> <last line> [base indent in spaces]

Prints the Rust statements for that line range (the statements of the method body; the `fn` header and any lines the
converter cannot express are written by hand around it). The generated text is the first draft: the checked-in `.rs`
(`port/crates/terraria-app/src/game_content/drawing/tile_drawing_flames.rs`) is the source of truth and was fixed by hand
where this script's output did not compile or read differently from the C#.

It understands only the dialect of those two methods: one statement per line, braces on their own lines, `if / else if /
else / for / switch / case / default / break / return`, `Main.tileBatch.Draw(...)`, `new Vector2/Rectangle/Color(...)`,
`(float)/(int)/(ulong)` casts, `Utils.RandomInt(ref seed, a, b)`, `TextureAssets.X[n].Value`.
"""
import re
import sys

KNOWN_FLOAT_VARS = set()
TEX_VARS = set()
TEX_COUNTER = [0]

SNAKE1 = re.compile(r'([a-z0-9])([A-Z])')
SNAKE2 = re.compile(r'([A-Z]+)([A-Z][a-z])')


def snake(name):
    return SNAKE1.sub(r'\1_\2', SNAKE2.sub(r'\1_\2', name)).lower()


def split_args(s):
    """Split on top-level commas."""
    out, depth, cur = [], 0, ''
    for ch in s:
        if ch in '([{':
            depth += 1
        elif ch in ')]}':
            depth -= 1
        if ch == ',' and depth == 0:
            out.append(cur.strip())
            cur = ''
        else:
            cur += ch
    if cur.strip():
        out.append(cur.strip())
    return out


def match_paren(s, i):
    """s[i] is an opening bracket; return the index of its partner."""
    pairs = {'(': ')', '[': ']', '{': '}'}
    depth = 0
    for j in range(i, len(s)):
        if s[j] in '([{':
            depth += 1
        elif s[j] in ')]}':
            depth -= 1
            if depth == 0:
                return j
    raise ValueError('unbalanced: ' + s)


PRIMARY = re.compile(r'-?[A-Za-z_][A-Za-z_0-9]*')


def read_primary(s, i):
    """Read a unary operand starting at i: [-](group | identifier chain with calls / indexers). Returns end index."""
    j = i
    assert j < len(s), 'cast at end of: ' + s
    if s[j] == '-':
        j += 1
    cm = re.compile(r'\((float|int|ulong|long|uint|byte|double)\)\s*').match(s, j)
    if cm:
        return read_primary(s, cm.end())
    if s[j] == '(':
        j = match_paren(s, j) + 1
    else:
        m = re.compile(r'[A-Za-z_][A-Za-z_0-9]*').match(s, j)
        if not m:
            m = re.compile(r'[0-9][0-9.]*f?').match(s, j)
        j = m.end()
    while j < len(s):
        if s[j] == '.' and re.compile(r'[A-Za-z_]').match(s, j + 1):
            j = re.compile(r'[A-Za-z_][A-Za-z_0-9]*').match(s, j + 1).end()
        elif s[j] in '([':
            j = match_paren(s, j) + 1
        else:
            break
    return j


CASTS = {'float': 'f32', 'int': 'i32', 'ulong': 'u64', 'long': 'i64', 'uint': 'u32', 'byte': 'u8', 'double': 'f64'}


def rewrite_casts(s):
    out = ''
    i = 0
    while i < len(s):
        m = re.compile(r'\((float|int|ulong|long|uint|byte|double)\)\s*').match(s, i)
        if m and (i == 0 or not (s[i - 1].isalnum() or s[i - 1] == '_')):
            j = read_primary(s, m.end())
            operand = s[m.end():j]
            if operand == 'Math.PI':
                out += 'std::f32::consts::PI'
            else:
                out += '(' + rewrite_casts(operand) + ' as ' + CASTS[m.group(1)] + ')'
            i = j
        else:
            out += s[i]
            i += 1
    return out


def is_float_expr(e):
    return bool(re.search(r'f32|f64|\d\.\d| as f32|\bflame_range_mult|\bwind', e)) or any(re.search(r'(?<![\w.])%s\b' % re.escape(v), e) for v in KNOWN_FLOAT_VARS)


def to_f32(e):
    return e if is_float_expr(e) and ' as i32' not in e.split('f32')[0][:0] and not re.fullmatch(r'\d+', e) else '(%s) as f32' % e


def to_i32(e):
    return e if re.fullmatch(r'-?\d+', e) else '(%s) as i32' % e


TEXTURES = {
    'Flames': 'flame', 'GlowMask': 'glow_mask', 'JellyfishBowl': 'jellyfish_bowl',
}
TEXTURES0 = {
    'FireflyJar': 'firefly_jar', 'LightningbugJar': 'lightningbug_jar', 'GlowSnail': 'glow_snail', 'XmasLight': 'xmas_light',
    'Timer': 'timer', 'SunAltar': 'sun_altar',
}


def rewrite_expr(s):
    s = s.strip()
    # constants and specials first (before the generic renaming)
    s = re.sub(r'\(ulong\)\(\(\(long\)(\w+) << 32\) \| \(uint\)(\w+)\)', r'(((\1 as i64) << 32) | (\2 as u32 as i64)) as u64', s)
    s = s.replace('(float)Math.PI', 'std::f32::consts::PI')
    # Vector2.UnitX.RotatedBy(arg)
    while 'Vector2.UnitX.RotatedBy(' in s:
        k = s.index('Vector2.UnitX.RotatedBy(')
        open_i = k + len('Vector2.UnitX.RotatedBy')
        close_i = match_paren(s, open_i)
        s = s[:k] + 'vector2_rotated_by(Vector2::UNIT_X, ' + rewrite_expr(s[open_i + 1:close_i]) + ')' + s[close_i + 1:]
    s = rewrite_casts(s)
    # TextureAssets
    def tex_idx(m):
        return 'ctx.%s(%s)' % (TEXTURES[m.group(1)], rewrite_expr(m.group(2)))
    s = re.sub(r'TextureAssets\.(Flames|GlowMask|JellyfishBowl)\[([^\]]*)\]\.Value', tex_idx, s)
    s = re.sub(r'TextureAssets\.(\w+)\.Value', lambda m: 'ctx.%s()' % TEXTURES0[m.group(1)], s)
    # Main.tile[a, b]
    def tile_idx(m):
        a, b = split_args(m.group(1))
        return 'ctx.g.main.tile.get(%s, %s)' % (rewrite_expr(a), rewrite_expr(b))
    s = re.sub(r'Main\.tile\[([^\]]*)\]', tile_idx, s)
    # Main statics
    s = re.sub(r'Main\.tileFlame\[([^\]]*)\]', lambda m: 'ctx.g.main.tile_flame[(%s) as usize]' % rewrite_expr(m.group(1)), s)
    s = s.replace('Main.DiscoR', 'ctx.g.main.disco_r').replace('Main.DiscoG', 'ctx.g.main.disco_g').replace('Main.DiscoB', 'ctx.g.main.disco_b')
    s = s.replace('Main.mouseTextColor', 'ctx.g.main.mouse_text_color')
    s = s.replace('Main.TileFrameSeed', 'ctx.state.tile_frame_seed')
    s = s.replace('Main.sundialCooldown', 'ctx.g.main.sundial_cooldown').replace('Main.moondialCooldown', 'ctx.g.main.moondial_cooldown')
    s = s.replace('Main.moonPhase', 'main_moon_phase(ctx.g)')
    s = s.replace('Main.GraveyardVisualIntensity', 'unported!("Main.GraveyardVisualIntensity", 0.0_f32)')
    s = s.replace('Color.White', 'Color::WHITE')
    s = s.replace('_zero', 'ZERO')
    # Utils.RandomInt(ref a, b, c)
    s = re.sub(r'Utils\.RandomInt\(ref ([\w.]+),', lambda m: 'utils_random_int(&mut %s,' % snake_path(m.group(1)), s)
    # value3.Frame()
    s = re.sub(r'(\w+)\.Frame\(\)', r'texture_frame_full(\1.as_ref())', s)
    # furniture flame colour helpers and GetTileFlameData
    s = re.sub(r'\bGet(\w+)FurnitureFlameColor\(\)', lambda m: 'self.get_%s_furniture_flame_color(ctx)' % snake(m.group(1)), s)
    s = re.sub(r'\bGetTileFlameData\(', 'self.get_tile_flame_data(ctx, ', s)
    # constructors
    s = rewrite_ctors(s)
    # literals
    s = re.sub(r'(?<![\w.])(\d+\.\d+)f\b', r'\1_f32', s)
    s = re.sub(r'(?<![\w.])(\d+)f\b', r'\1.0_f32', s)
    s = re.sub(r'\b0L\b', '0', s)
    # identifiers: camelCase -> snake_case (skip the already-converted `ctx.` paths and Rust paths)
    s = snake_idents(s)
    # indexers on arrays that remain
    return s


def snake_path(p):
    return '.'.join(snake(x) for x in p.split('.'))


IDENT = re.compile(r'(?<![\w.:])([a-z][A-Za-z0-9_]*)((?:\.[A-Za-z_][A-Za-z0-9_]*)*)')
KEEP = {'ctx', 'self', 'as', 'std', 'let', 'mut', 'true', 'false', 'if', 'else', 'f32', 'i32', 'u8', 'u64', 'i64', 'u32', 'f64', 'usize',
        'vector2_rotated_by', 'utils_random_int', 'texture_frame_full', 'main_moon_phase', 'unported'}


def snake_idents(s):
    out = ''
    i = 0
    in_str = False
    while i < len(s):
        if s[i] == '"':
            j = s.index('"', i + 1)
            out += s[i:j + 1]
            i = j + 1
            continue
        m = IDENT.match(s, i)
        if m:
            head, tail = m.group(1), m.group(2)
            if head in ('ctx', 'self'):
                out += head + tail  # already rust
            elif head in KEEP:
                out += head + tail
            else:
                out += snake(head) + ''.join('.' + snake(x) for x in tail.split('.')[1:])
            i = m.end()
        else:
            out += s[i]
            i += 1
    out = re.sub(r'\)\.([a-z]\w*[A-Z]\w*)', lambda m: ').' + snake(m.group(1)), out)
    # `draw_data.tile_frame_x` etc. are 16-bit in TileDrawInfo: C# promotes them to int
    out = re.sub(r'\bdraw_data\.(tile_frame_x|tile_frame_y|type_cache)\b(?!\s*=[^=])', r'(draw_data.\1 as i32)', out)
    out = re.sub(r'\bdraw_data\.tile_cache\.(frame_x|frame_y|type)\b', lambda m: '(draw_data.tile_cache.%s as i32)' % ('type_' if m.group(1) == 'type' else m.group(1)), out)
    out = re.sub(r'\.get\(([^()]*)\)\.(frame_x|frame_y)\b', r'.get(\1).\2 as i32', out)
    out = re.sub(r'\bctx\.g\.main\.mouse_text_color\b', 'ctx.g.main.mouse_text_color as i32', out)
    return out


def rewrite_ctors(s):
    for name in ('Vector2', 'Rectangle', 'Color'):
        while True:
            m = re.search(r'\bnew %s\(' % name, s)
            if not m:
                break
            open_i = m.end() - 1
            close_i = match_paren(s, open_i)
            args = [rewrite_expr(a) for a in split_args(s[open_i + 1:close_i])]
            if name == 'Vector2':
                new = 'Vector2::new(%s)' % ', '.join(to_f32(a) for a in args)
            elif name == 'Rectangle':
                new = 'Rectangle::new(%s)' % ', '.join(to_i32(a) for a in args)
            else:
                if any('f32' in a for a in args):
                    new = 'Color::from_floats(%s)' % ', '.join(to_f32(a) for a in args)
                else:
                    new = 'Color::from_ints(%s)' % ', '.join(to_i32(a) for a in args)
            s = s[:m.start()] + '\0' + new.replace('(', '\1').replace(')', '\2') + s[close_i + 1:]
        s = s.replace('\0', '').replace('\1', '(').replace('\2', ')')
    return s


# ------------------------------------------------------------------------------------------------ statements

class Out:
    def __init__(self, base):
        self.lines = []
        self.base = base

    def w(self, depth, text):
        self.lines.append(' ' * (self.base + 4 * depth) + text)


def hoist_textures(stmt):
    """`ctx.flame(n)` etc. inside a call that also borrows `ctx` must be evaluated first."""
    pre = []
    new = ''
    i = 0
    pat = re.compile(r'ctx\.(?:flame|glow_mask|jellyfish_bowl|firefly_jar|lightningbug_jar|glow_snail|xmas_light|timer|sun_altar)\(')
    while i < len(stmt):
        m = pat.match(stmt, i)
        if m:
            close_i = match_paren(stmt, m.end() - 1)
            TEX_COUNTER[0] += 1
            name = 'tex%d' % TEX_COUNTER[0]
            pre.append('let %s = %s;' % (name, stmt[i:close_i + 1]))
            new += name + '.as_ref()'
            i = close_i + 1
        else:
            new += stmt[i]
            i += 1
    return pre, new


def parse(lines, i, stop_at_close=True):
    """Return (nodes, next index). Each node is a tuple."""
    nodes = []
    while i < len(lines):
        t = lines[i].strip()
        if not t:
            i += 1
            continue
        if t == '}':
            return nodes, i + 1
        if t == '{':
            sub, i = parse(lines, i + 1)
            nodes.append(('block', sub))
            continue
        m = re.match(r'(else if|if|for|while|switch) \((.*)\)$', t)
        if m:
            kind, head = m.groups()
            assert lines[i + 1].strip() == '{', t
            body, i = parse(lines, i + 2)
            if kind == 'switch':
                nodes.append(('switch', head, body))
            else:
                nodes.append((kind, head, body))
            continue
        if t == 'else':
            assert lines[i + 1].strip() == '{'
            body, i = parse(lines, i + 2)
            nodes.append(('else', body))
            continue
        if re.match(r'(case .*|default):$', t):
            nodes.append(('label', t[:-1]))
            i += 1
            continue
        nodes.append(('stmt', t))
        i += 1
    return nodes, i


DECL = re.compile(r'^(int|float|ulong|long|bool|Color|Vector2|Rectangle|Texture2D|TileFlameData|byte|uint|double)\s+(\w+)\s*=\s*(.*);$')
RUST_TYPES = {'int': 'i32', 'float': 'f32', 'ulong': 'u64', 'long': 'i64', 'bool': 'bool', 'byte': 'u8', 'uint': 'u32', 'double': 'f64'}


def emit_stmt(o, depth, t):
    if t == 'break;':
        return
    if t == 'return;':
        o.w(depth, 'return;')
        return
    m = DECL.match(t)
    if m:
        ty, name, expr = m.groups()
        if ty == 'float':
            KNOWN_FLOAT_VARS.add(snake(name))
        if ty == 'Texture2D':
            TEX_VARS.add(snake(name))
        e = rewrite_expr(expr)
        pre, e = hoist_textures(e) if ty != 'Texture2D' else ([], e)
        for p in pre:
            o.w(depth, p)
        if ty == 'int' and 'as f32' in e and ' as i32' not in e:
            pass
        o.w(depth, 'let mut %s%s = %s;' % (snake(name), ': ' + RUST_TYPES[ty] if ty in RUST_TYPES else '', e))
        return
    m = re.match(r'^Main\.tileBatch\.Draw\((.*)\);$', t)
    if m:
        args = split_args(m.group(1))
        assert len(args) == 7, t
        tex = rewrite_expr(args[0])
        pre, tex = hoist_textures(tex)
        for p in pre:
            o.w(depth, p)
        if re.fullmatch(r'[a-z_0-9]+', tex) and tex in TEX_VARS:
            tex += '.as_ref()'
        elif re.fullmatch(r'[a-z_0-9.]*flame_texture', tex):
            tex += '.as_ref()'
        rest = [rewrite_expr(a) for a in args[1:]]
        rest[2] = '(%s).into()' % rest[2]
        o.w(depth, 'ctx.tile_batch.draw(%s, %s);' % (tex, ', '.join(rest)))
        return
    # assignment / compound assignment / other
    pre, e = hoist_textures(rewrite_expr(t.rstrip(';')))
    for p in pre:
        o.w(depth, p)
    o.w(depth, e + ';')


def cond(c):
    return rewrite_expr(c)


def emit(o, nodes, depth):
    i = 0
    while i < len(nodes):
        n = nodes[i]
        k = n[0]
        if k == 'stmt':
            emit_stmt(o, depth, n[1])
        elif k == 'block':
            o.w(depth, '{')
            emit(o, n[1], depth + 1)
            o.w(depth, '}')
        elif k in ('if', 'else if'):
            head = cond(n[1])
            o.w(depth, ('} else if ' if k == 'else if' else 'if ') + head + ' {') if False else None
            # chain: print `if`, then following `else if` / `else` nodes inline
            o.w(depth, 'if %s {' % head)
            emit(o, n[2], depth + 1)
            j = i + 1
            while j < len(nodes) and nodes[j][0] in ('else if', 'else'):
                if nodes[j][0] == 'else if':
                    o.w(depth, '} else if %s {' % cond(nodes[j][1]))
                    emit(o, nodes[j][2], depth + 1)
                else:
                    o.w(depth, '} else {')
                    emit(o, nodes[j][1], depth + 1)
                j += 1
            o.w(depth, '}')
            i = j - 1
        elif k == 'for':
            m = re.match(r'int (\w+) = (.*?); \1 (<|<=) (.*?); \1\+\+$', n[1])
            if m:
                v, a, op, b = m.groups()
                o.w(depth, 'for %s in %s%s%s {' % (snake(v), rewrite_expr(a), '..' if op == '<' else '..=', rewrite_expr(b)))
                emit(o, n[2], depth + 1)
                o.w(depth, '}')
            else:
                m = re.match(r'float (\w+) = (.*?); \1 < (.*?); \1 \+= (.*)$', n[1])
                assert m, n[1]
                v, a, b, step = m.groups()
                KNOWN_FLOAT_VARS.add(snake(v))
                o.w(depth, 'let mut %s: f32 = %s;' % (snake(v), rewrite_expr(a)))
                o.w(depth, 'while %s < %s {' % (snake(v), rewrite_expr(b)))
                emit(o, n[2], depth + 1)
                o.w(depth + 1, '%s += %s;' % (snake(v), rewrite_expr(step)))
                o.w(depth, '}')
        elif k == 'switch':
            emit_switch(o, n, depth)
        elif k == 'else':
            raise ValueError('stray else')
        elif k == 'label':
            raise ValueError('stray label ' + n[1])
        i += 1


def emit_switch(o, n, depth):
    head, body = n[1], n[2]
    arms = []  # (labels, nodes)
    cur_labels, cur_body = [], []
    for node in body:
        if node[0] == 'label':
            if cur_body:
                arms.append((cur_labels, cur_body))
                cur_labels, cur_body = [], []
            cur_labels.append(node[1])
        else:
            cur_body.append(node)
    arms.append((cur_labels, cur_body))
    default = [a for a in arms if any(l == 'default' for l in a[0])]
    others = [a for a in arms if a not in default]
    o.w(depth, 'match %s {' % rewrite_expr(head))
    for labels, nodes in others + default:
        pats = []
        for l in labels:
            if l == 'default':
                pats = ['_']
                break
            pats.append(rewrite_expr(l[len('case '):]))
        o.w(depth + 1, ' | '.join(pats) + ' => {')
        nodes = strip_trailing_break(nodes)
        if len(nodes) == 1 and nodes[0][0] == 'block':
            nodes = nodes[0][1]  # `case N: { ... }` -> the arm's own braces
        emit(o, nodes, depth + 2)
        o.w(depth + 1, '}')
    if not default:
        o.w(depth + 1, '_ => {}')
    o.w(depth, '}')


def strip_trailing_break(nodes):
    nodes = list(nodes)
    while nodes and nodes[-1] == ('stmt', 'break;'):
        nodes.pop()
    if nodes and nodes[-1][0] == 'block':
        inner = strip_trailing_break(nodes[-1][1])
        nodes[-1] = ('block', inner)
    return nodes


def main():
    path, first, last = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
    base = int(sys.argv[4]) if len(sys.argv) > 4 else 8
    lines = open(path, encoding='utf-8').read().split('\n')[first - 1:last]
    nodes, _ = parse(lines + ['}'], 0)
    o = Out(base)
    emit(o, nodes, 0)
    text = '\n'.join(o.lines)
    # tidy the double casts the pass above leaves: `((x as i32) as i32)` -> `(x as i32)`
    prev = None
    while prev != text:
        prev = text
        text = re.sub(r'\(\(([\w.]+) as i32\) as i32\)', r'(\1 as i32)', text)
        text = re.sub(r'\(([\w.]+) as i32\) as i32', r'(\1 as i32)', text)
    print(text)


if __name__ == '__main__':
    main()

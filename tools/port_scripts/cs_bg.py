#!/usr/bin/env python3
"""Typed C# -> Rust converter for the sky / background code of `Main.cs` and `WorldGen.cs`.

    python tools/port_scripts/cs_bg.py <spec.py> > out.rs

`spec.py` (see cs_bg_spec.py) lists the methods to convert (file, first line, last line, Rust signature, ...) and the
vocabulary (static fields, calls). The converter parses the C# method text (statements and expressions), types every
expression the way C# does (int / float / double / byte promotion) and writes the Rust text with the original statement
order and the original local names. It handles exactly the C# that these methods use; anything else stops with an error
naming the construct, so nothing is silently dropped. The output is checked in (`main_backgrounds_gen.rs`); this script is
how it was made.
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))


def snake(name):
    s = re.sub(r"(?<=[a-z0-9])(?=[A-Z])", "_", name)
    s = re.sub(r"(?<=[A-Z])(?=[A-Z][a-z])", "_", s)
    return s.lower()


RUST_KW = {'type', 'ref', 'in', 'loop', 'match', 'fn', 'mod', 'move', 'box', 'impl', 'let', 'use', 'where', 'as', 'self', 'super', 'crate', 'dyn', 'final', 'yield', 'async', 'await', 'abstract', 'become', 'do', 'macro', 'override', 'priv', 'try', 'typeof', 'unsized', 'virtual', 'static', 'const', 'struct', 'enum', 'trait', 'pub', 'mut', 'extern', 'unsafe', 'g', 'ctx'}


def lname(name):
    s = snake(name)
    if s in RUST_KW:
        s += '_'
    return s


class CsError(Exception):
    pass


NUM_RANK = {'byte': 1, 'short': 2, 'ushort': 2, 'int': 3, 'uint': 4, 'long': 5, 'float': 6, 'xfloat': 6, 'double': 7}
RUST_T = {'byte': 'u8', 'short': 'i16', 'ushort': 'u16', 'int': 'i32', 'uint': 'u32', 'long': 'i64', 'float': 'f32', 'xfloat': 'f64', 'double': 'f64', 'bool': 'bool',
          'Color': 'Color', 'Vector2': 'Vector2', 'Vector3': 'Vector3', 'Vector4': 'Vector4', 'Rectangle': 'Rectangle', 'Rectangle?': 'Option<Rectangle>',
          'Texture2D': 'Option<Texture2D>', 'SpriteEffects': 'SpriteEffects', 'VertexColors': 'VertexColors'}


def rust_type(t):
    if t.endswith('[]'):
        return 'Vec<%s>' % rust_type(t[:-2])
    if t in RUST_T:
        return RUST_T[t]
    raise CsError('no Rust type for %s' % t)


def is_num(t):
    return t in NUM_RANK


def promote(a, b):
    # 'xfloat' is a `float` temporary held at double precision (the legacy x86 JIT keeps float temporaries in x87 registers, PORTING.md):
    # float arithmetic produces it, a store to a float field / local / argument or an explicit (float) cast rounds it back to 32 bits
    if a == 'double' or b == 'double':
        return 'double'
    if a == 'xfloat' or b == 'xfloat':
        return 'xfloat'
    if a == 'float' or b == 'float':
        return 'float'
    if a == 'long' or b == 'long' or (a == 'uint' and b == 'int') or (a == 'int' and b == 'uint'):
        return 'long'
    if a == 'uint' and b == 'uint':
        return 'uint'
    return 'int'


def conv(code, frm, to):
    """C# implicit / explicit numeric conversion of an already typed Rust expression."""
    if frm == to:
        return code
    if is_num(frm) and is_num(to):
        return '(%s as %s)' % (code, RUST_T[to])
    if frm == 'Rectangle' and to == 'Rectangle?':
        return 'Some(%s)' % code
    if frm == 'null' and to == 'Rectangle?':
        return 'None'
    raise CsError('cannot convert %s to %s (%s)' % (frm, to, code))


# ------------------------------------------------------------------------------------------------ lexer

TOK = re.compile(r"""
    (?P<ws>\s+|//[^\n]*)
  | (?P<float>\d+\.\d+(?:[eE][+-]?\d+)?[fFdD]?|\d+[eE][+-]?\d+[fFdD]?|\d+[fFdD](?![A-Za-z_0-9]))
  | (?P<int>0[xX][0-9a-fA-F]+|\d+[uUlL]*)
  | (?P<id>[A-Za-z_][A-Za-z_0-9]*)
  | (?P<str>"(?:[^"\\]|\\.)*")
  | (?P<op>>>=|<<=|==|!=|<=|>=|&&|\|\||\+\+|--|\+=|-=|\*=|/=|%=|&=|\|=|\^=|>>|<<|=>|\?\?|[-+*/%<>=!&|^~?:;,.(){}\[\]])
""", re.X)


def lex(src):
    pos = 0
    out = []
    while pos < len(src):
        m = TOK.match(src, pos)
        if not m:
            raise CsError('cannot lex at: ' + src[pos:pos + 40])
        pos = m.end()
        k = m.lastgroup
        if k == 'ws':
            continue
        out.append((k, m.group()))
    out.append(('eof', ''))
    return out


# ------------------------------------------------------------------------------------------------ converter


class Conv:
    def __init__(self, spec, fn):
        self.spec = spec
        self.fn = fn
        self.toks = []
        self.i = 0
        self.scopes = []
        self.out = []
        self.ind = 1
        self.tmp = 0
        self.loops = []  # stack of (kind, label)
        self.lbl = 0
        self.ret = fn.get('ret', 'void')

    # ---- token helpers
    def peek(self, k=0):
        return self.toks[self.i + k]

    def next(self):
        t = self.toks[self.i]
        self.i += 1
        return t

    def at(self, v, k=0):
        t = self.toks[self.i + k]
        return t[1] == v and t[0] in ('op', 'id')

    def eat(self, v):
        if self.at(v):
            self.i += 1
            return True
        return False

    def expect(self, v):
        if not self.eat(v):
            raise CsError('expected %r, got %r near %s' % (v, self.peek()[1], ' '.join(t[1] for t in self.toks[self.i - 6:self.i + 6])))

    def emit(self, s):
        self.out.append('    ' * self.ind + s)

    def newtmp(self):
        self.tmp += 1
        return '__t%d' % self.tmp

    # ---- scopes
    def declare(self, name, t, byref=False):
        self.scopes[-1][name] = (t, byref)

    def lookup(self, name):
        for s in reversed(self.scopes):
            if name in s:
                return s[name]
        return None

    # ---- types
    def try_type(self):
        """Parse a type at the cursor; returns the normalised type or None (cursor restored)."""
        save = self.i
        if self.peek()[0] != 'id':
            return None
        parts = [self.next()[1]]
        while self.at('.') and self.peek(1)[0] == 'id':
            self.next()
            parts.append(self.next()[1])
        name = parts[-1]
        if len(parts) > 1 and parts[0] not in ('Microsoft', 'System', 'Terraria', 'ReLogic'):
            self.i = save
            return None
        if self.at('<') and name == 'Asset':
            # generic: Asset<Texture2D> etc.
            depth = 0
            while True:
                t = self.next()
                if t[1] == '<':
                    depth += 1
                elif t[1] == '>':
                    depth -= 1
                    if depth == 0:
                        break
                elif t[0] == 'eof':
                    raise CsError('unterminated generic')
            name = 'Asset'
        if self.at('?') and name in ('Rectangle', 'int', 'float'):
            self.next()
            name += '?'
        while self.at('[') and self.at(']', 1):
            self.next()
            self.next()
            name += '[]'
        if name not in KNOWN_TYPES:
            self.i = save
            return None
        return name

    # ---- expressions: each returns (code, type)
    def expr(self):
        return self.ternary()

    def ternary(self):
        c, t = self.binary(0)
        if self.at('?'):
            self.next()
            if t != 'bool':
                raise CsError('ternary condition is %s' % t)
            a, ta = self.ternary()
            self.expect(':')
            b, tb = self.ternary()
            if ta == tb:
                rt = ta
            elif is_num(ta) and is_num(tb):
                rt = promote(ta, tb)
                a, b = conv(a, ta, rt), conv(b, tb, rt)
            elif {ta, tb} == {'Rectangle', 'null'}:
                rt = 'Rectangle?'
                a, b = conv(a, ta, rt), conv(b, tb, rt)
            else:
                raise CsError('ternary types %s / %s' % (ta, tb))
            return ('(if %s { %s } else { %s })' % (c, a, b), rt)
        return c, t

    BIN = [['||'], ['&&'], ['|'], ['^'], ['&'], ['==', '!='], ['<', '>', '<=', '>='], ['<<', '>>'], ['+', '-'], ['*', '/', '%']]

    def binary(self, lvl):
        if lvl >= len(self.BIN):
            return self.unary()
        a, ta = self.binary(lvl + 1)
        while self.peek()[0] == 'op' and self.peek()[1] in self.BIN[lvl]:
            op = self.next()[1]
            b, tb = self.binary(lvl + 1)
            a, ta = self.apply_bin(op, a, ta, b, tb)
        return a, ta

    def apply_bin(self, op, a, ta, b, tb):
        if op in ('&&', '||'):
            if ta != 'bool' or tb != 'bool':
                raise CsError('%s on %s %s' % (op, ta, tb))
            return ('(%s %s %s)' % (a, op, b), 'bool')
        if op in ('==', '!=') and tb == 'null' and ta in ('Texture2D', 'Tile'):
            if ta == 'Tile':
                return ('false', 'bool')
            return ('%s.%s()' % (a, 'is_none' if op == '==' else 'is_some'), 'bool')
        if op in ('==', '!='):
            if ta == tb and ta in ('bool', 'Color', 'Vector2', 'SpriteEffects'):
                return ('(%s %s %s)' % (a, op, b), 'bool')
            if is_num(ta) and is_num(tb):
                rt = promote(ta, tb)
                return ('(%s %s %s)' % (conv(a, ta, rt), op, conv(b, tb, rt)), 'bool')
            raise CsError('%s on %s %s' % (op, ta, tb))
        if op in ('<', '>', '<=', '>='):
            if not (is_num(ta) and is_num(tb)):
                raise CsError('%s on %s %s' % (op, ta, tb))
            rt = promote(ta, tb)
            return ('(%s %s %s)' % (conv(a, ta, rt), op, conv(b, tb, rt)), 'bool')
        if op in ('<<', '>>'):
            return ('(%s %s %s)' % (conv(a, ta, 'int'), op, conv(b, tb, 'int')), 'int')
        if op in ('|', '&', '^'):
            if ta == 'bool' and tb == 'bool':
                return ('(%s %s %s)' % (a, op, b), 'bool')
            rt = promote(ta, tb)
            return ('(%s %s %s)' % (conv(a, ta, rt), op, conv(b, tb, rt)), rt)
        # arithmetic
        if is_num(ta) and is_num(tb):
            rt = promote(ta, tb)
            if rt == 'float' and op in ('+', '-', '*', '/'):
                rt = 'xfloat'
            return ('(%s %s %s)' % (conv(a, ta, rt), op, conv(b, tb, rt)), rt)
        if ta in ('Vector2', 'Vector3') and tb == ta and op in ('+', '-', '*', '/'):
            return ('(%s %s %s)' % (a, op, b), ta)
        if ta in ('Vector2', 'Vector3') and is_num(tb) and op in ('*', '/'):
            return ('(%s %s %s)' % (a, op, conv(b, tb, 'float')), ta)
        if tb in ('Vector2', 'Vector3') and is_num(ta) and op == '*':
            return ('(%s * %s)' % (conv(a, ta, 'float'), b), tb)
        if ta == 'Color' and is_num(tb) and op == '*':
            return ('Color::multiply(%s, %s)' % (a, conv(b, tb, 'float')), 'Color')
        if ta == 'Color' and tb == 'Color' and op == '*':
            return ('color_multiply_color(%s, %s)' % (a, b), 'Color')
        raise CsError('binary %s on %s %s' % (op, ta, tb))

    def unary(self):
        t = self.peek()
        if t[1] == '-' and t[0] == 'op':
            self.next()
            c, ty = self.unary()
            if is_num(ty):
                if ty in ('byte', 'short', 'ushort'):
                    c, ty = conv(c, ty, 'int'), 'int'
                return ('(-%s)' % c, ty)
            if ty in ('Vector2', 'Vector3'):
                return ('(-%s)' % c, ty)
            raise CsError('unary - on %s' % ty)
        if t[1] == '!' and t[0] == 'op':
            self.next()
            c, ty = self.unary()
            return ('(!%s)' % c, 'bool')
        if t[1] == '(' and t[0] == 'op':
            # cast?
            save = self.i
            self.next()
            ty = self.try_type()
            if ty is not None and self.at(')') and (is_num(ty) or ty in ('Color', 'Vector2')):
                nxt = self.peek(1)
                if nxt[0] in ('id', 'int', 'float', 'str') or nxt[1] in ('(', '-', '!'):
                    self.next()
                    c, frm = self.unary()
                    if is_num(ty) and is_num(frm):
                        return (conv(c, frm, ty) if frm != ty else c, ty)
                    raise CsError('cast %s -> %s' % (frm, ty))
            self.i = save
        return self.postfix()

    def postfix(self):
        c, t = self.primary()
        while True:
            if self.at('.'):
                self.next()
                name = self.next()[1]
                if self.at('('):
                    args = self.call_args()
                    c, t = self.method_call(c, t, name, args)
                else:
                    c, t = self.member(c, t, name)
            elif self.at('['):
                self.next()
                ic, it = self.expr()
                self.expect(']')
                c, t = self.index(c, t, ic, it)
            elif (self.at('++') or self.at('--')) and self.peek(1)[1] in (')', ',') and is_num(t):
                op = self.next()[1]
                c = '{ let __v = %s; %s %s= 1; __v }' % (c, c, op[0])
            else:
                return c, t

    def call_args(self):
        self.expect('(')
        args = []
        if not self.at(')'):
            while True:
                if self.at('ref') or self.at('out'):
                    kw = self.next()[1]
                    if kw == 'out' and self.at('var'):
                        self.next()
                        name = self.next()[1]
                        args.append((kw + ':' + name, 'out', None))
                    else:
                        c, t = self.expr()
                        args.append((c, t, kw))
                else:
                    c, t = self.expr()
                    args.append((c, t, None))
                if not self.eat(','):
                    break
        self.expect(')')
        return args

    def primary(self):
        k, v = self.peek()
        if k == 'int':
            self.next()
            suffix = v.lower().rstrip('0123456789abcdefx')
            digits = re.sub(r'[uUlL]+$', '', v)
            if 'l' in v.lower():
                return (digits + '_i64', 'long')
            if 'u' in v.lower():
                return (digits + '_u32', 'uint')
            return (digits, 'int')
        if k == 'float':
            self.next()
            vl = v.lower()
            if vl.endswith('f'):
                return (vl[:-1] + '_f32' if not re.fullmatch(r'\d+', vl[:-1]) else vl[:-1] + '_f32', 'float')
            if vl.endswith('d'):
                return (vl[:-1] + '_f64', 'double')
            if re.fullmatch(r'\d+', vl):
                return (vl + '_f64', 'double')
            return (vl + '_f64', 'double')
        if k == 'str':
            self.next()
            return (v, 'string')
        if v == '(':
            self.next()
            c, t = self.expr()
            self.expect(')')
            return ('(%s)' % c, t)
        if v == 'true' or v == 'false':
            self.next()
            return (v, 'bool')
        if v == 'null':
            self.next()
            return ('None', 'null')
        if v == 'default':
            self.next()
            self.expect('(')
            ty = self.try_type()
            self.expect(')')
            if ty == 'Vector2':
                return ('Vector2::ZERO', 'Vector2')
            if ty == 'Color':
                return ('Color::default()', 'Color')
            if ty == 'Rectangle':
                return ('Rectangle::EMPTY', 'Rectangle')
            raise CsError('default(%s)' % ty)
        if v == 'new':
            self.next()
            ty = self.try_type()
            if ty is None:
                raise CsError('new of unknown type near ' + ' '.join(t[1] for t in self.toks[self.i:self.i + 5]))
            args = self.call_args()
            return self.ctor(ty, args)
        if k == 'id':
            # qualified static classes: Math.X, MathHelper.X, Vector2.Zero, Color.White, SpriteEffects.None, TextureAssets.X...
            name = self.next()[1]
            if name == 'byte' and self.at('.') and self.peek(1)[1] == 'MaxValue':
                self.next()
                self.next()
                return ('255_u8', 'byte')
            if name in STATIC_CLASSES and self.at('.'):
                self.next()
                member = self.next()[1]
                if self.at('('):
                    args = self.call_args()
                    return self.static_call(name, member, args)
                return self.static_member(name, member)
            if self.at('('):
                args = self.call_args()
                return self.static_call(None, name, args)
            if name == 'tile' and self.at('[') and self.lookup('tile') is None:
                self.next()
                xc, xt = self.expr()
                self.expect(',')
                yc, yt = self.expr()
                self.expect(']')
                if getattr(self, 'in_try', 0):
                    return ('%s.main.tile.try_get(%s, %s)?' % (self.fn['G'], conv(xc, xt, 'int'), conv(yc, yt, 'int')), 'Tile')
                return ('(*%s.main.tile.get(%s, %s))' % (self.fn['G'], conv(xc, xt, 'int'), conv(yc, yt, 'int')), 'Tile')
            loc = self.lookup(name)
            if loc is not None:
                t, byref = loc
                return ('(*%s)' % lname(name) if byref else lname(name), t)
            return self.static_field(name)
        raise CsError('unexpected token %r' % (v,))

    # ---- hooks resolved through the spec
    def static_field(self, name):
        f = self.spec.resolve_static(self.fn, name) if hasattr(self.spec, 'resolve_static') else self.spec.STATICS.get(name)
        if f is None:
            raise CsError('unknown identifier %s' % name)
        t, tmpl = f
        return (tmpl.replace('{G}', self.fn['G']), t)

    def static_member(self, cls, member):
        key = cls + '.' + member
        if key in self.spec.STATIC_MEMBERS:
            t, code = self.spec.STATIC_MEMBERS[key]
            return (code.replace('{G}', self.fn['G']), t)
        raise CsError('unknown static member ' + key)

    def static_call(self, cls, name, args):
        key = (cls + '.' if cls else '') + name
        if cls is None and (name in self.spec.OWN or name in self.spec.HAND):
            return self.spec.own_call(self, name, args)
        h = self.spec.CALLS.get(key)
        if h is None and cls in ('WorldGen', 'Main') and name in self.spec.OWN:
            return self.spec.own_call(self, name, args)
        if h is None:
            raise CsError('unknown call %s(%s)' % (key, ', '.join(a[1] for a in args)))
        return h(self, args)

    def ctor(self, ty, args):
        tys = [a[1] for a in args]
        cs = [a[0] for a in args]
        if ty == 'Vector2':
            return ('Vector2::new(%s, %s)' % (conv(cs[0], tys[0], 'float'), conv(cs[1], tys[1], 'float')), 'Vector2') if len(args) == 2 else ('Vector2::splat(%s)' % conv(cs[0], tys[0], 'float'), 'Vector2')
        if ty == 'Vector3':
            if len(args) == 1:
                return ('Vector3::new(%s, %s, %s)' % ((conv(cs[0], tys[0], 'float'),) * 3), 'Vector3')
            return ('Vector3::new(%s)' % ', '.join(conv(c, t, 'float') for c, t in zip(cs, tys)), 'Vector3')
        if ty == 'Rectangle':
            return ('Rectangle::new(%s)' % ', '.join(conv(c, t, 'int') for c, t in zip(cs, tys)), 'Rectangle')
        if ty == 'Color':
            if len(args) == 1 and tys[0] == 'Vector3':
                return ('Color::from_vector3(%s)' % cs[0], 'Color')
            if all(t == 'byte' for t in tys):
                return ('Color::new(%s)' % ', '.join(cs), 'Color')
            if all(t in ('int',) for t in tys):
                return ('Color::from_ints%s(%s)' % ('_rgb' if len(args) == 3 else '', ', '.join(cs)), 'Color')
            if all(t in ('float', 'double') for t in tys):
                return ('Color::from_floats%s(%s)' % ('_rgb' if len(args) == 3 else '', ', '.join(conv(c, t, 'float') for c, t in zip(cs, tys))), 'Color')
            if len(args) == 4 and all(is_num(t) for t in tys):
                return ('Color::from_ints(%s)' % ', '.join(conv(c, t, 'int') for c, t in zip(cs, tys)), 'Color')
            raise CsError('new Color(%s)' % tys)
        raise CsError('new %s' % ty)

    def member(self, c, t, name):
        key = t + '.' + name
        m = self.spec.MEMBERS.get(key)
        if m is None:
            raise CsError('unknown member %s of %s' % (name, t))
        rt, code = m
        return (code.replace('{0}', c), rt)

    def method_call(self, c, t, name, args):
        key = t + '.' + name + '()'
        h = self.spec.METHODS.get(key)
        if h is None:
            raise CsError('unknown method %s on %s' % (name, t))
        return h(self, c, args)

    def index(self, c, t, ic, it):
        if t.endswith('[]') or t.startswith('arr:'):
            et = t[4:] if t.startswith('arr:') else t[:-2]
            idx = conv(ic, it, 'int')
            return ('%s[%s as usize]' % (c, idx), et)
        if t.startswith('TA:'):
            return (self.spec.texture_code(self.fn, t[3:], conv(ic, it, 'int')), 'Texture2D')
        raise CsError('index on %s' % t)

    # ---- lvalues & statements
    def lvalue_parse(self):
        """Parses an assignable expression; returns (kind, data, type). kind: 'plain' code | 'set' (setter format)."""
        save = self.i
        # try: parse a postfix expression, then inspect the last accessor by re-lexing is complex; we special-case by running expr() and
        # recording the resulting code, then mapping Color channel reads to setters.
        c, t = self.postfix()
        return c, t

    def block_or_stmt(self):
        if self.at('{'):
            self.next()
            self.emit('{')
            self.ind += 1
            self.scopes.append({})
            while not self.at('}'):
                self.stmt()
            self.next()
            self.scopes.pop()
            self.ind -= 1
            self.emit('}')
        else:
            self.emit('{')
            self.ind += 1
            self.scopes.append({})
            self.stmt()
            self.scopes.pop()
            self.ind -= 1
            self.emit('}')

    def cond(self):
        self.expect('(')
        c, t = self.expr()
        self.expect(')')
        if t != 'bool':
            raise CsError('condition of type %s' % t)
        return c

    def assign_to(self, lc, lt, op, rc, rt):
        if op != '=':
            bop = op[:-1]
            rc, rt = self.apply_bin(bop, lc, lt, rc, rt)
        if is_num(lt) and is_num(rt):
            rc = conv(rc, rt, lt)
        elif lt != rt:
            if lt == 'Rectangle?' and rt in ('Rectangle', 'null'):
                rc = conv(rc, rt, lt)
            elif lt == 'Color' and rt == 'Color':
                pass
            elif lt in ('Asset', 'Texture2D') and rt in ('Asset', 'Texture2D'):
                pass
            else:
                raise CsError('assign %s = %s' % (lt, rt))
        return rc

    def stmt(self):
        k, v = self.peek()
        if v == '{' and k == 'op':
            self.block_or_stmt()
            return
        if v == ';':
            self.next()
            return
        if v == 'if':
            self.next()
            c = self.cond()
            self.emit('if %s' % c)
            self.block_or_stmt_tail()
            while self.at('else'):
                self.next()
                if self.at('if'):
                    self.next()
                    c = self.cond()
                    self.out[-1] += ' else if %s' % c
                    self.block_or_stmt_tail()
                else:
                    self.out[-1] += ' else'
                    self.block_or_stmt_tail()
            return
        if v == 'for':
            self.for_stmt()
            return
        if v == 'while':
            self.next()
            c = self.cond()
            self.emit('while %s' % c)
            self.loops.append(('while', None))
            self.block_or_stmt_tail()
            self.loops.pop()
            return
        if v == 'switch':
            self.switch_stmt()
            return
        if v == 'try':
            # try { A } catch { B }: A runs in a closure; a `?` on a missing tile (a null / out-of-range access in the original) leaves it, B follows
            self.next()
            self.tryn = getattr(self, 'tryn', 0) + 1
            name = '__r%d' % self.tryn
            self.emit('let %s: Option<()> = (|| -> Option<()> {' % name)
            self.ind += 1
            self.in_try = getattr(self, 'in_try', 0) + 1
            self.expect('{')
            self.scopes.append({})
            while not self.at('}'):
                self.stmt()
            self.next()
            self.scopes.pop()
            self.in_try -= 1
            self.emit('Some(())')
            self.ind -= 1
            self.emit('})();')
            if not self.at('catch'):
                raise CsError('try without catch')
            self.next()
            self.emit('if %s.is_none() {' % name)
            self.ind += 1
            self.expect('{')
            self.scopes.append({})
            while not self.at('}'):
                self.stmt()
            self.next()
            self.scopes.pop()
            self.ind -= 1
            self.emit('}')
            return
        if v == 'return':
            self.next()
            if self.at(';'):
                self.next()
                self.emit('return;')
            else:
                c, t = self.expr()
                self.expect(';')
                self.emit('return %s;' % (conv(c, t, self.ret) if t != self.ret and (is_num(t) or t in ('null', 'Rectangle')) else c))
            return
        if v == 'continue':
            self.next()
            self.expect(';')
            kind, lbl = self.loops[-1]
            self.emit('continue;' if kind == 'while' else "break '%s;" % lbl)
            return
        if v == 'break':
            self.next()
            self.expect(';')
            if self.loops and self.loops[-1][0] == 'switch':
                if not self.at('}'):
                    raise CsError('break in the middle of a switch section')
                return
            self.emit('break;')
            return
        # declaration?
        save = self.i
        ty = self.try_type()
        if ty is not None and self.peek()[0] == 'id' and self.peek(1)[1] in ('=', ';'):
            name = self.next()[1]
            if self.eat('='):
                c, t = self.expr()
                if is_num(ty) and is_num(t):
                    c = conv(c, t, ty)
                elif ty != t:
                    if ty == 'Rectangle?' and t in ('Rectangle', 'null'):
                        c = conv(c, t, ty)
                    elif ty.endswith('[]') or ty == 'Asset' or ty == 'Texture2D' or ty == t:
                        if ty.endswith('[]'):
                            c = '%s.to_vec()' % c
                    else:
                        raise CsError('init %s %s = %s' % (ty, name, t))
                self.expect(';')
                if ty.endswith('[]') and not c.endswith('.to_vec()'):
                    c = '%s.to_vec()' % c
                self.declare(name, ty)
                self.emit_decl(name, ty, c)
            else:
                self.expect(';')
                self.declare(name, ty)
                self.emit('let mut %s: %s = %s;' % (lname(name), rust_type(ty), DEFAULT[ty]))
            return
        self.i = save
        if v == 'var':
            self.next()
            name = self.next()[1]
            self.expect('=')
            c, t = self.expr()
            self.expect(';')
            self.declare(name, t)
            self.emit_decl(name, t, c)
            return
        # expression statement / assignment / inc-dec
        self.expr_stmt()

    def emit_decl(self, name, ty, c):
        self.emit_code('let mut %s: %s = %%s;' % (lname(name), rust_type(ty) if ty not in ('Asset',) else 'Option<Texture2D>'), c)

    def emit_code(self, fmt, c):
        """Emits fmt % c, expanding pending {TEX:..} texture lookups into temporaries first."""
        pre, c = self.expand_tex(c)
        for p in pre:
            self.emit(p)
        self.emit(fmt % c if '%s' in fmt else fmt)

    def expand_tex(self, c):
        return [], c

    def block_or_stmt_tail(self):
        """Statement body appended to the current `if`/`while` header line."""
        self.out[-1] += ' {'
        self.ind += 1
        self.scopes.append({})
        if self.at('{'):
            self.next()
            while not self.at('}'):
                self.stmt()
            self.next()
        else:
            self.stmt()
        self.scopes.pop()
        self.ind -= 1
        self.emit('}')

    def for_stmt(self):
        self.next()
        self.expect('(')
        self.scopes.append({})
        # init
        init_lines = []
        if not self.at(';'):
            n0 = len(self.out)
            self.stmt()  # consumes the ';'
            init_lines = self.out[n0:]
            del self.out[n0:]
        else:
            self.next()
        c, t = ('true', 'bool')
        if not self.at(';'):
            c, t = self.expr()
        self.expect(';')
        step_start = self.i
        depth = 0
        while not (self.at(')') and depth == 0):
            if self.at('('):
                depth += 1
            if self.at(')'):
                depth -= 1
            self.next()
        step_end = self.i
        self.expect(')')
        self.lbl += 1
        label = 'c%d' % self.lbl
        self.emit('{')
        self.ind += 1
        for l in init_lines:
            self.out.append('    ' + l)
        self.emit('while %s {' % c)
        self.ind += 1
        self.loops.append(('for', label))
        self.emit("'%s: {" % label)
        self.ind += 1
        self.scopes.append({})
        if self.at('{'):
            self.next()
            while not self.at('}'):
                self.stmt()
            self.next()
        else:
            self.stmt()
        self.scopes.pop()
        self.ind -= 1
        self.emit('}')
        # step statements
        save_i, save_toks = self.i, self.toks
        self.toks = self.toks[step_start:step_end] + [('op', ';'), ('eof', '')]
        self.i = 0
        while not self.at(';') or self.i == 0:
            if self.peek()[0] == 'eof':
                break
            self.expr_stmt(step=True)
            if self.eat(','):
                continue
            break
        self.toks, self.i = save_toks, save_i
        self.loops.pop()
        self.ind -= 1
        self.emit('}')
        self.ind -= 1
        self.emit('}')
        self.scopes.pop()

    def switch_stmt(self):
        self.next()
        self.expect('(')
        c, t = self.expr()
        self.expect(')')
        self.expect('{')
        if t == 'bool':
            raise CsError('switch on bool')
        if is_num(t) and t != 'int':
            c = conv(c, t, 'int')
        arms = []
        default = None
        while not self.at('}'):
            labels = []
            is_default = False
            while self.at('case') or self.at('default'):
                if self.eat('default'):
                    self.expect(':')
                    is_default = True
                else:
                    self.next()
                    lc, lt = self.expr()
                    self.expect(':')
                    labels.append(lc)
            # body until break/return/next label at statement level
            n0 = len(self.out)
            self.ind += 2
            self.scopes.append({})
            self.loops.append(('switch', None))
            saved_loops = self.loops
            ended = False
            while not (self.at('case') or self.at('default') or self.at('}')):
                if self.at('break'):
                    self.next()
                    self.expect(';')
                    ended = True
                    break
                self.stmt()
                if self.out and self.out[-1].strip().startswith('return') and self.at('break'):
                    pass
            self.loops.pop()
            self.scopes.pop()
            self.ind -= 2
            body = self.out[n0:]
            del self.out[n0:]
            arms.append((labels, is_default, body))
        self.expect('}')
        self.emit('match %s {' % c)
        order = [a for a in arms if not a[1]] + [a for a in arms if a[1]]
        for labels, is_default, body in order:
            pat = ' | '.join(labels) if labels else '_'
            if is_default and labels:
                raise CsError('case labels together with default')
            self.emit('    %s => {' % pat)
            self.out.extend(body)
            self.emit('    }')
        if not any(a[1] for a in arms):
            self.emit('    _ => {}')
        self.emit('}')

    def expr_stmt(self, step=False):
        # statement forms: lhs op= expr ; lhs++ ; call(...) ;
        save = self.i
        # prefix ++/--
        if self.at('++') or self.at('--'):
            op = self.next()[1]
            c, t = self.postfix()
            self.finish_stmt(step)
            self.emit('%s %s= 1;' % (c, op[0]))
            return
        c, t = self.postfix()
        if self.at('++') or self.at('--'):
            op = self.next()[1]
            self.finish_stmt(step)
            self.emit_assign(c, t, op[0] + '=', '1', 'int')
            return
        k, v = self.peek()
        if k == 'op' and v in ('=', '+=', '-=', '*=', '/=', '%=', '&=', '|=', '^=', '>>=', '<<='):
            self.next()
            rc, rt = self.expr()
            self.finish_stmt(step)
            self.emit_assign(c, t, v, rc, rt)
            return
        self.finish_stmt(step)
        if c == '':
            return
        # plain call statement
        pre, c = self.expand_tex(c)
        for p in pre:
            self.emit(p)
        for line in c.split('\n'):
            self.emit(line + ';' if not line.rstrip().endswith(('}', ';')) else line)

    def finish_stmt(self, step):
        if step:
            return
        self.expect(';')

    def emit_assign(self, lc, lt, op, rc, rt):
        # setter forms are encoded by member(): a code containing '.__set__(' pattern
        m = re.fullmatch(r'(.*)\.(r|g|b|a)\(\)', lc)
        if m and lt == 'byte':
            base, ch = m.groups()
            if op != '=':
                rc2, rt2 = self.apply_bin(op[:-1], lc, lt, rc, rt)
                rc = conv(rc2, rt2, 'byte')
            else:
                rc = conv(rc, rt, 'byte')
            pre, rc = self.expand_tex(rc)
            self.emit('%s.set_%s(%s);' % (base, ch, rc))
            return
        rc = self.assign_to(lc, lt, op, rc, rt)
        pre, rc = self.expand_tex(rc)
        for p in pre:
            self.emit(p)
        lc2 = re.sub(r'^\(\*(\w+)\)$', r'*\1', lc)
        if lc2.endswith('.floor()'):
            raise CsError('assign to call result')
        self.emit('%s = %s;' % (lc2, rc))

    # ---- driver
    def run(self, body_src):
        self.toks = lex(body_src)
        self.i = 0
        self.scopes = [{}]
        for name, t, byref in self.fn['params']:
            self.declare(name, t, byref)
        while self.peek()[0] != 'eof':
            self.stmt()
        return '\n'.join(self.out)


KNOWN_TYPES = {'int', 'float', 'double', 'bool', 'byte', 'uint', 'long', 'short', 'ushort', 'Color', 'Vector2', 'Vector3', 'Rectangle', 'Texture2D', 'Asset', 'SpriteEffects', 'void', 'int[]', 'float[]', 'Color[]', 'Rectangle?', 'int?', 'float?', 'Vector4', 'VertexColors'}
STATIC_CLASSES = {'Math', 'MathHelper', 'Vector2', 'Color', 'SpriteEffects', 'TextureAssets', 'Lighting', 'WorldGen', 'Utils', 'Array', 'Camera', 'SceneMetrics', 'Main', 'Filters', 'SkyManager', 'Rectangle', 'Vector3', 'DontStarveSeed', 'NPC', 'LanternNight', 'AuroraSky', 'Terraria', 'SurfaceBackgroundID', 'MapHelper', 'HorizonHelper', 'Star', 'Cloud', 'RenderLayers', 'Overlays', 'PlayerInput', 'MoonPhase', 'HorizonRenderer', 'FocusHelper'}
DEFAULT = {'int': '0', 'float': '0.0_f32', 'double': '0.0_f64', 'bool': 'false', 'byte': '0', 'uint': '0', 'long': '0', 'short': '0', 'ushort': '0', 'Color': 'Color::default()', 'Vector2': 'Vector2::ZERO', 'Vector3': 'Vector3::ZERO', 'Rectangle': 'Rectangle::EMPTY', 'Rectangle?': 'None'}


def extract_method(path, first, last):
    with open(path, encoding='utf-8-sig') as f:
        lines = f.read().split('\n')
    seg = lines[first - 1:last]
    text = '\n'.join(seg)
    # body between the first '{' after the signature and the last '}'
    i = text.index('{')
    j = text.rindex('}')
    sig = text[:i]
    body = text[i + 1:j]
    return sig, body


def preprocess(body, keep_main=False):
    body = re.sub(r'\bMicrosoft\.Xna\.Framework\.Graphics\.', '', body)
    body = re.sub(r'\bMicrosoft\.Xna\.Framework\.', '', body)
    body = re.sub(r'\bTerraria\.Graphics\.Effects\.', '', body)
    body = re.sub(r'(?<![\w.])Main\.(?=[a-zA-Z_])', 'M_' if keep_main else '', body)
    body = re.sub(r'\binstance\.', '', body)
    return body


def main():
    spec_path = sys.argv[1]
    import importlib.util
    sp = importlib.util.spec_from_file_location('spec', spec_path)
    spec = importlib.util.module_from_spec(sp)
    sp.loader.exec_module(spec)
    outs = [spec.HEADER]
    for fn in spec.FUNCTIONS:
        sig, body = extract_method(fn['file'], fn['first'], fn['last'])
        for pat, rp in fn.get('pre', []):
            body, n = re.subn(pat, rp, body, flags=re.S)
            if n == 0 and 'try' not in pat and 'LoadBackground' not in pat and '_ =' not in pat and 'DrawToDepth' not in pat:
                sys.stderr.write('WARNING: rewrite %r did not match in %s\n' % (pat, fn['name']))
        body = preprocess(body, fn.get('keep_main'))
        c = Conv(spec, fn)
        try:
            text = c.run(body)
        except CsError as e:
            sys.stderr.write('ERROR in %s (%s:%d): %s\n' % (fn['name'], os.path.basename(fn['file']), fn['first'], e))
            raise SystemExit(1)
        outs.append('/// `%s` (%s:%d)' % (fn['cs'], os.path.basename(fn['file']), fn['first']))
        if fn.get('doc'):
            outs.append('/// ' + fn['doc'])
        outs.append(fn['sig'] + ' {')
        outs.append(text)
        outs.append('}\n')
    sys.stdout.write('\n'.join(outs))


if __name__ == '__main__':
    main()

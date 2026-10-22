//! A backtracking regular expression engine with the semantics of .NET's `System.Text.RegularExpressions` for the
//! constructs the game and ordinary chat / file-name patterns use (the `regex` crate has no look-around):
//!
//! literals, `.`, classes (`[a-z]`, `[^:]`, `\w \d \s` inside), `\w \W \d \D \s \S \b \B \A \z \Z`, escapes
//! (`\n \r \t \f \v \e \0 \xHH \uHHHH`), anchors `^ $` (with `Multiline`), groups (`( )`, `(?: )`, `(?<name> )`,
//! `(?'name' )`, atomic `(?> )`), look-ahead and look-behind (`(?= )`, `(?! )`, `(?<= )`, `(?<! )`), alternation,
//! greedy and lazy quantifiers (`* + ? {n} {n,} {n,m}`), back references (`\1`, `\k<name>`), inline options
//! (`(?i)`, `(?i: )`, `(?imsnx-imsnx)`), `\p{L}`-style categories for the common ones.
//!
//! Semantics that match .NET: leftmost match, alternatives and greedy / lazy quantifiers tried in the same order as
//! a backtracking engine does; **named groups are numbered after all unnamed ones**; a quantified group keeps the
//! capture of its last iteration; `$` matches at the end and before a final `\n` (anywhere before `\n` with
//! `Multiline`); `.` excludes only `\n` unless `Singleline`; an empty match makes the next search start one
//! character later. `\w` is letters, digits and connector punctuation as Rust classifies them (differs from .NET's
//! Unicode tables for a few exotic characters; combining marks are not word characters here).
//!
//! Not supported (pattern construction fails with `Unsupported`): `RightToLeft`, conditionals `(?(..)..)`, balancing
//! groups, class subtraction `[a-z-[aeiou]]`, `\G`, `\p{..}` outside the listed categories.

pub const IGNORE_CASE: i32 = 1;
pub const MULTILINE: i32 = 2;
pub const EXPLICIT_CAPTURE: i32 = 4;
pub const SINGLELINE: i32 = 16;
pub const IGNORE_PATTERN_WHITESPACE: i32 = 32;
pub const RIGHT_TO_LEFT: i32 = 64;

#[derive(Debug, Clone)]
pub struct ParseError(pub String);

#[derive(Debug, Clone)]
enum Item {
    Range(char, char),
    Word(bool),
    Digit(bool),
    Space(bool),
    Cat(char, bool),
}

#[derive(Debug, Clone)]
struct Class {
    negated: bool,
    items: Vec<Item>,
    ci: bool,
}

#[derive(Debug, Clone)]
enum Node {
    Empty,
    Char(char, bool),
    Any(bool),
    Class(Box<Class>),
    /// `^` (multiline flag), `$` (multiline flag), `\A`, `\z`, `\Z`, `\b`, `\B`
    LineStart(bool),
    LineEnd(bool),
    TextStart,
    TextEnd,
    TextEndZ,
    WordB(bool),
    Seq(Vec<Node>),
    Alt(Vec<Node>),
    /// capture appearance index
    Group(Box<Node>, usize),
    NonCapture(Box<Node>),
    Atomic(Box<Node>),
    Repeat { node: Box<Node>, min: u32, max: u32, greedy: bool },
    Look { node: Box<Node>, ahead: bool, negate: bool },
    BackRef(usize, bool),
    NamedBackRef(String, bool),
}

#[derive(Debug)]
pub struct Program {
    node: Node,
    /// group number of each capturing group by appearance index
    number_of: Vec<usize>,
    /// names[number] = Some(name) for a named group; group 0 is the whole match
    pub names: Vec<Option<String>>,
    pub group_count: usize,
}

// ------------------------------------------------------------------------------------------------ parser

struct Flags {
    ci: bool,
    multi: bool,
    single: bool,
    explicit: bool,
    ignore_ws: bool,
}

struct Parser<'a> {
    c: Vec<char>,
    i: usize,
    f: Flags,
    groups: Vec<Option<String>>,
    _p: std::marker::PhantomData<&'a ()>,
}

fn unsupported(what: &str) -> ParseError {
    ParseError(format!("Unsupported: {what}"))
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<char> {
        self.c.get(self.i).copied()
    }
    fn eat(&mut self, ch: char) -> bool {
        if self.peek() == Some(ch) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn parse_alt(&mut self) -> Result<Node, ParseError> {
        let mut alts = vec![self.parse_seq()?];
        while self.eat('|') {
            alts.push(self.parse_seq()?);
        }
        Ok(if alts.len() == 1 { alts.pop().expect("one") } else { Node::Alt(alts) })
    }

    fn parse_seq(&mut self) -> Result<Node, ParseError> {
        let mut items = Vec::new();
        loop {
            match self.peek() {
                None | Some('|') | Some(')') => break,
                _ => {}
            }
            if self.f.ignore_ws {
                match self.peek() {
                    Some(c) if c.is_whitespace() => {
                        self.i += 1;
                        continue;
                    }
                    Some('#') => {
                        while let Some(c) = self.peek() {
                            self.i += 1;
                            if c == '\n' {
                                break;
                            }
                        }
                        continue;
                    }
                    _ => {}
                }
            }
            let Some(atom) = self.parse_atom()? else { continue };
            let atom = self.parse_quantifier(atom)?;
            items.push(atom);
        }
        Ok(match items.len() {
            0 => Node::Empty,
            1 => items.pop().expect("one"),
            _ => Node::Seq(items),
        })
    }

    fn parse_quantifier(&mut self, mut atom: Node) -> Result<Node, ParseError> {
        loop {
            let (min, max) = match self.peek() {
                Some('*') => {
                    self.i += 1;
                    (0, u32::MAX)
                }
                Some('+') => {
                    self.i += 1;
                    (1, u32::MAX)
                }
                Some('?') => {
                    self.i += 1;
                    (0, 1)
                }
                Some('{') => match self.try_braces() {
                    Some(r) => r,
                    None => return Ok(atom),
                },
                _ => return Ok(atom),
            };
            let greedy = !self.eat('?');
            atom = Node::Repeat { node: Box::new(atom), min, max, greedy };
        }
    }

    /// `{n}`, `{n,}`, `{n,m}`; otherwise `{` is a literal and nothing is consumed.
    fn try_braces(&mut self) -> Option<(u32, u32)> {
        let save = self.i;
        self.i += 1;
        let num = |p: &mut Parser| -> Option<u32> {
            let st = p.i;
            while p.peek().is_some_and(|c| c.is_ascii_digit()) {
                p.i += 1;
            }
            if p.i == st {
                None
            } else {
                p.c[st..p.i].iter().collect::<String>().parse().ok()
            }
        };
        let r = (|| {
            let min = num(self)?;
            let max = if self.eat(',') {
                if self.peek() == Some('}') {
                    u32::MAX
                } else {
                    num(self)?
                }
            } else {
                min
            };
            if self.eat('}') {
                Some((min, max))
            } else {
                None
            }
        })();
        if r.is_none() {
            self.i = save;
        }
        r
    }

    fn new_group(&mut self, name: Option<String>) -> usize {
        // the same name is the same group
        if let Some(n) = &name {
            if let Some(i) = self.groups.iter().position(|g| g.as_deref() == Some(n.as_str())) {
                return i;
            }
        }
        self.groups.push(name);
        self.groups.len() - 1
    }

    fn parse_atom(&mut self) -> Result<Option<Node>, ParseError> {
        let c = self.peek().expect("atom");
        self.i += 1;
        Ok(Some(match c {
            '.' => Node::Any(self.f.single),
            '^' => Node::LineStart(self.f.multi),
            '$' => Node::LineEnd(self.f.multi),
            '[' => self.parse_class()?,
            '(' => return self.parse_group(),
            '\\' => self.parse_escape()?,
            '*' | '+' | '?' => return Err(ParseError("Quantifier {x,y} following nothing.".into())),
            c => Node::Char(c, self.f.ci),
        }))
    }

    fn parse_group(&mut self) -> Result<Option<Node>, ParseError> {
        let old_flags = Flags { ci: self.f.ci, multi: self.f.multi, single: self.f.single, explicit: self.f.explicit, ignore_ws: self.f.ignore_ws };
        let node;
        if self.eat('?') {
            match self.peek() {
                Some(':') => {
                    self.i += 1;
                    node = Node::NonCapture(Box::new(self.parse_alt()?));
                }
                Some('=') | Some('!') => {
                    let negate = self.peek() == Some('!');
                    self.i += 1;
                    node = Node::Look { node: Box::new(self.parse_alt()?), ahead: true, negate };
                }
                Some('>') => {
                    self.i += 1;
                    node = Node::Atomic(Box::new(self.parse_alt()?));
                }
                Some('#') => {
                    while let Some(c) = self.peek() {
                        self.i += 1;
                        if c == ')' {
                            return Ok(None);
                        }
                    }
                    return Err(ParseError("Unterminated (?#...) comment".into()));
                }
                Some('<') if matches!(self.c.get(self.i + 1), Some('=') | Some('!')) => {
                    let negate = self.c[self.i + 1] == '!';
                    self.i += 2;
                    node = Node::Look { node: Box::new(self.parse_alt()?), ahead: false, negate };
                }
                Some(q @ ('<' | '\'')) => {
                    self.i += 1;
                    let close = if q == '<' { '>' } else { '\'' };
                    let mut name = String::new();
                    loop {
                        match self.peek() {
                            Some(c) if c == close => {
                                self.i += 1;
                                break;
                            }
                            Some(c) => {
                                name.push(c);
                                self.i += 1;
                            }
                            None => return Err(ParseError("Unterminated group name".into())),
                        }
                    }
                    if name.is_empty() || name.contains('-') {
                        return Err(unsupported("balancing groups / empty group names"));
                    }
                    if name.chars().all(|c| c.is_ascii_digit()) {
                        return Err(unsupported("numeric group names"));
                    }
                    let idx = self.new_group(Some(name));
                    node = Node::Group(Box::new(self.parse_alt()?), idx);
                }
                Some('(') => return Err(unsupported("conditional groups")),
                _ => {
                    // (?imsnx-imsnx) or (?imsnx-imsnx: ... )
                    let mut on = true;
                    let (mut ci, mut multi, mut single, mut explicit, mut ws) = (self.f.ci, self.f.multi, self.f.single, self.f.explicit, self.f.ignore_ws);
                    loop {
                        match self.peek() {
                            Some('i') => ci = on,
                            Some('m') => multi = on,
                            Some('s') => single = on,
                            Some('n') => explicit = on,
                            Some('x') => ws = on,
                            Some('-') => on = false,
                            Some(':') => {
                                self.i += 1;
                                self.f = Flags { ci, multi, single, explicit, ignore_ws: ws };
                                let inner = self.parse_alt()?;
                                self.f = old_flags;
                                if !self.eat(')') {
                                    return Err(ParseError("Not enough )'s.".into()));
                                }
                                return Ok(Some(Node::NonCapture(Box::new(inner))));
                            }
                            Some(')') => {
                                self.i += 1;
                                // the flags apply to the rest of the enclosing group
                                self.f = Flags { ci, multi, single, explicit, ignore_ws: ws };
                                return Ok(None);
                            }
                            _ => return Err(ParseError("Unrecognized grouping construct.".into())),
                        }
                        self.i += 1;
                    }
                }
            }
        } else if self.f.explicit {
            node = Node::NonCapture(Box::new(self.parse_alt()?));
        } else {
            let idx = self.new_group(None);
            node = Node::Group(Box::new(self.parse_alt()?), idx);
        }
        if !self.eat(')') {
            return Err(ParseError("Not enough )'s.".into()));
        }
        // flags set inside a group end with the group
        self.f = old_flags;
        Ok(Some(node))
    }

    fn escape_class(&mut self, c: char) -> Option<Item> {
        Some(match c {
            'd' => Item::Digit(true),
            'D' => Item::Digit(false),
            'w' => Item::Word(true),
            'W' => Item::Word(false),
            's' => Item::Space(true),
            'S' => Item::Space(false),
            _ => return None,
        })
    }

    /// The character an escape stands for (`\n`, `\x41`, `A`, `\.`...).
    fn escape_char(&mut self, c: char) -> Result<char, ParseError> {
        Ok(match c {
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            'f' => '\u{c}',
            'v' => '\u{b}',
            'e' => '\u{1b}',
            'a' => '\u{7}',
            '0' => '\0',
            'x' | 'u' => {
                let n = if c == 'x' { 2 } else { 4 };
                let h: String = self.c.get(self.i..self.i + n).ok_or_else(|| ParseError("Insufficient hexadecimal digits.".into()))?.iter().collect();
                self.i += n;
                char::from_u32(u32::from_str_radix(&h, 16).map_err(|_| ParseError("Insufficient hexadecimal digits.".into()))?).unwrap_or('\u{fffd}')
            }
            c if c.is_ascii_alphanumeric() => return Err(ParseError(format!("Unrecognized escape sequence \\{c}."))),
            c => c,
        })
    }

    fn category(&mut self, negated: bool) -> Result<Item, ParseError> {
        if !self.eat('{') {
            return Err(ParseError("Malformed \\p{X} character escape.".into()));
        }
        let mut name = String::new();
        while let Some(c) = self.peek() {
            self.i += 1;
            if c == '}' {
                break;
            }
            name.push(c);
        }
        let key = match name.as_str() {
            "L" | "Lu" | "Ll" | "N" | "Nd" | "P" | "Z" | "Zs" | "S" => name.chars().next().expect("non-empty"),
            _ => return Err(unsupported(&format!("\\p{{{name}}}"))),
        };
        // single-letter keys: L letters, N numbers (Nd digits), P punctuation, Z separators; 'u' / 'l' for Lu / Ll
        let key = match name.as_str() {
            "Lu" => 'u',
            "Ll" => 'l',
            "Nd" => 'd',
            "Zs" => 'z',
            _ => key,
        };
        Ok(Item::Cat(key, !negated))
    }

    fn parse_escape(&mut self) -> Result<Node, ParseError> {
        let Some(c) = self.peek() else { return Err(ParseError("Illegal \\ at end of pattern.".into())) };
        self.i += 1;
        Ok(match c {
            'A' => Node::TextStart,
            'z' => Node::TextEnd,
            'Z' => Node::TextEndZ,
            'b' => Node::WordB(true),
            'B' => Node::WordB(false),
            'G' => return Err(unsupported("\\G")),
            'k' => {
                let close = match self.peek() {
                    Some('<') => '>',
                    Some('\'') => '\'',
                    _ => return Err(ParseError("Malformed \\k<...> named back reference.".into())),
                };
                self.i += 1;
                let mut name = String::new();
                while let Some(ch) = self.peek() {
                    self.i += 1;
                    if ch == close {
                        break;
                    }
                    name.push(ch);
                }
                Node::NamedBackRef(name, self.f.ci)
            }
            'p' | 'P' => Node::Class(Box::new(Class { negated: false, items: vec![self.category(c == 'P')?], ci: false })),
            '1'..='9' => {
                let mut n = c.to_digit(10).expect("digit") as usize;
                while let Some(d) = self.peek().and_then(|d| d.to_digit(10)) {
                    n = n * 10 + d as usize;
                    self.i += 1;
                }
                Node::BackRef(n, self.f.ci)
            }
            c => {
                if let Some(item) = self.escape_class(c) {
                    Node::Class(Box::new(Class { negated: false, items: vec![item], ci: false }))
                } else {
                    Node::Char(self.escape_char(c)?, self.f.ci)
                }
            }
        })
    }

    fn parse_class(&mut self) -> Result<Node, ParseError> {
        let negated = self.eat('^');
        let mut items = Vec::new();
        let mut first = true;
        loop {
            let Some(c) = self.peek() else { return Err(ParseError("Unterminated [] set.".into())) };
            self.i += 1;
            if c == ']' && !first {
                break;
            }
            first = false;
            if c == '[' && self.peek() == Some(':') {
                return Err(unsupported("POSIX classes"));
            }
            if c == '-' && self.peek() == Some('[') {
                return Err(unsupported("character class subtraction"));
            }
            let lo = if c == '\\' {
                let Some(e) = self.peek() else { return Err(ParseError("Unterminated [] set.".into())) };
                self.i += 1;
                if e == 'p' || e == 'P' {
                    items.push(self.category(e == 'P')?);
                    continue;
                }
                if let Some(item) = self.escape_class(e) {
                    items.push(item);
                    continue;
                }
                if e == 'b' {
                    '\u{8}'
                } else {
                    self.escape_char(e)?
                }
            } else {
                c
            };
            // a range `lo-hi`
            if self.peek() == Some('-') && self.c.get(self.i + 1).is_some_and(|n| *n != ']') {
                if self.c[self.i + 1] == '[' {
                    return Err(unsupported("character class subtraction"));
                }
                self.i += 1;
                let h = self.peek().expect("checked");
                self.i += 1;
                let hi = if h == '\\' {
                    let e = self.peek().ok_or_else(|| ParseError("Unterminated [] set.".into()))?;
                    self.i += 1;
                    self.escape_char(e)?
                } else {
                    h
                };
                if hi < lo {
                    return Err(ParseError("[x-y] range in reverse order.".into()));
                }
                items.push(Item::Range(lo, hi));
            } else {
                items.push(Item::Range(lo, lo));
            }
        }
        Ok(Node::Class(Box::new(Class { negated, items, ci: self.f.ci })))
    }
}

/// Parses a .NET pattern with `RegexOptions` bits.
pub fn compile(pattern: &str, options: i32) -> Result<Program, ParseError> {
    if options & RIGHT_TO_LEFT != 0 {
        return Err(unsupported("RegexOptions.RightToLeft"));
    }
    let mut p = Parser {
        c: pattern.chars().collect(),
        i: 0,
        f: Flags { ci: options & IGNORE_CASE != 0, multi: options & MULTILINE != 0, single: options & SINGLELINE != 0, explicit: options & EXPLICIT_CAPTURE != 0, ignore_ws: options & IGNORE_PATTERN_WHITESPACE != 0 },
        groups: Vec::new(),
        _p: std::marker::PhantomData,
    };
    let node = p.parse_alt()?;
    if p.i < p.c.len() {
        return Err(ParseError(if p.c[p.i] == ')' { "Too many )'s.".into() } else { "Unexpected character".into() }));
    }
    // .NET numbering: unnamed groups first, in order, then named groups in order
    let mut number_of = vec![0usize; p.groups.len()];
    let mut next = 1;
    for (i, g) in p.groups.iter().enumerate() {
        if g.is_none() {
            number_of[i] = next;
            next += 1;
        }
    }
    for (i, g) in p.groups.iter().enumerate() {
        if g.is_some() {
            number_of[i] = next;
            next += 1;
        }
    }
    let mut names = vec![None; next];
    for (i, g) in p.groups.iter().enumerate() {
        if let Some(n) = g {
            names[number_of[i]] = Some(n.clone());
        }
    }
    Ok(Program { node, number_of, names, group_count: next })
}

// ------------------------------------------------------------------------------------------------ matcher

fn is_word(c: char) -> bool {
    c == '_' || c.is_alphanumeric() || matches!(c, '\u{203f}' | '\u{2040}' | '\u{2054}' | '\u{fe33}' | '\u{fe34}' | '\u{fe4d}'..='\u{fe4f}' | '\u{ff3f}')
}
fn is_space(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | '\u{85}') || c.is_whitespace()
}

fn fold(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

impl Item {
    fn matches(&self, c: char, ci: bool) -> bool {
        match self {
            Item::Range(lo, hi) => {
                if (*lo..=*hi).contains(&c) {
                    return true;
                }
                ci && {
                    let (l, u) = (fold(c), c.to_uppercase().next().unwrap_or(c));
                    (*lo..=*hi).contains(&l) || (*lo..=*hi).contains(&u)
                }
            }
            Item::Word(p) => is_word(c) == *p,
            Item::Digit(p) => is_nd(c) == *p,
            Item::Space(p) => is_space(c) == *p,
            Item::Cat(k, p) => {
                (match k {
                    'L' => c.is_alphabetic(),
                    'u' => c.is_uppercase(),
                    'l' => c.is_lowercase(),
                    'N' => c.is_numeric(),
                    'd' => is_nd(c),
                    'P' => c.is_ascii_punctuation() && !matches!(c, '$' | '+' | '<' | '=' | '>' | '^' | '`' | '|' | '~') || matches!(c, '\u{a1}' | '\u{a7}' | '\u{ab}' | '\u{b6}' | '\u{b7}' | '\u{bb}' | '\u{bf}' | '\u{2010}'..='\u{2027}' | '\u{3001}'..='\u{3003}'),
                    'Z' => c == ' ' || c == '\u{a0}' || ('\u{2000}'..='\u{200a}').contains(&c) || matches!(c, '\u{1680}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}'),
                    'z' => c == ' ' || c == '\u{a0}' || ('\u{2000}'..='\u{200a}').contains(&c) || matches!(c, '\u{1680}' | '\u{202f}' | '\u{205f}' | '\u{3000}'),
                    'S' => matches!(c, '$' | '+' | '<' | '=' | '>' | '^' | '`' | '|' | '~' | '\u{a2}'..='\u{a9}' | '\u{ac}' | '\u{ae}'..='\u{b1}' | '\u{b4}' | '\u{d7}' | '\u{f7}'),
                    _ => false,
                }) == *p
            }
        }
    }
}

/// `\p{Nd}`: decimal digits (ASCII and the common Unicode digit blocks).
fn is_nd(c: char) -> bool {
    c.is_ascii_digit() || (c.is_numeric() && c.to_digit(10).is_none() && matches!(c, '\u{660}'..='\u{669}' | '\u{6f0}'..='\u{6f9}' | '\u{966}'..='\u{96f}' | '\u{ff10}'..='\u{ff19}'))
}

impl Class {
    fn matches(&self, c: char) -> bool {
        self.items.iter().any(|i| i.matches(c, self.ci)) != self.negated
    }
}

pub type Caps = Vec<Option<(usize, usize)>>;

struct Ctx<'a> {
    s: &'a [char],
    prog: &'a Program,
    steps: std::cell::Cell<u64>,
}

const STEP_LIMIT: u64 = 2_000_000_000;

impl Program {
    /// First match at or after `start` (char index): `caps[0]` is the whole match.
    pub fn find_at(&self, s: &[char], start: usize) -> Option<Caps> {
        let ctx = Ctx { s, prog: self, steps: std::cell::Cell::new(0) };
        for from in start..=s.len() {
            let mut caps: Caps = vec![None; self.group_count];
            let mut end = None;
            if ctx.run(&self.node, from, &mut caps, &mut |p, _| {
                end = Some(p);
                true
            }) {
                caps[0] = Some((from, end.expect("end set")));
                return Some(caps);
            }
        }
        None
    }
}

type K<'k> = &'k mut dyn FnMut(usize, &mut Caps) -> bool;

impl Ctx<'_> {
    fn one(&self, n: &Node, pos: usize) -> Option<bool> {
        // single-character nodes: does it match the char at pos
        let c = self.s.get(pos).copied();
        match n {
            Node::Char(x, ci) => Some(c.is_some_and(|c| c == *x || (*ci && fold(c) == fold(*x)))),
            Node::Any(single) => Some(c.is_some_and(|c| *single || c != '\n')),
            Node::Class(cl) => Some(c.is_some_and(|c| cl.matches(c))),
            _ => None,
        }
    }
    fn is_single(n: &Node) -> bool {
        matches!(n, Node::Char(..) | Node::Any(_) | Node::Class(_))
    }

    fn width(n: &Node) -> (usize, Option<usize>) {
        match n {
            Node::Empty | Node::LineStart(_) | Node::LineEnd(_) | Node::TextStart | Node::TextEnd | Node::TextEndZ | Node::WordB(_) | Node::Look { .. } => (0, Some(0)),
            Node::Char(..) | Node::Any(_) | Node::Class(_) => (1, Some(1)),
            Node::Seq(v) => v.iter().fold((0, Some(0)), |(a, b), n| {
                let (x, y) = Self::width(n);
                (a + x, b.zip(y).map(|(b, y)| b + y))
            }),
            Node::Alt(v) => {
                let w: Vec<_> = v.iter().map(Self::width).collect();
                (w.iter().map(|x| x.0).min().unwrap_or(0), w.iter().try_fold(0usize, |m, x| x.1.map(|y| m.max(y))))
            }
            Node::Group(n, _) | Node::NonCapture(n) | Node::Atomic(n) => Self::width(n),
            Node::Repeat { node, min, max, .. } => {
                let (a, b) = Self::width(node);
                (a * *min as usize, if *max == u32::MAX { if b == Some(0) { Some(0) } else { None } } else { b.map(|b| b * *max as usize) })
            }
            Node::BackRef(..) | Node::NamedBackRef(..) => (0, None),
        }
    }

    fn run(&self, n: &Node, pos: usize, caps: &mut Caps, k: K) -> bool {
        self.steps.set(self.steps.get() + 1);
        if self.steps.get() > STEP_LIMIT {
            // the original has a match timeout setting; the default is infinite, a runaway pattern is a bug here
            panic!("regex engine: step limit exceeded (catastrophic backtracking)");
        }
        match n {
            Node::Empty => k(pos, caps),
            Node::Char(..) | Node::Any(_) | Node::Class(_) => self.one(n, pos) == Some(true) && k(pos + 1, caps),
            Node::LineStart(multi) => (pos == 0 || (*multi && self.s[pos - 1] == '\n')) && k(pos, caps),
            Node::LineEnd(multi) => (pos == self.s.len() || (pos + 1 == self.s.len() && self.s[pos] == '\n') || (*multi && self.s[pos] == '\n')) && k(pos, caps),
            Node::TextStart => pos == 0 && k(pos, caps),
            Node::TextEnd => pos == self.s.len() && k(pos, caps),
            Node::TextEndZ => (pos == self.s.len() || (pos + 1 == self.s.len() && self.s[pos] == '\n')) && k(pos, caps),
            Node::WordB(want) => {
                let a = pos > 0 && is_word(self.s[pos - 1]);
                let b = pos < self.s.len() && is_word(self.s[pos]);
                ((a != b) == *want) && k(pos, caps)
            }
            Node::Seq(v) => self.seq(v, 0, pos, caps, k),
            Node::Alt(v) => {
                for a in v {
                    if self.run(a, pos, caps, k) {
                        return true;
                    }
                }
                false
            }
            Node::NonCapture(x) => self.run(x, pos, caps, k),
            Node::Group(x, idx) => {
                let num = self.prog.number_of[*idx];
                self.run(x, pos, caps, &mut |p, caps: &mut Caps| {
                    let old = caps[num];
                    caps[num] = Some((pos, p));
                    if k(p, caps) {
                        true
                    } else {
                        caps[num] = old;
                        false
                    }
                })
            }
            Node::Atomic(x) => {
                let mut end = None;
                if self.run(x, pos, caps, &mut |p, _| {
                    end = Some(p);
                    true
                }) {
                    k(end.expect("end"), caps)
                } else {
                    false
                }
            }
            Node::Look { node, ahead: true, negate } => {
                let saved = caps.clone();
                let r = self.run(node, pos, caps, &mut |_, _| true);
                if *negate {
                    *caps = saved;
                    !r && k(pos, caps)
                } else if r {
                    if k(pos, caps) {
                        true
                    } else {
                        *caps = saved;
                        false
                    }
                } else {
                    *caps = saved;
                    false
                }
            }
            Node::Look { node, ahead: false, negate } => {
                let (min, max) = Self::width(node);
                let saved = caps.clone();
                let lo = max.map_or(0, |m| pos.saturating_sub(m));
                let mut found = false;
                if pos >= min {
                    // the nearest start first
                    for start in (lo..=pos - min).rev() {
                        if self.run(node, start, caps, &mut |p, _| p == pos) {
                            found = true;
                            break;
                        }
                    }
                }
                if *negate {
                    *caps = saved;
                    !found && k(pos, caps)
                } else if found {
                    if k(pos, caps) {
                        true
                    } else {
                        *caps = saved;
                        false
                    }
                } else {
                    *caps = saved;
                    false
                }
            }
            Node::BackRef(num, ci) => {
                let num = *num;
                self.backref(num, *ci, pos, caps, k)
            }
            Node::NamedBackRef(name, ci) => match self.prog.names.iter().position(|n| n.as_deref() == Some(name.as_str())) {
                Some(num) => self.backref(num, *ci, pos, caps, k),
                None => false,
            },
            Node::Repeat { node, min, max, greedy } => {
                if Self::is_single(node) {
                    // iterative: how many times can it match in a row
                    let limit = if *max == u32::MAX { usize::MAX } else { *max as usize };
                    let mut n_match = 0;
                    while n_match < limit && self.one(node, pos + n_match) == Some(true) {
                        n_match += 1;
                    }
                    if n_match < *min as usize {
                        return false;
                    }
                    if *greedy {
                        for cnt in (*min as usize..=n_match).rev() {
                            if k(pos + cnt, caps) {
                                return true;
                            }
                        }
                    } else {
                        for cnt in *min as usize..=n_match {
                            if k(pos + cnt, caps) {
                                return true;
                            }
                        }
                    }
                    false
                } else {
                    self.rep(node, *min, *max, *greedy, 0, pos, caps, k)
                }
            }
        }
    }

    fn backref(&self, num: usize, ci: bool, pos: usize, caps: &mut Caps, k: K) -> bool {
        match caps.get(num).copied().flatten() {
            Some((a, b)) => {
                let len = b - a;
                if pos + len > self.s.len() {
                    return false;
                }
                let same = (0..len).all(|i| {
                    let (x, y) = (self.s[a + i], self.s[pos + i]);
                    x == y || (ci && fold(x) == fold(y))
                });
                same && k(pos + len, caps)
            }
            // an unset group never matches in .NET
            None => false,
        }
    }

    fn seq(&self, v: &[Node], i: usize, pos: usize, caps: &mut Caps, k: K) -> bool {
        if i == v.len() {
            return k(pos, caps);
        }
        let (first, _) = (&v[i], ());
        self.run(first, pos, caps, &mut |p, caps: &mut Caps| self.seq_dyn(v, i + 1, p, caps, k))
    }
    // helper that re-borrows self through a raw pointer-free path: closures capture `self` mutably above, so the
    // recursion goes through this function with the same receiver
    fn seq_dyn(&self, v: &[Node], i: usize, pos: usize, caps: &mut Caps, k: K) -> bool {
        self.seq(v, i, pos, caps, k)
    }

    #[allow(clippy::too_many_arguments)]
    fn rep(&self, node: &Node, min: u32, max: u32, greedy: bool, count: u32, pos: usize, caps: &mut Caps, k: K) -> bool {
        if greedy {
            if count < max {
                let ok = self.run(node, pos, caps, &mut |p, caps: &mut Caps| {
                    // an iteration that consumed nothing ends the loop (it would repeat forever)
                    if p == pos && count >= min {
                        return false;
                    }
                    self.rep_dyn(node, min, max, greedy, count + 1, p, caps, k)
                });
                if ok {
                    return true;
                }
            }
            count >= min && k(pos, caps)
        } else {
            if count >= min && k(pos, caps) {
                return true;
            }
            if count < max {
                return self.run(node, pos, caps, &mut |p, caps: &mut Caps| {
                    if p == pos && count >= min {
                        return false;
                    }
                    self.rep_dyn(node, min, max, greedy, count + 1, p, caps, k)
                });
            }
            false
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn rep_dyn(&self, node: &Node, min: u32, max: u32, greedy: bool, count: u32, pos: usize, caps: &mut Caps, k: K) -> bool {
        self.rep(node, min, max, greedy, count, pos, caps, k)
    }
}

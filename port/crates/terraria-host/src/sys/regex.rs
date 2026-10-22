//! `System.Text.RegularExpressions` (`Regex`, `Match`, `Group`, `Capture`, `MatchCollection`) over the backtracking
//! engine of `regex_engine.rs`, which follows .NET's semantics (leftmost, backtracking order, named groups numbered
//! after unnamed ones, look-behind, lazy quantifiers). Before 2026-10 this module sat on the `regex` crate, which has
//! no look-around, so `ChatManager.Regexes.Format` (`(?<!\\)\[(?<tag>[a-zA-Z]{1,10})(\/(?<options>[^:]+))?:(?<text>.+?)(?<!\\)\]`)
//! could not be constructed.
//!
//! The game's patterns (decompile grep, 2026-10): `[^a-z0-9]+`, `[^a-zA-Z0-9 ]+` (seeds), `(\\*)"` and `(\\+)$`
//! (command-line quoting), `^[./]+` (zip paths), `{(\?(?:!)?)?([a-zA-Z][\w\.]*)}` and `{(\?!?)?([a-zA-Z][\w\.]*)}`
//! (text substitution), `{\$(\w+\.\w+)}` (localisation references), `^[0-9a-f]+$`, `\r\n|\r|\n` (line split),
//! the file-name pattern of `FileUtilities` (named groups, lazy quantifiers) and `ChatManager.Regexes.Format`.
//! tests/dotnet/regex_probe.ps1 runs all of them (plus chat-like inputs) through real .NET and the expected matches
//! are compared here, group by group, with index and length.
//!
//! Replacement strings follow .NET: `$1`, `${name}`, `$$`, `$&`, `` $` ``, `$'`, `$+`, `$_`; a `$` that does not
//! start a valid reference is literal. Indices and lengths are in UTF-16 units, as in C#.

use terraria_gen_rt::rt::{self, Arr, Delegate, Gc, Marker, Str, Trace};

use super::regex_engine::{self as eng, Caps, Program};
use crate::gfx::states::throw;

/// `RegexOptions`
pub const IGNORE_CASE: i32 = 1;
pub const MULTILINE: i32 = 2;
pub const EXPLICIT_CAPTURE: i32 = 4;
pub const COMPILED: i32 = 8;
pub const SINGLELINE: i32 = 16;
pub const IGNORE_PATTERN_WHITESPACE: i32 = 32;

pub struct Regex {
    pub pattern: String,
    pub options: i32,
    prog: Program,
}
// SAFETY: no handles inside.
unsafe impl Trace for Regex {
    const NEEDS_TRACE: bool = false;
    unsafe fn trace(&self, _m: &mut Marker) {}
}

/// `Match` and `Group` / `Capture` (one type: a group is a match without groups of its own).
pub struct Match {
    pub success: bool,
    pub value: Str,
    /// UTF-16 index in the input
    pub index: i32,
    pub length: i32,
    pub groups: Vec<Gc<Match>>,
    /// `names[i]`: the name of group `i`, `None` for unnamed groups
    pub names: Vec<Option<String>>,
}
// SAFETY: every handle is reported.
unsafe impl Trace for Match {
    unsafe fn trace(&self, m: &mut Marker) {
        self.value.trace(m);
        for g in &self.groups {
            m.mark(*g);
        }
    }
}

/// The input as chars and the UTF-16 index of each char boundary.
struct Input {
    chars: Vec<char>,
    u16_at: Vec<usize>,
}
impl Input {
    fn new(s: &str) -> Input {
        let chars: Vec<char> = s.chars().collect();
        let mut u16_at = Vec::with_capacity(chars.len() + 1);
        let mut n = 0;
        for c in &chars {
            u16_at.push(n);
            n += c.len_utf16();
        }
        u16_at.push(n);
        Input { chars, u16_at }
    }
    fn text(&self, a: usize, b: usize) -> String {
        self.chars[a..b].iter().collect()
    }
}

impl Match {
    fn failed() -> Gc<Match> {
        rt::alloc_init(Match { success: false, value: Str::lit(""), index: 0, length: 0, groups: Vec::new(), names: Vec::new() })
    }
    fn group(inp: &Input, span: Option<(usize, usize)>) -> Gc<Match> {
        match span {
            Some((a, b)) => rt::alloc_init(Match { success: true, value: Str::new(&inp.text(a, b)), index: inp.u16_at[a] as i32, length: (inp.u16_at[b] - inp.u16_at[a]) as i32, groups: Vec::new(), names: Vec::new() }),
            None => Self::failed(),
        }
    }
    fn from_caps(prog: &Program, inp: &Input, caps: &Caps) -> Gc<Match> {
        let whole = Self::group(inp, caps[0]);
        // SAFETY: fresh object.
        let w = unsafe { &mut *whole.ptr() };
        // Groups[0] is the match itself
        w.groups.push(whole);
        for c in caps.iter().skip(1) {
            w.groups.push(Self::group(inp, *c));
        }
        w.names = prog.names.clone();
        whole
    }
    pub fn success(&self) -> bool {
        self.success
    }
    /// `Value` / `ToString()`
    pub fn value(&self) -> Str {
        self.value
    }
    pub fn index(&self) -> i32 {
        self.index
    }
    pub fn length(&self) -> i32 {
        self.length
    }
    /// `Groups[int]`: a failed (empty) group for a number that does not exist, as .NET returns `Group.Empty`.
    pub fn group_at(&self, i: i32) -> Gc<Match> {
        match usize::try_from(i).ok().and_then(|i| self.groups.get(i)) {
            Some(g) => *g,
            None => Self::failed(),
        }
    }
    /// `Groups[string]`
    pub fn group_named(&self, name: Str) -> Gc<Match> {
        let n = name.as_str();
        match self.names.iter().position(|x| x.as_deref() == Some(n)) {
            Some(i) => self.groups[i],
            None => match n.parse::<i32>() {
                Ok(i) => self.group_at(i),
                Err(_) => Self::failed(),
            },
        }
    }
    /// `Groups.Count`
    pub fn group_count(&self) -> i32 {
        self.groups.len() as i32
    }
}

/// `.NET` replacement pattern expansion for one match (`caps` are char spans into `inp`).
fn expand(replacement: &str, inp: &Input, caps: &Caps, names: &[Option<String>]) -> String {
    let c: Vec<char> = replacement.chars().collect();
    let text = |n: usize| caps.get(n).copied().flatten().map_or(String::new(), |(a, b)| inp.text(a, b));
    let mut out = String::new();
    let mut i = 0;
    while i < c.len() {
        if c[i] != '$' || i + 1 >= c.len() {
            out.push(c[i]);
            i += 1;
            continue;
        }
        match c[i + 1] {
            '$' => {
                out.push('$');
                i += 2;
            }
            '&' => {
                out.push_str(&text(0));
                i += 2;
            }
            '`' => {
                out.push_str(&inp.text(0, caps[0].expect("match").0));
                i += 2;
            }
            '\'' => {
                out.push_str(&inp.text(caps[0].expect("match").1, inp.chars.len()));
                i += 2;
            }
            '_' => {
                out.push_str(&inp.text(0, inp.chars.len()));
                i += 2;
            }
            '+' => {
                // the last group that matched
                let last = (1..caps.len()).rev().find(|n| caps[*n].is_some());
                out.push_str(&last.map_or(String::new(), text));
                i += 2;
            }
            d if d.is_ascii_digit() => {
                // the longest number that is a group of the pattern
                let mut j = i + 1;
                let mut n = 0usize;
                let mut best: Option<(usize, usize)> = None;
                while j < c.len() && c[j].is_ascii_digit() {
                    n = n * 10 + c[j].to_digit(10).expect("digit") as usize;
                    j += 1;
                    if n < caps.len() {
                        best = Some((n, j));
                    }
                }
                match best {
                    Some((n, end)) => {
                        out.push_str(&text(n));
                        i = end;
                    }
                    None => {
                        out.push('$');
                        i += 1;
                    }
                }
            }
            '{' => {
                let rest: String = c[i + 2..].iter().collect();
                match rest.split_once('}') {
                    Some((name, _)) => {
                        let idx = names.iter().position(|x| x.as_deref() == Some(name)).or_else(|| name.parse::<usize>().ok().filter(|n| *n < caps.len()));
                        match idx {
                            Some(n) => {
                                out.push_str(&text(n));
                                i += 3 + name.chars().count();
                            }
                            None => {
                                out.push('$');
                                i += 1;
                            }
                        }
                    }
                    None => {
                        out.push('$');
                        i += 1;
                    }
                }
            }
            _ => {
                out.push('$');
                i += 1;
            }
        }
    }
    out
}

fn compile(pattern: Str, options: i32) -> Program {
    let Some(p) = pattern.get() else { throw("System.ArgumentNullException", "pattern") };
    match eng::compile(p, options) {
        Ok(prog) => prog,
        Err(e) if e.0.starts_with("Unsupported:") => rt::throw(rt::exc("System.NotSupportedException", Str::new(&format!("Regex pattern \"{p}\": {}", e.0)))),
        Err(e) => rt::throw(rt::exc("System.ArgumentException", Str::new(&format!("parsing \"{p}\" - {}", e.0)))),
    }
}

impl Regex {
    /// `new Regex(pattern)` / `new Regex(pattern, options)`
    pub fn new_gc(pattern: Str, options: i32) -> Gc<Regex> {
        let prog = compile(pattern, options);
        rt::alloc_init(Regex { pattern: pattern.as_str().to_owned(), options, prog })
    }
    fn input(input: Str) -> Input {
        match input.get() {
            Some(s) => Input::new(s),
            None => throw("System.ArgumentNullException", "input"),
        }
    }
    /// Every match, left to right: the search resumes at the end of a match, one char later after an empty one.
    fn all(&self, inp: &Input) -> Vec<Caps> {
        let mut out = Vec::new();
        let mut pos = 0;
        while pos <= inp.chars.len() {
            let Some(caps) = self.prog.find_at(&inp.chars, pos) else { break };
            let (a, b) = caps[0].expect("whole match");
            pos = if b == a { b + 1 } else { b };
            out.push(caps);
        }
        out
    }
    pub fn is_match(&self, input: Str) -> bool {
        self.prog.find_at(&Self::input(input).chars, 0).is_some()
    }
    /// `Match(input)`: the first match, or a failed match.
    pub fn match_(&self, input: Str) -> Gc<Match> {
        let inp = Self::input(input);
        match self.prog.find_at(&inp.chars, 0) {
            Some(c) => Match::from_caps(&self.prog, &inp, &c),
            None => Match::failed(),
        }
    }
    /// `Matches(input)` as an array, in order.
    pub fn matches(&self, input: Str) -> Arr<Gc<Match>> {
        let inp = Self::input(input);
        let v: Vec<Gc<Match>> = self.all(&inp).iter().map(|c| Match::from_caps(&self.prog, &inp, c)).collect();
        Arr::from_slice(&v)
    }
    /// `Replace(input, replacement)`
    pub fn replace(&self, input: Str, replacement: Str) -> Str {
        let inp = Self::input(input);
        let Some(r) = replacement.get() else { throw("System.ArgumentNullException", "replacement") };
        let mut out = String::new();
        let mut last = 0;
        for caps in self.all(&inp) {
            let (a, b) = caps[0].expect("match");
            out.push_str(&inp.text(last, a));
            out.push_str(&expand(r, &inp, &caps, &self.prog.names));
            last = b;
        }
        out.push_str(&inp.text(last, inp.chars.len()));
        Str::new(&out)
    }
    /// `Replace(input, MatchEvaluator)`: the evaluator (translated code) is called once per match, left to right.
    pub fn replace_with(&self, input: Str, evaluator: Delegate<dyn Fn(Gc<Match>) -> Str>) -> Str {
        let inp = Self::input(input);
        // the matches are found first; the evaluator runs with no borrow of this object's data in use
        let found: Vec<((usize, usize), Gc<Match>)> = self.all(&inp).iter().map(|c| (c[0].expect("match"), Match::from_caps(&self.prog, &inp, c))).collect();
        let mut out = String::new();
        let mut last = 0;
        for ((a, b), m) in found {
            out.push_str(&inp.text(last, a));
            let r = evaluator.invoke(|f| f(m));
            out.push_str(r.get().unwrap_or(""));
            last = b;
        }
        out.push_str(&inp.text(last, inp.chars.len()));
        Str::new(&out)
    }
    /// `Split(input)`: the text between matches; captured groups are inserted after each piece, as .NET does.
    pub fn split(&self, input: Str) -> Arr<Str> {
        let inp = Self::input(input);
        let mut parts: Vec<Str> = Vec::new();
        let mut last = 0;
        for caps in self.all(&inp) {
            let (a, b) = caps[0].expect("match");
            parts.push(Str::new(&inp.text(last, a)));
            for c in caps.iter().skip(1).flatten() {
                parts.push(Str::new(&inp.text(c.0, c.1)));
            }
            last = b;
        }
        parts.push(Str::new(&inp.text(last, inp.chars.len())));
        Arr::from_slice(&parts)
    }
}

/// `Regex.Replace(input, pattern, replacement)` (static)
pub fn regex_replace(input: Str, pattern: Str, replacement: Str) -> Str {
    Regex { pattern: String::new(), options: 0, prog: compile(pattern, 0) }.replace(input, replacement)
}
/// `Regex.IsMatch(input, pattern)` (static)
pub fn regex_is_match(input: Str, pattern: Str) -> bool {
    Regex { pattern: String::new(), options: 0, prog: compile(pattern, 0) }.is_match(input)
}
/// `Regex.IsMatch(input, pattern, options)` (static)
pub fn regex_is_match_opt(input: Str, pattern: Str, options: i32) -> bool {
    Regex { pattern: String::new(), options, prog: compile(pattern, options) }.is_match(input)
}
/// `Regex.Match(input, pattern)` (static)
pub fn regex_match(input: Str, pattern: Str) -> Gc<Match> {
    Regex { pattern: String::new(), options: 0, prog: compile(pattern, 0) }.match_(input)
}
/// `Regex.Matches(input, pattern)` (static)
pub fn regex_matches(input: Str, pattern: Str) -> Arr<Gc<Match>> {
    Regex { pattern: String::new(), options: 0, prog: compile(pattern, 0) }.matches(input)
}
/// `Regex.Split(input, pattern)` (static)
pub fn regex_split(input: Str, pattern: Str) -> Arr<Str> {
    Regex { pattern: String::new(), options: 0, prog: compile(pattern, 0) }.split(input)
}
/// `Regex.Escape(text)`: backslash before `\ * + ? | { [ ( ) ^ $ . #` and white space (as `\t`, `\n`, `\f`, `\r`, `\ `).
pub fn regex_escape(text: Str) -> Str {
    let Some(s) = text.get() else { throw("System.ArgumentNullException", "str") };
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '\\' | '*' | '+' | '?' | '|' | '{' | '[' | '(' | ')' | '^' | '$' | '.' | '#' => {
                out.push('\\');
                out.push(c);
            }
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{c}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            ' ' => out.push_str("\\ "),
            c => out.push(c),
        }
    }
    Str::new(&out)
}

#[cfg(test)]
mod tests;

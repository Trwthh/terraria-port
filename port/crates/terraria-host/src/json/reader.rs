//! `JsonTextReader` of Newtonsoft.Json 10: a pull parser with the token types, tolerances and error messages of the
//! original (checked against the game's own dll, tests/dotnet/json_probe.ps1).
//!
//! Tolerances reproduced: comments (`/* */`, `//`) are tokens; single-quoted strings; unquoted property names;
//! a trailing comma before `]` / `}`; `[,1]` gives an `Undefined` token for the missing value; `NaN`, `Infinity`,
//! `-Infinity`, `undefined`; hexadecimal integers (a number text starting with `0` and not followed by `.`, `e`,
//! `E` goes through `Convert.ToInt64(text, 16)`, so `010` is 16 and `00.5` is a `FormatException`); integers that
//! overflow `long` become `BigInteger`; ISO 8601 and `/Date(ms)/` strings become `Date` tokens
//! (`DateParseHandling.DateTime`, the default). `new Name(...)` constructors are tokens.
//!
//! Positions are counted in `char`s (the original counts UTF-16 units: they differ only after astral characters).

use super::number::{self, Int};

/// `JsonToken` values.
pub const NONE: i32 = 0;
pub const START_OBJECT: i32 = 1;
pub const START_ARRAY: i32 = 2;
pub const START_CONSTRUCTOR: i32 = 3;
pub const PROPERTY_NAME: i32 = 4;
pub const COMMENT: i32 = 5;
pub const RAW: i32 = 6;
pub const INTEGER: i32 = 7;
pub const FLOAT: i32 = 8;
pub const STRING: i32 = 9;
pub const BOOLEAN: i32 = 10;
pub const NULL: i32 = 11;
pub const UNDEFINED: i32 = 12;
pub const END_OBJECT: i32 = 13;
pub const END_ARRAY: i32 = 14;
pub const END_CONSTRUCTOR: i32 = 15;
pub const DATE: i32 = 16;
pub const BYTES: i32 = 17;

/// A date value: `DateTime` ticks (since 0001-01-01) and how the text spelled the zone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DateVal {
    pub ticks: i64,
    pub zone: Zone,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Zone {
    /// `DateTimeKind.Unspecified`: no zone in the text
    Unspecified,
    /// `DateTimeKind.Utc`: `Z`
    Utc,
    /// an explicit offset in minutes (the original converts to the machine's local time; see json/mod.rs)
    Offset(i32),
}

/// A scalar token value.
#[derive(Clone, Debug, PartialEq)]
pub enum Scalar {
    Null,
    Undefined,
    Bool(bool),
    Int(i64),
    /// `BigInteger`, decimal digits
    Big(String),
    Float(f64),
    /// `float` (only produced by `JsonWriter`-side conversions, never by the reader)
    Float32(f32),
    Str(String),
    Date(DateVal),
    Bytes(Vec<u8>),
    Comment(String),
    Raw(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    None,
    StartObject,
    StartArray,
    StartConstructor(String),
    PropertyName(String),
    EndObject,
    EndArray,
    EndConstructor,
    Value(Scalar),
}

impl Token {
    /// `JsonToken` number.
    pub fn kind(&self) -> i32 {
        match self {
            Token::None => NONE,
            Token::StartObject => START_OBJECT,
            Token::StartArray => START_ARRAY,
            Token::StartConstructor(_) => START_CONSTRUCTOR,
            Token::PropertyName(_) => PROPERTY_NAME,
            Token::EndObject => END_OBJECT,
            Token::EndArray => END_ARRAY,
            Token::EndConstructor => END_CONSTRUCTOR,
            Token::Value(s) => match s {
                Scalar::Null => NULL,
                Scalar::Undefined => UNDEFINED,
                Scalar::Bool(_) => BOOLEAN,
                Scalar::Int(_) | Scalar::Big(_) => INTEGER,
                Scalar::Float(_) | Scalar::Float32(_) => FLOAT,
                Scalar::Str(_) => STRING,
                Scalar::Date(_) => DATE,
                Scalar::Bytes(_) => BYTES,
                Scalar::Comment(_) => COMMENT,
                Scalar::Raw(_) => RAW,
            },
        }
    }
}

/// Which .NET exception a failure maps to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ErrKind {
    /// `Newtonsoft.Json.JsonReaderException`
    Reader,
    /// `System.FormatException`
    Format,
}
#[derive(Clone, Debug, PartialEq)]
pub struct JsonError {
    pub kind: ErrKind,
    pub message: String,
}

#[derive(Clone, Debug)]
enum Frame {
    Object(Option<String>),
    Array(i64),
    Constructor(i64),
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum St {
    Start,
    ObjectStart,
    /// after a property name: a value follows
    Property,
    ObjectPost,
    ArrayStart,
    ArrayPost,
    ConstructorStart,
    ConstructorPost,
    Finished,
}

pub struct Reader {
    s: Vec<char>,
    pos: usize,
    line: usize,
    line_start: usize,
    stack: Vec<Frame>,
    st: St,
    /// `DateParseHandling.DateTime` (the default)
    pub parse_dates: bool,
    pub token: Token,
    /// path of the last token read (`JsonReader.Path`)
    path: String,
}

fn is_ws(c: char) -> bool {
    c.is_whitespace()
}
fn is_sep(c: Option<char>) -> bool {
    match c {
        None => true,
        Some(c) => matches!(c, ',' | ']' | '}' | ')' | '/') || is_ws(c),
    }
}

impl Reader {
    pub fn new(text: &str) -> Reader {
        Reader { s: text.chars().collect(), pos: 0, line: 1, line_start: 0, stack: Vec::new(), st: St::Start, parse_dates: true, token: Token::None, path: String::new() }
    }

    fn peek(&self) -> Option<char> {
        self.s.get(self.pos).copied()
    }
    fn at(&self, off: usize) -> Option<char> {
        self.s.get(self.pos + off).copied()
    }
    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += 1;
        match c {
            '\n' => {
                self.line += 1;
                self.line_start = self.pos;
            }
            '\r' => {
                if self.peek() != Some('\n') {
                    self.line += 1;
                    self.line_start = self.pos;
                }
            }
            _ => {}
        }
        Some(c)
    }

    /// `JsonReader.Path` of the current token.
    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn line_number(&self) -> usize {
        if self.pos == 0 {
            0
        } else {
            self.line
        }
    }
    pub fn line_position(&self) -> usize {
        self.pos - self.line_start
    }
    /// `Depth`
    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    fn build_path(&self, pending_index: bool) -> String {
        let mut out = String::new();
        let n = self.stack.len();
        for (i, f) in self.stack.iter().enumerate() {
            match f {
                Frame::Object(Some(name)) => {
                    if !out.is_empty() {
                        out.push('.');
                    }
                    if name.chars().any(|c| matches!(c, ' ' | '.' | '[' | ']' | '\'' | '"') || c.is_control()) {
                        out.push_str(&format!("['{}']", name));
                    } else {
                        out.push_str(name);
                    }
                }
                Frame::Object(None) => {}
                Frame::Array(idx) | Frame::Constructor(idx) => {
                    let idx = if pending_index && i == n - 1 { idx + 1 } else { *idx };
                    if idx >= 0 {
                        out.push_str(&format!("[{idx}]"));
                    }
                }
            }
        }
        out
    }

    fn err(&self, message: &str, pending_index: bool) -> JsonError {
        let path = self.build_path(pending_index);
        JsonError { kind: ErrKind::Reader, message: format!("{message} Path '{path}', line {}, position {}.", self.line_number(), self.line_position()) }
    }
    fn err_unexpected(&self, what: &str, c: Option<char>) -> JsonError {
        self.err(&format!("{what}: {}.", c.unwrap_or('\0')), false)
    }

    /// Skips white space. Comments are returned by `read` as tokens, so this does not skip them.
    fn skip_ws(&mut self) {
        while let Some(c) = self.peek() {
            if is_ws(c) {
                self.bump();
            } else {
                break;
            }
        }
    }

    fn read_comment(&mut self) -> Result<Token, JsonError> {
        // at '/'
        self.bump();
        match self.peek() {
            Some('*') => {
                self.bump();
                let mut text = String::new();
                loop {
                    match self.bump() {
                        None => return Err(self.err("Unexpected end while parsing comment.", false)),
                        Some('*') if self.peek() == Some('/') => {
                            self.bump();
                            break;
                        }
                        Some(c) => text.push(c),
                    }
                }
                Ok(Token::Value(Scalar::Comment(text)))
            }
            Some('/') => {
                self.bump();
                let mut text = String::new();
                while let Some(c) = self.peek() {
                    if c == '\r' || c == '\n' {
                        break;
                    }
                    text.push(c);
                    self.bump();
                }
                Ok(Token::Value(Scalar::Comment(text)))
            }
            c => Err(self.err(&format!("Error parsing comment. Expected: *, got {}.", c.unwrap_or('\0')), false)),
        }
    }

    fn set(&mut self, t: Token) {
        self.token = t;
    }

    fn finish_value(&mut self) {
        self.st = match self.stack.last() {
            None => St::Finished,
            Some(Frame::Object(_)) => St::ObjectPost,
            Some(Frame::Array(_)) => St::ArrayPost,
            Some(Frame::Constructor(_)) => St::ConstructorPost,
        };
    }

    /// Advances the array index for a value starting in an array / constructor.
    fn count_item(&mut self) {
        if let Some(Frame::Array(i) | Frame::Constructor(i)) = self.stack.last_mut() {
            *i += 1;
        }
    }

    /// `Read()`: `Ok(false)` at the end of the content.
    pub fn read(&mut self) -> Result<bool, JsonError> {
        loop {
            self.skip_ws();
            // end of the content inside a structure: the loader reports it (`Unexpected end of content ...`)
            if self.peek().is_none() && !matches!(self.st, St::Start | St::Finished) {
                self.set(Token::None);
                return Ok(false);
            }
            match self.st {
                St::Finished => {
                    return match self.peek() {
                        None => {
                            self.set(Token::None);
                            Ok(false)
                        }
                        Some('/') => {
                            let t = self.read_comment()?;
                            self.set(t);
                            self.path = self.build_path(false);
                            Ok(true)
                        }
                        Some(c) => {
                            // the original names the whole offending character; the position is the character's
                            Err(self.err(&format!("Additional text encountered after finished reading JSON content: {c}."), false))
                        }
                    };
                }
                St::Start => {
                    if self.peek().is_none() {
                        self.set(Token::None);
                        return Ok(false);
                    }
                    return self.read_value(false);
                }
                St::Property => return self.read_value(false),
                St::ObjectStart => match self.peek() {
                    Some('}') => {
                        self.bump();
                        return self.end(Token::EndObject);
                    }
                    Some('/') => return self.comment(),
                    _ => return self.read_property(),
                },
                St::ObjectPost => match self.peek() {
                    Some('}') => {
                        self.bump();
                        return self.end(Token::EndObject);
                    }
                    Some(',') => {
                        self.bump();
                        self.skip_ws();
                        // comments between the comma and the next property
                        loop {
                            match self.peek() {
                                Some('/') => return self.comment_then(St::ObjectStart),
                                Some('}') => {
                                    self.bump();
                                    return self.end(Token::EndObject);
                                }
                                _ => return self.read_property(),
                            }
                        }
                    }
                    Some('/') => return self.comment(),
                    Some(c) => return Err(self.err(&format!("After parsing a value an unexpected character was encountered: {c}."), false)),
                    None => unreachable!("end of content is handled above"),
                },
                St::ArrayStart => match self.peek() {
                    Some(']') => {
                        self.bump();
                        return self.end(Token::EndArray);
                    }
                    Some('/') => return self.comment(),
                    Some(',') => {
                        // [,1]: a missing value
                        self.count_item();
                        self.set(Token::Value(Scalar::Undefined));
                        self.path = self.build_path(false);
                        self.st = St::ArrayPost;
                        return Ok(true);
                    }
                    _ => return self.read_value(true),
                },
                St::ArrayPost => match self.peek() {
                    Some(']') => {
                        self.bump();
                        return self.end(Token::EndArray);
                    }
                    Some(',') => {
                        self.bump();
                        self.skip_ws();
                        match self.peek() {
                            Some(']') => {
                                self.bump();
                                return self.end(Token::EndArray);
                            }
                            Some(',') => {
                                self.count_item();
                                self.set(Token::Value(Scalar::Undefined));
                                self.path = self.build_path(false);
                                self.st = St::ArrayPost;
                                return Ok(true);
                            }
                            Some('/') => return self.comment_then(St::ArrayStart),
                            _ => return self.read_value(true),
                        }
                    }
                    Some('/') => return self.comment(),
                    Some(c) => return Err(self.err(&format!("After parsing a value an unexpected character was encountered: {c}."), false)),
                    None => unreachable!("end of content is handled above"),
                },
                St::ConstructorStart | St::ConstructorPost => match self.peek() {
                    Some(')') => {
                        self.bump();
                        return self.end(Token::EndConstructor);
                    }
                    Some(',') if self.st == St::ConstructorPost => {
                        self.bump();
                        self.st = St::ConstructorStart;
                        continue;
                    }
                    Some('/') => return self.comment(),
                    None => unreachable!("end of content is handled above"),
                    _ if self.st == St::ConstructorStart => return self.read_value(true),
                    Some(c) => return Err(self.err(&format!("After parsing a value an unexpected character was encountered: {c}."), false)),
                },
            }
        }
    }

    /// A comment token; the state does not change.
    fn comment(&mut self) -> Result<bool, JsonError> {
        let t = self.read_comment()?;
        self.set(t);
        self.path = self.build_path(false);
        Ok(true)
    }
    /// A comment token after a comma; the state becomes `next` (the comma has been consumed).
    fn comment_then(&mut self, next: St) -> Result<bool, JsonError> {
        let t = self.read_comment()?;
        self.set(t);
        self.path = self.build_path(false);
        self.st = next;
        Ok(true)
    }

    fn end(&mut self, t: Token) -> Result<bool, JsonError> {
        self.stack.pop();
        self.set(t);
        self.path = self.build_path(false);
        self.finish_value();
        Ok(true)
    }

    fn read_property(&mut self) -> Result<bool, JsonError> {
        let name = match self.peek() {
            Some(q @ ('"' | '\'')) => {
                self.bump();
                self.read_string_body(q)?
            }
            Some(c) if c.is_alphanumeric() || c == '_' || c == '$' => {
                let mut n = String::new();
                while let Some(c) = self.peek() {
                    if c.is_alphanumeric() || c == '_' || c == '$' {
                        n.push(c);
                        self.bump();
                    } else if c == ':' || is_ws(c) {
                        break;
                    } else {
                        return Err(self.err(&format!("Invalid JavaScript property identifier character: {c}."), false));
                    }
                }
                n
            }
            c => return Err(self.err_unexpected("Invalid property identifier character", c)),
        };
        self.skip_ws();
        match self.peek() {
            Some(':') => {
                self.bump();
            }
            c => return Err(self.err(&format!("Invalid character after parsing property name. Expected ':' but got: {}.", c.unwrap_or('\0')), false)),
        }
        if let Some(Frame::Object(n)) = self.stack.last_mut() {
            *n = Some(name.clone());
        }
        self.set(Token::PropertyName(name));
        self.path = self.build_path(false);
        self.st = St::Property;
        Ok(true)
    }

    fn start(&mut self, t: Token, frame: Frame, st: St) -> Result<bool, JsonError> {
        self.stack.push(frame);
        self.set(t);
        self.path = self.build_path(false);
        self.st = st;
        Ok(true)
    }

    fn value_done(&mut self, t: Token) -> Result<bool, JsonError> {
        self.set(t);
        self.path = self.build_path(false);
        self.finish_value();
        Ok(true)
    }

    /// `in_array`: the value is an item of the innermost array / constructor (its index counts).
    fn read_value(&mut self, in_array: bool) -> Result<bool, JsonError> {
        let c = self.peek();
        let start_item = |this: &mut Reader| {
            if in_array {
                this.count_item();
            }
        };
        match c {
            Some('/') => self.comment(),
            Some('{') => {
                start_item(self);
                self.bump();
                self.start(Token::StartObject, Frame::Object(None), St::ObjectStart)
            }
            Some('[') => {
                start_item(self);
                self.bump();
                self.start(Token::StartArray, Frame::Array(-1), St::ArrayStart)
            }
            Some(q @ ('"' | '\'')) => {
                start_item(self);
                self.bump();
                let s = self.read_string_body(q)?;
                let v = if self.parse_dates { parse_date(&s).map(Scalar::Date).unwrap_or(Scalar::Str(s)) } else { Scalar::Str(s) };
                self.value_done(Token::Value(v))
            }
            Some('t') => {
                start_item(self);
                self.read_literal("true", "Error parsing boolean value.", Scalar::Bool(true))
            }
            Some('f') => {
                start_item(self);
                self.read_literal("false", "Error parsing boolean value.", Scalar::Bool(false))
            }
            Some('u') => {
                start_item(self);
                self.read_literal("undefined", "Error parsing undefined value.", Scalar::Undefined)
            }
            Some('N') => {
                start_item(self);
                self.read_literal("NaN", "Error parsing NaN value.", Scalar::Float(f64::NAN))
            }
            Some('I') => {
                start_item(self);
                self.read_literal("Infinity", "Error parsing Infinity value.", Scalar::Float(f64::INFINITY))
            }
            Some('n') => {
                start_item(self);
                if self.at(1) == Some('e') {
                    self.read_constructor()
                } else {
                    self.read_literal("null", "Error parsing null value.", Scalar::Null)
                }
            }
            Some('-') if self.at(1) == Some('I') => {
                start_item(self);
                self.bump();
                self.read_literal("Infinity", "Error parsing Infinity value.", Scalar::Float(f64::NEG_INFINITY))
            }
            Some(c) if c == '-' || c == '.' || c.is_ascii_digit() => self.read_number(in_array),
            Some(')') if matches!(self.stack.last(), Some(Frame::Constructor(_))) => {
                self.bump();
                self.end(Token::EndConstructor)
            }
            c => Err(self.err_unexpected("Unexpected character encountered while parsing value", c)),
        }
    }

    fn read_literal(&mut self, word: &str, error: &str, v: Scalar) -> Result<bool, JsonError> {
        // the characters are consumed up to the first one that cannot belong to the word
        let w: Vec<char> = word.chars().collect();
        let mut matched = 0;
        while matched < w.len() && self.peek() == Some(w[matched]) {
            self.bump();
            matched += 1;
        }
        if matched < w.len() || !is_sep(self.peek()) {
            // the original consumes the identifier characters it has examined; the message position is the same here
            while matched < w.len() && self.peek().is_some_and(|c| c.is_alphanumeric()) {
                self.bump();
                matched += 1;
            }
            return Err(self.err(error, false));
        }
        self.value_done(Token::Value(v))
    }

    fn read_constructor(&mut self) -> Result<bool, JsonError> {
        for w in "new".chars() {
            if self.peek() != Some(w) {
                return Err(self.err("Unexpected character while parsing constructor.", false));
            }
            self.bump();
        }
        self.skip_ws();
        let mut name = String::new();
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                name.push(c);
                self.bump();
            } else {
                break;
            }
        }
        self.skip_ws();
        if self.peek() != Some('(') {
            return Err(self.err_unexpected("Unexpected character while parsing constructor", self.peek()));
        }
        self.bump();
        self.start(Token::StartConstructor(name), Frame::Constructor(-1), St::ConstructorStart)
    }

    fn read_number(&mut self, in_array: bool) -> Result<bool, JsonError> {
        let mut text = String::new();
        loop {
            match self.peek() {
                Some(c) if c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '+' | '-') => {
                    text.push(c);
                    self.bump();
                }
                c if is_sep(c) => break,
                c => return Err(self.err_unexpected("Unexpected character encountered while parsing number", c)),
            }
        }
        // the item counts from here on: an invalid number is reported at its own index
        if in_array {
            self.count_item();
        }
        let bad = |this: &Reader| this.err(&format!("Input string '{text}' is not a valid number."), false);
        let b = text.as_bytes();
        let hexish = b[0] == b'0' && b.len() > 1 && b[1] != b'.' && b[1] != b'e' && b[1] != b'E';
        let v = if hexish {
            // Convert.ToInt64(text, 16)
            let digits = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")).unwrap_or(&text);
            if digits.is_empty() {
                return Err(JsonError { kind: ErrKind::Format, message: "Could not find any recognizable digits.".into() });
            }
            match u64::from_str_radix(digits, 16) {
                Ok(u) => Scalar::Int(u as i64),
                Err(_) => {
                    let msg = if digits.bytes().all(|c| c.is_ascii_hexdigit()) { "Value was either too large or too small for an Int64." } else { "Additional non-parsable characters are at the end of the string." };
                    return Err(JsonError { kind: ErrKind::Format, message: msg.into() });
                }
            }
        } else if !text.bytes().any(|c| matches!(c, b'.' | b'e' | b'E')) {
            match number::parse_integer(&text) {
                Some(Int::Long(i)) => Scalar::Int(i),
                Some(Int::Big(s)) => Scalar::Big(s),
                None => return Err(bad(self)),
            }
        } else {
            match number::parse_float(&text) {
                Some(f) => Scalar::Float(f),
                None => return Err(bad(self)),
            }
        };
        self.value_done(Token::Value(v))
    }

    /// The characters of a string up to the closing `quote` (already past the opening one).
    fn read_string_body(&mut self, quote: char) -> Result<String, JsonError> {
        let mut out = String::new();
        let mut pending_high: Option<u16> = None;
        macro_rules! flush_high {
            () => {
                if pending_high.take().is_some() {
                    out.push('\u{fffd}');
                }
            };
        }
        loop {
            let Some(c) = self.bump() else {
                return Err(self.err(&format!("Unterminated string. Expected delimiter: {quote}."), false));
            };
            if c == quote {
                flush_high!();
                return Ok(out);
            }
            if c != '\\' {
                flush_high!();
                out.push(c);
                continue;
            }
            let Some(e) = self.bump() else {
                return Err(self.err(&format!("Unterminated string. Expected delimiter: {quote}."), false));
            };
            let ch = match e {
                'b' => '\u{8}',
                't' => '\t',
                'n' => '\n',
                'f' => '\u{c}',
                'r' => '\r',
                '\\' => '\\',
                '"' => '"',
                '\'' => '\'',
                '/' => '/',
                'u' => {
                    let mut v: u32 = 0;
                    for _ in 0..4 {
                        match self.bump() {
                            None => return Err(self.err("Unexpected end while parsing Unicode escape sequence.", false)),
                            Some(h) => match h.to_digit(16) {
                                Some(d) => v = v * 16 + d,
                                None => return Err(self.err(&format!("Invalid Unicode escape sequence: \\u{}.", h), false)),
                            },
                        }
                    }
                    let v = v as u16;
                    if (0xD800..0xDC00).contains(&v) {
                        flush_high!();
                        pending_high = Some(v);
                        continue;
                    }
                    if (0xDC00..0xE000).contains(&v) {
                        if let Some(h) = pending_high.take() {
                            let cp = 0x10000 + (((h as u32) - 0xD800) << 10) + (v as u32 - 0xDC00);
                            out.push(char::from_u32(cp).unwrap_or('\u{fffd}'));
                        } else {
                            out.push('\u{fffd}');
                        }
                        continue;
                    }
                    flush_high!();
                    out.push(char::from_u32(v as u32).unwrap_or('\u{fffd}'));
                    continue;
                }
                other => return Err(self.err(&format!("Bad JSON escape sequence: \\{other}."), false)),
            };
            flush_high!();
            out.push(ch);
        }
    }

    /// `ReadAsString`-style helper for tests and the converter path: skips comments.
    pub fn read_skip_comments(&mut self) -> Result<bool, JsonError> {
        loop {
            if !self.read()? {
                return Ok(false);
            }
            if self.token.kind() != COMMENT {
                return Ok(true);
            }
        }
    }
}

// ------------------------------------------------------------------------------------------------ dates

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}
/// Days since 0001-01-01 of a civil date.
pub fn date_days(y: i64, m: i64, d: i64) -> i64 {
    days_from_civil(y, m, d) - days_from_civil(1, 1, 1)
}
pub fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}
fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
                29
            } else {
                28
            }
        }
    }
}
pub const TICKS_PER_SECOND: i64 = 10_000_000;
const TICKS_PER_DAY: i64 = 86_400 * TICKS_PER_SECOND;

/// `DateTimeUtils.TryParseDateTime` for the two formats the reader recognises: `yyyy-MM-ddTHH:mm:ss[.f{1,7}][Z|±hh[:mm]]`
/// and `/Date(milliseconds[±hhmm])/`.
pub fn parse_date(s: &str) -> Option<DateVal> {
    let b = s.as_bytes();
    if b.len() >= 19 && b[4] == b'-' && b[7] == b'-' && b[10] == b'T' && b[13] == b':' && b[16] == b':' {
        let num = |a: usize, n: usize| -> Option<i64> {
            let t = s.get(a..a + n)?;
            if t.bytes().all(|c| c.is_ascii_digit()) {
                t.parse().ok()
            } else {
                None
            }
        };
        let (y, mo, d, h, mi, se) = (num(0, 4)?, num(5, 2)?, num(8, 2)?, num(11, 2)?, num(14, 2)?, num(17, 2)?);
        if !(1..=9999).contains(&y) || !(1..=12).contains(&mo) || d < 1 || d > days_in_month(y, mo) || h > 23 || mi > 59 || se > 59 {
            return None;
        }
        let mut i = 19;
        let mut frac: i64 = 0;
        if i < b.len() && b[i] == b'.' {
            i += 1;
            let st = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            let n = i - st;
            if n == 0 || n > 7 {
                return None;
            }
            frac = s[st..i].parse::<i64>().ok()? * 10i64.pow(7 - n as u32);
        }
        let zone = if i == b.len() {
            Zone::Unspecified
        } else if b[i] == b'Z' && i + 1 == b.len() {
            Zone::Utc
        } else if b[i] == b'+' || b[i] == b'-' {
            let sign = if b[i] == b'-' { -1 } else { 1 };
            let rest = &s[i + 1..];
            let (oh, om) = match rest.len() {
                2 => (rest.parse::<i32>().ok()?, 0),
                4 => (rest[..2].parse::<i32>().ok()?, rest[2..].parse::<i32>().ok()?),
                5 if rest.as_bytes()[2] == b':' => (rest[..2].parse::<i32>().ok()?, rest[3..].parse::<i32>().ok()?),
                _ => return None,
            };
            if oh > 23 || om > 59 || !rest.bytes().all(|c| c.is_ascii_digit() || c == b':') {
                return None;
            }
            Zone::Offset(sign * (oh * 60 + om))
        } else {
            return None;
        };
        let local = date_days(y, mo, d) * TICKS_PER_DAY + (h * 3600 + mi * 60 + se) * TICKS_PER_SECOND + frac;
        // an offset date is held as UTC ticks (local ticks minus the offset)
        let ticks = match zone {
            Zone::Offset(m) => local - m as i64 * 60 * TICKS_PER_SECOND,
            _ => local,
        };
        return Some(DateVal { ticks, zone });
    }
    // /Date(1577873472000)/ , /Date(1577873472000+0100)/
    let inner = s.strip_prefix("/Date(")?.strip_suffix(")/")?;
    let (digits, zone_text) = match inner.rfind(['+', '-']).filter(|p| *p > 0) {
        Some(p) => (&inner[..p], Some(&inner[p..])),
        None => (inner, None),
    };
    let ms: i64 = digits.parse().ok()?;
    let unix_epoch_ticks = date_days(1970, 1, 1) * TICKS_PER_DAY;
    let ticks = unix_epoch_ticks.checked_add(ms.checked_mul(10_000)?)?;
    let zone = match zone_text {
        None => Zone::Utc,
        Some(z) if z.len() == 5 && z[1..].bytes().all(|c| c.is_ascii_digit()) => {
            let m = z[1..3].parse::<i32>().ok()? * 60 + z[3..5].parse::<i32>().ok()?;
            Zone::Offset(if z.starts_with('-') { -m } else { m })
        }
        _ => return None,
    };
    Some(DateVal { ticks, zone })
}

/// `DateTimeUtils.WriteDateTimeString` with `DateFormatHandling.IsoDateFormat`:
/// `yyyy-MM-ddTHH:mm:ss.FFFFFFF` + `Z` / the offset.
pub fn format_date(d: &DateVal) -> String {
    let (ticks, suffix) = match d.zone {
        Zone::Unspecified => (d.ticks, String::new()),
        Zone::Utc => (d.ticks, "Z".into()),
        Zone::Offset(m) => (d.ticks + m as i64 * 60 * TICKS_PER_SECOND, format!("{}{:02}:{:02}", if m < 0 { '-' } else { '+' }, m.abs() / 60, m.abs() % 60)),
    };
    let days = ticks.div_euclid(TICKS_PER_DAY);
    let rem = ticks.rem_euclid(TICKS_PER_DAY);
    let (y, mo, da) = civil_from_days(days + days_from_civil(1, 1, 1));
    let secs = rem / TICKS_PER_SECOND;
    let frac = rem % TICKS_PER_SECOND;
    let mut out = format!("{y:04}-{mo:02}-{da:02}T{:02}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60);
    if frac != 0 {
        let f = format!("{frac:07}");
        out.push('.');
        out.push_str(f.trim_end_matches('0'));
    }
    out + &suffix
}

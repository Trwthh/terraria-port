//! `JsonTextWriter` of Newtonsoft.Json 10: separators, indentation, string escaping, number text.
//!
//! `Formatting.Indented`: two spaces per level, `\r\n` line ends (`Environment.NewLine` on Windows), `"name": value`,
//! empty containers as `{}` / `[]`. Strings are escaped as `StringEscapeHandling.Default` does: `"`, `\`, control
//! characters (`\b \t \n \f \r`, others `\u00xx` in lower case) and U+0085, U+2028, U+2029; all other characters
//! (including non-ASCII) are written as they are. NaN and infinities are written as the quoted strings `"NaN"`,
//! `"Infinity"`, `"-Infinity"` (`FloatFormatHandling.String`, the default).

use super::number;
use super::reader::{format_date, DateVal, Scalar};

pub const NEWLINE: &str = "\r\n";

struct Frame {
    array: bool,
    has_items: bool,
}

pub struct Writer {
    pub out: String,
    pub indented: bool,
    pub indentation: usize,
    pub indent_char: char,
    pub quote_char: char,
    /// `StringEscapeHandling.EscapeNonAscii`
    pub escape_non_ascii: bool,
    stack: Vec<Frame>,
    after_property: bool,
}

#[derive(PartialEq, Clone, Copy)]
enum Kind {
    Value,
    Property,
    Comment,
}

impl Writer {
    pub fn new(indented: bool) -> Writer {
        Writer { out: String::new(), indented, indentation: 2, indent_char: ' ', quote_char: '"', escape_non_ascii: false, stack: Vec::new(), after_property: false }
    }

    fn indent(&mut self) {
        self.out.push_str(NEWLINE);
        for _ in 0..self.stack.len() * self.indentation {
            self.out.push(self.indent_char);
        }
    }

    fn auto_complete(&mut self, kind: Kind) {
        let top = self.stack.last().map(|f| (f.array, f.has_items));
        if kind != Kind::Comment && !self.after_property {
            if let Some((_, true)) = top {
                self.out.push(',');
            }
        }
        if self.indented {
            if self.after_property {
                self.out.push(' ');
            }
            if matches!(top, Some((true, _))) || (kind == Kind::Property && top.is_some()) {
                self.indent();
            }
        }
        match kind {
            Kind::Property => self.after_property = true,
            Kind::Value => {
                self.after_property = false;
                if let Some(f) = self.stack.last_mut() {
                    f.has_items = true;
                }
            }
            Kind::Comment => {}
        }
    }

    pub fn start_object(&mut self) {
        self.auto_complete(Kind::Value);
        self.out.push('{');
        self.stack.push(Frame { array: false, has_items: false });
    }
    pub fn start_array(&mut self) {
        self.auto_complete(Kind::Value);
        self.out.push('[');
        self.stack.push(Frame { array: true, has_items: false });
    }
    pub fn start_constructor(&mut self, name: &str) {
        self.auto_complete(Kind::Value);
        self.out.push_str("new ");
        self.out.push_str(name);
        self.out.push('(');
        self.stack.push(Frame { array: true, has_items: false });
    }
    /// `WriteEndObject` / `WriteEndArray` / `WriteEndConstructor` (the caller knows which; `end` closes the innermost).
    pub fn end(&mut self, closer: char) {
        let f = self.stack.pop();
        if self.indented && f.as_ref().is_some_and(|f| f.has_items) {
            self.indent();
        }
        self.out.push(closer);
    }
    pub fn end_object(&mut self) {
        self.end('}')
    }
    pub fn end_array(&mut self) {
        self.end(']')
    }
    pub fn end_constructor(&mut self) {
        self.end(')')
    }
    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    pub fn property_name(&mut self, name: &str) {
        self.auto_complete(Kind::Property);
        self.escaped(name);
        self.out.push(':');
    }

    pub fn escaped(&mut self, s: &str) {
        let q = self.quote_char;
        self.out.push(q);
        for c in s.chars() {
            match c {
                '\t' => self.out.push_str("\\t"),
                '\n' => self.out.push_str("\\n"),
                '\r' => self.out.push_str("\\r"),
                '\u{c}' => self.out.push_str("\\f"),
                '\u{8}' => self.out.push_str("\\b"),
                '\\' => self.out.push_str("\\\\"),
                '\u{85}' | '\u{2028}' | '\u{2029}' => self.out.push_str(&format!("\\u{:04x}", c as u32)),
                c if c == q => {
                    self.out.push('\\');
                    self.out.push(c);
                }
                '\'' if q == '\'' => self.out.push_str("\\'"),
                c if (c as u32) < 0x20 => self.out.push_str(&format!("\\u{:04x}", c as u32)),
                c if self.escape_non_ascii && (c as u32) > 0x7e => {
                    let mut buf = [0u16; 2];
                    for u in c.encode_utf16(&mut buf) {
                        self.out.push_str(&format!("\\u{:04x}", u));
                    }
                }
                c => self.out.push(c),
            }
        }
        self.out.push(q);
    }

    pub fn null(&mut self) {
        self.auto_complete(Kind::Value);
        self.out.push_str("null");
    }
    pub fn undefined(&mut self) {
        self.auto_complete(Kind::Value);
        self.out.push_str("undefined");
    }
    pub fn bool(&mut self, v: bool) {
        self.auto_complete(Kind::Value);
        self.out.push_str(if v { "true" } else { "false" });
    }
    pub fn int(&mut self, v: i64) {
        self.auto_complete(Kind::Value);
        self.out.push_str(&v.to_string());
    }
    pub fn uint(&mut self, v: u64) {
        self.auto_complete(Kind::Value);
        self.out.push_str(&v.to_string());
    }
    pub fn big(&mut self, digits: &str) {
        self.auto_complete(Kind::Value);
        self.out.push_str(digits);
    }
    pub fn double(&mut self, v: f64) {
        self.auto_complete(Kind::Value);
        if v.is_finite() {
            self.out.push_str(&number::json_double(v));
        } else {
            self.escaped(&number::format_double_r(v));
        }
    }
    pub fn float(&mut self, v: f32) {
        self.auto_complete(Kind::Value);
        if v.is_finite() {
            self.out.push_str(&number::json_float(v));
        } else {
            self.escaped(&number::format_float_r(v));
        }
    }
    pub fn string(&mut self, s: &str) {
        self.auto_complete(Kind::Value);
        self.escaped(s);
    }
    pub fn date(&mut self, d: &DateVal) {
        self.auto_complete(Kind::Value);
        let q = self.quote_char;
        self.out.push(q);
        self.out.push_str(&format_date(d));
        self.out.push(q);
    }
    pub fn bytes(&mut self, b: &[u8]) {
        self.auto_complete(Kind::Value);
        let q = self.quote_char;
        self.out.push(q);
        self.out.push_str(&crate::sys::text::base64(b));
        self.out.push(q);
    }
    pub fn comment(&mut self, text: &str) {
        self.auto_complete(Kind::Comment);
        self.out.push_str("/*");
        self.out.push_str(text);
        self.out.push_str("*/");
    }
    pub fn raw_value(&mut self, text: &str) {
        self.auto_complete(Kind::Value);
        self.out.push_str(text);
    }

    /// Any scalar token.
    pub fn scalar(&mut self, v: &Scalar) {
        match v {
            Scalar::Null => self.null(),
            Scalar::Undefined => self.undefined(),
            Scalar::Bool(b) => self.bool(*b),
            Scalar::Int(i) => self.int(*i),
            Scalar::Big(s) => self.big(s),
            Scalar::Float(f) => self.double(*f),
            Scalar::Float32(f) => self.float(*f),
            Scalar::Str(s) => self.string(s),
            Scalar::Date(d) => self.date(d),
            Scalar::Bytes(b) => self.bytes(b),
            Scalar::Comment(c) => self.comment(c),
            Scalar::Raw(r) => self.raw_value(r),
        }
    }
}

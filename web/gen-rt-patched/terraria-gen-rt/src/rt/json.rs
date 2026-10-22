//! The part of Newtonsoft.Json the game's start-up path uses, natively: a JSON reader and
//! `JsonConvert.DeserializeObject<T>` for the container shapes the game asks for.
//!
//! Reader rules follow Newtonsoft's `JsonTextReader` where the game's files can tell: object members keep document
//! order, `//` and `/* */` comments are skipped, strings may use double or single quotes, escapes
//! `\" \\ \/ \b \f \n \r \t \uXXXX`. A dictionary is filled with `dictionary[key] = value` in document order (a
//! repeated key keeps its first position and takes the last value), as Newtonsoft's dictionary contract does.

use super::collections::{Dictionary, List};
use super::exc::{exc, throw};
use super::gc::Gc;
use super::string::Str;
use super::Val;
use std::any::TypeId;

#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    /// The number as written.
    Num(String),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

fn bad(what: &str, at: usize) -> ! {
    throw(exc("Newtonsoft.Json.JsonReaderException", Str::new(&format!("{what} (at character {at})"))))
}

const QUOTE: u8 = 0x22;
const APOSTROPHE: u8 = 0x27;
const BACKSLASH: u8 = 0x5C;

struct Reader<'a> {
    s: &'a [u8],
    i: usize,
}
impl Reader<'_> {
    fn ws(&mut self) {
        loop {
            while self.i < self.s.len() && matches!(self.s[self.i], b' ' | b'\t' | b'\r' | b'\n') {
                self.i += 1;
            }
            // UTF-8 byte order mark
            if self.s[self.i..].starts_with(&[0xEF, 0xBB, 0xBF]) {
                self.i += 3;
                continue;
            }
            if self.s[self.i..].starts_with(b"//") {
                while self.i < self.s.len() && self.s[self.i] != b'\n' {
                    self.i += 1;
                }
                continue;
            }
            if self.s[self.i..].starts_with(b"/*") {
                self.i += 2;
                while self.i < self.s.len() && !self.s[self.i..].starts_with(b"*/") {
                    self.i += 1;
                }
                self.i = (self.i + 2).min(self.s.len());
                continue;
            }
            return;
        }
    }

    fn value(&mut self) -> Json {
        self.ws();
        let Some(&c) = self.s.get(self.i) else { bad("Unexpected end of JSON", self.i) };
        match c {
            b'{' => {
                self.i += 1;
                let mut members = Vec::new();
                loop {
                    self.ws();
                    match self.s.get(self.i).copied() {
                        Some(b'}') => {
                            self.i += 1;
                            return Json::Obj(members);
                        }
                        Some(b',') => {
                            self.i += 1;
                        }
                        Some(QUOTE) | Some(APOSTROPHE) => {
                            let k = self.string();
                            self.ws();
                            if self.s.get(self.i) != Some(&b':') {
                                bad("Expected ':' after a property name", self.i)
                            }
                            self.i += 1;
                            let v = self.value();
                            members.push((k, v));
                        }
                        _ => bad("Invalid property name", self.i),
                    }
                }
            }
            b'[' => {
                self.i += 1;
                let mut items = Vec::new();
                loop {
                    self.ws();
                    match self.s.get(self.i) {
                        Some(b']') => {
                            self.i += 1;
                            return Json::Arr(items);
                        }
                        Some(b',') => {
                            self.i += 1;
                        }
                        Some(_) => items.push(self.value()),
                        None => bad("Unexpected end of JSON in an array", self.i),
                    }
                }
            }
            QUOTE | APOSTROPHE => Json::Str(self.string()),
            b't' if self.s[self.i..].starts_with(b"true") => {
                self.i += 4;
                Json::Bool(true)
            }
            b'f' if self.s[self.i..].starts_with(b"false") => {
                self.i += 5;
                Json::Bool(false)
            }
            b'n' if self.s[self.i..].starts_with(b"null") => {
                self.i += 4;
                Json::Null
            }
            b'-' | b'+' | b'.' | b'0'..=b'9' => {
                let a = self.i;
                while self.i < self.s.len() && matches!(self.s[self.i], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') {
                    self.i += 1;
                }
                Json::Num(String::from_utf8_lossy(&self.s[a..self.i]).into_owned())
            }
            _ => bad("Unexpected character", self.i),
        }
    }

    fn string(&mut self) -> String {
        let quote = self.s[self.i];
        self.i += 1;
        let mut out: Vec<u16> = Vec::new();
        let mut bytes: Vec<u8> = Vec::new();
        let flush = |bytes: &mut Vec<u8>, out: &mut Vec<u16>| {
            if !bytes.is_empty() {
                out.extend(String::from_utf8_lossy(bytes).encode_utf16());
                bytes.clear();
            }
        };
        loop {
            let Some(&c) = self.s.get(self.i) else { bad("Unterminated string", self.i) };
            self.i += 1;
            if c == quote {
                break;
            }
            if c != BACKSLASH {
                bytes.push(c);
                continue;
            }
            flush(&mut bytes, &mut out);
            let Some(&e) = self.s.get(self.i) else { bad("Unterminated string", self.i) };
            self.i += 1;
            let unit: u16 = match e {
                b'n' => 10,
                b'r' => 13,
                b't' => 9,
                b'b' => 8,
                b'f' => 12,
                b'u' => {
                    let hex = self.s.get(self.i..self.i + 4).and_then(|h| core::str::from_utf8(h).ok()).and_then(|h| u16::from_str_radix(h, 16).ok());
                    match hex {
                        Some(v) => {
                            self.i += 4;
                            v
                        }
                        None => bad("Invalid Unicode escape sequence", self.i),
                    }
                }
                other => other as u16,
            };
            out.push(unit);
        }
        flush(&mut bytes, &mut out);
        // a lone surrogate cannot be represented in a `Str`: U+FFFD
        String::from_utf16_lossy(&out)
    }
}

/// Parses one JSON document (`JsonReaderException` on malformed text).
pub fn parse(text: &str) -> Json {
    let mut r = Reader { s: text.as_bytes(), i: 0 };
    let v = r.value();
    r.ws();
    v
}

/// A C# type `JsonConvert.DeserializeObject<T>` can produce here.
pub trait FromJson: Val {
    fn from_json(v: &Json) -> Self;
}
fn serial(what: &str) -> ! {
    throw(exc("Newtonsoft.Json.JsonSerializationException", Str::new(what)))
}
impl FromJson for Str {
    fn from_json(v: &Json) -> Str {
        match v {
            Json::Null => Str::NULL,
            Json::Str(s) => Str::new(s),
            Json::Num(n) => Str::new(n),
            Json::Bool(b) => Str::lit(if *b { "True" } else { "False" }),
            _ => serial("Cannot deserialize a JSON container into a string"),
        }
    }
}
impl FromJson for bool {
    fn from_json(v: &Json) -> bool {
        match v {
            Json::Bool(b) => *b,
            Json::Str(s) if s.eq_ignore_ascii_case("true") => true,
            Json::Str(s) if s.eq_ignore_ascii_case("false") => false,
            _ => serial("Could not convert a JSON value to Boolean"),
        }
    }
}
macro_rules! json_num {
    ($($t:ty),*) => {$( impl FromJson for $t {
        fn from_json(v: &Json) -> $t {
            let text = match v { Json::Num(n) => n.as_str(), Json::Str(s) => s.as_str(), _ => serial("Could not convert a JSON value to a number") };
            match text.parse::<$t>() { Ok(x) => x, Err(_) => serial("Could not convert a JSON value to a number") }
        }
    } )*};
}
json_num!(i8, u8, i16, u16, i32, u32, i64, u64, f32, f64);
impl<T: FromJson> FromJson for Gc<List<T>> {
    fn from_json(v: &Json) -> Self {
        match v {
            Json::Null => Gc::NULL,
            Json::Arr(items) => {
                let l = List::<T>::new_gc();
                for x in items {
                    let item = T::from_json(x);
                    // SAFETY: fresh list.
                    unsafe { (*l.ptr()).add(item) };
                }
                l
            }
            _ => serial("Cannot deserialize a JSON value that is not an array into a list"),
        }
    }
}
impl<T: FromJson> FromJson for super::Arr<T> {
    fn from_json(v: &Json) -> Self {
        match v {
            Json::Null => super::Arr::NULL,
            Json::Arr(items) => {
                let a = super::Arr::<T>::new(items.len() as i32);
                for (i, x) in items.iter().enumerate() {
                    let item = T::from_json(x);
                    // SAFETY: index in range.
                    unsafe { *a.at(i as i32) = item };
                }
                a
            }
            _ => serial("Cannot deserialize a JSON value that is not an array into an array"),
        }
    }
}
/// Dictionary keys are the property names, converted to the key type.
pub trait FromJsonKey: super::CsKey {
    fn from_key(k: &str) -> Self;
}
impl FromJsonKey for Str {
    fn from_key(k: &str) -> Str {
        Str::new(k)
    }
}
impl FromJsonKey for i32 {
    fn from_key(k: &str) -> i32 {
        k.parse().unwrap_or_else(|_| serial("Could not convert a property name to Int32"))
    }
}
impl<K: FromJsonKey, V: FromJson> FromJson for Gc<Dictionary<K, V>> {
    fn from_json(v: &Json) -> Self {
        match v {
            Json::Null => Gc::NULL,
            Json::Obj(members) => {
                let d = Dictionary::<K, V>::new_gc();
                for (k, x) in members {
                    let value = V::from_json(x);
                    // SAFETY: fresh dictionary.
                    unsafe { (*d.ptr()).set_item(K::from_key(k), value) };
                }
                d
            }
            _ => serial("Cannot deserialize a JSON value that is not an object into a dictionary"),
        }
    }
}

fn build<T: Val, U: FromJson>(v: &Json) -> Option<T> {
    if TypeId::of::<T>() == TypeId::of::<U>() {
        let u = U::from_json(v);
        // SAFETY: T and U are the same type.
        Some(unsafe { core::mem::transmute_copy::<U, T>(&u) })
    } else {
        None
    }
}

/// `JsonConvert.DeserializeObject<T>(text)` for the shapes the game uses that need no reflection over a
/// translated class. Any other `T` is a host member that is not implemented (a plain panic naming `T`).
pub fn deserialize<T: Val>(text: Str) -> T {
    let v = parse(text.as_str());
    type D<K, V> = Gc<Dictionary<K, V>>;
    if let Some(r) = build::<T, D<Str, D<Str, Str>>>(&v) {
        return r;
    }
    if let Some(r) = build::<T, D<Str, Str>>(&v) {
        return r;
    }
    if let Some(r) = build::<T, D<Str, Gc<List<Str>>>>(&v) {
        return r;
    }
    if let Some(r) = build::<T, D<Str, D<Str, D<Str, Str>>>>(&v) {
        return r;
    }
    if let Some(r) = build::<T, Gc<List<Str>>>(&v) {
        return r;
    }
    if let Some(r) = build::<T, super::Arr<Str>>(&v) {
        return r;
    }
    if let Some(r) = build::<T, D<Str, i32>>(&v) {
        return r;
    }
    if let Some(r) = build::<T, D<i32, i32>>(&v) {
        return r;
    }
    crate::host::unimplemented_value::<T>(Box::leak(format!("Newtonsoft.Json.JsonConvert.DeserializeObject<{}>", core::any::type_name::<T>()).into_boxed_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_json_in_document_order_with_escapes_and_comments() {
        let v = parse("\u{feff}{ // c\n \"b\": 1, \"a\": [true, null, -2.5e3, \"x\\n\\u00e9\\\"\"], /* c */ 'q': {} }");
        assert_eq!(
            v,
            Json::Obj(vec![
                ("b".into(), Json::Num("1".into())),
                ("a".into(), Json::Arr(vec![Json::Bool(true), Json::Null, Json::Num("-2.5e3".into()), Json::Str("x\n\u{e9}\"".into())])),
                ("q".into(), Json::Obj(vec![])),
            ])
        );
        assert!(crate::rt::try_(|| parse("{ \"a\" 1 }")).is_err());
    }

    #[test]
    fn nested_string_dictionaries_keep_order_and_last_value() {
        let d: Gc<Dictionary<Str, Gc<Dictionary<Str, Str>>>> = deserialize(Str::lit("{\"Z\": {\"k2\": \"b\", \"k1\": \"a\", \"k2\": \"c\"}, \"A\": {}}"));
        unsafe {
            let keys: Vec<String> = Dictionary::keys(d).to_vec().iter().map(|k| k.as_str().to_owned()).collect();
            assert_eq!(keys, ["Z", "A"]);
            let z = (*d.ptr()).get_item(Str::lit("Z"));
            let inner: Vec<String> = Dictionary::keys(z).to_vec().iter().map(|k| k.as_str().to_owned()).collect();
            assert_eq!(inner, ["k2", "k1"]);
            assert_eq!((*z.ptr()).get_item(Str::lit("k2")).as_str(), "c");
        }
    }
}

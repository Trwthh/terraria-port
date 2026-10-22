//! `Newtonsoft.Json.Linq`: `JToken`, `JContainer`, `JObject`, `JArray`, `JProperty`, `JConstructor`, `JValue` as ONE
//! heap type ([`JToken`], like `Texture2D` / `RenderTarget2D` in the gfx module): `x is JObject` is a check of `kind`.
//!
//! Behaviour taken from the dll (tests/dotnet/json_probe.ps1 writes the expected outputs):
//! - property order is insertion order; a duplicate key read by `JToken.Parse` / `JObject.Parse` replaces the earlier
//!   property in place (last value wins, first position), through the serializer path (`JsonConvert.DeserializeObject`,
//!   [`JToken::deserialize`]) the duplicate is moved to the end;
//! - comments: `Parse` drops them in objects and arrays; the serializer path keeps them as items of arrays, drops them
//!   in objects and throws `ArgumentException` for a comment between a property name and its value;
//! - `JToken.ToString()` is `Formatting.Indented`; a `JValue.ToString()` is the plain text of its value.

use terraria_gen_rt::rt::{self, Gc, Marker, Str, Trace};

use super::number;
use super::reader::{self, DateVal, ErrKind, JsonError, Reader, Scalar, Token};
use super::writer::Writer;

/// `JTokenType`
pub const T_NONE: i32 = 0;
pub const T_OBJECT: i32 = 1;
pub const T_ARRAY: i32 = 2;
pub const T_CONSTRUCTOR: i32 = 3;
pub const T_PROPERTY: i32 = 4;
pub const T_COMMENT: i32 = 5;
pub const T_INTEGER: i32 = 6;
pub const T_FLOAT: i32 = 7;
pub const T_STRING: i32 = 8;
pub const T_BOOLEAN: i32 = 9;
pub const T_NULL: i32 = 10;
pub const T_UNDEFINED: i32 = 11;
pub const T_DATE: i32 = 12;
pub const T_RAW: i32 = 13;
pub const T_BYTES: i32 = 14;

pub struct JToken {
    /// `JTokenType`
    pub kind: i32,
    pub parent: Gc<JToken>,
    /// property name / constructor name
    pub name: String,
    /// object: its properties; array / constructor: the items; property: the value (0 or 1 entries)
    pub children: Vec<Gc<JToken>>,
    /// the value of a `JValue`
    pub value: Scalar,
    /// an integer that came from a .NET `int` (or a smaller integer type) through the serialiser: `BsonWriter`
    /// writes it as Int32 (`0x10`); integers of the DOM (parsed text, `new JValue(long)`) are `long` (`0x12`)
    pub int32: bool,
}

// SAFETY: the parent and every child are reported.
unsafe impl Trace for JToken {
    unsafe fn trace(&self, m: &mut Marker) {
        m.mark(self.parent);
        for c in &self.children {
            m.mark(*c);
        }
    }
}

pub fn throw_json(e: JsonError) -> ! {
    let kind = match e.kind {
        ErrKind::Reader => "Newtonsoft.Json.JsonReaderException",
        ErrKind::Format => "System.FormatException",
    };
    rt::throw(rt::exc(kind, Str::new(&e.message)))
}
fn throw_msg(kind: &'static str, msg: &str) -> ! {
    rt::throw(rt::exc(kind, Str::new(msg)))
}

fn node(kind: i32, name: &str, value: Scalar) -> Gc<JToken> {
    rt::alloc_init(JToken { kind, parent: Gc::NULL, name: name.to_owned(), children: Vec::new(), value, int32: false })
}
fn r(g: Gc<JToken>) -> &'static mut JToken {
    // SAFETY: a live node; the host is single threaded and callers copy what they need before calling again.
    unsafe { &mut *g.ptr() }
}

/// How the loader treats comments and duplicates (the two paths in the module docs).
#[derive(Clone, Copy, PartialEq)]
pub enum Path {
    /// `JToken.Parse`, `JObject.Parse`, `JToken.Load`
    Parse,
    /// `JsonConvert.DeserializeObject<JToken / JObject / JArray>`, values of `Dictionary<string, object>`
    Serializer,
}

impl JToken {
    // ---------------------------------------------------------------- construction

    pub fn new_object() -> Gc<JToken> {
        node(T_OBJECT, "", Scalar::Null)
    }
    pub fn new_array() -> Gc<JToken> {
        node(T_ARRAY, "", Scalar::Null)
    }
    pub fn new_property(name: Str, value: Gc<JToken>) -> Gc<JToken> {
        let p = node(T_PROPERTY, name.as_str(), Scalar::Null);
        if !value.is_null() {
            Self::attach(p, value);
        }
        p
    }
    /// `new JValue(...)`: `kind` from the scalar.
    /// An integer value typed `int` by the serialiser (see [`JToken::int32`]).
    pub fn new_int32(v: i32) -> Gc<JToken> {
        let t = JToken::new_value(Scalar::Int(v as i64));
        r(t).int32 = true;
        t
    }
    pub fn new_value(v: Scalar) -> Gc<JToken> {
        let kind = match &v {
            Scalar::Null => T_NULL,
            Scalar::Undefined => T_UNDEFINED,
            Scalar::Bool(_) => T_BOOLEAN,
            Scalar::Int(_) | Scalar::Big(_) => T_INTEGER,
            Scalar::Float(_) | Scalar::Float32(_) => T_FLOAT,
            Scalar::Str(_) => T_STRING,
            Scalar::Date(_) => T_DATE,
            Scalar::Bytes(_) => T_BYTES,
            Scalar::Comment(_) => T_COMMENT,
            Scalar::Raw(_) => T_RAW,
        };
        node(kind, "", v)
    }
    pub fn new_str(s: Str) -> Gc<JToken> {
        if s.is_null() {
            Self::new_value(Scalar::Null)
        } else {
            Self::new_value(Scalar::Str(s.as_str().to_owned()))
        }
    }

    fn attach(parent: Gc<JToken>, child: Gc<JToken>) {
        let c = r(child);
        if !c.parent.is_null() {
            // C#: a token that already has a parent is cloned
            let copy = Self::deep_clone(child);
            r(copy).parent = parent;
            r(parent).children.push(copy);
            return;
        }
        c.parent = parent;
        r(parent).children.push(child);
    }

    pub fn deep_clone(t: Gc<JToken>) -> Gc<JToken> {
        let s = r(t);
        let n = node(s.kind, &s.name.clone(), s.value.clone());
        r(n).int32 = s.int32;
        let kids: Vec<Gc<JToken>> = s.children.clone();
        for k in kids {
            let c = Self::deep_clone(k);
            r(c).parent = n;
            r(n).children.push(c);
        }
        n
    }

    // ---------------------------------------------------------------- loading

    /// `JToken.Parse(json)` / `JObject.Parse` / `JArray.Parse`. `None` for text with no token (the original throws).
    pub fn parse(text: Str) -> Gc<JToken> {
        match Self::load(text.get().unwrap_or_else(|| throw_msg("System.ArgumentNullException", "Value cannot be null.\r\nParameter name: value")), Path::Parse) {
            Ok(Some(t)) => t,
            Ok(None) => throw_json(JsonError { kind: ErrKind::Reader, message: "Error reading JToken from JsonReader. Path '', line 0, position 0.".into() }),
            Err(e) => throw_json(e),
        }
    }
    /// `JObject.Parse(json)`: the root must be an object.
    pub fn parse_object(text: Str) -> Gc<JToken> {
        let t = Self::parse(text);
        if r(t).kind != T_OBJECT {
            throw_json(JsonError { kind: ErrKind::Reader, message: "Error reading JObject from JsonReader. Current JsonReader item is not an object: ".to_owned() + &token_name(t) + ". Path '', line 1, position 1." });
        }
        t
    }
    pub fn parse_array(text: Str) -> Gc<JToken> {
        let t = Self::parse(text);
        if r(t).kind != T_ARRAY {
            throw_json(JsonError { kind: ErrKind::Reader, message: "Error reading JArray from JsonReader. Current JsonReader item is not an array: ".to_owned() + &token_name(t) + ". Path '', line 1, position 1." });
        }
        t
    }

    /// `JsonConvert.DeserializeObject<JToken>(json)` (also `<JObject>`, `<JArray>`): null for empty text.
    pub fn deserialize(text: Str) -> Gc<JToken> {
        match Self::load(text.get().unwrap_or(""), Path::Serializer) {
            Ok(Some(t)) => t,
            Ok(None) => Gc::NULL,
            Err(e) => throw_json(e),
        }
    }

    /// Reads one value from `text` and the rest of the content (so trailing garbage is an error).
    pub fn load(text: &str, path: Path) -> Result<Option<Gc<JToken>>, JsonError> {
        let text = text.strip_prefix('\u{feff}').filter(|_| false).unwrap_or(text);
        let mut rd = Reader::new(text);
        let root = Self::load_from(&mut rd, path)?;
        // the rest of the content: comments are fine, anything else is an error raised by the reader
        while rd.read()? {}
        Ok(root)
    }

    /// `JToken.Load(reader)`.
    pub fn load_from(rd: &mut Reader, path: Path) -> Result<Option<Gc<JToken>>, JsonError> {
        if path == Path::Serializer {
            // the serializer skips comments before the root value
            if !rd.read_skip_comments()? {
                return Ok(None);
            }
        } else if !rd.read()? {
            let msg = format!("Error reading JToken from JsonReader. Path '', line {}, position {}.", rd.line_number(), rd.line_position());
            return Err(JsonError { kind: ErrKind::Reader, message: msg });
        }
        Self::read_token(rd, path).map(Some)
    }

    /// Builds the token that starts at the reader's current token.
    fn read_token(rd: &mut Reader, path: Path) -> Result<Gc<JToken>, JsonError> {
        let cur = rd.token.clone();
        match cur {
            Token::StartObject | Token::StartArray | Token::StartConstructor(_) => {
                let (root, name) = match &cur {
                    Token::StartObject => (Self::new_object(), "JObject"),
                    Token::StartArray => (Self::new_array(), "JArray"),
                    Token::StartConstructor(n) => (node(T_CONSTRUCTOR, n, Scalar::Null), "JConstructor"),
                    _ => unreachable!(),
                };
                let mut cur = root;
                loop {
                    if !rd.read()? {
                        if path == Path::Serializer {
                            return Ok(root);
                        }
                        let msg = format!("Unexpected end of content while loading {name}. Path '{}', line {}, position {}.", rd.path(), rd.line_number(), rd.line_position());
                        return Err(JsonError { kind: ErrKind::Reader, message: msg });
                    }
                    match rd.token.clone() {
                        Token::StartObject | Token::StartArray | Token::StartConstructor(_) => {
                            let c = match rd.token.clone() {
                                Token::StartObject => Self::new_object(),
                                Token::StartArray => Self::new_array(),
                                Token::StartConstructor(n) => node(T_CONSTRUCTOR, &n, Scalar::Null),
                                _ => unreachable!(),
                            };
                            Self::add_child(cur, c, path, rd)?;
                            cur = c;
                        }
                        Token::EndObject | Token::EndArray | Token::EndConstructor => {
                            if cur == root {
                                return Ok(root);
                            }
                            cur = r(cur).parent;
                            // the container was the value of a property: the property is complete
                            if r(cur).kind == T_PROPERTY {
                                cur = r(cur).parent;
                            }
                        }
                        Token::PropertyName(n) => {
                            let p = node(T_PROPERTY, &n, Scalar::Null);
                            Self::add_child(cur, p, path, rd)?;
                            cur = p;
                        }
                        Token::Value(sc) => {
                            // the serializer turns `undefined` into null
                            let sc = if path == Path::Serializer && sc == Scalar::Undefined { Scalar::Null } else { sc };
                            let v = Self::new_value(sc);
                            let attached = Self::add_child(cur, v, path, rd)?;
                            if attached && r(cur).kind == T_PROPERTY {
                                cur = r(cur).parent;
                            }
                        }
                        Token::None => {}
                    }
                }
            }
            Token::PropertyName(n) => {
                // JToken.Load on a property name: the property with its value
                let p = node(T_PROPERTY, &n, Scalar::Null);
                if !rd.read()? {
                    return Err(JsonError { kind: ErrKind::Reader, message: "Error reading JProperty from JsonReader.".into() });
                }
                let v = Self::read_token(rd, path)?;
                Self::attach(p, v);
                Ok(p)
            }
            Token::Value(s) => Ok(Self::new_value(if path == Path::Serializer && s == Scalar::Undefined { Scalar::Null } else { s })),
            Token::None | Token::EndObject | Token::EndArray | Token::EndConstructor => Err(JsonError { kind: ErrKind::Reader, message: "Error reading JToken from JsonReader.".into() }),
        }
    }

    /// `parent.Add(child)` with the comment and duplicate rules of the two paths.
    fn add_child(parent: Gc<JToken>, child: Gc<JToken>, path: Path, rd: &Reader) -> Result<bool, JsonError> {
        let pk = r(parent).kind;
        let ck = r(child).kind;
        if ck == T_COMMENT {
            match (pk, path) {
                (T_OBJECT, _) => return Ok(false),
                (T_PROPERTY, Path::Parse) => return Ok(false),
                (T_PROPERTY, Path::Serializer) => {
                    let _ = rd;
                    return Err(JsonError { kind: ErrKind::Reader, message: "Can not add Newtonsoft.Json.Linq.JValue to Newtonsoft.Json.Linq.JObject.".into() });
                }
                (T_ARRAY | T_CONSTRUCTOR, Path::Parse) => return Ok(false),
                _ => {}
            }
        }
        if pk == T_OBJECT && ck == T_PROPERTY {
            let name = r(child).name.clone();
            let existing = r(parent).children.iter().position(|c| r(*c).name == name);
            if let Some(i) = existing {
                match path {
                    Path::Parse => {
                        let old = r(parent).children[i];
                        r(old).parent = Gc::NULL;
                        r(parent).children[i] = child;
                        r(child).parent = parent;
                    }
                    Path::Serializer => {
                        let old = r(parent).children.remove(i);
                        r(old).parent = Gc::NULL;
                        r(child).parent = parent;
                        r(parent).children.push(child);
                    }
                }
                return Ok(true);
            }
        }
        r(child).parent = parent;
        r(parent).children.push(child);
        Ok(true)
    }

    // ---------------------------------------------------------------- writing

    /// Writes this token into `w` (`WriteTo`).
    pub fn write_to(&self, w: &mut Writer) {
        match self.kind {
            T_OBJECT => {
                w.start_object();
                for p in &self.children {
                    r(*p).write_to(w);
                }
                w.end_object();
            }
            T_ARRAY => {
                w.start_array();
                for c in &self.children {
                    r(*c).write_to(w);
                }
                w.end_array();
            }
            T_CONSTRUCTOR => {
                w.start_constructor(&self.name);
                for c in &self.children {
                    r(*c).write_to(w);
                }
                w.end_constructor();
            }
            T_PROPERTY => {
                w.property_name(&self.name);
                match self.children.first() {
                    Some(v) => r(*v).write_to(w),
                    None => w.null(),
                }
            }
            _ => w.scalar(&self.value),
        }
    }

    /// `ToString(Formatting)`: 0 `None`, 1 `Indented`. A `JValue` writes its JSON form here too (`ToString()` of a
    /// value uses [`JToken::to_text`]).
    pub fn to_json(&self, formatting: i32) -> String {
        let mut w = Writer::new(formatting == 1);
        self.write_to(&mut w);
        w.out
    }

    /// `ToString()`: a container is indented JSON, a value is its text (`_value.ToString()`).
    pub fn to_text(&self) -> Str {
        match self.kind {
            T_OBJECT | T_ARRAY | T_CONSTRUCTOR | T_PROPERTY => Str::new(&self.to_json(1)),
            _ => Str::new(&self.value_text()),
        }
    }
    /// `ToString(Formatting, params JsonConverter[])`: always JSON, also for a `JValue` (`"hello"`, `true`).
    pub fn to_string_fmt(&self, formatting: i32) -> Str {
        Str::new(&self.to_json(formatting))
    }

    /// `JValue.ToString()` with the invariant-like culture the game forces nowhere: .NET's `Object.ToString()`.
    fn value_text(&self) -> String {
        match &self.value {
            Scalar::Null | Scalar::Undefined => String::new(),
            Scalar::Bool(b) => if *b { "True" } else { "False" }.into(),
            Scalar::Int(i) => i.to_string(),
            Scalar::Big(s) => s.clone(),
            Scalar::Float(f) => number::format_g(*f, 15),
            Scalar::Float32(f) => number::format_g(*f as f64, 7),
            Scalar::Str(s) | Scalar::Comment(s) | Scalar::Raw(s) => s.clone(),
            Scalar::Date(d) => reader::format_date(d).replace('T', " "),
            Scalar::Bytes(b) => crate::sys::text::base64(b),
        }
    }

    // ---------------------------------------------------------------- navigation (JContainer / JObject / JArray)

    pub fn token_type(&self) -> i32 {
        self.kind
    }
    pub fn parent(&self) -> Gc<JToken> {
        if self.parent.is_null() {
            return Gc::NULL;
        }
        // the parent of a value inside a property is the property
        self.parent
    }
    /// `JContainer.Count`
    pub fn count(&self) -> i32 {
        self.children.len() as i32
    }
    pub fn has_values(&self) -> bool {
        !self.children.is_empty()
    }
    /// `JObject[string]`: the property's value, null when absent.
    pub fn get(&self, key: Str) -> Gc<JToken> {
        if self.kind != T_OBJECT {
            throw_msg("System.ArgumentException", &format!("Accessed JValue of type {} with a string key; JObject expected.", token_type_name(self.kind)));
        }
        let Some(k) = key.get() else { throw_msg("System.ArgumentNullException", "Value cannot be null.\r\nParameter name: propertyName") };
        match self.children.iter().find(|p| r(**p).name == k) {
            Some(p) => r(*p).children.first().copied().unwrap_or(Gc::NULL),
            None => Gc::NULL,
        }
    }
    /// `JObject.TryGetValue(name, out value)`
    pub fn try_get_value(&self, key: Str, out: &mut Gc<JToken>) -> bool {
        let v = self.get(key);
        *out = v;
        !v.is_null()
    }
    /// `JObject[string] = value`
    pub fn set(this: Gc<JToken>, key: Str, value: Gc<JToken>) {
        let me = this;
        let this = r(this);
        let Some(k) = key.get() else { throw_msg("System.ArgumentNullException", "Value cannot be null.\r\nParameter name: propertyName") };
        match this.children.iter().position(|p| r(*p).name == k) {
            Some(i) => {
                let p = this.children[i];
                if let Some(old) = r(p).children.pop() {
                    r(old).parent = Gc::NULL;
                }
                Self::attach(p, value);
            }
            None => {
                let p = Self::new_property(key, value);
                Self::attach(me, p);
            }
        }
    }
    /// `JObject.Add(name, value)` (throws on a duplicate) / `JObject.Add(JProperty)`
    pub fn add_property(this: Gc<JToken>, key: Str, value: Gc<JToken>) {
        let k = key.as_str();
        if r(this).children.iter().any(|p| r(*p).name == k) {
            throw_msg("System.ArgumentException", &format!("Can not add property {k} to Newtonsoft.Json.Linq.JObject. Property with the same name already exists on object."));
        }
        let p = Self::new_property(key, value);
        Self::attach(this, p);
    }
    /// `JArray.Add(item)` / `JContainer.Add`
    pub fn add(this: Gc<JToken>, item: Gc<JToken>) {
        match r(this).kind {
            T_ARRAY | T_CONSTRUCTOR => Self::attach(this, item),
            T_OBJECT => {
                let it = r(item);
                if it.kind == T_PROPERTY {
                    let key = Str::new(&it.name);
                    let v = it.children.first().copied().unwrap_or(Gc::NULL);
                    Self::add_property(this, key, v);
                } else {
                    throw_msg("System.ArgumentException", &format!("Can not add {} to Newtonsoft.Json.Linq.JObject.", clr_name(it.kind)));
                }
            }
            _ => throw_msg("System.InvalidOperationException", &format!("Cannot add or remove items from {}.", clr_type(r(this).kind))),
        }
    }
    /// `JArray[int]`
    pub fn get_at(&self, i: i32) -> Gc<JToken> {
        if self.kind != T_ARRAY && self.kind != T_CONSTRUCTOR {
            throw_msg("System.ArgumentException", &format!("Accessed {} with an int key; JArray expected.", clr_type(self.kind)));
        }
        match usize::try_from(i).ok().and_then(|i| self.children.get(i)) {
            Some(c) => *c,
            None => throw_msg("System.ArgumentOutOfRangeException", "Index was out of range. Must be non-negative and less than the size of the collection.\r\nParameter name: index"),
        }
    }
    pub fn properties(&self) -> Vec<Gc<JToken>> {
        self.children.clone()
    }
    /// `JProperty.Name`
    pub fn property_name(&self) -> Str {
        Str::new(&self.name)
    }
    /// `JProperty.Value`
    pub fn property_value(&self) -> Gc<JToken> {
        self.children.first().copied().unwrap_or(Gc::NULL)
    }
    /// `JValue.Value` as a scalar.
    pub fn scalar(&self) -> &Scalar {
        &self.value
    }
    /// `Path`
    pub fn path(&self) -> Str {
        let mut parts: Vec<String> = Vec::new();
        let mut cur: *const JToken = self;
        loop {
            // SAFETY: parents are live nodes
            let n = unsafe { &*cur };
            let p = n.parent;
            if p.is_null() {
                break;
            }
            let par = r(p);
            match par.kind {
                T_PROPERTY => {}
                T_ARRAY | T_CONSTRUCTOR => {
                    let i = par.children.iter().position(|c| c.ptr() as *const JToken == cur).unwrap_or(0);
                    parts.push(format!("[{i}]"));
                }
                T_OBJECT => parts.push(n.name.clone()),
                _ => {}
            }
            cur = p.ptr();
        }
        parts.reverse();
        let mut out = String::new();
        for (i, p) in parts.iter().enumerate() {
            if !p.starts_with('[') && i > 0 {
                out.push('.');
            }
            out.push_str(p);
        }
        Str::new(&out)
    }
}

/// Name of the CLR type behind a token kind (`GetType().Name`).
pub fn clr_type(kind: i32) -> &'static str {
    match kind {
        T_OBJECT => "Newtonsoft.Json.Linq.JObject",
        T_ARRAY => "Newtonsoft.Json.Linq.JArray",
        T_CONSTRUCTOR => "Newtonsoft.Json.Linq.JConstructor",
        T_PROPERTY => "Newtonsoft.Json.Linq.JProperty",
        _ => "Newtonsoft.Json.Linq.JValue",
    }
}
fn clr_name(kind: i32) -> &'static str {
    clr_type(kind)
}
pub fn token_type_name(kind: i32) -> &'static str {
    match kind {
        T_OBJECT => "JObject",
        T_ARRAY => "JArray",
        T_CONSTRUCTOR => "JConstructor",
        T_PROPERTY => "JProperty",
        _ => "JValue",
    }
}
fn token_name(t: Gc<JToken>) -> String {
    match r(t).kind {
        T_OBJECT => "StartObject".into(),
        T_ARRAY => "StartArray".into(),
        T_PROPERTY => "PropertyName".into(),
        _ => "String".into(),
    }
}

impl From<DateVal> for Scalar {
    fn from(d: DateVal) -> Scalar {
        Scalar::Date(d)
    }
}

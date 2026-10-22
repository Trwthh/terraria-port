//! Minimal JSON reader for the reference dumps (`refdata/*.json`). Numbers keep their source text, because
//! `tools/refdump` printed floats with "R" (round-trip) formatting and the comparison must be exact.

#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    /// The number exactly as written in the file.
    Num(String),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(m) => m.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn arr(&self) -> &[Json] {
        match self {
            Json::Arr(a) => a,
            _ => panic!("not an array"),
        }
    }
    pub fn obj(&self) -> &[(String, Json)] {
        match self {
            Json::Obj(m) => m,
            _ => panic!("not an object"),
        }
    }
    pub fn int(&self) -> i64 {
        match self {
            Json::Num(n) => n.parse().expect("integer"),
            _ => panic!("not a number"),
        }
    }
    pub fn text(&self) -> String {
        match self {
            Json::Null => "null".to_owned(),
            Json::Bool(b) => b.to_string(),
            Json::Num(n) => n.clone(),
            Json::Str(s) => format!("{s:?}"),
            Json::Arr(a) => format!("[{}]", a.iter().map(|x| x.text()).collect::<Vec<_>>().join(",")),
            Json::Obj(m) => format!("{{{}}}", m.iter().map(|(k, v)| format!("{k}:{}", v.text())).collect::<Vec<_>>().join(",")),
        }
    }
}

pub fn parse(text: &str) -> Json {
    let b = text.as_bytes();
    let mut p = Parser { b, i: 0 };
    let v = p.value();
    p.ws();
    assert_eq!(p.i, b.len(), "trailing data in JSON");
    v
}

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\n' | b'\r' | b'\t') {
            self.i += 1;
        }
    }
    fn eat(&mut self, c: u8) {
        self.ws();
        assert_eq!(self.b[self.i], c, "JSON: expected {:?} at byte {}", c as char, self.i);
        self.i += 1;
    }
    fn value(&mut self) -> Json {
        self.ws();
        match self.b[self.i] {
            b'{' => {
                self.i += 1;
                let mut m = Vec::new();
                self.ws();
                if self.b[self.i] == b'}' {
                    self.i += 1;
                    return Json::Obj(m);
                }
                loop {
                    self.ws();
                    if self.b[self.i] == b'}' {
                        // trailing comma (the game's localisation files have them)
                        self.i += 1;
                        return Json::Obj(m);
                    }
                    let k = self.string();
                    self.eat(b':');
                    let v = self.value();
                    m.push((k, v));
                    self.ws();
                    if self.b[self.i] == b',' {
                        self.i += 1;
                    } else {
                        self.eat(b'}');
                        return Json::Obj(m);
                    }
                }
            }
            b'[' => {
                self.i += 1;
                let mut a = Vec::new();
                self.ws();
                if self.b[self.i] == b']' {
                    self.i += 1;
                    return Json::Arr(a);
                }
                loop {
                    a.push(self.value());
                    self.ws();
                    if self.b[self.i] == b',' {
                        self.i += 1;
                    } else {
                        self.eat(b']');
                        return Json::Arr(a);
                    }
                }
            }
            b'"' => Json::Str(self.string()),
            b't' => {
                self.i += 4;
                Json::Bool(true)
            }
            b'f' => {
                self.i += 5;
                Json::Bool(false)
            }
            b'n' => {
                self.i += 4;
                Json::Null
            }
            _ => {
                let s = self.i;
                while self.i < self.b.len() && matches!(self.b[self.i], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') {
                    self.i += 1;
                }
                assert!(self.i > s, "JSON: unexpected byte at {}", s);
                Json::Num(String::from_utf8(self.b[s..self.i].to_vec()).expect("utf8"))
            }
        }
    }
    fn string(&mut self) -> String {
        assert_eq!(self.b[self.i], b'"');
        self.i += 1;
        let mut out: Vec<u16> = Vec::new();
        let mut raw: Vec<u8> = Vec::new();
        let flush = |raw: &mut Vec<u8>, out: &mut Vec<u16>| {
            if !raw.is_empty() {
                out.extend(std::str::from_utf8(raw).expect("utf8").encode_utf16());
                raw.clear();
            }
        };
        loop {
            let c = self.b[self.i];
            self.i += 1;
            match c {
                b'"' => break,
                b'\\' => {
                    flush(&mut raw, &mut out);
                    let e = self.b[self.i];
                    self.i += 1;
                    match e {
                        b'n' => out.push(b'\n' as u16),
                        b'r' => out.push(b'\r' as u16),
                        b't' => out.push(b'\t' as u16),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'u' => {
                            let h = std::str::from_utf8(&self.b[self.i..self.i + 4]).expect("hex");
                            out.push(u16::from_str_radix(h, 16).expect("hex"));
                            self.i += 4;
                        }
                        other => out.push(other as u16),
                    }
                }
                _ => raw.push(c),
            }
        }
        flush(&mut raw, &mut out);
        String::from_utf16_lossy(&out)
    }
}

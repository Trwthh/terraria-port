//! `CsvHelper.CsvReader` (version 2.0, the game's copy) for the one place the game uses it:
//! `LanguageManager.LoadLanguageFromFileTextCsv` (`Configuration.HasHeaderRecord = true`, `ReadHeader()`,
//! `FieldHeaders`, `Read()`, `CurrentRecord`) on community translation files.
//!
//! Behaviour checked against the dll (tests/dotnet/csv_probe.ps1, 18 inputs): fields separated by `,`; a field in
//! double quotes may contain separators and line breaks, `""` is a quote; a quote inside an unquoted field is kept;
//! an unterminated quote runs to the end of the input; lines end at CRLF, LF or CR; a truly empty line is skipped (a
//! line of spaces is a record); white space and a byte order mark are not trimmed; **a record shorter than the header
//! is padded with null fields up to the header's length, a longer one is returned as is**; an input without any line
//! makes `ReadHeader()` throw `CsvReaderException: No header record was found.` (and so does `Read()` before it).

use terraria_gen_rt::rt::{self, Arr, Gc, Marker, Str, Trace};

use crate::gfx::states::throw;

pub struct CsvReader {
    text: Vec<char>,
    pos: usize,
    has_header_record: bool,
    header: Option<Vec<Str>>,
    current: Vec<Str>,
    disposed: bool,
}
// SAFETY: every Str handle is reported.
unsafe impl Trace for CsvReader {
    unsafe fn trace(&self, m: &mut Marker) {
        for s in self.header.iter().flatten().chain(self.current.iter()) {
            s.trace(m);
        }
    }
}

fn csv_error(msg: &str) -> ! {
    rt::throw(rt::exc("CsvHelper.CsvReaderException", Str::new(msg)))
}

impl CsvReader {
    /// `new CsvReader(TextReader)`: the text is read at once.
    pub fn new_from_text(text: Str) -> Gc<CsvReader> {
        let t = text.get().unwrap_or_else(|| throw("System.ArgumentNullException", "reader"));
        rt::alloc_init(CsvReader { text: t.chars().collect(), pos: 0, has_header_record: true, header: None, current: Vec::new(), disposed: false })
    }
    /// `new CsvReader(reader)` for the host's `StreamReader` / `StringReader`.
    pub fn new_gc(reader: Gc<crate::sys::io::StreamReader>) -> Gc<CsvReader> {
        // SAFETY: a live reader object
        let t = unsafe { (*reader.ptr()).read_to_end() };
        Self::new_from_text(t)
    }

    /// The next non-empty line as fields; `None` at the end of the input.
    fn next_record(&mut self) -> Option<Vec<Str>> {
        loop {
            if self.pos >= self.text.len() {
                return None;
            }
            let mut fields: Vec<String> = Vec::new();
            let mut cur = String::new();
            let mut was_quoted = false;
            let mut any = false;
            loop {
                let Some(&c) = self.text.get(self.pos) else {
                    fields.push(std::mem::take(&mut cur));
                    break;
                };
                self.pos += 1;
                match c {
                    '"' if cur.is_empty() && !was_quoted => {
                        // a quoted field: up to the closing quote, `""` is one quote
                        any = true;
                        was_quoted = true;
                        loop {
                            match self.text.get(self.pos) {
                                None => break,
                                Some('"') => {
                                    if self.text.get(self.pos + 1) == Some(&'"') {
                                        cur.push('"');
                                        self.pos += 2;
                                    } else {
                                        self.pos += 1;
                                        break;
                                    }
                                }
                                Some(&ch) => {
                                    cur.push(ch);
                                    self.pos += 1;
                                }
                            }
                        }
                    }
                    ',' => {
                        any = true;
                        fields.push(std::mem::take(&mut cur));
                        was_quoted = false;
                    }
                    '\r' | '\n' => {
                        if c == '\r' && self.text.get(self.pos) == Some(&'\n') {
                            self.pos += 1;
                        }
                        if !any && cur.is_empty() && fields.is_empty() {
                            // an empty line
                            fields.clear();
                            break;
                        }
                        fields.push(std::mem::take(&mut cur));
                        break;
                    }
                    c => {
                        any = true;
                        cur.push(c);
                    }
                }
            }
            if fields.is_empty() {
                continue;
            }
            return Some(fields.iter().map(|f| Str::new(f)).collect());
        }
    }

    fn check(&self) {
        if self.disposed {
            throw("System.ObjectDisposedException", "Cannot access a disposed object. Object name: 'CsvReader'.");
        }
    }
    /// `Configuration.HasHeaderRecord = value`
    pub fn set_has_header_record(&mut self, v: bool) {
        self.has_header_record = v;
    }
    pub fn has_header_record(&self) -> bool {
        self.has_header_record
    }
    /// `ReadHeader()`: reads the first record as the header; `CsvReaderException` when there is none.
    pub fn read_header(&mut self) -> bool {
        self.check();
        if self.header.is_some() {
            csv_error("Field headers have already been read.");
        }
        if !self.has_header_record {
            csv_error("Configuration.HasHeaderRecord is false.");
        }
        match self.next_record() {
            Some(h) => {
                self.header = Some(h);
                true
            }
            None => csv_error("No header record was found."),
        }
    }
    /// `FieldHeaders`
    pub fn field_headers(&self) -> Arr<Str> {
        match &self.header {
            Some(h) => Arr::from_slice(h),
            None => csv_error("You must call ReadHeader or Read before accessing the field headers."),
        }
    }
    /// `Read()`: false at the end. With a header record configured, the first call reads the header if `ReadHeader`
    /// was not called.
    pub fn read(&mut self) -> bool {
        self.check();
        if self.has_header_record && self.header.is_none() {
            self.read_header();
        }
        match self.next_record() {
            Some(mut r) => {
                let n = self.header.as_ref().map_or(0, |h| h.len());
                while r.len() < n {
                    r.push(Str::NULL);
                }
                self.current = r;
                true
            }
            None => {
                self.current.clear();
                false
            }
        }
    }
    /// `CurrentRecord`
    pub fn current_record(&self) -> Arr<Str> {
        if self.current.is_empty() {
            return Arr::NULL;
        }
        Arr::from_slice(&self.current)
    }
    pub fn dispose(&mut self) {
        self.disposed = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::tests::b64d;

    fn esc(s: &str) -> String {
        s.replace('\r', "\\r").replace('\n', "\\n")
    }
    fn field(s: Str) -> String {
        if s.is_null() {
            "<null>".into()
        } else {
            esc(s.as_str())
        }
    }

    /// The same probe loop as csv_probe.ps1, rendered to the same text.
    fn run(text: &str) -> String {
        let r = CsvReader::new_from_text(Str::new(text));
        // SAFETY: a live object
        let r = unsafe { &mut *r.ptr() };
        match rt::run(|| {
            let h = r.read_header();
            let mut out = format!("header={} fields={}", if h { "True" } else { "False" }, r.field_headers().as_slice().iter().map(|f| format!("[{}]", field(*f))).collect::<String>());
            let mut n = 0;
            while r.read() {
                n += 1;
                let rec = r.current_record();
                out += &format!(" | rec{n}({}):{}", rec.len(), rec.as_slice().iter().map(|f| format!("[{}]", field(*f))).collect::<String>());
            }
            out
        }) {
            Ok(s) => s,
            Err(e) => format!(" ERR {}", e.replace("CsvHelper.", "")),
        }
    }

    #[test]
    fn csv_reader_matches_csvhelper() {
        let t = std::fs::read_to_string(crate::manifest_dir().join("tests").join("dotnet").join("csv.expected")).expect("csv.expected");
        let mut n = 0;
        for line in t.lines() {
            let p: Vec<&str> = line.split('\t').collect();
            let (name, input, expected) = (p[0], b64d(p[1]), b64d(p[2]));
            let got = run(&input);
            // the exception type name is printed as CsvReaderException by the script; ours carries the namespace
            let got = got.replace(" ERR CsvReaderException: ", " ERR CsvReaderException: ");
            assert_eq!(got, expected.replace("ERR CsvReaderException: ", "ERR CsvReaderException: "), "{name}: {input:?}");
            n += 1;
        }
        assert_eq!(n, 18);
    }

    /// What `LoadLanguageFromFileTextCsv` does with the records.
    #[test]
    fn translation_file_shape() {
        let r = CsvReader::new_from_text(Str::new("Key,Translation\nItemName.Foo,\"Foo, bar\"\nItemName.Baz,\n"));
        let r = unsafe { &mut *r.ptr() };
        assert!(r.read_header());
        assert_eq!(r.field_headers().len(), 2);
        assert!(r.read());
        assert_eq!(r.current_record().as_slice()[1].as_str(), "Foo, bar");
        assert!(r.read());
        assert_eq!(r.current_record().as_slice()[1].as_str(), "");
        assert!(!r.read());
    }
}

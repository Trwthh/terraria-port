//! `Newtonsoft.Json.Bson.BsonWriter` / `BsonReader` for DOM tokens: the format of `Preferences` with `useBson` and of
//! `achievements.dat` (`AchievementManager`, inside the AES layer of `sys::crypto`).
//!
//! Writer rules (checked against the dll, tests/dotnet/json_probe.ps1 writes `bson_cases.expected`): the root is a
//! document (object) or an array (a document with keys `"0"`, `"1"`...); integers are always written as int64
//! (`0x12`) because the DOM holds `long`; floats as double (`0x01`), strings as `0x02`, `true`/`false` as `0x08`,
//! null as `0x0A`, dates as `0x09` (milliseconds since 1970 of the tick count, no zone conversion), bytes as binary
//! `0x05` subtype 0. Reader: int32 and int64 become `Integer`, double `Float`, date `Date` (kind Utc), `0x06`
//! `Undefined`.

use terraria_gen_rt::rt::Gc;

use super::reader::{DateVal, JsonError, Scalar, Zone};
use super::reader::ErrKind;
use super::token::*;

const EPOCH_TICKS: i64 = 621_355_968_000_000_000;

fn tok(t: Gc<JToken>) -> &'static JToken {
    // SAFETY: a live node
    unsafe { &*t.ptr() }
}

fn cstring(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(s.as_bytes());
    out.push(0);
}

fn element(out: &mut Vec<u8>, name: &str, t: &JToken) {
    let ty_pos = out.len();
    out.push(0);
    cstring(out, name);
    let ty = match t.kind {
        T_OBJECT => {
            document(out, t, false);
            0x03
        }
        T_ARRAY | T_CONSTRUCTOR => {
            document(out, t, true);
            0x04
        }
        T_PROPERTY => {
            // a property as an item: its value
            out.truncate(ty_pos);
            match t.children.first() {
                Some(v) => element(out, name, tok(*v)),
                None => element(out, name, tok(JToken::new_value(Scalar::Null))),
            }
            return;
        }
        _ => match &t.value {
            Scalar::Null | Scalar::Undefined => 0x0A,
            Scalar::Bool(b) => {
                out.push(*b as u8);
                0x08
            }
            Scalar::Int(i) if t.int32 => {
                out.extend_from_slice(&(*i as i32).to_le_bytes());
                0x10
            }
            Scalar::Int(i) => {
                out.extend_from_slice(&i.to_le_bytes());
                0x12
            }
            Scalar::Big(_) => 0x12,
            Scalar::Float(f) => {
                out.extend_from_slice(&f.to_le_bytes());
                0x01
            }
            Scalar::Float32(f) => {
                out.extend_from_slice(&(*f as f64).to_le_bytes());
                0x01
            }
            Scalar::Str(s) | Scalar::Raw(s) | Scalar::Comment(s) => {
                out.extend_from_slice(&((s.len() + 1) as i32).to_le_bytes());
                cstring(out, s);
                0x02
            }
            Scalar::Date(d) => {
                out.extend_from_slice(&((d.ticks - EPOCH_TICKS).div_euclid(10_000)).to_le_bytes());
                0x09
            }
            Scalar::Bytes(b) => {
                out.extend_from_slice(&(b.len() as i32).to_le_bytes());
                out.push(0);
                out.extend_from_slice(b);
                0x05
            }
        },
    };
    out[ty_pos] = ty;
}

fn document(out: &mut Vec<u8>, t: &JToken, array: bool) {
    let start = out.len();
    out.extend_from_slice(&[0; 4]);
    for (i, c) in t.children.iter().enumerate() {
        let c = tok(*c);
        if array {
            element(out, &i.to_string(), c);
        } else {
            element(out, &c.name, c);
        }
    }
    out.push(0);
    let len = (out.len() - start) as i32;
    out[start..start + 4].copy_from_slice(&len.to_le_bytes());
}

/// `token.WriteTo(new BsonWriter(stream))`. The root must be an object or an array.
pub fn write(root: Gc<JToken>) -> Result<Vec<u8>, String> {
    let t = tok(root);
    let mut out = Vec::new();
    match t.kind {
        T_OBJECT => document(&mut out, t, false),
        T_ARRAY => document(&mut out, t, true),
        _ => return Err("BsonWriter: the root must be an object or an array".into()),
    }
    Ok(out)
}

struct Cur<'a> {
    b: &'a [u8],
    p: usize,
}
impl Cur<'_> {
    fn err(&self, m: &str) -> JsonError {
        JsonError { kind: ErrKind::Reader, message: format!("{m} Path '', line 0, position 0.") }
    }
    fn take(&mut self, n: usize) -> Result<&[u8], JsonError> {
        let s = self.b.get(self.p..self.p + n).ok_or_else(|| self.err("Unexpected end of stream while reading BSON."))?;
        self.p += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, JsonError> {
        Ok(self.take(1)?[0])
    }
    fn i32(&mut self) -> Result<i32, JsonError> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().expect("4")))
    }
    fn i64(&mut self) -> Result<i64, JsonError> {
        Ok(i64::from_le_bytes(self.take(8)?.try_into().expect("8")))
    }
    fn cstr(&mut self) -> Result<String, JsonError> {
        let start = self.p;
        while self.u8()? != 0 {}
        Ok(String::from_utf8_lossy(&self.b[start..self.p - 1]).into_owned())
    }
}

fn read_doc(c: &mut Cur, array: bool) -> Result<Gc<JToken>, JsonError> {
    let size = c.i32()?;
    let end = c.p + (size as usize).saturating_sub(4);
    let node = if array { JToken::new_array() } else { JToken::new_object() };
    loop {
        let ty = c.u8()?;
        if ty == 0 {
            break;
        }
        let name = c.cstr()?;
        let v: Gc<JToken> = match ty {
            0x01 => JToken::new_value(Scalar::Float(f64::from_le_bytes(c.take(8)?.try_into().expect("8")))),
            0x02 => {
                let n = c.i32()? as usize;
                let s = c.take(n)?;
                JToken::new_value(Scalar::Str(String::from_utf8_lossy(&s[..n.saturating_sub(1)]).into_owned()))
            }
            0x03 => read_doc(c, false)?,
            0x04 => read_doc(c, true)?,
            0x05 => {
                let n = c.i32()? as usize;
                let _subtype = c.u8()?;
                JToken::new_value(Scalar::Bytes(c.take(n)?.to_vec()))
            }
            0x06 | 0x0A => JToken::new_value(if ty == 0x06 { Scalar::Undefined } else { Scalar::Null }),
            0x08 => JToken::new_value(Scalar::Bool(c.u8()? != 0)),
            0x09 => {
                let ms = c.i64()?;
                JToken::new_value(Scalar::Date(DateVal { ticks: EPOCH_TICKS + ms * 10_000, zone: Zone::Utc }))
            }
            0x10 => JToken::new_value(Scalar::Int(c.i32()? as i64)),
            0x12 => JToken::new_value(Scalar::Int(c.i64()?)),
            t => return Err(c.err(&format!("Unexpected BsonType value encountered: {t}."))),
        };
        if array {
            JToken::add(node, v);
        } else {
            JToken::add_property(node, terraria_gen_rt::rt::Str::new(&name), v);
        }
    }
    if c.p != end {
        return Err(c.err("BSON document size does not match its content."));
    }
    Ok(node)
}

/// `new BsonReader(stream)` + `JToken.ReadFrom` (`ReadRootValueAsArray` = false: the root is an object).
pub fn read(bytes: &[u8], root_is_array: bool) -> Result<Gc<JToken>, JsonError> {
    let mut c = Cur { b: bytes, p: 0 };
    read_doc(&mut c, root_is_array)
}

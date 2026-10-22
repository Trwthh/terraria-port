//! `System.IO.BinaryReader` / `BinaryWriter` semantics used by `WorldFile.cs`: little-endian primitives,
//! booleans as one byte, strings as a 7-bit encoded UTF-8 byte count followed by the bytes.

use super::error::WldError;

pub(crate) struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
    /// Section name used in errors.
    pub section: &'static str,
}

impl<'a> Reader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Reader { buf, pos: 0, section: "FileFormatHeader" }
    }
    pub fn pos(&self) -> usize {
        self.pos
    }
    pub fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }
    pub fn invalid(&self, reason: impl Into<String>) -> WldError {
        WldError::Invalid { section: self.section, offset: self.pos, reason: reason.into() }
    }
    pub fn invalid_at(&self, offset: usize, reason: impl Into<String>) -> WldError {
        WldError::Invalid { section: self.section, offset, reason: reason.into() }
    }
    pub fn take(&mut self, n: usize) -> Result<&'a [u8], WldError> {
        if self.remaining() < n {
            return Err(WldError::UnexpectedEof { section: self.section, offset: self.pos, wanted: n });
        }
        let s = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], WldError> {
        let s = self.take(N)?;
        let mut a = [0u8; N];
        a.copy_from_slice(s);
        Ok(a)
    }
    pub fn u8(&mut self) -> Result<u8, WldError> {
        Ok(self.array::<1>()?[0])
    }
    pub fn i16(&mut self) -> Result<i16, WldError> {
        Ok(i16::from_le_bytes(self.array()?))
    }
    pub fn u16(&mut self) -> Result<u16, WldError> {
        Ok(u16::from_le_bytes(self.array()?))
    }
    pub fn i32(&mut self) -> Result<i32, WldError> {
        Ok(i32::from_le_bytes(self.array()?))
    }
    pub fn u32(&mut self) -> Result<u32, WldError> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    pub fn i64(&mut self) -> Result<i64, WldError> {
        Ok(i64::from_le_bytes(self.array()?))
    }
    pub fn u64(&mut self) -> Result<u64, WldError> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    pub fn f32(&mut self) -> Result<f32, WldError> {
        Ok(f32::from_le_bytes(self.array()?))
    }
    pub fn f64(&mut self) -> Result<f64, WldError> {
        Ok(f64::from_le_bytes(self.array()?))
    }
    /// `BinaryReader.ReadBoolean` accepts any non-zero byte as true. A value other than 0/1 would not
    /// survive a write, so it is rejected here instead of being normalised silently.
    pub fn bool(&mut self) -> Result<bool, WldError> {
        let at = self.pos;
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            v => Err(self.invalid_at(at, format!("boolean byte {v}, expected 0 or 1"))),
        }
    }
    /// `Read7BitEncodedInt` (at most 5 bytes).
    pub fn int7(&mut self) -> Result<i32, WldError> {
        let at = self.pos;
        let mut result: u32 = 0;
        for i in 0..5 {
            let b = self.u8()?;
            result |= ((b & 0x7F) as u32) << (7 * i);
            if b & 0x80 == 0 {
                if i == 4 && b > 0x0F {
                    break;
                }
                return Ok(result as i32);
            }
        }
        Err(self.invalid_at(at, "bad 7-bit encoded integer"))
    }
    /// `BinaryReader.ReadString`.
    pub fn string(&mut self) -> Result<String, WldError> {
        let at = self.pos;
        let n = self.int7()?;
        if n < 0 {
            return Err(self.invalid_at(at, format!("negative string length {n}")));
        }
        let bytes = self.take(n as usize)?;
        match std::str::from_utf8(bytes) {
            Ok(s) => Ok(s.to_owned()),
            Err(_) => Err(self.invalid_at(at, "string is not valid UTF-8")),
        }
    }
    /// A signed 32-bit count followed by that many records of at least `min_record` bytes each.
    /// Refuses counts the rest of the file cannot hold (the original would hit the end of the stream).
    pub fn count(&mut self, min_record: usize, what: &str) -> Result<usize, WldError> {
        let at = self.pos;
        let n = self.i32()?;
        self.checked_count(at, n as i64, min_record, what)
    }
    pub fn checked_count(&self, at: usize, n: i64, min_record: usize, what: &str) -> Result<usize, WldError> {
        if n < 0 {
            return Err(self.invalid_at(at, format!("negative {what} count {n}")));
        }
        let n = n as usize;
        if n.saturating_mul(min_record) > self.remaining() {
            return Err(self.invalid_at(at, format!("{what} count {n} exceeds the remaining file size")));
        }
        Ok(n)
    }
}

#[derive(Default)]
pub(crate) struct Writer {
    pub buf: Vec<u8>,
}

impl Writer {
    pub fn pos(&self) -> usize {
        self.buf.len()
    }
    pub fn bytes(&mut self, b: &[u8]) {
        self.buf.extend_from_slice(b);
    }
    pub fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }
    pub fn i16(&mut self, v: i16) {
        self.bytes(&v.to_le_bytes());
    }
    pub fn u16(&mut self, v: u16) {
        self.bytes(&v.to_le_bytes());
    }
    pub fn i32(&mut self, v: i32) {
        self.bytes(&v.to_le_bytes());
    }
    pub fn u32(&mut self, v: u32) {
        self.bytes(&v.to_le_bytes());
    }
    pub fn i64(&mut self, v: i64) {
        self.bytes(&v.to_le_bytes());
    }
    pub fn u64(&mut self, v: u64) {
        self.bytes(&v.to_le_bytes());
    }
    pub fn f32(&mut self, v: f32) {
        self.bytes(&v.to_le_bytes());
    }
    pub fn f64(&mut self, v: f64) {
        self.bytes(&v.to_le_bytes());
    }
    pub fn bool(&mut self, v: bool) {
        self.u8(v as u8);
    }
    /// `BinaryWriter.Write(string)`.
    pub fn string(&mut self, s: &str) {
        let mut n = s.len() as u32;
        loop {
            if n >= 0x80 {
                self.u8((n as u8) | 0x80);
                n >>= 7;
            } else {
                self.u8(n as u8);
                break;
            }
        }
        self.bytes(s.as_bytes());
    }
    /// `BinaryWriter.Write(int)` of a collection length, refusing lengths that do not fit.
    pub fn len_i32(&mut self, section: &'static str, n: usize, what: &str) -> Result<(), WldError> {
        match i32::try_from(n) {
            Ok(v) => {
                self.i32(v);
                Ok(())
            }
            Err(_) => Err(WldError::Unrepresentable { section, reason: format!("{what} count {n} does not fit in i32") }),
        }
    }
    /// `BinaryWriter.Write(short)` of a collection length (`SaveChests`/`SaveSigns` count).
    pub fn len_i16(&mut self, section: &'static str, n: usize, what: &str) -> Result<(), WldError> {
        match i16::try_from(n) {
            Ok(v) => {
                self.i16(v);
                Ok(())
            }
            Err(_) => Err(WldError::Unrepresentable { section, reason: format!("{what} count {n} does not fit in i16") }),
        }
    }
    pub fn patch_i32(&mut self, at: usize, v: i32) {
        self.buf[at..at + 4].copy_from_slice(&v.to_le_bytes());
    }
}

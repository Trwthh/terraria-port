//! `System.Security.Cryptography` as the game uses it: `RijndaelManaged` (AES, CBC) through `CryptoStream`, and
//! `RandomNumberGenerator`.
//!
//! Player files (`Player.cs` `InternalSavePlayerFile` / `LoadPlayer`) are written with `new RijndaelManaged()`
//! defaults (CBC, PKCS7, 128-bit blocks) and key = IV = `UnicodeEncoding.GetBytes("h3y_gUyZ")` (16 bytes, so
//! AES-128), and read back with `Padding = PaddingMode.None`. Expected ciphertexts: tests/dotnet/system_probe.ps1.
//!
//! `CryptoStream` is a `Stream` in C#, and the runtime has ONE Rust type for every stream (`rt::io::Stream`, a
//! memory buffer), so:
//! - `CryptoStreamMode.Read`: the rest of the inner stream is decrypted when the `CryptoStream` is created and the
//!   result is a read-only memory stream. Sequential reads cannot tell the difference.
//! - `CryptoStreamMode.Write`: a memory stream; `FlushFinalBlock()` encrypts what was written and writes it to the
//!   inner stream. The game calls `FlushFinalBlock` explicitly where it writes player files. A `CryptoStream` that
//!   is only disposed (never `FlushFinalBlock`ed) writes NOTHING here, where .NET flushes in `Dispose`: that needs
//!   a close hook on `rt::io::Stream` (REQUESTS.md).


use aes::cipher::{generic_array::GenericArray, BlockDecrypt, BlockEncrypt, KeyInit};
use terraria_gen_rt::host;
use terraria_gen_rt::rt::{self, io::Stream, Arr, Gc};

use crate::gfx::states::{plain_trace, throw};

/// `PaddingMode`
pub const PADDING_NONE: i32 = 1;
pub const PADDING_PKCS7: i32 = 2;
pub const PADDING_ZEROS: i32 = 3;
/// `CipherMode.CBC`
pub const CIPHER_MODE_CBC: i32 = 1;

enum Aes {
    K128(aes::Aes128),
    K192(aes::Aes192),
    K256(aes::Aes256),
}
impl Aes {
    fn new(key: &[u8]) -> Aes {
        match key.len() {
            16 => Aes::K128(aes::Aes128::new(GenericArray::from_slice(key))),
            24 => Aes::K192(aes::Aes192::new(GenericArray::from_slice(key))),
            32 => Aes::K256(aes::Aes256::new(GenericArray::from_slice(key))),
            _ => throw("System.Security.Cryptography.CryptographicException", "Specified key is not a valid size for this algorithm."),
        }
    }
    fn encrypt(&self, block: &mut [u8; 16]) {
        let b = GenericArray::from_mut_slice(block);
        match self {
            Aes::K128(c) => c.encrypt_block(b),
            Aes::K192(c) => c.encrypt_block(b),
            Aes::K256(c) => c.encrypt_block(b),
        }
    }
    fn decrypt(&self, block: &mut [u8; 16]) {
        let b = GenericArray::from_mut_slice(block);
        match self {
            Aes::K128(c) => c.decrypt_block(b),
            Aes::K192(c) => c.decrypt_block(b),
            Aes::K256(c) => c.decrypt_block(b),
        }
    }
}

fn iv16(iv: &[u8]) -> [u8; 16] {
    match iv.try_into() {
        Ok(v) => v,
        Err(_) => throw("System.Security.Cryptography.CryptographicException", "Specified initialization vector (IV) does not match the block size for this algorithm."),
    }
}

/// AES-CBC encryption of a whole message with the given `PaddingMode`.
pub fn aes_cbc_encrypt(key: &[u8], iv: &[u8], plain: &[u8], padding: i32) -> Vec<u8> {
    let aes = Aes::new(key);
    let mut data = plain.to_vec();
    match padding {
        PADDING_PKCS7 => {
            let n = 16 - data.len() % 16;
            data.extend(std::iter::repeat(n as u8).take(n));
        }
        PADDING_ZEROS => {
            let n = (16 - data.len() % 16) % 16;
            data.extend(std::iter::repeat(0).take(n));
        }
        PADDING_NONE => {
            if data.len() % 16 != 0 {
                throw("System.Security.Cryptography.CryptographicException", "Length of the data to encrypt is invalid.");
            }
        }
        _ => {
            return host::unimplemented_value("System.Security.Cryptography.SymmetricAlgorithm.Padding other than None / PKCS7 / Zeros");
        }
    }
    let mut prev = iv16(iv);
    for chunk in data.chunks_exact_mut(16) {
        let mut b: [u8; 16] = (&*chunk).try_into().expect("16 bytes");
        for k in 0..16 {
            b[k] ^= prev[k];
        }
        aes.encrypt(&mut b);
        chunk.copy_from_slice(&b);
        prev = b;
    }
    data
}

/// AES-CBC decryption of a whole message. `None` / `Zeros`: the plaintext keeps its padding bytes (.NET does not
/// strip zeros either). A trailing partial block is an error with PKCS7 and is ignored with `None`... .NET throws
/// "Length of the data to decrypt is invalid" for both; so does this.
pub fn aes_cbc_decrypt(key: &[u8], iv: &[u8], cipher: &[u8], padding: i32) -> Vec<u8> {
    let aes = Aes::new(key);
    if cipher.len() % 16 != 0 {
        throw("System.Security.Cryptography.CryptographicException", "Length of the data to decrypt is invalid.");
    }
    let mut prev = iv16(iv);
    let mut out = Vec::with_capacity(cipher.len());
    for chunk in cipher.chunks_exact(16) {
        let c: [u8; 16] = chunk.try_into().expect("16 bytes");
        let mut b = c;
        aes.decrypt(&mut b);
        for k in 0..16 {
            b[k] ^= prev[k];
        }
        out.extend_from_slice(&b);
        prev = c;
    }
    match padding {
        PADDING_PKCS7 => {
            let n = out.last().copied().unwrap_or(0) as usize;
            if n == 0 || n > 16 || n > out.len() || out[out.len() - n..].iter().any(|b| *b as usize != n) {
                throw("System.Security.Cryptography.CryptographicException", "Padding is invalid and cannot be removed.");
            }
            out.truncate(out.len() - n);
        }
        PADDING_NONE | PADDING_ZEROS => {}
        _ => return host::unimplemented_value("System.Security.Cryptography.SymmetricAlgorithm.Padding other than None / PKCS7 / Zeros"),
    }
    out
}

/// `RijndaelManaged` (also `SymmetricAlgorithm`): the settings the game touches.
pub struct RijndaelManaged {
    pub padding: i32,
    pub mode: i32,
}
plain_trace!(RijndaelManaged, CryptoTransform);

impl RijndaelManaged {
    /// `new RijndaelManaged()`: CBC, PKCS7.
    pub fn new_gc() -> Gc<RijndaelManaged> {
        rt::alloc_init(RijndaelManaged { padding: PADDING_PKCS7, mode: CIPHER_MODE_CBC })
    }
    pub fn padding(&self) -> i32 {
        self.padding
    }
    pub fn set_padding(&mut self, v: i32) {
        self.padding = v;
    }
    pub fn set_mode(&mut self, v: i32) {
        if v != CIPHER_MODE_CBC {
            host::unimplemented("System.Security.Cryptography.SymmetricAlgorithm.Mode.set other than CipherMode.CBC (CBC is used)");
            return;
        }
        self.mode = v;
    }
    fn transform(&self, key: Arr<u8>, iv: Arr<u8>, encrypt: bool) -> Gc<CryptoTransform> {
        if key.is_null() {
            throw("System.ArgumentNullException", "rgbKey");
        }
        if iv.is_null() {
            throw("System.ArgumentNullException", "rgbIV");
        }
        Aes::new(key.as_slice());
        iv16(iv.as_slice());
        rt::alloc_init(CryptoTransform { key: key.as_slice().to_vec(), iv: iv.as_slice().to_vec(), encrypt, padding: self.padding })
    }
    /// `CreateEncryptor(rgbKey, rgbIV)`
    pub fn create_encryptor(&self, key: Arr<u8>, iv: Arr<u8>) -> Gc<CryptoTransform> {
        self.transform(key, iv, true)
    }
    /// `CreateDecryptor(rgbKey, rgbIV)`
    pub fn create_decryptor(&self, key: Arr<u8>, iv: Arr<u8>) -> Gc<CryptoTransform> {
        self.transform(key, iv, false)
    }
}

/// `ICryptoTransform` from `RijndaelManaged.CreateEncryptor / CreateDecryptor`.
pub struct CryptoTransform {
    key: Vec<u8>,
    iv: Vec<u8>,
    encrypt: bool,
    padding: i32,
}
impl CryptoTransform {
    /// `TransformFinalBlock` over a whole message.
    pub fn transform_all(&self, data: &[u8]) -> Vec<u8> {
        if self.encrypt {
            aes_cbc_encrypt(&self.key, &self.iv, data, self.padding)
        } else {
            aes_cbc_decrypt(&self.key, &self.iv, data, self.padding)
        }
    }
}

/// `Stream::tag` of a `CryptoStream`.
pub const CRYPTO_STREAM_TAG: u32 = 1;

/// `new CryptoStream(stream, transform, mode)`; `mode`: Read 0, Write 1. See the module docs.
///
/// Write mode: the stream collects what is written; the transform runs over all of it when the final block is
/// flushed (`FlushFinalBlock`, or `Dispose` / `Close`, which is what `using` does) and the result goes to the inner
/// stream. `Dispose` then closes the inner stream, as .NET's `CryptoStream.Dispose` does.
pub fn crypto_stream_new(stream: Gc<Stream>, transform: Gc<CryptoTransform>, mode: i32) -> Gc<Stream> {
    if stream.is_null() {
        throw("System.ArgumentNullException", "stream");
    }
    // SAFETY: `ptr()` checks null; live runtime objects.
    let (inner, t) = unsafe { (&mut *stream.ptr(), &*transform.ptr()) };
    match mode {
        0 => {
            if !inner.can_read() {
                throw("System.ArgumentException", "Stream was not readable.");
            }
            let mut rest = vec![0u8; (inner.length() - inner.position()).max(0) as usize];
            let n = inner.read_slice(&mut rest);
            rest.truncate(n);
            let cs = Stream::from_vec(t.transform_all(&rest));
            // SAFETY: fresh object.
            unsafe { (*cs.ptr()).set_wrapper(stream, CRYPTO_STREAM_TAG) };
            cs
        }
        1 => {
            if !inner.can_write() {
                throw("System.ArgumentException", "Stream was not writable.");
            }
            let hook = Box::new(move |bytes: &[u8]| {
                // SAFETY: both objects are kept alive by the crypto stream (`close_roots`) and are distinct from it.
                unsafe {
                    let out = (*transform.ptr()).transform_all(bytes);
                    (*stream.ptr()).write_slice(&out);
                }
            });
            let cs = Stream::new_sink(hook, vec![stream.up(), transform.up()]);
            // SAFETY: fresh object.
            unsafe { (*cs.ptr()).set_wrapper(stream, CRYPTO_STREAM_TAG) };
            cs
        }
        _ => throw("System.ArgumentException", "mode"),
    }
}

/// `CryptoStream.FlushFinalBlock()`: encrypts everything written and writes it to the inner stream. A second call
/// throws `NotSupportedException`, as in .NET.
pub fn crypto_stream_flush_final_block(crypto_stream: Gc<Stream>) {
    // SAFETY: `ptr()` checks null.
    let cs = unsafe { &mut *crypto_stream.ptr() };
    if !cs.has_close_hook() {
        throw("System.NotSupportedException", "FlushFinalBlock() method was called twice on a CryptoStream. It can only be called once.");
    }
    cs.finish();
}

/// `x is CryptoStream` for a stream object.
pub fn is_crypto_stream(stream: Gc<Stream>) -> bool {
    // SAFETY: a live runtime object when not null.
    !stream.is_null() && unsafe { (*stream.ptr()).tag() } == CRYPTO_STREAM_TAG
}

// ------------------------------------------------------------------------------------------------ random bytes

/// Bytes from the operating system's generator (`RandomNumberGenerator.GetBytes`, `Guid.NewGuid`).
pub fn os_random(buf: &mut [u8]) {
    #[cfg(windows)]
    {
        #[link(name = "advapi32")]
        extern "system" {
            #[link_name = "SystemFunction036"]
            fn rtl_gen_random(buffer: *mut u8, len: u32) -> u8;
        }
        // SAFETY: the buffer is valid for `len` bytes.
        let ok = unsafe { rtl_gen_random(buf.as_mut_ptr(), buf.len() as u32) };
        assert!(ok != 0, "RtlGenRandom failed");
    }
    // WASI: the host's random_get (in a browser the shim answers it with crypto.getRandomValues)
    #[cfg(target_os = "wasi")]
    {
        #[link(wasm_import_module = "wasi_snapshot_preview1")]
        extern "C" {
            fn random_get(buf: *mut u8, len: usize) -> u16;
        }
        // SAFETY: the buffer is valid for `len` bytes.
        let errno = unsafe { random_get(buf.as_mut_ptr(), buf.len()) };
        assert!(errno == 0, "random_get failed: errno {errno}");
    }
    #[cfg(not(any(windows, target_os = "wasi")))]
    {
        use std::io::Read;
        std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(buf)).expect("/dev/urandom");
    }
}

/// `RandomNumberGenerator` (from `RandomNumberGenerator.Create()`).
pub struct RandomNumberGenerator;
plain_trace!(RandomNumberGenerator);
impl RandomNumberGenerator {
    pub fn create() -> Gc<RandomNumberGenerator> {
        rt::alloc_init(RandomNumberGenerator)
    }
    /// `GetBytes(byte[])`: fills the array.
    pub fn get_bytes(&self, data: Arr<u8>) {
        if data.is_null() {
            throw("System.ArgumentNullException", "data");
        }
        if data.len() > 0 {
            // SAFETY: `at(0)` is the start of `len` bytes of the array's buffer.
            os_random(unsafe { core::slice::from_raw_parts_mut(data.at(0), data.len() as usize) });
        }
    }
    pub fn dispose(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use terraria_gen_rt::rt::io::{BinaryReader, BinaryWriter};

    fn key() -> Vec<u8> {
        "h3y_gUyZ".encode_utf16().flat_map(|u| u.to_le_bytes()).collect()
    }
    fn hex(b: &[u8]) -> String {
        b.iter().map(|c| format!("{c:02x}")).collect()
    }
    fn plain(len: usize) -> Vec<u8> {
        (0..len).map(|i| ((i * 7 + 3) % 256) as u8).collect()
    }

    /// Ciphertexts produced by .NET's RijndaelManaged + CryptoStream (tests/dotnet/system_probe.out, "AES").
    #[test]
    fn aes_cbc_pkcs7_matches_dotnet() {
        let k = key();
        for (len, expected) in [
            (0, "1066184b6545ca444e58e6e787f47bef"),
            (1, "963d892e051e3d954b35f9ec02053ace"),
            (15, "d3dbe79fff0c1790a13054653d9af222"),
            (16, "315042b4d1a88eded0adc432c62dac48d13b0b06967d111732a1531e269cf8c7"),
            (17, "315042b4d1a88eded0adc432c62dac4857d3568cacb84e834dc8845a31a5742f"),
            (40, "315042b4d1a88eded0adc432c62dac4871b3e6a45f68911e46ff5abf2c8915d1fff405916a825fa3638b945441f39808"),
        ] {
            let c = aes_cbc_encrypt(&k, &k, &plain(len), PADDING_PKCS7);
            assert_eq!(hex(&c), expected, "length {len}");
            assert_eq!(aes_cbc_decrypt(&k, &k, &c, PADDING_PKCS7), plain(len));
            // the game reads with PaddingMode.None: the padding bytes stay at the end
            let raw = aes_cbc_decrypt(&k, &k, &c, PADDING_NONE);
            assert_eq!((raw.len(), &raw[..len]), (c.len(), &plain(len)[..]));
        }
    }

    /// The object path of `Player.InternalSavePlayerFile` / `Player.LoadPlayer`.
    #[test]
    fn crypto_stream_round_trip_like_the_player_file() {
        let k = Arr::from_slice(&key());
        unsafe {
            let file = Stream::new_memory();
            let cs = crypto_stream_new(file, (*RijndaelManaged::new_gc().ptr()).create_encryptor(k, k), 1);
            let w = BinaryWriter::new_gc(cs);
            (*w.ptr()).write::<i32>(326);
            (*w.ptr()).write::<terraria_gen_rt::rt::Str>(terraria_gen_rt::rt::Str::lit("Player name"));
            (*w.ptr()).flush();
            assert_eq!((*file.ptr()).length(), 0, "nothing reaches the file before FlushFinalBlock");
            crypto_stream_flush_final_block(cs);
            let bytes = (*file.ptr()).to_array();
            assert_eq!(bytes.len(), 32, "4 + 12 bytes fill one block exactly: PKCS7 adds a whole block of padding");
            let e = rt::try_(|| crypto_stream_flush_final_block(cs)).unwrap_err();
            assert!(rt::exc_is(e, "System.NotSupportedException"));

            let r = RijndaelManaged::new_gc();
            (*r.ptr()).set_padding(PADDING_NONE);
            let input = crypto_stream_new(Stream::from_bytes(bytes), (*r.ptr()).create_decryptor(k, k), 0);
            let reader = BinaryReader::new_gc(input);
            assert_eq!((*reader.ptr()).read_i32(), 326);
            assert_eq!((*reader.ptr()).read_string().as_str(), "Player name");
        }
    }

    #[test]
    fn errors_and_random() {
        let e = rt::try_(|| aes_cbc_encrypt(&[0; 5], &[0; 16], &[], PADDING_PKCS7)).unwrap_err();
        assert!(rt::exc_is(e, "System.Security.Cryptography.CryptographicException"));
        let k = key();
        let e = rt::try_(|| aes_cbc_decrypt(&k, &k, &[0; 16], PADDING_PKCS7)).unwrap_err();
        assert!(rt::exc_is(e, "System.Security.Cryptography.CryptographicException"));
        let a = Arr::<u8>::new(32);
        unsafe { (*RandomNumberGenerator::create().ptr()).get_bytes(a) };
        assert!(a.as_slice().iter().any(|b| *b != 0));
    }
}

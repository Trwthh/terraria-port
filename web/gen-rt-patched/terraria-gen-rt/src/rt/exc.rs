//! Exceptions: `throw` is an unwinding panic with a typed payload; `try` is `catch_unwind` (DESIGN.md section 7).

use super::gc::{alloc_init, Gc, Marker, Trace};
use super::string::Str;
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};

/// A `System.Exception` (or subclass) of the .NET library. `kind` is the full type name.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Exception {
    pub kind: &'static str,
    pub message: Str,
}
unsafe impl Trace for Exception {
    // the message is written when the object is made (`exc_init`, behind the barrier)
    const RESCAN: bool = false;
    unsafe fn trace(&self, m: &mut Marker) {
        self.message.trace(m)
    }
    /// `Exception.ToString()`: `Namespace.Type: message`. Deviation: .NET appends the stack trace of a thrown
    /// exception (`\r\n   at ...`); there is none here.
    unsafe fn obj_to_string(this: *const Self) -> Str {
        let e = &*this;
        match e.message.get() {
            Some(m) => Str::new(&format!("{}: {}", e.kind, m)),
            None => Str::new(&format!("{}: Exception of type '{}' was thrown.", e.kind, e.kind)),
        }
    }
}

/// Panic payload of a thrown C# exception (the address of the `Exception`; payloads must be `Send`).
struct Thrown(usize);

/// `new SomeException(message)`.
pub fn exc(kind: &'static str, message: Str) -> Gc<Exception> {
    alloc_init(Exception { kind, message })
}

/// Base-constructor call of a translated exception class (its first field is the runtime `Exception`):
/// `kind` is the nearest library exception type, which is what `catch (LibraryType)` tests.
///
/// # Safety
/// `e` must point at the exception part of a live object.
pub unsafe fn exc_init(e: *mut Exception, kind: &'static str, message: Str) {
    core::ptr::addr_of_mut!((*e).kind).write(kind);
    (*e).message = super::gc::wbv(message);
}

/// `throw e;`. Does not run the panic hook, so a caught exception prints nothing.
#[cold]
#[inline(never)]
pub fn throw(e: Gc<Exception>) -> ! {
    if e.is_null() {
        super::gc::null_reference();
    }
    trace_throw(e);
    resume_unwind(Box::new(Thrown(e.0 as usize)))
}

/// DIAGNOSTIC (`CS2RS_TRACE_THROW=<exception kind>` or `=all`): prints where a C# exception of that kind was
/// thrown, as the generated functions on the Rust stack (a thrown exception carries no stack trace here). At most
/// `CS2RS_TRACE_THROW_MAX` (default 5) traces per kind.
fn trace_throw(e: Gc<Exception>) {
    use std::sync::OnceLock;
    static WANT: OnceLock<Option<String>> = OnceLock::new();
    let Some(want) = WANT.get_or_init(|| std::env::var("CS2RS_TRACE_THROW").ok()) else { return };
    // SAFETY: a live exception object.
    let (kind, msg) = unsafe { ((*e.ptr()).kind, (*e.ptr()).message) };
    if want != "all" && want != kind {
        return;
    }
    thread_local! { static SEEN: std::cell::RefCell<std::collections::HashMap<&'static str, u32>> = Default::default(); }
    let max: u32 = std::env::var("CS2RS_TRACE_THROW_MAX").ok().and_then(|v| v.parse().ok()).unwrap_or(5);
    let n = SEEN.with(|s| {
        let mut s = s.borrow_mut();
        let n = s.entry(kind).or_insert(0);
        *n += 1;
        *n
    });
    if n > max {
        return;
    }
    let bt = std::backtrace::Backtrace::force_capture().to_string();
    let frames: Vec<&str> = bt.lines().map(str::trim).filter(|l| l.contains("::gen::") || l.contains("terraria_host::")).take(14).collect();
    eprintln!("[throw #{n}] {kind}: {}", msg.get().unwrap_or(""));
    for f in frames {
        eprintln!("[throw]     {f}");
    }
}

/// `throw;` inside a catch.
#[cold]
pub fn rethrow(e: Gc<Exception>) -> ! {
    throw(e)
}

/// Runs `f`; a C# exception thrown inside comes back as `Err`. Rust panics that are not C# exceptions
/// (a bug in the runtime, `rt::unported`) keep unwinding: a game `catch { }` must not hide them.
pub fn try_<R>(f: impl FnOnce() -> R) -> Result<R, Gc<Exception>> {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(r) => Ok(r),
        Err(payload) => match payload.downcast::<Thrown>() {
            Ok(t) => Err(Gc(t.0 as *mut Exception)),
            Err(other) => resume_unwind(other),
        },
    }
}

/// For hosts and tests: like [`try_`] but renders the exception as text.
pub fn run<R>(f: impl FnOnce() -> R) -> Result<R, String> {
    try_(f).map_err(exc_text)
}

/// `"Kind: message"`.
pub fn exc_text(e: Gc<Exception>) -> String {
    // SAFETY: exceptions are live runtime objects.
    let e = unsafe { *e.ptr() };
    format!("{}: {}", e.kind, if e.message.is_null() { "" } else { e.message.as_str() })
}

/// Base type of each .NET exception the runtime or the game code names. Unknown kinds derive from `System.Exception`.
fn parent(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "System.Exception" => return None,
        "System.SystemException" | "System.ApplicationException" => "System.Exception",
        "System.IndexOutOfRangeException"
        | "System.NullReferenceException"
        | "System.InvalidCastException"
        | "System.InvalidOperationException"
        | "System.ArithmeticException"
        | "System.ArgumentException"
        | "System.FormatException"
        | "System.NotSupportedException"
        | "System.NotImplementedException"
        | "System.IO.IOException"
        | "System.OutOfMemoryException"
        | "System.StackOverflowException"
        | "System.ArrayTypeMismatchException"
        | "System.Collections.Generic.KeyNotFoundException" => "System.SystemException",
        "System.DivideByZeroException" | "System.OverflowException" => "System.ArithmeticException",
        "System.ArgumentNullException" | "System.ArgumentOutOfRangeException" => "System.ArgumentException",
        "System.ObjectDisposedException" => "System.InvalidOperationException",
        "System.IO.FileNotFoundException" | "System.IO.EndOfStreamException" | "System.IO.DirectoryNotFoundException" | "System.IO.PathTooLongException" => {
            "System.IO.IOException"
        }
        // kinds the host layer throws (terraria-host REQUESTS.md 9 and 14), parents as in the .NET Framework
        "System.UnauthorizedAccessException"
        | "System.TypeLoadException"
        | "System.Security.Cryptography.CryptographicException"
        | "System.Threading.ThreadStateException"
        | "System.Reflection.AmbiguousMatchException"
        | "System.Runtime.InteropServices.ExternalException" => "System.SystemException",
        "System.DllNotFoundException" | "System.EntryPointNotFoundException" => "System.TypeLoadException",
        "Newtonsoft.Json.JsonReaderException" | "Newtonsoft.Json.JsonWriterException" | "Newtonsoft.Json.JsonSerializationException" => "Newtonsoft.Json.JsonException",
        "Ionic.Zip.BadReadException" | "Ionic.Zip.BadCrcException" => "Ionic.Zip.ZipException",
        _ => "System.Exception",
    })
}

/// Does `catch (T)` with `T` = `type_name` catch `e`?
pub fn exc_is(e: Gc<Exception>, type_name: &str) -> bool {
    // SAFETY: exceptions are live runtime objects.
    let mut k = unsafe { (*e.ptr()).kind };
    loop {
        if k == type_name {
            return true;
        }
        match parent(k) {
            Some(p) => k = p,
            None => return false,
        }
    }
}

/// `o as SomeLibraryException`: the exception when `o` is an exception object (the runtime's, or a translated
/// class deriving from one: `is_exc_class` answers for a translated type id) of that kind or a kind derived from
/// it; null otherwise.
pub fn as_exception(o: super::Obj, type_name: &str, is_exc_class: fn(u32) -> bool) -> Gc<Exception> {
    if o.is_null() {
        return Gc::NULL;
    }
    // SAFETY: a non-null handle points at an object from `alloc`.
    let id = unsafe { super::type_id_of(o.0 as *const u8) };
    if !(super::obj_is::<Exception>(o) || is_exc_class(id)) {
        return Gc::NULL;
    }
    let e: Gc<Exception> = o.up();
    if exc_is(e, type_name) {
        e
    } else {
        Gc::NULL
    }
}

/// `(SomeLibraryException)o`: null stays null, anything else that is not such an exception throws
/// `InvalidCastException`.
pub fn cast_exception(o: super::Obj, type_name: &str, is_exc_class: fn(u32) -> bool) -> Gc<Exception> {
    if o.is_null() {
        return Gc::NULL;
    }
    let e = as_exception(o, type_name, is_exc_class);
    if e.is_null() {
        throw(exc("System.InvalidCastException", Str::lit("Specified cast is not valid.")));
    }
    e
}

/// How control left a `try` body: fell through, returned from the method, or jumped (break / continue / goto)
/// to exit number `n` of that `try`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Flow<R: Copy> {
    Next,
    Return(R),
    Jump(u32),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::s;

    #[test]
    fn catch_by_base_type() {
        let e = exc("System.IndexOutOfRangeException", s!("x"));
        assert!(exc_is(e, "System.IndexOutOfRangeException"));
        assert!(exc_is(e, "System.SystemException"));
        assert!(exc_is(e, "System.Exception"));
        assert!(!exc_is(e, "System.NullReferenceException"));
        assert!(exc_is(exc("System.DivideByZeroException", Str::NULL), "System.ArithmeticException"));
    }

    #[test]
    fn throw_is_caught_with_state_intact() {
        let mut x = 0;
        let r = try_(|| {
            x = 1;
            throw(exc("System.Exception", s!("boom")));
        });
        assert_eq!(x, 1);
        assert_eq!(run(|| ()).is_ok(), true);
        assert_eq!(exc_text(r.unwrap_err()), "System.Exception: boom");
    }

    #[test]
    fn foreign_panics_are_not_caught() {
        let outer = std::panic::catch_unwind(|| {
            let _ = try_(|| crate::rt::unreachable());
        });
        assert!(outer.is_err());
    }
}

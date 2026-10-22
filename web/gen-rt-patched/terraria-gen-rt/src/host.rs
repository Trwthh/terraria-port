//! The single registry behind the generated host boundary (`host::api`).
//!
//! - [`unimplemented`]: a `void` library member the host does not provide yet. Recorded, logged once, continues.
//! - [`unimplemented_value`]: a value is needed from a library member the host does not provide. Recorded, then
//!   panics (a plain Rust panic: a translated `catch { }` cannot swallow it). Never returns a made-up value.
//! - [`configured`]: the translator config (`stubs`) gives the value to continue with. Recorded, returned.
//! - [`is_instance`]: `x is T` / `x as T` / `(T)x` against a library class. Null is answered (it is no instance
//!   of anything); for a non-null opaque object the host has to know, so it panics until implemented.

use crate::rt::Obj;
use std::collections::BTreeMap;
use std::sync::Mutex;

static REACHED: Mutex<BTreeMap<&'static str, u64>> = Mutex::new(BTreeMap::new());

fn hit(name: &'static str) -> bool {
    let mut m = match REACHED.lock() {
        Ok(m) => m,
        Err(p) => p.into_inner(),
    };
    let n = m.entry(name).or_insert(0);
    *n += 1;
    *n == 1
}

/// Every host member reached so far with its hit count.
pub fn reached() -> Vec<(&'static str, u64)> {
    let m = match REACHED.lock() {
        Ok(m) => m,
        Err(p) => p.into_inner(),
    };
    m.iter().map(|(k, v)| (*k, *v)).collect()
}

static INERT: Mutex<BTreeMap<&'static str, u64>> = Mutex::new(BTreeMap::new());

/// A library member the translator config declares inert (`"inert"`: it does nothing the translated program can
/// observe; the reason is written there). Counted, not logged, not a gap.
pub fn inert(member: &'static str) {
    let mut m = match INERT.lock() {
        Ok(m) => m,
        Err(p) => p.into_inner(),
    };
    *m.entry(member).or_insert(0) += 1;
}

/// Every inert member reached so far with its hit count.
pub fn inert_reached() -> Vec<(&'static str, u64)> {
    let m = match INERT.lock() {
        Ok(m) => m,
        Err(p) => p.into_inner(),
    };
    m.iter().map(|(k, v)| (*k, *v)).collect()
}

/// Names of the `void` members that were reached without an implementation (logged, execution continued), and of
/// the constructors that produced an opaque object: what a proof must not have on its path.
pub fn unimplemented_reached() -> Vec<(&'static str, u64)> {
    let m = match UNIMPL.lock() {
        Ok(m) => m,
        Err(p) => p.into_inner(),
    };
    m.iter().map(|(k, v)| (*k, *v)).collect()
}
static UNIMPL: Mutex<BTreeMap<&'static str, u64>> = Mutex::new(BTreeMap::new());
fn unimpl_hit(name: &'static str) {
    let mut m = match UNIMPL.lock() {
        Ok(m) => m,
        Err(p) => p.into_inner(),
    };
    *m.entry(name).or_insert(0) += 1;
}

#[cold]
#[inline(never)]
pub fn unimplemented(member: &'static str) {
    unimpl_hit(member);
    if hit(member) {
        log(&format!("[host] not implemented (void, continuing): {member}"));
        problem(format!("host member not implemented (the call did nothing): {member}"));
    }
}

/// `IDisposable.Dispose()` reached on a runtime / host object whose type has no `Dispose` (`Trace::obj_dispose`).
#[cold]
#[inline(never)]
pub fn unimplemented_dispose(rust_type: &'static str) {
    static NAMES: Mutex<BTreeMap<&'static str, &'static str>> = Mutex::new(BTreeMap::new());
    let name: &'static str = {
        let mut m = match NAMES.lock() {
            Ok(m) => m,
            Err(p) => p.into_inner(),
        };
        *m.entry(rust_type).or_insert_with(|| Box::leak(format!("System.IDisposable.Dispose() of {rust_type}").into_boxed_str()))
    };
    unimplemented(name)
}

/// `ICollection<T>.Clear()` reached on an object that is no runtime collection (`Trace::obj_clear`).
#[cold]
#[inline(never)]
pub fn unimplemented_clear(rust_type: &'static str) {
    static NAMES: Mutex<BTreeMap<&'static str, &'static str>> = Mutex::new(BTreeMap::new());
    let name: &'static str = {
        let mut m = match NAMES.lock() {
            Ok(m) => m,
            Err(p) => p.into_inner(),
        };
        *m.entry(rust_type).or_insert_with(|| Box::leak(format!("System.Collections.Generic.ICollection<T>.Clear() of {rust_type}").into_boxed_str()))
    };
    unimplemented(name)
}

#[cold]
#[inline(never)]
pub fn unimplemented_value<T>(member: &'static str) -> T {
    let first = hit(member);
    if survey() {
        // DIAGNOSTIC ONLY (`CS2RS_SURVEY=1`): continue with a zeroed value so that one run lists every missing
        // member on a path. What runs after the first such value proves nothing.
        if first {
            eprintln!("[survey] missing value (continuing with zero): {member}");
        }
        // SAFETY: every value type of the runtime is valid when zeroed (DESIGN.md section 1).
        return unsafe { core::mem::zeroed() };
    }
    if first && std::env::var_os("CS2RS_TRACE_HOST").is_some() {
        // DIAGNOSTIC: the generated functions on the stack of the first call
        let bt = std::backtrace::Backtrace::force_capture().to_string();
        eprintln!("[host] {member} needed by:");
        for f in bt.lines().map(str::trim).filter(|l| l.contains("::gen::") || l.contains("terraria_host::") || l.contains("::host::")).take(12) {
            eprintln!("[host]     {f}");
        }
    }
    panic!("host: not implemented (a value is needed): {member}")
}

/// `CS2RS_SURVEY=1`: see [`unimplemented_value`]. Never set for a comparison with the real game.
pub fn survey() -> bool {
    use std::sync::OnceLock;
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var("CS2RS_SURVEY").map(|v| v == "1").unwrap_or(false))
}

/// The constructor of a library class the host does not provide yet: a fresh opaque object. Translated code
/// cannot look inside a library object; it can only compare the handle with null and pass it to other host
/// members, each of which is recorded when reached.
pub fn new_opaque(member: &'static str) -> Obj {
    hit(member);
    unimpl_hit(member);
    crate::rt::new_object()
}

#[inline]
pub fn configured<T>(member: &'static str, value: T) -> T {
    hit(member);
    value
}

/// `o is <type_name>` for a library class.
pub fn is_instance(o: Obj, type_name: &'static str) -> bool {
    if o.is_null() {
        return false;
    }
    hit(type_name);
    panic!("host: type test of an opaque object against {type_name} is not implemented")
}

/// `o as <type_name>` for a library class.
pub fn as_instance(o: Obj, type_name: &'static str) -> Obj {
    if is_instance(o, type_name) {
        o
    } else {
        crate::rt::Gc::NULL
    }
}

/// `(<type_name>)o` for a library class: null stays null.
pub fn cast_instance(o: Obj, type_name: &'static str) -> Obj {
    if o.is_null() {
        return o;
    }
    let _ = is_instance(o, type_name);
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn void_continues_value_panics_configured_returns() {
        unimplemented("Test.Host.Void()");
        unimplemented("Test.Host.Void()");
        assert!(reached().iter().any(|(k, n)| *k == "Test.Host.Void()" && *n >= 2));
        assert_eq!(configured("Test.Host.Configured", 7), 7);
        let r = std::panic::catch_unwind(|| crate::rt::try_(|| unimplemented_value::<i32>("Test.Host.Value")));
        assert!(r.is_err(), "a missing host value must not be catchable as a C# exception");
        assert!(!is_instance(crate::rt::Gc::NULL, "Some.Type"));
    }
}

// ---------------------------------------------------------------------------------------------- the log's tail

static LOG_TAIL: Mutex<std::collections::VecDeque<String>> = Mutex::new(std::collections::VecDeque::new());
static PROBLEMS: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// Lines kept by [`log`].
pub const LOG_TAIL_LINES: usize = 200;

/// A line of the host's log: printed to stderr and kept (the last [`LOG_TAIL_LINES`]) for a crash report
/// ([`log_tail`]).
pub fn log(line: &str) {
    eprintln!("{line}");
    let mut t = match LOG_TAIL.lock() {
        Ok(t) => t,
        Err(p) => p.into_inner(),
    };
    if t.len() == LOG_TAIL_LINES {
        t.pop_front();
    }
    t.push_back(line.to_owned());
}

/// The last lines given to [`log`], oldest first.
pub fn log_tail() -> Vec<String> {
    match LOG_TAIL.lock() {
        Ok(t) => t.iter().cloned().collect(),
        Err(p) => p.into_inner().iter().cloned().collect(),
    }
}

/// Something the port does not have was reached and the program went on (an unimplemented `void` host member, the
/// first time): the driver takes these ([`take_problems`]) to tell the player and to write its crash log.
fn problem(text: String) {
    match PROBLEMS.lock() {
        Ok(mut p) => p.push(text),
        Err(p) => p.into_inner().push(text),
    }
}

/// The problems reported since the last call.
pub fn take_problems() -> Vec<String> {
    match PROBLEMS.lock() {
        Ok(mut p) => core::mem::take(&mut *p),
        Err(p) => core::mem::take(&mut *p.into_inner()),
    }
}

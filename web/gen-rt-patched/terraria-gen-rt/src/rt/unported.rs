//! The explicit gap list at run time (see `port/PORTING.md` "unported!" and DESIGN.md section 10).
//!
//! - [`unported`]: a value is needed from something that is not translated. Records the name and panics
//!   (a plain Rust panic, so a translated `catch { }` cannot swallow it).
//! - [`gap`]: a `void` call that is not translated. Records the name, logs once, continues ("did nothing").
//! - [`stub`]: a callee whose replacement value a human wrote in the config. Records the name, returns it.

use std::collections::BTreeMap;
use std::sync::Mutex;

static GAPS: Mutex<BTreeMap<&'static str, u64>> = Mutex::new(BTreeMap::new());
static STUBS: Mutex<BTreeMap<&'static str, u64>> = Mutex::new(BTreeMap::new());

fn hit(map: &Mutex<BTreeMap<&'static str, u64>>, name: &'static str) -> bool {
    let mut m = match map.lock() {
        Ok(m) => m,
        Err(p) => p.into_inner(),
    };
    let n = m.entry(name).or_insert(0);
    *n += 1;
    *n == 1
}
fn list(map: &Mutex<BTreeMap<&'static str, u64>>) -> Vec<(&'static str, u64)> {
    let m = match map.lock() {
        Ok(m) => m,
        Err(p) => p.into_inner(),
    };
    m.iter().map(|(k, v)| (*k, *v)).collect()
}

/// DIAGNOSTIC ONLY (`CS2RS_SURVEY=1`, as `host::unimplemented_value`): a stop continues with a zeroed value, so that
/// one run lists every missing piece on a path instead of the first. What runs after such a value proves nothing.
fn survey() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("CS2RS_SURVEY").is_some_and(|v| !v.is_empty() && v != "0"))
}

/// Something the original reads or calls here is neither translated nor mapped, and a value is needed.
#[cold]
#[inline(never)]
pub fn unported<T>(name: &'static str) -> T {
    let first = hit(&GAPS, name);
    if survey() {
        if first {
            eprintln!("[survey] unported (continuing with zero): {name}");
        }
        // SAFETY: every value type of the runtime is valid when zeroed (DESIGN.md section 1).
        return unsafe { core::mem::zeroed() };
    }
    panic!("unported: {name}")
}

/// The emitter could not translate this method (`reason` names the first construct it refused).
#[cold]
#[inline(never)]
pub fn unported_method<T>(name: &'static str, reason: &'static str) -> T {
    let first = hit(&GAPS, name);
    if survey() {
        if first {
            eprintln!("[survey] unported method (continuing with zero): {name} ({reason})");
        }
        // SAFETY: as `unported`.
        return unsafe { core::mem::zeroed() };
    }
    panic!("unported method: {name} ({reason})")
}

/// `new X(..)` of a game class outside the translated set: a fresh opaque object. Recorded like a gap (the
/// constructor body did not run); every member of the object is itself unported.
#[cold]
#[inline(never)]
pub fn unported_new(name: &'static str) -> super::Obj {
    if hit(&GAPS, name) {
        eprintln!("[unported] {name} (opaque object)");
    }
    super::new_object()
}

/// A `void` call the original makes here that is not translated: logged once, then execution continues.
#[cold]
#[inline(never)]
pub fn gap(name: &'static str) {
    if hit(&GAPS, name) {
        eprintln!("[unported] {name}");
    }
}

/// A callee replaced by the value given in the translator config.
#[inline]
pub fn stub<T>(name: &'static str, value: T) -> T {
    hit(&STUBS, name);
    value
}

/// Every gap reached so far (`unported`, `unported_method`, `gap`) with its hit count.
pub fn gaps() -> Vec<(&'static str, u64)> {
    list(&GAPS)
}

/// Every stub reached so far with its hit count.
pub fn stubs_hit() -> Vec<(&'static str, u64)> {
    list(&STUBS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gap_continues_and_is_listed() {
        gap("Test.Gap");
        gap("Test.Gap");
        assert!(gaps().iter().any(|(k, n)| *k == "Test.Gap" && *n >= 2));
        assert_eq!(stub("Test.Stub", 5), 5);
        assert!(stubs_hit().iter().any(|(k, _)| *k == "Test.Stub"));
    }

    #[test]
    fn unported_panics_and_is_not_a_csharp_exception() {
        let r = std::panic::catch_unwind(|| {
            let _ = crate::rt::try_(|| unported::<i32>("Test.Unported"));
        });
        assert!(r.is_err());
        assert!(gaps().iter().any(|(k, _)| *k == "Test.Unported"));
    }
}

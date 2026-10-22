//! Registry of calls into parts of the game that are not ported yet (see `port/PORTING.md`).

use std::collections::BTreeMap;
use std::sync::Mutex;

static HITS: Mutex<BTreeMap<&'static str, u64>> = Mutex::new(BTreeMap::new());

/// Called by `unported!`. Counts every hit; prints the name the first time.
pub fn hit(name: &'static str) {
    let mut hits = match HITS.lock() {
        Ok(h) => h,
        Err(poisoned) => poisoned.into_inner(),
    };
    let n = hits.entry(name).or_insert(0);
    if *n == 0 {
        eprintln!("[unported] {name}");
    }
    *n += 1;
}

/// Every unported call reached so far, with its hit count.
pub fn report() -> Vec<(&'static str, u64)> {
    let hits = match HITS.lock() {
        Ok(h) => h,
        Err(poisoned) => poisoned.into_inner(),
    };
    hits.iter().map(|(k, v)| (*k, *v)).collect()
}

/// Marks a call the original makes that the port cannot make yet.
/// `unported!("Class.Method")` in statement position; `unported!("Class.Method", value)` where a value is needed.
#[macro_export]
macro_rules! unported {
    ($name:literal) => {
        $crate::unported::hit($name)
    };
    ($name:literal, $value:expr) => {{
        $crate::unported::hit($name);
        $value
    }};
}

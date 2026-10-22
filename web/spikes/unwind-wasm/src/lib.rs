//! Spike: C# exceptions as the runtime implements them (DESIGN.md section 7; terraria-gen-rt/src/rt/exc.rs:99
//! `try_` = `catch_unwind`, `throw` = `resume_unwind` with a typed payload) on wasm32-unknown-unknown.
//! Exports return 1 when the thrown payload was caught and recognised, and the `finally` ran.
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};

struct Thrown(u32);

static mut FINALLY_RAN: u32 = 0;

#[inline(never)]
fn deep_throw(n: u32, code: u32) -> u32 {
    if n == 0 {
        resume_unwind(Box::new(Thrown(code)));
    }
    // a value with a destructor on every frame: the `finally` / `using` the unwinder must run
    struct Fin;
    impl Drop for Fin {
        fn drop(&mut self) {
            unsafe { FINALLY_RAN += 1 };
        }
    }
    let _f = Fin;
    deep_throw(n - 1, code) + 1
}

/// `try { deep_throw } catch (Thrown e) when e.code == 7 { ... }`: 1 if caught with the right payload and every
/// frame's destructor ran.
#[no_mangle]
pub extern "C" fn try_catch() -> u32 {
    unsafe { FINALLY_RAN = 0 };
    let r = catch_unwind(AssertUnwindSafe(|| deep_throw(10, 7)));
    match r {
        Err(p) => match p.downcast::<Thrown>() {
            Ok(t) if t.0 == 7 && unsafe { FINALLY_RAN } == 10 => 1,
            _ => 0,
        },
        Ok(_) => 0,
    }
}

/// An index error caught as the game's `try { Main.tile[x, y] } catch { }` bounds check does.
#[no_mangle]
pub extern "C" fn bounds_check(i: u32) -> u32 {
    let v = vec![1u32, 2, 3];
    std::panic::set_hook(Box::new(|_| {}));
    match catch_unwind(|| v[i as usize]) {
        Ok(x) => x,
        Err(_) => 99,
    }
}

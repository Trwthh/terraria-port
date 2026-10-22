//! The game assembly's embedded resources (`Assembly.GetManifestResourceNames` / `GetManifestResourceStream`),
//! served from files: the decompile keeps each resource as a file named like the resource
//! (`decompile/Terraria/Terraria.Localization.Content.en-US.json`). The host installs the directory and the names
//! in MANIFEST ORDER (the game filters `GetManifestResourceNames()` with `Array.FindAll` and loads in that order).

use super::gc::{Gc, Obj};
use super::string::Str;
use super::Arr;
use std::cell::RefCell;
use std::path::PathBuf;

thread_local! {
    static RES: RefCell<Option<(PathBuf, Vec<String>)>> = const { RefCell::new(None) };
    static ASM: super::Rooted<Obj> = super::Rooted::new(super::new_object());
}

/// Installs the resource directory and the resource names in manifest order.
pub fn install(dir: impl Into<PathBuf>, names: Vec<String>) {
    RES.with(|r| *r.borrow_mut() = Some((dir.into(), names)));
}

fn with<R>(f: impl FnOnce(&PathBuf, &[String]) -> R) -> R {
    RES.with(|r| match &*r.borrow() {
        Some((d, n)) => f(d, n),
        None => panic!("host: the embedded resources were not installed (rt::resources::install)"),
    })
}

/// `typeof(X).Assembly` / `Assembly.GetExecutingAssembly()`: the game assembly (one opaque object).
pub fn assembly() -> Obj {
    ASM.with(|a| a.get())
}

/// `assembly.GetManifestResourceNames()`.
pub fn names() -> Arr<Str> {
    with(|_, names| {
        let a = Arr::<Str>::new(names.len() as i32);
        for (i, n) in names.iter().enumerate() {
            // SAFETY: index in range.
            unsafe { *a.at(i as i32) = Str::new(n) };
        }
        a
    })
}

/// `assembly.GetManifestResourceStream(name)`: null when there is no such resource (as .NET).
pub fn open(name: Str) -> Gc<super::io::Stream> {
    let name = name.as_str();
    with(|dir, names| {
        if !names.iter().any(|n| n == name) {
            return Gc::NULL;
        }
        match std::fs::read(dir.join(name)) {
            Ok(bytes) => super::io::Stream::from_vec(bytes),
            Err(e) => panic!("host: embedded resource {name} is listed but its file cannot be read from {}: {e}", dir.display()),
        }
    })
}

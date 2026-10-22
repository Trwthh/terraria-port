//! The game assembly's embedded resources (`Assembly.GetManifestResourceNames` / `GetManifestResourceStream`),
//! served from files: the decompile keeps each resource as a file named like the resource
//! (`decompile/Terraria/Terraria.Localization.Content.en-US.json`). The host installs the directory and the names
//! in MANIFEST ORDER (the game filters `GetManifestResourceNames()` with `Array.FindAll` and loads in that order).

use super::gc::{Gc, Obj};
use super::string::Str;
use super::Arr;
use std::cell::RefCell;
use std::path::PathBuf;

/// Where the resources come from: a folder of extracted files, or the assembly file itself (the browser build: the
/// player's own Terraria.exe, `clr_resources`).
enum Source {
    Dir(PathBuf),
    Assembly(std::rc::Rc<Vec<u8>>, std::collections::HashMap<String, std::ops::Range<usize>>),
}

thread_local! {
    static RES: RefCell<Option<(Source, Vec<String>)>> = const { RefCell::new(None) };
    static ASM: super::Rooted<Obj> = super::Rooted::new(super::new_object());
}

/// Installs the resource directory and the resource names in manifest order.
pub fn install(dir: impl Into<PathBuf>, names: Vec<String>) {
    RES.with(|r| *r.borrow_mut() = Some((Source::Dir(dir.into()), names)));
}

/// Installs the resources embedded in an assembly file (its bytes), in its manifest order.
pub fn install_from_assembly(file: Vec<u8>) -> Result<usize, String> {
    let list = super::clr_resources::manifest_resources(&file)?;
    let names: Vec<String> = list.iter().map(|(n, _)| n.clone()).collect();
    let n = names.len();
    RES.with(|r| *r.borrow_mut() = Some((Source::Assembly(std::rc::Rc::new(file), list.into_iter().collect()), names)));
    Ok(n)
}

fn with<R>(f: impl FnOnce(&Source, &[String]) -> R) -> R {
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
        match dir {
            Source::Dir(dir) => match std::fs::read(dir.join(name)) {
                Ok(bytes) => super::io::Stream::from_vec(bytes),
                Err(e) => panic!("host: embedded resource {name} is listed but its file cannot be read from {}: {e}", dir.display()),
            },
            Source::Assembly(bytes, ranges) => super::io::Stream::from_vec(bytes[ranges[name].clone()].to_vec()),
        }
    })
}

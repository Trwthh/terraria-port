//! Native host layer for the machine-translated Terraria client: the parts of XNA 4.0, ReLogic's native edges and
//! `System.*` that the game calls, without Bevy. See README.md (architecture, driver protocol, gaps) and
//! COVERAGE.md (member by member).

pub mod audio;
pub mod content;
pub mod game;
pub mod halfvec;
pub mod gfx;
pub mod image;
pub mod input;
pub mod json;
mod input_keys;
pub mod os;
pub mod pinvoke;
pub mod readback;
pub mod sys;
/// Box ids of this crate's value types (`rt::tid::HOST_FIRST + n`): the one place they are numbered.
pub mod tid {
    use terraria_gen_rt::rt::tid::HOST_FIRST;
    /// `System.DateTime`
    pub const DATE_TIME: u32 = HOST_FIRST + 1;
}
pub mod winforms;

/// This crate's folder (tests, fixtures, the project root above it). Natively the path cargo compiled it from; on
/// WASI the same folder below the preopen `/w` (the project root: web/ci/wasm_tests.sh), since a native Windows path
/// means nothing inside the sandbox.
pub fn manifest_dir() -> std::path::PathBuf {
    #[cfg(target_os = "wasi")]
    {
        std::path::PathBuf::from("/w/port/crates/terraria-host")
    }
    #[cfg(not(target_os = "wasi"))]
    {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }
}

/// A scratch folder for tests and tools: the system's temp folder natively; on WASI (no temp folder there) a fresh
/// `work/wasm-tmp/<random>` below the preopened project root, new for every process (Node's WASI cannot remove a
/// folder tree, so a test could not clean up what an earlier run left).
pub fn scratch_dir() -> std::path::PathBuf {
    #[cfg(target_os = "wasi")]
    {
        thread_local! {
            static DIR: std::path::PathBuf = {
                let mut r = [0u8; 8];
                crate::sys::crypto::os_random(&mut r);
                std::path::PathBuf::from(format!("/w/work/wasm-tmp/{:016x}", u64::from_le_bytes(r)))
            };
        }
        let d = DIR.with(|d| d.clone());
        let _ = std::fs::create_dir_all(&d);
        d
    }
    #[cfg(not(target_os = "wasi"))]
    {
        std::env::temp_dir()
    }
}

/// Can this host list a folder? Always natively. Node's WASI on Windows cannot (`fd_readdir`: "Function not
/// implemented"), so a test run there skips its listing checks and says so with a `WASM-PARTIAL:` line, which
/// web/ci/wasm_tests.sh counts apart. The browser build lists through its own file layer.
pub fn host_lists_directories() -> bool {
    match std::fs::read_dir(scratch_dir()) {
        Err(e) => e.kind() != std::io::ErrorKind::Unsupported,
        Ok(_) => true,
    }
}

/// Runs `f` on a stack of at least `bytes` (the game recurses deeply: tests and tools give it a big one). Natively on
/// a new thread of that size, joined; a panic in `f` is re-raised with `what`. On wasm32 (no threads in WASI or in the
/// one-Worker browser build) on the calling thread: the wasm stack size is set when linking (`-zstack-size`,
/// web/ci/wasm_tests.sh and the web build).
pub fn run_with_stack<F: FnOnce() + Send + 'static>(bytes: usize, what: &str, f: F) {
    #[cfg(target_family = "wasm")]
    {
        let _ = (bytes, what);
        f()
    }
    #[cfg(not(target_family = "wasm"))]
    {
        std::thread::Builder::new().stack_size(bytes).spawn(f).expect("thread").join().expect(what);
    }
}

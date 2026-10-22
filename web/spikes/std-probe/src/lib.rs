//! Each export exercises one std API the port's runtime or host calls (citations in web/WEB_PORT.md).
//! Run by run.mjs under Node: a trap means the call panics (panic = abort -> `unreachable`) on wasm32-unknown-unknown.
use std::hint::black_box;

#[no_mangle] pub extern "C" fn instant_now() -> u32 { let t = std::time::Instant::now(); black_box(t); 1 }
#[no_mangle] pub extern "C" fn system_time_now() -> u32 { let t = std::time::SystemTime::now(); black_box(t); 1 }
#[no_mangle] pub extern "C" fn thread_current_id() -> u32 { let id = std::thread::current().id(); black_box(id); 1 }
/// terraria-gen-rt rt/arr.rs: `let _ = std::thread::Builder::new()...spawn(..)` -> 1 = Ok, 0 = Err (graceful)
#[no_mangle] pub extern "C" fn thread_spawn() -> u32 { std::thread::Builder::new().spawn(|| {}).is_ok() as u32 }
#[no_mangle] pub extern "C" fn thread_sleep() -> u32 { std::thread::sleep(std::time::Duration::from_millis(1)); 1 }
/// 1 = Ok, 0 = Err
#[no_mangle] pub extern "C" fn fs_read() -> u32 { std::fs::read("Content/Images/Item_1.xnb").is_ok() as u32 }
#[no_mangle] pub extern "C" fn env_var() -> u32 { std::env::var("TERRARIA_DIR").is_ok() as u32 }
#[no_mangle] pub extern "C" fn tcp_connect() -> u32 { std::net::TcpStream::connect("127.0.0.1:7777").is_ok() as u32 }
#[no_mangle] pub extern "C" fn mpsc_channel() -> u32 { let (tx, rx) = std::sync::mpsc::channel::<u32>(); tx.send(5).unwrap(); rx.recv().unwrap() }
#[no_mangle] pub extern "C" fn mutex_lock() -> u32 { let m = std::sync::Mutex::new(3u32); let v = *m.lock().unwrap(); v }
#[no_mangle] pub extern "C" fn available_parallelism() -> u32 { std::thread::available_parallelism().map_or(0, |n| n.get() as u32) }
/// f64::sin on wasm32 comes from compiler-builtins' libm, on windows-gnu from the C runtime: print the bits
#[no_mangle] pub extern "C" fn sin_bits_lo() -> u32 { (black_box(1.0e6f64).sin().to_bits() & 0xffff_ffff) as u32 }
#[no_mangle] pub extern "C" fn sin_bits_hi() -> u32 { (black_box(1.0e6f64).sin().to_bits() >> 32) as u32 }
#[no_mangle] pub extern "C" fn pow_bits_lo() -> u32 { (black_box(1.7f64).powf(black_box(33.3)).to_bits() & 0xffff_ffff) as u32 }
#[no_mangle] pub extern "C" fn exp_bits_lo() -> u32 { (black_box(0.3f64).exp().to_bits() & 0xffff_ffff) as u32 }

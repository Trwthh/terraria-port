//! The audio trace: every call the translated game's audio reaches the audio engine with, as one text line, in the
//! format of the ORIGINAL's recorder (`tools/refdump_draw/AudioRec.cs`, which records the real `Terraria.exe` at the
//! boundary between XNA's managed audio classes and their native engine). `crates/terraria-gen-game/tests/draw.rs`
//! (`DRAW_MODE=audio`) records the audio run with this module and compares the two line by line
//! (`tools/cs2rs/VERIFICATION.md`, "Audio output").
//!
//! The lines are written by [`super::engine`] (the counterpart of XNA's `SoundEffectUnsafeNativeMethods` and of the
//! XACT `UnsafeNativeMethods`), so what is compared is what reaches the engine, including the derived XAudio2
//! parameters the mixer uses (voice volume, output matrix, frequency ratio). The line table is in AudioRec.cs.
//!
//! Off unless a driver calls [`set_active`]; with [`set_full`] the lines are also kept ([`take_output`]). Host code
//! that uses the engine on its own behalf (the voices behind XACT cues) runs inside [`internal`]: not recorded, as
//! the original's XACT engine plays its waves below the recorded boundary.

use std::cell::{Cell, RefCell};

pub const FNV_BASIS: u64 = 14695981039346656037;
pub const FNV_PRIME: u64 = 1099511628211;

#[derive(Default)]
struct Tracer {
    active: bool,
    full: bool,
    out: Vec<u8>,
    frame_hash: u64,
    frame_lines: u64,
    lines: u64,
}

thread_local! {
    static TR: RefCell<Tracer> = RefCell::new(Tracer { frame_hash: FNV_BASIS, ..Default::default() });
    static DEPTH: Cell<u32> = const { Cell::new(0) };
}

/// Driver: record (digest) from now on, or stop.
pub fn set_active(on: bool) {
    TR.with(|t| t.borrow_mut().active = on);
}
/// Driver: also keep the lines ([`take_output`]).
pub fn set_full(on: bool) {
    TR.with(|t| t.borrow_mut().full = on);
}
/// Driver: the lines kept since the last call.
pub fn take_output() -> Vec<u8> {
    TR.with(|t| std::mem::take(&mut t.borrow_mut().out))
}
/// Driver: appends text to the kept output as it is (the `frame N` headers).
pub fn write_raw(s: &str) {
    TR.with(|t| {
        let mut t = t.borrow_mut();
        if t.full {
            t.out.extend_from_slice(s.as_bytes());
        }
    });
}
/// Lines written in full mode so far.
pub fn lines_written() -> u64 {
    TR.with(|t| t.borrow().lines)
}
/// Driver: a frame starts (its digest restarts).
pub fn frame_start() {
    TR.with(|t| {
        let mut t = t.borrow_mut();
        t.frame_hash = FNV_BASIS;
        t.frame_lines = 0;
    });
}
/// The frame so far: (lines, FNV-1a 64 of the lines with their newlines).
pub fn frame_digest() -> (u64, u64) {
    TR.with(|t| {
        let t = t.borrow();
        (t.frame_lines, t.frame_hash)
    })
}

/// Inside host-internal use of the engine (not recorded).
pub fn is_internal() -> bool {
    DEPTH.get() > 0
}
/// Is a call recorded now? (Active, and not inside host-internal use.)
pub fn on() -> bool {
    DEPTH.get() == 0 && TR.with(|t| t.borrow().active)
}

/// Records a line (built only when recording).
pub fn line(f: impl FnOnce() -> String) {
    if !on() {
        return;
    }
    let s = f();
    TR.with(|t| {
        let mut t = t.borrow_mut();
        let mut h = t.frame_hash;
        for &b in s.as_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(FNV_PRIME);
        }
        h ^= 10;
        h = h.wrapping_mul(FNV_PRIME);
        t.frame_hash = h;
        t.frame_lines += 1;
        if t.full {
            t.out.extend_from_slice(s.as_bytes());
            t.out.push(b'\n');
            t.lines += 1;
        }
    });
}

/// Host code using the engine on its own behalf: not recorded.
pub fn internal<R>(f: impl FnOnce() -> R) -> R {
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            DEPTH.set(DEPTH.get() - 1);
        }
    }
    DEPTH.set(DEPTH.get() + 1);
    let _g = Guard;
    f()
}

/// A float as the 8 hex digits of its bits.
pub fn f(v: f32) -> String {
    format!("{:08x}", v.to_bits())
}
/// FNV-1a 64 of bytes.
pub fn fnv(b: &[u8]) -> u64 {
    let mut h = FNV_BASIS;
    for &x in b {
        h ^= x as u64;
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}
/// `<length>:<FNV-1a 64>` of bytes (AudioRec.cs `Bytes`).
pub fn bytes(b: &[u8]) -> String {
    format!("{}:{:016x}", b.len(), fnv(b))
}
/// A path below the install's Content folder as a label (xna/Graphics/Rec.cs `AssetLabel`): `/` separators, the part
/// after the last `Content/`, without `.xnb`.
pub fn asset_label(path: &str) -> String {
    let p = path.replace('\\', "/");
    let lower = p.to_ascii_lowercase();
    let mut p = match lower.rfind("content/") {
        Some(i) => p[i + 8..].to_owned(),
        None => p,
    };
    if p.to_ascii_lowercase().ends_with(".xnb") {
        p.truncate(p.len() - 4);
    }
    p
}

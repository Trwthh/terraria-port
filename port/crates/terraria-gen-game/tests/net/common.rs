// Shared by tests/net.rs (the server side) and tests/draw.rs DRAW_MODE=net (the client side): the recorded streams of
// the net run (NetVirt.cs NetStreamLog), message splitting, the first difference of two byte runs, delivery into the
// game's read buffer (NetVirt.cs Deliver).
#![allow(dead_code)]

use terraria_gen_game::gen::terraria;
use terraria_gen_game::rt::Arr;

/// NetVirt.cs `NetStreamLog`: per tick (tick, length, bytes), little endian.
pub fn read_stream(path: &std::path::Path) -> Vec<(i32, Vec<u8>)> {
    let b = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut out = Vec::new();
    let mut o = 0;
    while o + 8 <= b.len() {
        let t = i32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        let n = i32::from_le_bytes(b[o + 4..o + 8].try_into().unwrap()) as usize;
        out.push((t, b[o + 8..o + 8 + n].to_vec()));
        o += 8 + n;
    }
    out
}

/// The messages of a byte run (length-prefixed: u16 length including itself, then the type byte): (offset, length, type).
pub fn messages(b: &[u8]) -> Vec<(usize, usize, u8)> {
    let mut v = Vec::new();
    let mut o = 0;
    while o + 3 <= b.len() {
        let n = u16::from_le_bytes([b[o], b[o + 1]]) as usize;
        if n < 3 || o + n > b.len() {
            break;
        }
        v.push((o, n, b[o + 2]));
        o += n;
    }
    v
}

/// Where two byte runs part: "message <k> (type t, at byte o) differs at its byte i: original .. / port .."
pub fn first_difference(a: &[u8], b: &[u8]) -> String {
    let i = (0..a.len().min(b.len())).find(|&i| a[i] != b[i]).unwrap_or(a.len().min(b.len()));
    let ma = messages(a);
    let k = ma.iter().position(|&(o, n, _)| i >= o && i < o + n);
    let ctx = |x: &[u8]| x[i.saturating_sub(4)..(i + 12).min(x.len())].iter().map(|v| format!("{v:02x}")).collect::<Vec<_>>().join(" ");
    match k {
        Some(k) => {
            let (o, n, t) = ma[k];
            let mb = messages(b);
            format!("message {k} (type {t}, {n} bytes, at byte {o}) at its byte {}; port's message there: {:?}; original bytes {} / port {} (lengths {} / {})", i - o, mb.get(k).map(|m| m.2), ctx(a), ctx(b), a.len(), b.len())
        }
        None => format!("at byte {i} (lengths {} / {}); original {} / port {}", a.len(), b.len(), ctx(a), ctx(b)),
    }
}

pub const FNV_BASIS: u64 = 14695981039346656037;
pub fn fnv(b: &[u8]) -> u64 {
    let mut h = FNV_BASIS;
    for &x in b {
        h = (h ^ x as u64).wrapping_mul(1099511628211);
    }
    h
}
/// NetServer.cs `NetDeliver`: queued bytes into the game's read buffer `index`, as many as fit.
pub unsafe fn deliver(pending: &mut Vec<u8>, index: i32) {
    if pending.is_empty() {
        return;
    }
    let buf = *(*terraria::net_message::s()).buffer.at(index);
    let room = 131070 - (*buf.ptr()).total_data;
    let n = (room.max(0) as usize).min(pending.len());
    if n == 0 {
        return;
    }
    let chunk: Vec<u8> = pending.drain(..n).collect();
    terraria::net_message::receive_bytes(Arr::from_slice(&chunk), n as i32, index);
}


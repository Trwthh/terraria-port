//! Raw deflate exactly as zlib 1.3.1 writes it at level 6 (window bits 15, memory level 8, default strategy): the
//! `deflate_slow` lazy matcher of deflate.c and the Huffman coding of trees.c, statement for statement.
//!
//! Why: the game compresses with Ionic.Zlib (DotNetZip's managed port of zlib), whose `DeflateStream` writes the same
//! bytes as zlib 1.3.1 at those settings (checked on world data, random data, zeros and mixed blocks, written whole or
//! in pieces of 7 and 4,096 bytes: all identical; Ionic writes nothing at all for an empty stream, zlib a final empty
//! block). The tile sections the server sends (`NetMessage.CompressTileBlock`, message 10) and the map file are
//! therefore compared byte for byte with the original's (tests/net.rs). Only what level 6 uses is here: no other
//! level, strategy or flush mode; the whole input is compressed at once (zlib's output does not depend on how the
//! input is split when nothing is flushed in between).
//!
//! The window bytes beyond the input are those zlib leaves there (`high_water`, `WIN_INIT`): `longest_match` compares
//! past the end of the data and the result is cut to the lookahead afterwards, so even those bytes are modelled.

const MIN_MATCH: usize = 3;
const MAX_MATCH: usize = 258;
const MIN_LOOKAHEAD: usize = MAX_MATCH + MIN_MATCH + 1;
const W_BITS: usize = 15;
const W_SIZE: usize = 1 << W_BITS;
const W_MASK: usize = W_SIZE - 1;
const WINDOW_SIZE: usize = 2 * W_SIZE;
const MAX_DIST: usize = W_SIZE - MIN_LOOKAHEAD;
const HASH_BITS: usize = 8 + 7;
const HASH_SIZE: usize = 1 << HASH_BITS;
const HASH_MASK: usize = HASH_SIZE - 1;
const HASH_SHIFT: usize = HASH_BITS.div_ceil(MIN_MATCH);
const WIN_INIT: usize = MAX_MATCH;
const TOO_FAR: usize = 4096;
const NIL: usize = 0;
// level 6: good_length, max_lazy, nice_length, max_chain
const GOOD_MATCH: usize = 8;
const MAX_LAZY: usize = 16;
const NICE_MATCH: usize = 128;
const MAX_CHAIN: usize = 128;
const LIT_BUFSIZE: usize = 1 << (8 + 6);
const SYM_END: usize = (LIT_BUFSIZE - 1) * 3;

const LENGTH_CODES: usize = 29;
const LITERALS: usize = 256;
const L_CODES: usize = LITERALS + 1 + LENGTH_CODES;
const D_CODES: usize = 30;
const BL_CODES: usize = 19;
const HEAP_SIZE: usize = 2 * L_CODES + 1;
const MAX_BITS: usize = 15;
const MAX_BL_BITS: usize = 7;
const END_BLOCK: usize = 256;
const REP_3_6: usize = 16;
const REPZ_3_10: usize = 17;
const REPZ_11_138: usize = 18;
const EXTRA_LBITS: [u32; LENGTH_CODES] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const EXTRA_DBITS: [u32; D_CODES] = [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];
const EXTRA_BLBITS: [u32; BL_CODES] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 3, 7];
const BL_ORDER: [usize; BL_CODES] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];

/// `ct_data`: `fc` is Freq / Code, `dl` is Dad / Len (unions in zlib: the same storage).
#[derive(Clone, Copy, Default)]
struct Ct {
    fc: u16,
    dl: u16,
}

struct Tables {
    length_code: [u8; 256],
    dist_code: [u8; 512],
    base_length: [u32; LENGTH_CODES],
    base_dist: [u32; D_CODES],
    static_ltree: [Ct; L_CODES + 2],
    static_dtree: [Ct; D_CODES],
}

fn bi_reverse(mut code: u32, mut len: u32) -> u32 {
    let mut res = 0;
    loop {
        res |= code & 1;
        code >>= 1;
        res <<= 1;
        len -= 1;
        if len == 0 {
            break;
        }
    }
    res >> 1
}

fn gen_codes(tree: &mut [Ct], max_code: usize, bl_count: &[u16; MAX_BITS + 1]) {
    let mut next_code = [0u16; MAX_BITS + 1];
    let mut code: u32 = 0;
    for bits in 1..=MAX_BITS {
        code = (code + bl_count[bits - 1] as u32) << 1;
        next_code[bits] = code as u16;
    }
    for n in 0..=max_code {
        let len = tree[n].dl as usize;
        if len == 0 {
            continue;
        }
        tree[n].fc = bi_reverse(next_code[len] as u32, len as u32) as u16;
        next_code[len] = next_code[len].wrapping_add(1);
    }
}

fn tables() -> &'static Tables {
    static T: std::sync::OnceLock<Tables> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        let mut t = Tables { length_code: [0; 256], dist_code: [0; 512], base_length: [0; LENGTH_CODES], base_dist: [0; D_CODES], static_ltree: [Ct::default(); L_CODES + 2], static_dtree: [Ct::default(); D_CODES] };
        let mut length = 0usize;
        let mut code = 0usize;
        while code < LENGTH_CODES - 1 {
            t.base_length[code] = length as u32;
            for _ in 0..(1 << EXTRA_LBITS[code]) {
                t.length_code[length] = code as u8;
                length += 1;
            }
            code += 1;
        }
        t.length_code[length - 1] = code as u8;
        let mut dist = 0usize;
        code = 0;
        while code < 16 {
            t.base_dist[code] = dist as u32;
            for _ in 0..(1 << EXTRA_DBITS[code]) {
                t.dist_code[dist] = code as u8;
                dist += 1;
            }
            code += 1;
        }
        dist >>= 7;
        while code < D_CODES {
            t.base_dist[code] = (dist << 7) as u32;
            for _ in 0..(1 << (EXTRA_DBITS[code] - 7)) {
                t.dist_code[256 + dist] = code as u8;
                dist += 1;
            }
            code += 1;
        }
        let mut bl_count = [0u16; MAX_BITS + 1];
        let mut n = 0;
        while n <= 143 {
            t.static_ltree[n].dl = 8;
            n += 1;
            bl_count[8] += 1;
        }
        while n <= 255 {
            t.static_ltree[n].dl = 9;
            n += 1;
            bl_count[9] += 1;
        }
        while n <= 279 {
            t.static_ltree[n].dl = 7;
            n += 1;
            bl_count[7] += 1;
        }
        while n <= 287 {
            t.static_ltree[n].dl = 8;
            n += 1;
            bl_count[8] += 1;
        }
        gen_codes(&mut t.static_ltree, L_CODES + 1, &bl_count);
        for n in 0..D_CODES {
            t.static_dtree[n].dl = 5;
            t.static_dtree[n].fc = bi_reverse(n as u32, 5) as u16;
        }
        t
    })
}

#[derive(Clone, Copy)]
enum Desc {
    L,
    D,
    Bl,
}

struct Deflate<'a> {
    input: &'a [u8],
    next_in: usize,
    window: Vec<u8>,
    prev: Vec<u16>,
    head: Vec<u16>,
    ins_h: usize,
    strstart: usize,
    block_start: i64,
    lookahead: usize,
    insert: usize,
    match_length: usize,
    prev_length: usize,
    match_available: bool,
    match_start: usize,
    prev_match: usize,
    high_water: usize,
    sym_buf: Vec<u8>,
    sym_next: usize,
    dyn_ltree: [Ct; HEAP_SIZE],
    dyn_dtree: [Ct; 2 * D_CODES + 1],
    bl_tree: [Ct; 2 * BL_CODES + 1],
    l_max_code: usize,
    d_max_code: usize,
    bl_max_code: usize,
    bl_count: [u16; MAX_BITS + 1],
    heap: [usize; HEAP_SIZE],
    heap_len: usize,
    heap_max: usize,
    depth: [u8; HEAP_SIZE],
    opt_len: u64,
    static_len: u64,
    out: Vec<u8>,
    bi_buf: u64,
    bi_valid: u32,
}

impl<'a> Deflate<'a> {
    fn tree(&mut self, d: Desc) -> &mut [Ct] {
        match d {
            Desc::L => &mut self.dyn_ltree,
            Desc::D => &mut self.dyn_dtree,
            Desc::Bl => &mut self.bl_tree,
        }
    }

    // ---------------------------------------------------------------- bits
    fn send_bits(&mut self, value: u32, length: u32) {
        self.bi_buf |= (value as u64) << self.bi_valid;
        self.bi_valid += length;
        while self.bi_valid >= 8 {
            self.out.push(self.bi_buf as u8);
            self.bi_buf >>= 8;
            self.bi_valid -= 8;
        }
    }
    fn bi_windup(&mut self) {
        if self.bi_valid > 0 {
            self.out.push(self.bi_buf as u8);
        }
        self.bi_buf = 0;
        self.bi_valid = 0;
    }
    fn send_code(&mut self, c: usize, tree: &[Ct]) {
        let (code, len) = (tree[c].fc as u32, tree[c].dl as u32);
        self.send_bits(code, len);
    }

    // ---------------------------------------------------------------- input and window
    fn update_hash(&mut self, c: u8) {
        self.ins_h = ((self.ins_h << HASH_SHIFT) ^ c as usize) & HASH_MASK;
    }
    fn insert_string(&mut self, s: usize) -> usize {
        let c = self.window[s + MIN_MATCH - 1];
        self.update_hash(c);
        let mh = self.head[self.ins_h] as usize;
        self.prev[s & W_MASK] = mh as u16;
        self.head[self.ins_h] = s as u16;
        mh
    }
    fn slide_hash(&mut self) {
        for m in self.head.iter_mut() {
            *m = if *m as usize >= W_SIZE { (*m as usize - W_SIZE) as u16 } else { NIL as u16 };
        }
        for m in self.prev.iter_mut() {
            *m = if *m as usize >= W_SIZE { (*m as usize - W_SIZE) as u16 } else { NIL as u16 };
        }
    }
    fn fill_window(&mut self) {
        loop {
            let mut more = WINDOW_SIZE - self.lookahead - self.strstart;
            if self.strstart >= W_SIZE + MAX_DIST {
                self.window.copy_within(W_SIZE..W_SIZE + (W_SIZE - more), 0);
                self.match_start = self.match_start.wrapping_sub(W_SIZE);
                self.strstart -= W_SIZE;
                self.block_start -= W_SIZE as i64;
                if self.insert > self.strstart {
                    self.insert = self.strstart;
                }
                self.slide_hash();
                more += W_SIZE;
            }
            if self.next_in == self.input.len() {
                break;
            }
            let n = more.min(self.input.len() - self.next_in);
            let at = self.strstart + self.lookahead;
            self.window[at..at + n].copy_from_slice(&self.input[self.next_in..self.next_in + n]);
            self.next_in += n;
            self.lookahead += n;
            if self.lookahead + self.insert >= MIN_MATCH {
                let mut str = self.strstart - self.insert;
                self.ins_h = self.window[str] as usize;
                let c = self.window[str + 1];
                self.update_hash(c);
                while self.insert > 0 {
                    let c = self.window[str + MIN_MATCH - 1];
                    self.update_hash(c);
                    self.prev[str & W_MASK] = self.head[self.ins_h];
                    self.head[self.ins_h] = str as u16;
                    str += 1;
                    self.insert -= 1;
                    if self.lookahead + self.insert < MIN_MATCH {
                        break;
                    }
                }
            }
            if !(self.lookahead < MIN_LOOKAHEAD && self.next_in != self.input.len()) {
                break;
            }
        }
        if self.high_water < WINDOW_SIZE {
            let curr = self.strstart + self.lookahead;
            if self.high_water < curr {
                let init = (WINDOW_SIZE - curr).min(WIN_INIT);
                self.window[curr..curr + init].fill(0);
                self.high_water = curr + init;
            } else if self.high_water < curr + WIN_INIT {
                let init = (curr + WIN_INIT - self.high_water).min(WINDOW_SIZE - self.high_water);
                let hw = self.high_water;
                self.window[hw..hw + init].fill(0);
                self.high_water += init;
            }
        }
    }
    fn longest_match(&mut self, mut cur_match: usize) -> usize {
        let mut chain_length = MAX_CHAIN;
        let scan0 = self.strstart;
        let mut best_len = self.prev_length;
        let mut nice_match = NICE_MATCH;
        let limit = if self.strstart > MAX_DIST { self.strstart - MAX_DIST } else { NIL };
        let strend = self.strstart + MAX_MATCH;
        let w = &self.window;
        let mut scan_end1 = w[scan0 + best_len - 1];
        let mut scan_end = w[scan0 + best_len];
        if self.prev_length >= GOOD_MATCH {
            chain_length >>= 2;
        }
        if nice_match > self.lookahead {
            nice_match = self.lookahead;
        }
        loop {
            let m = cur_match;
            let ok = !(w[m + best_len] != scan_end || w[m + best_len - 1] != scan_end1 || w[m] != w[scan0] || w[m + 1] != w[scan0 + 1]);
            if ok {
                // scan += 2, match++ (match already advanced once): both at index 2
                let (mut s, mut mm) = (scan0 + 2, m + 2);
                loop {
                    // eight pre-incremented comparisons, then the bound
                    let mut stop = false;
                    for _ in 0..8 {
                        s += 1;
                        mm += 1;
                        if w[s] != w[mm] {
                            stop = true;
                            break;
                        }
                    }
                    if stop || s >= strend {
                        break;
                    }
                }
                let len = MAX_MATCH - (strend - s);
                if len > best_len {
                    self.match_start = cur_match;
                    best_len = len;
                    if len >= nice_match {
                        break;
                    }
                    scan_end1 = w[scan0 + best_len - 1];
                    scan_end = w[scan0 + best_len];
                }
            }
            cur_match = self.prev[cur_match & W_MASK] as usize;
            if cur_match <= limit {
                break;
            }
            chain_length -= 1;
            if chain_length == 0 {
                break;
            }
        }
        if best_len <= self.lookahead {
            best_len
        } else {
            self.lookahead
        }
    }

    // ---------------------------------------------------------------- symbols
    fn tally_lit(&mut self, c: u8) -> bool {
        self.sym_buf[self.sym_next] = 0;
        self.sym_buf[self.sym_next + 1] = 0;
        self.sym_buf[self.sym_next + 2] = c;
        self.sym_next += 3;
        self.dyn_ltree[c as usize].fc += 1;
        self.sym_next == SYM_END
    }
    fn tally_dist(&mut self, distance: usize, length: usize) -> bool {
        let t = tables();
        self.sym_buf[self.sym_next] = distance as u8;
        self.sym_buf[self.sym_next + 1] = (distance >> 8) as u8;
        self.sym_buf[self.sym_next + 2] = length as u8;
        self.sym_next += 3;
        let dist = distance - 1;
        self.dyn_ltree[t.length_code[length] as usize + LITERALS + 1].fc += 1;
        let dc = if dist < 256 { t.dist_code[dist] } else { t.dist_code[256 + (dist >> 7)] };
        self.dyn_dtree[dc as usize].fc += 1;
        self.sym_next == SYM_END
    }

    // ---------------------------------------------------------------- trees
    fn init_block(&mut self) {
        for n in 0..L_CODES {
            self.dyn_ltree[n].fc = 0;
        }
        for n in 0..D_CODES {
            self.dyn_dtree[n].fc = 0;
        }
        for n in 0..BL_CODES {
            self.bl_tree[n].fc = 0;
        }
        self.dyn_ltree[END_BLOCK].fc = 1;
        self.opt_len = 0;
        self.static_len = 0;
        self.sym_next = 0;
    }
    fn smaller(tree: &[Ct], depth: &[u8], n: usize, m: usize) -> bool {
        tree[n].fc < tree[m].fc || (tree[n].fc == tree[m].fc && depth[n] <= depth[m])
    }
    fn pqdownheap(&mut self, d: Desc, mut k: usize) {
        let v = self.heap[k];
        let mut j = k << 1;
        while j <= self.heap_len {
            let (heap, depth) = (self.heap, self.depth);
            let tree: &[Ct] = match d {
                Desc::L => &self.dyn_ltree,
                Desc::D => &self.dyn_dtree,
                Desc::Bl => &self.bl_tree,
            };
            if j < self.heap_len && Self::smaller(tree, &depth, heap[j + 1], heap[j]) {
                j += 1;
            }
            if Self::smaller(tree, &depth, v, heap[j]) {
                break;
            }
            self.heap[k] = self.heap[j];
            k = j;
            j <<= 1;
        }
        self.heap[k] = v;
    }
    fn build_tree(&mut self, d: Desc) {
        let t = tables();
        let (stree, elems): (Option<&[Ct]>, usize) = match d {
            Desc::L => (Some(&t.static_ltree[..]), L_CODES),
            Desc::D => (Some(&t.static_dtree[..]), D_CODES),
            Desc::Bl => (None, BL_CODES),
        };
        let mut max_code: i64 = -1;
        self.heap_len = 0;
        self.heap_max = HEAP_SIZE;
        for n in 0..elems {
            if self.tree(d)[n].fc != 0 {
                self.heap_len += 1;
                self.heap[self.heap_len] = n;
                max_code = n as i64;
                self.depth[n] = 0;
            } else {
                self.tree(d)[n].dl = 0;
            }
        }
        while self.heap_len < 2 {
            let node = if max_code < 2 {
                max_code += 1;
                max_code as usize
            } else {
                0
            };
            self.heap_len += 1;
            self.heap[self.heap_len] = node;
            self.tree(d)[node].fc = 1;
            self.depth[node] = 0;
            self.opt_len = self.opt_len.wrapping_sub(1);
            if let Some(st) = stree {
                self.static_len = self.static_len.wrapping_sub(st[node].dl as u64);
            }
        }
        let max_code = max_code as usize;
        match d {
            Desc::L => self.l_max_code = max_code,
            Desc::D => self.d_max_code = max_code,
            Desc::Bl => self.bl_max_code = max_code,
        }
        let mut n = self.heap_len / 2;
        while n >= 1 {
            self.pqdownheap(d, n);
            n -= 1;
        }
        let mut node = elems;
        loop {
            // pqremove
            let n = self.heap[1];
            self.heap[1] = self.heap[self.heap_len];
            self.heap_len -= 1;
            self.pqdownheap(d, 1);
            let m = self.heap[1];
            self.heap_max -= 1;
            self.heap[self.heap_max] = n;
            self.heap_max -= 1;
            self.heap[self.heap_max] = m;
            let f = self.tree(d)[n].fc.wrapping_add(self.tree(d)[m].fc);
            self.tree(d)[node].fc = f;
            self.depth[node] = (if self.depth[n] >= self.depth[m] { self.depth[n] } else { self.depth[m] }) + 1;
            self.tree(d)[n].dl = node as u16;
            self.tree(d)[m].dl = node as u16;
            self.heap[1] = node;
            node += 1;
            self.pqdownheap(d, 1);
            if self.heap_len < 2 {
                break;
            }
        }
        self.heap_max -= 1;
        self.heap[self.heap_max] = self.heap[1];
        self.gen_bitlen(d, max_code);
        let bl_count = self.bl_count;
        gen_codes(self.tree(d), max_code, &bl_count);
    }
    fn gen_bitlen(&mut self, d: Desc, max_code: usize) {
        let t = tables();
        let (stree, extra, base, max_length): (Option<&[Ct]>, &[u32], usize, usize) = match d {
            Desc::L => (Some(&t.static_ltree[..]), &EXTRA_LBITS[..], LITERALS + 1, MAX_BITS),
            Desc::D => (Some(&t.static_dtree[..]), &EXTRA_DBITS[..], 0, MAX_BITS),
            Desc::Bl => (None, &EXTRA_BLBITS[..], 0, MAX_BL_BITS),
        };
        self.bl_count = [0; MAX_BITS + 1];
        let root = self.heap[self.heap_max];
        self.tree(d)[root].dl = 0;
        let mut overflow: i64 = 0;
        let mut h = self.heap_max + 1;
        while h < HEAP_SIZE {
            let n = self.heap[h];
            let dad = self.tree(d)[n].dl as usize;
            let mut bits = self.tree(d)[dad].dl as usize + 1;
            if bits > max_length {
                bits = max_length;
                overflow += 1;
            }
            self.tree(d)[n].dl = bits as u16;
            h += 1;
            if n > max_code {
                continue;
            }
            self.bl_count[bits] += 1;
            let xbits = if n >= base { extra[n - base] as u64 } else { 0 };
            let f = self.tree(d)[n].fc as u64;
            self.opt_len = self.opt_len.wrapping_add(f * (bits as u64 + xbits));
            if let Some(st) = stree {
                self.static_len = self.static_len.wrapping_add(f * (st[n].dl as u64 + xbits));
            }
        }
        if overflow == 0 {
            return;
        }
        loop {
            let mut bits = max_length - 1;
            while self.bl_count[bits] == 0 {
                bits -= 1;
            }
            self.bl_count[bits] -= 1;
            self.bl_count[bits + 1] += 2;
            self.bl_count[max_length] -= 1;
            overflow -= 2;
            if overflow <= 0 {
                break;
            }
        }
        let mut h = HEAP_SIZE;
        let mut bits = max_length;
        while bits != 0 {
            let mut n = self.bl_count[bits];
            while n != 0 {
                h -= 1;
                let m = self.heap[h];
                if m > max_code {
                    continue;
                }
                let len = self.tree(d)[m].dl as usize;
                if len != bits {
                    let f = self.tree(d)[m].fc as u64;
                    self.opt_len = self.opt_len.wrapping_add((bits as u64).wrapping_sub(len as u64).wrapping_mul(f));
                    self.tree(d)[m].dl = bits as u16;
                }
                n -= 1;
            }
            bits -= 1;
        }
    }
    fn scan_tree(&mut self, d: Desc, max_code: usize) {
        let mut prevlen: i32 = -1;
        let mut nextlen = self.tree(d)[0].dl as i32;
        let mut count = 0;
        let (mut max_count, mut min_count) = (7, 4);
        if nextlen == 0 {
            max_count = 138;
            min_count = 3;
        }
        self.tree(d)[max_code + 1].dl = 0xffff;
        for n in 0..=max_code {
            let curlen = nextlen;
            nextlen = self.tree(d)[n + 1].dl as i32;
            count += 1;
            if count < max_count && curlen == nextlen {
                continue;
            } else if count < min_count {
                self.bl_tree[curlen as usize].fc += count as u16;
            } else if curlen != 0 {
                if curlen != prevlen {
                    self.bl_tree[curlen as usize].fc += 1;
                }
                self.bl_tree[REP_3_6].fc += 1;
            } else if count <= 10 {
                self.bl_tree[REPZ_3_10].fc += 1;
            } else {
                self.bl_tree[REPZ_11_138].fc += 1;
            }
            count = 0;
            prevlen = curlen;
            if nextlen == 0 {
                max_count = 138;
                min_count = 3;
            } else if curlen == nextlen {
                max_count = 6;
                min_count = 3;
            } else {
                max_count = 7;
                min_count = 4;
            }
        }
    }
    fn send_tree(&mut self, d: Desc, max_code: usize) {
        let mut prevlen: i32 = -1;
        let mut nextlen = self.tree(d)[0].dl as i32;
        let mut count = 0;
        let (mut max_count, mut min_count) = (7, 4);
        if nextlen == 0 {
            max_count = 138;
            min_count = 3;
        }
        let bl = self.bl_tree;
        for n in 0..=max_code {
            let curlen = nextlen;
            nextlen = self.tree(d)[n + 1].dl as i32;
            count += 1;
            if count < max_count && curlen == nextlen {
                continue;
            } else if count < min_count {
                loop {
                    self.send_code(curlen as usize, &bl);
                    count -= 1;
                    if count == 0 {
                        break;
                    }
                }
            } else if curlen != 0 {
                if curlen != prevlen {
                    self.send_code(curlen as usize, &bl);
                    count -= 1;
                }
                self.send_code(REP_3_6, &bl);
                self.send_bits((count - 3) as u32, 2);
            } else if count <= 10 {
                self.send_code(REPZ_3_10, &bl);
                self.send_bits((count - 3) as u32, 3);
            } else {
                self.send_code(REPZ_11_138, &bl);
                self.send_bits((count - 11) as u32, 7);
            }
            count = 0;
            prevlen = curlen;
            if nextlen == 0 {
                max_count = 138;
                min_count = 3;
            } else if curlen == nextlen {
                max_count = 6;
                min_count = 3;
            } else {
                max_count = 7;
                min_count = 4;
            }
        }
    }
    fn build_bl_tree(&mut self) -> usize {
        let (l, dm) = (self.l_max_code, self.d_max_code);
        self.scan_tree(Desc::L, l);
        self.scan_tree(Desc::D, dm);
        self.build_tree(Desc::Bl);
        let mut max_blindex = BL_CODES - 1;
        while max_blindex >= 3 {
            if self.bl_tree[BL_ORDER[max_blindex]].dl != 0 {
                break;
            }
            max_blindex -= 1;
        }
        self.opt_len = self.opt_len.wrapping_add(3 * (max_blindex as u64 + 1) + 5 + 5 + 4);
        max_blindex
    }
    fn send_all_trees(&mut self, lcodes: usize, dcodes: usize, blcodes: usize) {
        self.send_bits((lcodes - 257) as u32, 5);
        self.send_bits((dcodes - 1) as u32, 5);
        self.send_bits((blcodes - 4) as u32, 4);
        for rank in 0..blcodes {
            let l = self.bl_tree[BL_ORDER[rank]].dl as u32;
            self.send_bits(l, 3);
        }
        self.send_tree(Desc::L, lcodes - 1);
        self.send_tree(Desc::D, dcodes - 1);
    }
    fn compress_block(&mut self, ltree: &[Ct], dtree: &[Ct]) {
        let t = tables();
        let mut sx = 0;
        while sx < self.sym_next {
            let mut dist = self.sym_buf[sx] as u32 | (self.sym_buf[sx + 1] as u32) << 8;
            let mut lc = self.sym_buf[sx + 2] as u32;
            sx += 3;
            if dist == 0 {
                self.send_code(lc as usize, ltree);
            } else {
                let code = t.length_code[lc as usize] as usize;
                self.send_code(code + LITERALS + 1, ltree);
                let extra = EXTRA_LBITS[code];
                if extra != 0 {
                    lc -= t.base_length[code];
                    self.send_bits(lc, extra);
                }
                dist -= 1;
                let code = (if dist < 256 { t.dist_code[dist as usize] } else { t.dist_code[256 + (dist as usize >> 7)] }) as usize;
                self.send_code(code, dtree);
                let extra = EXTRA_DBITS[code];
                if extra != 0 {
                    dist -= t.base_dist[code];
                    self.send_bits(dist, extra);
                }
            }
        }
        self.send_code(END_BLOCK, ltree);
    }
    fn flush_block(&mut self, last: bool) {
        let stored_len = (self.strstart as i64 - self.block_start) as usize;
        let have_buf = self.block_start >= 0;
        let t = tables();
        self.build_tree(Desc::L);
        self.build_tree(Desc::D);
        let max_blindex = self.build_bl_tree();
        let mut opt_lenb = (self.opt_len.wrapping_add(3 + 7)) >> 3;
        let static_lenb = (self.static_len.wrapping_add(3 + 7)) >> 3;
        if static_lenb <= opt_lenb {
            opt_lenb = static_lenb;
        }
        if stored_len as u64 + 4 <= opt_lenb && have_buf {
            // _tr_stored_block
            self.send_bits(last as u32, 3);
            self.bi_windup();
            self.out.extend_from_slice(&(stored_len as u16).to_le_bytes());
            self.out.extend_from_slice(&(!(stored_len as u16)).to_le_bytes());
            let bs = self.block_start as usize;
            let data = self.window[bs..bs + stored_len].to_vec();
            self.out.extend_from_slice(&data);
        } else if static_lenb == opt_lenb {
            self.send_bits((1 << 1) + last as u32, 3);
            self.compress_block(&t.static_ltree, &t.static_dtree);
        } else {
            self.send_bits((2 << 1) + last as u32, 3);
            let (l, d) = (self.l_max_code, self.d_max_code);
            self.send_all_trees(l + 1, d + 1, max_blindex + 1);
            let (lt, dt) = (self.dyn_ltree, self.dyn_dtree);
            self.compress_block(&lt, &dt);
        }
        self.init_block();
        if last {
            self.bi_windup();
        }
        self.block_start = self.strstart as i64;
    }

    /// `deflate_slow`, called once with `Z_NO_FLUSH` for the whole input and once with `Z_FINISH`.
    fn deflate_slow(&mut self, finish: bool) {
        loop {
            if self.lookahead < MIN_LOOKAHEAD {
                self.fill_window();
                if self.lookahead < MIN_LOOKAHEAD && !finish {
                    return;
                }
                if self.lookahead == 0 {
                    break;
                }
            }
            let mut hash_head = NIL;
            if self.lookahead >= MIN_MATCH {
                hash_head = self.insert_string(self.strstart);
            }
            self.prev_length = self.match_length;
            self.prev_match = self.match_start;
            self.match_length = MIN_MATCH - 1;
            if hash_head != NIL && self.prev_length < MAX_LAZY && self.strstart - hash_head <= MAX_DIST {
                self.match_length = self.longest_match(hash_head);
                if self.match_length <= 5 && self.match_length == MIN_MATCH && self.strstart - self.match_start > TOO_FAR {
                    self.match_length = MIN_MATCH - 1;
                }
            }
            if self.prev_length >= MIN_MATCH && self.match_length <= self.prev_length {
                let max_insert = self.strstart + self.lookahead - MIN_MATCH;
                let bflush = self.tally_dist(self.strstart - 1 - self.prev_match, self.prev_length - MIN_MATCH);
                self.lookahead -= self.prev_length - 1;
                self.prev_length -= 2;
                loop {
                    self.strstart += 1;
                    if self.strstart <= max_insert {
                        let _ = self.insert_string(self.strstart);
                    }
                    self.prev_length -= 1;
                    if self.prev_length == 0 {
                        break;
                    }
                }
                self.match_available = false;
                self.match_length = MIN_MATCH - 1;
                self.strstart += 1;
                if bflush {
                    self.flush_block(false);
                }
            } else if self.match_available {
                let c = self.window[self.strstart - 1];
                let bflush = self.tally_lit(c);
                if bflush {
                    self.flush_block(false);
                }
                self.strstart += 1;
                self.lookahead -= 1;
            } else {
                self.match_available = true;
                self.strstart += 1;
                self.lookahead -= 1;
            }
        }
        if self.match_available {
            let c = self.window[self.strstart - 1];
            let _ = self.tally_lit(c);
            self.match_available = false;
        }
        self.insert = if self.strstart < MIN_MATCH - 1 { self.strstart } else { MIN_MATCH - 1 };
        self.flush_block(true);
    }
}

/// Raw deflate of `input` as zlib 1.3.1 (level 6, window bits -15, memory level 8, default strategy) writes it.
pub fn deflate_raw_level6(input: &[u8]) -> Vec<u8> {
    let mut d = Deflate {
        input,
        next_in: 0,
        window: vec![0; WINDOW_SIZE],
        prev: vec![0; W_SIZE],
        head: vec![0; HASH_SIZE],
        ins_h: 0,
        strstart: 0,
        block_start: 0,
        lookahead: 0,
        insert: 0,
        match_length: MIN_MATCH - 1,
        prev_length: MIN_MATCH - 1,
        match_available: false,
        match_start: 0,
        prev_match: 0,
        high_water: 0,
        sym_buf: vec![0; LIT_BUFSIZE * 3],
        sym_next: 0,
        dyn_ltree: [Ct::default(); HEAP_SIZE],
        dyn_dtree: [Ct::default(); 2 * D_CODES + 1],
        bl_tree: [Ct::default(); 2 * BL_CODES + 1],
        l_max_code: 0,
        d_max_code: 0,
        bl_max_code: 0,
        bl_count: [0; MAX_BITS + 1],
        heap: [0; HEAP_SIZE],
        heap_len: 0,
        heap_max: 0,
        depth: [0; HEAP_SIZE],
        opt_len: 0,
        static_len: 0,
        out: Vec::with_capacity(input.len() / 2 + 64),
        bi_buf: 0,
        bi_valid: 0,
    };
    d.init_block();
    d.deflate_slow(false);
    d.deflate_slow(true);
    d.out
}

#[cfg(test)]
mod tests {
    use super::deflate_raw_level6;

    fn fnv(b: &[u8]) -> u64 {
        b.iter().fold(0xcbf29ce484222325u64, |h, &x| (h ^ x as u64).wrapping_mul(0x100000001b3))
    }

    /// Against zlib 1.3.1 / Ionic.Zlib (lengths and FNV-1a 64 of their output, computed with Python's zlib, which
    /// Ionic's DeflateStream equals on these inputs): slices of the reference world file, zeros, a short text.
    #[test]
    fn equals_zlib_level6() {
        let w = std::fs::read(crate::sys::io::project_dir().join("work/refsave/Worlds/ref_small.wld")).expect("work/refsave/Worlds/ref_small.wld");
        let cases: Vec<(&str, Vec<u8>)> = vec![
            ("small", b"hello hello hello".to_vec()),
            ("world100k", w[200000..300000].to_vec()),
            ("world1m", w[1000000..2000000].to_vec()),
            ("zeros", vec![0u8; 300000]),
            ("mix", [&w[0..5000], &vec![0u8; 5000][..], &w[50000..60000]].concat()),
        ];
        let want = include_str!("zlib_deflate_expected.txt");
        for (name, data) in cases {
            let out = deflate_raw_level6(&data);
            let line = format!("{name} {} {:016x}", out.len(), fnv(&out));
            assert!(want.lines().any(|l| l == line), "{line} is not in zlib_deflate_expected.txt");
        }
    }
}

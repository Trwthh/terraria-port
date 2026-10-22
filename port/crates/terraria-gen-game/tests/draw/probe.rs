// The probe run (`DRAW_MODE=probe`, refdata/probe): tools/refdump_draw/Probe.cs, step for step. Included by
// tests/draw.rs. Besides the scenes, digests and state lines: probe lines (return values, out values, tile hashes,
// fields; in scenes.state) and direct draw scenes (draw calls of a method called outside the frame loop).

/// Probe.cs `PV`: a value as the reference writes it (floats as their bits).
trait Pv {
    fn pv(&self) -> String;
}
impl Pv for f32 {
    fn pv(&self) -> String {
        format!("{:08x}", self.to_bits())
    }
}
impl Pv for f64 {
    fn pv(&self) -> String {
        format!("{:016x}", self.to_bits())
    }
}
impl Pv for bool {
    fn pv(&self) -> String {
        (if *self { "1" } else { "0" }).to_owned()
    }
}
macro_rules! pv_int {
    ($($t:ty),*) => { $(impl Pv for $t { fn pv(&self) -> String { self.to_string() } })* };
}
pv_int!(i8, u8, i16, u16, i32, u32, i64, u64, usize);
impl Pv for terraria_core::xna::Vector2 {
    fn pv(&self) -> String {
        format!("{},{}", self.x.pv(), self.y.pv())
    }
}
impl Pv for terraria_core::xna::Color {
    fn pv(&self) -> String {
        format!("{:08x}", self.packed_value())
    }
}
impl Pv for terraria_core::xna::Rectangle {
    fn pv(&self) -> String {
        format!("{},{},{},{}", self.x, self.y, self.width, self.height)
    }
}
impl Pv for terraria_core::xna::Point {
    fn pv(&self) -> String {
        format!("{},{}", self.x, self.y)
    }
}
impl Pv for Str {
    fn pv(&self) -> String {
        if self.is_null() {
            return "-".to_owned();
        }
        format!("\"{}\"", self.as_str().replace('\n', "\\n"))
    }
}
impl Pv for &str {
    fn pv(&self) -> String {
        format!("\"{}\"", self.replace('\n', "\\n"))
    }
}
impl Pv for String {
    fn pv(&self) -> String {
        format!("\"{}\"", self.replace('\n', "\\n"))
    }
}

impl Run {
    /// Probe.cs `P`: one probe line in scenes.state.
    fn probe(&mut self, key: &str, vals: &[&dyn Pv]) {
        let mut s = format!("probe {key}");
        for v in vals {
            s.push(' ');
            s.push_str(&v.pv());
        }
        s.push('\n');
        self.state.push_str(&s);
    }
    /// Probe.cs `PTiles`.
    unsafe fn probe_tiles(&mut self, key: &str, x: i32, y: i32, w: i32, h: i32) {
        let s = format!("probe {key} tiles {x},{y},{w},{h} {:016x}\n", tile_hash(x, y, w, h));
        self.state.push_str(&s);
    }
    /// Probe.cs `DirectDraw`: the draw calls of `f`, called outside the frame loop, as a scene of its own.
    unsafe fn direct(&mut self, scene: &str, f: impl FnOnce(&mut Run)) {
        trace::set_full(true);
        let _ = trace::take_output();
        let mut body: Vec<u8> = b"frame direct\n".to_vec();
        f(self);
        trace::set_full(false);
        body.extend_from_slice(&trace::take_output());
        std::fs::write(self.out_dir.join(format!("{scene}.trace")), &body).expect("write trace");
        self.state.push_str(&state_line(scene));
        self.compare_scene(scene, body, "direct");
    }
}

/// Probe.cs `Group` (`PROBE_ONLY`).
fn probe_group(r: &mut Run, name: &str) -> bool {
    let on = std::env::var("PROBE_ONLY").ok().filter(|v| !v.is_empty()).is_none_or(|v| v.split(',').any(|g| g == name));
    if on {
        r.note(format!("probe group {name}"));
    }
    on
}

/// Probe.cs `TileHash`.
unsafe fn tile_hash(x: i32, y: i32, w: i32, h: i32) -> u64 {
    fn put(h: &mut u64, v: i64, n: u32) {
        for k in 0..n {
            *h ^= ((v >> (8 * k)) & 0xff) as u64;
            *h = h.wrapping_mul(1099511628211);
        }
    }
    let m = terraria::main::s();
    let (mx, my) = ((*m).max_tiles_x, (*m).max_tiles_y);
    let mut hsh = 14695981039346656037u64;
    for i in x.max(0)..(x + w).min(mx) {
        for j in y.max(0)..(y + h).min(my) {
            let t = gallery_tile(i, j);
            if t.is_null() {
                put(&mut hsh, 0xff, 1);
                continue;
            }
            let t = &*t.ptr();
            for v in [t.type_ as i64, t.wall as i64, t.liquid as i64, t.s_tile_header as i64, t.b_tile_header as i64, t.b_tile_header2 as i64, t.b_tile_header3 as i64, t.frame_x as i64, t.frame_y as i64] {
                put(&mut hsh, v, 2);
            }
        }
    }
    hsh
}

/// Probe.cs `InBatch`: Main.spriteBatch.Begin() ... End() around `f`.
unsafe fn in_batch(f: impl FnOnce()) {
    let sb = (*terraria::main::s()).sprite_batch;
    (*sb.ptr()).begin();
    f();
    (*sb.ptr()).end();
}

unsafe fn npc_at(i: i32) -> Gc<terraria::npc::NPC> {
    *(*terraria::main::s()).npc.at(i)
}
unsafe fn proj_at(i: i32) -> Gc<terraria::projectile::Projectile> {
    *(*terraria::main::s()).projectile.at(i)
}
unsafe fn src() -> Gc<rt::Object> {
    terraria::data_structures::entity_source_debug_command::new().up::<rt::Object>()
}

/// Probe.cs `ProbeWorld`.
unsafe fn probe_world(r: &mut Run) {
    let m = terraria::main::s();
    let (sx, sy) = ((*m).spawn_tile_x, (*m).spawn_tile_y);
    set_time(true, 27000.0);
    place_player(sx, sy);
    gallery_immune();
    r.frames(30);
    let x0 = sx + BUILD_DX;
    let gy = ground_below(x0, sy - 12);
    r.note(format!("probe ground at {x0},{gy}"));
    for x in x0 - 2..=x0 + BUILD_W {
        for y in gy - 12..gy {
            kill_tile(x, y);
        }
        for y in gy..=gy + 1 {
            place_tile(x, y, 0, 0);
        }
    }
    r.frames(5);
    probe_a(r, sx, sy, x0, gy);
    probe_b(r, sx, sy, x0, gy);
    probe_c(r, sx, sy, x0, gy);
    probe_d(r, sx, sy, x0, gy);
    probe_e(r, sx, sy, x0, gy);
    probe_f(r, sx, sy, x0, gy);
    probe_g(r, sx, sy, x0, gy);
    probe_h(r, sx, sy, x0, gy);
    probe_i(r, sx, sy, x0, gy);
    probe_j(r, sx, sy, x0, gy);
    probe_k(r, sx, sy, x0, gy);
    probe_l(r, sx, sy, x0, gy);
    probe_m(r, sx, sy, x0, gy);
    probe_n(r, sx, sy, x0, gy);
    probe_o(r, sx, sy, x0, gy);
    probe_p(r, sx, sy, x0, gy);
    probe_q(r, sx, sy, x0, gy);
    probe_r(r, sx, sy, x0, gy);
    probe_s(r, sx, sy, x0, gy);
}

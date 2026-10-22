//! The translated `Terraria.Collision` against the real game: 62 method families x 20,000 recorded calls
//! (`tools/refdump_collision`; layout of the case files in `Cases.cs`, mirrored from
//! `terraria-sim/tests/collision_reference.rs`, which runs the hand-written port on the same files).
//!
//! State before every call, as the harness set it: the eight `Collision` flags false, `Main.getGoodWorld` /
//! `Main.dontStarveWorld` false, `Main.netMode = 0`, `Main.rand = new UnifiedRandom(seed)`. The world is the
//! reference world with the harness' stamps, verified against the per-column hashes the harness wrote.
//! Compared bit for bit: return value, `ref` / `out` parameters, the flags, and whether the call threw.

use crate::game::{self, Report, RECORD};
use std::collections::BTreeMap;
use terraria_core::xna::{Point, Rectangle, Vector2, Vector4};
use terraria_gen_game::gen::terraria;
use terraria_gen_game::gen::terraria::collision as c;
use terraria_gen_game::rt::{self, Arr, Delegate, Gc, List};

struct Case {
    threw: bool,
    inputs: Vec<u32>,
    outputs: Vec<u32>,
}

fn read_cases(family: &str) -> Vec<Case> {
    let path = game::root().join("refdata/collision").join(format!("{family}.bin"));
    let d = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert_eq!(&d[0..4], b"CLSN");
    assert_eq!(u32::from_le_bytes(d[4..8].try_into().unwrap()), 1);
    let word = |p: usize| u32::from_le_bytes(d[p..p + 4].try_into().unwrap());
    let mut p = 8;
    let mut cases = Vec::new();
    while p < d.len() {
        let threw = d[p] != 0;
        let (n_in, n_out) = (word(p + 1) as usize, word(p + 5) as usize);
        p += 9;
        let inputs = (0..n_in).map(|k| word(p + k * 4)).collect();
        p += n_in * 4;
        let outputs = (0..n_out).map(|k| word(p + k * 4)).collect();
        p += n_out * 4;
        cases.push(Case { threw, inputs, outputs });
    }
    cases
}

/// Reads the input words of a case.
struct R<'a> {
    w: &'a [u32],
    p: usize,
}

impl R<'_> {
    fn u32(&mut self) -> u32 {
        let v = self.w[self.p];
        self.p += 1;
        v
    }
    fn i32(&mut self) -> i32 {
        self.u32() as i32
    }
    fn bool(&mut self) -> bool {
        self.u32() != 0
    }
    fn f32(&mut self) -> f32 {
        f32::from_bits(self.u32())
    }
    fn v2(&mut self) -> Vector2 {
        let x = self.f32();
        let y = self.f32();
        Vector2::new(x, y)
    }
    fn pt(&mut self) -> Point {
        let x = self.i32();
        let y = self.i32();
        Point::new(x, y)
    }
    fn rect(&mut self) -> Rectangle {
        let (x, y, w, h) = (self.i32(), self.i32(), self.i32(), self.i32());
        Rectangle::new(x, y, w, h)
    }
    fn pts(&mut self) -> Gc<List<Point>> {
        let n = self.u32();
        let l = List::<Point>::new_gc();
        for _ in 0..n {
            let p = self.pt();
            unsafe { (*l.ptr()).add(p) };
        }
        l
    }
}

/// Output words with a mark for the float ones.
#[derive(Default)]
struct Out {
    w: Vec<u32>,
    is_f: Vec<bool>,
}

impl Out {
    fn i(&mut self, v: i32) -> &mut Self {
        self.w.push(v as u32);
        self.is_f.push(false);
        self
    }
    fn b(&mut self, v: bool) -> &mut Self {
        self.i(v as i32)
    }
    fn f(&mut self, v: f32) -> &mut Self {
        self.w.push(v.to_bits());
        self.is_f.push(true);
        self
    }
    fn v2(&mut self, v: Vector2) -> &mut Self {
        self.f(v.x).f(v.y)
    }
    fn v4(&mut self, v: Vector4) -> &mut Self {
        self.f(v.x).f(v.y).f(v.z).f(v.w)
    }
    fn pt(&mut self, p: Point) -> &mut Self {
        self.i(p.x).i(p.y)
    }
    fn rect(&mut self, r: Rectangle) -> &mut Self {
        self.i(r.x).i(r.y).i(r.width).i(r.height)
    }
    unsafe fn pts(&mut self, l: Gc<List<Point>>) -> &mut Self {
        let n = (*l.ptr()).count();
        self.i(n);
        for k in 0..n {
            let p = (*l.ptr()).get_item(k);
            self.pt(p);
        }
        self
    }
    fn floats(&mut self, a: Arr<f32>) -> &mut Self {
        self.i(a.len());
        for v in a.as_slice() {
            self.f(*v);
        }
        self
    }
    /// The eight static flags of `Collision` in one word: bit 0..7 = up, down, stair, stairFall, honey, shimmer, sloping, landMine.
    unsafe fn flags(&mut self) -> &mut Self {
        let s = c::s();
        let bits = [(*s).up, (*s).down, (*s).stair, (*s).stair_fall, (*s).honey, (*s).shimmer, (*s).sloping, (*s).land_mine];
        let mut w = 0i32;
        for (k, b) in bits.iter().enumerate() {
            if *b {
                w |= 1 << k;
            }
        }
        self.i(w)
    }
}

unsafe fn reset_state(seed: u32) {
    let s = c::s();
    (*s).up = false;
    (*s).down = false;
    (*s).stair = false;
    (*s).stair_fall = false;
    (*s).honey = false;
    (*s).shimmer = false;
    (*s).sloping = false;
    (*s).land_mine = false;
    let main = terraria::main::s();
    (*main).get_good_world = false;
    (*main).dont_starve_world = false;
    (*main).net_mode = 0;
    seed_rand(seed);
}

/// `Main.rand = new UnifiedRandom(seed)`: the harness set it after building the arguments, right before the call.
unsafe fn seed_rand(seed: u32) {
    (*terraria::main::s()).rand = rt::alloc_init(terraria_core::UnifiedRandom::new(seed as i32));
}

/// The mask the harness derives from a seed (`Mask` in Cases.cs).
fn mask(seed: u32, len: usize) -> Arr<bool> {
    let v: Vec<bool> = (0..len).map(|t| (((t as u32).wrapping_mul(2654435761) ^ seed.wrapping_mul(40503)) >> 11 & 3) == 0).collect();
    Arr::from_slice(&v)
}

/// `MakeEntity` of Cases.cs: `Activator.CreateInstance(type)`, then position / width / height.
unsafe fn place(e: Gc<terraria::entity::Entity>, x: f32, y: f32, w: i32, h: i32) -> Gc<terraria::entity::Entity> {
    (*e.ptr()).position = Vector2::new(x, y);
    (*e.ptr()).width = w;
    (*e.ptr()).height = h;
    e
}

unsafe fn player_arg(seed: u32, which: i32) -> Gc<terraria::player::Player> {
    if which < 0 {
        return Gc::NULL;
    }
    let p = terraria::player::new();
    (*p.ptr()).fire_walk = which == 1;
    seed_rand(seed);
    p
}

type Family = (&'static str, Box<dyn Fn(u32, &mut R) -> Out>);

fn families() -> Vec<Family> {
    let mut f: Vec<Family> = Vec::new();
    macro_rules! fam {
        ($name:literal, |$seed:ident, $r:ident, $o:ident| $body:block) => {
            f.push((
                $name,
                Box::new(|$seed: u32, $r: &mut R| {
                    let _ = $seed;
                    let mut $o = Out::default();
                    #[allow(unused_unsafe)]
                    unsafe {
                        $body
                    }
                    $o
                }),
            ));
        };
    }

    // ---- pure geometry
    fam!("CheckLinevLine", |_s, r, o| {
        let list = c::check_linev_line(r.v2(), r.v2(), r.v2(), r.v2());
        o.i(list.len());
        for v in list.as_slice() {
            o.v2(*v);
        }
        o.flags();
    });
    fam!("CheckAABBvAABBCollision", |_s, r, o| {
        let v = c::check_aab_bv_aabb_collision(r.v2(), r.v2(), r.v2(), r.v2());
        o.b(v).flags();
    });
    fam!("CheckAABBvLineCollision4", |_s, r, o| {
        let v = c::check_aab_bv_line_collision__vector2_vector2_vector2_vector2(r.v2(), r.v2(), r.v2(), r.v2());
        o.b(v).flags();
    });
    fam!("CheckAABBvLineCollision2", |_s, r, o| {
        let v = c::check_aab_bv_line_collision2(r.v2(), r.v2(), r.v2(), r.v2());
        o.b(v).flags();
    });
    fam!("CheckAABBvLineCollision6", |_s, r, o| {
        let (a, b, cc, d) = (r.v2(), r.v2(), r.v2(), r.v2());
        let line_width = r.f32();
        let mut collision_point = r.f32();
        let v = c::check_aab_bv_line_collision__vector2_vector2_vector2_vector2_float_ref_float(a, b, cc, d, line_width, &mut collision_point);
        o.b(v).f(collision_point).flags();
    });

    // ---- lines of sight
    fam!("CanHit_Vector2", |_s, r, o| {
        let v = c::can_hit__vector2_int_int_vector2_int_int(r.v2(), r.i32(), r.i32(), r.v2(), r.i32(), r.i32());
        o.b(v).flags();
    });
    fam!("CanHit_Point", |_s, r, o| {
        let v = c::can_hit__point_int_int_point_int_int(r.pt(), r.i32(), r.i32(), r.pt(), r.i32(), r.i32());
        o.b(v).flags();
    });
    fam!("CanHitLine", |_s, r, o| {
        let v = c::can_hit_line(r.v2(), r.i32(), r.i32(), r.v2(), r.i32(), r.i32());
        o.b(v).flags();
    });
    fam!("CanHitWithCheck", |_s, r, o| {
        let cs = r.u32();
        let (p1, w1, h1, p2, w2, h2) = (r.v2(), r.i32(), r.i32(), r.v2(), r.i32(), r.i32());
        let calls = std::rc::Rc::new(std::cell::Cell::new(0u32));
        let counter = calls.clone();
        // CheckImpl of Cases.cs
        let check: Delegate<dyn Fn(i32, i32) -> bool> = Delegate::new(Box::new(move |x: i32, y: i32| {
            counter.set(counter.get() + 1);
            ((x.wrapping_mul(73856093) as u32) ^ (y.wrapping_mul(19349663) as u32) ^ cs) & 7 != 0
        }));
        let v = c::can_hit_with_check__vector2_int_int_vector2_int_int_tile_action_attempt(p1, w1, h1, p2, w2, h2, check);
        o.b(v).i(calls.get() as i32).flags();
    });
    fam!("HitLine", |_s, r, o| {
        let (x1, y1, x2, y2, ix, iy) = (r.i32(), r.i32(), r.i32(), r.i32(), r.i32(), r.i32());
        let targets = r.pts();
        let mut col = Point::new(0, 0);
        let v = c::hit_line(x1, y1, x2, y2, ix, iy, targets, &mut col);
        o.b(v).pt(col).flags();
    });
    fam!("AnyWallOfTypeOnLine", |_s, r, o| {
        let v = c::any_wall_of_type_on_line(r.i32(), r.i32(), r.i32(), r.i32(), r.u32() as u16);
        o.b(v).flags();
    });
    fam!("HitLineWall", |_s, r, o| {
        let p = c::hit_line_wall(r.i32(), r.i32(), r.i32(), r.i32());
        o.pt(p).flags();
    });
    fam!("HitSpecificWallSubstep", |_s, r, o| {
        let v = c::hit_specific_wall_substep(r.i32(), r.i32(), r.u32() as u16);
        o.b(v).flags();
    });
    fam!("HitWallSubstep", |_s, r, o| {
        let v = c::hit_wall_substep(r.i32(), r.i32());
        o.b(v).flags();
    });
    fam!("EmptyTile", |_s, r, o| {
        let v = c::empty_tile(r.i32(), r.i32(), r.bool());
        o.b(v).flags();
    });

    // ---- liquids and points
    fam!("DrownCollision", |_s, r, o| {
        let v = c::drown_collision(r.v2(), r.i32(), r.i32(), r.f32(), r.bool());
        o.b(v).flags();
    });
    fam!("IsWorldPointSolid", |_s, r, o| {
        let v = c::is_world_point_solid(r.v2(), r.bool());
        o.b(v).flags();
    });
    fam!("GetWaterLine_Point", |_s, r, o| {
        let mut h = 0f32;
        let v = c::get_water_line__point_out_float(r.pt(), &mut h);
        o.b(v).f(h).flags();
    });
    fam!("GetWaterLine_Ints", |_s, r, o| {
        let mut h = 0f32;
        let v = c::get_water_line__int_int_out_float(r.i32(), r.i32(), &mut h);
        o.b(v).f(h).flags();
    });
    fam!("GetWaterLineIterate_Point", |_s, r, o| {
        let p = r.pt();
        let mut h = 0f32;
        let res = game::run(|| c::get_water_line_iterate__point_out_float(p, &mut h));
        forget_allocated_tiles(p.x, p.y);
        match res {
            Ok(v) => {
                o.b(v).f(h).flags();
            }
            Err(e) => rethrow(e),
        }
    });
    fam!("GetWaterLineIterate_Ints", |_s, r, o| {
        let (x, y) = (r.i32(), r.i32());
        let mut h = 0f32;
        let res = game::run(|| c::get_water_line_iterate__int_int_out_float(x, y, &mut h));
        forget_allocated_tiles(x, y);
        match res {
            Ok(v) => {
                o.b(v).f(h).flags();
            }
            Err(e) => rethrow(e),
        }
    });
    fam!("WetCollision", |_s, r, o| {
        let v = c::wet_collision(r.v2(), r.i32(), r.i32());
        o.b(v).flags();
    });
    fam!("LavaCollision", |_s, r, o| {
        let v = c::lava_collision(r.v2(), r.i32(), r.i32());
        o.b(v).flags();
    });

    // ---- slopes
    fam!("WalkDownSlope", |_s, r, o| {
        let v = c::walk_down_slope(r.v2(), r.v2(), r.i32(), r.i32(), r.f32());
        o.v4(v).flags();
    });
    fam!("SlopeCollision", |_s, r, o| {
        let v = c::slope_collision(r.v2(), r.v2(), r.i32(), r.i32(), r.f32(), r.bool(), r.bool());
        o.v4(v).flags();
    });
    fam!("noSlopeCollision", |_s, r, o| {
        let v = c::no_slope_collision(r.v2(), r.v2(), r.i32(), r.i32(), r.bool(), r.bool());
        o.v2(v).flags();
    });
    fam!("BuildTileContacts", |_s, r, o| {
        let list = List::<c::tile_contact::TileContact>::new_gc();
        c::build_tile_contacts(r.v2(), r.i32(), r.i32(), list);
        let n = (*list.ptr()).count();
        o.i(n);
        for k in 0..n {
            let t = (*list.ptr()).get_item(k);
            o.i(t.side).i(t.overlap).i(t.x).i(t.y).i(t.slope).i(t.type_);
        }
        o.flags();
    });
    fam!("TileCollision", |_s, r, o| {
        let v = c::tile_collision(r.v2(), r.v2(), r.i32(), r.i32(), r.bool(), r.bool(), r.i32(), r.bool(), r.bool(), r.bool());
        o.v2(v).flags();
    });
    fam!("TileCollisionInStepsOf16", |_s, r, o| {
        let v = c::tile_collision_in_steps_of16(r.v2(), r.v2(), r.f32(), r.i32(), r.i32());
        o.f(v).flags();
    });
    fam!("TryChangingSizeFromBottomCenter", |_s, r, o| {
        let mut rect = Rectangle::new(0, 0, 0, 0);
        let v = c::try_changing_size_from_bottom_center(r.rect(), r.i32(), r.i32(), &mut rect);
        o.b(v).rect(rect).flags();
    });
    fam!("IsClearSpotTest", |_s, r, o| {
        let v = c::is_clear_spot_test(r.v2(), r.f32(), r.i32(), r.i32(), r.bool(), r.bool(), r.i32(), r.bool(), r.bool());
        o.b(v).flags();
    });
    fam!("FindCollisionTile", |_s, r, o| {
        let l = c::find_collision_tile(r.i32(), r.v2(), r.f32(), r.i32(), r.i32(), r.bool(), r.bool(), r.i32(), r.bool(), r.bool());
        o.pts(l).flags();
    });
    fam!("FindCollisionDirection", |_s, r, o| {
        let mut d = 0i32;
        let v = c::find_collision_direction(&mut d, r.v2(), r.i32(), r.i32(), r.bool(), r.bool(), r.i32());
        o.b(v).i(d).flags();
    });
    fam!("SolidCollision3", |_s, r, o| {
        let v = c::solid_collision__vector2_int_int(r.v2(), r.i32(), r.i32());
        o.b(v).flags();
    });
    fam!("SolidCollision4", |_s, r, o| {
        let v = c::solid_collision__vector2_int_int_bool(r.v2(), r.i32(), r.i32(), r.bool());
        o.b(v).flags();
    });
    fam!("WaterCollision", |_s, r, o| {
        let v = c::water_collision(r.v2(), r.v2(), r.i32(), r.i32(), r.bool(), r.bool(), r.bool());
        o.v2(v).flags();
    });
    fam!("AnyCollisionWithSpecificTiles", |_s, r, o| {
        let m = mask(r.u32(), 754);
        let (p, v, w, h) = (r.v2(), r.v2(), r.i32(), r.i32());
        let res = c::any_collision_with_specific_tiles(p, v, w, h, m, r.bool());
        o.v2(res).flags();
    });
    fam!("AnyCollision", |_s, r, o| {
        let v = c::any_collision(r.v2(), r.v2(), r.i32(), r.i32(), r.bool());
        o.v2(v).flags();
    });

    // ---- tile queries
    fam!("AnyHurtingTiles", |_s, r, o| {
        let main = terraria::main::s();
        (*main).get_good_world = r.bool();
        (*main).dont_starve_world = r.bool();
        let _player = r.i32();
        let v = c::any_hurting_tiles(r.v2(), r.i32(), r.i32());
        o.b(v).flags();
    });
    fam!("HurtTiles", |seed, r, o| {
        let main = terraria::main::s();
        (*main).get_good_world = r.bool();
        (*main).dont_starve_world = r.bool();
        let player = player_arg(seed, r.i32());
        let t = c::hurt_tiles(r.v2(), r.i32(), r.i32(), player);
        o.i(t.type_).i(t.x).i(t.y).flags();
    });
    fam!("CanTileHurt", |seed, r, o| {
        let main = terraria::main::s();
        (*main).get_good_world = r.bool();
        (*main).dont_starve_world = r.bool();
        let player = player_arg(seed, r.i32());
        let v = c::can_tile_hurt(r.u32() as u16, r.i32(), r.i32(), player);
        o.b(v).flags();
    });
    fam!("SwitchTiles", |_s, r, o| {
        // the harness saved the 13 x 13 tiles around the tile the entity is near and restored them after the call
        let (tx, ty) = (r.i32(), r.i32());
        let main = terraria::main::s();
        let (x0, y0) = ((tx - 6).max(0), (ty - 6).max(0));
        let (x1, y1) = ((tx + 6).min((*main).max_tiles_x - 1), (ty + 6).min((*main).max_tiles_y - 1));
        let mut saved: Vec<[u8; RECORD]> = Vec::new();
        for x in x0..=x1 {
            for y in y0..=y1 {
                let mut rec = [0u8; RECORD];
                game::read_record(game::tile(x, y).ptr(), &mut rec);
                saved.push(rec);
            }
        }
        let (p, w, h, old, ty_) = (r.v2(), r.i32(), r.i32(), r.v2(), r.i32());
        let res = game::run(|| c::switch_tiles(Gc::NULL, p, w, h, old, ty_));
        let mut k = 0;
        for x in x0..=x1 {
            for y in y0..=y1 {
                let mut cur = [0u8; RECORD];
                game::read_record(game::tile(x, y).ptr(), &mut cur);
                if cur != saved[k] {
                    game::write_record(game::tile(x, y).ptr(), &saved[k]);
                }
                k += 1;
            }
        }
        match res {
            Ok(v) => {
                o.b(v).flags();
            }
            Err(e) => rethrow(e),
        }
    });
    fam!("StickyTiles", |_s, r, o| {
        let v = c::sticky_tiles(r.v2(), r.v2(), r.i32(), r.i32());
        o.v2(v).flags();
    });
    fam!("SolidTilesVersatile", |_s, r, o| {
        let v = c::solid_tiles_versatile(r.i32(), r.i32(), r.i32(), r.i32());
        o.b(v).flags();
    });
    fam!("SolidTiles_Vector2", |_s, r, o| {
        let v = c::solid_tiles__vector2_int_int(r.v2(), r.i32(), r.i32());
        o.b(v).flags();
    });
    fam!("SolidTiles_Ints", |_s, r, o| {
        let v = c::solid_tiles__int_int_int_int(r.i32(), r.i32(), r.i32(), r.i32());
        o.b(v).flags();
    });
    fam!("SolidTiles_Vector2_Top", |_s, r, o| {
        let v = c::solid_tiles__vector2_int_int_bool(r.v2(), r.i32(), r.i32(), r.bool());
        o.b(v).flags();
    });
    fam!("SolidTiles_Ints_Top", |_s, r, o| {
        let v = c::solid_tiles__int_int_int_int_bool(r.i32(), r.i32(), r.i32(), r.i32(), r.bool());
        o.b(v).flags();
    });
    fam!("SolidFullTiles", |_s, r, o| {
        let v = c::solid_full_tiles(r.v2(), r.v2());
        o.b(v).flags();
    });

    // ---- stepping
    fam!("StepDown", |_s, r, o| {
        let (mut pos, mut vel, w, h, mut step, mut gfx, grav_dir, water_walk) = (r.v2(), r.v2(), r.i32(), r.i32(), r.f32(), r.f32(), r.i32(), r.bool());
        c::step_down(&mut pos, &mut vel, w, h, &mut step, &mut gfx, grav_dir, water_walk);
        o.v2(pos).v2(vel).f(step).f(gfx).flags();
    });
    fam!("StepUp", |_s, r, o| {
        let (mut pos, mut vel, w, h, mut step, mut gfx, grav_dir, holds, mode) = (r.v2(), r.v2(), r.i32(), r.i32(), r.f32(), r.f32(), r.i32(), r.bool(), r.i32());
        c::step_up(&mut pos, &mut vel, w, h, &mut step, &mut gfx, grav_dir, holds, mode);
        o.v2(pos).v2(vel).f(step).f(gfx).flags();
    });
    fam!("InTileBounds", |_s, r, o| {
        let v = c::in_tile_bounds(r.i32(), r.i32(), r.i32(), r.i32(), r.i32(), r.i32());
        o.b(v).flags();
    });
    fam!("GetTileRotation", |_s, r, o| {
        let v = c::get_tile_rotation(r.v2());
        o.f(v).flags();
    });
    fam!("GetEntityEdgeTiles", |seed, r, o| {
        let (x, y, w, h) = (r.f32(), r.f32(), r.i32(), r.i32());
        let entity = place(terraria::world_item::new__void().up(), x, y, w, h);
        let p = List::<Point>::new_gc();
        seed_rand(seed);
        c::get_entity_edge_tiles(p, entity, r.bool(), r.bool(), r.bool(), r.bool());
        o.pts(p).flags();
    });
    fam!("GetTilesIn", |_s, r, o| {
        let l = c::get_tiles_in(r.v2(), r.v2());
        o.pts(l).flags();
    });
    fam!("ExpandVertically", |_s, r, o| {
        let (sx, sy) = (r.i32(), r.i32());
        let (mut top, mut bottom) = (0i32, 0i32);
        c::expand_vertically(sx, sy, &mut top, &mut bottom, r.i32(), r.i32());
        o.i(top).i(bottom).flags();
    });
    fam!("AdvancedTileCollision", |_s, r, o| {
        let m = mask(r.u32(), 754);
        let v = c::advanced_tile_collision(m, r.v2(), r.v2(), r.i32(), r.i32(), r.bool(), r.bool(), r.i32());
        o.v2(v).flags();
    });
    fam!("LaserScan", |_s, r, o| {
        let n = r.u32() as i32;
        let samples = Arr::<f32>::new(n);
        c::laser_scan(r.v2(), r.v2(), r.f32(), r.f32(), samples);
        o.floats(samples).flags();
    });
    fam!("AimingLaserScan", |_s, r, o| {
        let mut v = Vector2::new(0.0, 0.0);
        let mut samples: Arr<f32> = Arr::NULL;
        c::aiming_laser_scan(r.v2(), r.v2(), r.f32(), r.i32(), &mut v, &mut samples);
        o.v2(v).floats(samples).flags();
    });

    // ---- conveyor belts
    fam!("ApplyConveyorBeltMovementToVelocity", |seed, r, o| {
        let (x, y, w, h) = (r.f32(), r.f32(), r.i32(), r.i32());
        let item = terraria::world_item::new__void();
        place(item.up(), x, y, w, h);
        let mut velocity = r.v2();
        seed_rand(seed);
        let v = c::apply_conveyor_belt_movement_to_velocity(item, &mut velocity);
        let pos = (*item.up::<terraria::entity::Entity>().ptr()).position;
        o.b(v).v2(velocity).v2(pos).flags();
    });
    fam!("StepConveyorBelt", |seed, r, o| {
        let as_player = r.bool();
        let (x, y, w, h) = (r.f32(), r.f32(), r.i32(), r.i32());
        let (gfx, grap, pulley, on_track) = (r.f32(), r.i32(), r.bool(), r.bool());
        let entity: Gc<terraria::entity::Entity> = if as_player {
            let p = terraria::player::new();
            (*p.ptr()).gfx_off_y = gfx;
            (*p.ptr()).grap_count = grap;
            (*p.ptr()).pulley = pulley;
            (*p.ptr()).on_track = on_track;
            p.up()
        } else {
            terraria::projectile::new().up()
        };
        place(entity, x, y, w, h);
        seed_rand(seed);
        c::step_conveyor_belt(entity, r.f32(), r.bool());
        let e = entity.ptr();
        o.v2((*e).position).i((*e).width).i((*e).height).flags();
    });
    fam!("TryFindingConveyorBeltRising", |seed, r, o| {
        let is_world_item = r.bool();
        let (x, y, w, h) = (r.f32(), r.f32(), r.i32(), r.i32());
        let entity: Gc<terraria::entity::Entity> = if is_world_item { terraria::world_item::new__void().up() } else { terraria::projectile::new().up() };
        place(entity, x, y, w, h);
        seed_rand(seed);
        let v = c::try_finding_conveyor_belt_rising(entity, r.f32());
        o.b(v).flags();
    });
    f
}

/// Re-raises a failure caught with `run` inside a family (after the family restored the world).
fn rethrow(e: String) -> ! {
    std::panic::resume_unwind(Box::new(e))
}

/// `ForgetAllocatedTiles` of Cases.cs: `GetWaterLineIterate` allocates `Main.tile[X, Y]` for slots outside the
/// world; the harness set those slots back to null after the call.
unsafe fn forget_allocated_tiles(x: i32, y: i32) {
    let main = terraria::main::s();
    let tiles = (*main).tile;
    let (l0, l1) = (tiles.dim(0), tiles.dim(1));
    let (w, h) = ((*main).max_tiles_x, (*main).max_tiles_y);
    if x < 0 || x >= l0 {
        return;
    }
    if x < w {
        let mut yy = h;
        while yy < h + 3 && yy < l1 {
            *tiles.at(x, yy) = Gc::NULL;
            yy += 1;
        }
    } else if y + 1 >= 0 && y + 1 < l1 {
        *tiles.at(x, y + 1) = Gc::NULL;
    }
}

fn ulp_distance(a: u32, b: u32) -> u64 {
    fn key(x: u32) -> i64 {
        if x & 0x8000_0000 != 0 {
            -((x & 0x7FFF_FFFF) as i64)
        } else {
            x as i64
        }
    }
    (key(a) - key(b)).unsigned_abs()
}

#[derive(Default)]
struct Stats {
    cases: u64,
    exact: u64,
    game_threw: u64,
    mismatch: u64,
    /// mismatching cases in which every differing word is a float
    float_only: u64,
    max_ulp: u64,
    /// cases that ended in a Rust panic that is not a C# exception (an unported member was reached)
    unported: u64,
    examples: Vec<String>,
}

/// The hand-written port (`terraria-sim`) on the same dump: (family, mismatching cases, of which not float-only).
/// Every other family is bit-exact there. From `X87_FAMILIES` in terraria-sim/tests/collision_reference.rs.
const HAND_PORT_X87: &[(&str, u64, u64)] = &[
    ("AdvancedTileCollision", 5, 0),
    ("AimingLaserScan", 3152, 0),
    ("AnyCollision", 8, 0),
    ("AnyCollisionWithSpecificTiles", 2, 0),
    ("ApplyConveyorBeltMovementToVelocity", 12, 0),
    ("CanHitLine", 16, 16),
    ("CheckAABBvLineCollision6", 2966, 2),
    ("CheckLinevLine", 3581, 0),
    ("SlopeCollision", 3, 0),
    ("StepDown", 20, 0),
    ("StepUp", 2, 0),
    ("TileCollision", 4, 0),
    ("TileCollisionInStepsOf16", 313, 0),
    ("noSlopeCollision", 10, 0),
];

/// The translation with float mode "x87" on the same dump: (family, mismatching cases, of which not float-only).
/// Every other family is bit-exact (57 of 62).
const TRANSLATED_X87: &[(&str, u64, u64)] = &[
    ("AimingLaserScan", 595, 0),
    ("CanHitLine", 16, 16),
    ("CheckAABBvLineCollision6", 1333, 2),
    ("CheckLinevLine", 1019, 2),
    ("TileCollisionInStepsOf16", 300, 0),
];

pub fn run(report: &mut Report) {
    let dir = game::root().join("refdata/collision");
    let (Ok(stamps), Ok(cols)) = (std::fs::read(dir.join("stamps.bin")), std::fs::read(dir.join("world_columns.bin"))) else {
        report.fail(format!("missing stamps.bin / world_columns.bin in {}", dir.display()));
        return;
    };
    let Some((w, h)) = (unsafe { game::load_reference_world(report) }) else { return };
    unsafe {
        game::apply_liquid("ref_small");
        assert_eq!(&stamps[0..4], b"STMP");
        let n = i32::from_le_bytes(stamps[4..8].try_into().unwrap()) as usize;
        assert_eq!(stamps.len(), 8 + n * (8 + RECORD));
        for k in 0..n {
            let o = 8 + k * (8 + RECORD);
            let x = i32::from_le_bytes(stamps[o..o + 4].try_into().unwrap());
            let y = i32::from_le_bytes(stamps[o + 4..o + 8].try_into().unwrap());
            game::write_record(game::tile(x, y).ptr(), &stamps[o + 8..o + 8 + RECORD]);
        }
        let mine = game::column_hashes(w, h);
        let theirs: Vec<u64> = cols.chunks_exact(8).map(|c| u64::from_le_bytes(c.try_into().unwrap())).collect();
        let bad: Vec<usize> = (0..w as usize).filter(|&x| mine[x] != theirs[x]).collect();
        if !bad.is_empty() {
            report.fail(format!("{} tile columns differ from the tiles the game had (first {:?}): the collision cases would not be comparable", bad.len(), &bad[..bad.len().min(8)]));
            return;
        }
        report.line(format!("world {w} x {h} with {n} stamps: all {w} column hashes equal the game's"));
    }

    let only = std::env::var("CS2RS_FAMILY").unwrap_or_default();
    let mut all: BTreeMap<&'static str, Stats> = BTreeMap::new();
    for (name, f) in families() {
        if !only.is_empty() && only != name {
            continue;
        }
        let mut st = Stats::default();
        for (idx, case) in read_cases(name).iter().enumerate() {
            st.cases += 1;
            if case.threw {
                st.game_threw += 1;
            }
            let mut r = R { w: &case.inputs, p: 0 };
            let seed = r.u32();
            unsafe { reset_state(seed) };
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| rt::run(|| f(seed, &mut r))));
            let result: Result<Out, bool> = match result {
                Ok(Ok(o)) => Ok(o),
                Ok(Err(_)) => Err(true),
                Err(p) => {
                    // a C# exception re-raised by a family is a String payload starting with "C# exception"
                    let text = game::panic_text(&p);
                    if text.starts_with("C# exception") {
                        Err(true)
                    } else {
                        if st.examples.len() < 3 {
                            st.examples.push(format!("case {idx}: {text}"));
                        }
                        Err(false)
                    }
                }
            };
            match (case.threw, result) {
                (true, Err(true)) => st.exact += 1,
                (_, Err(false)) => {
                    st.mismatch += 1;
                    st.unported += 1;
                }
                (false, Ok(out)) => {
                    if r.p != case.inputs.len() {
                        panic!("{name}: case {idx} did not consume all its input words");
                    }
                    if out.w == case.outputs {
                        st.exact += 1;
                    } else {
                        st.mismatch += 1;
                        let mut float_only = out.w.len() == case.outputs.len();
                        let mut worst = 0u64;
                        if float_only {
                            for k in 0..out.w.len() {
                                if out.w[k] != case.outputs[k] {
                                    if out.is_f[k] {
                                        worst = worst.max(ulp_distance(out.w[k], case.outputs[k]));
                                    } else {
                                        float_only = false;
                                    }
                                }
                            }
                        }
                        if float_only {
                            st.float_only += 1;
                            st.max_ulp = st.max_ulp.max(worst);
                        }
                        if st.examples.len() < 3 {
                            st.examples.push(format!("case {idx}: inputs {:?}\n          game {:?}\n          here {:?}", case.inputs, case.outputs, out.w));
                        }
                    }
                }
                (true, Ok(out)) => {
                    st.mismatch += 1;
                    if st.examples.len() < 3 {
                        st.examples.push(format!("case {idx}: the game threw, the translation returned {:?}", out.w));
                    }
                }
                (false, Err(true)) => {
                    st.mismatch += 1;
                    if st.examples.len() < 3 {
                        st.examples.push(format!("case {idx}: the translation threw, the game returned {:?}", case.outputs));
                    }
                }
            }
            if idx % 5000 == 4999 {
                // the cases allocate lists, players, arrays: nothing of a finished case is referenced any more
                unsafe { rt::collect() };
            }
        }
        all.insert(name, st);
    }

    report.line(format!(
        "{:<38} {:>6} {:>6} {:>8} {:>6} {:>10} {:>8} {:>9}   {}",
        "family", "cases", "exact", "mismatch", "threw", "float-only", "max ulp", "unported", "hand port (mismatch / not float-only)"
    ));
    let (mut cases, mut exact, mut exact_families) = (0u64, 0u64, 0usize);
    for (name, s) in &all {
        let hand = HAND_PORT_X87.iter().find(|(n, _, _)| n == name).map(|(_, m, nf)| format!("{m} / {nf}")).unwrap_or_else(|| "0 / 0".to_owned());
        report.line(format!("{:<38} {:>6} {:>6} {:>8} {:>6} {:>10} {:>8} {:>9}   {}", name, s.cases, s.exact, s.mismatch, s.game_threw, s.float_only, s.max_ulp, s.unported, hand));
        cases += s.cases;
        exact += s.exact;
        if s.mismatch == 0 {
            exact_families += 1;
        }
    }
    report.line(format!("total: {} families, {cases} cases, {exact} exact; {exact_families} families bit-exact (hand port: 48)", all.len()));
    for (name, s) in &all {
        if s.mismatch != 0 {
            for e in &s.examples {
                report.line(format!("  {name}: {e}"));
            }
        }
    }
    // Pinned to the measured result of the generator's float mode "x87" (tools/cs2rs/core.json): every family not
    // listed is bit-exact; the listed ones may not get worse.
    for (name, s) in &all {
        let (max, max_not_float) = TRANSLATED_X87.iter().find(|(n, _, _)| n == name).map(|(_, m, nf)| (*m, *nf)).unwrap_or((0, 0));
        if s.unported != 0 {
            report.fail(format!("collision {name}: {} cases reached an unported member", s.unported));
        } else if s.mismatch > max || s.mismatch - s.float_only > max_not_float {
            report.fail(format!("collision {name}: {} mismatching cases ({} not float-only); recorded for the translation: {max} ({max_not_float})", s.mismatch, s.mismatch - s.float_only));
        }
    }
}

//! `Terraria.Collision` against the real game: every pure method (and overload) is replayed on the inputs
//! `tools/refdump_collision` fed to the game's own `Collision` (TerrariaServer.exe run headless over `ref_small.wld`)
//! and every output (return value, `ref` / `out` parameters, the eight static flags) is compared bit for bit.
//!
//! The recorded layout of each family (which words are inputs, which are outputs) is in `tools/refdump_collision/Cases.cs`;
//! the closures below read the inputs in the order the harness wrote them and write the outputs in the order it recorded them.
//! Skipped with a message when the world or the dump is missing.

mod collision_common;

use std::collections::BTreeMap;

use collision_common::*;
use terraria_core::xna::{Point, Vector2};
use terraria_sim::collision::*;
use terraria_sim::Game;

/// The mask the harness derives from a seed (`Mask` in Cases.cs).
fn mask(seed: u32, len: usize) -> Vec<bool> {
    (0..len).map(|t| (((t as u32).wrapping_mul(2654435761) ^ seed.wrapping_mul(40503)) >> 11 & 3) == 0).collect()
}

fn families() -> Vec<(&'static str, Box<dyn Fn(&mut Game, u32, &mut R) -> Out>)> {
    let mut f: Vec<(&'static str, Box<dyn Fn(&mut Game, u32, &mut R) -> Out>)> = Vec::new();
    macro_rules! fam {
        ($name:literal, |$g:ident, $seed:ident, $r:ident, $o:ident| $body:block) => {
            f.push((
                $name,
                Box::new(|$g: &mut Game, $seed: u32, $r: &mut R| {
                    let _ = $seed;
                    let mut $o = Out::default();
                    $body
                    $o
                }),
            ));
        };
    }

    // ---- pure geometry
    fam!("CheckLinevLine", |g, _s, r, o| {
        let (a1, a2, b1, b2) = (r.v2(), r.v2(), r.v2(), r.v2());
        let list = check_linev_line(g, a1, a2, b1, b2);
        o.i(list.len() as i32);
        for v in list {
            o.v2(v);
        }
        o.flags(g);
    });
    fam!("CheckAABBvAABBCollision", |g, _s, r, o| {
        let v = check_aab_bv_aabb_collision(r.v2(), r.v2(), r.v2(), r.v2());
        o.b(v).flags(g);
    });
    fam!("CheckAABBvLineCollision4", |g, _s, r, o| {
        let v = check_aab_bv_line_collision(r.v2(), r.v2(), r.v2(), r.v2());
        o.b(v).flags(g);
    });
    fam!("CheckAABBvLineCollision2", |g, _s, r, o| {
        let v = check_aab_bv_line_collision2(r.v2(), r.v2(), r.v2(), r.v2());
        o.b(v).flags(g);
    });
    fam!("CheckAABBvLineCollision6", |g, _s, r, o| {
        let (a, b, c, d) = (r.v2(), r.v2(), r.v2(), r.v2());
        let line_width = r.f32();
        let mut collision_point = r.f32();
        let v = check_aab_bv_line_collision_with_width(a, b, c, d, line_width, &mut collision_point);
        o.b(v).f(collision_point).flags(g);
    });

    // ---- lines of sight
    fam!("CanHit_Vector2", |g, _s, r, o| {
        let v = can_hit_vector2(g, r.v2(), r.i32(), r.i32(), r.v2(), r.i32(), r.i32());
        o.b(v).flags(g);
    });
    fam!("CanHit_Point", |g, _s, r, o| {
        let v = can_hit(g, r.pt(), r.i32(), r.i32(), r.pt(), r.i32(), r.i32());
        o.b(v).flags(g);
    });
    fam!("CanHitLine", |g, _s, r, o| {
        let v = can_hit_line(g, r.v2(), r.i32(), r.i32(), r.v2(), r.i32(), r.i32());
        o.b(v).flags(g);
    });
    fam!("CanHitWithCheck", |g, _s, r, o| {
        let cs = r.u32();
        let (p1, w1, h1, p2, w2, h2) = (r.v2(), r.i32(), r.i32(), r.v2(), r.i32(), r.i32());
        let mut calls = 0u32;
        // CheckImpl of Cases.cs
        let mut check = |x: i32, y: i32| {
            calls += 1;
            ((x.wrapping_mul(73856093) as u32) ^ (y.wrapping_mul(19349663) as u32) ^ cs) & 7 != 0
        };
        let v = can_hit_with_check(g, p1, w1, h1, p2, w2, h2, &mut check);
        o.b(v).i(calls as i32).flags(g);
    });
    fam!("HitLine", |g, _s, r, o| {
        let (x1, y1, x2, y2, ix, iy) = (r.i32(), r.i32(), r.i32(), r.i32(), r.i32(), r.i32());
        let targets = r.pts();
        let (v, col) = hit_line(g, x1, y1, x2, y2, ix, iy, &targets);
        o.b(v).pt(col).flags(g);
    });
    fam!("AnyWallOfTypeOnLine", |g, _s, r, o| {
        let v = any_wall_of_type_on_line(g, r.i32(), r.i32(), r.i32(), r.i32(), r.u32() as u16);
        o.b(v).flags(g);
    });
    fam!("HitLineWall", |g, _s, r, o| {
        let p = hit_line_wall(g, r.i32(), r.i32(), r.i32(), r.i32());
        o.pt(p).flags(g);
    });
    fam!("HitSpecificWallSubstep", |g, _s, r, o| {
        let v = hit_specific_wall_substep(g, r.i32(), r.i32(), r.u32() as u16);
        o.b(v).flags(g);
    });
    fam!("HitWallSubstep", |g, _s, r, o| {
        let v = hit_wall_substep(g, r.i32(), r.i32());
        o.b(v).flags(g);
    });
    fam!("EmptyTile", |g, _s, r, o| {
        let v = empty_tile(g, r.i32(), r.i32(), r.bool());
        o.b(v).flags(g);
    });

    // ---- liquids and points
    fam!("DrownCollision", |g, _s, r, o| {
        let v = drown_collision(g, r.v2(), r.i32(), r.i32(), r.f32(), r.bool());
        o.b(v).flags(g);
    });
    fam!("IsWorldPointSolid", |g, _s, r, o| {
        let v = is_world_point_solid(g, r.v2(), r.bool());
        o.b(v).flags(g);
    });
    fam!("GetWaterLine_Point", |g, _s, r, o| {
        let (v, h) = get_water_line_point(g, r.pt());
        o.b(v).f(h).flags(g);
    });
    fam!("GetWaterLine_Ints", |g, _s, r, o| {
        let (v, h) = get_water_line(g, r.i32(), r.i32());
        o.b(v).f(h).flags(g);
    });
    fam!("GetWaterLineIterate_Point", |g, _s, r, o| {
        let (v, h) = get_water_line_iterate_point(g, r.pt());
        o.b(v).f(h).flags(g);
    });
    fam!("GetWaterLineIterate_Ints", |g, _s, r, o| {
        let (v, h) = get_water_line_iterate(g, r.i32(), r.i32());
        o.b(v).f(h).flags(g);
    });
    fam!("WetCollision", |g, _s, r, o| {
        let v = wet_collision(g, r.v2(), r.i32(), r.i32());
        o.b(v).flags(g);
    });
    fam!("LavaCollision", |g, _s, r, o| {
        let v = lava_collision(g, r.v2(), r.i32(), r.i32());
        o.b(v).flags(g);
    });

    // ---- slopes
    fam!("WalkDownSlope", |g, _s, r, o| {
        let v = walk_down_slope(g, r.v2(), r.v2(), r.i32(), r.i32(), r.f32());
        o.v4(v).flags(g);
    });
    fam!("SlopeCollision", |g, _s, r, o| {
        let v = slope_collision(g, r.v2(), r.v2(), r.i32(), r.i32(), r.f32(), r.bool(), r.bool());
        o.v4(v).flags(g);
    });
    fam!("noSlopeCollision", |g, _s, r, o| {
        let v = no_slope_collision(g, r.v2(), r.v2(), r.i32(), r.i32(), r.bool(), r.bool());
        o.v2(v).flags(g);
    });
    fam!("BuildTileContacts", |g, _s, r, o| {
        let mut list: Vec<TileContact> = Vec::new();
        build_tile_contacts(g, r.v2(), r.i32(), r.i32(), &mut list);
        o.i(list.len() as i32);
        for c in &list {
            o.i(c.side as i32).i(c.overlap).i(c.x).i(c.y).i(c.slope).i(c.type_);
        }
        o.flags(g);
    });
    fam!("TileCollision", |g, _s, r, o| {
        let v = tile_collision(g, r.v2(), r.v2(), r.i32(), r.i32(), r.bool(), r.bool(), r.i32(), r.bool(), r.bool(), r.bool());
        o.v2(v).flags(g);
    });
    fam!("TileCollisionInStepsOf16", |g, _s, r, o| {
        let v = tile_collision_in_steps_of_16(g, r.v2(), r.v2(), r.f32(), r.i32(), r.i32());
        o.f(v).flags(g);
    });
    fam!("TryChangingSizeFromBottomCenter", |g, _s, r, o| {
        let (v, rect) = try_changing_size_from_bottom_center(g, r.rect(), r.i32(), r.i32());
        o.b(v).rect(rect).flags(g);
    });
    fam!("IsClearSpotTest", |g, _s, r, o| {
        let v = is_clear_spot_test(g, r.v2(), r.f32(), r.i32(), r.i32(), r.bool(), r.bool(), r.i32(), r.bool(), r.bool());
        o.b(v).flags(g);
    });
    fam!("FindCollisionTile", |g, _s, r, o| {
        let l = find_collision_tile(g, r.i32(), r.v2(), r.f32(), r.i32(), r.i32(), r.bool(), r.bool(), r.i32(), r.bool(), r.bool());
        o.pts(&l).flags(g);
    });
    fam!("FindCollisionDirection", |g, _s, r, o| {
        let (v, d) = find_collision_direction(g, r.v2(), r.i32(), r.i32(), r.bool(), r.bool(), r.i32());
        o.b(v).i(d).flags(g);
    });
    fam!("SolidCollision3", |g, _s, r, o| {
        let v = solid_collision(g, r.v2(), r.i32(), r.i32());
        o.b(v).flags(g);
    });
    fam!("SolidCollision4", |g, _s, r, o| {
        let v = solid_collision_accept_top_surfaces(g, r.v2(), r.i32(), r.i32(), r.bool());
        o.b(v).flags(g);
    });
    fam!("WaterCollision", |g, _s, r, o| {
        let v = water_collision(g, r.v2(), r.v2(), r.i32(), r.i32(), r.bool(), r.bool(), r.bool());
        o.v2(v).flags(g);
    });
    fam!("AnyCollisionWithSpecificTiles", |g, _s, r, o| {
        let m = mask(r.u32(), 754);
        let v = any_collision_with_specific_tiles(g, r.v2(), r.v2(), r.i32(), r.i32(), &m, r.bool());
        o.v2(v).flags(g);
    });
    fam!("AnyCollision", |g, _s, r, o| {
        let v = any_collision(g, r.v2(), r.v2(), r.i32(), r.i32(), r.bool());
        o.v2(v).flags(g);
    });

    // ---- tile queries
    fam!("AnyHurtingTiles", |g, _s, r, o| {
        g.main.get_good_world = r.bool();
        g.main.dont_starve_world = r.bool();
        let _player = r.i32();
        let v = any_hurting_tiles(g, r.v2(), r.i32(), r.i32());
        o.b(v).flags(g);
    });
    fam!("HurtTiles", |g, _s, r, o| {
        g.main.get_good_world = r.bool();
        g.main.dont_starve_world = r.bool();
        let player = r.i32();
        let player = if player < 0 { None } else { Some(player == 1) };
        let t = hurt_tiles(g, r.v2(), r.i32(), r.i32(), player);
        o.i(t.type_).i(t.x).i(t.y).flags(g);
    });
    fam!("CanTileHurt", |g, _s, r, o| {
        g.main.get_good_world = r.bool();
        g.main.dont_starve_world = r.bool();
        let player = r.i32();
        let player = if player < 0 { None } else { Some(player == 1) };
        let v = can_tile_hurt(g, r.u32() as u16, r.i32(), r.i32(), player);
        o.b(v).flags(g);
    });
    fam!("SwitchTiles", |g, _s, r, o| {
        let _near = (r.i32(), r.i32());
        let v = switch_tiles(g, r.v2(), r.i32(), r.i32(), r.v2(), r.i32());
        o.b(v).flags(g);
    });
    fam!("StickyTiles", |g, _s, r, o| {
        let v = sticky_tiles(g, r.v2(), r.v2(), r.i32(), r.i32());
        o.v2(v).flags(g);
    });
    fam!("SolidTilesVersatile", |g, _s, r, o| {
        let v = solid_tiles_versatile(g, r.i32(), r.i32(), r.i32(), r.i32());
        o.b(v).flags(g);
    });
    fam!("SolidTiles_Vector2", |g, _s, r, o| {
        let v = solid_tiles_at(g, r.v2(), r.i32(), r.i32());
        o.b(v).flags(g);
    });
    fam!("SolidTiles_Ints", |g, _s, r, o| {
        let v = solid_tiles(g, r.i32(), r.i32(), r.i32(), r.i32());
        o.b(v).flags(g);
    });
    fam!("SolidTiles_Vector2_Top", |g, _s, r, o| {
        let v = solid_tiles_at_allow_top_surfaces(g, r.v2(), r.i32(), r.i32(), r.bool());
        o.b(v).flags(g);
    });
    fam!("SolidTiles_Ints_Top", |g, _s, r, o| {
        let v = solid_tiles_allow_top_surfaces(g, r.i32(), r.i32(), r.i32(), r.i32(), r.bool());
        o.b(v).flags(g);
    });
    fam!("SolidFullTiles", |g, _s, r, o| {
        let v = solid_full_tiles(g, r.v2(), r.v2());
        o.b(v).flags(g);
    });

    // ---- stepping
    fam!("StepDown", |g, _s, r, o| {
        let (mut pos, mut vel, w, h, mut step, mut gfx, grav_dir, water_walk) = (r.v2(), r.v2(), r.i32(), r.i32(), r.f32(), r.f32(), r.i32(), r.bool());
        step_down(g, &mut pos, &mut vel, w, h, &mut step, &mut gfx, grav_dir, water_walk);
        o.v2(pos).v2(vel).f(step).f(gfx).flags(g);
    });
    fam!("StepUp", |g, _s, r, o| {
        let (mut pos, mut vel, w, h, mut step, mut gfx, grav_dir, holds, mode) = (r.v2(), r.v2(), r.i32(), r.i32(), r.f32(), r.f32(), r.i32(), r.bool(), r.i32());
        step_up(g, &mut pos, &mut vel, w, h, &mut step, &mut gfx, grav_dir, holds, mode);
        o.v2(pos).v2(vel).f(step).f(gfx).flags(g);
    });
    fam!("InTileBounds", |g, _s, r, o| {
        let v = in_tile_bounds(r.i32(), r.i32(), r.i32(), r.i32(), r.i32(), r.i32());
        o.b(v).flags(g);
    });
    fam!("GetTileRotation", |g, _s, r, o| {
        let v = get_tile_rotation(g, r.v2());
        o.f(v).flags(g);
    });
    fam!("GetEntityEdgeTiles", |g, _s, r, o| {
        let (x, y, w, h) = (r.f32(), r.f32(), r.i32(), r.i32());
        let entity = EntityBox::new(Vector2::new(x, y), w, h);
        let mut p: Vec<Point> = Vec::new();
        get_entity_edge_tiles(&mut p, &entity, r.bool(), r.bool(), r.bool(), r.bool());
        o.pts(&p).flags(g);
    });
    fam!("GetTilesIn", |g, _s, r, o| {
        let l = get_tiles_in(g, r.v2(), r.v2());
        o.pts(&l).flags(g);
    });
    fam!("ExpandVertically", |g, _s, r, o| {
        let (top, bottom) = expand_vertically(g, r.i32(), r.i32(), r.i32(), r.i32());
        o.i(top).i(bottom).flags(g);
    });
    fam!("AdvancedTileCollision", |g, _s, r, o| {
        let m = mask(r.u32(), 754);
        let v = advanced_tile_collision(g, &m, r.v2(), r.v2(), r.i32(), r.i32(), r.bool(), r.bool(), r.i32());
        o.v2(v).flags(g);
    });
    fam!("LaserScan", |g, _s, r, o| {
        let n = r.u32() as usize;
        let mut samples = vec![0f32; n];
        laser_scan(g, r.v2(), r.v2(), r.f32(), r.f32(), &mut samples);
        o.floats(&samples).flags(g);
    });
    fam!("AimingLaserScan", |g, _s, r, o| {
        let (v, samples) = aiming_laser_scan(g, r.v2(), r.v2(), r.f32(), r.i32());
        o.v2(v).floats(&samples).flags(g);
    });

    // ---- conveyor belts
    fam!("ApplyConveyorBeltMovementToVelocity", |g, _s, r, o| {
        let (x, y, w, h) = (r.f32(), r.f32(), r.i32(), r.i32());
        let mut item = EntityBox::new(Vector2::new(x, y), w, h);
        let mut velocity = r.v2();
        let v = apply_conveyor_belt_movement_to_velocity(g, &mut item, &mut velocity);
        o.b(v).v2(velocity).v2(item.position).flags(g);
    });
    fam!("StepConveyorBelt", |g, _s, r, o| {
        let as_player = r.bool();
        let (x, y, w, h) = (r.f32(), r.f32(), r.i32(), r.i32());
        let fields = PlayerConveyorFields { gfx_off_y: r.f32(), grap_count: r.i32(), pulley: r.bool(), on_track: r.bool() };
        let mut entity = EntityBox::new(Vector2::new(x, y), w, h);
        step_conveyor_belt(g, &mut entity, if as_player { Some(&fields) } else { None }, r.f32(), r.bool());
        o.v2(entity.position).i(entity.width).i(entity.height).flags(g);
    });
    fam!("TryFindingConveyorBeltRising", |g, _s, r, o| {
        let is_world_item = r.bool();
        let (x, y, w, h) = (r.f32(), r.f32(), r.i32(), r.i32());
        let entity = EntityBox::new(Vector2::new(x, y), w, h);
        let v = try_finding_conveyor_belt_rising(g, &entity, is_world_item, r.f32());
        o.b(v).flags(g);
    });
    f
}

/// Families whose results can differ from the game's in the last bits because `TerrariaServer.exe` is an x86 program whose JIT
/// computes `float` on the x87 unit without rounding locals and temporaries to 32 bits (see `collision_x87_evidence.rs`;
/// the harness probe is in `refdata/collision/collision.json`), while the port computes in `f32` as the source says.
/// Per family: (name, most mismatching cases of 20,000 seen, most of them where something other than float bits differs:
/// a `bool` / `int` result, a count). The numbers are those of the recorded dump.
const X87_FAMILIES: &[(&str, u64, u64)] = &[
    ("AdvancedTileCollision", 5, 0),
    ("AimingLaserScan", 3152, 0),
    ("AnyCollision", 8, 0),
    ("AnyCollisionWithSpecificTiles", 2, 0),
    ("ApplyConveyorBeltMovementToVelocity", 12, 0),
    ("CanHitLine", 16, 16), // explained exactly by collision_x87_evidence.rs
    ("CheckAABBvLineCollision6", 2966, 2),
    ("CheckLinevLine", 3581, 0),
    ("SlopeCollision", 3, 0),
    ("StepDown", 20, 0),
    ("StepUp", 2, 0),
    ("TileCollision", 4, 0),
    ("TileCollisionInStepsOf16", 313, 0),
    ("noSlopeCollision", 10, 0),
];

/// Families compared without the `Collision` flags word: the game runs unported code in them that sets flags (see `replay_opts`).
const IGNORE_FLAGS: &[&str] = &["SwitchTiles"];

fn run_families(only_x87: bool) -> BTreeMap<String, Stats> {
    let Some(mut g) = reference_game() else { return BTreeMap::new() };
    let mut all: BTreeMap<String, Stats> = BTreeMap::new();
    for (name, run) in families() {
        let is_x87 = X87_FAMILIES.iter().any(|(n, _, _)| *n == name);
        if is_x87 != only_x87 {
            continue;
        }
        let st = replay_opts(&mut g, name, IGNORE_FLAGS.contains(&name), &*run);
        all.insert(name.to_string(), st);
    }
    all
}

/// Strict: every family that is not in `X87_FAMILIES` is bit-identical to the game on all its cases (return values, `ref` / `out`
/// parameters, exceptions and the eight static flags).
#[test]
fn strict_every_other_collision_family_matches_the_real_game_bit_for_bit() {
    let all = run_families(false);
    let total: u64 = all.values().map(|s| s.cases).sum();
    let exact: u64 = all.values().map(|s| s.exact).sum();
    eprintln!("strict families: {} families, {total} cases, {exact} exact", all.len());
    let bad = report(&all);
    assert!(bad.is_empty(), "families that differ from the real game: {bad:?}");
}

/// Tolerant (named so on purpose): the x87 families. Not bit-identical; this test pins how far they are: no more mismatching cases
/// than recorded, no more non-float differences than recorded, and every other mismatch is a float-only difference.
#[test]
fn tolerant_x87_collision_families_stay_within_the_recorded_float_differences() {
    let all = run_families(true);
    let total: u64 = all.values().map(|s| s.cases).sum();
    let exact: u64 = all.values().map(|s| s.exact).sum();
    eprintln!("x87 families: {} families, {total} cases, {exact} exact", all.len());
    report(&all);
    for (name, st) in &all {
        let (_, max_mismatch, max_non_float) = X87_FAMILIES.iter().find(|(n, _, _)| n == name).unwrap();
        assert!(st.mismatch <= *max_mismatch, "{name}: {} mismatching cases, recorded at most {max_mismatch}", st.mismatch);
        let non_float = st.mismatch - st.float_only;
        assert!(non_float <= *max_non_float, "{name}: {non_float} cases differ in more than float bits, recorded at most {max_non_float}");
    }
}
//! Diagnosis of the dual-dungeon pass: `WORLDGEN_DUNGEON=<pass index>` writes, after that pass, every dungeon of
//! `DungeonCrawler.dungeonData` with its rooms, halls, doors, platforms, protected bounds and features, one line each,
//! to `worldgen_<tag>.dungeon.txt` in the current directory. The lines are those of the reference harness
//! (`tools/refdump_worldgen/GenX.cs`, `REFDUMP_DUNGEON=<pass>`, `<tag>.dungeon.txt`), so the two files diff line by
//! line: which room or hall the port was given differently (positions, sizes, seeds, shapes).

use terraria_gen_game::gen::{re_logic, terraria};
use terraria_gen_game::rt::{self, Gc, Nullable};
use std::fmt::Write;

use terraria::game_content::generation::dungeon as dg;

fn b(v: bool) -> &'static str {
    if v { "1" } else { "0" }
}
fn d(v: f64) -> String {
    format!("{:016x}", v.to_bits())
}
fn f(v: f32) -> String {
    format!("{:08x}", v.to_bits())
}
fn v2(v: re_logic::utilities::vector2_d::Vector2D) -> String {
    format!("{},{}", d(v.x), d(v.y))
}
fn pt(p: terraria_core::xna::Point) -> String {
    format!("{},{}", p.x, p.y)
}
fn ni(n: Nullable<i32>) -> String {
    if n.has_value { n.value.to_string() } else { "null".into() }
}
unsafe fn bounds(g: Gc<dg::dungeon_bounds::DungeonBounds>) -> String {
    if g.is_null() {
        return "null".into();
    }
    let x = &*g.ptr();
    let hb = if x._hitbox.has_value {
        let r = x._hitbox.value;
        format!("{},{},{},{}", r.x, r.y, r.width, r.height)
    } else {
        "null".into()
    };
    format!("{},{},{},{},{}", x._bounds_left, x._bounds_top, x._bounds_right, x._bounds_bottom, hb)
}
unsafe fn style(g: Gc<dg::dungeon_generation_style_data::DungeonGenerationStyleData>) -> String {
    if g.is_null() { "null".into() } else { ((*g.ptr()).style as i64).to_string() }
}
/// The C# `GetType().Name` of a translated object: the last part of its full name.
fn name<T>(g: Gc<T>) -> String {
    if g.is_null() {
        return "null".into();
    }
    let full = rt::obj_to_string(g.up()).as_str().to_owned();
    full.rsplit(|c| c == '.' || c == '+').next().unwrap_or("").to_owned()
}
unsafe fn shape(g: Gc<terraria::world_building::gen_shape::GenShape>) -> String {
    let n = name(g);
    let p = g.ptr() as *mut u8;
    match n.as_str() {
        "null" => n,
        "CircleRoom" => {
            let s = &*(p as *const dg::dungeon_shapes::circle_room::CircleRoom);
            format!("CircleRoom({},{})", s._horizontal_radius, s._vertical_radius)
        }
        "QuadCircleRoom" => {
            let s = &*(p as *const dg::dungeon_shapes::quad_circle_room::QuadCircleRoom);
            format!("QuadCircleRoom({},{})", s._radius, s._distance_between_spheres)
        }
        "MoundRoom" => {
            let s = &*(p as *const dg::dungeon_shapes::mound_room::MoundRoom);
            format!("MoundRoom({},{})", s._half_width, s._height)
        }
        "HourglassRoom" => {
            let s = &*(p as *const dg::dungeon_shapes::hourglass_room::HourglassRoom);
            format!("HourglassRoom({},{},{})", s._width, s._height, f(s._percentile_addon))
        }
        _ => n,
    }
}

/// The lines of dungeon `di` of `DungeonCrawler.dungeonData`.
pub unsafe fn dump_one(di: i32) -> String {
    let all = (*dg::dungeon_crawler::s()).dungeon_data;
    let mut w = String::new();
    {
        let dd = &*(*all.ptr()).get_item(di).ptr();
        let rooms = dd.dungeon_rooms;
        let halls = dd.dungeon_halls;
        let feats = dd.dungeon_features;
        let doors = dd.dungeon_door_data;
        let plats = dd.dungeon_platform_data;
        let prot = dd.protected_dungeon_bounds;
        writeln!(
            w,
            "D {di} Type={} Iteration={} makeNextPitTrapFlooded={} useSkewedDungeonEntranceHalls={} createdDungeonEntranceOnSurface={} dungeonEntranceStrengthX={} dungeonEntranceStrengthY={} dungeonEntranceStrengthX2={} dungeonEntranceStrengthY2={} lastDungeonHall={} dungeonBounds={} globalFeatureScalar={} dungeonStepScalar={} hallStrengthScalar={} hallStepScalar={} hallInteriorToExteriorRatio={} hallSlantVariantScalar={} roomStrengthScalar={} roomStepScalar={} roomInteriorToExteriorRatio={} roomSlantVariantScalar={} rooms={} halls={} features={} doors={} platforms={} protected={} entrance={}",
            dd.type_, dd.iteration, b(dd.make_next_pit_trap_flooded), b(dd.use_skewed_dungeon_entrance_halls), b(dd.created_dungeon_entrance_on_surface),
            d(dd.dungeon_entrance_strength_x), d(dd.dungeon_entrance_strength_y), d(dd.dungeon_entrance_strength_x2), d(dd.dungeon_entrance_strength_y2),
            v2(dd.last_dungeon_hall), bounds(dd.dungeon_bounds), d(dd.global_feature_scalar), d(dd.dungeon_step_scalar), d(dd.hall_strength_scalar),
            d(dd.hall_step_scalar), d(dd.hall_interior_to_exterior_ratio), d(dd.hall_slant_variant_scalar), d(dd.room_strength_scalar), d(dd.room_step_scalar),
            d(dd.room_interior_to_exterior_ratio), d(dd.room_slant_variant_scalar),
            (*rooms.ptr()).count(), (*halls.ptr()).count(), (*feats.ptr()).count(), (*doors.ptr()).count(), (*plats.ptr()).count(), (*prot.ptr()).count(),
            name(dd.dungeon_entrance)
        )
        .unwrap();
        for k in 0..(*rooms.ptr()).count() {
            let rg = (*rooms.ptr()).get_item(k);
            let r = &*rg.ptr();
            let st = &*r.settings.ptr();
            let stn = name(r.settings);
            write!(
                w,
                "R {di}.{k} {} {} calculated={} generated={} InnerBounds={} OuterBounds={} RoomPosition={} RoomType={} RandomSeed={} StyleData={} ProgressionStage={} StartingRoom={} OverridePaintTile={} OverridePaintWall={} ForceStyleForDoorsAndPlatforms={} OnCurvedLine={} Orientation={} HallwayPointAdjuster={}",
                name(rg), stn, b(r.calculated), b(r.generated), bounds(r.inner_bounds), bounds(r.outer_bounds), pt(st.room_position), st.room_type,
                st.random_seed, style(st.style_data), st.progression_stage, b(st.starting_room), st.override_paint_tile, st.override_paint_wall,
                b(st.force_style_for_doors_and_platforms), b(st.on_curved_line), st.orientation, ni(st.hallway_point_adjuster)
            )
            .unwrap();
            let sp = r.settings.ptr() as *mut u8;
            match stn.as_str() {
                "GenShapeDungeonRoomSettings" => {
                    let s = &*(sp as *const dg::rooms::gen_shape_dungeon_room_settings::GenShapeDungeonRoomSettings);
                    let room = &*(rg.ptr() as *const dg::rooms::gen_shape_dungeon_room::GenShapeDungeonRoom);
                    write!(w, " ShapeType={} BoundingRadius={} inner={} outer={} _floodedTileCount={}", s.shape_type, s.bounding_radius, shape(s.inner_shape), shape(s.outer_shape), room._flooded_tile_count).unwrap();
                }
                "RegularDungeonRoomSettings" => {
                    let s = &*(sp as *const dg::rooms::regular_dungeon_room_settings::RegularDungeonRoomSettings);
                    write!(w, " OverrideInnerBoundsSize={} OverrideOuterBoundsSize={}", s.override_inner_bounds_size, s.override_outer_bounds_size).unwrap();
                }
                "WormlikeDungeonRoomSettings" => {
                    let s = &*(sp as *const dg::rooms::wormlike_dungeon_room_settings::WormlikeDungeonRoomSettings);
                    write!(w, " FirstSideIterations={} SecondSideIterations={}", s.first_side_iterations, s.second_side_iterations).unwrap();
                }
                "LivingTreeDungeonRoomSettings" => {
                    let s = &*(sp as *const dg::rooms::living_tree_dungeon_room_settings::LivingTreeDungeonRoomSettings);
                    write!(w, " InnerWidth={} InnerHeight={} Depth={} BoundingRadius={}", s.inner_width, s.inner_height, s.depth, s.bounding_radius).unwrap();
                }
                "LegacyDungeonRoomSettings" => {
                    let l = &*(sp as *const dg::rooms::legacy_dungeon_room_settings::LegacyDungeonRoomSettings);
                    let s = &*(sp as *const dg::rooms::step_based_dungeon_room_settings::StepBasedDungeonRoomSettings);
                    write!(w, " OverrideStrength={} OverrideSteps={} OverrideStartPosition={} OverrideEndPosition={} OverrideVelocity={} OverrideInteriorToExteriorRatio={} IsEntranceRoom={}",
                        s.override_strength, s.override_steps, v2(s.override_start_position), v2(s.override_end_position), v2(s.override_velocity), d(s.override_interior_to_exterior_ratio), b(l.is_entrance_room)).unwrap();
                }
                _ => {}
            }
            w.push('\n');
        }
        for k in 0..(*halls.ptr()).count() {
            let hg = (*halls.ptr()).get_item(k);
            let h = &*hg.ptr();
            let st = &*h.settings.ptr();
            let stn = name(h.settings);
            write!(
                w,
                "H {di}.{k} {} {} calculated={} generated={} Bounds={} StartPosition={} EndPosition={} StartDirection={} EndDirection={} CrackedBrick={} HallType={} RandomSeed={} StyleData={} OverridePaintTile={} OverridePaintWall={} CrackedBrickChance={} PlaceOverProtectedBricks={} ZigzagChance={} ForceStyleForDoorsAndPlatforms={} CarveOnly={}",
                name(hg), stn, b(h.calculated), b(h.generated), bounds(h.bounds), v2(h.start_position), v2(h.end_position), v2(h.start_direction), v2(h.end_direction),
                b(h.cracked_brick), st.hall_type, st.random_seed, style(st.style_data), st.override_paint_tile, st.override_paint_wall, d(st.cracked_brick_chance),
                b(st.place_over_protected_bricks), d(st.zigzag_chance), b(st.force_style_for_doors_and_platforms), b(st.carve_only)
            )
            .unwrap();
            let sp = h.settings.ptr() as *mut u8;
            match stn.as_str() {
                "RegularDungeonHallSettings" => {
                    let s = &*(sp as *const dg::halls::regular_dungeon_hall_settings::RegularDungeonHallSettings);
                    write!(w, " OverrideInnerBoundsSize={} OverrideOuterBoundsSize={}", s.override_inner_bounds_size, s.override_outer_bounds_size).unwrap();
                }
                "SineDungeonHallSettings" => {
                    let s = &*(sp as *const dg::halls::sine_dungeon_hall_settings::SineDungeonHallSettings);
                    write!(w, " Magnitude={} Iterations={} FlipSine={}", f(s.magnitude), s.iterations, b(s.flip_sine)).unwrap();
                }
                "StairwellDungeonHallSettings" => {
                    let s = &*(sp as *const dg::halls::stairwell_dungeon_hall_settings::StairwellDungeonHallSettings);
                    write!(w, " MaxDistFromLine={} PointVariance={} InnerBoundsSize={} OuterBoundsSize={} Gradient={} IsEntranceHall={}", s.max_dist_from_line, s.point_variance, s.inner_bounds_size, s.outer_bounds_size, d(s.gradient), b(s.is_entrance_hall)).unwrap();
                }
                "LegacyDungeonHallSettings" | "LegacyEntranceDungeonHallSettings" => {
                    let s = &*(sp as *const dg::halls::step_based_dungeon_hall_settings::StepBasedDungeonHallSettings);
                    write!(w, " OverrideStrength={} OverrideSteps={} ForceHorizontal={} OverrideInteriorToExteriorRatio={}", s.override_strength, s.override_steps, b(s.force_horizontal), d(s.override_interior_to_exterior_ratio)).unwrap();
                    if stn == "LegacyEntranceDungeonHallSettings" {
                        let e = &*(sp as *const dg::halls::legacy_entrance_dungeon_hall_settings::LegacyEntranceDungeonHallSettings);
                        write!(w, " UsePrecalculatedEntrance={}", b(e.use_precalculated_entrance)).unwrap();
                    }
                }
                _ => {}
            }
            w.push('\n');
        }
        for k in 0..(*doors.ptr()).count() {
            let o = (*doors.ptr()).get_item(k);
            writeln!(w, "O {di}.{k} Position={} OverrideStyle={} Direction={} InAHallway={} OverrideWidthFluff={} SkipOtherDoorsCheck={} SkipSpaceCheck={} AlwaysClearArea={}",
                pt(o.position), ni(o.override_style), o.direction, b(o.in_a_hallway), ni(o.override_width_fluff), b(o.skip_other_doors_check), b(o.skip_space_check), b(o.always_clear_area)).unwrap();
        }
        for k in 0..(*plats.ptr()).count() {
            let p = (*plats.ptr()).get_item(k);
            writeln!(w, "P {di}.{k} Position={} OverrideStyle={} OverrideMaxLengthAllowed={} OverrideHeightFluff={} InAHallway={} ForcePlacement={} SkipOtherPlatformsCheck={} SkipSpaceCheck={} PlaceBooksChance={} NoWaterbolt={} PlacePotsChance={} PlaceWaterCandlesChance={} PlacePotionBottlesChance={}",
                pt(p.position), ni(p.override_style), p.override_max_length_allowed, ni(p.override_height_fluff), b(p.in_a_hallway), b(p.force_placement), b(p.skip_other_platforms_check),
                b(p.skip_space_check), d(p.place_books_chance), b(p.no_waterbolt), d(p.place_pots_chance), d(p.place_water_candles_chance), d(p.place_potion_bottles_chance)).unwrap();
        }
        for k in 0..(*prot.ptr()).count() {
            writeln!(w, "B {di}.{k} {}", bounds((*prot.ptr()).get_item(k))).unwrap();
        }
        for k in 0..(*feats.ptr()).count() {
            writeln!(w, "F {di}.{k} {}", name((*feats.ptr()).get_item(k))).unwrap();
        }
    }
    w
}

// The pass clears DungeonCrawler.dungeonData when it ends (WorldGen.cs:14917): as the harness does, the dump is taken
// DURING the pass, when Main.statusText becomes "WorldGeneration.DungeonLate" (DungeonCrawler.MakeDungeon, after the
// layout, doors, platforms and the other global features), for dungeon GenVars.CurrentDungeon. The check runs at
// every yield point (the head of every translated loop body) through the runtime's yield hook.
struct Watch {
    late: String,
    path: String,
    done: Vec<i32>,
    out: String,
}
thread_local! {
    static WATCH: std::cell::RefCell<Option<Watch>> = std::cell::RefCell::new(None);
}
fn hook() {
    WATCH.with(|w| {
        let mut w = w.borrow_mut();
        let Some(w) = w.as_mut() else { return };
        unsafe {
            let st = (*terraria::main::s()).status_text;
            if st.is_null() || st.as_str() != w.late {
                return;
            }
            let d = terraria::world_building::gen_vars::current_dungeon();
            if w.done.contains(&d) {
                return;
            }
            w.done.push(d);
            w.out.push_str(&dump_one(d));
            std::fs::write(&w.path, &w.out).expect("dungeon dump");
            eprintln!("dungeon dump: dungeon {d} -> {}", w.path);
        }
    });
    rt::threads::YIELD_ARMED.store(true, std::sync::atomic::Ordering::Relaxed);
}
/// Arms the watch for the pass about to run.
pub unsafe fn start(path: &str) {
    let late = terraria::localization::language::get_text_value__string(rt::Str::new("WorldGeneration.DungeonLate")).as_str().to_owned();
    WATCH.with(|w| *w.borrow_mut() = Some(Watch { late, path: path.to_owned(), done: Vec::new(), out: String::new() }));
    rt::threads::set_yield_hook(Some(hook));
    rt::threads::YIELD_ARMED.store(true, std::sync::atomic::Ordering::Relaxed);
}
/// Disarms it after the pass.
pub fn stop() {
    rt::threads::YIELD_ARMED.store(false, std::sync::atomic::Ordering::Relaxed);
    rt::threads::set_yield_hook(None);
    WATCH.with(|w| *w.borrow_mut() = None);
}

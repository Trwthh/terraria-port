//! Tests that read the reference world (`work/refsave/Worlds/ref_small.wld`, made by the real game with seed 12345). They skip
//! (pass) when the file is not there. `world_census` is an `#[ignore]`d helper that prints where things are in that world
//! (`cargo test -p terraria-app world_census -- --ignored --nocapture`).

use std::collections::BTreeMap;
use std::path::PathBuf;

use terraria_sim::Game;

fn ref_world() -> Option<Game> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../work/refsave/Worlds/ref_small.wld");
    let bytes = std::fs::read(path).ok()?;
    let world = terraria_formats::wld::read_world(&bytes).ok()?;
    let mut g = Game::new();
    terraria_sim::io::world_file::load_world(&mut g, world);
    Some(g)
}

#[test]
#[ignore]
fn world_census() {
    let Some(g) = ref_world() else { return };
    println!("world {}x{} surface {} rock {} spawn ({}, {})", g.main.max_tiles_x, g.main.max_tiles_y, g.main.world_surface, g.main.rock_layer, g.main.spawn_tile_x, g.main.spawn_tile_y);
    println!("treeX {:?} treeStyle {:?} treeTops {:?} hallowBG {} wind {}", g.main.tree_x, g.main.tree_style, g.world_gen.tree_tops, g.world_gen.hallow_bg, g.main.wind_speed_current);
    let mut counts: BTreeMap<u16, (u32, i32, i32)> = BTreeMap::new();
    for x in 0..g.main.max_tiles_x {
        for y in 0..g.main.max_tiles_y {
            let t = g.main.tile.get(x, y);
            if t.active() {
                let e = counts.entry(t.type_).or_insert((0, x, y));
                e.0 += 1;
            }
        }
    }
    for (ty, (n, x, y)) in &counts {
        println!("type {ty:>4} x{n:>7} first at ({x}, {y})");
    }
    // tree tops: frameX 22/44/66 at frameY >= 198 of tile type 5 and palms (323)
    let mut tops: Vec<(i32, i32, u16, i16, i16)> = Vec::new();
    for x in 0..g.main.max_tiles_x {
        for y in 0..g.main.max_tiles_y {
            let t = g.main.tile.get(x, y);
            if t.active() && ((t.type_ == 5 && t.frame_y >= 198 && t.frame_x == 22) || (t.type_ == 323 && t.frame_x >= 88)) {
                tops.push((x, y, t.type_, t.frame_x, t.frame_y));
            }
        }
    }
    // light sources the flame code draws: torches (4), candles (33), chandeliers (34), lamps (93, 100), lanterns (42), campfires (215)
    for ty in [4u16, 33, 34, 42, 93, 100, 215, 49, 98, 372] {
        let mut at: Vec<(i32, i32)> = Vec::new();
        for x in 0..g.main.max_tiles_x {
            for y in 0..g.main.max_tiles_y {
                let t = g.main.tile.get(x, y);
                if t.active() && t.type_ == ty {
                    at.push((x, y));
                }
            }
        }
        println!("type {ty}: {} tiles, first 30 {:?}", at.len(), &at[..at.len().min(30)]);
    }
    let mut by_kind: BTreeMap<String, Vec<(i32, i32)>> = BTreeMap::new();
    for &(x, y, _, _, _) in &tops {
        let kind = format!("{:?}", terraria_sim::world_gen::tree_foliage::get_tree_type_at(&g, x, y));
        by_kind.entry(kind).or_default().push((x, y));
    }
    for (k, v) in &by_kind {
        println!("tree kind {k}: {} tops, surface ones (y < {}): {:?}", v.len(), g.main.world_surface, v.iter().filter(|p| (p.1 as f64) < g.main.world_surface).take(6).collect::<Vec<_>>());
    }
    println!("{} tree tops, first 40:", tops.len());
    for t in tops.iter().take(40) {
        println!("  {t:?}");
    }
}

use terraria_sim::world_gen::tree_foliage::{get_common_tree_foliage_data, get_tree_type_at, TreeFoliageOut, TreeTypes};

use super::tile_drawing::{get_palm_tree_biome, get_tree_biome};

/// Foliage data of tree tops of the reference world, derived by hand from `WorldGen.GetCommonTreeFoliageData` (WorldGen.cs:63454)
/// and the header values of the world: `treeX = [2858, 4200, 4200]`, `TreeTops = [2, 4, 0, 0, 1, 5, 0, 2, 3, ...]`, `worldSurface = 331`.
#[test]
fn foliage_data_of_reference_world_trees() {
    let Some(g) = ref_world() else { return };
    let mut frame = 0;
    let mut style = -1;
    let mut out = TreeFoliageOut::default();
    // forest top at (602, 78): x <= treeX[0] -> TreeTops[0] = 2 -> style 5 + 2 = 7; an 80 x 80 top (WorldGen.cs:63476-63491)
    assert_eq!(get_tree_type_at(&g, 602, 78), TreeTypes::Forest);
    assert!(get_common_tree_foliage_data(&g, 602, 78, 0, &mut frame, &mut style, &mut out));
    assert_eq!((style, out.top_texture_frame_width, out.top_texture_frame_height), (7, 80, 80));
    // jungle top at (276, 219): ground 60 above the surface line, TreeTops[5] = 5 (not 1) -> style 2, 114 x 96 (WorldGen.cs:63506-63518)
    assert_eq!(get_tree_type_at(&g, 276, 219), TreeTypes::Jungle);
    assert!(get_common_tree_foliage_data(&g, 276, 219, 0, &mut frame, &mut style, &mut out));
    assert_eq!((style, out.top_texture_frame_width, out.top_texture_frame_height), (2, 114, 96));
    // snow top at (2529, 221): ground 147, TreeTops[6] = 0 -> style 12, and 2529 % 10 != 0 keeps it (WorldGen.cs:63519-63527)
    assert_eq!(get_tree_type_at(&g, 2529, 221), TreeTypes::Snow);
    assert!(get_common_tree_foliage_data(&g, 2529, 221, 0, &mut frame, &mut style, &mut out));
    assert_eq!(style, 12);
    // crimson top at (1030, 249): ground 199 -> style 5 (WorldGen.cs:63500-63503)
    assert_eq!(get_tree_type_at(&g, 1030, 249), TreeTypes::Crimson);
    assert!(get_common_tree_foliage_data(&g, 1030, 249, 0, &mut frame, &mut style, &mut out));
    assert_eq!(style, 5);
}

/// `TileDrawing.GetTreeBiome` / `GetPalmTreeBiome` on the reference world's trees
#[test]
fn tree_texture_variants_of_reference_world_trees() {
    let Some(g) = ref_world() else { return };
    // the palm at (2988, 213) stands on sand (53): variant 0, +4 because 380 <= 2988 <= 4200 - 380 (an oasis palm)
    assert_eq!(get_palm_tree_biome(&g, 2988, 213), 4);
    // the snow tree at (2529, 221) grows from snow (147): GetTreeVariant -1 for plain forest ground, 3 for snow;
    // the jungle tree at (276, 219) from jungle grass (60) above the surface line: 1
    let tile = *g.main.tile.get(2529, 221);
    assert_eq!(get_tree_biome(&g, 2529, 221, tile.frame_x as i32, tile.frame_y as i32), 3);
    let tile = *g.main.tile.get(276, 219);
    assert_eq!(get_tree_biome(&g, 276, 219, tile.frame_x as i32, tile.frame_y as i32), 1);
}

/// Header values `load_world` applies (WorldFile.cs:2066-2072, 2162, `TreeTops.Load`)
#[test]
fn load_world_applies_the_tree_and_wind_header_fields() {
    let Some(g) = ref_world() else { return };
    assert_eq!(g.main.tree_x, [2858, 4200, 4200]);
    assert_eq!(g.main.tree_style, [2, 4, 0, 0]);
    assert_eq!(g.world_gen.tree_tops[..9], [2, 4, 0, 0, 1, 5, 0, 2, 3]);
    assert_eq!(g.world_gen.hallow_bg, 2);
    assert!(g.main.wind_speed_current != 0.0); // windSpeedCurrent = windSpeedTarget
}

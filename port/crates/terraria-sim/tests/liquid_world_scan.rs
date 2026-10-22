//! Finds where the reference world keeps its liquids (used to pick screenshot views and the scenarios of
//! `liquid_dynamics.rs`): connected bodies of liquid per liquid type, with bounding box and tile count, largest first.
//! Prints only; asserts that the world has water and lava.
//!
//! Run with `cargo test -p terraria-sim --test liquid_world_scan -- --nocapture`.

use std::path::PathBuf;

use terraria_sim::io::world_file::load_world;
use terraria_sim::Game;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

#[test]
fn list_liquid_bodies_of_the_reference_world() {
    let wld = root().join("work/refsave/Worlds/ref_small.wld");
    let Ok(bytes) = std::fs::read(&wld) else {
        eprintln!("SKIP: need {}", wld.display());
        return;
    };
    let world = terraria_formats::wld::read_world(&bytes).expect("read ref_small.wld");
    let mut g = Game::new();
    g.liquid.cycles = 7; // Main.cs:17289 with gfxQuality 1
    g.liquid.cur_max_liquid = 25000; // Main.cs:17290
    load_world(&mut g, world);
    let (w, h) = (g.main.tile.width, g.main.tile.height);
    eprintln!("world {w} x {h}, worldSurface {}, rockLayer {}, spawn ({}, {})", g.main.world_surface, g.main.rock_layer, g.main.spawn_tile_x, g.main.spawn_tile_y);
    let mut seen = vec![false; (w * h) as usize];
    let mut bodies: Vec<(u8, i32, i32, i32, i32, i32, i32)> = Vec::new(); // type, count, minx, miny, maxx, maxy, maxLiquid
    let mut totals = [0u64; 4];
    for x in 0..w {
        for y in 0..h {
            let t = g.main.tile.get(x, y);
            if t.liquid == 0 {
                continue;
            }
            totals[t.liquid_type() as usize] += 1;
            if seen[(x * h + y) as usize] {
                continue;
            }
            let ty = t.liquid_type();
            let mut stack = vec![(x, y)];
            seen[(x * h + y) as usize] = true;
            let (mut count, mut minx, mut miny, mut maxx, mut maxy) = (0, x, y, x, y);
            while let Some((cx, cy)) = stack.pop() {
                count += 1;
                minx = minx.min(cx);
                maxx = maxx.max(cx);
                miny = miny.min(cy);
                maxy = maxy.max(cy);
                for (nx, ny) in [(cx - 1, cy), (cx + 1, cy), (cx, cy - 1), (cx, cy + 1)] {
                    if nx < 0 || ny < 0 || nx >= w || ny >= h || seen[(nx * h + ny) as usize] {
                        continue;
                    }
                    let n = g.main.tile.get(nx, ny);
                    if n.liquid > 0 && n.liquid_type() == ty {
                        seen[(nx * h + ny) as usize] = true;
                        stack.push((nx, ny));
                    }
                }
            }
            bodies.push((ty, count, minx, miny, maxx, maxy, 0));
        }
    }
    eprintln!("liquid tiles by type (water, lava, honey, shimmer): {totals:?}");
    for (ty, name) in [(0u8, "water"), (1, "lava"), (2, "honey"), (3, "shimmer")] {
        let mut list: Vec<_> = bodies.iter().filter(|b| b.0 == ty).collect();
        list.sort_by_key(|b| -b.1);
        eprintln!("{name}: {} bodies; largest:", list.len());
        for b in list.iter().take(8) {
            eprintln!("  {} tiles, x {}..{}, y {}..{}", b.1, b.2, b.4, b.3, b.5);
        }
    }
    assert!(totals[0] > 0 && totals[1] > 0);
}

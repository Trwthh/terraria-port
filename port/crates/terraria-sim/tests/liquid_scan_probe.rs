//! Probe (ignored by default): prints a rectangle of the reference world after the load settle (`PROBE=x0,x1,y0,y1`, `W`/`w` water,
//! `L` lava, `H` honey, `S` shimmer, `#` solid, `o` other tile) so scenario and screenshot coordinates can be chosen.
//! `cargo test -p terraria-sim --test liquid_scan_probe -- --ignored --nocapture`

use std::path::PathBuf;

use terraria_sim::io::world_file::load_world;
use terraria_sim::Game;

#[test]
#[ignore]
fn probe() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let bytes = std::fs::read(root.join("work/refsave/Worlds/ref_small.wld")).unwrap();
    let world = terraria_formats::wld::read_world(&bytes).unwrap();
    let mut g = Game::new();
    g.liquid.cycles = 7;
    g.liquid.cur_max_liquid = 25000;
    load_world(&mut g, world);
    let regions = std::env::var("PROBE").unwrap_or("2466,2523,232,261".into());
    let v: Vec<i32> = regions.split(',').map(|s| s.parse().unwrap()).collect();
    for y in v[2]..=v[3] {
        let mut line = String::new();
        for x in v[0]..=v[1] {
            let t = g.main.tile.get(x, y);
            let c = if t.liquid > 0 {
                match t.liquid_type() {
                    0 => if t.liquid > 200 { 'W' } else { 'w' },
                    1 => 'L',
                    2 => 'H',
                    _ => 'S',
                }
            } else if t.active() {
                if g.main.tile_solid[t.type_ as usize] { '#' } else { 'o' }
            } else {
                '.'
            };
            line.push(c);
        }
        eprintln!("{y:4} {line}");
    }
}

//! Load -> save round trip of a world written by the real TerrariaServer 1.4.5.8.

use std::path::PathBuf;
use terraria_formats::wld::{read_world, section_at, write_world, write_world_with, WriteOptions, SECTION_NAMES};

fn ref_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../work/refsave/Worlds/ref_small.wld")
}

fn load() -> Option<Vec<u8>> {
    match std::fs::read(ref_path()) {
        Ok(b) => Some(b),
        Err(e) => {
            eprintln!("SKIP: {} not readable ({e}); generate it with the reference server first", ref_path().display());
            None
        }
    }
}

#[test]
fn ref_small_round_trips_byte_for_byte() {
    let Some(bytes) = load() else { return };
    let world = read_world(&bytes).expect("read ref_small.wld");

    // every section ended exactly at the next pointer (read_world returns BadSectionPointer otherwise);
    // the pointers are strictly increasing and the footer follows the last one
    let p = &world.section_pointers;
    assert_eq!(p.len(), 11);
    assert!(p.windows(2).all(|w| w[0] < w[1]), "pointers {p:?}");
    assert!((p[10] as usize) < bytes.len());
    println!("section end offsets:");
    for (i, name) in SECTION_NAMES.iter().enumerate().take(11) {
        println!("  {name:<24} ends at {}", p[i]);
    }
    println!("  {:<24} ends at {} (end of file)", SECTION_NAMES[11], bytes.len());

    let out = write_world(&world).expect("write ref_small.wld");
    if out != bytes {
        let at = out.iter().zip(bytes.iter()).position(|(a, b)| a != b).unwrap_or(out.len().min(bytes.len()));
        panic!(
            "round trip differs: first differing offset {at} (section {}), lengths {} vs {}",
            section_at(p, at),
            out.len(),
            bytes.len()
        );
    }
    assert_eq!(out.len(), bytes.len());
    println!("round trip: {} bytes identical", out.len());
}

#[test]
fn ref_small_known_facts() {
    let Some(bytes) = load() else { return };
    assert_eq!(bytes.len(), 2_987_873);
    let world = read_world(&bytes).expect("read");

    assert_eq!(world.version, 326);
    assert_eq!(world.header.world_name, "ref_small");
    assert_eq!(world.header.seed_text, "12345");
    assert_eq!(world.footer.world_name, "ref_small");
    assert_eq!((world.header.max_tiles_x, world.header.max_tiles_y), (4200, 1200));
    assert_eq!((world.tiles.width, world.tiles.height), (4200, 1200));
    assert_eq!(world.importance.len(), 754);

    let (mut active, mut walls, mut liquid) = (0u64, 0u64, 0u64);
    for t in world.tiles.as_slice() {
        active += t.active() as u64;
        walls += (t.wall != 0) as u64;
        liquid += (t.liquid != 0) as u64;
    }
    println!("size            {} x {}", world.tiles.width, world.tiles.height);
    println!("active tiles    {active}");
    println!("tiles with wall {walls}");
    println!("tiles w/ liquid {liquid}");
    println!("chests          {}", world.chests.len());
    println!("signs           {}", world.signs.len());
    println!(
        "npcs            {} town + {} pillar/other, {} shimmered",
        world.npcs.town_npcs.len(),
        world.npcs.saves_and_loads_npcs.len(),
        world.npcs.shimmered_town_npcs.len()
    );
    println!("tile entities   {}", world.tile_entities.len());
    println!("pressure plates {}", world.weighted_pressure_plates.len());
    println!("town rooms      {}", world.town_rooms.len());
    println!(
        "bestiary        {} kills, {} sights, {} chats",
        world.bestiary.kills.len(),
        world.bestiary.sights.len(),
        world.bestiary.chats.len()
    );
    println!("creative powers {}", world.creative_powers.len());
    println!("revision {} favorite {}", world.metadata.revision, world.metadata.is_favorite);

    assert!(active > 0 && walls > 0 && liquid > 0);
    assert!(!world.chests.is_empty());
    // spawn is on the surface inside the world
    let f = &world.header.flags;
    assert!(f.spawn_tile_x > 0 && f.spawn_tile_x < 4200 && f.spawn_tile_y > 0 && f.spawn_tile_y < 1200);
    assert!(f.world_surface > 0.0 && f.world_surface < f.rock_layer && f.rock_layer < 1200.0);
}

#[test]
fn writing_like_the_game_bumps_revision_and_last_played_only() {
    let Some(bytes) = load() else { return };
    let world = read_world(&bytes).expect("read");
    let opts = WriteOptions { increment_revision: true, last_played: Some(0x1234_5678) };
    let out = write_world_with(&world, &opts).expect("write");
    assert_eq!(out.len(), bytes.len());
    let again = read_world(&out).expect("re-read");
    assert_eq!(again.metadata.revision, world.metadata.revision + 1);
    assert_eq!(again.header.flags.last_played, 0x1234_5678);
    // everything else is unchanged: rewriting without options gives the same bytes back
    assert_eq!(write_world(&again).expect("rewrite"), out);
}

#[test]
fn truncated_or_damaged_files_are_errors() {
    let Some(bytes) = load() else { return };
    assert!(read_world(&bytes[..100]).is_err());
    assert!(read_world(&bytes[..bytes.len() - 1]).is_err());
    let mut bad = bytes.clone();
    bad[26] ^= 1; // first section pointer
    assert!(read_world(&bad).is_err());
    let mut old = bytes;
    old[0..4].copy_from_slice(&200i32.to_le_bytes());
    let err = read_world(&old).expect_err("version 200 is unsupported");
    println!("{err}");
}

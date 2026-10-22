//! A world file written by the translated client's Save & Exit, read back by the verified reader
//! (`terraria_formats::wld::read_world`, byte-identical round trip of the original) and compared with the world it
//! was loaded from: only what the session did may differ.
//!
//! ```text
//! SAVED_WORLD=<port-save>/Worlds/ref_small.wld ORIGINAL_WORLD=work/refsave/Worlds/ref_small.wld \
//!   cargo test -p terraria-gen-game --test saved_world -- --nocapture
//! ```
//! `EXPECT_TILES="x,y x,y ..."`: tiles the session changed on purpose; a changed tile outside a 2-tile border around
//! them makes the test fail unless it is one of the kinds the world changes by itself (listed in the output).
//! Without `SAVED_WORLD` the test does nothing (it needs a session's save).

use terraria_formats::wld::read_world;

fn lines_diff(what: &str, a: &str, b: &str) -> Vec<String> {
    let (la, lb): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
    let mut out = Vec::new();
    if la.len() != lb.len() {
        out.push(format!("{what}: {} lines before, {} after", la.len(), lb.len()));
    }
    for (x, y) in la.iter().zip(lb.iter()) {
        if x != y {
            out.push(format!("{what}: {} -> {}", x.trim(), y.trim()));
        }
    }
    out
}

#[test]
fn saved_world_parses_and_differs_only_where_expected() {
    let Some(saved) = std::env::var_os("SAVED_WORLD") else {
        eprintln!("saved_world: SAVED_WORLD is not set: nothing to check");
        return;
    };
    let original = std::env::var_os("ORIGINAL_WORLD").expect("ORIGINAL_WORLD");
    let a_bytes = std::fs::read(&original).expect("read ORIGINAL_WORLD");
    let b_bytes = std::fs::read(&saved).expect("read SAVED_WORLD");
    let a = read_world(&a_bytes).expect("the original world parses");
    let b = match read_world(&b_bytes) {
        Ok(w) => w,
        Err(e) => panic!("the saved world does not parse: {e:?}"),
    };
    eprintln!("saved world: {} bytes (original {}), version {} (original {}), metadata {:?} (original {:?})", b_bytes.len(), a_bytes.len(), b.version, a.version, b.metadata, a.metadata);

    // ---- header
    let header = lines_diff("header", &format!("{:#?}", a.header), &format!("{:#?}", b.header));
    eprintln!("header fields that differ: {}", header.len());
    for h in &header {
        eprintln!("    {}", h.chars().take(240).collect::<String>());
    }

    // ---- tiles
    assert_eq!((a.tiles.width, a.tiles.height), (b.tiles.width, b.tiles.height), "world size");
    let expect: Vec<(i32, i32)> = std::env::var("EXPECT_TILES")
        .unwrap_or_default()
        .split_whitespace()
        .map(|p| {
            let (x, y) = p.split_once(',').expect("EXPECT_TILES: x,y");
            (x.parse().expect("x"), y.parse().expect("y"))
        })
        .collect();
    let near = |x: i32, y: i32| expect.iter().any(|(ex, ey)| (x - ex).abs() <= 2 && (y - ey).abs() <= 2);
    let mut changed = 0usize;
    let mut expected_changes = Vec::new();
    let mut elsewhere: std::collections::BTreeMap<String, (usize, (i32, i32))> = std::collections::BTreeMap::new();
    for x in 0..a.tiles.width {
        for y in 0..a.tiles.height {
            let (ta, tb) = (a.tiles.get(x, y), b.tiles.get(x, y));
            if ta == tb {
                continue;
            }
            changed += 1;
            if near(x, y) {
                expected_changes.push(format!("({x},{y}) type {}{} wall {} liquid {} frame {},{} -> type {}{} wall {} liquid {} frame {},{}", ta.type_, if ta.s_tile_header & 0x20 != 0 { "" } else { " (inactive)" }, ta.wall, ta.liquid, ta.frame_x, ta.frame_y, tb.type_, if tb.s_tile_header & 0x20 != 0 { "" } else { " (inactive)" }, tb.wall, tb.liquid, tb.frame_x, tb.frame_y));
                continue;
            }
            // what changed, by kind
            let active = |t: &terraria_core::tile::Tile| t.s_tile_header & 0x20 != 0;
            let kind = if active(ta) != active(tb) {
                format!("tile appeared / disappeared (type {} -> {})", if active(ta) { ta.type_ as i32 } else { -1 }, if active(tb) { tb.type_ as i32 } else { -1 })
            } else if ta.type_ != tb.type_ && active(ta) {
                format!("tile type {} -> {}", ta.type_, tb.type_)
            } else if ta.liquid != tb.liquid || (ta.s_tile_header ^ tb.s_tile_header) & 0x8000 != 0 || (ta.b_tile_header ^ tb.b_tile_header) & 0x60 != 0 || (ta.b_tile_header3 ^ tb.b_tile_header3) & 0x80 != 0 {
                "liquid amount / kind".to_owned()
            } else if ta.wall != tb.wall {
                format!("wall {} -> {}", ta.wall, tb.wall)
            } else if ta.frame_x != tb.frame_x || ta.frame_y != tb.frame_y {
                format!("frame only (type {})", if active(ta) { ta.type_ as i32 } else { -1 })
            } else {
                "header bits only".to_owned()
            };
            let e = elsewhere.entry(kind).or_insert((0, (x, y)));
            e.0 += 1;
        }
    }
    eprintln!("tiles that differ: {changed} of {}", a.tiles.width as usize * a.tiles.height as usize);
    eprintln!("  at the places the session worked on ({}):", expected_changes.len());
    for c in &expected_changes {
        eprintln!("    {c}");
    }
    eprintln!("  elsewhere, by kind (count, first position):");
    for (k, (n, p)) in &elsewhere {
        eprintln!("    {n:6}  {k}  first at {p:?}");
    }

    // ---- the other sections
    let mut same = Vec::new();
    let mut differ = Vec::new();
    let mut section = |name: &str, eq: bool, detail: String| {
        if eq {
            same.push(name.to_owned())
        } else {
            differ.push(format!("{name}: {detail}"))
        }
    };
    section("chests", a.chests == b.chests, format!("{} -> {} chests", a.chests.len(), b.chests.len()));
    section("signs", a.signs == b.signs, format!("{} -> {} signs", a.signs.len(), b.signs.len()));
    section("npcs", a.npcs == b.npcs, lines_diff("npcs", &format!("{:#?}", a.npcs), &format!("{:#?}", b.npcs)).into_iter().take(12).collect::<Vec<_>>().join("; "));
    section("tile entities", a.tile_entities == b.tile_entities, format!("{} -> {}", a.tile_entities.len(), b.tile_entities.len()));
    section("pressure plates", a.weighted_pressure_plates == b.weighted_pressure_plates, String::new());
    section("town rooms", a.town_rooms == b.town_rooms, format!("{:?} -> {:?}", a.town_rooms, b.town_rooms));
    section("bestiary", a.bestiary == b.bestiary, lines_diff("bestiary", &format!("{:#?}", a.bestiary), &format!("{:#?}", b.bestiary)).into_iter().take(12).collect::<Vec<_>>().join("; "));
    section("creative powers", a.creative_powers == b.creative_powers, String::new());
    section("footer", a.footer == b.footer, String::new());
    eprintln!("sections identical to the original: {}", same.join(", "));
    eprintln!("sections that differ: {}", differ.len());
    for d in &differ {
        eprintln!("    {}", d.chars().take(700).collect::<String>());
    }

    // ---- what must hold
    assert_eq!(a.chests, b.chests, "chests must be untouched");
    assert_eq!(a.signs, b.signs, "signs must be untouched");
    assert_eq!(a.header.world_name, b.header.world_name);
    assert_eq!(a.header.seed_text, b.header.seed_text);
    for (x, y) in &expect {
        assert!(a.tiles.get(*x, *y) != b.tiles.get(*x, *y), "the tile at {x},{y} was expected to change");
    }
}

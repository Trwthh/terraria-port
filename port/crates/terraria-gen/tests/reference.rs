//! The machine-translated game code against the real game (reference dumps under `refdata/`).
//!
//! One `#[test]` for everything: generated code keeps the game's static state and must stay on one thread, and
//! one test binary links the generated crate once. The sections run in a fixed order on one game state:
//!
//! | section | reference | what is compared |
//! |---|---|---|
//! | `items` | `refdata/items.json` | `new Item(); SetDefaults(id)` for every id: every dumped member |
//! | `npcs` | `refdata/npcs.json` | `new NPC(); SetDefaults(id)` for every id -65..696 |
//! | `projectiles` | `refdata/projectiles.json` | `new Projectile(); SetDefaults(id)` for every id |
//! | `collision` | `refdata/collision/*.bin` | 62 method families x 20,000 calls of `Terraria.Collision` |
//! | `lighting` | `refdata/light/*` | `Lighting.GetColor` and the raw light map of ten areas |
//! | `framing` | `refdata/framed/*` | `WorldGen.SectionTileFrame` over the whole reference world (plain and scrambled) |
//! | `gc` | - | a collection after all of the above frees garbage and the item comparison still passes |
//!
//! `CS2RS_ONLY=collision,lighting` runs a subset (the game initialisation always runs).
//! Run with `-- --nocapture` to see the tables; they are also written to `target*/cs2rs-reference/*.txt`.

mod common;
#[path = "suite/collision.rs"]
mod collision;
#[path = "suite/entities.rs"]
mod entities;
#[path = "suite/framing.rs"]
mod framing;
#[path = "suite/game.rs"]
mod game;
#[path = "suite/lighting.rs"]
mod lighting;

#[test]
fn translated_code_matches_the_real_game() {
    // panics of translated code (unported members, host values) are caught and reported in the tables
    std::panic::set_hook(Box::new(|_| {}));
    let only = std::env::var("CS2RS_ONLY").unwrap_or_default();
    let want = |name: &str| only.is_empty() || only.split(',').any(|s| s.trim() == name);
    let mut report = game::Report::default();

    game::init_game(&mut report);
    type Section = fn(&mut game::Report);
    let sections: [(&str, Section); 7] = [
        ("items", entities::items),
        ("npcs", entities::npcs),
        ("projectiles", entities::projectiles),
        ("collision", collision::run),
        ("lighting", lighting::run),
        ("framing", framing::run),
        ("gc", entities::collect_and_recheck),
    ];
    for (name, f) in sections {
        if !want(name) {
            continue;
        }
        let t0 = std::time::Instant::now();
        report.section(name);
        if let Err(p) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&mut report))) {
            report.fail(format!("section {name} panicked: {}", game::panic_text(&p)));
        }
        report.line(format!("[{name}] {:.1} s", t0.elapsed().as_secs_f64()));
    }
    report.section("what was reached");
    report.line(format!("gaps reached (rt::gap / rt::unported / refused bodies): {}", terraria_gen::rt::gaps().len()));
    for (g, n) in terraria_gen::rt::gaps() {
        report.line(format!("    {g}: {n}"));
    }
    report.line(format!("host members reached: {}", terraria_gen::host::reached().len()));
    for (g, n) in terraria_gen::host::reached() {
        report.line(format!("    {g}: {n}"));
    }
    report.line(format!("stubs reached: {:?}", terraria_gen::rt::stubs_hit().iter().map(|(k, _)| *k).collect::<Vec<_>>()));
    let _ = std::panic::take_hook();
    report.finish();
}

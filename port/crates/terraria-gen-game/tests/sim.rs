//! The simulation comparisons with the real game on the initialisation of Proof 1 (the whole-game crate, translated
//! `Main.Initialize()` as one call): `Collision` (62 method families x 20,000 calls), `Lighting` (ten areas),
//! `WorldGen.SectionTileFrame` (the reference world, plain and scrambled). The sections are the ones of
//! `terraria-gen/tests/reference.rs` (the core set), unchanged except for the set-up.
//!
//! `CS2RS_ONLY=collision,lighting` runs a subset. The tables are written to `<target>/tmp/cs2rs-reference.txt`.

#[path = "p1/json.rs"]
mod common;
#[path = "suite/collision.rs"]
mod collision;
#[path = "suite/framing.rs"]
mod framing;
#[path = "suite/game.rs"]
mod game;
#[path = "suite/lighting.rs"]
mod lighting;

#[test]
fn simulation_sections_match_the_real_game() {
    let run = || {
        std::panic::set_hook(Box::new(|_| {}));
        let only = std::env::var("CS2RS_ONLY").unwrap_or_default();
        let want = |name: &str| only.is_empty() || only.split(',').any(|s| s.trim() == name);
        let mut report = game::Report::default();
        game::init_game(&mut report);
        type Section = fn(&mut game::Report);
        let sections: [(&str, Section); 3] = [("collision", collision::run), ("lighting", lighting::run), ("framing", framing::run)];
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
        let _ = std::panic::take_hook();
        report.finish();
    };
    terraria_host::run_with_stack(256 << 20, "sim", run);
}

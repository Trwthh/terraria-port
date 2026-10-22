//! Shared by the reference sections: the report, running translated code, the game initialisation the reference
//! harnesses performed (`tools/refdump*/`: `new Main()`, `Main.rand = new UnifiedRandom(0)`, `main.Initialize()`
//! with `Main.dedServ = true`), and the reference world.

use crate::common::{self, Json};
use std::any::Any;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use terraria_gen_game::gen::terraria;
use terraria_gen_game::rt::{self, Gc};

pub fn root() -> PathBuf {
    terraria_host::sys::io::project_dir()
}

pub fn refdata(name: &str) -> Json {
    let path = root().join("refdata").join(name);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    common::parse(&text)
}

pub fn panic_text(p: &Box<dyn Any + Send>) -> String {
    p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_else(|| "<panic without a message>".to_owned())
}

/// Runs translated code. `Err` carries a C# exception (`"<Kind>: message"`) or a Rust panic (an unported member,
/// a host value that is not implemented, a bug).
pub fn run<R>(f: impl FnOnce() -> R) -> Result<R, String> {
    match catch_unwind(AssertUnwindSafe(|| rt::run(f))) {
        Ok(Ok(r)) => Ok(r),
        Ok(Err(e)) => Err(format!("C# exception {e}")),
        Err(p) => Err(format!("panic: {}", panic_text(&p))),
    }
}

#[derive(Default)]
pub struct Report {
    text: String,
    failures: Vec<String>,
}

impl Report {
    pub fn section(&mut self, name: &str) {
        self.line(format!("\n==== {name}"));
    }
    pub fn line(&mut self, s: impl AsRef<str>) {
        println!("{}", s.as_ref());
        self.text.push_str(s.as_ref());
        self.text.push('\n');
    }
    pub fn fail(&mut self, s: String) {
        self.line(format!("FAIL: {s}"));
        self.failures.push(s);
    }
    pub fn finish(self) {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("cs2rs-reference.txt");
        let _ = std::fs::write(&path, &self.text);
        println!("\nreport written to {}", path.display());
        assert!(self.failures.is_empty(), "{} failure(s):\n  {}", self.failures.len(), self.failures.join("\n  "));
    }
}

/// The initialisation of Proof 1 (`tests/proof1.rs`, `refdata/MANIFEST.json` "initSequence"): translated code only,
/// `Main.Initialize()` as one call. No hand-sequenced initialisers, no host-supplied names or texts.
pub fn init_game(report: &mut Report) {
    report.section("game initialisation (the sequence of refdata/MANIFEST.json; Main.Initialize() as one call)");
    let mut step = |name: &str, f: &mut dyn FnMut()| match run(f) {
        Ok(()) => report.line(format!("ok    {name}")),
        Err(e) => report.fail(format!("init step `{name}`: {e}")),
    };
    unsafe {
        terraria_gen_game::host::launch::install_resources(&root());
        terraria_host::game::init(terraria_host::game::WindowState { width: 800, height: 600, x: 0, y: 0, active: true, handle: 0, display_width: 1920, display_height: 1080, scale: 1.0 });
        step("Program.SavePath = <scratch>", &mut || (*terraria::program::s()).save_path = rt::Str::new("work/proof1-save"));
        step("Main.dedServ = true; Main.showSplash = false", &mut || {
            (*terraria::main::s()).ded_serv = true;
            (*terraria::main::s()).show_splash = false;
        });
        step("LanguageManager.Instance.SetLanguage(GameCulture.DefaultCulture)", &mut || {
            let lm = (*terraria::localization::language_manager::s()).instance;
            terraria::localization::language_manager::set_language__game_culture(lm.ptr(), terraria::localization::game_culture::default_culture());
        });
        let mut main: Gc<terraria::main::Main> = Gc::NULL;
        step("main = new Main()", &mut || main = terraria::main::new());
        step("Lang.InitializeLegacyLocalization()", &mut || terraria::lang::initialize_legacy_localization());
        step("Main.rand = new UnifiedRandom(0)", &mut || (*terraria::main::s()).rand = rt::alloc_init(terraria_core::UnifiedRandom::new(0)));
        step("main.Initialize()", &mut || terraria::main::initialize(main.ptr()));
    }
}

// ---------------------------------------------------------------------------------------------- reference world

pub const RECORD: usize = 14;

/// The 14-byte tile record of the reference dumps: type u16, wall u16, liquid u8, sTileHeader u16, bTileHeader u8,
/// bTileHeader2 u8, bTileHeader3 u8, frameX i16, frameY i16 (little endian).
pub unsafe fn write_record(t: *mut terraria::tile::Tile, rec: &[u8]) {
    let u16_at = |o: usize| u16::from_le_bytes([rec[o], rec[o + 1]]);
    (*t).type_ = u16_at(0);
    (*t).wall = u16_at(2);
    (*t).liquid = rec[4];
    (*t).s_tile_header = u16_at(5);
    (*t).b_tile_header = rec[7];
    (*t).b_tile_header2 = rec[8];
    (*t).b_tile_header3 = rec[9];
    (*t).frame_x = u16_at(10) as i16;
    (*t).frame_y = u16_at(12) as i16;
}

pub unsafe fn read_record(t: *const terraria::tile::Tile, out: &mut [u8]) {
    out[0..2].copy_from_slice(&(*t).type_.to_le_bytes());
    out[2..4].copy_from_slice(&(*t).wall.to_le_bytes());
    out[4] = (*t).liquid;
    out[5..7].copy_from_slice(&(*t).s_tile_header.to_le_bytes());
    out[7] = (*t).b_tile_header;
    out[8] = (*t).b_tile_header2;
    out[9] = (*t).b_tile_header3;
    out[10..12].copy_from_slice(&(*t).frame_x.to_le_bytes());
    out[12..14].copy_from_slice(&(*t).frame_y.to_le_bytes());
}

/// `Main.tile[x, y]` of the translated game (null outside the loaded world, as in the original).
pub unsafe fn tile(x: i32, y: i32) -> Gc<terraria::tile::Tile> {
    *(*terraria::main::s()).tile.at(x, y)
}

/// FNV-1a 64 over the 14-byte records of each column: the hashes the harnesses wrote (`*_columns.bin`).
pub unsafe fn column_hashes(w: i32, h: i32) -> Vec<u64> {
    let mut rec = [0u8; RECORD];
    (0..w)
        .map(|x| {
            let mut hash: u64 = 14695981039346656037;
            for y in 0..h {
                read_record(tile(x, y).ptr(), &mut rec);
                for &b in &rec {
                    hash ^= b as u64;
                    hash = hash.wrapping_mul(1099511628211);
                }
            }
            hash
        })
        .collect()
}

/// The liquid state `WorldFile.LoadWorld`'s settle pass left in the game (`refdata/framed/<stem>.liquid.bin`, as the
/// hand-written port's tests use it): `liquid`, the liquid type bits of bTileHeader and the checkingLiquid /
/// skipLiquid bits of bTileHeader3 of the listed tiles. Call after [`load_reference_world`] (which clears them).
pub unsafe fn apply_liquid(stem: &str) -> usize {
    let path = root().join("refdata/framed").join(format!("{stem}.liquid.bin"));
    let liquid = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let ht = (*terraria::main::s()).max_tiles_y;
    for rec in liquid.chunks_exact(8) {
        let idx = i32::from_le_bytes(rec[0..4].try_into().unwrap());
        let p = tile(idx / ht, idx % ht).ptr();
        (*p).liquid = rec[4];
        (*p).b_tile_header |= rec[5];
        (*p).b_tile_header3 |= rec[6];
    }
    liquid.len() / 8
}

/// Loads `work/refsave/Worlds/ref_small.wld` into the translated `Main.tile` and the world header fields the
/// translated code reads, with the liquid state cleared (see [`apply_liquid`]).
///
/// This is an adapter, not the translated `WorldFile.LoadWorld`: the file is parsed by `terraria_formats`
/// (verified against the game separately) and the tiles are copied into generated objects.
/// Returns (width, height), or `None` when the files are missing.
pub unsafe fn load_reference_world(report: &mut Report) -> Option<(i32, i32)> {
    let root = root();
    let wld = root.join("work/refsave/Worlds/ref_small.wld");
    let Ok(wld_bytes) = std::fs::read(&wld) else {
        report.fail(format!("missing {}", wld.display()));
        return None;
    };
    let world = terraria_formats::wld::read_world(&wld_bytes).expect("read ref_small.wld");
    let h = &world.header;
    let main = terraria::main::s();
    let (w, ht) = (h.max_tiles_x, h.max_tiles_y);
    // WorldFile.LoadHeader (WorldFile.cs:1967-2096): the fields translated code reads in the sections of this test
    (*main).left_world = h.left_world as f32;
    (*main).right_world = h.right_world as f32;
    (*main).top_world = h.top_world as f32;
    (*main).bottom_world = h.bottom_world as f32;
    (*main).max_tiles_x = w;
    (*main).max_tiles_y = ht;
    (*main).spawn_tile_x = h.flags.spawn_tile_x;
    (*main).spawn_tile_y = h.flags.spawn_tile_y;
    (*main).world_surface = h.flags.world_surface;
    (*main).rock_layer = h.flags.rock_layer;
    (*main).dungeon_x = h.flags.dungeon_x;
    (*main).dungeon_y = h.flags.dungeon_y;
    (*main).hard_mode = h.flags.hard_mode;
    (*main).remix_world = h.flags.remix_world;
    (*main).drunk_world = h.flags.drunk_world;
    (*main).tenth_anniversary_world = h.flags.tenth_anniversary_world;
    // WorldFile.LoadWorldTiles (WorldFile.cs:2534): every tile of the world is an object, the rest of the array stays null
    let tiles = (*main).tile;
    for x in 0..w {
        for y in 0..ht {
            let src = world.tiles.get(x, y);
            let mut t = *tiles.at(x, y);
            if t.is_null() {
                t = terraria::tile::new();
                *tiles.at(x, y) = t;
            }
            let p = t.ptr();
            (*p).type_ = src.type_;
            (*p).wall = src.wall;
            (*p).liquid = 0;
            (*p).s_tile_header = src.s_tile_header;
            (*p).b_tile_header = src.b_tile_header & !0x60;
            (*p).b_tile_header2 = src.b_tile_header2;
            (*p).b_tile_header3 = src.b_tile_header3 & !0x18;
            (*p).frame_x = src.frame_x;
            (*p).frame_y = src.frame_y;
        }
    }
    Some((w, ht))
}

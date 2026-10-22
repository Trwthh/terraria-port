//! Shared by the reference sections: the report, running translated code, the game initialisation the reference
//! harnesses performed (`tools/refdump*/`: `new Main()`, `Main.rand = new UnifiedRandom(0)`, `main.Initialize()`
//! with `Main.dedServ = true`), and the reference world.

use crate::common::{self, Json};
use std::any::Any;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use terraria_gen::gen::terraria;
use terraria_gen::rt::{self, Gc};

pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
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

thread_local! {
    /// `main` of the harness (`new Main()`).
    pub static MAIN: std::cell::Cell<Gc<terraria::main::Main>> = const { std::cell::Cell::new(Gc::NULL) };
}

/// Texts of the game's embedded localisation files (`decompile/Terraria/Terraria.Localization.Content.en-US*.json`):
/// `{ "Category": { "Key": "text" } }` becomes ("Category.Key", text), which is what `LanguageManager` builds.
fn load_texts() -> Vec<(String, String)> {
    let dir = root().join("decompile/Terraria");
    let mut out = Vec::new();
    let mut names: Vec<_> = std::fs::read_dir(&dir).expect("decompile/Terraria").filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    for n in names {
        if !(n.starts_with("Terraria.Localization.Content.en-US") && n.ends_with(".json")) {
            continue;
        }
        let text = std::fs::read_to_string(dir.join(&n)).expect("localisation file");
        let json = common::parse(text.trim_start_matches('\u{feff}'));
        for (cat, entries) in json.obj() {
            if let Json::Obj(m) = entries {
                for (k, v) in m {
                    if let Json::Str(s) = v {
                        out.push((format!("{cat}.{k}"), s.clone()));
                    }
                }
            }
        }
    }
    out
}

/// One line of the initialisation: what the original does there and how the test performs it.
/// `[gen]` = translated code ran; `[set]` = the test wrote the state the original line writes; `[skip]` = not
/// performed (the callee is outside the translated set), with what that leaves undone.
pub fn init_game(report: &mut Report) {
    report.section("game initialisation (tools/refdump: dedServ, new Main(), Main.rand = new UnifiedRandom(0), main.Initialize())");
    let mut step = |name: &str, f: &dyn Fn()| match run(f) {
        Ok(()) => report.line(format!("ok    {name}")),
        Err(e) => report.fail(format!("init step `{name}`: {e}")),
    };
    unsafe {
        // ---- host data (not game code): localisation tables
        step("[host] item / NPC names (refdata/lang_en.json), texts (en-US localisation files)", &|| {
            let lang = refdata("lang_en.json");
            let item_count = 6195usize;
            let mut names: Vec<Option<String>> = vec![None; item_count + 1];
            for (k, v) in lang.get("item").expect("lang item table").obj() {
                let id: usize = k.parse().expect("item id");
                if let Json::Str(s) = v {
                    if id < names.len() {
                        names[id] = Some(s.clone());
                    }
                }
            }
            terraria_gen::host::set_item_names(&names);
            let npc: Vec<(i32, String)> = lang
                .get("npc")
                .expect("lang npc table")
                .obj()
                .iter()
                .filter_map(|(k, v)| match v {
                    Json::Str(s) => Some((k.parse().expect("npc id"), s.clone())),
                    _ => None,
                })
                .collect();
            terraria_gen::host::set_npc_names(&npc);
            terraria_gen::host::install_texts(&load_texts());
        });
        // RefDump.cs:181  Main.dedServ = true   (the static initialiser of Main runs here)
        step("[gen]  Main static initialiser; Main.dedServ = true", &|| (*terraria::main::s()).ded_serv = true);
        // RefDump.cs:191  main = new Main()
        step("[gen]  main = new Main()", &|| MAIN.with(|m| m.set(terraria::main::new())));
        // RefDump.cs:193  Lang.InitializeLegacyLocalization(): Lang is not translated (texts come from the host)
        report_skip(report, "Lang.InitializeLegacyLocalization()", "Lang is outside the translated set; names and texts are host data");
        let mut step = |name: &str, f: &dyn Fn()| match run(f) {
            Ok(()) => report.line(format!("ok    {name}")),
            Err(e) => report.fail(format!("init step `{name}`: {e}")),
        };
        // RefDump.cs:195  Main.rand = new UnifiedRandom(0)
        step("[set]  Main.rand = new UnifiedRandom(0)", &|| (*terraria::main::s()).rand = rt::alloc_init(terraria_core::UnifiedRandom::new(0)));
        if std::env::var("CS2RS_WHOLE_INIT").is_ok() {
            // the original call, whole: reaches members outside the translated set (see the gap list at the end)
            step("[gen]  main.Initialize()   (Main.cs:6635, whole)", &|| terraria::main::initialize__v(MAIN.with(|m| m.get()).ptr()));
            return;
        }
        let main = terraria::main::s();
        // ---- Main.Initialize (Main.cs:6635-6693), dedicated-server path, line by line
        step("[set]  if (dedServ) netMode = 2   (Main.cs:6639)", &|| (*main).net_mode = 2);
        step("[gen]  SetTitle(initialSetup: true)   (Main.cs:6652)", &|| terraria::main::set_title(MAIN.with(|m| m.get()).ptr(), true));
        step("[set]  lo = rand.Next(6)   (Main.cs:6653)", &|| (*main).lo = (*(*main).rand.ptr()).next(6));
        step("[gen]  if (player[myPlayer] == null) player[myPlayer] = new Player()   (Main.cs:6659)", &|| {
            if (*(*main).player.at((*main).my_player)).is_null() {
                *(*main).player.at((*main).my_player) = terraria::player::new();
            }
        });
        // ContentSamples.Initialize() (ContentSamples.cs:840-887) as a whole needs NPCID.Search / ItemID.Search, the
        // name <-> id maps that IdDictionary builds by reflection over the id classes (stubbed null in core.json).
        // Its three SetDefaults loops are performed here with the translated SetDefaults; the name maps are not filled.
        step("[gen]  ContentSamples.Initialize(): NpcsByNetId / ProjectilesByType / ItemsByType loops   (ContentSamples.cs:849-884; name maps skipped)", &|| {
            let cs = terraria::id::content_samples::s();
            for i in -65..(*terraria::id::npcid::s()).count as i32 {
                let npc = terraria::npc::new();
                terraria::npc::set_defaults(npc.ptr(), i, rt::zero());
                (*(*cs).npcs_by_net_id.ptr()).set_item(i, npc);
            }
            // ModifyNPCIds() (ContentSamples.cs:949) rewrites the bestiary credit names: needs the name maps, skipped
            (*(*cs).projectiles_by_type.ptr()).clear();
            for j in 0..(*terraria::id::projectile_id::s()).count as i32 {
                let p = terraria::projectile::new();
                terraria::projectile::set_defaults(p.ptr(), j);
                (*(*cs).projectiles_by_type.ptr()).set_item(j, p);
            }
            (*(*cs).items_by_type.ptr()).clear();
            for k in 0..(*terraria::id::item_id::s()).count as i32 {
                let item = terraria::item::new__void();
                terraria::item::set_defaults(item.ptr(), k, Gc::NULL);
                (*(*cs).items_by_type.ptr()).set_item(k, item);
            }
            let mut after = rt::get_enumerator((*terraria::id::item_id::sets::s()).items_that_are_processed_after_normal_content_sample);
            while rt::Enumerator::move_next(&mut after) {
                let id = rt::Enumerator::current(&after);
                let item = terraria::item::new__void();
                terraria::item::set_defaults(item.ptr(), id, Gc::NULL);
                (*(*cs).items_by_type.ptr()).set_item(id, item);
            }
            terraria::id::content_samples::fill_research_item_overrides();
            terraria::id::content_samples::fill_npc_rarities();
        });
        step("[gen]  player[myPlayer] = new Player()   (Main.cs:6665)", &|| *(*main).player.at((*main).my_player) = terraria::player::new());
        // ---- Initialize_AlmostEverything (Main.cs:6726-6866)
        step("[gen]  Initialize_Entities()   (Main.cs:6734)", &|| terraria::main::initialize_entities(MAIN.with(|m| m.get()).ptr()));
        step("[gen]  FindAnnouncementBoxStatus()   (Main.cs:6735)", &|| terraria::main::find_announcement_box_status());
        step("[gen]  CustomCurrencyManager.Initialize()   (Main.cs:6736)", &|| terraria::game_content::ui::custom_currency_manager::initialize());
        step("[gen]  TileObjectData.Initialize()   (Main.cs:6738)", &|| terraria::object_data::tile_object_data::initialize());
        step("[gen]  Chest.Initialize()   (Main.cs:6740)", &|| terraria::chest::initialize());
        step("[gen]  Wiring.Initialize()   (Main.cs:6741)", &|| terraria::wiring::initialize());
        step("[gen]  Framing.Initialize()   (Main.cs:6742)", &|| terraria::framing::initialize());
        step("[gen]  TileEntity.InitializeAll()   (Main.cs:6744)", &|| terraria::data_structures::tile_entity::initialize_all());
        step("[gen]  TorchID.Initialize()   (Main.cs:6745)", &|| terraria::id::torch_id::initialize());
        step("[gen]  InitializeItemAnimations()   (Main.cs:6748)", &|| terraria::main::initialize_item_animations());
        step("[gen]  Mount.Initialize()   (Main.cs:6777, dedServ)", &|| terraria::mount::initialize());
        step("[gen]  Minecart.Initialize()   (Main.cs:6778, dedServ)", &|| terraria::minecart::initialize());
        step("[gen]  WorldGen.RandomizeBackgrounds(rand) x1..3, RandomizeCaveBackgrounds()   (Main.cs:6780-6789)", &|| {
            terraria::world_gen::randomize_backgrounds((*main).rand);
            if *(*main).tree_bg_set1.at(0) == 173 {
                terraria::world_gen::randomize_backgrounds((*main).rand);
            }
            if *(*main).tree_bg_set1.at(0) == 173 {
                terraria::world_gen::randomize_backgrounds((*main).rand);
            }
            terraria::world_gen::randomize_cave_backgrounds();
        });
        step("[gen]  WorldGen.Hooks.Initialize()   (Main.cs:6790)", &|| terraria::world_gen::hooks::initialize());
        step("[gen]  Initialize_TileAndNPCData1()   (Main.cs:6797)", &|| terraria::main::initialize_tile_and_npc_data1());
        step("[gen]  Initialize_TileAndNPCData2()   (Main.cs:6798)", &|| terraria::main::initialize_tile_and_npc_data2());
        step("[gen]  Initialize_Items()   (Main.cs:6799)", &|| terraria::main::initialize_items());
        step("[gen]  for each projectile: SetDefaults; projHostile / projHook   (Main.cs:6800-6812)", &|| {
            let n = (*terraria::id::projectile_id::s()).count as i32;
            for i in 1..n {
                let p = terraria::projectile::new();
                terraria::projectile::set_defaults(p.ptr(), i);
                if (*p.ptr()).hostile {
                    *(*main).proj_hostile.at(i) = true;
                }
                if (*p.ptr()).ai_style == 7 {
                    *(*main).proj_hook.at(i) = true;
                }
            }
        });
        step("[gen]  Recipe.SetupRecipeGroups()   (Main.cs:6813)", &|| terraria::recipe::setup_recipe_groups());
        step("[gen]  ItemID.Sets.PostSetupContent(); TileID.Sets.PostSetupContent()   (Main.cs:6817-6818)", &|| {
            terraria::id::item_id::sets::post_setup_content();
            terraria::id::tile_id::sets::post_setup_content();
        });
        step("[gen]  DyeInitializer.Load()   (Main.cs:6820)", &|| terraria::initializers::dye_initializer::load());
        step("[gen]  ContentSamples.DyeShaderIDs.Initialize()   (Main.cs:6821)", &|| terraria::id::content_samples::dye_shader_i_ds::initialize());
        step("[gen]  recipe[j] = new Recipe(); Recipe.SetupRecipes()   (Main.cs:6822-6826)", &|| {
            for j in 0..(*terraria::recipe::s()).max_recipes {
                *(*main).recipe.at(j) = terraria::recipe::new();
            }
            terraria::recipe::setup_recipes();
        });
        step("[gen]  ContentSamples.FixItemsAfterRecipesAreAdded()   (Main.cs:6827)", &|| terraria::id::content_samples::fix_items_after_recipes_are_added());
    }
    report_skip(
        report,
        "the other lines of Main.Initialize / Initialize_AlmostEverything",
        "PlayerInput, DontStarveSeed, CreativePowerManager, WingStatsInitializer, Animation, ItemRarity, LeashedEntity, NPCInteractions, the bestiary and item-drop \
         databases, ConditionalDialogue, ArmorSetBonuses, ItemSorting, Netplay, chat and network initialisers: classes outside the translated set; \
         the shops (Chest.SetupShop needs the bestiary and pylon systems)",
    );
}

fn report_skip(report: &mut Report, what: &str, why: &str) {
    report.line(format!("skip  {what}: {why}"));
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

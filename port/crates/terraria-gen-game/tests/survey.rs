//! DIAGNOSTIC ONLY, proves nothing: the statements of `Main.Initialize()` and `Main.Initialize_AlmostEverything()`
//! (Main.cs:6635-6866, dedicated-server branch) run one by one, each caught on its own, so that ONE build lists the
//! first stop of every sub-initialiser instead of the first stop of the whole call. State after a failed step is
//! incomplete, so later failures may be consequences; the proof is `tests/proof1.rs`, which calls
//! `Main.Initialize()` once. Run: `sh tools/cs2rs/p1.sh --survey`.

use terraria_gen_game as game;
use terraria_gen_game::gen::terraria;
use terraria_gen_game::rt::{self, Delegate, Dictionary, Str};

fn panic_text(p: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = p.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = p.downcast_ref::<&str>() {
        (*s).to_owned()
    } else {
        "<non-string panic payload>".to_owned()
    }
}

static mut FAILED: u32 = 0;

fn step(name: &str, f: impl FnOnce()) {
    let t0 = std::time::Instant::now();
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| rt::run(f))) {
        Ok(Ok(())) => eprintln!("ok   {name}   ({:.2} s)", t0.elapsed().as_secs_f64()),
        Ok(Err(e)) => {
            unsafe { FAILED += 1 };
            eprintln!("FAIL {name}: C# exception: {e}")
        }
        Err(p) => {
            unsafe { FAILED += 1 };
            eprintln!("FAIL {name}: stopped: {}", panic_text(&p))
        }
    }
}

macro_rules! st {
    ($($e:tt)*) => { step(stringify!($($e)*), || { $($e)*; }) };
}

#[test]
fn survey_of_main_initialize() {
    let run = || unsafe {
        std::panic::set_hook(Box::new(|_| {}));
        let root = terraria_host::sys::io::project_dir();
        terraria_gen_game::host::launch::install_resources(&root);
        terraria_host::game::init(terraria_host::game::WindowState { width: 800, height: 600, x: 0, y: 0, active: true, handle: 0, display_width: 1920, display_height: 1080, scale: 1.0 });
        step("Program.SavePath", || (*terraria::program::s()).save_path = Str::new("work/proof1-save"));
        step("Main.dedServ", || {
            (*terraria::main::s()).ded_serv = true;
            (*terraria::main::s()).show_splash = false;
        });
        step("SetLanguage", || {
            let lm = (*terraria::localization::language_manager::s()).instance;
            terraria::localization::language_manager::set_language__game_culture(lm.ptr(), terraria::localization::game_culture::default_culture());
        });
        let mut main = rt::Gc::NULL;
        step("new Main()", || main = terraria::main::new());
        step("Lang.InitializeLegacyLocalization()", || terraria::lang::initialize_legacy_localization());
        step("Main.rand", || (*terraria::main::s()).rand = rt::alloc_init(terraria_core::UnifiedRandom::new(0)));
        let this = main.ptr();
        let ms = terraria::main::s;

        // ---- Main.Initialize (Main.cs:6635)
        (*ms()).net_mode = 2;
        st!((*ms())._rngs = Dictionary::<Str, rt::Gc<terraria_core::UnifiedRandom>>::new_gc());
        st!(terraria::main::set_title(this, true));
        st!((*ms()).lo = (*(*ms()).rand.ptr()).next(6));
        st!((*this).waterfall_manager = terraria::waterfall_manager::new());
        st!((*ms())._window_mover = terraria::graphics::window_state_controller::new());
        st!((*ms()).sitting_manager = terraria::data_structures::anchored_entities_collection::new());
        st!((*ms()).sleeping_manager = terraria::data_structures::anchored_entities_collection::new());
        st!((*ms()).game_tips = terraria::game_content::ui::game_tips_display::new(terraria::game_content::ui::game_tips_provider::new().up::<rt::Object>()));
        st!(*(*ms()).player.at((*ms()).my_player) = terraria::player::new());
        st!(terraria::id::content_samples::initialize());
        st!(terraria::game_input::player_input::initialize());
        st!(*(*ms()).player.at((*ms()).my_player) = terraria::player::new());
        st!(terraria::game_content::dont_starve_seed::initialize());
        st!((*ms()).resource_sets_manager = terraria::game_content::ui::resource_sets::player_resource_sets_manager::new());
        st!((*ms()).minimap_frame_manager_instance = terraria::game_content::ui::minimap::minimap_frame_manager::new());
        st!(terraria::main::bind_settings_to(this, (*ms()).configuration));

        // ---- Main.Initialize_AlmostEverything (Main.cs:6726)
        st!((*this).tile_paint_system = terraria::game_content::tile_paint_system_v2::new());
        st!((*this).tiles_renderer = terraria::game_content::drawing::tile_drawing::new((*this).tile_paint_system));
        st!((*this).walls_renderer = terraria::game_content::drawing::wall_drawing::new((*this).tile_paint_system));
        st!(terraria::game_content::creative::creative_power_manager::initialize());
        st!(terraria::io::favorites_file::load((*ms()).local_favorite_data.ptr()));
        st!(terraria::io::favorites_file::load((*ms()).cloud_favorites_data.ptr()));
        st!(terraria::main::initialize_entities(this));
        st!(terraria::main::find_announcement_box_status());
        st!(terraria::game_content::ui::custom_currency_manager::initialize());
        st!(terraria::initializers::wing_stats_initializer::load());
        st!(terraria::object_data::tile_object_data::initialize());
        st!(terraria::animation::initialize());
        st!(terraria::chest::initialize());
        st!(terraria::wiring::initialize());
        st!(terraria::framing::initialize());
        st!(terraria::game_content::ui::item_rarity::initialize());
        st!(terraria::data_structures::tile_entity::initialize_all());
        st!(terraria::id::torch_id::initialize());
        st!(terraria::game_content::leashed_entity::registry::register_all());
        st!(terraria::game_content::npc_interactions::initialize());
        st!(terraria::main::initialize_item_animations());
        let mut db = rt::Gc::NULL;
        let mut pop = rt::Gc::NULL;
        st!(db = terraria::game_content::bestiary::bestiary_database::new());
        st!(pop = terraria::game_content::bestiary::bestiary_database_np_cs_populator::new());
        st!(terraria::game_content::bestiary::bestiary_database_np_cs_populator::populate(pop.ptr(), db));
        (*ms()).bestiary_db = db;
        st!(terraria::id::content_samples::rebuild_bestiary_sorting_i_ds_by_bestiary_database_contents(db));
        st!((*ms()).bestiary_tracker = terraria::game_content::bestiary::bestiary_unlocks_tracker::new());
        let mut drops = rt::Gc::NULL;
        st!(drops = terraria::game_content::item_drop_rules::item_drop_database::new());
        st!(terraria::game_content::item_drop_rules::item_drop_database::populate(drops.ptr()));
        (*ms()).item_drops_db = drops;
        st!(terraria::game_content::bestiary::bestiary_database::merge(db.ptr(), (*ms()).item_drops_db));
        st!(terraria::game_content::bestiary::bestiary_database_np_cs_populator::add_drop_overrides(pop.ptr(), db));
        let mut fish = rt::Gc::NULL;
        st!(fish = terraria::game_content::fish_drop_rules::fish_drop_rule_list::new());
        st!(terraria::game_content::fish_drop_rules::game_content_fish_drop_populator::populate(terraria::game_content::fish_drop_rules::game_content_fish_drop_populator::new(fish).ptr()));
        (*ms()).fish_drops_db = fish;
        st!((*ms()).pylon_system = terraria::game_content::teleport_pylons_system::new());
        st!((*ms()).item_drop_solver = terraria::game_content::item_drop_rules::item_drop_resolver::new(drops));
        st!((*ms()).shop_helper = terraria::game_content::shop_helper::new());
        st!(terraria::game_content::creative::creative_item_sacrifices_catalog::initialize((*terraria::game_content::creative::creative_item_sacrifices_catalog::s()).instance.ptr()));
        st!(terraria::mount::initialize());
        st!(terraria::minecart::initialize());
        st!(terraria::world_gen::randomize_backgrounds((*ms()).rand));
        st!(terraria::world_gen::randomize_cave_backgrounds());
        st!(terraria::world_gen::hooks::initialize());
        st!({
            let p = core::ptr::addr_of_mut!((*terraria::world_gen::hooks::s()).on_world_load__event);
            *p = Delegate::combine(*p, Delegate::<dyn Fn()>::new(Box::new(move || terraria::main::reset_game_counter())))
        });
        st!(terraria::main::initialize_tile_and_npc_data1());
        st!(terraria::main::initialize_tile_and_npc_data2());
        st!(terraria::main::initialize_items());
        st!(for i in 1..(*terraria::id::projectile_id::s()).count as i32 {
            let obj = terraria::projectile::new();
            terraria::projectile::set_defaults(obj.ptr(), i);
            if (*obj.ptr()).hostile {
                *(*ms()).proj_hostile.at(i) = true;
            }
            if (*obj.ptr()).ai_style == 7 {
                *(*ms()).proj_hook.at(i) = true;
            }
        });
        st!(terraria::recipe::setup_recipe_groups());
        st!(terraria::game_content::conditional_dialogue::init());
        st!(terraria::data_structures::armor_set_bonuses::initialize());
        st!(terraria::data_structures::armor_set_bonuses::build_lookup());
        st!(terraria::id::item_id::sets::post_setup_content());
        st!(terraria::id::tile_id::sets::post_setup_content());
        st!(terraria::game_content::conditional_dialogue::item_groups::post_setup_content());
        st!(terraria::initializers::dye_initializer::load());
        st!(terraria::id::content_samples::dye_shader_i_ds::initialize());
        st!(for j in 0..(*terraria::recipe::s()).max_recipes {
            *(*ms()).recipe.at(j) = terraria::recipe::new();
        });
        st!(terraria::recipe::setup_recipes());
        st!(terraria::id::content_samples::fix_items_after_recipes_are_added());
        st!(terraria::ui::item_sorting::setup_white_lists());
        st!(terraria::id::content_samples::rebuild_item_creative_sorting_i_ds_after_recipes_are_set_up());
        st!(*(*this).shop.at(0) = terraria::chest::create_shop());
        st!(terraria::chest::setup_travel_shop());
        st!(for m in 1..100 {
            *(*this).shop.at(m) = terraria::chest::create_shop();
            terraria::chest::setup_shop((*(*this).shop.at(m)).ptr(), m);
        });
        st!(terraria::netplay::initialize());
        st!(terraria::initializers::network_initializer::load());
        st!(terraria::initializers::chat_initializer::load());
        st!(terraria::game_content::lucy_axe_message::initialize());

        eprintln!("survey: {} step(s) failed", FAILED);
        eprintln!("gaps reached: {}", rt::gaps().len());
        for (g, n) in rt::gaps() {
            eprintln!("    {g}: {n}");
        }
        eprintln!("host members reached: {}", game::host::reached().len());
        for (g, n) in game::host::reached() {
            eprintln!("    {g}: {n}");
        }
    };
    terraria_host::run_with_stack(256 << 20, "survey", run);
}

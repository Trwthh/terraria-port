// The probe run, part S: tools/refdump_draw/ProbeS.cs, step for step. Included by tests/draw.rs.

/// ProbeS.cs `ProbeS`.
unsafe fn probe_s(r: &mut Run, sx: i32, sy: i32, _x0: i32, _gy: i32) {
    use terraria::ui::ingame_fancy_ui as IFU;
    let m = terraria::main::s();
    let pl = local_player();
    let p_ = pl.ptr();
    if probe_group(r, "achieve2") {
        clear_entities();
        place_player(sx, sy);
        r.frames(2);
        IFU::open_achievements();
        r.frames(3);
        let menu = (*m).achievements_menu;
        let ach = terraria::achievements::achievement_manager::get_achievement(terraria::main::achievements().ptr(), s!("BULLDOZER"));
        terraria::game_content::ui::states::ui_achievements_menu::goto_achievement(menu.ptr(), ach);
        r.frames(2);
        r.record("achievements_bulldozer", 1);
        IFU::close(false);
        r.frames(2);
        r.state.push_str(&state_line("achieve2"));
    }
    if probe_group(r, "keybinds2") {
        use terraria::game_content::ui::states::ui_manage_controls as UMC;
        clear_entities();
        place_player(sx, sy);
        r.frames(2);
        IFU::open_keybinds();
        r.frames(3);
        let mc = (*m).manage_controls_menu;
        let mce: Gc<terraria::ui::ui_element::UIElement> = mc.up();
        UMC::gamepad_button_click(mc.ptr(), mouse_evt(mce), mce);
        UMC::vs_menu_button_click(mc.ptr(), mouse_evt(mce), mce);
        r.frames(2);
        let list = (*mc.ptr())._uilist;
        for k in 0..6 {
            terraria::game_content::ui::elements::ui_list::scroll_wheel(list.ptr(), terraria::ui::ui_scroll_wheel_event::new(list.up(), v2(640.0, 360.0), -360));
            r.frames(2);
            r.record(&format!("keybinds_scroll_{k}"), 1);
        }
        UMC::keyboard_button_click(mc.ptr(), mouse_evt(mce), mce);
        UMC::vs_gameplay_button_click(mc.ptr(), mouse_evt(mce), mce);
        IFU::close(false);
        r.frames(2);
        r.state.push_str(&state_line("keybinds2"));
    }
    if probe_group(r, "crafting2") {
        clear_entities();
        place_player(sx, sy);
        r.frames(2);
        set_inv(10, 9, 99);
        set_inv(11, 3, 99);
        set_inv(12, 23, 99);
        (*m).player_inventory = true;
        terraria::recipe::update_recipe_list();
        r.frames(2);
        r.probe("recipes", &[&(*m).num_available_recipes]);
        terraria::game_content::ui::new_crafting_ui::open(true, Gc::NULL);
        r.frames(4);
        r.record("newcrafting2", 1);
        terraria::game_content::ui::new_crafting_ui::close(true, false);
        (*m).player_inventory = false;
        for k in 10..=12 {
            give_item(k, 0);
        }
        r.frames(2);
        r.state.push_str(&state_line("crafting2"));
    }
    if probe_group(r, "currency2") {
        clear_entities();
        place_player(sx, sy);
        r.frames(2);
        set_inv(20, 3817, 37);
        let bk = *(*(*p_).bank.ptr()).item.at(0);
        terraria::item::set_defaults(bk.ptr(), 3817, Gc::NULL);
        (*bk.ptr()).stack = 37;
        let sb = (*m).sprite_batch;
        r.direct("currency_savings", |_| {
            in_batch(|| terraria::game_content::ui::custom_currency_manager::draw_savings(sb, (*terraria::id::custom_currency_id::s()).defender_medals, 500.0, 300.0, false))
        });
        r.direct("money_savings", |_| {
            in_batch(|| terraria::ui::item_slot::draw_money(sb, s!("Savings"), 500.0, 300.0, Arr::<i32>::from_slice(&[12, 34, 5, 1]), false, true))
        });
        give_item(20, 0);
        terraria::item::turn_to_air(bk.ptr());
        r.state.push_str(&state_line("currency2"));
    }
    if probe_group(r, "sunrise") {
        clear_entities();
        clear_area(_x0, _x0 + 40, _gy);
        for x in _x0 + 8..=_x0 + 12 {
            (*gallery_tile(x, _gy).ptr()).type_ = 2;
        }
        let grown = terraria::world_gen::grow_tree(_x0 + 10, _gy, 0, true);
        r.probe("sunrise_tree", &[&grown]);
        place_player(_x0 + 4, _gy - 1);
        let (day, time) = ((*m).day_time, (*m).time);
        for t in [400.0f64, 1500.0, 6000.0, 48000.0, 52500.0, 53600.0] {
            set_time(true, t);
            r.frames(20);
            r.record(&format!("sun_{}", t as i32), 1);
        }
        set_time(day, time);
        clear_area(_x0, _x0 + 40, _gy);
        clear_entities();
        clear_fx();
        clear_items();
        r.frames(5);
        r.state.push_str(&state_line("sunrise"));
    }
    if probe_group(r, "respacks") {
        clear_entities();
        place_player(sx, sy);
        r.frames(2);
        let menu = (*m).menu_mode;
        terraria::main::open_resource_packs_menu(Gc::NULL);
        let ui = (*m).menu_ui.ptr();
        let state = terraria::ui::user_interface::current_state(ui);
        terraria::ui::user_interface::set_state(ui, Gc::NULL);
        (*m).menu_mode = menu;
        let _root = rt::Rooted::new(state);
        IFU::open_ui_state__ui_state(state);
        r.frames(3);
        r.record("respacks", 1);
        let sel: Gc<terraria::game_content::ui::states::ui_resource_pack_selection_menu::UIResourcePackSelectionMenu> = state.up();
        let packs = (*sel.ptr())._packs_list;
        let pack = terraria::io::resource_pack_list::all_packs(packs.ptr()).to_vec().first().copied().unwrap_or(Gc::NULL);
        if pack.is_null() {
            r.probe("respacks", &[&"-"]);
        } else {
            r.probe("respacks", &[&terraria::io::resource_pack::name(pack.ptr())]);
            use terraria::game_content::ui::states::ui_resource_pack_selection_menu as RSM;
            terraria::io::resource_pack::set_is_enabled(pack.ptr(), true);
            RSM::populate_pack_list(sel.ptr());
            r.frames(3);
            r.record("respacks_enabled", 1);
            terraria::io::resource_pack::set_is_enabled(pack.ptr(), false);
            RSM::populate_pack_list(sel.ptr());
            let info = terraria::game_content::ui::states::ui_resource_pack_info_menu::new(sel, pack);
            let _root2 = rt::Rooted::new(info);
            IFU::open_ui_state__ui_state(info.up());
            r.frames(3);
            r.record("respack_info", 1);
        }
        IFU::close(false);
        r.frames(2);
        r.state.push_str(&state_line("respacks"));
    }
    if probe_group(r, "hairstyles") {
        use terraria::game_content::ui::states::ui_character_creation as ucc;
        clear_entities();
        place_player(sx, sy);
        r.frames(2);
        (*m).hair_window = true;
        terraria::game_content::hairstyle_unlocks_helper::update_unlocks((*m).hairstyles.ptr());
        let np = terraria::player::new();
        let cc = ucc::new(np);
        let _roots = (rt::Rooted::new(np), rt::Rooted::new(cc));
        fancy(cc.up());
        r.frames(3);
        ucc::click_hair_styles(cc.ptr(), Gc::NULL, Gc::NULL);
        r.frames(3);
        r.record("hairstyles", 1);
        fancy_close();
        (*m).hair_window = false;
        terraria::game_content::hairstyle_unlocks_helper::update_unlocks((*m).hairstyles.ptr());
        r.frames(2);
        r.state.push_str(&state_line("hairstyles"));
    }
    if probe_group(r, "dungeonchest") {
        use terraria::game_content::generation::dungeon as D;
        clear_entities();
        place_player(_x0 + 2, _gy - 1);
        r.frames(2);
        let rs = D::rooms::legacy_dungeon_room_settings::new();
        let rsb: Gc<D::rooms::dungeon_room_settings::DungeonRoomSettings> = rs.up();
        (*rsb.ptr()).style_data = (*D::dungeon_generation_styles::s()).snow;
        let room = D::rooms::legacy_dungeon_room::new(rsb);
        let (cx, cy) = (_x0 + 10, _gy + 20);
        (*room.ptr()).end_position = re_logic::utilities::vector2_d::new__double_double(cx as f64, cy as f64);
        (*room.ptr()).strength = 2;
        let wt = gallery_tile(cx - 1, cy - 1);
        let wall = (*wt.ptr()).wall;
        (*wt.ptr()).wall = 350;
        let data = D::dungeon_data::new();
        let datas = (*D::dungeon_crawler::s()).dungeon_data;
        let cur = terraria::world_building::gen_vars::current_dungeon();
        let before = (*datas.ptr()).count();
        terraria::world_building::gen_vars::set_current_dungeon(before);
        (*datas.ptr()).add(data);
        let feat = D::features::dungeon_global_biome_chests::new(D::features::dungeon_feature_settings::new());
        let _roots = (rt::Rooted::new(room), rt::Rooted::new(feat));
        let ok = D::rooms::legacy_dungeon_room::dual_dungeons_try_generate_biome_chest_in_room(room.ptr(), data, feat);
        r.probe("dungeonchest", &[&ok]);
        (*wt.ptr()).wall = wall;
        (*datas.ptr()).remove_at(before);
        terraria::world_building::gen_vars::set_current_dungeon(cur);
        r.frames(2);
        r.state.push_str(&state_line("dungeonchest"));
    }
    if probe_group(r, "creative2") {
        use terraria::game_content::creative::creative_ui as CU;
        clear_entities();
        place_player(sx, sy);
        r.frames(2);
        let diff = (*p_).difficulty;
        (*p_).difficulty = 3;
        (*m).player_inventory = true;
        let cm = (*m).creative_menu.ptr();
        if !CU::enabled(cm) {
            CU::toggle_menu(cm);
        }
        r.frames(3);
        let st = (*cm)._ui_state.ptr();
        if (*(*st)._main_category.ptr()).current_option != 2 {
            terraria::game_content::ui::states::ui_creative_powers_menu::toggle_main_category(st, 2);
        }
        terraria::game_content::ui::states::ui_creative_powers_menu::refresh_elements_order(st);
        r.frames(3);
        let it = cover_new_item(9);
        (*it.ptr()).stack = 999;
        CU::set_itemby_index(cm, it, 0);
        let (mut a, mut b, mut c) = (0i32, 0i32, 0i32);
        let ok = CU::get_sacrifice_numbers(cm, &mut a, &mut b, &mut c);
        let showing = CU::is_showing_research_menu(cm);
        r.probe("research_numbers", &[&ok, &a, &b, &c, &showing]);
        CU::sacrifice_item_in_sacrifice_slot(cm);
        for k in 0..4 {
            r.frames(15);
            r.record(&format!("creative_research2_{k}"), 1);
        }
        if CU::enabled(cm) {
            CU::toggle_menu(cm);
        }
        (*m).player_inventory = false;
        (*p_).difficulty = diff;
        clear_items();
        r.frames(2);
        r.state.push_str(&state_line("creative2"));
    }
    if probe_group(r, "seaweed") {
        clear_entities();
        place_player(_x0 + 24, _gy - 1);
        r.frames(2);
        let (wx, gy) = (_x0 + 30, _gy);
        let mut walls = [[0u16; 7]; 4];
        for x in wx..wx + 4 {
            for y in gy..=gy + 6 {
                kill_tile(x, y);
            }
            for y in gy..=gy + 5 {
                let t = gallery_tile(x, y);
                walls[(x - wx) as usize][(y - gy) as usize] = (*t.ptr()).wall;
                (*t.ptr()).wall = 0;
                (*t.ptr()).liquid = 255;
                terraria::tile::liquid_type__int(t.ptr(), 0);
            }
            place_tile(x, gy + 6, 53, 0);
        }
        for x in wx..wx + 4 {
            if x == wx + 1 {
                continue;
            }
            for y in (gy + 3..=gy + 5).rev() {
                terraria::world_gen::place_underwater_plant(549, x, y);
            }
        }
        r.probe_tiles("seaweed", wx, gy, 4, 7);
        r.frames(10);
        r.record("seaweed", 1);
        for x in wx..wx + 4 {
            for y in gy..=gy + 6 {
                kill_tile(x, y);
                (*gallery_tile(x, y).ptr()).liquid = 0;
                if y <= gy + 5 {
                    (*gallery_tile(x, y).ptr()).wall = walls[(x - wx) as usize][(y - gy) as usize];
                }
                place_tile(x, y, 0, 0);
            }
        }
        clear_entities();
        clear_fx();
        clear_items();
        r.frames(2);
        r.state.push_str(&state_line("seaweed"));
    }
}

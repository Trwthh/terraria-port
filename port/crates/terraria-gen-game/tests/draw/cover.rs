// The coverage run (`DRAW_MODE=cover`, refdata/cover): tools/refdump_draw/Cover.cs, step for step.
// Included by tests/draw.rs.

fn cs_bool(b: bool) -> &'static str {
    if b { "True" } else { "False" }
}

/// Cover.cs `NewItemOf`.
unsafe fn cover_new_item(ty: i32) -> Gc<terraria::item::Item> {
    let it = terraria::item::new__void();
    terraria::item::set_defaults(it.ptr(), ty, Gc::NULL);
    it
}

/// Cover.cs `CoverDollItems`: the same rule.
unsafe fn cover_doll_items() -> Vec<[i32; 2]> {
    let mut list = Vec::new();
    let it = terraria::item::new__void();
    let pr = terraria::projectile::new();
    let poses = (*terraria::game_content::tile_entities::te_display_doll::s()).supported_use_style_poses;
    let mut seen = std::collections::HashSet::new();
    let count = (*terraria::id::item_id::s()).count as i32;
    for t in 1..count {
        terraria::item::set_defaults(it.ptr(), t, Gc::NULL);
        let (us, shoot) = ((*it.ptr()).use_style, (*it.ptr()).shoot);
        if shoot <= 0 || shoot >= GALLERY_PROJ_COUNT || !(*poses.ptr()).contains_key(us) {
            continue;
        }
        terraria::projectile::set_defaults(pr.ptr(), shoot);
        if !seen.insert((*pr.ptr()).ai_style) {
            continue;
        }
        let l = (*poses.ptr()).get_item(us);
        for k in 0..(*l.ptr()).count() {
            list.push([t, (*l.ptr()).get_item(k).pose]);
        }
    }
    list
}

/// Cover.cs `CoverDiag`.
unsafe fn cover_diag(x: i32, y: i32, w: i32, h: i32) -> String {
    let mut s = String::new();
    for j in y..y + h {
        if j != y {
            s.push('|');
        }
        for i in x..x + w {
            if i != x {
                s.push(' ');
            }
            let t = gallery_tile(i, j);
            if !t.is_null() && ((*t.ptr()).s_tile_header & 0x20) != 0 {
                s.push_str(&(*t.ptr()).type_.to_string());
            } else {
                s.push('.');
            }
        }
    }
    s
}

unsafe fn cover_entity(id: i32) -> Gc<terraria::data_structures::tile_entity::TileEntity> {
    (*(*terraria::data_structures::tile_entity::s()).by_id.ptr()).get_item(id)
}

/// Cover.cs `CoverWorld`.
unsafe fn cover_world(r: &mut Run) {
    let m = terraria::main::s();
    let (sx, sy) = ((*m).spawn_tile_x, (*m).spawn_tile_y);
    let pl = local_player();
    set_time(true, 27000.0);
    place_player(sx, sy);
    gallery_immune();
    r.frames(30);
    let x0 = sx + BUILD_DX;
    let gy = ground_below(x0, sy - 12);
    r.note(format!("cover ground at {x0},{gy}"));
    for x in x0 - 2..=x0 + BUILD_W {
        for y in gy - 12..gy {
            kill_tile(x, y);
        }
        for y in gy..=gy + 1 {
            place_tile(x, y, 0, 0);
        }
    }
    r.frames(5);
    // ---- 1. display dolls
    {
        use terraria::game_content::tile_entities::{te_display_doll as tdd, te_hat_rack as thr};
        let mut ids = Vec::new();
        for k in 0..6 {
            let x = x0 + 40 + k * 3;
            let ok = terraria::world_gen::place_object(x, gy - 1, 470, true, 0, 0, -1, 1);
            r.note(format!("doll {k}: {}", cs_bool(ok)));
            let id = tdd::hook_after_placement(x, gy - 1, 470, 0, 1, 0);
            ids.push(cover_entity(id).up::<tdd::TEDisplayDoll>());
        }
        let equip = [231, 232, 233];
        for d in &ids {
            let eq = (*d.ptr())._equip;
            for k in 0..3 {
                *eq.at(k) = rt::wbv(cover_new_item(equip[k as usize]));
            }
        }
        place_player(x0 + 38, gy - 1);
        r.frames(5);
        let items = cover_doll_items();
        r.note(format!("doll items: {}: {}", items.len(), items.iter().map(|i| format!("{}/{}", i[0], i[1])).collect::<Vec<_>>().join(",")));
        let mut b = 0;
        while b < items.len() {
            for k in 0..6 {
                let d = ids[k];
                let e = if b + k < items.len() { items[b + k] } else { [0, 0] };
                *(*d.ptr())._misc.at(0) = rt::wbv(cover_new_item(e[0]));
                (*d.ptr())._pose = e[1] as u8;
            }
            r.frames(2);
            r.record(&format!("doll_{b}"), 1);
            b += 6;
        }
        for k in 0..2 {
            let x = x0 + 60 + k * 4;
            let ok = terraria::world_gen::place_object(x, gy - 1, 475, true, 0, 0, -1, 1);
            r.note(format!("hatrack {k}: {}", cs_bool(ok)));
            let id = thr::hook_after_placement(x, gy - 1, 475, 0, 1, 0);
            let h = cover_entity(id).up::<thr::TEHatRack>();
            let hi = (*h.ptr())._items;
            *hi.at(0) = rt::wbv(cover_new_item(if k == 0 { 1243 } else { 3884 }));
            *hi.at(1) = rt::wbv(cover_new_item(if k == 0 { 1282 } else { 869 }));
        }
        r.frames(3);
        r.record("hatracks", 1);
        r.frames(3);
    }
    // ---- 2. doors
    {
        use terraria::world_gen as wg;
        let dx = x0 + 10;
        place_tile(dx, gy - 4, 0, 0);
        let ok = place_tile(dx, gy - 1, 10, 0);
        r.note(format!("door: {} {}", cs_bool(ok), cover_diag(dx - 1, gy - 4, 3, 5)));
        place_player(dx - 3, gy - 1);
        r.frames(3);
        let ok = wg::open_door(dx, gy - 2, 1);
        r.note(format!("open right: {}", cs_bool(ok)));
        r.frames(5);
        r.record("door_open", 1);
        let ok = wg::close_door(dx, gy - 2, false);
        r.note(format!("close: {}", cs_bool(ok)));
        r.frames(5);
        let ok = wg::open_door(dx, gy - 2, -1);
        r.note(format!("open left: {}", cs_bool(ok)));
        r.frames(5);
        let ok = wg::close_door(dx, gy - 2, false);
        r.note(format!("close: {}", cs_bool(ok)));
        r.frames(3);
        let tx = x0 + 20;
        for x in tx..=tx + 1 {
            kill_tile(x, gy);
            kill_tile(x, gy + 1);
        }
        let ok = wg::place_object(tx, gy, 387, true, 0, 0, -1, 1);
        r.note(format!("trapdoor: {} {}", cs_bool(ok), cover_diag(tx - 1, gy - 1, 4, 4)));
        r.frames(3);
        let ok = wg::shift_trapdoor(tx, gy, false, -1);
        r.note(format!("trapdoor open: {}", cs_bool(ok)));
        r.frames(5);
        r.record("trapdoor_open", 1);
        let (mut ox, mut oy) = (-1, -1);
        'f: for y in gy..=gy + 2 {
            for x in tx..=tx + 1 {
                let t = gallery_tile(x, y);
                if !t.is_null() && ((*t.ptr()).s_tile_header & 0x20) != 0 && (*t.ptr()).type_ == 386 {
                    (ox, oy) = (x, y);
                    break 'f;
                }
            }
        }
        r.note(format!("trapdoor open at {ox},{oy} {}", cover_diag(tx - 1, gy - 1, 4, 4)));
        let ok = ox >= 0 && wg::shift_trapdoor(ox, oy, true, -1);
        r.note(format!("trapdoor close: {}", cs_bool(ok)));
        r.frames(5);
        for x in x0..=x0 + 24 {
            for y in gy - 4..gy {
                kill_tile(x, y);
            }
        }
        for x in tx..=tx + 1 {
            kill_tile(x, gy);
            place_tile(x, gy, 0, 0);
            place_tile(x, gy + 1, 0, 0);
        }
        r.frames(3);
    }
    // ---- 3. a Christmas tree, minecart tracks
    {
        let xx = x0 + 6;
        terraria::world_gen::place_xmas_tree(xx, gy - 1, 171);
        let t = gallery_tile(xx - 1, gy - 8);
        let active = !t.is_null() && ((*t.ptr()).s_tile_header & 0x20) != 0;
        r.note(format!("xmas tree: {} {}", cs_bool(active), cover_diag(xx - 1, gy - 8, 4, 9)));
        if !t.is_null() {
            (*t.ptr()).frame_y = (1 | (1 << 3) | (1 << 7) | (1 << 11)) as i16;
        }
        for x in x0 + 14..=x0 + 24 {
            place_tile(x, gy - 1, 314, 0);
        }
        place_player(x0 + 2, gy - 1);
        r.frames(5);
        r.record("xmas_tracks", 1);
        kill_tile(xx, gy - 1);
        r.frames(5);
        r.record("xmas_killed", 1);
        for x in x0..=x0 + 26 {
            for y in gy - 9..gy {
                kill_tile(x, y);
            }
        }
        clear_entities();
        r.frames(3);
    }
    // ---- 4. teleporters, a logic gate
    {
        use terraria::world_gen as wg;
        let (a, b) = (x0 + 4, x0 + 20);
        let (oa, ob, os) = (place_tile(a, gy - 1, 235, 0), place_tile(b, gy - 1, 235, 0), place_tile(x0 + 12, gy - 1, 136, 0));
        r.note(format!("teleporter a: {}, b: {}, switch: {} {}", cs_bool(oa), cs_bool(ob), cs_bool(os), cover_diag(a - 1, gy - 2, 5, 3)));
        for x in a..=b + 2 {
            let _ = wg::place_wire(x, gy);
        }
        for x in a..=b + 2 {
            let _ = wg::place_wire(x, gy - 1);
        }
        place_player(a + 1, gy - 1);
        r.frames(5);
        terraria::wiring::hit_switch(x0 + 12, gy - 1);
        r.frames(10);
        r.record("teleport", 1);
        let lx = x0 + 28;
        let (og, ol) = (place_tile(lx, gy - 1, 420, 0), place_tile(lx, gy - 2, 419, 0));
        r.note(format!("gate: {}, lamp: {} {}", cs_bool(og), cs_bool(ol), cover_diag(lx - 1, gy - 3, 3, 4)));
        let _ = wg::place_wire(lx - 1, gy - 1);
        let _ = wg::place_wire(lx, gy - 1);
        let _ = wg::place_wire(lx - 1, gy - 1);
        let _ = wg::place_wire(lx - 2, gy - 1);
        let ok = place_tile(lx - 2, gy - 1, 136, 0);
        r.note(format!("switch 2: {}", cs_bool(ok)));
        terraria::wiring::hit_switch(lx - 2, gy - 1);
        r.frames(10);
        r.record("logicgate", 1);
        for x in x0 - 2..=x0 + 34 {
            for y in gy - 6..gy {
                kill_tile(x, y);
                let _ = wg::kill_wire(x, y);
            }
        }
        for x in x0 - 2..=x0 + 34 {
            let _ = wg::kill_wire(x, gy);
        }
        r.frames(3);
    }
    // ---- 5. the inventory pages
    {
        place_player(sx, sy);
        (*m).player_inventory = true;
        let cb = (*terraria::game_content::banner_system::s()).claimable_banners;
        for k in 1..=6 {
            *cb.at(k) = 3;
        }
        let _ = terraria::main::try_change_pips_page(1);
        r.frames(3);
        r.record("inv_banners", 1);
        (*m).pips_use_grid = true;
        r.frames(3);
        r.record("inv_banners_grid", 1);
        let _ = terraria::main::try_change_pips_page(0);
        (*m).pips_use_grid = true;
        r.frames(3);
        r.record("inv_recipes_grid", 1);
        (*m).pips_use_grid = false;
        (*m).equip_page_selected = 1;
        (*m).equip_page = 1;
        r.frames(3);
        r.record("inv_housing", 1);
        (*m).equip_page_selected = 0;
        (*m).equip_page = 0;
        (*m).player_inventory = false;
        for k in 1..=6 {
            *cb.at(k) = 0;
        }
        r.frames(3);
    }
    // ---- 6. the map saved and loaded again
    {
        let map = (*m).map;
        terraria::map::world_map::save(map.ptr());
        terraria::map::world_map::load(map.ptr());
        r.frames(3);
        (*m).map_fullscreen = true;
        r.frames(3);
        r.record("map_loaded", 1);
        (*m).map_fullscreen = false;
        r.frames(3);
    }
    // ---- 7. every accessory, five at a time
    {
        let it = terraria::item::new__void();
        let mut acc = Vec::new();
        let count = (*terraria::id::item_id::s()).count as i32;
        for t in 1..count {
            terraria::item::set_defaults(it.ptr(), t, Gc::NULL);
            if (*it.ptr()).accessory {
                acc.push(t);
            }
        }
        r.note(format!("accessories: {}", acc.len()));
        let mut b = 0;
        while b < acc.len() {
            place_player(sx, sy);
            for k in 0..5 {
                set_armor(3 + k as i32, if b + k < acc.len() { acc[b + k] } else { 0 });
            }
            r.frames(3);
            r.record(&format!("acc_{b}"), 1);
            b += 5;
        }
        for k in 0..5 {
            set_armor(3 + k, 0);
        }
        clear_entities();
        r.frames(5);
    }
    // ---- 8. every mount, walking right
    {
        let mc = (*terraria::id::mount_id::s()).count;
        for mt in 0..mc {
            place_player(sx, sy);
            let mount = (*pl.ptr()).mount;
            terraria::mount::set_mount(mount.ptr(), mt, pl);
            r.input.keys = vec![KEY_D];
            r.frames(12);
            r.record(&format!("mount_{mt}"), 1);
            r.input.keys.clear();
            let mount = (*pl.ptr()).mount;
            terraria::mount::dismount(mount.ptr(), pl, false);
            r.frames(3);
        }
        clear_entities();
        place_player(sx, sy);
        r.frames(10);
    }
    // ---- 9. skies
    {
        use terraria::game_content::events::{birthday_party, lantern_night};
        birthday_party::toggle_manual_party();
        r.frames(60);
        r.record("sky_party", 1);
        birthday_party::toggle_manual_party();
        set_time(false, 3600.0);
        lantern_night::toggle_manual_lanterns();
        r.frames(90);
        r.record("sky_lanterns", 1);
        lantern_night::toggle_manual_lanterns();
        set_time(true, 27000.0);
        r.frames(30);
        terraria::main::start_invasion(4);
        r.frames(90);
        r.record("sky_martian", 1);
        (*m).invasion_type = 0;
        (*m).invasion_size = 0;
        clear_entities();
        r.frames(30);
    }
    // ---- 10. the Wall of Flesh
    {
        let p = (*pl.ptr()).base.position;
        terraria::npc::spawn_wof(p);
        r.frames(30);
        r.record("wof_spawn", 1);
        clear_entities();
        place_player(sx, sy);
        r.frames(20);
    }
    cover_world2(r, sx, sy, x0, gy);
}

/// Cover.cs `CoverWorld2`.
unsafe fn cover_world2(r: &mut Run, sx: i32, sy: i32, x0: i32, gy: i32) {
    let m = terraria::main::s();
    let pl = local_player();
    // ---- 11. UI states shown in game
    {
        use terraria::game_content::ui::states as st;
        let wc = st::ui_world_creation::new();
        let adv = st::ui_world_creation_advanced::new(wc, false);
        let states: Vec<(&str, Gc<terraria::ui::ui_state::UIState>)> = vec![
            ("ui_charselect", st::ui_character_select::new().up()),
            ("ui_achievements", (*m).achievements_menu.up()),
            ("ui_controls", (*m).manage_controls_menu.up()),
            ("ui_worldcreation", wc.up()),
            ("ui_worldcreation_adv", adv.up()),
            ("ui_secretseeds", st::ui_world_creation_advanced_secret_seeds_list::new(adv, wc).up()),
            ("ui_keyboard", st::ui_virtual_keyboard::new(rt::Str::new("Test"), rt::Str::new("abc"), rt::zero(), rt::zero(), 0, false, 20).up()),
        ];
        // the states live across frames: rooted for the collector (in the harness they are locals of CoverWorld2)
        let roots: Vec<rt::Rooted<Gc<terraria::ui::ui_state::UIState>>> = states.iter().map(|(_, st)| rt::Rooted::new(*st)).collect();
        for (name, state) in states {
            terraria::ui::ingame_fancy_ui::open_ui_state__ui_state_bool(state, true);
            r.frames(10);
            r.record(name, 1);
            terraria::ui::ingame_fancy_ui::close(false);
            r.frames(3);
        }
        drop(roots);
    }
    // ---- 12. the four lighting modes
    {
        for k in 0..4 {
            terraria::lighting::set_mode(k);
            r.frames(10);
            r.record(&format!("light_{k}"), 1);
        }
        terraria::lighting::set_mode(3);
        r.frames(5);
    }
    // ---- 13. the torch god
    {
        place_player(x0 + 10, gy - 1);
        for k in 0..12 {
            place_tile(x0 + k * 2, gy - 1, 4, 0);
        }
        r.frames(3);
        (*pl.ptr()).happy_fun_torch_time = true;
        r.frames(90);
        r.record("torchgod", 1);
        (*pl.ptr()).happy_fun_torch_time = false;
        clear_entities();
        for k in 0..12 {
            kill_tile(x0 + k * 2, gy - 1);
        }
        r.frames(5);
    }
    // ---- 14. the mystic sash
    {
        place_player(sx, sy);
        set_armor(3, 6189);
        r.frames(5);
        for _ in 0..60 {
            (*pl.ptr()).immune = false;
            (*pl.ptr()).immune_time = 0;
            (*pl.ptr()).stat_life = 100;
            let _ = terraria::player::hurt(pl.ptr(), terraria::data_structures::player_death_reason::legacy_default(), 1, 1, false, false, false, -1, true);
            r.frames(2);
        }
        r.record("player_mysticsash", 1);
        set_armor(3, 0);
        gallery_immune();
        r.frames(5);
    }
    // ---- 15. every sentry, carried
    {
        let pr = terraria::projectile::new();
        let mut sentries = Vec::new();
        for t in 1..GALLERY_PROJ_COUNT {
            terraria::projectile::set_defaults(pr.ptr(), t);
            if (*pr.ptr()).sentry {
                sentries.push(t);
            }
        }
        r.note(format!("sentries: {}", sentries.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",")));
        let p0 = (*pl.ptr()).base.position;
        let mut b = 0;
        while b < sentries.len() {
            clear_entities();
            place_player(sx, sy);
            let mut k = 0;
            while k < 10 && b + k < sentries.len() {
                let i = gallery_new_proj(p0.x - 160.0 + k as f32 * 32.0, p0.y - 40.0, 0.0, 0.0, sentries[b + k]);
                let q = *(*m).projectile.at(i);
                *(*q.ptr()).ai.at(2) = 1.0;
                k += 1;
            }
            r.frames(20);
            r.record(&format!("sentry_{b}"), 1);
            b += 10;
        }
        clear_entities();
        place_player(sx, sy);
        r.frames(10);
    }
    // ---- 16. a drunk world's music choice
    {
        (*m).drunk_world = true;
        r.frames(30);
        r.record("drunk_music", 1);
        (*m).drunk_world = false;
        r.frames(5);
    }
    // ---- 17. the gamepad user interface
    {
        use terraria::game_input::player_input as pi;
        let mouse_mode = pi::current_input_mode();
        pi::set_current_input_mode(4);
        (*m).player_inventory = true;
        r.frames(5);
        r.record("pad_inventory", 1);
        (*m).player_inventory = false;
        r.frames(3);
        terraria::ui::ingame_fancy_ui::open_ui_state__ui_state_bool((*m).manage_controls_menu.up(), true);
        r.frames(5);
        r.record("pad_controls", 1);
        terraria::ui::ingame_fancy_ui::close(false);
        r.frames(3);
        let emotes = rt::Rooted::new(terraria::game_content::ui::states::ui_emotes_menu::new().up::<terraria::ui::ui_state::UIState>());
        terraria::ui::ingame_fancy_ui::open_ui_state__ui_state_bool(emotes.get(), true);
        r.frames(5);
        r.record("pad_emotes", 1);
        terraria::ui::ingame_fancy_ui::close(false);
        r.frames(3);
        drop(emotes);
        let names = ["Keyboard", "KeyboardUI", "Mouse", "XBoxGamepad", "XBoxGamepadUI"];
        r.note(format!("input mode at the end: {}", names.get(pi::current_input_mode() as usize).copied().unwrap_or("?")));
        pi::set_current_input_mode(mouse_mode);
        r.frames(5);
    }
    // ---- 18. the surface backgrounds of the corruption, the hallow and the crimson
    {
        for (ty, k) in [(25, 0), (117, 1), (203, 2)] {
            for x in x0 - 2..=x0 + 34 {
                for y in gy + 1..=gy + 9 {
                    kill_tile(x, y);
                    place_tile(x, y, ty, 0);
                }
            }
            place_player(x0 + 16, gy - 1);
            r.frames(150);
            r.record(&format!("bg_biome_{k}"), 1);
            for x in x0 - 2..=x0 + 34 {
                for y in gy + 1..=gy + 9 {
                    kill_tile(x, y);
                    place_tile(x, y, 0, 0);
                }
            }
            r.frames(30);
        }
        clear_entities();
        r.frames(5);
    }
    // ---- 19. an item left in shimmer until it transforms
    {
        let px = x0 + 4;
        for x in px..px + 6 {
            for y in gy..=gy + 1 {
                kill_tile(x, y);
                let t = gallery_tile(x, y);
                (*t.ptr()).liquid = 255;
                terraria::tile::liquid_type__int(t.ptr(), 3);
            }
        }
        for x in px - 1..=px + 6 {
            place_tile(x, gy + 2, 0, 0);
        }
        place_tile(px - 1, gy + 1, 0, 0);
        place_tile(px + 6, gy + 1, 0, 0);
        for ty in [9, 75] {
            interact_new_item((px + 2) * 16, gy * 16, ty, 1);
        }
        place_player(px - 3, gy - 1);
        r.frames(240);
        r.record("shimmer_item_long", 1);
    }
}

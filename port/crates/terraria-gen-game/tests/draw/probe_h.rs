// The probe run, part H: tools/refdump_draw/ProbeH.cs, step for step. Included by tests/draw.rs.

/// ProbeH.cs `CombatList`.
unsafe fn combat_list() -> String {
    let cts = (*terraria::main::s()).combat_text;
    let mut out = String::new();
    for i in 0..cts.len() {
        let c = *cts.at(i);
        if !(*c.ptr()).active {
            continue;
        }
        let c = &*c.ptr();
        out.push_str(&format!("{}:{}:{}:{}:{}:{}:{};", i, c.position.pv(), c.velocity.pv(), c.scale.pv(), c.alpha.pv(), c.rotation.pv(), c.life_time));
    }
    out
}

/// ProbeH.cs `ProbeH`.
unsafe fn probe_h(r: &mut Run, sx: i32, sy: i32, x0: i32, gy: i32) {
    let m = terraria::main::s();
    let pl = local_player();
    // ---- every hair dye worn
    if probe_group(r, "hairdye") {
        place_player(sx, sy);
        let it = terraria::item::new__void();
        let count = (*terraria::id::item_id::s()).count as i32;
        let mut dyes = Vec::new();
        for t in 1..count {
            terraria::item::set_defaults(it.ptr(), t, Gc::NULL);
            if (*it.ptr()).hair_dye >= 0 {
                dyes.push((*it.ptr()).hair_dye);
            }
        }
        r.note(format!("hair dyes: {}", dyes.iter().map(|d| d.to_string()).collect::<Vec<_>>().join(",")));
        for (k, &d) in dyes.iter().enumerate() {
            (*pl.ptr()).hair_dye = d as u8;
            (*pl.ptr()).stat_life = 20 + k as i32 * 7;
            r.frames(2);
            if k % 4 == 0 {
                r.record(&format!("hairdye_{k}"), 1);
            }
        }
        (*pl.ptr()).hair_dye = 0;
        (*pl.ptr()).stat_life = 100;
        r.frames(2);
    }
    // ---- every torch at night
    if probe_group(r, "torches") {
        set_time(false, 3600.0);
        place_player(x0 + 20, gy - 1);
        let n = (*terraria::id::torch_id::s()).count as i32;
        for k in 0..n {
            place_tile(x0 + 2 + k * 2, gy - 1, 4, k);
        }
        r.frames(30);
        r.record("torches", 1);
        clear_area(x0, x0 + 2 + n * 2, gy);
        set_time(true, 27000.0);
        r.frames(3);
    }
    // ---- every gore type
    if probe_group(r, "gores") {
        place_player(sx, sy);
        let pp = (*pl.ptr()).base.position;
        let count = (*terraria::id::gore_id::s()).count;
        let mut b = 1;
        while b < count {
            clear_fx();
            let mut t = b;
            while t < b + 60 && t < count {
                terraria::gore::new_gore(v2(pp.x - 300.0 + (t - b) as f32 * 10.0, pp.y - 100.0), v2(((t % 7) - 3) as f32 * 0.5, -2.0 - (t % 3) as f32), t, 1.0);
                t += 1;
            }
            r.frames(15);
            probe_fx(r, &format!("gores_{b}"));
            if (b / 60) % 4 == 0 {
                r.record(&format!("gores_{b}"), 1);
            }
            b += 60;
        }
        clear_fx();
        r.frames(2);
    }
    // ---- the legacy lighting engine
    if probe_group(r, "legacylight") {
        for md in [0, 1, 2] {
            terraria::lighting::set_mode(md);
            terraria::lighting::clear();
            set_time(false, 31000.0);
            for k in 0..4 {
                r.frames(10);
                (*m).time = 31000.0 + k as f64 * 300.0;
            }
            set_time(true, 0.0);
            r.frames(10);
            r.record(&format!("legacylight_{md}"), 1);
        }
        terraria::lighting::set_mode(3);
        set_time(true, 27000.0);
        r.frames(10);
    }
    // ---- a minecart ride
    if probe_group(r, "minecart") {
        for x in x0 - 2..=x0 + BUILD_W {
            place_tile(x, gy - 1, 314, 0);
        }
        r.frames(3);
        let mc = (*terraria::id::mount_id::s()).count;
        let is_cart = (*terraria::id::mount_id::sets::s()).cart;
        let carts: Vec<i32> = (0..mc).filter(|&k| *is_cart.at(k)).collect();
        r.note(format!("carts: {}", carts.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(",")));
        for &cart in &carts {
            clear_entities();
            place_player(x0 + 2, gy - 2);
            terraria::mount::set_mount((*pl.ptr()).mount.ptr(), cart, pl);
            (*pl.ptr()).base.velocity = v2(12.0, 0.0);
            r.input.keys = vec![68];
            r.frames(20);
            r.input.keys = vec![65];
            r.frames(10);
            r.input.keys = vec![32];
            r.frames(10);
            r.input.keys = Vec::new();
            r.frames(10);
            r.probe(&format!("cart_{cart}"), &[&(*pl.ptr()).base.position, &(*pl.ptr()).base.velocity, &(*pl.ptr()).full_rotation]);
            r.probe(&format!("cart_ct_{cart}"), &[&combat_list()]);
            if cart % 3 == 0 {
                r.record(&format!("cart_{cart}"), 1);
            }
            terraria::mount::dismount((*pl.ptr()).mount.ptr(), pl, false);
            r.frames(3);
        }
        clear_area(x0 - 2, x0 + BUILD_W, gy);
        clear_entities();
        place_player(sx, sy);
        r.frames(10);
    }
    // ---- small direct calls
    if probe_group(r, "direct2") {
        let mut pure = false;
        let text = terraria::lang::get_dryad_world_status_dialog(&mut pure);
        r.probe("dryadstatus", &[&text, &pure]);
        let wfd = (*m).active_world_file_data;
        let (a, b) = (terraria::io::world_file_data::get_full_seed_text(wfd.ptr(), false), terraria::io::world_file_data::get_full_seed_text(wfd.ptr(), true));
        r.probe("seedtext", &[&a, &b]);
        use terraria::initializers::ui_links_initializer as uli;
        for v in [0.0f32, 0.3, 0.99] {
            let (h, vv) = (uli::handle_slider_horizontal_input(v, 0.0, 1.0, 0.2, 0.5), uli::handle_slider_vertical_input(v, 0.0, 1.0, 0.2, 0.5));
            r.probe(&format!("slider_{}", v.pv()), &[&h, &vv]);
        }
        let pp = (*pl.ptr()).base.position;
        for v in [pp, v2(pp.x + 37.5, pp.y - 19.25), v2(31999.7, 9000.3)] {
            let l = terraria::lighting::get_sub_light(v);
            r.probe(&format!("sublight_{}", v.pv()), &[&l]);
        }
        let ad = terraria::id::colors::alpha_darken(terraria_core::xna::Color::new(200, 100, 50, 255));
        r.probe("alphadarken", &[&ad]);
        clear_entities();
        use terraria::game_input::lock_on_helper as lo;
        for k in 0..3 {
            new_npc_at(pp.x as i32 + 100 + k * 60, pp.y as i32, 3);
        }
        r.frames(2);
        lo::refresh_targets(v2(pp.x + 100.0, pp.y), 2000.0);
        r.note(format!("lock-on targets {}", (*(*lo::s())._targets.ptr()).count()));
        for k in 0..3 {
            lo::cycle_target_focus();
            let (picked, pred) = ((*lo::s())._picked_target, lo::predicted_position());
            r.probe(&format!("lockon_{k}"), &[&picked, &pred]);
        }
        clear_entities();
        terraria::game_input::player_input::handle_dpad_snap();
        r.probe("dpadsnap", &[&(*m).mouse_x, &(*m).mouse_y]);
        let ssp = (*terraria::game_input::player_input::s()).smart_select_pointer;
        let (mut tx, mut ty) = (0i32, 0i32);
        terraria::game_input::smart_select_gamepad_pointer::smart_select_lookup_get_target_tile(ssp.ptr(), pl, &mut tx, &mut ty);
        r.probe("smartselect", &[&tx, &ty]);
        r.frames(2);
    }
}

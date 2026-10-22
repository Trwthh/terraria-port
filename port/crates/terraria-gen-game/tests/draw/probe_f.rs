// The probe run, part F: tools/refdump_draw/ProbeF.cs, step for step. Included by tests/draw.rs.

/// ProbeF.cs `PEocEntities`.
unsafe fn probe_eoc_entities(r: &mut Run, sky: *mut terraria::game_content::skies::ambient_sky::AmbientSky, key: &str) {
    use re_logic::utilities::slot_vector__sky_entity as sv;
    use terraria::game_content::skies::ambient_sky::{eoc_sky_entity as eoc, fading_sky_entity as fse, sky_entity as se};
    let v = (*sky)._entities;
    let arr = (*v.ptr())._array;
    let used = (*v.ptr())._used_space_length;
    for i in 0..used as i32 {
        if !sv::has__int(v.ptr(), i) {
            continue;
        }
        let e = (*arr.at(i)).value;
        if e.is_null() || !rt::obj_is::<eoc::EOCSkyEntity>(e.up()) {
            continue;
        }
        let (s, f, x) = (e.ptr() as *mut se::SkyEntity, e.ptr() as *mut fse::FadingSkyEntity, e.ptr() as *mut eoc::EOCSkyEntity);
        r.probe(&format!("{key}_{i}"), &[&(*s).position, &(*f).velocity, &(*s).depth, &(*x)._state, &(*f).life_time]);
    }
}

/// ProbeF.cs `ProbeF`.
unsafe fn probe_f(r: &mut Run, sx: i32, sy: i32, x0: i32, gy: i32) {
    let m = terraria::main::s();
    let pl = local_player();
    // ---- luck rolls
    if probe_group(r, "luck") {
        use terraria::game_content::luck;
        for lk in [-0.5f32, -0.1, 0.0, 0.3, 1.0] {
            for range in [10, 50] {
                let mut a = Vec::new();
                for _ in 0..4 {
                    a.push(luck::roll_bad_luck(lk, range));
                    a.push(luck::roll_bad_luck_extreme(lk, range));
                }
                let refs: Vec<&dyn Pv> = a.iter().map(|v| v as &dyn Pv).collect();
                r.probe(&format!("luck_{}_{range}", lk.pv()), &refs);
            }
        }
    }
    // ---- Lucy the axe
    if probe_group(r, "lucy") {
        for (from, to) in [(0, 3), (3, 0), (0, 0), (4, 0)] {
            let mut info: terraria::ui::item_slot::item_transfer_info::ItemTransferInfo = rt::zero();
            info.item_type = 5095;
            info.transfer_amount = 1;
            info.from_contenxt = from;
            info.to_context = to;
            terraria::game_content::lucy_axe_message::item_slot_on_item_transferred(info);
            r.frames(1);
        }
        r.frames(30);
        r.record("lucy", 1);
    }
    // ---- the digger cart
    if probe_group(r, "digger") {
        let (tx, ty) = (1999, 547);
        for x in tx - 6..=tx + 6 {
            kill_tile(x, ty);
            place_tile(x, ty, 314, 0);
        }
        let dh = (*terraria::game_content::minecart_digger_helper::s()).instance;
        place_player(tx, ty - 1);
        r.probe_tiles("digger_before", tx - 10, ty - 6, 21, 10);
        for dir in [1, -1] {
            terraria::game_content::minecart_digger_helper::try_digging(dh.ptr(), pl, v2((tx * 16 + 8 + dir * 96) as f32, (ty * 16 + 8) as f32), dir, 0);
        }
        r.probe_tiles("digger_after", tx - 10, ty - 6, 21, 10);
        r.frames(5);
        r.record("digger", 1);
        place_player(sx, sy);
        r.frames(5);
    }
    // ---- a boss damage report
    if probe_group(r, "dmgreport") {
        use terraria::game_content::npc_damage_tracker as tr;
        clear_entities();
        let boss = new_npc_at(sx * 16 + 300, sy * 16 - 200, 50);
        for d in [120, 37, 455] {
            tr::add_damage__npc_int_int(npc_at(boss), (*m).my_player, d);
        }
        let active = (*tr::s())._active_trackers;
        r.note(format!("trackers {}", (*active.ptr()).count()));
        let rep = tr::get_report((*active.ptr()).get_item(0).ptr(), pl);
        let text = terraria::localization::network_text::to_string(rep.ptr());
        r.probe("dmgreport", &[&text]);
        let arr = Arr::<i32>::new(4);
        for (k, v) in [120, 37, 455, 1].iter().enumerate() {
            *arr.at(k as i32) = *v;
        }
        let pc = tr::calculate_percentages(arr);
        let vals: Vec<i32> = (0..pc.len()).map(|k| *pc.at(k)).collect();
        let refs: Vec<&dyn Pv> = vals.iter().map(|v| v as &dyn Pv).collect();
        r.probe("dmgpercent", &refs);
        tr::reset();
        clear_entities();
        r.frames(2);
    }
    // ---- beds
    if probe_group(r, "beds") {
        use terraria::game_content::player_sleeping_helper as psh;
        let bx = x0 + 4;
        for style in 0..48 {
            if terraria::object_data::tile_object_data::get_tile_data__int_int_int(79, style, 0).is_null() {
                continue;
            }
            clear_area(x0, x0 + 20, gy);
            let ok = terraria::world_gen::place_object(bx, gy - 1, 79, true, style, 0, -1, 1);
            let (mut dir, mut anchor, mut off) = (0i32, v2(0.0, 0.0), v2(0.0, 0.0));
            let res = psh::get_sleeping_target_info(bx, gy - 1, &mut dir, &mut anchor, &mut off);
            r.probe(&format!("bed_{style}"), &[&ok, &res, &dir, &anchor, &off]);
        }
        clear_area(x0, x0 + 20, gy);
        terraria::world_gen::place_object(bx, gy - 1, 79, true, 0, 0, -1, 1);
        place_player(bx + 2, gy - 1);
        r.frames(2);
        let mut copy = (*pl.ptr()).sleeping;
        psh::start_sleeping(&mut copy, pl, bx, gy - 1);
        (*pl.ptr()).sleeping = copy;
        r.frames(30);
        r.record("bed_sleep", 1);
        let mut copy = (*pl.ptr()).sleeping;
        psh::stop_sleeping(&mut copy, pl, true);
        (*pl.ptr()).sleeping = copy;
        clear_area(x0, x0 + 20, gy);
        r.frames(3);
    }
    // ---- portals
    if probe_group(r, "portal") {
        clear_entities();
        for y in gy - 8..gy {
            place_tile(x0 + 2, y, 1, 0);
            place_tile(x0 + 30, y, 1, 0);
        }
        place_player(x0 + 16, gy - 1);
        r.frames(2);
        let pp = (*pl.ptr()).base.position;
        for (vx, form) in [(12.0f32, 0.0f32), (-12.0, 1.0), (12.0, 0.0)] {
            let i = gallery_new_proj(pp.x + 10.0, pp.y + 10.0, vx, 0.0, 601);
            *(*proj_at(i).ptr()).ai.at(0) = form;
            r.frames(15);
        }
        let portals = (*m).projectile.as_slice().iter().filter(|p| (*p.ptr()).active && (*p.ptr()).type_ == 602).count() as i32;
        r.probe("portals", &[&portals]);
        r.input.keys = vec![68];
        r.frames(40);
        r.input.keys = Vec::new();
        r.probe("portal_player", &[&(*pl.ptr()).base.position]);
        r.record("portal", 1);
        clear_entities();
        clear_area(x0, x0 + 34, gy);
        place_player(sx, sy);
        r.frames(5);
    }
    // ---- the potion of return's gates
    if probe_group(r, "returngate") {
        place_player(sx, sy);
        let pp = (*pl.ptr()).base.position;
        (*pl.ptr()).potion_of_return_original_use_position = Nullable { has_value: true, value: v2(pp.x + 100.0, pp.y + 40.0) };
        (*pl.ptr()).potion_of_return_home_position = Nullable { has_value: true, value: v2(pp.x - 120.0, pp.y + 40.0) };
        r.frames(30);
        r.record("returngate", 1);
        (*pl.ptr()).potion_of_return_original_use_position = rt::zero();
        (*pl.ptr()).potion_of_return_home_position = rt::zero();
        r.frames(3);
    }
    // ---- quick stacking
    if probe_group(r, "quickstack") {
        place_player(x0 + 4, gy - 1);
        let cx = x0 + 6;
        place_tile(cx, gy - 1, 21, 0);
        place_tile(x0 + 9, gy - 1, 19, 0);
        place_tile(x0 + 10, gy - 1, 19, 0);
        terraria::world_gen::place_object(x0 + 9, gy - 2, 29, true, 0, 0, -1, 1);
        let ci = terraria::chest::find_chest(cx, gy - 2);
        r.note(format!("quickstack chest {ci} {}", cover_diag(x0 + 4, gy - 3, 8, 3)));
        let chest = *(*m).chest.at(ci);
        let citems = (*chest.ptr()).item;
        terraria::item::set_defaults((*citems.at(0)).ptr(), 9, Gc::NULL);
        (*(*citems.at(0)).ptr()).stack = 5;
        terraria::item::set_defaults((*citems.at(1)).ptr(), 3, Gc::NULL);
        (*(*citems.at(1)).ptr()).stack = 990;
        let bitems = (*(*pl.ptr()).bank.ptr()).item;
        terraria::item::set_defaults((*bitems.at(0)).ptr(), 2, Gc::NULL);
        (*(*bitems.at(0)).ptr()).stack = 7;
        set_inv(10, 9, 20);
        set_inv(11, 3, 30);
        set_inv(12, 2, 15);
        terraria::game_content::quick_stacking::quick_stack_to_nearby_inventories(pl, false);
        r.frames(5);
        let (c0, c1, b0) = ((*(*citems.at(0)).ptr()).stack, (*(*citems.at(1)).ptr()).stack, (*(*bitems.at(0)).ptr()).stack);
        r.probe("quickstack", &[&count_item(9), &count_item(3), &count_item(2), &c0, &c1, &b0]);
        r.record("quickstack", 1);
        for k in 10..13 {
            give_item(k, 0);
        }
        for k in 0..citems.len() {
            terraria::item::turn_to_air((*citems.at(k)).ptr());
        }
        for k in 0..bitems.len() {
            terraria::item::turn_to_air((*bitems.at(k)).ptr());
        }
        clear_area(x0, x0 + 12, gy);
        clear_items();
        r.frames(3);
    }
    // ---- weather skies
    if probe_group(r, "weather") {
        terraria::main::start_rain(true, Nullable { has_value: true, value: 1.0 }, false);
        place_player(2499, 298);
        r.frames(150);
        r.record("blizzard", 1);
        terraria::main::stop_rain(true);
        r.frames(150);
        use terraria::game_content::events::sandstorm as ss;
        ss::start_sandstorm();
        (*ss::s()).severity = 1.0;
        (*ss::s()).intended_severity = 1.0;
        place_player(1720, 352);
        r.frames(150);
        r.record("sandstorm", 1);
        ss::stop_sandstorm();
        (*ss::s()).severity = 0.0;
        r.frames(150);
        place_player(sx, sy);
        let life = (*pl.ptr()).stat_life_max;
        (*pl.ptr()).stat_life_max = 400;
        terraria::main::start_invasion(4);
        r.note(format!("invasion {}", (*m).invasion_type));
        r.frames(60);
        r.record("martian_sky", 1);
        (*m).invasion_type = 0;
        (*m).invasion_size = 0;
        (*pl.ptr()).stat_life_max = life;
        clear_entities();
        r.frames(60);
    }
    // ---- shimmer-free landing spots
    if probe_group(r, "shimmerspot") {
        for (x, y, e) in [(sx, sy, 5), (sx + 40, sy - 20, 10), (1999, 547, 8)] {
            for top in [false, true] {
                let res = terraria::game_content::shimmer_helper::find_spot_without_shimmer(pl.up(), x, y, e, top);
                r.probe(&format!("shimmerspot_{x}_{y}_{}", cs_bool(top)), &[&res]);
            }
        }
    }
    // ---- the Eye of Cthulhu in the ambient sky
    if probe_group(r, "eocsky") {
        use terraria::graphics::effects::effect_manager__custom_sky as em;
        let mgr = (*terraria::graphics::effects::sky_manager::s()).instance;
        let sky = em::get_item(mgr.ptr() as *mut em::EffectManager, s!("Ambience"));
        set_time(false, 16200.0);
        for seed in 2000..2012 {
            terraria::game_content::skies::ambient_sky::spawn(sky.0 as *mut terraria::game_content::skies::ambient_sky::AmbientSky, pl, 4, seed);
        }
        let amb = sky.0 as *mut terraria::game_content::skies::ambient_sky::AmbientSky;
        probe_eoc_entities(r, amb, "eocsky_spawned");
        r.frames(1);
        probe_eoc_entities(r, amb, "eocsky_1");
        r.frames(119);
        probe_eoc_entities(r, amb, "eocsky_120");
        r.record("eocsky", 1);
        set_time(true, 27000.0);
        r.frames(30);
    }
}

// The probe run, part C: tools/refdump_draw/ProbeC.cs, step for step. Included by tests/draw.rs.

/// ProbeC.cs `FirstItemPerSlot`.
unsafe fn probe_first_item_per_slot(field: &str) -> Vec<i32> {
    let it = terraria::item::new__void();
    let count = (*terraria::id::item_id::s()).count as i32;
    let mut seen = std::collections::HashSet::new();
    let mut list = Vec::new();
    for t in 1..count {
        terraria::item::set_defaults(it.ptr(), t, Gc::NULL);
        let i = &*it.ptr();
        let v = match field {
            "headSlot" => i.head_slot,
            "bodySlot" => i.body_slot,
            "legSlot" => i.leg_slot,
            "wingSlot" => i.wing_slot as i32,
            _ => unreachable!(),
        };
        if v > 0 && seen.insert(v) {
            list.push(t);
        }
    }
    list
}

/// ProbeC.cs `BuildHouse`.
unsafe fn build_house(hx: i32, gy: i32) {
    for x in hx..=hx + 9 {
        for y in gy - 7..gy {
            kill_tile(x, y);
        }
    }
    for x in hx..=hx + 9 {
        place_tile(x, gy - 7, 30, 0);
        place_tile(x, gy, 30, 0);
    }
    for y in gy - 6..gy {
        place_tile(hx, y, 30, 0);
        place_tile(hx + 9, y, 30, 0);
    }
    for x in hx + 1..=hx + 8 {
        for y in gy - 6..gy {
            terraria::world_gen::place_wall(x, y, 4, true);
        }
    }
    for y in gy - 3..gy {
        kill_tile(hx, y);
    }
    place_tile(hx, gy - 1, 10, 0);
    place_tile(hx + 4, gy - 1, 14, 0);
    place_tile(hx + 7, gy - 1, 15, 0);
    place_tile(hx + 2, gy - 4, 4, 0);
}
/// ProbeC.cs `ClearArea`.
unsafe fn clear_area(xa: i32, xb: i32, gy: i32) {
    for x in xa..=xb {
        for y in gy - 12..gy {
            kill_tile(x, y);
            terraria::world_gen::kill_wall(x, y, false);
        }
        place_tile(x, gy, 0, 0);
        place_tile(x, gy + 1, 0, 0);
    }
}
/// ProbeC.cs `TopLeftOf`.
unsafe fn top_left_of(ty: i32, x: i32, y: i32) -> (i32, i32) {
    for j in y - 4..=y {
        for i in x - 2..=x {
            let t = gallery_tile(i, j);
            if !t.is_null() && ((*t.ptr()).s_tile_header & 0x20) != 0 && (*t.ptr()).type_ as i32 == ty {
                return (i, j);
            }
        }
    }
    (-1, -1)
}
/// RefDumpDraw.cs `GiveItem` with type 0 (an empty slot).
unsafe fn select_item(slot: i32) {
    terraria::player::selected_item_state::select(&mut (*local_player().ptr()).selected_item_state, slot);
}

/// ProbeC.cs `ProbeC`.
unsafe fn probe_c(r: &mut Run, sx: i32, sy: i32, x0: i32, gy: i32) {
    let m = terraria::main::s();
    let pl = local_player();
    // ---- every head, body, legs and wings look worn
    if probe_group(r, "wardrobe") {
        for (field, slot) in [("headSlot", 0), ("bodySlot", 1), ("legSlot", 2)] {
            let items = probe_first_item_per_slot(field);
            r.note(format!("wardrobe {field}: {}", items.len()));
            place_player(sx, sy);
            for (k, &it) in items.iter().enumerate() {
                set_armor(slot, it);
                r.frames(1);
                if k % 25 == 0 {
                    r.record(&format!("wardrobe_{field}_{k}"), 1);
                }
            }
            set_armor(slot, 0);
        }
        let wings = probe_first_item_per_slot("wingSlot");
        r.note(format!("wardrobe wingSlot: {}", wings.len()));
        for (k, &it) in wings.iter().enumerate() {
            set_armor(3, it);
            place_player(sx, sy - 30);
            r.frames(3);
            if k % 10 == 0 {
                r.record(&format!("wardrobe_wings_{k}"), 1);
            }
        }
        set_armor(3, 0);
        place_player(sx, sy);
        r.frames(60);
    }
    // ---- a house checked by the housing query
    if probe_group(r, "housing") {
        let hx = x0 + 4;
        build_house(hx, gy);
        place_player(hx + 12, gy - 1);
        r.frames(5);
        terraria::main::perform_housing_check(r.main.ptr(), hx + 4, gy - 2);
        r.frames(20);
        r.record("housing_good", 1);
        r.frames(150);
        kill_tile(hx, gy - 1);
        kill_tile(hx + 4, gy - 1);
        r.frames(3);
        terraria::main::perform_housing_check(r.main.ptr(), hx + 4, gy - 2);
        r.frames(20);
        r.record("housing_bad", 1);
        r.frames(150);
        clear_area(hx - 1, hx + 10, gy);
        r.frames(3);
    }
    // ---- the credits roll
    if probe_group(r, "credits") {
        use terraria::graphics::effects::effect_manager__custom_sky as em;
        place_player(sx, sy);
        terraria::game_content::events::credits_roll_event::try_starting_credits_roll();
        r.frames(2);
        let mgr = (*terraria::graphics::effects::sky_manager::s()).instance;
        let sky = em::get_item(mgr.ptr() as *mut em::EffectManager, s!("CreditsRoll"));
        let sky = sky.0 as *mut terraria::game_content::skies::credits_roll_sky::CreditsRollSky;
        let end = (*sky)._end_time;
        let active = terraria::graphics::effects::custom_sky::is_active__v(sky as *mut terraria::graphics::effects::custom_sky::CustomSky);
        r.note(format!("credits: active {}, end {end}", cs_bool(active)));
        let (mut t, mut k) = (0, 0);
        while t < end {
            (*sky)._current_time = t;
            r.frames(1);
            if k % 12 == 0 {
                r.record(&format!("credits_{t}"), 1);
            }
            t += 240;
            k += 1;
        }
        terraria::game_content::events::credits_roll_event::reset();
        r.frames(150);
    }
    // ---- a graveyard's wind
    if probe_group(r, "graveyard") {
        for k in 0..12 {
            place_tile(x0 + 4 + k * 3, gy - 1, 85, 0);
        }
        place_player(x0 + 20, gy - 1);
        r.frames(240);
        r.record("graveyard", 1);
        probe_fx(r, "graveyard");
        clear_area(x0, x0 + 45, gy);
        r.frames(60);
    }
    // ---- critters and kites on anchors
    if probe_group(r, "leashed") {
        use terraria::game_content::tile_entities::{te_critter_anchor as tca, te_kite_anchor as tka};
        let protos = (*tca::s()).critter_prototypes;
        let is_kite = (*terraria::id::item_id::sets::s()).is_a_kite;
        let (mut critters, mut kites) = (Vec::new(), Vec::new());
        let mut seen: Vec<Gc<terraria::game_content::leashed_entities::leashed_critter::LeashedCritter>> = Vec::new();
        let it = terraria::item::new__void();
        let count = (*terraria::id::item_id::s()).count as i32;
        for t in 1..count {
            if *is_kite.at(t) {
                if kites.len() < 6 {
                    kites.push(t);
                }
                continue;
            }
            terraria::item::set_defaults(it.ptr(), t, Gc::NULL);
            let mn = (*it.ptr()).make_npc as i32;
            if mn <= 0 {
                continue;
            }
            let proto = *protos.at(mn);
            if seen.contains(&proto) {
                continue;
            }
            seen.push(proto);
            critters.push(t);
        }
        r.note(format!(
            "leashed critters: {}; kites: {}",
            critters.iter().map(|t| t.to_string()).collect::<Vec<_>>().join(","),
            kites.iter().map(|t| t.to_string()).collect::<Vec<_>>().join(",")
        ));
        place_player(x0 + 30, gy - 1);
        for round in [critters.iter().map(|&t| (t, 724)).collect::<Vec<_>>(), kites.iter().map(|&t| (t, 723)).collect::<Vec<_>>()] {
            let mut ax = x0;
            for &(item, ty) in &round {
                give_item(0, item);
                select_item(0);
                let ok = terraria::world_gen::place_object(ax, gy - 1, ty, false, 0, 0, -1, 1);
                let (tx, ty2) = top_left_of(ty, ax, gy - 1);
                let id = if ty == 724 { tca::hook_after_placement(tx, ty2, ty, 0, 1, 0) } else { tka::hook_after_placement(tx, ty2, ty, 0, 1, 0) };
                r.note(format!("anchor {item} at {ax}: {} {tx},{ty2} id {id}", cs_bool(ok)));
                ax += 3;
            }
            give_item(0, 0);
            r.frames(60);
            r.record(&format!("leashed_{}_a", round[0].1), 1);
            r.frames(120);
            r.record(&format!("leashed_{}_b", round[0].1), 1);
            clear_area(x0 - 1, ax + 1, gy);
            r.frames(30);
        }
    }
    // ---- tile entities used by the player
    if probe_group(r, "tileentities") {
        use terraria::game_content::tile_entities as te;
        place_player(x0 + 2, gy - 1);
        let tx = x0 + 6;
        type Hook = unsafe fn(i32, i32, i32, i32, i32, i32) -> i32;
        type Inter = unsafe fn(Gc<terraria::player::Player>, i32, i32);
        let kinds: [(&str, i32, Hook, Inter); 4] = [
            ("TEItemFrame", 395, te::te_item_frame::hook_after_placement, te::te_item_frame::on_player_interaction),
            ("TEWeaponsRack", 471, te::te_weapons_rack::hook_after_placement, te::te_weapons_rack::on_player_interaction),
            ("TEFoodPlatter", 520, te::te_food_platter::hook_after_placement, te::te_food_platter::on_player_interaction),
            ("TEDeadCellsDisplayJar", 698, te::te_dead_cells_display_jar::hook_after_placement, te::te_dead_cells_display_jar::on_player_interaction),
        ];
        for (name, ty, hook, inter) in kinds {
            for x in tx - 2..=tx + 3 {
                for y in gy - 5..gy {
                    terraria::world_gen::place_wall(x, y, 4, true);
                }
            }
            if ty == 520 {
                place_tile(tx, gy - 1, 19, 0);
            }
            if ty == 698 {
                place_tile(tx, gy - 4, 30, 0);
            }
            let py = if ty == 698 { gy - 3 } else { gy - 2 };
            let ok = terraria::world_gen::place_object(tx, py, ty, false, 0, 0, -1, 1);
            let (lx, ly) = top_left_of(ty, tx + 1, py + 1);
            let id = hook(lx, ly, ty, 0, 1, 0);
            r.note(format!("{name}: {} at {lx},{ly} id {id} {}", cs_bool(ok), cover_diag(tx - 1, gy - 5, 5, 6)));
            assert!(lx >= 0, "{name} was not placed");
            give_item(0, if ty == 520 { 357 } else if ty == 471 { 3507 } else { 1 });
            select_item(0);
            (*pl.ptr()).item_time = 0;
            inter(pl, lx, ly);
            r.frames(3);
            let held = *(*pl.ptr()).inventory.at(0);
            r.probe(&format!("te_{name}_held"), &[&(*held.ptr()).type_, &(*held.ptr()).stack]);
            r.record(&format!("te_{name}"), 1);
            give_item(0, 0);
            (*pl.ptr()).item_time = 0;
            inter(pl, lx, ly);
            r.frames(3);
            let h = tile_hash(tx - 1, gy - 5, 5, 6);
            r.probe(&format!("te_{name}_emptied"), &[&h]);
            for x in tx - 2..=tx + 3 {
                for y in gy - 5..gy {
                    kill_tile(x, y);
                    terraria::world_gen::kill_wall(x, y, false);
                }
            }
            clear_entities();
            r.frames(3);
        }
        // a display doll and a hat rack
        let dx = x0 + 10;
        terraria::world_gen::place_object(dx, gy - 1, 470, false, 0, 0, -1, 1);
        let (dlx, dly) = top_left_of(470, dx + 1, gy - 1);
        let did = te::te_display_doll::hook_after_placement(dlx, dly, 470, 0, 1, 0);
        let doll = cover_entity(did).up::<te::te_display_doll::TEDisplayDoll>();
        r.note(format!("doll at {dlx},{dly} id {did}"));
        place_player(dx - 1, gy - 1);
        r.frames(2);
        (*m).player_inventory = true;
        te::te_display_doll::on_player_interaction(pl, dlx, dly);
        r.frames(3);
        r.record("te_doll_open", 1);
        let inv = (*pl.ptr()).inventory;
        let fit = [231, 232, 233, 1243, 3884];
        for (k, &f) in fit.iter().enumerate() {
            give_item(10 + k as i32, f);
        }
        (*m).cursor_override = 9;
        for (k, &f) in fit.iter().enumerate() {
            let ok = te::te_display_doll::perform_shift_click_action(doll.ptr(), inv, 0, 10 + k as i32);
            r.probe(&format!("doll_fit_{f}"), &[&ok]);
        }
        (*m).cursor_override = 0;
        for k in 0..3 {
            let ok = te::te_display_doll::try_change_pose(dlx, dly + 1);
            r.probe(&format!("doll_pose_{k}"), &[&ok, &(*doll.ptr())._pose]);
        }
        r.frames(3);
        r.record("te_doll_fitted", 1);
        te::te_display_doll::on_player_interaction(pl, dlx, dly);
        r.frames(3);
        let hrx = x0 + 16;
        terraria::world_gen::place_object(hrx, gy - 1, 475, false, 0, 0, -1, 1);
        let (hlx, hly) = top_left_of(475, hrx + 1, gy - 1);
        let hid = te::te_hat_rack::hook_after_placement(hlx, hly, 475, 0, 1, 0);
        let rack = cover_entity(hid).up::<te::te_hat_rack::TEHatRack>();
        place_player(hrx - 1, gy - 1);
        r.frames(2);
        te::te_hat_rack::on_player_interaction(pl, hlx, hly);
        r.frames(3);
        give_item(10, 1243);
        give_item(11, 3884);
        give_item(12, 869);
        (*m).cursor_override = 9;
        for k in 0..3 {
            let ok = te::te_hat_rack::perform_shift_click_action(rack.ptr(), inv, 0, 10 + k);
            r.probe(&format!("hatrack_fit_{k}"), &[&ok]);
        }
        (*m).cursor_override = 0;
        r.frames(3);
        r.record("te_hatrack", 1);
        te::te_hat_rack::on_player_interaction(pl, hlx, hly);
        (*m).player_inventory = false;
        for k in 10..15 {
            give_item(k, 0);
        }
        r.frames(3);
        // a training dummy
        let ddx = x0 + 24;
        terraria::world_gen::place_object(ddx, gy - 1, 378, false, 0, 0, -1, 1);
        let (tlx, tly) = top_left_of(378, ddx + 1, gy - 1);
        let tid = te::te_training_dummy::hook_after_placement(tlx, tly, 378, 0, 1, 0);
        r.note(format!("dummy at {tlx},{tly} id {tid}"));
        place_player(ddx - 2, gy - 1);
        r.frames(30);
        r.record("te_dummy", 1);
        let dummy = cover_entity(tid).up::<te::te_training_dummy::TETrainingDummy>();
        r.probe("dummy_npc", &[&(*dummy.ptr()).npc]);
        // everything taken down again
        for arr in [(*doll.ptr())._equip, (*doll.ptr())._dyes, (*doll.ptr())._misc, (*rack.ptr())._items, (*rack.ptr())._dyes] {
            for k in 0..arr.len() {
                let item = *arr.at(k);
                if !item.is_null() {
                    terraria::item::turn_to_air(item.ptr());
                }
            }
        }
        for x in x0..=x0 + 30 {
            for y in gy - 5..gy {
                kill_tile(x, y);
            }
        }
        clear_entities();
        r.frames(10);
    }
}

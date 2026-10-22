// The probe run, part D: tools/refdump_draw/ProbeD.cs, step for step. Included by tests/draw.rs.

/// ProbeD.cs `ClearItems`.
unsafe fn clear_items() {
    for wi in (*terraria::main::s()).item.as_slice() {
        if !wi.is_null() && terraria::world_item::active(wi.ptr()) {
            terraria::world_item::turn_to_air(wi.ptr());
        }
    }
}
/// ProbeD.cs `CountItem`.
unsafe fn count_item(ty: i32) -> i32 {
    let inv = (*local_player().ptr()).inventory;
    (0..inv.len()).map(|k| *inv.at(k)).filter(|it| (*it.ptr()).type_ == ty).map(|it| (*it.ptr()).stack).sum()
}

/// ProbeD.cs `ProbeD`.
unsafe fn probe_d(r: &mut Run, sx: i32, sy: i32, x0: i32, gy: i32) {
    let m = terraria::main::s();
    let pl = local_player();
    // ---- crafting
    if probe_group(r, "craft") {
        place_player(sx, sy);
        set_inv(0, 9, 30);
        (*m).player_inventory = true;
        terraria::recipe::update_recipe_list();
        r.frames(2);
        let recipes = (*m).recipe;
        let mut wb = -1;
        for i in 0..recipes.len() {
            let rc = *recipes.at(i);
            if !rc.is_null() && (*(*rc.ptr()).create_item.ptr()).type_ == 36 {
                wb = i;
                break;
            }
        }
        r.note(format!("work bench recipe {wb}"));
        terraria::game_content::crafting_requests::craft_item(*recipes.at(wb), 1, false);
        let mi = (*m).mouse_item;
        r.probe("craft_wood", &[&count_item(9), &(*mi.ptr()).type_, &(*mi.ptr()).stack]);
        terraria::recipe::update_recipe_list();
        let focus = (*m).focus_recipe;
        r.note(format!("focus recipe {focus} of {}", (*m).num_available_recipes));
        terraria::main::hover_over_crafting_item_button(focus);
        r.frames(2);
        r.record("craft", 1);
        terraria::item::turn_to_air((*(*pl.ptr()).inventory.at(0)).ptr());
        terraria::item::turn_to_air((*m).mouse_item.ptr());
        (*m).player_inventory = false;
        r.frames(2);
    }
    // ---- coins taken back from enemies
    if probe_group(r, "revenge") {
        clear_entities();
        place_player(sx, sy);
        r.frames(2);
        let pp = (*pl.ptr()).base.position;
        let rm = (*terraria::npc::s()).revenge_manager;
        let min = (*terraria::game_content::coin_loss_revenge_system::s()).minimum_coins_for_caching;
        for (ty, dx) in [(3, 900), (1, 1300), (2, -1100), (489, 1600)] {
            let id = new_npc_at(pp.x as i32 + dx, pp.y as i32, ty);
            let n = npc_at(id);
            (*n.ptr()).extra_value = min * 3;
            terraria::game_content::coin_loss_revenge_system::cache_enemy(rm.ptr(), n);
            (*n.ptr()).active = false;
        }
        r.frames(3);
        for k in 0..6 {
            place_player(sx + k * 20, sy);
            r.frames(5);
        }
        r.record("revenge", 1);
        clear_entities();
        place_player(sx, sy);
        r.frames(3);
    }
    // ---- journey-mode helpers
    if probe_group(r, "creative") {
        use terraria::game_content::creative::creative_powers::modify_wind_direction_and_strength as wind;
        let w = wind::new();
        let el = terraria::ui::ui_element::new();
        let _roots = (rt::Rooted::new(w), rt::Rooted::new(el));
        (*el.ptr()).is_mouse_hovering__backing = true;
        for v in [-0.5f32, 0.0, 0.37] {
            (*m).wind_speed_current = v;
            wind::update_slider_and_show_multiplier_mouse_over(w.ptr(), el);
            r.frames(1);
        }
        r.record("creative_wind", 1);
        use terraria::game_content::creative::item_filters::by_search as bs;
        let search = bs::new();
        bs::set_search(search.ptr(), s!("sword"));
        for t in [1, 4, 9, 46, 121, 757, 3063] {
            let ok = bs::fits_filter(search.ptr(), cover_new_item(t));
            r.probe(&format!("bysearch_{t}"), &[&ok]);
        }
        (*m).wind_speed_current = 0.0;
    }
    // ---- the constant world's darkness
    if probe_group(r, "dontstarve") {
        (*m).dont_starve_world = true;
        set_time(false, 9000.0);
        place_player(1999, 547);
        (*pl.ptr()).immune = false;
        (*pl.ptr()).immune_time = 0;
        r.frames(300);
        r.record("dontstarve_dark", 1);
        place_player(sx, sy);
        r.frames(10);
        r.record("dontstarve_night", 1);
        (*m).dont_starve_world = false;
        gallery_immune();
        (*pl.ptr()).stat_life = 100;
        set_time(true, 27000.0);
        r.frames(5);
    }
    // ---- doors that open by themselves
    if probe_group(r, "doors") {
        let doh = terraria::game_content::door_opening_helper::s();
        let old = (*doh).preference_settings;
        (*doh).preference_settings = 2;
        let (d1, d2) = (x0 + 10, x0 + 20);
        for x in x0..=x0 + 30 {
            place_tile(x, gy - 6, 30, 0);
        }
        place_tile(d1, gy - 4, 30, 0);
        place_tile(d1, gy - 5, 30, 0);
        let (a, b) = (place_tile(d1, gy - 1, 10, 0), place_tile(d2, gy - 1, 388, 0));
        r.note(format!("door: {}, gate: {} {} / {}", cs_bool(a), cs_bool(b), cover_diag(d1 - 1, gy - 6, 3, 6), cover_diag(d2 - 1, gy - 6, 3, 6)));
        place_player(x0 + 4, gy - 1);
        r.frames(2);
        r.input.keys = vec![68];
        for k in 0..8 {
            r.frames(10);
            let (h1, h2) = (tile_hash(d1, gy - 5, 2, 5), tile_hash(d2, gy - 6, 2, 6));
            r.probe(&format!("doors_r_{k}"), &[&(*pl.ptr()).base.position, &h1, &h2]);
        }
        r.record("doors_right", 1);
        r.input.keys = vec![65];
        for k in 0..8 {
            r.frames(10);
            let (h1, h2) = (tile_hash(d1, gy - 5, 2, 5), tile_hash(d2, gy - 6, 2, 6));
            r.probe(&format!("doors_l_{k}"), &[&(*pl.ptr()).base.position, &h1, &h2]);
        }
        r.input.keys = Vec::new();
        r.frames(30);
        r.record("doors_left", 1);
        (*doh).preference_settings = old;
        clear_area(x0, x0 + 30, gy);
        r.frames(5);
    }
    // ---- lava clouds, living tree leaves, furniture flames
    if probe_group(r, "tiledraw") {
        place_player(x0 + 20, gy - 1);
        for x in x0 + 4..x0 + 14 {
            place_tile(x, gy - 8, 717, 0);
        }
        for x in x0 + 18..x0 + 30 {
            place_tile(x, gy - 8, 192, 0);
        }
        (*m).wind_speed_current = 0.8;
        (*m).wind_speed_target = 0.8;
        r.frames(120);
        r.record("tiledraw_wind_e", 1);
        (*m).wind_speed_current = -0.8;
        (*m).wind_speed_target = -0.8;
        r.frames(120);
        r.record("tiledraw_wind_w", 1);
        (*m).wind_speed_current = 0.0;
        (*m).wind_speed_target = 0.0;
        clear_area(x0, x0 + 34, gy);
        set_time(false, 3600.0);
        for (ty, hang) in [(33, false), (174, false), (100, false), (173, false), (93, false), (34, true), (42, true)] {
            let mut style = 0;
            while style < 72 {
                for x in x0..=x0 + 40 {
                    place_tile(x, gy - 7, 30, 0);
                }
                let on_table = ty == 33 || ty == 174;
                if on_table {
                    for x in x0..=x0 + 40 {
                        place_tile(x, gy - 1, 19, 0);
                    }
                }
                let mut placed = Vec::new();
                for k in 0..12 {
                    let x = x0 + 2 + k * 3;
                    if terraria::object_data::tile_object_data::get_tile_data__int_int_int(ty, style + k, 0).is_null() {
                        continue;
                    }
                    let y = if hang { gy - 6 } else if on_table { gy - 2 } else { gy - 1 };
                    if terraria::world_gen::place_object(x, y, ty, true, style + k, 0, -1, 1) {
                        placed.push((style + k).to_string());
                    }
                }
                r.frames(3);
                r.note(format!("flames {ty} styles {}", placed.join(",")));
                r.record(&format!("flames_{ty}_{style}"), 1);
                clear_area(x0, x0 + 40, gy);
                style += 12;
            }
        }
        set_time(true, 27000.0);
        r.frames(5);
    }
    // ---- town slimes transforming
    if probe_group(r, "slimefx") {
        let pp = (*pl.ptr()).base.position;
        clear_fx();
        for u in 0..3 {
            gallery_particle(26, pp.x + 40.0 * u as f32, pp.y - 30.0, 1.0, -2.0, u);
        }
        probe_fx(r, "townslime");
        r.frames(3);
        r.record("townslime", 1);
    }
    // ---- the world item cap
    if probe_group(r, "itemcap") {
        clear_items();
        place_player(sx, sy);
        let pp = (*pl.ptr()).base.position;
        for k in 0..410 {
            interact_new_item(pp.x as i32 - 300 + (k % 40) * 15, pp.y as i32 - 200 + (k / 40) * 10, if k % 2 == 0 { 2 } else { 3 }, 1);
        }
        let active = (*m).item.as_slice().iter().filter(|wi| terraria::world_item::active(wi.ptr())).count() as i32;
        r.probe("itemcap_active", &[&active]);
        r.frames(10);
        r.record("itemcap", 1);
        clear_items();
        r.frames(2);
    }
    // ---- small direct calls
    if probe_group(r, "direct1") {
        use terraria::data_structures::player_intention_guesser as pig;
        let ig = (*pl.ptr()).intention_guesser;
        for (it, w, h) in [(10, 15, 5), (1, 15, 15), (3509, 4, 4)] {
            pig::prepare_usage_proxy(ig.ptr(), pl, it, w, h);
            let up = &*(*ig.ptr()).usage_proxy.ptr();
            r.probe(&format!("usageproxy_{it}"), &[&up.screen_target_x, &up.screen_target_y, &up.reachable_start_x, &up.reachable_start_y, &up.reachable_end_x, &up.reachable_end_y]);
        }
        terraria::game_content::achievements::achievements_helper::handle_nurse_service(1234);
        let mut bx = x0 + 120;
        for b in ["SpikePitBiome", "CorruptionPitBiome"] {
            for oy in [gy - 30, 400, 650] {
                let sm = terraria::world_building::structure_map::new();
                let gp = terraria::world_building::generation_progress::new();
                r.probe_tiles(&format!("biome_{b}_{oy}_before"), bx - 30, oy - 10, 60, 120);
                let origin = terraria_core::xna::Point { x: bx, y: oy };
                let ok = if b == "SpikePitBiome" {
                    let bio = terraria::game_content::biomes::spike_pit_biome::new();
                    terraria::game_content::biomes::spike_pit_biome::place(bio.ptr(), origin, sm, gp)
                } else {
                    let bio = terraria::game_content::biomes::corruption_pit_biome::new();
                    terraria::game_content::biomes::corruption_pit_biome::place(bio.ptr(), origin, sm, gp)
                };
                r.probe(&format!("biome_{b}_{oy}"), &[&ok]);
                r.probe_tiles(&format!("biome_{b}_{oy}_after"), bx - 30, oy - 10, 60, 120);
            }
            bx += 80;
        }
        use terraria::graphics::effects::effect_manager__custom_sky as em;
        let mgr = (*terraria::graphics::effects::sky_manager::s()).instance;
        let sky = em::get_item(mgr.ptr() as *mut em::EffectManager, s!("CreditsRoll")).0 as *mut terraria::game_content::skies::credits_roll_sky::CreditsRollSky;
        let end = (*sky)._end_time;
        (*m).game_menu = true;
        let (mut t, mut k) = (0, 0);
        while t < end {
            (*sky)._current_time = t;
            let draw = || in_batch(|| terraria::game_content::skies::credits_roll_sky::draw(sky, (*terraria::main::s()).sprite_batch, 0.0, 2.0));
            if k % 8 == 0 {
                r.direct(&format!("menucredits_{t}"), |_| draw());
            } else {
                draw();
            }
            t += 600;
            k += 1;
        }
        (*m).game_menu = false;
        (*sky)._current_time = 0;
        r.frames(3);
    }
}

// The probe run, part K: tools/refdump_draw/ProbeK.cs, step for step. Included by tests/draw.rs.

/// ProbeK.cs `TileActive`.
unsafe fn tile_obj_active(t: Gc<terraria::tile::Tile>) -> bool {
    !t.is_null() && terraria::tile::active__void(t.ptr())
}
/// ProbeK.cs `ClearPlayerBuffs`.
unsafe fn clear_player_buffs() {
    let p = local_player().ptr();
    for k in 0..(*p).buff_type.len() {
        *(*p).buff_type.at(k) = 0;
        *(*p).buff_time.at(k) = 0;
    }
}
/// ProbeK.cs `ClearLiquid`.
unsafe fn clear_liquid(x: i32, y: i32, w: i32, h: i32) {
    for i in x..x + w {
        for j in y..y + h {
            let t = gallery_tile(i, j);
            if !t.is_null() {
                (*t.ptr()).liquid = 0;
            }
        }
    }
}
/// ProbeK.cs `CountProj`.
unsafe fn count_proj(ty: i32) -> i32 {
    (*terraria::main::s()).projectile.as_slice().iter().filter(|p| (*p.ptr()).active && (*p.ptr()).type_ == ty).count() as i32
}

/// ProbeK.cs `ProbeK`.
unsafe fn probe_k(r: &mut Run, sx: i32, sy: i32, x0: i32, gy: i32) {
    use terraria::projectile as J;
    use terraria::utils as U;
    use terraria::world_gen as W;
    use terraria_core::xna::{Color, Point};
    let m = terraria::main::s();
    let pl = local_player();
    let p_ = pl.ptr();
    let me = (*m).my_player;
    // ---- projectile helpers
    if probe_group(r, "projdirect") {
        clear_entities();
        clear_fx();
        place_player(x0 + 2, gy - 1);
        r.frames(2);
        let pp = (*p_).base.position;
        let z = npc_near(3, 80.0, 0.0);
        let zi = (*z.ptr()).base.who_am_i;
        let zc = npc_center(z);
        let p = proj_at(gallery_new_proj(pp.x + 40.0, pp.y - 40.0, 0.0, 0.0, 360));
        J::ai_061_fishing_bobber_do_a_splash(p.ptr());
        let p = proj_at(gallery_new_proj(pp.x, pp.y - 40.0, 0.0, 0.0, 388));
        J::ai_066_try_intercepting_target(p.ptr(), v2(80.0, -10.0), v2(1.5, 0.0), 12.0);
        r.probe("intercept", &[&(*p.ptr()).base.velocity]);
        let pirate = proj_at(gallery_new_proj(pp.x + 30.0, pp.y, 0.0, 0.0, 1093));
        J::ai_067_freaking_pirates_hit_intention(pirate.ptr(), v2(((x0 + 3) * 16 + 8) as f32, (gy * 16 + 8) as f32));
        r.probe_tiles("pirate_hit", x0 + 2, gy - 1, 3, 3);
        let p = proj_at(gallery_new_proj(pp.x - 30.0, pp.y, 0.0, 0.0, 1022));
        J::ai_067_freaking_pirates_try_jumping_to_target(p.ptr(), v2(0.0, 0.0), v2(pp.x, pp.y - 200.0), v2(pp.x, pp.y - 200.0));
        r.probe("pirate_jump", &[&(*p.ptr()).base.velocity]);
        let p = proj_at(gallery_new_proj(pp.x - 10.0, pp.y, 0.0, 0.0, 834));
        let tg = J::ai_067_tiger_special_attack(p.ptr());
        r.probe("tiger", &[&tg, &(*p.ptr()).base.velocity]);
        J::ai_155_spawn_rope_in(p.ptr(), x0 + 5, gy - 3);
        J::ai_198_flint_emit_spikes(p.ptr(), pp, 1);
        J::betsy_sharpnel(p.ptr(), zi);
        J::boulder_explosion(p.ptr());
        (*p_).item_animation_max = 30;
        let whip = proj_at(gallery_new_proj(pp.x, pp.y, 0.0, 0.0, 847));
        *(*whip.ptr()).ai.at(0) = 10.0;
        J::constellation_whip_spawn_constellation_stars_towards(whip.ptr(), z, 50);
        J::spawn_moon_lord_whip_proc(whip, z, 30, 0);
        (*p_).item_animation_max = 0;
        let origin = v2(((x0 + 5) * 16) as f32, (gy * 16) as f32);
        let (mut a0, mut a1, mut shock) = (Point { x: x0 + 2, y: gy - 2 }, Point { x: x0 + 8, y: gy + 1 }, false);
        J::create_impact_explosion(p.ptr(), 1, origin, &mut a0, &mut a1, 80, &mut shock);
        r.probe("impact", &[&a0, &a1, &shock]);
        J::create_impact_explosion2_flail_tile_collision(p.ptr(), origin, true, v2(3.0, 4.0));
        for x in x0 + 12..=x0 + 14 {
            place_tile(x, gy - 1, 481, 0);
        }
        J::explode_cracked_tiles(p.ptr(), v2(((x0 + 13) * 16) as f32, ((gy - 1) * 16) as f32), 3, x0 + 10, x0 + 16, gy - 3, gy);
        r.probe_tiles("cracked", x0 + 10, gy - 3, 7, 4);
        let _ = J::get_petting_info(pirate.ptr(), pl);
        for k in 0..3 {
            let jv = proj_at(gallery_new_proj(zc.x, zc.y, 0.0, 0.0, 598));
            *(*jv.ptr()).ai.at(0) = 1.0;
            *(*jv.ptr()).ai.at(1) = zi as f32;
            (*jv.ptr()).time_left = 100 + k * 50;
        }
        J::kill_oldest_javelin(-1, 598, zi, Arr::<Point>::new(2));
        r.probe("javelins", &[&count_proj(598)]);
        let lb = proj_at(gallery_new_proj(((x0 + 20) * 16) as f32, ((gy - 3) * 16) as f32, 0.0, 0.0, 1));
        J::lava_boulder_explosion(lb.ptr());
        r.probe_tiles("lava", x0 + 15, gy - 8, 11, 9);
        clear_liquid(x0 + 15, gy - 8, 11, 9);
        let st = proj_at(gallery_new_proj(pp.x, pp.y - 100.0, 0.0, 0.0, 856));
        let mut dmg = 100i32;
        J::modify_stellar_tune_damage(st.ptr(), &mut dmg);
        r.probe("stellartune", &[&dmg]);
        let (mut sp, mut sv, mut i0, mut i1) = (v2(0.0, 0.0), v2(0.0, 0.0), 0.0f32, 0.0f32);
        J::randomize_insanity_shadow_for(pl.up::<terraria::entity::Entity>(), true, &mut sp, &mut sv, &mut i0, &mut i1);
        r.probe("insanity", &[&sp, &sv, &i0, &i1]);
        J::spawn_lightning_liquid_electrification_particles__void(p.ptr());
        let web = proj_at(gallery_new_proj(((x0 + 30) * 16) as f32, ((gy - 4) * 16) as f32, 0.0, 0.0, 1));
        J::spawn_webs(web.ptr());
        r.probe_tiles("webs", x0 + 23, gy - 11, 15, 12);
        (*p_).melee_enchant = 1;
        (*p.ptr()).melee = true;
        J::status_pv_p(p.ptr(), me);
        (*p_).melee_enchant = 0;
        r.probe("statuspvp", &[&*(*p_).buff_type.at(0)]);
        clear_player_buffs();
        J::summon_monk_ghast(p.ptr());
        J::summon_super_star_slash(p.ptr(), zc);
        let mut sdmg = 50i32;
        J::try_making_snapping_stone_strike(src(), pl.up::<terraria::entity::Entity>(), me, z, &mut sdmg, 0);
        r.probe("snapping", &[&sdmg]);
        let cx = x0 + 34;
        place_tile(cx, gy - 1, 21, 0);
        let ci = terraria::chest::find_chest(cx, gy - 2);
        r.probe("gaschest", &[&ci]);
        if ci >= 0 {
            let chest = *(*m).chest.at(ci);
            let citems = (*chest.ptr()).item;
            terraria::item::set_defaults((*citems.at(0)).ptr(), 9, Gc::NULL);
            (*(*citems.at(0)).ptr()).stack = 2;
            J::use_gas_trap_in_chest(ci, chest, 0, 0, 0);
            r.probe("gastrap", &[&(*(*citems.at(0)).ptr()).stack, &(*(*citems.at(1)).ptr()).type_]);
            for k in 0..citems.len() {
                terraria::item::turn_to_air((*citems.at(k)).ptr());
            }
        }
        (*p_).life_steal = 100.0;
        (*p.ptr()).magic = true;
        (*p.ptr()).damage = 40;
        J::ghost_heal(p.ptr(), 50, zc, z.up::<terraria::entity::Entity>());
        J::ghost_hurt(p.ptr(), 50, zc, z.up::<terraria::entity::Entity>());
        J::vampire_heal(p.ptr(), 50, zc, z.up::<terraria::entity::Entity>());
        r.probe("heals", &[&(*p_).life_steal, &(*p_).ghost_dmg]);
        probe_fx(r, "projdirect");
        gallery_immune();
        (*p_).stat_life = 100;
        r.state.push_str(&state_line("projdirect"));
        r.frames(3);
        r.record("projdirect", 1);
        clear_entities();
        clear_fx();
        clear_items();
        clear_area(x0, x0 + 40, gy);
        clear_liquid(x0, gy - 12, 41, 12);
        r.frames(2);
    }
    // ---- Utils helpers
    if probe_group(r, "utilsdirect") {
        clear_entities();
        clear_fx();
        place_player(sx, sy);
        r.frames(2);
        let pp = (*p_).base.position;
        let z0 = v2(0.0, 0.0);
        let (a, b) = (U::does_fit_in_cone(v2(100.0, 20.0), z0, 200.0, 0.1, 0.6), U::does_fit_in_cone(v2(-50.0, 80.0), z0, 200.0, 0.1, 0.6));
        r.probe("fitcone", &[&a, &b]);
        let (a, b) = (U::intersects_cone_fast_inaccurate(rect(90, 0, 20, 20), z0, 200.0, 0.2, 0.5), U::intersects_cone_fast_inaccurate(rect(-90, 60, 20, 20), z0, 200.0, 0.2, 0.5));
        r.probe("conefast", &[&a, &b]);
        r.probe("easeincirc", &[&U::ease_in_circ(0.3), &U::ease_in_circ(0.85)]);
        r.probe("jump", &[&U::get_jump_force(64.0, 0.3), &U::get_jump_time_to_apex(64.0, 0.3)]);
        r.probe("pingpong", &[&U::ping_pong_from01_to010(0.3), &U::ping_pong_from01_to010(0.8), &U::turn01_to_cyclic010(0.3), &U::turn01_to_cyclic010(0.7)]);
        let mut seed = 12345678901u64;
        let rf = U::random_float(&mut seed);
        r.probe("randomfloat", &[&rf, &seed]);
        r.probe("shifthue", &[&U::shift_hue(Color::new(200, 100, 50, 255), 0.3, 0.1, 0.2)]);
        let (mut q3, mut q4) = (0.0f32, 0.0f32);
        let ok = U::solve_quadratic(1.0, -3.0, 2.0, &mut q3, &mut q4);
        r.probe("quadratic", &[&ok, &q3, &q4]);
        let (mut q3, mut q4) = (0.0f32, 0.0f32);
        let ok = U::solve_quadratic(1.0, 0.0, 5.0, &mut q3, &mut q4);
        r.probe("quadratic_none", &[&ok, &q3, &q4]);
        U::poof_of_smoke(pp);
        probe_fx(r, "poof");
        clear_fx();
        let sb = (*m).sprite_batch;
        r.direct("utils_lines", |_| {
            in_batch(|| {
                U::draw_line__sprite_batch_vector2_vector2_color(sb, pp, pp + v2(120.0, -40.0), Color::RED);
                U::draw_line__sprite_batch_point_point_color(sb, Point { x: sx, y: sy }, Point { x: sx + 5, y: sy - 3 }, Color::YELLOW);
                U::draw_line__sprite_batch_vector2_vector2_color_color_float(sb, pp, pp + v2(100.0, -50.0), Color::RED, Color::BLUE, 2.0);
                U::draw_rect__sprite_batch_rectangle_color(sb, rect(sx, sy - 4, 4, 3), Color::GREEN);
            })
        });
        r.state.push_str(&state_line("utilsdirect"));
        r.frames(2);
    }
    // ---- WorldGen helpers
    if probe_group(r, "wgdirect") {
        clear_entities();
        clear_fx();
        place_player(x0 + 2, gy - 1);
        r.frames(2);
        let w = (W::place_wire2(x0 + 5, gy - 2), W::place_wire3(x0 + 6, gy - 2), W::place_wire4(x0 + 7, gy - 2), W::place_wire2(x0 + 5, gy - 2));
        r.probe("wires", &[&w.0, &w.1, &w.2, &w.3]);
        r.probe("gate", &[&place_tile(x0 + 10, gy - 1, 388, 0)]);
        let (a, b) = (W::shift_tall_gate(x0 + 10, gy - 1, false, false), W::shift_tall_gate(x0 + 10, gy - 1, true, false));
        r.probe("tallgate", &[&a, &b]);
        r.probe_tiles("tallgate", x0 + 9, gy - 6, 3, 6);
        W::shoot_from_cannon(x0 + 14, gy - 3, 2, 1, 50, 4.0, me, false);
        W::shoot_from_cannon(x0 + 14, gy - 3, 0, 4, 0, 0.0, me, true);
        W::spawn_plantera_thorns(v2(((x0 + 20) * 16) as f32, ((gy - 6) * 16) as f32));
        r.probe_tiles("thorns", x0, gy - 12, 41, 12);
        for x in x0 + 22..=x0 + 26 {
            for y in gy - 5..gy {
                W::place_wall(x, y, 4, true);
            }
        }
        r.probe("gemlock", &[&W::place_object(x0 + 24, gy - 2, 440, true, 0, 0, -1, 1)]);
        W::toggle_gem_lock(x0 + 24, gy - 2, true);
        W::toggle_gem_lock(x0 + 24, gy - 2, false);
        r.probe_tiles("gemlock", x0 + 22, gy - 5, 5, 5);
        let (mut tx, mut ty) = (-1i32, -1i32);
        let is5 = |t: Gc<terraria::tile::Tile>| tile_obj_active(t) && (*t.ptr()).type_ == 5;
        let mut x = sx - 80;
        while x <= sx + 80 && tx < 0 {
            for y in sy - 40..=sy + 20 {
                let (t, up, dn) = (gallery_tile(x, y), gallery_tile(x, y - 1), gallery_tile(x, y + 1));
                if is5(t) && is5(up) && tile_obj_active(dn) && (*dn.ptr()).type_ != 5 {
                    tx = x;
                    ty = y;
                    break;
                }
            }
            x += 1;
        }
        r.probe("tree", &[&tx, &ty]);
        if tx >= 0 {
            W::tree_grow_fx_check(tx, ty);
            W::convert_tree_and_ground(tx, ty, 2, 5, 5);
        }
        W::tree_grow_fx_ash_shake(x0 + 28, gy - 1, 8, false);
        W::tree_grow_fx_ash_shake(x0 + 28, gy - 1, 8, true);
        place_tile(x0 + 32, gy - 1, 10, 11);
        W::unlock_door(x0 + 32, gy - 1);
        r.probe_tiles("door", x0 + 31, gy - 4, 3, 4);
        W::paint_effect(x0 + 5, gy, 3, 0);
        W::paint_effect(x0 + 6, gy, 0, 7);
        W::paint_coat_effect(x0 + 7, gy, 1, List::<Color>::new_gc());
        let cols = List::<Color>::new_gc();
        (*cols.ptr()).add(Color::RED);
        (*cols.ptr()).add(Color::BLUE);
        W::paint_coat_effect(x0 + 8, gy, 0, cols);
        W::spider_wall_remover(x0 + 60, gy + 30);
        r.probe_tiles("spiderwalls", x0 + 30, gy, 61, 61);
        let safe = List::<terraria_core::xna::Rectangle>::new_gc();
        let mut skip = false;
        let ok = W::try_spawn_lightning_bolt__list_rectangle_int_int_ref_bool(safe, x0 + 8, gy - 1, &mut skip);
        r.probe("lightning", &[&ok, &skip]);
        W::spawn_storm_lightning_near_player(safe, pl);
        r.probe("lightning_proj", &[&count_proj(1091)]);
        npc_near(368, 200.0, 0.0);
        W::unspawn_travel_npc();
        let tn = (*m).npc.as_slice().iter().filter(|n| (*n.ptr()).active && (*n.ptr()).type_ == 368).count() as i32;
        r.probe("travelnpc", &[&tn]);
        r.probe("planterabulb", &[&W::generate_plantera_bulb_on_all_mechs_defeated()]);
        probe_fx(r, "wgdirect");
        r.probe("wg_projs", &[&proj_list()]);
        r.state.push_str(&state_line("wgdirect"));
        r.frames(3);
        r.record("wgdirect", 1);
        r.probe("wg_projs3", &[&proj_list()]);
        clear_entities();
        clear_fx();
        clear_items();
        clear_area(x0, x0 + 40, gy);
        r.frames(2);
    }
}

// The probe run, part J: tools/refdump_draw/ProbeJ.cs, step for step. Included by tests/draw.rs.

/// ProbeJ.cs `ProjList`.
unsafe fn proj_list() -> String {
    let ps = (*terraria::main::s()).projectile;
    let mut out = String::new();
    for i in 0..ps.len() {
        let p = *ps.at(i);
        if !(*p.ptr()).active {
            continue;
        }
        out.push_str(&format!("{}:{}:{}:{};", i, (*p.ptr()).type_, (*p.ptr()).scale.pv(), (*p.ptr()).base.position.pv()));
    }
    out
}

/// ProbeJ.cs `ProbeJ`.
unsafe fn probe_j(r: &mut Run, sx: i32, sy: i32, x0: i32, gy: i32) {
    use terraria::player as P;
    let m = terraria::main::s();
    let pl = local_player();
    let p = pl.ptr();
    // ---- combat helpers
    if probe_group(r, "playercombat") {
        clear_entities();
        place_player(sx, sy);
        r.frames(2);
        let pp = (*p).base.position;
        let z = npc_near(3, 60.0, 0.0);
        let zi = (*z.ptr()).base.who_am_i;
        P::apply_damage_to_npc__npc_int_float_int_bool_string_int_int(p, z, 25, 2.0, 1, false, s!("probe"), 0, 0);
        r.probe("applydamage", &[&(*z.ptr()).life]);
        (*p).counter_weight = 556;
        let yo = gallery_new_proj(pp.x, pp.y, 0.0, 0.0, 541);
        *(*proj_at(yo).ptr()).ai.at(0) = 0.0;
        P::counterweight(p, npc_center(z), 20, 3.0);
        (*p).counter_weight = 0;
        let blood = cover_new_item(5094);
        (*p)._spawn_blood_butcherer = true;
        P::blood_butcherer_try_butchering(p, z, blood, 30.0, 2.0);
        r.probe("butcher", &[&*(*z.ptr()).buff_type.at(0)]);
        let vol = cover_new_item(5097);
        (*p)._spawn_volcano_explosion = true;
        P::volcano_try_spawning_volcano(p, z, vol, 40.0, 3.0, rect(pp.x as i32, pp.y as i32, 40, 40));
        P::horsemans_blade_spawn_pumpkin(p, zi, 30, 2.0);
        let jelly = npc_near(63, -60.0, 0.0);
        P::take_damage_from_jellyfish(p, (*jelly.ptr()).base.who_am_i);
        r.probe("jelly", &[&(*p).stat_life]);
        (*p).immune = false;
        let c = P::collide_with_np_cs(p, rect(pp.x as i32 - 40, pp.y as i32 - 20, 120, 60), 20.0, 4.0, 10, 10, 0);
        r.probe("collide", &[&c]);
        let (a, b) = (P::can_defend_with_paladins_shield(p, 0), P::can_defend_with_paladins_shield(p, 1));
        r.probe("paladin", &[&a, &b]);
        let (mut dmg, mut kb) = (0i32, 0.0f32);
        P::get_minecart_damage(p, 8.0, &mut dmg, &mut kb);
        r.probe("minecartdamage", &[&dmg, &kb]);
        gallery_immune();
        (*p).stat_life = 100;
        r.state.push_str(&state_line("playercombat"));
        r.frames(3);
        r.record("playercombat", 1);
        clear_entities();
        r.frames(2);
    }
    // ---- movement and placement helpers
    if probe_group(r, "playermove") {
        clear_entities();
        place_player(x0 + 10, gy - 1);
        r.frames(2);
        let pp = (*p).base.position;
        let (a, b) = (P::is_within_snappng_range_to_tile(p, x0 + 12, gy - 1, 100), P::is_within_snappng_range_to_tile(p, x0 + 40, gy - 1, 100));
        r.probe("snaprange", &[&a, &b]);
        let mut snap = true;
        P::try_allowing_snapping_to_position(p, &mut snap, pp, v2(pp.x + 30.0, pp.y));
        r.probe("snap", &[&snap]);
        for y in gy - 12..gy {
            place_tile(x0 + 14, y, 213, 0);
        }
        let (a, b) = (P::can_move_forward_on_rope(p, 1, x0 + 14, gy - 6), P::can_move_forward_on_rope(p, -1, x0 + 14, gy - 6));
        r.probe("rope", &[&a, &b]);
        let (a, b) = (P::can_fit_space(p, 0), P::can_fit_space(p, 3));
        r.probe("fitspace", &[&a, &b]);
        let (a, b) = (P::do_boots_effect_place_flames_on_tile(p, x0 + 11, gy - 1), P::do_boots_effect_place_flames_on_tile(p, x0 + 12, gy - 1));
        r.probe("bootsflames", &[&a, &b]);
        let paint = cover_new_item(1073);
        P::apply_paint(p, x0 + 9, gy, false, false, paint);
        let coat = cover_new_item(4668);
        P::apply_coating(p, x0 + 9, gy, false, false, coat);
        r.probe_tiles("paint", x0 + 8, gy - 1, 3, 3);
        place_tile(x0 + 18, gy - 1, 82, 0);
        P::try_replanting_herbs(p, 0);
        r.probe_tiles("herbs", x0 + 17, gy - 2, 3, 3);
        terraria::world_gen::place_object(x0 + 22, gy - 2, 471, true, 0, 0, -1, 1);
        give_item(0, 3507);
        select_item(0);
        P::place_weapon(p, x0 + 22, gy - 2);
        give_item(0, 0);
        place_tile(x0 + 26, gy - 1, 209, 0);
        P::shoot_from_cannon(p, x0 + 26, gy - 1);
        P::do_queen_slime_hook_teleport(p, v2(pp.x + 64.0, pp.y - 32.0));
        r.probe("qshook", &[&(*p).base.position]);
        P::do_dead_cells_ground_pound_effect(p);
        for x in x0 + 30..=x0 + 45 {
            place_tile(x, gy - 1, 314, 0);
        }
        place_player(x0 + 36, gy - 2);
        r.frames(2);
        P::launch_minecart_hook(p, x0 + 38, gy - 1);
        P::quick_minecart(p);
        let mt = (*p).mount.ptr();
        let (act, ty) = (terraria::mount::active(mt), terraria::mount::type_(mt));
        r.probe("quickminecart", &[&act, &ty]);
        if terraria::mount::active(mt) {
            terraria::mount::dismount(mt, pl, false);
        }
        r.state.push_str(&state_line("playermove"));
        r.frames(3);
        r.record("playermove", 1);
        clear_entities();
        clear_area(x0, x0 + 48, gy);
        r.frames(2);
    }
    // ---- small player helpers
    if probe_group(r, "playermisc") {
        clear_entities();
        place_player(sx, sy);
        r.frames(2);
        let pp = (*p).base.position;
        P::add_coin_luck(p, v2(pp.x + 20.0, pp.y), 12345);
        r.probe("coinluck", &[&(*p).coin_luck, &P::normalized_luck(p)]);
        P::adjust_respawn_timer_for_world_joining(pl);
        r.probe("respawn", &[&(*p).respawn_timer]);
        for t in [1, 46, 3507, 5000] {
            let (mut sell, mut buy) = (0i64, 0i64);
            P::get_item_expected_price(p, cover_new_item(t), &mut sell, &mut buy);
            r.probe(&format!("price_{t}"), &[&sell, &buy]);
        }
        let vinv = (*(*p).bank4.ptr()).item;
        terraria::item::set_defaults((*vinv.at(0)).ptr(), 9, Gc::NULL);
        (*(*vinv.at(0)).ptr()).stack = 5;
        let wood = cover_new_item(9);
        (*wood.ptr()).stack = 20;
        let settings = (*terraria::get_item_settings::s()).pickup_item_from_world;
        let ok = P::get_item_fill_into_occupied_slot_void_bag(p, vinv, wood, settings, cover_new_item(0), 0);
        r.probe("voidbag", &[&ok, &(*(*vinv.at(0)).ptr()).stack, &(*wood.ptr()).stack]);
        terraria::item::turn_to_air((*vinv.at(0)).ptr());
        P::try_apply_mana_heat(p, 0.5, 120.0);
        r.probe("manaheat", &[&(*p).mana_sick, &(*p).mana_sick_reduction]);
        (*p).slow_magic_use = true;
        P::undo_slow_magic_use(p);
        P::shadow_dodge(p);
        P::nebula_levelup(p, 178);
        r.probe("nebula", &[&(*p).nebula_level_damage, &(*p).nebula_level_life, &(*p).nebula_level_mana]);
        P::play_drums(p, 0.75);
        r.probe("shimmerfree", &[&P::try_finding_shimmer_free_spot(p)]);
        npc_near(3, 80.0, 0.0);
        r.aim(pp.x + 90.0, pp.y + 10.0);
        r.frames(1);
        P::minion_npc_target_aim(p, false);
        P::minion_rest_target_aim(p);
        r.probe("miniontarget", &[&(*p).minion_attack_target_npc, &(*p).minion_rest_target_point]);
        r.input.x = 640;
        r.input.y = 360;
        r.probe("spectate", &[&P::closest_spectatable_player_to(p, v2(pp.x, pp.y))]);
        P::try_to_poop(p);
        let (a, b) = (P::biome_campfire_hold_style(p, 4689), P::biome_campfire_hold_style(p, 966));
        r.probe("campfire_hold", &[&a, &b]);
        let (mut ct, mut cst) = (215i32, 0i32);
        P::biome_campfire_place_style(p, &mut ct, &mut cst);
        r.probe("campfire_place", &[&ct, &cst]);
        let rc = P::apply_range_compensation(p, 0.2, v2(pp.x, pp.y), v2(pp.x + 300.0, pp.y - 120.0));
        r.probe("rangecomp", &[&rc]);
        let reach = (*terraria::data_structures::tile_reach_check_settings::s()).simple;
        let (a, b) = (P::in_tile_entity_interaction_range(p, sx + 2, sy - 2, 2, 2, reach), P::in_tile_entity_interaction_range(p, sx + 60, sy - 2, 2, 2, reach));
        r.probe("tereach", &[&a, &b]);
        gallery_immune();
        (*p).stat_life = 100;
        r.state.push_str(&state_line("playermisc"));
        r.frames(3);
        r.record("playermisc", 1);
        clear_entities();
        r.frames(2);
    }
    // ---- the forbidden storm, a fishing bobber, an item pulled
    if probe_group(r, "playerstorm") {
        clear_entities();
        place_player(sx, sy);
        set_armor(0, 3776);
        set_armor(1, 3777);
        set_armor(2, 3778);
        r.frames(3);
        let pp = (*p).base.position;
        r.aim(pp.x + 100.0, pp.y - 60.0);
        r.frames(1);
        P::command_forbidden_storm(p);
        let ok = terraria::stray_methods::can_spawn_sandstorm_friendly(v2(pp.x + 100.0, pp.y), 3, 3);
        r.probe("sandstormfriendly", &[&ok]);
        r.frames(10);
        r.probe("storm_projs", &[&proj_list()]);
        r.record("forbiddenstorm", 1);
        r.probe("storm_projs2", &[&proj_list()]);
        for k in 0..3 {
            set_armor(k, 0);
        }
        r.input.x = 640;
        r.input.y = 360;
        set_inv(10, 2002, 5);
        let bob = gallery_new_proj(pp.x + 60.0, pp.y, 0.0, 0.0, 360);
        let bobber = proj_at(bob);
        *(*bobber.ptr()).ai.at(1) = -1.0;
        *(*bobber.ptr()).local_ai.at(1) = 2002.0;
        let mut bait = 0i32;
        let ok = P::item_check_check_fishing_bobber_consume_bait(p, bobber, &mut bait);
        r.probe("bait", &[&ok, &bait]);
        P::item_check_check_fishing_bobber_pull_bobber(p, bobber, 2002);
        give_item(10, 0);
        let wi = terraria::item::new_item__i_entity_source_int_int_int_int_int_int_bool_int_new_item_ownership_nullable_vector2_new_item_modifier(
            src(),
            pp.x as i32 + 150,
            pp.y as i32,
            16,
            16,
            9,
            1,
            false,
            0,
            0,
            rt::zero::<Nullable<terraria_core::xna::Vector2>>(),
            rt::zero::<rt::Delegate<dyn Fn(Gc<terraria::world_item::WorldItem>)>>(),
        );
        let w = *(*m).item.at(wi);
        P::pull_item_pickup(p, w, 12.0, 5);
        r.probe("pullitem", &[&(*w.ptr()).base.velocity]);
        clear_items();
        r.state.push_str(&state_line("playerstorm"));
        r.frames(3);
        clear_entities();
        r.frames(2);
    }
}
